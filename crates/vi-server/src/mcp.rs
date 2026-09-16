//! MCP over streamable HTTP: JSON-RPC 2.0 on `POST /v1/mcp` (default index)
//! and `POST /v1/indexes/{index}/mcp`. Stateless: every request carries what
//! it needs, `GET` (server-initiated streams) is not offered, and
//! notifications are acknowledged with 202. Tools are the agent's own
//! (`search`, `list_videos`, `timeline`, `get_transcript`, `get_ocr`,
//! `get_descriptions`, `view`, `describe`) plus `index_state` and `ask`.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::Engine;
use serde_json::{json, Value};
use vi_agent::tools::{execute, specs, ToolCall};
use vi_index::Storage;
use vi_providers::ImageData;

use crate::auth::ApiKey;
use crate::error::ApiError;
use crate::indexes::{to_json, tool_context};
use crate::query::{ask_stream, collect, run_search, AskBody, BudgetBody, SearchBody};
use crate::state::{AppState, OpenIndex};

const PROTOCOL: &str = "2025-06-18";

/// `GET /v1/mcp`: no server-initiated stream.
pub async fn get() -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        Json(json!({"error": "this MCP server is stateless; POST JSON-RPC requests"})),
    )
        .into_response()
}

/// `POST /v1/mcp`.
pub async fn handle_default(
    State(state): State<Arc<AppState>>,
    Extension(key): Extension<ApiKey>,
    Json(body): Json<Value>,
) -> Response {
    if !state.config.server.mcp {
        return ApiError::not_found("MCP is disabled (server.mcp = false)").into_response();
    }
    let ix = match state.default_index() {
        Ok(ix) => ix,
        Err(e) => return e.into_response(),
    };
    dispatch(&state, &key, &ix, body).await
}

/// `POST /v1/indexes/{index}/mcp`.
pub async fn handle_for_index(
    State(state): State<Arc<AppState>>,
    Extension(key): Extension<ApiKey>,
    Path(index): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    if !state.config.server.mcp {
        return ApiError::not_found("MCP is disabled (server.mcp = false)").into_response();
    }
    let ix = match state.index(&index) {
        Ok(ix) => ix,
        Err(e) => return e.into_response(),
    };
    dispatch(&state, &key, &ix, body).await
}

async fn dispatch(state: &AppState, key: &ApiKey, ix: &OpenIndex, body: Value) -> Response {
    match body {
        Value::Array(items) => {
            let mut out = Vec::new();
            for item in items {
                if let Some(r) = handle_one(state, key, ix, item).await {
                    out.push(r);
                }
            }
            if out.is_empty() {
                StatusCode::ACCEPTED.into_response()
            } else {
                Json(Value::Array(out)).into_response()
            }
        }
        item => match handle_one(state, key, ix, item).await {
            Some(r) => Json(r).into_response(),
            None => StatusCode::ACCEPTED.into_response(),
        },
    }
}

fn rpc_error(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message.into()}})
}

fn rpc_ok(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

/// The tool list.
pub fn tool_list(state: &AppState) -> Vec<Value> {
    let with_describe = state
        .providers
        .has_role(vi_core::config::roles::VLM_DESCRIBE);
    let mut tools: Vec<Value> = specs(with_describe)
        .into_iter()
        .map(|s| json!({"name": s.name, "description": s.description, "inputSchema": s.parameters}))
        .collect();
    tools.push(json!({
        "name": "index_state",
        "description": "The index: videos with ids, titles, durations and index state (coarse/fine), sizes and counts.",
        "inputSchema": {"type": "object", "properties": {}}
    }));
    tools.push(json!({
        "name": "ask",
        "description": "Ask VideoIndex's own agent a question over the index; returns an answer with timestamp citations [[cite:VIDEO_ID:T0-T1]] and the usage. Slower and costlier than the primitive tools; use when you want a finished answer.",
        "inputSchema": {"type": "object", "properties": {
            "question": {"type": "string"},
            "video_id": {"type": "string", "description": "Restrict to one video."},
            "max_tool_calls": {"type": "integer", "default": 6},
            "max_cost_usd": {"type": "number", "default": 0.3},
            "model": {"type": "string", "description": "Chat model: a provider name or model id from GET /v1/models; default is the agent_llm role."}
        }, "required": ["question"]}
    }));
    tools
}

async fn handle_one(state: &AppState, key: &ApiKey, ix: &OpenIndex, req: Value) -> Option<Value> {
    let id = req.get("id").cloned().unwrap_or(Value::Null);
    let method = req.get("method").and_then(Value::as_str).unwrap_or("");
    let params = req.get("params").cloned().unwrap_or(json!({}));
    if req.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Some(rpc_error(id, -32600, "jsonrpc must be \"2.0\""));
    }
    // Notifications carry no id and get no response.
    let is_notification = req.get("id").is_none();
    if method.starts_with("notifications/") || is_notification {
        return None;
    }
    match method {
        "initialize" => Some(rpc_ok(
            id,
            json!({
                "protocolVersion": params.get("protocolVersion").cloned().unwrap_or(json!(PROTOCOL)),
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "videoindex", "version": env!("CARGO_PKG_VERSION")},
                "instructions": format!("Tools query the VideoIndex index '{}'. Times are seconds; cite moments as [[cite:VIDEO_ID:T0-T1]].", ix.id),
            }),
        )),
        "ping" => Some(rpc_ok(id, json!({}))),
        "tools/list" => Some(rpc_ok(id, json!({"tools": tool_list(state)}))),
        "resources/list" => Some(rpc_ok(id, json!({"resources": []}))),
        "prompts/list" => Some(rpc_ok(id, json!({"prompts": []}))),
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            state.metrics.mcp_calls.fetch_add(1, Ordering::Relaxed);
            Some(match call_tool(state, key, ix, &name, args).await {
                Ok(result) => rpc_ok(id, result),
                Err(e) => rpc_ok(
                    id,
                    json!({"content": [{"type": "text", "text": e.message}], "isError": true}),
                ),
            })
        }
        other => Some(rpc_error(id, -32601, format!("method not found: {other}"))),
    }
}

