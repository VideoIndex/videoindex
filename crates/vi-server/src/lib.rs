//! `vi-server`: the HTTP API (`docs/09-sdk-and-apis.md`), Server-Sent Events
//! for jobs and answers, blob serving, bearer API keys with a daily spend cap,
//! and an MCP endpoint (streamable HTTP, JSON-RPC 2.0) exposing the agent's
//! tools (`docs/06-query-and-agents.md`). One process serves every index
//! under `server.index_root`; indexes open lazily on first use.

pub mod auth;
pub mod error;
pub mod indexes;
pub mod jobs;
pub mod mcp;
pub mod metrics;
pub mod openapi;
pub mod query;
pub mod state;

use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::http::{header, HeaderName, HeaderValue, Method};
use axum::middleware;
use axum::routing::{get, post};
use axum::Router;
use tokio_util::sync::CancellationToken;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::trace::TraceLayer;
use vi_core::config::Config;

pub use state::AppState;

/// Build the router. `/healthz`, `/metrics` and `/v1/openapi.json` are open;
/// everything under `/v1` else requires a key when keys are configured.
pub fn router(state: Arc<AppState>) -> Router {
    let api = Router::new()
        .route("/models", get(indexes::models))
        .route("/indexes", get(indexes::list).post(indexes::create))
        .route("/indexes/{index}", get(indexes::status))
        .route(
            "/indexes/{index}/videos",
            get(indexes::videos).post(jobs::add_videos),
        )
        .route("/indexes/{index}/videos/{video}", get(indexes::video))
        .route(
            "/indexes/{index}/videos/{video}/timeline",
            get(indexes::timeline),
        )
        .route(
            "/indexes/{index}/videos/{video}/transcript",
            get(indexes::transcript),
        )
        .route("/indexes/{index}/search", post(query::search))
        .route("/indexes/{index}/ask", post(query::ask))
        .route("/indexes/{index}/view", post(indexes::view))
        .route("/indexes/{index}/blobs/{*key}", get(indexes::blob))
        .route(
            "/indexes/{index}/mcp",
            post(mcp::handle_for_index).get(mcp::get),
        )
        .route("/jobs", get(jobs::list))
        .route("/jobs/{job}", get(jobs::get).delete(jobs::cancel))
        .route("/mcp", post(mcp::handle_default).get(mcp::get))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_key,
        ));
    let open = Router::new()
        .route("/healthz", get(indexes::healthz))
        .route("/metrics", get(metrics::render))
        .route("/v1/openapi.json", get(openapi::document));
    let mut app = Router::new()
        .merge(open)
        .nest("/v1", api)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            metrics::record,
        ))
        .layer(DefaultBodyLimit::max(state.config.server.max_body_bytes))
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone());
    let origins = &state.config.server.cors_origins;
    if !origins.is_empty() {
        let allow = if origins.iter().any(|o| o == "*") {
            AllowOrigin::any()
        } else {
            AllowOrigin::list(origins.iter().filter_map(|o| HeaderValue::from_str(o).ok()))
        };
        app = app.layer(
            CorsLayer::new()
                .allow_origin(allow)
                .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
                .allow_headers([
                    header::AUTHORIZATION,
                    header::CONTENT_TYPE,
                    header::ACCEPT,
                    HeaderName::from_static("x-api-key"),
                ])
                .expose_headers(Any),
        );
    }
    app
}

/// Bind and serve until `cancel` fires.
pub async fn serve(config: Arc<Config>, cancel: CancellationToken) -> vi_core::Result<()> {
    let state = Arc::new(AppState::new(config.clone()));
    let listener = tokio::net::TcpListener::bind(&config.server.bind)
        .await
        .map_err(|e| vi_core::Error::Config(format!("cannot bind {}: {e}", config.server.bind)))?;
    let addr = listener
        .local_addr()
        .map_err(|e| vi_core::Error::Config(e.to_string()))?;
    if config.server.api_keys.is_empty() {
        tracing::warn!(%addr, "no api_keys configured: every request is accepted (self-hosted mode)");
    }
    tracing::info!(%addr, index_root = %config.server.index_root.display(), mcp = config.server.mcp, "vi-server listening");
    let app = router(state);
    axum::serve(listener, app)
        .with_graceful_shutdown(async move { cancel.cancelled().await })
        .await
        .map_err(|e| vi_core::Error::Other(format!("server: {e}")))
}
