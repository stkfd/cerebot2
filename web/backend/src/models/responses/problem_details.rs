use std::borrow::Cow;

use actix_web::error::JsonPayloadError;
use serde::Serialize;

use crate::error::{error_type, UserError};

#[derive(Debug, Serialize)]
pub struct ProblemDetails<'a> {
    #[serde(rename = "type", skip_serializing_if = "str::is_empty")]
    pub error_type: &'static str,
    pub title: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Cow<'static, str>>,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_errors: Option<&'a validator::ValidationErrors>,
}

impl<'a> From<&'a UserError> for ProblemDetails<'a> {
    fn from(source: &'a UserError) -> Self {
        match source {
            UserError::Unauthorized => ProblemDetails {
                error_type: "",
                title: "Unauthorized",
                details: None,
                status: 403,
                validation_errors: None,
            },
            UserError::Unauthenticated => ProblemDetails {
                error_type: "",
                title: "Unauthenticated",
                details: None,
                status: 401,
                validation_errors: None,
            },
            UserError::Deserialize(err) => ProblemDetails {
                error_type: "",
                title: "Bad Request",
                details: Some(err.to_string().into()),
                status: 400,
                validation_errors: None,
            },
            UserError::Validation(err) => ProblemDetails {
                error_type: error_type("validation"),
                title: "Bad Request",
                details: Some("A request parameter failed validation.".into()),
                status: 400,
                validation_errors: Some(err),
            },
            UserError::Json(err) => err.into(),
            UserError::NotFound => ProblemDetails {
                error_type: error_type("not_found"),
                title: "Not found",
                details: None,
                status: 404,
                validation_errors: None,
            },
        }
    }
}

impl<'a> From<&'a JsonPayloadError> for ProblemDetails<'a> {
    fn from(err: &'a JsonPayloadError) -> Self {
        match err {
            JsonPayloadError::Overflow => ProblemDetails {
                error_type: "",
                title: "Payload too large",
                details: Some("The json payload was larger than allowed.".into()),
                status: 413,
                validation_errors: None,
            },
            JsonPayloadError::ContentType => ProblemDetails {
                error_type: "",
                title: "Unsupported content type",
                details: Some("Expected json content type for this request.".into()),
                status: 415,
                validation_errors: None,
            },
            JsonPayloadError::Deserialize(err) => ProblemDetails {
                error_type: "",
                title: "Bad Request",
                details: Some(err.to_string().into()),
                status: 400,
                validation_errors: None,
            },
            JsonPayloadError::Payload(_) => ProblemDetails {
                error_type: "",
                title: "Payload error",
                details: Some(err.to_string().into()),
                status: 400,
                validation_errors: None,
            },
        }
    }
}
