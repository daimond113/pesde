use actix_web::{web, Responder};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{errors, AppState};

#[derive(Deserialize)]
pub struct Query {
    query: String,
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

    let query = query.query.trim();

    if query.is_empty() {
        return Ok(web::Json(vec![]));
    }

    let query_parser =
        tantivy::query::QueryParser::for_index(&searcher.index(), vec![name, description]);
    let query = query_parser.parse_query(&query)?;

    let top_docs = searcher
        .search(&query, &tantivy::collector::TopDocs::with_limit(10))
        .unwrap();

    Ok(web::Json(
        top_docs
            .into_iter()
            .map(|(_, doc_address)| {
                let retrieved_doc = searcher.doc(doc_address).unwrap();

                json!({
                    "name": retrieved_doc.get_first(name).unwrap().as_text().unwrap(),
                    "version": retrieved_doc.get_first(version).unwrap().as_text().unwrap(),
                    "description": retrieved_doc.get_first(description).unwrap().as_text().unwrap(),
                })
            })
            .collect::<Vec<Value>>(),
    ))
}
