//! Bearer API keys. With no keys configured every request passes (self-hosted
//! on localhost); otherwise `Authorization: Bearer <key>` or `X-API-Key`.
//! The key (or `anonymous`) travels in request extensions for spend
//! accounting.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::error::ApiError;
use crate::state::AppState;

/// The caller's key, for quotas and logs.
#[derive(Debug, Clone)]
pub struct ApiKey(pub String);

impl ApiKey {
    /// A short, non-secret label for logs and quota buckets.
    pub fn label(&self) -> String {
        if self.0 == "anonymous" {
            return self.0.clone();
        }
        let n = self.0.len();
        if n <= 8 {
            "key".to_string()
        } else {
            format!("{}…{}", &self.0[..4], &self.0[n - 4..])
        }
    }
}

/// Middleware.
pub async fn require_key(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Response {
    let keys = &state.config.server.api_keys;
    if keys.is_empty() {
        req.extensions_mut().insert(ApiKey("anonymous".into()));
        return next.run(req).await;
    }
    let presented = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
        })
        .map(str::trim)
        .map(str::to_string)
        .or_else(|| {
            req.headers()
                .get("x-api-key")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
        });
    match presented {
        Some(k) if keys.iter().any(|known| constant_eq(known, &k)) => {
            req.extensions_mut().insert(ApiKey(k));
            next.run(req).await
        }
        Some(_) => ApiError::unauthorized("unknown API key").into_response(),
        None => ApiError::unauthorized("provide Authorization: Bearer <key> or X-API-Key")
            .into_response(),
    }
}

fn constant_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}
