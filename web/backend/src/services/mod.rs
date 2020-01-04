use actix_web::error::QueryPayloadError;
use actix_web::web;
use actix_web::web::{JsonConfig, QueryConfig};

use crate::error::UserError;

pub mod commands;

pub fn web_config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/1.0")
            .app_data(query_error_handler())
            .app_data(payload_error_handler())
            .service(commands::index)
            .service(commands::get),
    );
}

fn query_error_handler() -> QueryConfig {
    QueryConfig::default().error_handler(|err, _req| match err {
        QueryPayloadError::Deserialize(serde_err) => UserError::Deserialize(serde_err).into(),
    })
}

fn payload_error_handler() -> JsonConfig {
    JsonConfig::default().error_handler(|err, _req| UserError::Json(err).into())
}
