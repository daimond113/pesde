use actix_web::HttpResponse;
use actix_web::Responder;
use actix_web::get;
use actix_web::web;
use merkleberg::MMRIVER;
use pesde::source::pesde::registry::*;
use pesde_registry_core::db::Backend;

use crate::AppState;
use crate::api::log::Error;
use crate::shared::log::LogHeadQuery;
use crate::shared::log::log_head;

#[get("/scope/{scope_id}/log/head")]
pub(super) async fn http_v2(
	app_state: web::Data<AppState>,
	path: web::Path<ScopeId>,
	query: web::Query<LogHeadQuery>,
) -> Result<impl Responder, Error> {
	let Some(head) = handler(app_state.db.as_ref(), path.into_inner(), query.into_inner()).await?
	else {
		return Ok(HttpResponse::NotFound().finish());
	};

	Ok(HttpResponse::Ok().json(head))
}

async fn handler(
	db: &dyn Backend,
	scope_id: ScopeId,
	query: LogHeadQuery,
) -> Result<Option<LogHeadResponse>, Error> {
	let current_size = db.scope_log_size(&scope_id).await?;
	let mmr = MMRIVER::new(current_size, db.scope_mmr_read_store(&scope_id));

	log_head(mmr, query).await
}
