use actix_web::{web, Responder};
use semver::Version;
use serde::Deserialize;
use serde_json::{json, Value};
use tantivy::{query::AllQuery, DateTime, DocAddress, Order};

use pesde::{index::Index, package_name::StandardPackageName};

use crate::{errors, AppState};

#[derive(Deserialize)]
pub struct Query {
    query: Option<String>,
}

pub async fn search_packages(
    app_state: web::Data<AppState>,
    query: web::Query<Query>,
) -> Result<impl Responder, errors::Errors> {
    let searcher = app_state.search_reader.searcher();
    let schema = searcher.schema();

    let name = schema.get_field("name").unwrap();
    let version = schema.get_field("version").unwrap();
    let description = schema.get_field("description").unwrap();

    let query = query.query.as_deref().unwrap_or_default().trim();

    let query_parser =
        tantivy::query::QueryParser::for_index(searcher.index(), vec![name, description]);
    let query = if query.is_empty() {
        Box::new(AllQuery)
    } else {
        query_parser.parse_query(query)?
    };

    let top_docs: Vec<(DateTime, DocAddress)> = searcher
        .search(
            &query,
            &tantivy::collector::TopDocs::with_limit(10)
                .order_by_fast_field("published_at", Order::Desc),
        )
        .unwrap();

    {
        let index = app_state.index.lock().unwrap();

        Ok(web::Json(
            top_docs
                .into_iter()
                .map(|(published_at, doc_address)| {
                    let retrieved_doc = searcher.doc(doc_address).unwrap();
                    let name: StandardPackageName = retrieved_doc
                        .get_first(name)
                        .and_then(|v| v.as_text())
                        .and_then(|v| v.parse().ok())
                        .unwrap();

                    let version: Version = retrieved_doc
                        .get_first(version)
                        .and_then(|v| v.as_text())
                        .and_then(|v| v.parse().ok())
                        .unwrap();

                    let entry = index
                        .package(&name.clone().into())
                        .unwrap()
                        .and_then(|v| v.into_iter().find(|v| v.version == version))
                        .unwrap();

                    json!({
                        "name": name,
                        "version": version,
                        "description": entry.description,
                        "published_at": published_at.into_timestamp_secs(),
                    })
                })
                .collect::<Vec<Value>>(),
        ))
    }
}
