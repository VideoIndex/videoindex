//! Live C5: answers bounded by "now" (`AskRequest.until`), the per-ask
//! system addendum, module tools through the `Tool` registry, and media
//! located through `MediaInput`. Imitates `two_calls_in_one_turn` in
//! `tests/agent.rs`; that file is the evaluation branches' and stays as it
//! is.

#![allow(clippy::unwrap_used)]

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::StreamExt;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use vi_agent::tool_ext::{self, Extensions, Tool};
use vi_agent::tools::{self, ToolCall, ToolContext, ToolOutput};
use vi_agent::until;
use vi_agent::{Agent, AskEvent, AskRequest};
use vi_core::config::{Config, IndexPolicy, ProviderConfig, RoleBinding};
use vi_core::{EventBus, Result, VideoId};
use vi_index::{EmbeddedIndex, Storage};
use vi_media::Source;
use vi_pipeline::{JobOptions, Scheduler};
use vi_providers::{ProviderRegistry, ToolSpec};
use vi_testkit as fx;

/// The fixture indexed with its 24 five-second captions (`kw0` at 0 s to
/// `kw23` at 115 s) and two chapters cut at 60 s.
async fn build_index(dir: &std::path::Path) -> (Arc<EmbeddedIndex>, Arc<Config>) {
    let cache = dir.join("videos");
    let incoming = cache.join("incoming").join("PLtest");
    std::fs::create_dir_all(&incoming).unwrap();
    std::fs::copy(fx::fixture_path(), incoming.join("001-fixture.mp4")).unwrap();
    std::fs::write(
        incoming.join("001-fixture.info.json"),
        json!({"id":"fixture","title":"Synthetic workshop","webpage_url":"https://www.youtube.com/watch?v=fixture",
            "subtitles":{"en":[{"ext":"srt"}]},"automatic_captions":{},
            "chapters":[{"start_time":0.0,"end_time":60.0,"title":"First half"},{"start_time":60.0,"end_time":120.0,"title":"Second half"}]})
        .to_string(),
    )
    .unwrap();
    let mut srt = String::new();
    for i in 0..24 {
        let t0 = i * 5;
        srt.push_str(&format!(
            "{}\n00:0{}:{:02},000 --> 00:0{}:{:02},500\ncaption kw{i} about segment {}\n\n",
            i + 1,
            t0 / 60,
            t0 % 60,
            (t0 + 4) / 60,
            (t0 + 4) % 60,
            t0 / 10
        ));
    }
    std::fs::write(incoming.join("001-fixture.en.srt"), srt).unwrap();
    let idx = Arc::new(EmbeddedIndex::create(&dir.join("t.vidx")).unwrap());
    let mut c = Config::default();
    c.media.worker.path = Some(fx::worker_path());
    c.media.sample_max_dim = 320;
    c.media.cache_dir = cache;
    c.policy.insert(
        "t".into(),
        IndexPolicy {
            coarse: vec![
                "subtitle_import".into(),
                "sample".into(),
                "shot_boundary".into(),
                "phash".into(),
                "thumbnail".into(),
            ],
            fine: vec![],
            ..IndexPolicy::m0()
        },
    );
    let sched = Scheduler::new(idx.clone(), Arc::new(c.clone()), EventBus::default());
    let r = sched
        .run(
            Source::Path(incoming.join("001-fixture.mp4")),
            JobOptions {
                policy: Some("t".into()),
                ..JobOptions::default()
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(r.ok, "{r:?}");
    (idx, Arc::new(c))
}

/// The config with a fake OpenAI-compatible chat server bound to `agent_llm`.
fn with_fake_llm(config: &Config, addr: std::net::SocketAddr) -> Arc<Config> {
    let mut c = config.clone();
    c.providers.insert(
        "fake".into(),
        ProviderConfig {
            adapter: "openai_compat".into(),
            base_url: Some(format!("http://{addr}/v1")),
            model: Some("fake".into()),
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
    Arc::new(c)
}

async fn read_request(sock: &mut tokio::net::TcpStream) -> String {
    use tokio::io::AsyncReadExt;
    let mut buf = vec![0u8; 1 << 20];
    let mut n = 0;
    loop {
        let r = sock.read(&mut buf[n..]).await.unwrap_or(0);
        if r == 0 {
            break;
        }
        n += r;
        let head = String::from_utf8_lossy(&buf[..n]).to_string();
        if let Some(pos) = head.find("\r\n\r\n") {
            let len: usize = head
                .lines()
                .find_map(|l| l.strip_prefix("content-length: "))
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0);
            if n >= pos + 4 + len {
                break;
            }
        }
    }
    String::from_utf8_lossy(&buf[..n]).to_string()
}

/// A scripted chat server: `turns[i]` is the SSE chunk list for the i-th
/// request; every request body is recorded for the test to inspect.
async fn scripted_server(
    turns: Vec<Vec<String>>,
    bodies: Arc<Mutex<Vec<Value>>>,
) -> std::net::SocketAddr {
    use tokio::io::AsyncWriteExt;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let mut turn = 0;
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
            };
            let req = read_request(&mut sock).await;
            let body_json: Value = req
                .split("\r\n\r\n")
                .nth(1)
                .and_then(|b| serde_json::from_str(b).ok())
                .unwrap_or_default();
            bodies.lock().unwrap().push(body_json);
            let chunks = turns.get(turn).cloned().unwrap_or_else(|| {
                vec![r#"{"choices":[{"delta":{"content":"done"},"finish_reason":"stop"}]}"#.into()]
            });
            turn += 1;
            let mut body = String::new();
            for c in chunks {
                body.push_str(&format!("data: {c}\n\n"));
            }
            body.push_str("data: [DONE]\n\n");
            let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            let _ = sock.write_all(resp.as_bytes()).await;
            let _ = sock.shutdown().await;
        }
    });
    addr
}

