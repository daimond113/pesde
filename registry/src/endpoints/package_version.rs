use std::str::FromStr;

use actix_web::{http::header::ACCEPT, web, HttpRequest, HttpResponse, Responder};
use rusty_s3::{actions::GetObject, S3Action};
use semver::Version;
use serde::{Deserialize, Deserializer};

use crate::{
    error::Error,
    package::{s3_name, PackageResponse, S3_SIGN_DURATION},
    AppState,
};
use pesde::{
    manifest::target::TargetKind,
    names::PackageName,
    source::{git_index::GitBasedSource, pesde::IndexFile},
};

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

#[derive(Debug)]
pub enum TargetRequest {
    All,
    Specific(TargetKind),
}

impl<'de> Deserialize<'de> for TargetRequest {
    fn deserialize<D>(deserializer: D) -> Result<TargetRequest, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if s.eq_ignore_ascii_case("all") {
            return Ok(TargetRequest::All);
        }

        TargetKind::from_str(&s)
            .map(TargetRequest::Specific)
            .map_err(serde::de::Error::custom)
    }
}

pub async fn get_package_version(
    request: HttpRequest,
    app_state: web::Data<AppState>,
    path: web::Path<(PackageName, VersionRequest, TargetRequest)>,
) -> Result<impl Responder, Error> {
    let (name, version, target) = path.into_inner();

    let (scope, name_part) = name.as_str();

    let versions: IndexFile = {
        let source = app_state.source.lock().unwrap();

        match source.read_file([scope, name_part], &app_state.project, None)? {
            Some(versions) => toml::de::from_str(&versions)?,
            None => return Ok(HttpResponse::NotFound().finish()),
        }
    };

    let version = match version {
        VersionRequest::Latest => versions
            .iter()
            .filter(|(v_id, _)| match target {
                TargetRequest::All => true,
                TargetRequest::Specific(target) => *v_id.target() == target,
            })
            .max_by_key(|(v, _)| v.version().clone()),
        VersionRequest::Specific(version) => versions.iter().find(|(v, _)| {
            *v.version() == version
                && match target {
                    TargetRequest::All => true,
                    TargetRequest::Specific(target) => *v.target() == target,
                }
        }),
    };

    let Some((v_id, entry)) = version else {
        return Ok(HttpResponse::NotFound().finish());
    };

    let other_targets = versions
        .iter()
        .filter(|(v, _)| v.version() == v_id.version() && v.target() != v_id.target())
        .map(|(v_id, _)| v_id.target().to_string())
        .collect::<Vec<_>>();

    if request
        .headers()
        .get(ACCEPT)
        .and_then(|accept| accept.to_str().ok())
        .is_some_and(|accept| accept.eq_ignore_ascii_case("application/octet-stream"))
    {
        let object_url = GetObject::new(
            &app_state.s3_bucket,
            Some(&app_state.s3_credentials),
            &s3_name(&name, v_id),
        )
        .sign(S3_SIGN_DURATION);

        return Ok(HttpResponse::Ok().body(
            app_state
                .reqwest_client
                .get(object_url)
                .send()
                .await?
                .error_for_status()?
                .bytes()
                .await?,
        ));
    }

    let entry = entry.clone();

    let mut response = serde_json::to_value(PackageResponse {
        name: name.to_string(),
        version: v_id.version().to_string(),
        target: Some(entry.target.into()),
        description: entry.description.unwrap_or_default(),
        published_at: entry.published_at,
        license: entry.license.unwrap_or_default(),
    })?;

    if !other_targets.is_empty() {
        response["other_targets"] = serde_json::to_value(other_targets)?;
    }

    Ok(HttpResponse::Ok().json(response))
}
