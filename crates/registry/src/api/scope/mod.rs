mod error;
mod get_entry;
mod get_head;
mod update_manifest;

pub fn http_v2(cfg: &mut actix_web::web::ServiceConfig) {
	cfg.service(get_head::http_v2)
		.service(get_entry::http_v2)
		.service(update_manifest::http_v2);
}
