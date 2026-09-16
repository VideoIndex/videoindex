//! Index, video, timeline, transcript, view and blob handlers.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use vi_agent::tools::{execute, ToolCall, ToolContext};
use vi_core::model::SegmentLevel;
use vi_core::time::flatten_timestamps_json;
use vi_core::VideoId;
use vi_index::{BlobKey, Storage};

use crate::error::{ApiError, ApiResult};
use crate::state::{AppState, OpenIndex};

/// Serialize with rational timestamps flattened to seconds.
pub fn to_json<T: serde::Serialize>(v: &T) -> ApiResult<Value> {
    let mut value = serde_json::to_value(v).map_err(|e| ApiError::internal(e.to_string()))?;
    flatten_timestamps_json(&mut value);
    Ok(value)
}

fn parse_video(s: &str) -> ApiResult<VideoId> {
    VideoId::parse(s).map_err(|_| ApiError::bad_request(format!("'{s}' is not a video id")))
}

/// `GET /healthz`.
pub async fn healthz(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_secs": state.started.elapsed().as_secs(),
        "indexes": state.index_ids(),
        "mcp": state.config.server.mcp,
        "auth": !state.config.server.api_keys.is_empty(),
    }))
}

/// `GET /v1/models`: the chat models `ask` accepts as `model`.
pub async fn models(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({"models": state.providers.llm_providers()}))
}

/// `GET /v1/indexes`.
pub async fn list(State(state): State<Arc<AppState>>) -> Json<Value> {
    let items: Vec<Value> = state
        .index_ids()
        .into_iter()
        .map(|id| json!({"id": id, "path": state.index_path(&id)}))
        .collect();
    Json(json!({"indexes": items}))
}

/// Body of `POST /v1/indexes`.
#[derive(Debug, Deserialize)]
pub struct CreateIndex {
    /// Id.
    pub id: String,
}

/// `POST /v1/indexes`.
pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateIndex>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let ix = state.create_index(&body.id)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({"id": ix.id, "path": ix.path})),
    ))
}

/// `GET /v1/indexes/{index}`.
pub async fn status(
    State(state): State<Arc<AppState>>,
    Path(index): Path<String>,
) -> ApiResult<Json<Value>> {
    let ix = state.index(&index)?;
    let stats = ix.storage.stats().await?;
    let mut v = to_json(&stats)?;
    if let Value::Object(m) = &mut v {
        m.insert("id".into(), json!(ix.id));
    }
    Ok(Json(v))
}

/// `GET /v1/indexes/{index}/videos`.
pub async fn videos(
    State(state): State<Arc<AppState>>,
    Path(index): Path<String>,
) -> ApiResult<Json<Value>> {
    let ix = state.index(&index)?;
    let videos = ix.storage.list_videos().await?;
    Ok(Json(json!({"videos": to_json(&videos)?})))
}

