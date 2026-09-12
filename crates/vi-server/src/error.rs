//! One error shape for every handler: `{"error": {"code", "message"}}` with
//! an HTTP status that follows the core error kinds.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// An API error.
#[derive(Debug)]
pub struct ApiError {
    /// HTTP status.
    pub status: StatusCode,
    /// Stable machine-readable code.
    pub code: &'static str,
    /// Human-readable message (keys redacted upstream).
    pub message: String,
}

impl ApiError {
    /// Construct.
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    /// 400.
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid", message)
    }

    /// 404.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", message)
    }

    /// 401.
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized", message)
    }

    /// 429 (spend cap).
    pub fn quota(message: impl Into<String>) -> Self {
        Self::new(StatusCode::TOO_MANY_REQUESTS, "quota_exceeded", message)
    }

    /// 500.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({"error": {"code": self.code, "message": self.message}})),
        )
            .into_response()
    }
}

impl From<vi_core::Error> for ApiError {
    fn from(e: vi_core::Error) -> Self {
        use vi_core::Error as E;
        let (status, code) = match &e {
            E::Invalid(_) | E::Config(_) => (StatusCode::BAD_REQUEST, "invalid"),
            E::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            E::Unsupported(_) => (StatusCode::NOT_IMPLEMENTED, "unsupported"),
            E::Provider(_) => (StatusCode::BAD_GATEWAY, "provider"),
            E::Budget(_) => (StatusCode::PAYMENT_REQUIRED, "budget"),
            E::Cancelled | E::Timeout(_) => (StatusCode::GATEWAY_TIMEOUT, "timeout"),
            E::Media(_) => (StatusCode::UNPROCESSABLE_ENTITY, "media"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        };
        Self::new(status, code, e.to_string())
    }
}

impl From<vi_index::IndexError> for ApiError {
    fn from(e: vi_index::IndexError) -> Self {
        use vi_index::IndexError as I;
        let (status, code) = match &e {
            I::NotAnIndex(_) => (StatusCode::NOT_FOUND, "not_found"),
            I::Invalid(_) => (StatusCode::BAD_REQUEST, "invalid"),
            I::Unsupported(_) => (StatusCode::NOT_IMPLEMENTED, "unsupported"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "storage"),
        };
        Self::new(status, code, e.to_string())
    }
}

impl From<vi_media::MediaError> for ApiError {
    fn from(e: vi_media::MediaError) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, "media", e.to_string())
    }
}

/// Handler result.
pub type ApiResult<T> = Result<T, ApiError>;
