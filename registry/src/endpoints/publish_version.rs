use std::{
    collections::BTreeSet,
    io::{Cursor, Read, Write},
};

use actix_multipart::Multipart;
use actix_web::{web, HttpResponse, Responder};
use flate2::read::GzDecoder;
use futures::StreamExt;
use git2::{Remote, Repository, Signature};
use reqwest::header::{CONTENT_ENCODING, CONTENT_TYPE};
use rusty_s3::{actions::PutObject, S3Action};
use tar::Archive;

use pesde::{
    manifest::Manifest,
    source::{
        git_index::GitBasedSource,
        pesde::{IndexFile, IndexFileEntry, ScopeInfo, SCOPE_INFO_FILE},
        specifiers::DependencySpecifiers,
        version_id::VersionId,
        IGNORED_DIRS, IGNORED_FILES,
    },
    MANIFEST_FILE_NAME,
};

use crate::{
    auth::UserId,
    benv,
    error::{Error, ErrorResponse},
    package::{s3_name, S3_SIGN_DURATION},
    search::update_version,
    AppState,
};

fn signature<'a>() -> Signature<'a> {
    Signature::now(
        &benv!(required "COMMITTER_GIT_NAME"),
        &benv!(required "COMMITTER_GIT_EMAIL"),
    )
    .unwrap()
}

fn get_refspec(repo: &Repository, remote: &mut Remote) -> Result<String, git2::Error> {
    let upstream_branch_buf = repo.branch_upstream_name(repo.head()?.name().unwrap())?;
    let upstream_branch = upstream_branch_buf.as_str().unwrap();

    let refspec_buf = remote
        .refspecs()
        .find(|r| r.direction() == git2::Direction::Fetch && r.dst_matches(upstream_branch))
        .unwrap()
        .rtransform(upstream_branch)?;
    let refspec = refspec_buf.as_str().unwrap();

    Ok(refspec.to_string())
}

const ADDITIONAL_FORBIDDEN_FILES: &[&str] = &["default.project.json"];

