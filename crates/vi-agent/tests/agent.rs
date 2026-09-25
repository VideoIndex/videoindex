//! The loop against a scripted LLM over an index built from the fixture
//! with sidecar subtitles: one search, then an answer with a citation; two
//! calls in one turn run together and land in order; the call budget is a
//! hard stop whose last turn names the count.

#![allow(clippy::unwrap_used)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures::StreamExt;
use tokio_util::sync::CancellationToken;
use vi_agent::{Agent, AskBudget, AskEvent, AskRequest, RetrievalOnlyPolicy};
use vi_core::config::{Config, IndexPolicy, ProviderConfig, RoleBinding};
use vi_core::EventBus;
use vi_index::EmbeddedIndex;
use vi_media::Source;
use vi_pipeline::{JobOptions, Scheduler};
use vi_providers::ProviderRegistry;
use vi_testkit as fx;

async fn build_index(dir: &std::path::Path) -> (Arc<EmbeddedIndex>, Arc<Config>) {
    let cache = dir.join("videos");
    let incoming = cache.join("incoming").join("PLtest");
    std::fs::create_dir_all(&incoming).unwrap();
    std::fs::copy(fx::fixture_path(), incoming.join("001-fixture.mp4")).unwrap();
    std::fs::write(
        incoming.join("001-fixture.info.json"),
        serde_json::json!({"id":"fixture","title":"Synthetic workshop","webpage_url":"https://www.youtube.com/watch?v=fixture",
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

#[tokio::test]
async fn loop_searches_then_answers_with_a_typed_citation() {
    let dir = tempfile::tempdir().unwrap();
    let (idx, config) = build_index(dir.path()).await;
    let video_id = {
        use vi_index::Storage;
        idx.list_videos().await.unwrap()[0].id
    };
    // The agent talks to a fake OpenAI-compatible server that scripts two
    // turns: a `search` call, then an answer citing the first hit.
    let addr = fake_openai_server().await;
    let mut c = (*config).clone();
    c.providers.insert(
        "fake".into(),
        ProviderConfig {
            adapter: "openai_compat".into(),
            base_url: Some(format!("http://{addr}/v1")),
            model: Some("fake-vl".into()),
            pricing: Some(vi_core::config::Pricing {
                input_per_mtok: 1.0,
                output_per_mtok: 2.0,
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
    let config = Arc::new(c);
    let providers = Arc::new(ProviderRegistry::new(
        config.clone(),
        CancellationToken::new(),
    ));
    let agent = Agent::new(idx.clone(), providers, config);
    let mut events = Vec::new();
    let mut stream = Box::pin(agent.ask(AskRequest::new("where is segment seven?")));
    while let Some(ev) = stream.next().await {
        events.push(ev);
    }
    let tool_calls: Vec<&AskEvent> = events
        .iter()
        .filter(|e| matches!(e, AskEvent::ToolCall { .. }))
        .collect();
    assert_eq!(tool_calls.len(), 1, "{events:?}");
    assert!(
        matches!(tool_calls[0], AskEvent::ToolCall { tool, turn, .. } if tool == "search" && *turn == 1)
    );
    assert!(events.iter().any(|e| matches!(e, AskEvent::ToolResult { tool, summary, turn, .. } if tool == "search" && summary.contains("hits") && *turn == 1)));
    let text: String = events
        .iter()
        .filter_map(|e| match e {
            AskEvent::Token { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(text.contains("Segment seven is at 35 s"), "{text}");
    assert!(!text.contains("[[cite"), "markers must not leak: {text}");
    let cites: Vec<&AskEvent> = events
        .iter()
        .filter(|e| matches!(e, AskEvent::Citation { .. }))
        .collect();
    assert_eq!(cites.len(), 1, "{events:?}");
    if let AskEvent::Citation {
        video_id: v,
        t0,
        kind,
        ..
    } = cites[0]
    {
        assert_eq!(*v, video_id);
        assert!(*t0 >= 30.0 && *t0 <= 40.0, "cited {t0}");
        assert_eq!(kind, "transcript");
    }
    match events.last().unwrap() {
        AskEvent::Done { partial, usage, .. } => {
            assert!(!partial);
            assert_eq!(usage.tool_calls, 1);
            assert_eq!(usage.provider_calls, 2);
            assert_eq!(usage.tokens_in, 200);
            assert!(usage.cost_usd > 0.0);
        }
        other => panic!("last event {other:?}"),
    }
}

/// A fake OpenAI chat server: turn 1 calls `search`, turn 2 answers with a
/// citation of the first hit in the tool result.
async fn fake_openai_server() -> std::net::SocketAddr {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let mut turn = 0;
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
            };
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
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            let body_json: serde_json::Value = req
                .split("\r\n\r\n")
                .nth(1)
                .and_then(|b| serde_json::from_str(b).ok())
                .unwrap_or_default();
            let chunks: Vec<String> = if turn == 0 {
                vec![
                    r#"{"choices":[{"delta":{"content":"Let me look. "}}]}"#.into(),
                    r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"search","arguments":"{\"query\":\"kw7\",\"k\":3}"}}]},"finish_reason":"tool_calls"}]}"#.into(),
                    r#"{"choices":[],"usage":{"prompt_tokens":100,"completion_tokens":20}}"#.into(),
                ]
            } else {
                // Cite the first hit from the tool message.
                let mut cite = String::new();
                for m in body_json["messages"].as_array().into_iter().flatten() {
                    if m["role"] == "tool" {
                        let v: serde_json::Value =
                            serde_json::from_str(m["content"].as_str().unwrap_or("{}"))
                                .unwrap_or_default();
                        let h = &v["hits"][0];
                        cite = format!(
                            "[[cite:{}:{}-{}]]",
                            h["video_id"].as_str().unwrap_or(""),
                            h["t0"],
                            h["t1"]
                        );
                    }
                }
                let (a, b) = cite.split_at(cite.len() / 2);
                vec![
                    r#"{"choices":[{"delta":{"content":"Segment seven is at 35 s "}}]}"#.into(),
                    format!(
                        r#"{{"choices":[{{"delta":{{"content":{}}}}}]}}"#,
                        serde_json::Value::String(a.to_string())
                    ),
                    format!(
                        r#"{{"choices":[{{"delta":{{"content":{}}},"finish_reason":"stop"}}]}}"#,
                        serde_json::Value::String(format!("{b}."))
                    ),
                    r#"{"choices":[],"usage":{"prompt_tokens":100,"completion_tokens":30}}"#.into(),
                ]
            };
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

#[tokio::test]
async fn retrieval_only_policy_searches_once_then_answers() {
    let dir = tempfile::tempdir().unwrap();
    let (idx, config) = build_index(dir.path()).await;
    let video_id = {
        use vi_index::Storage;
        idx.list_videos().await.unwrap()[0].id
    };
    // A server whose every turn is a plain answer (no tool calls).
    let addr = plain_answer_server().await;
    let mut c = (*config).clone();
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
    let config = Arc::new(c);
    let providers = Arc::new(ProviderRegistry::new(
        config.clone(),
        CancellationToken::new(),
    ));
    let agent =
        Agent::new(idx, providers, config).with_policy(Arc::new(RetrievalOnlyPolicy { k: 5 }));
    let out = agent
        .ask_collect(AskRequest {
            videos: vec![video_id],
            ..AskRequest::new("kw3")
        })
        .await;
    assert_eq!(out.tool_calls.len(), 1);
    assert_eq!(out.tool_calls[0].0, "search");
    assert_eq!(out.text, "Answer.");
    assert!(!out.partial);
    assert_eq!(out.usage.provider_calls, 1);
}

async fn plain_answer_server() -> std::net::SocketAddr {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
            };
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
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            // The policy's search result arrives as a user message; tools stay
            // defined (the history holds a call) but the model must answer.
            assert!(req.contains("Result of search"), "{req}");
            assert!(req.contains("\"tool_choice\":\"none\""), "{req}");
            assert!(req.contains("no further calls are available"), "{req}");
            let body = "data: {\"choices\":[{\"delta\":{\"content\":\"Answer.\"},\"finish_reason\":\"stop\"}]}\n\ndata: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2}}\n\ndata: [DONE]\n\n";
            let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            let _ = sock.write_all(resp.as_bytes()).await;
            let _ = sock.shutdown().await;
        }
    });
    addr
}
/// Every turn, whatever it asks, gets a tool call back. The flag records
/// whether some request carried the exhausted-turn nudge with the count.
async fn always_tool_call_server() -> (std::net::SocketAddr, Arc<AtomicBool>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let saw_nudge = Arc::new(AtomicBool::new(false));
    let flag = saw_nudge.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
            };
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
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            if req.contains("You have used 1 of 1 tool calls; no further calls are available")
                && req.contains("If the question is multiple choice")
            {
                flag.store(true, Ordering::Relaxed);
            }
            let body = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"function\":{\"name\":\"search\",\"arguments\":\"{\\\"query\\\":\\\"kw7\\\",\\\"k\\\":3}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\ndata: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":9}}\n\ndata: [DONE]\n\n";
            let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            let _ = sock.write_all(resp.as_bytes()).await;
            let _ = sock.shutdown().await;
        }
    });
    (addr, saw_nudge)
}

