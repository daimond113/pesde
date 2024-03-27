use actix_multipart::form::{bytes::Bytes, MultipartForm};
use actix_web::{web, HttpResponse, Responder};
use flate2::read::GzDecoder;
use log::error;
use reqwest::StatusCode;
use rusty_s3::S3Action;
use tantivy::{doc, DateTime, Term};
use tar::Archive;

use pesde::{
    dependencies::DependencySpecifier, index::Index, manifest::Manifest,
    package_name::StandardPackageName, project::DEFAULT_INDEX_NAME, IGNORED_FOLDERS,
    MANIFEST_FILE_NAME,
};

use crate::{commit_signature, errors, AppState, UserId, S3_EXPIRY};

#[derive(MultipartForm)]
pub struct CreateForm {
    #[multipart(limit = "4 MiB")]
    tarball: Bytes,
}

pub async fn create_package(
    form: MultipartForm<CreateForm>,
    app_state: web::Data<AppState>,
    user_id: web::ReqData<UserId>,
) -> Result<impl Responder, errors::Errors> {
    let bytes = form.tarball.data.as_ref().to_vec();
    let mut decoder = GzDecoder::new(bytes.as_slice());
    let mut archive = Archive::new(&mut decoder);

    let archive_entries = archive.entries()?.filter_map(|e| e.ok());

    let mut manifest = None;

    for mut e in archive_entries {
        let Ok(path) = e.path() else {
            return Ok(HttpResponse::BadRequest().json(errors::ErrorResponse {
                error: "Attached file contains non-UTF-8 path".to_string(),
            }));
        };

        let Some(path) = path.as_os_str().to_str() else {
            return Ok(HttpResponse::BadRequest().json(errors::ErrorResponse {
                error: "Attached file contains non-UTF-8 path".to_string(),
            }));
        };

        match path {
            MANIFEST_FILE_NAME => {
                if !e.header().entry_type().is_file() {
                    continue;
                }

                let received_manifest: Manifest =
                    serde_yaml::from_reader(&mut e).map_err(errors::Errors::UserYaml)?;

                manifest = Some(received_manifest);
            }
            path => {
                if e.header().entry_type().is_file() {
                    continue;
                }

                if IGNORED_FOLDERS.contains(&path) {
                    return Ok(HttpResponse::BadRequest().json(errors::ErrorResponse {
                        error: format!("Attached file contains forbidden directory {}", path),
                    }));
                }
            }
        }
    }

    let Some(manifest) = manifest else {
        return Ok(HttpResponse::BadRequest().json(errors::ErrorResponse {
            error: format!("Attached file doesn't contain {MANIFEST_FILE_NAME}"),
        }));
    };

    let (scope, name) = manifest.name.parts();

    let entry = {
        let mut index = app_state.index.lock().unwrap();
        let config = index.config()?;

        for (dependency, _) in manifest.dependencies().into_values() {
            match dependency {
                DependencySpecifier::Git(_) => {
                    if !config.git_allowed {
                        return Ok(HttpResponse::BadRequest().json(errors::ErrorResponse {
                            error: "Git dependencies are not allowed on this registry".to_string(),
                        }));
                    }
                }
                DependencySpecifier::Registry(registry) => {
                    if index
                        .package(&registry.name.clone().into())
                        .unwrap()
                        .is_none()
                    {
                        return Ok(HttpResponse::BadRequest().json(errors::ErrorResponse {
                            error: format!("Dependency {} not found", registry.name),
                        }));
                    }

                    if registry.index != DEFAULT_INDEX_NAME && !config.custom_registry_allowed {
                        return Ok(HttpResponse::BadRequest().json(errors::ErrorResponse {
                            error: "Custom registries are not allowed on this registry".to_string(),
                        }));
                    }
                }
                #[allow(unreachable_patterns)]
                _ => {}
            };
        }

        match index.create_package_version(&manifest, &user_id.0)? {
            Some(entry) => {
                index.commit_and_push(
                    &format!("Add version {}@{}", manifest.name, manifest.version),
                    &commit_signature(),
                )?;

                entry
            }
            None => {
                return Ok(HttpResponse::BadRequest().json(errors::ErrorResponse {
                    error: format!(
                        "Version {} of {} already exists",
                        manifest.version, manifest.name
                    ),
                }));
            }
        }
    };

    {
        let mut search_writer = app_state.search_writer.lock().unwrap();
        let schema = search_writer.index().schema();
        let name_field = schema.get_field("name").unwrap();

        search_writer.delete_term(Term::from_field_text(
            name_field,
            &manifest.name.to_string(),
        ));

        search_writer.add_document(
            doc!(
                name_field => manifest.name.to_string(),
                schema.get_field("version").unwrap() => manifest.version.to_string(),
                schema.get_field("description").unwrap() => manifest.description.unwrap_or_default(),
                schema.get_field("published_at").unwrap() => DateTime::from_timestamp_secs(entry.published_at.timestamp())
            )
        ).unwrap();

        search_writer.commit().unwrap();
    }

    let url = app_state
        .s3_bucket
        .put_object(
            Some(&app_state.s3_credentials),
            &format!("{scope}-{name}-{}.tar.gz", manifest.version),
        )
        .sign(S3_EXPIRY);

    app_state.reqwest_client.put(url).body(bytes).send().await?;

    Ok(HttpResponse::Ok().body(format!(
        "Successfully published {}@{}",
        manifest.name, manifest.version
    )))
}

pub async fn get_package_version(
    app_state: web::Data<AppState>,
    path: web::Path<(String, String, String)>,
) -> Result<impl Responder, errors::Errors> {
    let (scope, name, mut version) = path.into_inner();

    let package_name = StandardPackageName::new(&scope, &name)?;

    {
        let index = app_state.index.lock().unwrap();

        match index.package(&package_name.clone().into())? {
            Some(package) => {
                if version == "latest" {
                    version = package.last().map(|v| v.version.to_string()).unwrap();
                } else if !package.iter().any(|v| v.version.to_string() == version) {
                    return Ok(HttpResponse::NotFound().finish());
                }
            }
            None => return Ok(HttpResponse::NotFound().finish()),
        }
    }

    let url = app_state
        .s3_bucket
        .get_object(
            Some(&app_state.s3_credentials),
            &format!("{scope}-{name}-{version}.tar.gz"),
        )
        .sign(S3_EXPIRY);

    let response = match app_state
        .reqwest_client
        .get(url)
        .send()
        .await?
        .error_for_status()
    {
        Ok(response) => response,
        Err(e) => {
            if let Some(status) = e.status() {
                if status == StatusCode::NOT_FOUND {
                    error!(
                        "package {}@{} not found in S3, but found in index",
                        package_name, version
                    );
                    return Ok(HttpResponse::InternalServerError().finish());
                }
            }

            return Err(e.into());
        }
    };

    Ok(HttpResponse::Ok().body(response.bytes().await?))
}

pub async fn get_package_versions(
    app_state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> Result<impl Responder, errors::Errors> {
    let (scope, name) = path.into_inner();

    let package_name = StandardPackageName::new(&scope, &name)?;

    {
        let index = app_state.index.lock().unwrap();

        match index.package(&package_name.into())? {
            Some(package) => {
                let versions = package
                    .iter()
                    .map(|v| (v.version.to_string(), v.published_at.timestamp()))
                    .collect::<Vec<_>>();

                Ok(HttpResponse::Ok().json(versions))
            }
            None => Ok(HttpResponse::NotFound().finish()),
        }
    }
}
