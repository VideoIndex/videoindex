//! Indexing jobs: `POST /v1/indexes/{index}/videos` starts one over a list of
//! sources; `GET /v1/jobs/{job}` reports it as JSON or, with
//! `Accept: text/event-stream`, streams the pipeline's events until it ends.

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use axum::extract::{Extension, Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, Utc};
use futures::Stream;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use vi_core::time::flatten_timestamps_json;
use vi_core::{EventBus, JobId};
use vi_media::Source;
use vi_pipeline::{JobOptions, Scheduler};

use crate::auth::ApiKey;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// Where a job is.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    /// Sources still being indexed.
    Running,
    /// Every source processed.
    Finished,
    /// Stopped by `DELETE`.
    Cancelled,
    /// Aborted by an error.
    Failed,
}

/// One job.
pub struct JobEntry {
    /// Id.
    pub id: JobId,
    /// Index it writes to.
    pub index: String,
    /// Sources as given.
    pub sources: Vec<String>,
    /// Policy name.
    pub policy: Option<String>,
    /// Start.
    pub started_at: DateTime<Utc>,
    /// Owner key label.
    pub key: String,
    status: RwLock<JobStatus>,
    reports: RwLock<Vec<Value>>,
    error: RwLock<Option<String>>,
    /// Pipeline events.
    pub events: EventBus,
    /// Cancel.
    pub cancel: CancellationToken,
}

impl JobEntry {
    /// Snapshot as JSON.
    pub fn to_json(&self) -> Value {
        let status = self
            .status
            .read()
            .map(|s| s.clone())
            .unwrap_or(JobStatus::Failed);
        let reports = self.reports.read().map(|r| r.clone()).unwrap_or_default();
        let error = self.error.read().ok().and_then(|e| e.clone());
        let ok = reports
            .iter()
            .filter(|r| r["ok"].as_bool() == Some(true))
            .count();
        json!({
            "job_id": self.id.to_string(),
            "index": self.index,
            "status": status,
            "policy": self.policy,
            "sources": self.sources,
            "started_at": self.started_at,
            "done": reports.len(),
            "ok": ok,
            "reports": reports,
            "error": error,
        })
    }

    fn status(&self) -> JobStatus {
        self.status
            .read()
            .map(|s| s.clone())
            .unwrap_or(JobStatus::Failed)
    }
}

/// All jobs of this process.
#[derive(Default)]
pub struct JobRegistry {
    inner: RwLock<HashMap<String, Arc<JobEntry>>>,
}

impl JobRegistry {
    /// Look up.
    pub fn get(&self, id: &str) -> Option<Arc<JobEntry>> {
        self.inner.read().ok().and_then(|m| m.get(id).cloned())
    }

    /// All entries, newest first.
    pub fn all(&self) -> Vec<Arc<JobEntry>> {
        let mut v: Vec<Arc<JobEntry>> = self
            .inner
            .read()
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default();
        v.sort_by_key(|e| std::cmp::Reverse(e.started_at));
        v
    }

    /// Finished jobs kept for `GET /v1/jobs` before the oldest are dropped.
    const KEEP_FINISHED: usize = 500;

    fn insert(&self, e: Arc<JobEntry>) {
        if let Ok(mut m) = self.inner.write() {
            m.insert(e.id.to_string(), e);
            let mut finished: Vec<(chrono::DateTime<chrono::Utc>, String)> = m
                .values()
                .filter(|j| j.status() != JobStatus::Running)
                .map(|j| (j.started_at, j.id.to_string()))
                .collect();
            if finished.len() > Self::KEEP_FINISHED {
                finished.sort();
                for (_, id) in finished.iter().take(finished.len() - Self::KEEP_FINISHED) {
                    m.remove(id);
                }
            }
        }
    }
}

/// Body of `POST /v1/indexes/{index}/videos`.
#[derive(Debug, Deserialize)]
pub struct AddVideos {
    /// Local paths, `s3://`, `https://`, YouTube URLs, or directories.
    pub sources: Vec<String>,
    /// Policy name; the config default when absent.
    pub policy: Option<String>,
    /// Ignore the operator cache.
    #[serde(default)]
    pub force: bool,
}

