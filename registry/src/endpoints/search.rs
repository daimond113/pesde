use std::collections::HashMap;

use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;
use tantivy::{query::AllQuery, schema::Value, DateTime, Order};

use crate::{error::Error, package::PackageResponse, AppState};
use pesde::{
    names::PackageName,
    source::{git_index::GitBasedSource, pesde::IndexFile},
};

#[derive(Deserialize)]
pub struct Request {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    offset: Option<usize>,
}

pub async fn search_packages(
    app_state: web::Data<AppState>,
    request: web::Query<Request>,
) -> Result<impl Responder, Error> {
    let searcher = app_state.search_reader.searcher();
    let schema = searcher.schema();

    let id = schema.get_field("id").unwrap();

    let scope = schema.get_field("scope").unwrap();
    let name = schema.get_field("name").unwrap();
    let description = schema.get_field("description").unwrap();

    let query = request.query.as_deref().unwrap_or_default().trim();

    let query = if query.is_empty() {
        Box::new(AllQuery)
    } else {
        let mut query_parser = tantivy::query::QueryParser::for_index(
            searcher.index(),
            vec![scope, name, description],
        );
        query_parser.set_field_boost(scope, 2.0);
        query_parser.set_field_boost(name, 3.5);

        query_parser.parse_query(query)?
    };

    let top_docs = searcher
        .search(
            &query,
            &tantivy::collector::TopDocs::with_limit(50)
                .and_offset(request.offset.unwrap_or_default())
                .order_by_fast_field::<DateTime>("published_at", Order::Desc),
        )
        .unwrap();

    let source = app_state.source.lock().unwrap();

    let top_docs = top_docs
        .into_iter()
        .map(|(_, doc_address)| {
            let doc = searcher.doc::<HashMap<_, _>>(doc_address).unwrap();

            let id = doc
                .get(&id)
                .unwrap()
                .as_str()
                .unwrap()
                .parse::<PackageName>()
                .unwrap();
            let (scope, name) = id.as_str();

            let mut versions: IndexFile = toml::de::from_str(
                &source
                    .read_file([scope, name], &app_state.project, None)
                    .unwrap()
                    .unwrap(),
            )
            .unwrap();

            let (version_id, entry) = versions.pop_last().unwrap();

            PackageResponse {
                name: id.to_string(),
                version: version_id.version().to_string(),
                target: None,
                description: entry.description.unwrap_or_default(),
                published_at: entry.published_at,
                license: entry.license.unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>();

    Ok(HttpResponse::Ok().json(top_docs))
}