/// `GET /v1/indexes/{index}/videos/{video}`.
pub async fn video(
    State(state): State<Arc<AppState>>,
    Path((index, video)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    let ix = state.index(&index)?;
    let vid = parse_video(&video)?;
    let v = ix
        .storage
        .get_video(vid)
        .await?
        .ok_or_else(|| ApiError::not_found(format!("no video {vid}")))?;
    let tracks = ix.storage.tracks(vid).await?;
    Ok(Json(
        json!({"video": to_json(&v)?, "tracks": to_json(&tracks)?}),
    ))
}

/// Query of the timeline endpoint.
#[derive(Debug, Deserialize)]
pub struct TimelineQuery {
    /// `chapter` (default), `scene` or `shot`.
    #[serde(default = "default_level")]
    pub level: String,
}

fn default_level() -> String {
    "chapter".into()
}

/// `GET /v1/indexes/{index}/videos/{video}/timeline?level=`.
pub async fn timeline(
    State(state): State<Arc<AppState>>,
    Path((index, video)): Path<(String, String)>,
    Query(q): Query<TimelineQuery>,
) -> ApiResult<Json<Value>> {
    let ix = state.index(&index)?;
    let vid = parse_video(&video)?;
    let level = SegmentLevel::parse(&q.level)
        .ok_or_else(|| ApiError::bad_request("level must be chapter, scene or shot"))?;
    let segs = ix.storage.segments(vid, level).await?;
    Ok(Json(json!({"level": q.level, "segments": to_json(&segs)?})))
}

/// Query of the transcript endpoint.
#[derive(Debug, Deserialize)]
pub struct WindowQuery {
    /// Start, seconds.
    #[serde(default)]
    pub t0: f64,
    /// End, seconds; defaults to the whole video.
    pub t1: Option<f64>,
    /// `transcript` (default), `ocr` or `description`.
    #[serde(default = "default_kind")]
    pub kind: String,
}

fn default_kind() -> String {
    "transcript".into()
}

/// The agent's tool context over an index.
pub fn tool_context(state: &AppState, ix: &OpenIndex, videos: Vec<VideoId>) -> ToolContext {
    ToolContext {
        storage: ix.storage.clone(),
        providers: state.providers.clone(),
        config: state.config.clone(),
        videos,
    }
}

/// `GET /v1/indexes/{index}/videos/{video}/transcript?t0&t1&kind=`. Runs the
/// same tool the agent uses so both see identical text.
pub async fn transcript(
    State(state): State<Arc<AppState>>,
    Path((index, video)): Path<(String, String)>,
    Query(q): Query<WindowQuery>,
) -> ApiResult<Json<Value>> {
    let ix = state.index(&index)?;
    let vid = parse_video(&video)?;
    let tool = match q.kind.as_str() {
        "transcript" => "get_transcript",
        "ocr" => "get_ocr",
        "description" | "descriptions" => "get_descriptions",
        other => {
            return Err(ApiError::bad_request(format!(
                "kind must be transcript, ocr or description, got '{other}'"
            )))
        }
    };
    let t1 = match q.t1 {
        Some(t) => t,
        None => {
            let v = ix
                .storage
                .get_video(vid)
                .await?
                .ok_or_else(|| ApiError::not_found(format!("no video {vid}")))?;
            v.duration.as_secs_f64()
        }
    };
    let ctx = tool_context(&state, &ix, vec![]);
    let out = execute(
        &ctx,
        &ToolCall {
            id: "http".into(),
            name: tool.into(),
            args: json!({"video_id": vid.to_string(), "t0": q.t0, "t1": t1}),
            signature: None,
        },
    )
    .await?;
    let content: Value = serde_json::from_str(&out.content).unwrap_or(Value::String(out.content));
    // The tool layer reports argument and lookup errors inside its content.
    if let Some(msg) = content.get("error").and_then(Value::as_str) {
        return Err(ApiError::bad_request(msg.to_string()));
    }
    Ok(Json(content))
}

/// Body of `POST /v1/indexes/{index}/view`.
#[derive(Debug, Deserialize)]
pub struct ViewBody {
    /// Video.
    pub video_id: String,
    /// Start, seconds.
    pub t0: f64,
    /// End, seconds.
    pub t1: f64,
    /// Frames per second sampled into the grid.
    #[serde(default = "default_fps")]
    pub fps: f64,
    /// Grid columns.
    #[serde(default = "default_cols")]
    pub cols: u32,
    /// Longest side of each cell.
    pub max_dim: Option<u32>,
}

fn default_fps() -> f64 {
    1.0
}

fn default_cols() -> u32 {
    3
}

/// `POST /v1/indexes/{index}/view` → PNG.
pub async fn view(
    State(state): State<Arc<AppState>>,
    Path(index): Path<String>,
    Json(body): Json<ViewBody>,
) -> ApiResult<Response> {
    let ix = state.index(&index)?;
    let vid = parse_video(&body.video_id)?;
    let v = ix
        .storage
        .get_video(vid)
        .await?
        .ok_or_else(|| ApiError::not_found(format!("no video {vid}")))?;
    let mut req = vi_agent::ViewRequest {
        t0: body.t0,
        t1: body.t1,
        fps: body.fps,
        cols: body.cols,
        ..vi_agent::ViewRequest::default()
    };
    if let Some(d) = body.max_dim {
        // Bounded: each decoded frame slot is max_dim² × 3 bytes of shared memory.
        req.max_dim = d.clamp(64, 1920);
    }
    let out = vi_agent::render_view(&state.config.media.worker, &v, req).await?;
    Ok((
        [
            (header::CONTENT_TYPE, "image/png".to_string()),
            (
                header::HeaderName::from_static("x-vi-timestamps"),
                serde_json::to_string(&out.timestamps).unwrap_or_default(),
            ),
        ],
        out.png,
    )
        .into_response())
}

/// `GET /v1/indexes/{index}/blobs/{key}`: thumbnails and grids.
pub async fn blob(
    State(state): State<Arc<AppState>>,
    Path((index, key)): Path<(String, String)>,
) -> ApiResult<Response> {
    let ix = state.index(&index)?;
    // Accept the bare 64-hex key, the `ab/cd/<hex>` path form and the
    // `blob:ab/cd/<hex>` URI that search hits carry: the key is the last segment.
    let raw = key.trim_start_matches('/');
    let raw = raw.strip_prefix("blob:").unwrap_or(raw);
    let last = raw.rsplit('/').next().unwrap_or(raw);
    let key =
        BlobKey::parse(last).map_err(|e| ApiError::bad_request(format!("bad blob key: {e}")))?;
    let bytes: Bytes = ix
        .storage
        .get_blob(&key)
        .await?
        .ok_or_else(|| ApiError::not_found("no such blob"))?;
    let mime = sniff(&bytes);
    Ok((
        [
            (header::CONTENT_TYPE, mime.to_string()),
            (
                header::CACHE_CONTROL,
                "public, max-age=31536000, immutable".to_string(),
            ),
        ],
        bytes,
    )
        .into_response())
}

fn sniff(b: &[u8]) -> &'static str {
    if b.starts_with(b"RIFF") && b.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else if b.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if b.starts_with(&[0xFF, 0xD8]) {
        "image/jpeg"
    } else {
        "application/octet-stream"
    }
}
