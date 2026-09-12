//! End-to-end over a real socket: create an index, index the fixture through
//! a job (JSON and SSE), read videos, timeline, blobs, search, MCP, auth,
//! metrics, OpenAPI, and `ask` against a scripted OpenAI-compatible server.
#![allow(clippy::unwrap_used)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::response::IntoResponse;
use axum::routing::post;
use axum::Router;
use futures::StreamExt;
use serde_json::{json, Value};
use vi_core::config::{Config, ProviderConfig, RoleBinding};
use vi_index::{EmbeddedIndex, Storage};
use vi_server::AppState;
use vi_testkit as fx;

async fn start(config: Config) -> (SocketAddr, Arc<AppState>) {
    let state = Arc::new(AppState::new(Arc::new(config)));
    let app = vi_server::router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (addr, state)
}

fn base_config(root: &std::path::Path) -> Config {
    let mut c = Config::default();
    c.media.worker.path = Some(fx::worker_path());
    c.media.sample_max_dim = 320;
    c.media.cache_dir = root.join("videos");
    c.server.index_root = root.join("indexes");
    c
}

async fn wait_job(client: &reqwest::Client, base: &str, job: &str) -> Value {
    for _ in 0..600 {
        let v: Value = client
            .get(format!("{base}/v1/jobs/{job}"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if v["status"] != "running" {
            return v;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    panic!("job did not finish");
}

#[tokio::test]
async fn index_job_search_timeline_blobs_and_mcp() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, _state) = start(base_config(dir.path())).await;
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();

    let h: Value = client
        .get(format!("{base}/healthz"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(h["ok"], true);
    assert_eq!(h["auth"], false);

    let r = client
        .post(format!("{base}/v1/indexes"))
        .json(&json!({"id": "t"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 201, "{}", r.text().await.unwrap());
    let r = client
        .post(format!("{base}/v1/indexes"))
        .json(&json!({"id": "t"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 409);
    let r = client
        .post(format!("{base}/v1/indexes"))
        .json(&json!({"id": "../x"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);

    // Index the fixture with the M0 policy through a job.
    let r = client
        .post(format!("{base}/v1/indexes/t/videos"))
        .json(&json!({"sources": [fx::fixture_path()], "policy": "m0"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 202);
    let job = r.json::<Value>().await.unwrap()["job_id"]
        .as_str()
        .unwrap()
        .to_string();

    // SSE progress: read until the stream ends.
    let resp = client
        .get(format!("{base}/v1/jobs/{job}"))
        .header("accept", "text/event-stream")
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.headers()["content-type"].to_str().unwrap(),
        "text/event-stream"
    );
    let mut stream = resp.bytes_stream();
    let mut text = String::new();
    while let Some(chunk) = stream.next().await {
        text.push_str(&String::from_utf8_lossy(&chunk.unwrap()));
        if text.len() > 2_000_000 {
            break;
        }
    }
    assert!(text.contains("event: job"), "{text}");
    assert!(
        text.contains("event: stage_started") || text.contains("event: progress"),
        "{text}"
    );

    let done = wait_job(&client, &base, &job).await;
    assert_eq!(done["status"], "finished", "{done}");
    assert_eq!(done["ok"], 1);
    assert_eq!(done["reports"][0]["ok"], true);

    let st: Value = client
        .get(format!("{base}/v1/indexes/t"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(st["videos"].as_array().unwrap().len(), 1);
    let videos: Value = client
        .get(format!("{base}/v1/indexes/t/videos"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let vid = videos["videos"][0]["id"].as_str().unwrap().to_string();
    assert!(
        videos["videos"][0]["duration"].is_number(),
        "timestamps are seconds: {}",
        videos["videos"][0]["duration"]
    );

    let tl: Value = client
        .get(format!(
            "{base}/v1/indexes/t/videos/{vid}/timeline?level=shot"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(tl["level"], "shot");

    // A thumbnail blob.
    let idx = EmbeddedIndex::open(&dir.path().join("indexes/t.vidx")).unwrap();
    let v = idx.list_videos().await.unwrap()[0].clone();
    let tracks = idx.tracks(v.id).await.unwrap();
    let vt = tracks
        .iter()
        .find(|t| t.kind == vi_core::model::TrackKind::Video)
        .unwrap();
    let samples = idx.frame_samples(vt.id, None).await.unwrap();
    let key = samples
        .iter()
        .find_map(|s| s.thumbnail_blob.clone())
        .unwrap();
    let r = client
        .get(format!("{base}/v1/indexes/t/blobs/{key}"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(r.headers()["content-type"], "image/webp");
    assert!(r.bytes().await.unwrap().len() > 100);
    let missing = "a".repeat(64);
    let r = client
        .get(format!("{base}/v1/indexes/t/blobs/{missing}"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404, "{}", r.text().await.unwrap());
    let r = client
        .get(format!("{base}/v1/indexes/t/blobs/not-a-key"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);

    // Search over a text-only index: no rows, but a well-formed response.
    let s: Value = client
        .post(format!("{base}/v1/indexes/t/search"))
        .json(&json!({"query": "anything", "text_only": true}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(s["hits"].is_array());
    let r = client
        .post(format!("{base}/v1/indexes/t/search"))
        .json(&json!({"query": "x", "kinds": ["bogus"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);

    // MCP: initialize, tools/list, tools/call list_videos and timeline; notification -> 202.
    let mcp = format!("{base}/v1/mcp");
    let init: Value = client
        .post(&mcp)
        .json(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(init["result"]["serverInfo"]["name"], "videoindex");
    let r = client
        .post(&mcp)
        .json(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 202);
    let tools: Value = client
        .post(&mcp)
        .json(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for n in [
        "search",
        "list_videos",
        "timeline",
        "get_transcript",
        "view",
        "index_state",
        "ask",
    ] {
        assert!(names.contains(&n), "{names:?}");
    }
    let lv: Value = client
        .post(&mcp)
        .json(&json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "list_videos", "arguments": {}}}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        lv["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains(&vid),
        "{lv}"
    );
    let bad: Value = client
        .post(&mcp)
        .json(&json!({"jsonrpc": "2.0", "id": 4, "method": "nope"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(bad["error"]["code"], -32601);
    let r = client.get(&mcp).send().await.unwrap();
    assert_eq!(r.status(), 405);

    // Metrics and OpenAPI.
    let m = client
        .get(format!("{base}/metrics"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        m.contains("vi_http_requests_total{route=\"/v1/indexes/{index}/search\",status=\"200\"}"),
        "{m}"
    );
    assert!(m.contains("vi_jobs_ok_total 1"));
    let o: Value = client
        .get(format!("{base}/v1/openapi.json"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(o["paths"]["/v1/indexes/{index}/ask"].is_object());
}

#[tokio::test]
async fn api_keys_gate_the_v1_routes_but_not_health() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = base_config(dir.path());
    c.server.api_keys = vec!["secret-key-0123456789".into()];
    let (addr, _) = start(c).await;
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    assert_eq!(
        client
            .get(format!("{base}/healthz"))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        client
            .get(format!("{base}/v1/indexes"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        client
            .get(format!("{base}/v1/indexes"))
            .bearer_auth("wrong")
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        client
            .get(format!("{base}/v1/indexes"))
            .bearer_auth("secret-key-0123456789")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        client
            .get(format!("{base}/v1/indexes"))
            .header("x-api-key", "secret-key-0123456789")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
}

/// A chat-completions endpoint that always streams "Answer." and stops.
async fn fake_llm() -> SocketAddr {
    async fn chat() -> axum::response::Response {
        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"Answer.\"},\"finish_reason\":\"stop\"}]}\n\ndata: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2}}\n\ndata: [DONE]\n\n";
        (
            [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
            body,
        )
            .into_response()
    }
    let app = Router::new().route("/v1/chat/completions", post(chat));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

#[tokio::test]
async fn ask_streams_events_and_returns_json_and_enforces_the_daily_cap() {
    let dir = tempfile::tempdir().unwrap();
    let llm = fake_llm().await;
    let mut c = base_config(dir.path());
    c.providers.insert(
        "fake".into(),
        ProviderConfig {
            adapter: "openai_compat".into(),
            base_url: Some(format!("http://{llm}/v1")),
            model: Some("fake".into()),
            pricing: Some(vi_core::config::Pricing {
                input_per_mtok: 1000.0,
                output_per_mtok: 1000.0,
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    c.roles.insert(
        "agent_llm".into(),
        RoleBinding {
            provider: "fake".into(),
            ..Default::default()
        },
    );
    c.server.daily_cost_cap_usd = 0.02;
    let (addr, _) = start(c).await;
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    client
        .post(format!("{base}/v1/indexes"))
        .json(&json!({"id": "t"}))
        .send()
        .await
        .unwrap();
    let job = client
        .post(format!("{base}/v1/indexes/t/videos"))
        .json(&json!({"sources": [fx::fixture_path()], "policy": "m0"}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()["job_id"]
        .as_str()
        .unwrap()
        .to_string();
    wait_job(&client, &base, &job).await;

    // JSON answer.
    let a: Value = client
        .post(format!("{base}/v1/indexes/t/ask"))
        .json(&json!({"question": "what?", "policy": "retrieval-only"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(a["answer"], "Answer.", "{a}");
    assert!(a["usage"]["cost_usd"].as_f64().unwrap() > 0.0);

    // SSE answer.
    let resp = client
        .post(format!("{base}/v1/indexes/t/ask"))
        .header("accept", "text/event-stream")
        .json(&json!({"question": "what?", "policy": "retrieval-only"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let text = resp.text().await.unwrap();
    assert!(text.contains("event: token"), "{text}");
    assert!(text.contains("event: done"), "{text}");
    assert!(text.contains("\"type\":\"done\""));

    // The cap (each ask costs 12 tokens at $1000/M = $0.012) trips on the third.
    let r = client
        .post(format!("{base}/v1/indexes/t/ask"))
        .json(&json!({"question": "again", "policy": "retrieval-only"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 429, "{}", r.text().await.unwrap());
}