async fn call_tool(
    state: &AppState,
    key: &ApiKey,
    ix: &OpenIndex,
    name: &str,
    args: Value,
) -> Result<Value, ApiError> {
    match name {
        "index_state" => {
            let stats = ix.storage.stats().await?;
            let videos = ix.storage.list_videos().await?;
            let v = json!({
                "id": ix.id,
                "videos": to_json(&videos)?,
                "dir_bytes": stats.dir_bytes,
                "blob_count": stats.blob_count,
            });
            Ok(
                json!({"content": [{"type": "text", "text": v.to_string()}], "structuredContent": v}),
            )
        }
        "ask" => {
            let question = args
                .get("question")
                .and_then(Value::as_str)
                .ok_or_else(|| ApiError::bad_request("ask needs a question"))?
                .to_string();
            let videos = args
                .get("video_id")
                .and_then(Value::as_str)
                .map(|s| vec![s.to_string()])
                .unwrap_or_default();
            let body = AskBody {
                question,
                videos,
                budget: BudgetBody {
                    max_tool_calls: args
                        .get("max_tool_calls")
                        .and_then(Value::as_u64)
                        .map(|n| n as u32),
                    max_cost_usd: args.get("max_cost_usd").and_then(Value::as_f64),
                    ..BudgetBody::default()
                },
                session_id: None,
                policy: "agent".into(),
                model: args.get("model").and_then(Value::as_str).map(str::to_string),
            };
            let stream = ask_stream(state, ix, body)?;
            let v = collect(state, &key.bucket(), stream).await;
            let text = v
                .get("answer")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            Ok(json!({"content": [{"type": "text", "text": text}], "structuredContent": v}))
        }
        "search" => {
            // Same arguments as the agent's tool, answered with the HTTP
            // search so MCP callers get the full hit objects.
            let body = SearchBody {
                query: args
                    .get("query")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                k: args.get("k").and_then(Value::as_u64).unwrap_or(8) as usize,
                videos: args
                    .get("video_id")
                    .and_then(Value::as_str)
                    .map(|s| vec![s.to_string()])
                    .unwrap_or_default(),
                kinds: args
                    .get("kind")
                    .and_then(Value::as_str)
                    .map(|s| vec![s.to_string()])
                    .unwrap_or_default(),
                text_only: false,
            };
            if body.query.is_empty() {
                return Err(ApiError::bad_request("search needs a query"));
            }
            let v = run_search(state, ix, body).await?;
            Ok(
                json!({"content": [{"type": "text", "text": v.to_string()}], "structuredContent": v}),
            )
        }
        _ => {
            let videos = args
                .get("video_id")
                .and_then(Value::as_str)
                .and_then(|s| vi_core::VideoId::parse(s).ok())
                .map(|v| vec![v])
                .unwrap_or_default();
            let ctx = tool_context(state, ix, videos);
            let out = execute(
                &ctx,
                &ToolCall {
                    id: "mcp".into(),
                    name: name.to_string(),
                    args,
                    signature: None,
                },
            )
            .await?;
            if out.cost_usd > 0.0 {
                if let Err(e) = state.charge(&key.bucket(), out.cost_usd) {
                    tracing::warn!(key = %key.label(), "{}", e.message);
                }
            }
            let mut content = vec![json!({"type": "text", "text": out.content})];
            if let Some(img) = out.image {
                let (mime, bytes) = match img {
                    ImageData::Encoded { mime, bytes } => (mime, bytes.to_vec()),
                    ImageData::Rgb8 { .. } => ("", Vec::new()),
                };
                if !bytes.is_empty() {
                    content.push(json!({
                        "type": "image",
                        "mimeType": mime,
                        "data": base64::engine::general_purpose::STANDARD.encode(&bytes),
                    }));
                }
            }
            let structured: Value =
                serde_json::from_str(content[0]["text"].as_str().unwrap_or("null"))
                    .unwrap_or(Value::Null);
            let mut result = json!({"content": content});
            if structured.is_object() || structured.is_array() {
                result["structuredContent"] = structured;
            }
            Ok(result)
        }
    }
}
