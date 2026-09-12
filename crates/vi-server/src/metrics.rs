//! Prometheus text metrics without a metrics crate: request counts and
//! latency buckets per route template, jobs, asks, tool calls and provider
//! spend.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::extract::{MatchedPath, Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::state::AppState;

const BUCKETS: &[f64] = &[0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0];

/// Per-route latency: bucket counts, sum of seconds, count.
type Latency = ([u64; 11], f64, u64);

/// Counters.
#[derive(Default)]
pub struct Metrics {
    requests: Mutex<BTreeMap<(String, u16), u64>>,
    latency: Mutex<BTreeMap<String, Latency>>,
    /// `ask` calls.
    pub asks: AtomicU64,
    /// Tool calls inside asks.
    pub tool_calls: AtomicU64,
    /// Jobs started.
    pub jobs_started: AtomicU64,
    /// Jobs finished ok.
    pub jobs_ok: AtomicU64,
    /// Jobs failed or cancelled.
    pub jobs_failed: AtomicU64,
    /// Running jobs.
    pub jobs_running: AtomicU64,
    /// MCP tool calls.
    pub mcp_calls: AtomicU64,
    cost_micros: AtomicU64,
}

impl Metrics {
    /// Record one request.
    pub fn observe(&self, route: &str, status: u16, secs: f64) {
        if let Ok(mut m) = self.requests.lock() {
            *m.entry((route.to_string(), status)).or_default() += 1;
        }
        if let Ok(mut l) = self.latency.lock() {
            let e = l.entry(route.to_string()).or_insert(([0; 11], 0.0, 0));
            let mut placed = false;
            for (i, b) in BUCKETS.iter().enumerate() {
                if secs <= *b {
                    e.0[i] += 1;
                    placed = true;
                    break;
                }
            }
            if !placed {
                e.0[10] += 1;
            }
            e.1 += secs;
            e.2 += 1;
        }
    }

    /// Add provider spend.
    pub fn add_cost(&self, usd: f64) {
        self.cost_micros
            .fetch_add((usd.max(0.0) * 1e6) as u64, Ordering::Relaxed);
    }

    /// Prometheus exposition text.
    pub fn render_text(&self) -> String {
        let mut out = String::new();
        out.push_str("# TYPE vi_http_requests_total counter\n");
        if let Ok(m) = self.requests.lock() {
            for ((route, status), n) in m.iter() {
                out.push_str(&format!(
                    "vi_http_requests_total{{route=\"{route}\",status=\"{status}\"}} {n}\n"
                ));
            }
        }
        out.push_str("# TYPE vi_http_request_duration_seconds histogram\n");
        if let Ok(l) = self.latency.lock() {
            for (route, (buckets, sum, count)) in l.iter() {
                let mut cum = 0;
                for (i, b) in BUCKETS.iter().enumerate() {
                    cum += buckets[i];
                    out.push_str(&format!(
                        "vi_http_request_duration_seconds_bucket{{route=\"{route}\",le=\"{b}\"}} {cum}\n"
                    ));
                }
                cum += buckets[10];
                out.push_str(&format!(
                    "vi_http_request_duration_seconds_bucket{{route=\"{route}\",le=\"+Inf\"}} {cum}\n"
                ));
                out.push_str(&format!(
                    "vi_http_request_duration_seconds_sum{{route=\"{route}\"}} {sum}\n"
                ));
                out.push_str(&format!(
                    "vi_http_request_duration_seconds_count{{route=\"{route}\"}} {count}\n"
                ));
            }
        }
        let g = |name: &str, kind: &str, v: u64| format!("# TYPE {name} {kind}\n{name} {v}\n");
        out.push_str(&g(
            "vi_asks_total",
            "counter",
            self.asks.load(Ordering::Relaxed),
        ));
        out.push_str(&g(
            "vi_ask_tool_calls_total",
            "counter",
            self.tool_calls.load(Ordering::Relaxed),
        ));
        out.push_str(&g(
            "vi_mcp_tool_calls_total",
            "counter",
            self.mcp_calls.load(Ordering::Relaxed),
        ));
        out.push_str(&g(
            "vi_jobs_started_total",
            "counter",
            self.jobs_started.load(Ordering::Relaxed),
        ));
        out.push_str(&g(
            "vi_jobs_ok_total",
            "counter",
            self.jobs_ok.load(Ordering::Relaxed),
        ));
        out.push_str(&g(
            "vi_jobs_failed_total",
            "counter",
            self.jobs_failed.load(Ordering::Relaxed),
        ));
        out.push_str(&g(
            "vi_jobs_running",
            "gauge",
            self.jobs_running.load(Ordering::Relaxed),
        ));
        out.push_str(&format!(
            "# TYPE vi_provider_cost_usd_total counter\nvi_provider_cost_usd_total {}\n",
            self.cost_micros.load(Ordering::Relaxed) as f64 / 1e6
        ));
        out
    }
}

/// Middleware recording every request.
pub async fn record(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| "unmatched".to_string());
    let started = Instant::now();
    let resp = next.run(req).await;
    state.metrics.observe(
        &route,
        resp.status().as_u16(),
        started.elapsed().as_secs_f64(),
    );
    resp
}

/// `GET /metrics`.
pub async fn render(State(state): State<Arc<AppState>>) -> Response {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )],
        state.metrics.render_text(),
    )
        .into_response()
}

impl std::fmt::Debug for Metrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Metrics")
            .field("asks", &self.asks)
            .field("jobs_running", &self.jobs_running)
            .finish_non_exhaustive()
    }
}