/// A server that ignores `tool_choice: none` and keeps calling tools: the
/// tool-call budget must still be a hard stop and the loop must end.
#[tokio::test]
async fn tool_calls_after_the_budget_are_not_executed_and_the_loop_ends() {
    let dir = tempfile::tempdir().unwrap();
    let (idx, config) = build_index(dir.path()).await;
    let (addr, saw_nudge) = always_tool_call_server().await;
    let mut c = (*config).clone();
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
    let config = Arc::new(c);
    let providers = Arc::new(ProviderRegistry::new(
        config.clone(),
        CancellationToken::new(),
    ));
    let agent = Agent::new(idx, providers, config);
    let out = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        agent.ask_collect(AskRequest {
            budget: AskBudget {
                max_tool_calls: 1,
                ..AskBudget::default()
            },
            ..AskRequest::new("kw3")
        }),
    )
    .await
    .expect("the loop must terminate");
    assert_eq!(out.usage.tool_calls, 1, "{out:?}");
    assert!(out.partial);
    assert!(out.usage.provider_calls <= 3, "{out:?}");
    assert!(
        saw_nudge.load(Ordering::Relaxed),
        "the exhausted-turn nudge must name the calls used"
    );
}

/// One HTTP request body from a scripted fake server's socket.
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

/// Turn 1 asks for `search` and `get_transcript` in one message; turn 2
/// answers "ordered" when the tool results came back in call order (c1 then
/// c2), else "unordered".
async fn two_calls_server() -> std::net::SocketAddr {
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
            let body_json: serde_json::Value = req
                .split("\r\n\r\n")
                .nth(1)
                .and_then(|b| serde_json::from_str(b).ok())
                .unwrap_or_default();
            let chunks: Vec<String> = if turn == 0 {
                vec![
                    r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"search","arguments":"{\"query\":\"kw7\",\"k\":3}"}}]}}]}"#.into(),
                    r#"{"choices":[{"delta":{"tool_calls":[{"index":1,"id":"c2","function":{"name":"get_transcript","arguments":"{\"t0\":30,\"t1\":45}"}}]},"finish_reason":"tool_calls"}]}"#.into(),
                    r#"{"choices":[],"usage":{"prompt_tokens":100,"completion_tokens":20}}"#.into(),
                ]
            } else {
                let ids: Vec<String> = body_json["messages"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|m| m["role"] == "tool")
                    .map(|m| m["tool_call_id"].as_str().unwrap_or("").to_string())
                    .collect();
                let word = if ids == ["c1", "c2"] {
                    "ordered"
                } else {
                    "unordered"
                };
                vec![
                    format!(
                        r#"{{"choices":[{{"delta":{{"content":"{word} {}"}},"finish_reason":"stop"}}]}}"#,
                        ids.join(",")
                    ),
                    r#"{"choices":[],"usage":{"prompt_tokens":100,"completion_tokens":5}}"#.into(),
                ]
            };
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