/// `POST /v1/indexes/{index}/videos` → 202 with the job id.
pub async fn add_videos(
    State(state): State<Arc<AppState>>,
    Extension(key): Extension<ApiKey>,
    Path(index): Path<String>,
    Json(body): Json<AddVideos>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    if body.sources.is_empty() {
        return Err(ApiError::bad_request("sources must not be empty"));
    }
    let ix = state.index(&index)?;
    state.check_cap(&key.bucket())?;
    if let Some(p) = &body.policy {
        state.config.policy(p)?;
    }
    let events = EventBus::default();
    let entry = Arc::new(JobEntry {
        id: JobId::new(),
        index: ix.id.clone(),
        sources: body.sources.clone(),
        policy: body.policy.clone(),
        started_at: Utc::now(),
        key: key.label(),
        status: RwLock::new(JobStatus::Running),
        reports: RwLock::new(Vec::new()),
        error: RwLock::new(None),
        events: events.clone(),
        cancel: CancellationToken::new(),
    });
    state.jobs.insert(entry.clone());
    state.metrics.jobs_started.fetch_add(1, Ordering::Relaxed);
    state.metrics.jobs_running.fetch_add(1, Ordering::Relaxed);
    let sched = Scheduler::with_providers(
        ix.storage.clone(),
        state.config.clone(),
        events,
        state.providers.clone(),
    );
    let st = state.clone();
    let e2 = entry.clone();
    let key_label = key.bucket();
    tokio::spawn(async move {
        // Like `vidx index`: a failing source is recorded and the job goes on
        // with the next one; the job is Failed only when nothing succeeded.
        let mut errors: Vec<String> = Vec::new();
        let mut succeeded = 0usize;
        'outer: for src in &e2.sources {
            let source = Source::parse(src);
            let expanded = match sched.expand(&source).await {
                Ok(v) => v,
                Err(e) => {
                    errors.push(format!("{src}: {e}"));
                    continue;
                }
            };
            for s in expanded {
                if e2.cancel.is_cancelled() {
                    break 'outer;
                }
                let uri = s.uri();
                match sched
                    .run(
                        s,
                        JobOptions {
                            policy: e2.policy.clone(),
                            force: body.force,
                            ..JobOptions::default()
                        },
                        e2.cancel.child_token(),
                    )
                    .await
                {
                    Ok(r) => {
                        if let Err(e) = st.charge(&key_label, r.budget.cost_usd) {
                            tracing::warn!(job = %e2.id, "{}", e.message);
                        }
                        if r.ok {
                            succeeded += 1;
                        } else {
                            errors.push(format!("{uri}: one or more stages failed"));
                        }
                        let mut v = serde_json::to_value(&r).unwrap_or(Value::Null);
                        flatten_timestamps_json(&mut v);
                        if let Ok(mut reps) = e2.reports.write() {
                            reps.push(v);
                        }
                    }
                    Err(vi_core::Error::Cancelled) => break 'outer,
                    Err(e) => errors.push(format!("{uri}: {e}")),
                }
            }
        }
        if !errors.is_empty() {
            set(&e2.error, Some(errors.join("; ")));
        }
        let final_status = if e2.cancel.is_cancelled() {
            JobStatus::Cancelled
        } else if succeeded == 0 && !errors.is_empty() {
            JobStatus::Failed
        } else {
            JobStatus::Finished
        };
        if let Ok(mut s) = e2.status.write() {
            *s = final_status.clone();
        }
        st.metrics.jobs_running.fetch_sub(1, Ordering::Relaxed);
        if final_status == JobStatus::Finished {
            st.metrics.jobs_ok.fetch_add(1, Ordering::Relaxed);
        } else {
            st.metrics.jobs_failed.fetch_add(1, Ordering::Relaxed);
        }
    });
    Ok((
        StatusCode::ACCEPTED,
        Json(
            json!({"job_id": entry.id.to_string(), "status": "running", "sources": entry.sources.len()}),
        ),
    ))
}

fn set(slot: &RwLock<Option<String>>, v: Option<String>) {
    if let Ok(mut s) = slot.write() {
        *s = v;
    }
}

/// `GET /v1/jobs`.
pub async fn list(State(state): State<Arc<AppState>>) -> Json<Value> {
    let jobs: Vec<Value> = state
        .jobs
        .all()
        .iter()
        .map(|e| {
            let mut v = e.to_json();
            if let Value::Object(m) = &mut v {
                m.remove("reports");
            }
            v
        })
        .collect();
    Json(json!({"jobs": jobs}))
}

fn wants_sse(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|a| a.contains("text/event-stream"))
        .unwrap_or(false)
}

/// `GET /v1/jobs/{job}`: JSON, or SSE with `Accept: text/event-stream`.
pub async fn get(
    State(state): State<Arc<AppState>>,
    Path(job): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let entry = state
        .jobs
        .get(&job)
        .ok_or_else(|| ApiError::not_found(format!("no job {job}")))?;
    if !wants_sse(&headers) {
        return Ok(Json(entry.to_json()).into_response());
    }
    let stream = job_events(entry);
    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response())
}

fn job_events(entry: Arc<JobEntry>) -> impl Stream<Item = Result<SseEvent, Infallible>> {
    async_stream::stream! {
        let mut rx = entry.events.subscribe();
        yield Ok(SseEvent::default().event("job").data(entry.to_json().to_string()));
        loop {
            if entry.status() != JobStatus::Running {
                yield Ok(SseEvent::default().event("job").data(entry.to_json().to_string()));
                break;
            }
            match tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
                Ok(Ok(ev)) => {
                    let mut v = serde_json::to_value(&ev).unwrap_or(Value::Null);
                    flatten_timestamps_json(&mut v);
                    let name = v.get("type").and_then(Value::as_str).unwrap_or("event").to_string();
                    yield Ok(SseEvent::default().event(name).data(v.to_string()));
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(n))) => {
                    yield Ok(SseEvent::default().event("lagged").data(json!({"dropped": n}).to_string()));
                }
                Ok(Err(_)) => break,
                Err(_) => {}
            }
        }
    }
}

/// `DELETE /v1/jobs/{job}`.
pub async fn cancel(
    State(state): State<Arc<AppState>>,
    Path(job): Path<String>,
) -> ApiResult<Json<Value>> {
    let entry = state
        .jobs
        .get(&job)
        .ok_or_else(|| ApiError::not_found(format!("no job {job}")))?;
    entry.cancel.cancel();
    Ok(Json(json!({"job_id": job, "cancelled": true})))
}

impl std::fmt::Debug for JobEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobEntry")
            .field("id", &self.id)
            .field("index", &self.index)
            .field("policy", &self.policy)
            .field("started_at", &self.started_at)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for JobRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobRegistry").finish_non_exhaustive()
    }
}
