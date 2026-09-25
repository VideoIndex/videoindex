//! `search` and `ask`.

use std::convert::Infallible;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Extension, Path, State};
use axum::http::{header, HeaderMap};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures::{Stream, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use vi_agent::{Agent, AskBudget, AskEvent, AskRequest, RetrievalOnlyPolicy};
use vi_core::time::flatten_timestamps_json;
use vi_core::VideoId;
use vi_index::Kind;

use crate::auth::ApiKey;
use crate::error::{ApiError, ApiResult};
use crate::indexes::to_json;
use crate::state::{AppState, OpenIndex};

fn parse_videos(v: &[String]) -> ApiResult<Vec<VideoId>> {
    v.iter()
        .map(|s| {
            VideoId::parse(s).map_err(|_| ApiError::bad_request(format!("'{s}' is not a video id")))
        })
        .collect()
}

/// Body of `POST /v1/indexes/{index}/search`.
#[derive(Debug, Deserialize)]
pub struct SearchBody {
    /// Query text.
    pub query: String,
    /// Results.
    #[serde(default = "default_k")]
    pub k: usize,
    /// Restrict to videos.
    #[serde(default)]
    pub videos: Vec<String>,
    /// `transcript`, `ocr`, `description`, `frame`; empty means all.
    #[serde(default)]
    pub kinds: Vec<String>,
    /// BM25 only (no embedding calls).
    #[serde(default)]
    pub text_only: bool,
    /// At most this many hits per video; unset means no cap.
    #[serde(default)]
    pub per_video_k: Option<usize>,
}

fn default_k() -> usize {
    10
}

/// Run a search over an index; shared with MCP.
pub async fn run_search(state: &AppState, ix: &OpenIndex, body: SearchBody) -> ApiResult<Value> {
    let kinds = body
        .kinds
        .iter()
        .map(|k| match k.as_str() {
            "transcript" => Ok(Kind::Transcript),
            "ocr" => Ok(Kind::Ocr),
            "description" => Ok(Kind::Description),
            "frame" => Ok(Kind::Frame),
            other => Err(ApiError::bad_request(format!("unknown kind '{other}'"))),
        })
        .collect::<ApiResult<Vec<_>>>()?;
    let req = vi_query::SearchRequest {
        query: body.query,
        videos: parse_videos(&body.videos)?,
        kinds,
        k: body.k.clamp(1, 100),
        text_only: body.text_only,
        per_video_k: body.per_video_k.map(|v| v.clamp(1, 100)),
    };
    let resp = vi_query::search(ix.storage.as_ref(), Some(&state.providers), &req).await?;
    to_json(&resp)
}

/// `POST /v1/indexes/{index}/search`.
pub async fn search(
    State(state): State<Arc<AppState>>,
    Path(index): Path<String>,
    Json(body): Json<SearchBody>,
) -> ApiResult<Json<Value>> {
    let ix = state.index(&index)?;
    Ok(Json(run_search(&state, &ix, body).await?))
}

/// Budget fields of an `ask`.
#[derive(Debug, Deserialize, Default)]
pub struct BudgetBody {
    /// Tokens across all calls.
    pub max_tokens: Option<u64>,
    /// USD.
    pub max_cost_usd: Option<f64>,
    /// Seconds.
    pub max_wallclock_secs: Option<f64>,
    /// Tool calls.
    pub max_tool_calls: Option<u32>,
    /// Output tokens per model turn (answer length).
    pub max_answer_tokens: Option<u64>,
}

impl BudgetBody {
    fn into_budget(self) -> AskBudget {
        let d = AskBudget::default();
        AskBudget {
            max_tokens: self.max_tokens.unwrap_or(d.max_tokens),
            max_cost_usd: self.max_cost_usd.unwrap_or(d.max_cost_usd),
            max_wallclock_secs: self.max_wallclock_secs.unwrap_or(d.max_wallclock_secs),
            max_tool_calls: self.max_tool_calls.unwrap_or(d.max_tool_calls),
            max_answer_tokens: self.max_answer_tokens.unwrap_or(d.max_answer_tokens),
        }
    }
}

/// Body of `POST /v1/indexes/{index}/ask`.
#[derive(Debug, Deserialize)]
pub struct AskBody {
    /// Question.
    pub question: String,
    /// Restrict to videos.
    #[serde(default)]
    pub videos: Vec<String>,
    /// Limits.
    #[serde(default)]
    pub budget: BudgetBody,
    /// Conversation to continue.
    pub session_id: Option<String>,
    /// `agent` (default) or `retrieval-only`.
    #[serde(default = "default_policy")]
    pub policy: String,
    /// Chat model for this answer: a provider name or model id from
    /// `GET /v1/models`; default is the `agent_llm` role.
    #[serde(default)]
    pub model: Option<String>,
}

fn default_policy() -> String {
    "agent".into()
}

/// The event stream of an ask over an index; shared with MCP.
pub fn ask_stream(
    state: &AppState,
    ix: &OpenIndex,
    body: AskBody,
) -> ApiResult<impl Stream<Item = AskEvent> + Send + 'static> {
    let mut agent = Agent::new(
        ix.storage.clone(),
        state.providers.clone(),
        state.config.clone(),
    );
    match body.policy.as_str() {
        "agent" => {}
        "retrieval-only" | "retrieval_only" => {
            agent = agent.with_policy(Arc::new(RetrievalOnlyPolicy { k: 8 }));
        }
        other => return Err(ApiError::bad_request(format!("unknown policy '{other}'"))),
    }
    if let Some(model) = body
        .model
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
    {
        let provider = state.providers.find_llm_provider(model).ok_or_else(|| {
            ApiError::bad_request(format!(
                "unknown model '{model}'; GET /v1/models lists the choices"
            ))
        })?;
        agent = agent.with_provider(provider);
    }
    let req = AskRequest {
        question: body.question,
        videos: parse_videos(&body.videos)?,
        budget: body.budget.into_budget(),
        session_id: body.session_id,
    };
    state.metrics.asks.fetch_add(1, Ordering::Relaxed);
    Ok(agent.ask(req))
}

