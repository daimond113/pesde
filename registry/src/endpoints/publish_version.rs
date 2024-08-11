use std::{
    collections::BTreeSet,
    io::{Cursor, Read, Write},
};

use actix_multipart::Multipart;
use actix_web::{web, HttpResponse, Responder};
use actix_web_lab::__reexports::futures_util::StreamExt;
use flate2::read::GzDecoder;
use git2::{Remote, Repository, Signature};
use rusty_s3::{actions::PutObject, S3Action};
use tar::Archive;

use pesde::{
    manifest::Manifest,
    source::{
        git_index::GitBasedSource,
        pesde::{IndexFile, IndexFileEntry, ScopeInfo, SCOPE_INFO_FILE},
        specifiers::DependencySpecifiers,
        version_id::VersionId,
    },
    DEFAULT_INDEX_NAME, MANIFEST_FILE_NAME,
};

use crate::{
    auth::UserId,
    benv,
    error::Error,
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

const FORBIDDEN_FILES: &[&str] = &[".DS_Store", "default.project.json"];
const FORBIDDEN_DIRECTORIES: &[&str] = &[".git"];

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

    for entry in entries {
        let mut entry = entry?;
        let path = entry.path()?;
        let path = path.to_str().ok_or(Error::InvalidArchive)?;

        if entry.header().entry_type().is_dir() {
            if FORBIDDEN_DIRECTORIES.contains(&path) {
                return Err(Error::InvalidArchive);
            }

            continue;
        }

        if FORBIDDEN_FILES.contains(&path) {
            return Err(Error::InvalidArchive);
        }

        if path == MANIFEST_FILE_NAME {
            let mut content = String::new();
            entry.read_to_string(&mut content)?;
            manifest = Some(toml::de::from_str(&content).map_err(|_| Error::InvalidArchive)?);
        }
    }

    let Some(manifest) = manifest else {
        return Err(Error::InvalidArchive);
    };

    {
        let source = app_state.source.lock().unwrap();
        source.refresh(&app_state.project).map_err(Box::new)?;
        let config = source.config(&app_state.project)?;

        if manifest
            .indices
            .get(DEFAULT_INDEX_NAME)
            .filter(|index_url| *index_url == source.repo_url())
            .is_none()
        {
            return Err(Error::InvalidArchive);
        }

        let dependencies = manifest
            .all_dependencies()
            .map_err(|_| Error::InvalidArchive)?;

        for (specifier, _) in dependencies.values() {
            match specifier {
                DependencySpecifiers::Pesde(specifier) => {
                    if specifier
                        .index
                        .as_ref()
                        .is_some_and(|index| index != DEFAULT_INDEX_NAME)
                        && !config.other_registries_allowed
                    {
                        return Err(Error::InvalidArchive);
                    }

                    let (dep_scope, dep_name) = specifier.name.as_str();
                    if source
                        .read_file([dep_scope, dep_name], &app_state.project, None)?
                        .is_none()
                    {
                        return Err(Error::InvalidArchive);
                    }
                }
                DependencySpecifiers::Wally(_) => {
                    if !config.wally_allowed {
                        return Err(Error::InvalidArchive);
                    }
                }
                DependencySpecifiers::Git(_) => {
                    if !config.git_allowed {
                        return Err(Error::InvalidArchive);
                    }
                }
            };
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

            dependencies,
        };

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

    let object_url = PutObject::new(
        &app_state.s3_bucket,
        Some(&app_state.s3_credentials),
        &s3_name(
            &manifest.name,
            &VersionId::new(manifest.version.clone(), manifest.target.kind()),
        ),
    )
    .sign(S3_SIGN_DURATION);

    app_state
        .reqwest_client
        .put(object_url)
        .body(bytes)
        .send()
        .await?;

    Ok(HttpResponse::Ok().body(format!(
        "published {}@{} {}",
        manifest.name, manifest.version, manifest.target
    )))
}
