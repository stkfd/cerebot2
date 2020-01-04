use actix_web::error::JsonPayloadError;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError};
use once_cell::sync::OnceCell;
use thiserror::Error;

use crate::config::Config;
use crate::models::responses::problem_details::ProblemDetails;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("{0}")]
    Internal(#[from] InternalError),
    #[error("{0}")]
    User(#[from] UserError),
}

impl From<persistence::Error> for ApiError {
    fn from(source: persistence::Error) -> Self {
        match source {
            persistence::Error::NotFound => ApiError::User(UserError::NotFound),
            other_err => ApiError::Internal(InternalError::Persistence(other_err)),
        }
    }
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ApiError::Internal(e) => e.error_response(),
            ApiError::User(e) => e.error_response(),
        }
    }
}

#[derive(Debug, Error)]
pub enum InternalError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Persistence(#[from] persistence::Error),
}

impl ResponseError for InternalError {
    fn error_response(&self) -> HttpResponse {
        let status_code = self.status_code();

        HttpResponse::build(status_code).json(ProblemDetails {
            error_type: error_type("internal"),
            title: "Internal Server Error",
            details: None,
            status: status_code.into(),
            validation_errors: None,
        })
    }
}

#[derive(Debug, Error)]
pub enum UserError {
    #[error("Not found")]
    NotFound,

    #[error("Unauthorized access")]
    Unauthorized,

    #[error("Unauthenticated access")]
    Unauthenticated,

    #[error("Deserialization error")]
    Deserialize(#[from] serde::de::value::Error),

    #[error("JSON error")]
    Json(JsonPayloadError),

    #[error("Validation error")]
    Validation(#[from] validator::ValidationErrors),
}

impl ResponseError for UserError {
    fn error_response(&self) -> HttpResponse {
        let problem_details = ProblemDetails::from(self);
        HttpResponse::build(StatusCode::from_u16(problem_details.status).unwrap())
            .json(problem_details)
    }
}

pub fn error_type(error: &'static str) -> &'static str {
    static LAZY: OnceCell<String> = OnceCell::new();

    &LAZY.get_or_init(|| {
        let app_url = &Config::get().app_url;
        format!("{}/api/1.0/errors/{}", app_url, error)
    })
}
