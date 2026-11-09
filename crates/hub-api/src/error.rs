//! Every refusal and failure as one JSON shape:
//! `{"error": {"kind": "...", "field": "...", "reason": "...", "message": "..."}}`.
//! Invalid input carries the field and a reason code (as the UI's errors do); internal failures
//! say nothing about the inside (the detail goes to `hub.log`).

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub kind: &'static str,
    pub field: Option<String>,
    pub reason: Option<String>,
    pub message: String,
}

#[derive(Serialize)]
struct Body<'a> {
    error: Inner<'a>,
}

#[derive(Serialize)]
struct Inner<'a> {
    kind: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'a str>,
    message: &'a str,
}

impl ApiError {
    pub fn new(status: StatusCode, kind: &'static str, reason: &str, message: impl Into<String>) -> ApiError {
        ApiError { status, kind, field: None, reason: Some(reason.into()), message: message.into() }
    }

    pub fn forbidden(reason: &str, message: &str) -> ApiError {
        ApiError::new(StatusCode::FORBIDDEN, "forbidden", reason, message)
    }

    pub fn unauthorized() -> ApiError {
        ApiError::new(StatusCode::UNAUTHORIZED, "unauthorized", "token", "a valid bearer token is required")
    }

    pub fn invalid(field: &str, reason: &str) -> ApiError {
        ApiError {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            kind: "invalid",
            field: Some(field.into()),
            reason: Some(reason.into()),
            message: format!("invalid {field}: {reason}"),
        }
    }

    pub fn internal() -> ApiError {
        ApiError { status: StatusCode::INTERNAL_SERVER_ERROR, kind: "internal", field: None, reason: None, message: "internal error".into() }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Body {
            error: Inner { kind: self.kind, field: self.field.as_deref(), reason: self.reason.as_deref(), message: &self.message },
        };
        let mut response = (self.status, Json(body)).into_response();
        if self.status == StatusCode::UNAUTHORIZED {
            response.headers_mut().insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        if self.status == StatusCode::TOO_MANY_REQUESTS {
            response.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
        }
        response
    }
}

/// A core error as an HTTP answer. Internal ones are logged by the caller, which has the host.
pub fn from_core(error: &hub_core::Error) -> ApiError {
    match error {
        hub_core::Error::Invalid { field, reason } => ApiError::invalid(field, reason.as_str()),
        hub_core::Error::NotFound(what) => ApiError {
            status: StatusCode::NOT_FOUND,
            kind: "not_found",
            field: Some((*what).into()),
            reason: None,
            message: format!("{what} not found"),
        },
        hub_core::Error::NewerDatabase { .. } => {
            ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "unavailable", "schema", "the database is newer than this Hub")
        }
        _ => ApiError::internal(),
    }
}