pub async fn publish_package(
    app_state: web::Data<AppState>,
    mut body: Multipart,
    user_id: web::ReqData<UserId>,
) -> Result<impl Responder, Error> {
    let max_archive_size = {
        let source = app_state.source.lock().unwrap();
        source.refresh(&app_state.project).map_err(Box::new)?;
        source.config(&app_state.project)?.max_archive_size
    };

    let bytes = body
        .next()
        .await
        .ok_or(Error::InvalidArchive)?
        .map_err(|_| Error::InvalidArchive)?
        .bytes(max_archive_size)
        .await
        .map_err(|_| Error::InvalidArchive)?
        .map_err(|_| Error::InvalidArchive)?;
    let mut decoder = GzDecoder::new(Cursor::new(&bytes));
    let mut archive = Archive::new(&mut decoder);

    let entries = archive.entries()?;
    let mut manifest = None::<Manifest>;
    let mut readme = None::<Vec<u8>>;

    for entry in entries {
        let mut entry = entry?;
        let path = entry.path()?;

        if entry.header().entry_type().is_dir() {
            if path.components().next().is_some_and(|ct| {
                ct.as_os_str()
                    .to_str()
                    .map_or(true, |s| IGNORED_DIRS.contains(&s))
            }) {
                return Err(Error::InvalidArchive);
            }

            continue;
        }

        let path = path.to_str().ok_or(Error::InvalidArchive)?;

        if IGNORED_FILES.contains(&path) || ADDITIONAL_FORBIDDEN_FILES.contains(&path) {
            return Err(Error::InvalidArchive);
        }

        if path == MANIFEST_FILE_NAME {
            let mut content = String::new();
            entry.read_to_string(&mut content)?;
            manifest = Some(toml::de::from_str(&content).map_err(|_| Error::InvalidArchive)?);
        } else if path.to_lowercase() == "readme"
            || path
                .to_lowercase()
                .split_once('.')
                .filter(|(file, ext)| *file == "readme" && (*ext == "md" || *ext == "txt"))
                .is_some()
        {
            if readme.is_some() {
                return Err(Error::InvalidArchive);
            }

            let mut gz = flate2::read::GzEncoder::new(entry, flate2::Compression::best());
            let mut bytes = vec![];
            gz.read_to_end(&mut bytes)?;
            readme = Some(bytes);
        }
    }

    let Some(manifest) = manifest else {
        return Err(Error::InvalidArchive);
    };

    {
        let source = app_state.source.lock().unwrap();
        source.refresh(&app_state.project).map_err(Box::new)?;
        let config = source.config(&app_state.project)?;

        let dependencies = manifest
            .all_dependencies()
            .map_err(|_| Error::InvalidArchive)?;

        for (specifier, _) in dependencies.values() {
            match specifier {
                DependencySpecifiers::Pesde(specifier) => {
                    if specifier
                        .index
                        .as_ref()
                        .filter(|index| match index.parse::<url::Url>() {
                            Ok(_) if config.other_registries_allowed => true,
                            Ok(url) => url == env!("CARGO_PKG_REPOSITORY").parse().unwrap(),
                            Err(_) => false,
                        })
                        .is_none()
                    {
                        return Err(Error::InvalidArchive);
                    }

                    let (dep_scope, dep_name) = specifier.name.as_str();
                    match source.read_file([dep_scope, dep_name], &app_state.project, None) {
                        Ok(Some(_)) => {}
                        Ok(None) => return Err(Error::InvalidArchive),
                        Err(e) => return Err(e.into()),
                    }
                }
                DependencySpecifiers::Wally(specifier) => {
                    if !config.wally_allowed {
                        return Err(Error::InvalidArchive);
                    }

                    if specifier
                        .index
                        .as_ref()
                        .filter(|index| index.parse::<url::Url>().is_ok())
                        .is_none()
                    {
                        return Err(Error::InvalidArchive);
                    }
                }
                DependencySpecifiers::Git(_) => {
                    if !config.git_allowed {
                        return Err(Error::InvalidArchive);
                    }
                }
            }
        }

        let repo = source.repo_git2(&app_state.project)?;

        let (scope, name) = manifest.name.as_str();
        let mut oids = vec![];

        match source.read_file([scope, SCOPE_INFO_FILE], &app_state.project, None)? {
            Some(info) => {
                let info: ScopeInfo = toml::de::from_str(&info)?;
                if !info.owners.contains(&user_id.0) {
                    return Ok(HttpResponse::Forbidden().finish());
                }
            }
            None => {
                let scope_info = toml::to_string(&ScopeInfo {
                    owners: BTreeSet::from([user_id.0]),
                })?;

                let mut blob_writer = repo.blob_writer(None)?;
                blob_writer.write_all(scope_info.as_bytes())?;
                oids.push((SCOPE_INFO_FILE, blob_writer.commit()?));
            }
        };

        let mut entries: IndexFile = toml::de::from_str(
            &source
                .read_file([scope, name], &app_state.project, None)?
                .unwrap_or_default(),
        )?;

        let new_entry = IndexFileEntry {
            target: manifest.target.clone(),
            published_at: chrono::Utc::now(),
            description: manifest.description.clone(),
            license: manifest.license.clone(),
            authors: manifest.authors.clone(),
            repository: manifest.repository.clone(),

            dependencies,
        };

        let this_version = entries
            .keys()
            .find(|v_id| *v_id.version() == manifest.version);
        if let Some(this_version) = this_version {
            let other_entry = entries.get(this_version).unwrap();

            // description cannot be different - which one to render in the "Recently published" list?
            // the others cannot be different because what to return from the versions endpoint?
            if other_entry.description != new_entry.description
                || other_entry.license != new_entry.license
                || other_entry.authors != new_entry.authors
                || other_entry.repository != new_entry.repository
            {
                return Ok(HttpResponse::BadRequest().json(ErrorResponse {
                    error: "same version with different description or license already exists"
                        .to_string(),
                }));
            }
        }

        if entries
            .insert(
                VersionId::new(manifest.version.clone(), manifest.target.kind()),
                new_entry.clone(),
            )
            .is_some()
        {
            return Ok(HttpResponse::Conflict().finish());
        }

        let mut remote = repo.find_remote("origin")?;
        let refspec = get_refspec(&repo, &mut remote)?;

        let reference = repo.find_reference(&refspec)?;

        {
            let index_content = toml::to_string(&entries)?;
            let mut blob_writer = repo.blob_writer(None)?;
            blob_writer.write_all(index_content.as_bytes())?;
            oids.push((name, blob_writer.commit()?));
        }

        let old_root_tree = reference.peel_to_tree()?;
        let old_scope_tree = match old_root_tree.get_name(scope) {
            Some(entry) => Some(repo.find_tree(entry.id())?),
            None => None,
        };

        let mut scope_tree = repo.treebuilder(old_scope_tree.as_ref())?;
        for (file, oid) in oids {
            scope_tree.insert(file, oid, 0o100644)?;
        }

        let scope_tree_id = scope_tree.write()?;
        let mut root_tree = repo.treebuilder(Some(&repo.find_tree(old_root_tree.id())?))?;
        root_tree.insert(scope, scope_tree_id, 0o040000)?;

        let tree_oid = root_tree.write()?;

        repo.commit(
            Some("HEAD"),
            &signature(),
            &signature(),
            &format!(
                "add {}@{} {}",
                manifest.name, manifest.version, manifest.target
            ),
            &repo.find_tree(tree_oid)?,
            &[&reference.peel_to_commit()?],
        )?;

        let mut push_options = git2::PushOptions::new();
        let mut remote_callbacks = git2::RemoteCallbacks::new();

        let git_creds = app_state.project.auth_config().git_credentials().unwrap();
        remote_callbacks.credentials(|_, _, _| {
            git2::Cred::userpass_plaintext(&git_creds.username, &git_creds.password)
        });

        push_options.remote_callbacks(remote_callbacks);

        remote.push(&[refspec], Some(&mut push_options))?;

        update_version(&app_state, &manifest.name, new_entry);
    }

    let version_id = VersionId::new(manifest.version.clone(), manifest.target.kind());

    let object_url = PutObject::new(
        &app_state.s3_bucket,
        Some(&app_state.s3_credentials),
        &s3_name(&manifest.name, &version_id, false),
    )
    .sign(S3_SIGN_DURATION);

    app_state
        .reqwest_client
        .put(object_url)
        .header(CONTENT_TYPE, "application/gzip")
        .header(CONTENT_ENCODING, "gzip")
        .body(bytes)
        .send()
        .await?;

    if let Some(readme) = readme {
        let object_url = PutObject::new(
            &app_state.s3_bucket,
            Some(&app_state.s3_credentials),
            &s3_name(&manifest.name, &version_id, true),
        )
        .sign(S3_SIGN_DURATION);

        app_state
            .reqwest_client
            .put(object_url)
            .header(CONTENT_TYPE, "text/plain")
            .header(CONTENT_ENCODING, "gzip")
            .body(readme)
            .send()
            .await?;
    }

    Ok(HttpResponse::Ok().body(format!(
        "published {}@{} {}",
        manifest.name, manifest.version, manifest.target
    )))
}
