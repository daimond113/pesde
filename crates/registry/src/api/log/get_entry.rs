use actix_web::HttpResponse;
use actix_web::Responder;
use actix_web::get;
use actix_web::web;
use merkleberg::MMRIVER;
use pesde::source::pesde::registry::*;
use pesde_registry_core::db::Backend;

use crate::AppState;
use crate::api::log::error::Error;
use crate::shared::log::LogEntryQuery;
use crate::shared::log::log_entry;

#[get("/log/entry/{pos}")]
pub(super) async fn http_v2(
	app_state: web::Data<AppState>,
	path: web::Path<u64>,
	query: web::Query<LogEntryQuery>,
) -> Result<impl Responder, Error> {
	let Some(entry) = handler(app_state.db.as_ref(), path.into_inner(), query.into_inner()).await?
	else {
		return Ok(HttpResponse::NotFound().finish());
	};

	Ok(HttpResponse::Ok().json(entry))
}

async fn handler(
	db: &dyn Backend,
	pos: u64,
	query: LogEntryQuery,
) -> Result<Option<LogEntryResponse<GlobalEntryPayload>>, Error> {
	log_entry(
		pos,
		|pos| db.global_log_entry(pos),
		async || {
			let size = db.global_log_size().await?;
			Ok(MMRIVER::new(size, db.global_mmr_read_store()))
		},
		query,
	)
	.await
}
