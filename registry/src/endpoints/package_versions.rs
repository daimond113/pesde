use actix_web::{web, HttpResponse, Responder};

use crate::{error::Error, package::PackageResponse, AppState};
use pesde::{
    names::PackageName,
    source::{git_index::GitBasedSource, pesde::IndexFile},
};

pub async fn get_package_versions(
    app_state: web::Data<AppState>,
    path: web::Path<PackageName>,
) -> Result<impl Responder, Error> {
    let name = path.into_inner();

    let (scope, name_part) = name.as_str();

    let source = app_state.source.lock().unwrap();
    let versions: IndexFile =
        match source.read_file([scope, name_part], &app_state.project, None)? {
            Some(versions) => toml::de::from_str(&versions)?,
            None => return Ok(HttpResponse::NotFound().finish()),
        };

    Ok(HttpResponse::Ok().json(
        versions
            .into_iter()
            .map(|(v_id, entry)| PackageResponse {
                name: name.to_string(),
                version: v_id.version().to_string(),
                target: entry.target.into(),
                description: entry.description.unwrap_or_default(),
                published_at: entry.published_at,
                license: entry.license.unwrap_or_default(),
            })
            .collect::<Vec<_>>(),
    ))
}