/// Collected answer for non-streaming callers.
pub async fn collect(
    state: &AppState,
    key: &str,
    stream: impl Stream<Item = AskEvent> + Send,
) -> Value {
    let mut text = String::new();
    let mut citations = Vec::new();
    let mut tools = Vec::new();
    let mut done = Value::Null;
    let mut stream = Box::pin(stream);
    while let Some(ev) = stream.next().await {
        match &ev {
            AskEvent::Token { text: t } => text.push_str(t),
            AskEvent::Citation { .. } => citations.push(to_json(&ev).unwrap_or(Value::Null)),
            AskEvent::ToolCall { .. } => {
                state.metrics.tool_calls.fetch_add(1, Ordering::Relaxed);
                tools.push(to_json(&ev).unwrap_or(Value::Null));
            }
            AskEvent::Done { usage, .. } => {
                if let Err(e) = state.charge(key, usage.cost_usd) {
                    tracing::warn!(key, "{}", e.message);
                }
                done = to_json(&ev).unwrap_or(Value::Null);
            }
            _ => {}
        }
    }
    json!({
        "answer": text,
        "citations": citations,
        "tool_calls": tools,
        "usage": done.get("usage").cloned().unwrap_or(Value::Null),
        "partial": done.get("partial").cloned().unwrap_or(json!(false)),
        "reason": done.get("reason").cloned().unwrap_or(Value::Null),
    })
}

/// `POST /v1/indexes/{index}/ask`: SSE with `Accept: text/event-stream`
/// (one event per `AskEvent`, `event:` = type), else a JSON answer.
pub async fn ask(
    State(state): State<Arc<AppState>>,
    Extension(key): Extension<ApiKey>,
    Path(index): Path<String>,
    headers: HeaderMap,
    Json(body): Json<AskBody>,
) -> ApiResult<Response> {
    let ix = state.index(&index)?;
    state.check_cap(&key.bucket())?;
    let wants_sse = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|a| a.contains("text/event-stream"))
        .unwrap_or(false);
    let stream = ask_stream(&state, &ix, body)?;
    if !wants_sse {
        return Ok(Json(collect(&state, &key.bucket(), stream).await).into_response());
    }
    let label = key.bucket();
    let sse: std::pin::Pin<Box<dyn Stream<Item = Result<SseEvent, Infallible>> + Send>> =
        Box::pin(stream.map(move |ev| {
            if let AskEvent::ToolCall { .. } = &ev {
                state.metrics.tool_calls.fetch_add(1, Ordering::Relaxed);
            }
            if let AskEvent::Done { usage, .. } = &ev {
                let _ = state.charge(&label, usage.cost_usd);
            }
            let mut v = serde_json::to_value(&ev).unwrap_or(Value::Null);
            flatten_timestamps_json(&mut v);
            let name = v
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("event")
                .to_string();
            Ok(SseEvent::default().event(name).data(v.to_string()))
        }));
    Ok(Sse::new(sse)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response())
}
