use crate::{
    error::Error,
    package::{s3_name, PackageResponse, S3_SIGN_DURATION},
    AppState,
};
use actix_web::{
    http::header::{ACCEPT, LOCATION},
    web, HttpRequest, HttpResponse, Responder,
};
use pesde::{
    manifest::target::TargetKind,
    names::PackageName,
    source::{git_index::GitBasedSource, pesde::IndexFile},
};
use rusty_s3::{actions::GetObject, S3Action};
use semver::Version;
use serde::{Deserialize, Deserializer};
use std::collections::BTreeSet;

#[derive(Debug)]
pub enum VersionRequest {
    Latest,
    Specific(Version),
}

impl<'de> Deserialize<'de> for VersionRequest {
    fn deserialize<D>(deserializer: D) -> Result<VersionRequest, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if s.eq_ignore_ascii_case("latest") {
            return Ok(VersionRequest::Latest);
        }

        Version::parse(&s)
            .map(VersionRequest::Specific)
            .map_err(serde::de::Error::custom)
    }
}

pub async fn get_package_version(
    request: HttpRequest,
    app_state: web::Data<AppState>,
    path: web::Path<(PackageName, VersionRequest, TargetKind)>,
) -> Result<impl Responder, Error> {
    let (name, version, target) = path.into_inner();

    let (scope, name_part) = name.as_str();

    let entries: IndexFile = {
        let source = app_state.source.lock().unwrap();

        match source.read_file([scope, name_part], &app_state.project, None)? {
            Some(versions) => toml::de::from_str(&versions)?,
            None => return Ok(HttpResponse::NotFound().finish()),
        }
    };

    let mut versions = entries
        .into_iter()
        .filter(|(v_id, _)| *v_id.target() == target);

    let version = match version {
        VersionRequest::Latest => versions.max_by_key(|(v, _)| v.version().clone()),
        VersionRequest::Specific(version) => versions.find(|(v, _)| *v.version() == version),
    };

    let Some((v_id, entry)) = version else {
        return Ok(HttpResponse::NotFound().finish());
    };

    let accept = request
        .headers()
        .get(ACCEPT)
        .and_then(|accept| accept.to_str().ok())
        .and_then(|accept| match accept.to_lowercase().as_str() {
            "text/plain" => Some(true),
            "application/octet-stream" => Some(false),
            _ => None,
        });

    if let Some(readme) = accept {
        let object_url = GetObject::new(
            &app_state.s3_bucket,
            Some(&app_state.s3_credentials),
            &s3_name(&name, &v_id, readme),
        )
        .sign(S3_SIGN_DURATION);

        return Ok(HttpResponse::TemporaryRedirect()
            .append_header((LOCATION, object_url.as_str()))
            .finish());
    }

    Ok(HttpResponse::Ok().json(PackageResponse {
        name: name.to_string(),
        version: v_id.version().to_string(),
        targets: BTreeSet::from([entry.target.into()]),
        description: entry.description.clone().unwrap_or_default(),
        published_at: entry.published_at,
        license: entry.license.clone().unwrap_or_default(),
        authors: entry.authors.clone(),
        repository: entry.repository.clone().map(|url| url.to_string()),
    }))
}