#[tokio::test]
async fn two_calls_in_one_turn_run_together_and_land_in_call_order() {
    let dir = tempfile::tempdir().unwrap();
    let (idx, config) = build_index(dir.path()).await;
    let video_id = {
        use vi_index::Storage;
        idx.list_videos().await.unwrap()[0].id
    };
    let addr = two_calls_server().await;
    let mut c = (*config).clone();
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
    let config = Arc::new(c);
    let providers = Arc::new(ProviderRegistry::new(
        config.clone(),
        CancellationToken::new(),
    ));
    let agent = Agent::new(idx, providers, config);
    let mut events = Vec::new();
    let mut stream = Box::pin(agent.ask(AskRequest {
        videos: vec![video_id],
        ..AskRequest::new("what is at 35 s?")
    }));
    while let Some(ev) = stream.next().await {
        events.push(ev);
    }
    // Both calls are announced (same turn) before either result; both
    // results arrive in call order, before the answer.
    let order: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            AskEvent::ToolCall { tool, turn, .. } => Some(format!("call:{tool}@{turn}")),
            AskEvent::ToolResult { tool, turn, .. } => Some(format!("result:{tool}@{turn}")),
            AskEvent::Token { .. } => Some("token".into()),
            _ => None,
        })
        .collect();
    let first_token = order.iter().position(|s| s == "token").unwrap();
    assert_eq!(
        &order[..first_token],
        &[
            "call:search@1",
            "call:get_transcript@1",
            "result:search@1",
            "result:get_transcript@1"
        ],
        "{order:?}"
    );
    assert!(events.iter().any(|e| matches!(e, AskEvent::ToolResult { tool, summary, .. } if tool == "get_transcript" && summary.contains("transcript lines"))), "{events:?}");
    let text: String = events
        .iter()
        .filter_map(|e| match e {
            AskEvent::Token { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(text.starts_with("ordered"), "{text}");
    match events.last().unwrap() {
        AskEvent::Done { partial, usage, .. } => {
            assert!(!partial);
            assert_eq!(usage.tool_calls, 2);
            assert_eq!(usage.provider_calls, 2);
        }
        other => panic!("last event {other:?}"),
    }
}

/// `view` with two windows is one tool call that returns two grids, in
/// window order, with the window list in the text result.
#[tokio::test]
async fn view_with_two_windows_returns_two_grids_in_order() {
    use vi_agent::tools::{self, ToolCall, ToolContext};
    let dir = tempfile::tempdir().unwrap();
    let (idx, config) = build_index(dir.path()).await;
    let video_id = {
        use vi_index::Storage;
        idx.list_videos().await.unwrap()[0].id
    };
    let providers = Arc::new(ProviderRegistry::new(
        config.clone(),
        CancellationToken::new(),
    ));
    let ctx = ToolContext {
        storage: idx,
        providers,
        config,
        videos: vec![video_id],
    };
    let out = tools::execute(
        &ctx,
        &ToolCall {
            id: "v".into(),
            name: "view".into(),
            args: serde_json::json!({"windows": [{"t0": 40, "t1": 46}, {"t0": 5, "t1": 11}], "fps": 1}),
            signature: None,
        },
    )
    .await
    .unwrap();
    let v: serde_json::Value = serde_json::from_str(&out.content).unwrap();
    assert!(v.get("error").is_none(), "{v}");
    assert_eq!(out.images.len(), 2, "{}", out.summary);
    let wins = v["windows"].as_array().unwrap();
    assert_eq!(wins.len(), 2);
    assert_eq!(wins[0]["t0"], 5.0, "{v}");
    assert_eq!(wins[1]["t0"], 40.0);
    assert!(wins[0]["frames"].as_u64().unwrap() >= 1);
    assert!(
        wins[0]["transcript"].as_str().unwrap().contains("kw1"),
        "{v}"
    );
    assert!(v["note"].as_str().unwrap().starts_with("2 frame grids"));
    assert!(out.summary.starts_with("2 windows,"), "{}", out.summary);

    // Four windows is too many for one view.
    let out = tools::execute(
        &ctx,
        &ToolCall {
            id: "v".into(),
            name: "view".into(),
            args: serde_json::json!({"windows": [{"t0": 0, "t1": 5}, {"t0": 10, "t1": 15}, {"t0": 20, "t1": 25}, {"t0": 30, "t1": 35}]}),
            signature: None,
        },
    )
    .await
    .unwrap();
    assert!(out.summary.contains("at most 3 windows"), "{}", out.summary);
    assert!(out.images.is_empty());
}

/// Two calls in one message with one call left: the first runs, the second
/// is answered with an error payload and never executed, and the budget
/// count stays exact.
#[tokio::test]
async fn calls_past_the_budget_in_one_turn_are_not_run() {
    let dir = tempfile::tempdir().unwrap();
    let (idx, config) = build_index(dir.path()).await;
    let video_id = {
        use vi_index::Storage;
        idx.list_videos().await.unwrap()[0].id
    };
    let addr = two_calls_server().await;
    let mut c = (*config).clone();
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
    let config = Arc::new(c);
    let providers = Arc::new(ProviderRegistry::new(
        config.clone(),
        CancellationToken::new(),
    ));
    let agent = Agent::new(idx, providers, config);
    let mut events = Vec::new();
    let mut stream = Box::pin(agent.ask(AskRequest {
        videos: vec![video_id],
        budget: AskBudget {
            max_tool_calls: 1,
            ..AskBudget::default()
        },
        ..AskRequest::new("what is at 35 s?")
    }));
    while let Some(ev) = stream.next().await {
        events.push(ev);
    }
    let called: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            AskEvent::ToolCall { tool, .. } => Some(tool.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(called, ["search"], "{events:?}");
    // The model still saw a result for both call ids, in order.
    let text: String = events
        .iter()
        .filter_map(|e| match e {
            AskEvent::Token { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(text.starts_with("ordered c1,c2"), "{text}");
    match events.last().unwrap() {
        AskEvent::Done { usage, .. } => assert_eq!(usage.tool_calls, 1),
        other => panic!("last event {other:?}"),
    }
}