fn tool_call_chunk(index: u32, id: &str, name: &str, args: &Value, last: bool) -> String {
    let finish = if last {
        r#","finish_reason":"tool_calls""#
    } else {
        ""
    };
    format!(
        r#"{{"choices":[{{"delta":{{"tool_calls":[{{"index":{index},"id":"{id}","function":{{"name":"{name}","arguments":{}}}}}]}}{finish}}}]}}"#,
        Value::String(args.to_string())
    )
}

fn text_chunk(text: &str) -> String {
    format!(
        r#"{{"choices":[{{"delta":{{"content":{}}},"finish_reason":"stop"}}]}}"#,
        Value::String(text.to_string())
    )
}

const USAGE: &str = r#"{"choices":[],"usage":{"prompt_tokens":100,"completion_tokens":20}}"#;

/// Text of a chat message whether it is a string or a list of parts.
fn message_text(m: &Value) -> String {
    match &m["content"] {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

/// The tool results of a request body by tool name, parsed as JSON.
fn tool_results(body: &Value) -> Vec<(String, Value)> {
    let calls: std::collections::BTreeMap<String, String> = body["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m["role"] == "assistant")
        .flat_map(|m| m["tool_calls"].as_array().cloned().unwrap_or_default())
        .filter_map(|c| {
            Some((
                c["id"].as_str()?.to_string(),
                c["function"]["name"].as_str()?.to_string(),
            ))
        })
        .collect();
    body["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m["role"] == "tool")
        .filter_map(|m| {
            let name = calls.get(m["tool_call_id"].as_str()?)?.clone();
            let v: Value = serde_json::from_str(&message_text(m)).ok()?;
            Some((name, v))
        })
        .collect()
}

/// `[HH:MM:SS]` prefixes of transcript lines, as seconds.
fn line_times(text: &str) -> Vec<f64> {
    text.lines()
        .filter_map(|l| {
            let inner = l.strip_prefix('[')?.split(']').next()?;
            let mut parts = inner.split(':').map(|p| p.parse::<f64>().ok());
            let h = parts.next()??;
            let m = parts.next()??;
            let s = parts.next()??;
            Some(h * 3600.0 + m * 60.0 + s)
        })
        .collect()
}

#[tokio::test]
async fn until_bounds_tool_reads_and_drops_late_citations() {
    let dir = tempfile::tempdir().unwrap();
    let (idx, config) = build_index(dir.path()).await;
    let video_id = idx.list_videos().await.unwrap()[0].id;
    let addendum = "This video is a live stream titled Synthetic workshop. The index is committed up to 00:01:00.";
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let addr = scripted_server(
        vec![
            vec![
                tool_call_chunk(0, "c1", "search", &json!({"query": "caption segment", "k": 20}), false),
                tool_call_chunk(1, "c2", "get_transcript", &json!({"t0": 0, "t1": 120}), false),
                tool_call_chunk(2, "c3", "timeline", &json!({"level": "chapter"}), true),
                USAGE.into(),
            ],
            vec![
                text_chunk(&format!(
                    "Kept [[cite:{video_id}:30-35]] cut [[cite:{video_id}:55-70]] dropped [[cite:{video_id}:70-80]]."
                )),
                USAGE.into(),
            ],
        ],
        bodies.clone(),
    )
    .await;
    let config = with_fake_llm(&config, addr);
    let providers = Arc::new(ProviderRegistry::new(
        config.clone(),
        CancellationToken::new(),
    ));
    let agent = Agent::new(idx.clone(), providers, config);
    let mut events = Vec::new();
    let mut stream = Box::pin(agent.ask(AskRequest {
        videos: vec![video_id],
        until: Some(60.0),
        system_addendum: Some(addendum.into()),
        ..AskRequest::new("what happened so far?")
    }));
    while let Some(ev) = stream.next().await {
        events.push(ev);
    }
    match events.last().unwrap() {
        AskEvent::Done { partial, usage, .. } => {
            assert!(!partial, "{events:?}");
            assert_eq!(usage.tool_calls, 3);
        }
        other => panic!("last event {other:?}"),
    }

    // Every tool read stopped below 60 s.
    let bodies = bodies.lock().unwrap().clone();
    assert_eq!(bodies.len(), 2, "two model turns");
    let results = tool_results(&bodies[1]);
    assert_eq!(results.len(), 3, "{results:#?}");
    for (name, v) in &results {
        assert!(v.get("error").is_none(), "{name}: {v}");
        match name.as_str() {
            "search" => {
                let hits = v["hits"].as_array().unwrap();
                assert!(!hits.is_empty(), "{v}");
                for h in hits {
                    assert!(h["t0"].as_f64().unwrap() < 60.0, "{h}");
                    assert!(h["t1"].as_f64().unwrap() <= 60.0, "{h}");
                    for e in h["evidence"].as_array().unwrap() {
                        assert!(e["t"].as_f64().unwrap() < 60.0, "{e}");
                    }
                }
            }
            "get_transcript" => {
                assert_eq!(v["t1"].as_f64().unwrap(), 60.0, "{v}");
                let text = v["text"].as_str().unwrap();
                let times = line_times(text);
                // Captions are stored as 15 s spans: four in [0, 60), the
                // last holding kw9 to kw11; kw12 starts at 60 s.
                assert_eq!(times.len(), 4, "{v}");
                assert!(times.iter().all(|t| *t < 60.0), "{times:?}");
                assert!(text.contains("kw11") && !text.contains("kw12"), "{text}");
            }
            "timeline" => {
                assert_eq!(v["duration_secs"].as_f64().unwrap(), 60.0, "{v}");
                let segs = v["segments"].as_array().unwrap();
                assert_eq!(segs.len(), 1, "{v}");
                assert_eq!(segs[0]["title"], "First half");
                assert!(segs[0]["t0"].as_f64().unwrap() < 60.0);
                assert!(segs[0]["t1"].as_f64().unwrap() <= 60.0);
            }
            other => panic!("unexpected tool {other}"),
        }
    }

    // Citations: kept below the bound, cut at it, dropped past it.
    let cites: Vec<(f64, f64)> = events
        .iter()
        .filter_map(|e| match e {
            AskEvent::Citation { t0, t1, .. } => Some((*t0, *t1)),
            _ => None,
        })
        .collect();
    assert_eq!(cites, vec![(30.0, 35.0), (55.0, 60.0)], "{events:?}");
    let text: String = events
        .iter()
        .filter_map(|e| match e {
            AskEvent::Token { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        text.contains("dropped") && !text.contains("cite:"),
        "{text}"
    );

    // The system prompt carries the bound and the addendum; the addendum's
    // hash is in its provenance row.
    let system = message_text(&bodies[0]["messages"][0]);
    assert_eq!(bodies[0]["messages"][0]["role"], "system");
    assert!(system.contains(addendum), "{system}");
    assert!(system.contains("Answer as of 00:01:00"), "{system}");
    let prov = idx
        .get_provenance(until::addendum_provenance_id(addendum))
        .await
        .unwrap()
        .expect("provenance row for the addendum");
    assert_eq!(prov.operator, "ask");
    assert_eq!(
        prov.prompt_hash.as_deref(),
        Some(until::addendum_hash(addendum).as_str())
    );
    assert_eq!(prov.params["kind"], "system_addendum");
    assert_eq!(prov.params["until"], 60.0);
}

/// Module state a live module hands to its tools.
#[derive(Debug, PartialEq)]
struct Head(f64);

/// A module tool: echoes its arguments and the head it finds in the
/// extensions.
struct EchoLive;

#[async_trait]
impl Tool for EchoLive {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "echo_live".into(),
            description: "Echo the arguments with the stream head.".into(),
            parameters: json!({"type":"object","properties":{"x":{"type":"integer"}}}),
        }
    }

    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<ToolOutput> {
        let head = ctx.extensions().get::<Head>().map(|h| h.0);
        let bound = ctx.bound().and_then(|b| b.until());
        Ok(ToolOutput {
            content:
                json!({"echo": args, "head": head, "until": bound, "videos": ctx.videos.len()})
                    .to_string(),
            summary: "echoed".into(),
            ..ToolOutput::default()
        })
    }
}

/// A process-wide tool, as a module server registers for MCP.
struct EchoGlobal;

#[async_trait]
impl Tool for EchoGlobal {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "echo_global".into(),
            description: "Process-wide echo.".into(),
            parameters: json!({"type":"object"}),
        }
    }

    async fn execute(&self, _ctx: &ToolContext, args: Value) -> Result<ToolOutput> {
        Ok(ToolOutput {
            content: json!({"global": args}).to_string(),
            summary: "global".into(),
            ..ToolOutput::default()
        })
    }
}

#[tokio::test]
async fn registered_tool_is_listed_dispatched_and_recorded_with_turn_and_ms() {
    let dir = tempfile::tempdir().unwrap();
    let (idx, config) = build_index(dir.path()).await;
    let video_id = idx.list_videos().await.unwrap()[0].id;
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let addr = scripted_server(
        vec![
            vec![
                tool_call_chunk(0, "c1", "echo_live", &json!({"x": 7}), true),
                USAGE.into(),
            ],
            vec![text_chunk("echoed back"), USAGE.into()],
        ],
        bodies.clone(),
    )
    .await;
    let config = with_fake_llm(&config, addr);
    let providers = Arc::new(ProviderRegistry::new(
        config.clone(),
        CancellationToken::new(),
    ));
    let agent = Agent::new(idx.clone(), providers.clone(), config.clone())
        .with_tools(vec![Arc::new(EchoLive) as Arc<dyn Tool>])
        .with_extensions(Extensions::new().with(Head(90.5)));
    let mut events = Vec::new();
    let mut stream = Box::pin(agent.ask(AskRequest {
        videos: vec![video_id],
        until: Some(84.0),
        ..AskRequest::new("echo something")
    }));
    while let Some(ev) = stream.next().await {
        events.push(ev);
    }
    // Announced and recorded like a built-in tool, with turn and time.
    let call = events
        .iter()
        .find(|e| matches!(e, AskEvent::ToolCall { tool, .. } if tool == "echo_live"));
    assert!(
        matches!(call, Some(AskEvent::ToolCall { turn: 1, .. })),
        "{events:?}"
    );
    let result = events
        .iter()
        .find_map(|e| match e {
            AskEvent::ToolResult {
                tool,
                summary,
                turn,
                ms,
            } if tool == "echo_live" => Some((summary.clone(), *turn, *ms)),
            _ => None,
        })
        .expect("tool result event");
    assert_eq!(result.0, "echoed");
    assert_eq!(result.1, 1);
    assert!(result.2 < 60_000, "wall time in milliseconds: {}", result.2);
    match events.last().unwrap() {
        AskEvent::Done { partial, usage, .. } => {
            assert!(!partial, "{events:?}");
            assert_eq!(usage.tool_calls, 1);
        }
        other => panic!("last event {other:?}"),
    }
    // The model saw the tool among the specs, after the built-in ones, and
    // its result carried the module state and the ask's bound.
    let bodies = bodies.lock().unwrap().clone();
    let names: Vec<&str> = bodies[0]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["function"]["name"].as_str())
        .collect();
    // The fake chat provider cannot see images, so `view` and `zoom` are
    // withheld as for any such model; the text tools are there.
    assert!(
        names.contains(&"search") && names.contains(&"get_transcript") && !names.contains(&"view"),
        "{names:?}"
    );
    assert_eq!(names.last(), Some(&"echo_live"), "{names:?}");
    let results = tool_results(&bodies[1]);
    assert_eq!(results.len(), 1, "{results:#?}");
    let (name, v) = &results[0];
    assert_eq!(name, "echo_live");
    assert_eq!(v["echo"]["x"], 7);
    assert_eq!(v["head"], 90.5);
    assert_eq!(v["until"], 84.0);
    assert_eq!(v["videos"], 1);

    // Process-wide registration is what the MCP `tools/list` (built from
    // `tools::specs`) sees outside any ask; it dispatches there too.
    assert!(!tools::specs(false).iter().any(|s| s.name == "echo_global"));
    tool_ext::register_global(Arc::new(EchoGlobal));
    let specs = tools::specs(true);
    assert_eq!(
        specs.last().map(|s| s.name.as_str()),
        Some("echo_global"),
        "appended after the built-ins"
    );
    assert!(specs.iter().any(|s| s.name == "describe"));
    let ctx = ToolContext {
        storage: idx.clone(),
        providers,
        config,
        videos: vec![video_id],
    };
    let out = tools::execute(
        &ctx,
        &ToolCall {
            id: "g".into(),
            name: "echo_global".into(),
            args: json!({"y": 1}),
            signature: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(out.summary, "global");
    let v: Value = serde_json::from_str(&out.content).unwrap();
    assert_eq!(v["global"]["y"], 1);
    assert!(ctx.extensions().is_empty(), "no ask, no extensions");
    // A built-in name is still the built-in tool.
    let out = tools::execute(
        &ctx,
        &ToolCall {
            id: "l".into(),
            name: "list_videos".into(),
            args: json!({}),
            signature: None,
        },
    )
    .await
    .unwrap();
    assert!(out.summary.ends_with("videos"), "{}", out.summary);
}

/// `view` over a `Segments` input renders the same grid as over the file
/// for the same window. The worker decodes only files today; the
/// segment-feed window decode is live C2 part 2 (session A), after which
/// `media_input::decode_path` hands the feed to `render_view` and this test
/// runs.
#[tokio::test]
#[ignore = "needs live C2 part 2"]
async fn view_over_segments_renders_the_same_grid_as_over_the_file() {
    use vi_agent::media_input::media_input;
    use vi_agent::{render_view, ViewRequest};
    use vi_media::MediaInput;
    let dir = tempfile::tempdir().unwrap();
    let (idx, config) = build_index(dir.path()).await;
    let file_video = idx.list_videos().await.unwrap()[0].clone();
    assert!(matches!(
        media_input(&file_video),
        Some(MediaInput::File { .. })
    ));
    let mut live_video = file_video.clone();
    live_video.id = VideoId::new();
    live_video.content_hash = "live-copy".into();
    live_video.probe = json!({"live": {"source": "fixture", "store": fx::fixture_segments_dir()}});
    idx.put_video(&live_video).await.unwrap();
    assert!(matches!(
        media_input(&live_video),
        Some(MediaInput::Segments(_))
    ));
    let req = ViewRequest {
        t0: 40.0,
        t1: 46.0,
        fps: 1.0,
        ..ViewRequest::default()
    };
    let over_file = render_view(&config.media.worker, &file_video, req.clone())
        .await
        .unwrap();
    let over_feed = render_view(&config.media.worker, &live_video, req)
        .await
        .unwrap();
    assert_eq!(over_feed.timestamps.len(), over_file.timestamps.len());
    for (a, b) in over_feed.timestamps.iter().zip(&over_file.timestamps) {
        assert!((a - b).abs() < 1.0 / 25.0, "{a} vs {b}");
    }
    assert_eq!(over_feed.distinct, over_file.distinct);
    assert_eq!(
        (over_feed.width, over_feed.height),
        (over_file.width, over_file.height)
    );
}
