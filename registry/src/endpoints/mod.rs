use actix_web::web;

pub mod packages;
pub mod search;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.configure(packages::configure);
}
