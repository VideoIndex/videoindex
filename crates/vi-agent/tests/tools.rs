//! The library-wide tools (`find_mentions`, `count_mentions`,
//! `library_stats`), `search`'s per-video cap and the multi-window text
//! tools over a hand-built index.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use chrono::Utc;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use vi_agent::tools::{self, ToolCall, ToolContext};
use vi_core::config::Config;
use vi_core::model::*;
use vi_core::*;
use vi_index::{EmbeddedIndex, Storage};
use vi_providers::ProviderRegistry;

async fn seed(idx: &EmbeddedIndex) -> (VideoId, VideoId) {
    let prov = Provenance::local("asr", 1, json!({}));
    idx.put_provenance(&prov).await.unwrap();
    let mk_video = |title: &str, channel: &str, secs: i64| Video {
        id: VideoId::new(),
        source_uri: format!("https://example/{title}"),
        content_hash: title.to_string(),
        title: Some(title.into()),
        description: None,
        channel: Some(channel.into()),
        published_at: None,
        duration: Timestamp::from_secs(secs),
        start_wallclock: None,
        probe: json!({}),
        index_state: IndexState::Coarse,
        created_at: Utc::now(),
    };
    let a = mk_video("Talk A", "MOOC", 3600);
    let b = mk_video("Talk B", "Workshop", 1800);
    let c = mk_video("Talk C", "MOOC", 600);
    for v in [&a, &b, &c] {
        idx.put_video(v).await.unwrap();
    }
    let mk_track = |video_id: VideoId| Track {
        id: TrackId::new(),
        video_id,
        kind: TrackKind::Subtitle,
        stream_index: 1000,
        codec: "srt".into(),
        timebase_num: 1,
        timebase_den: 1000,
        width: None,
        height: None,
        fps: None,
        sample_rate: None,
        channels: None,
        language: Some("en".into()),
    };
    let ta = mk_track(a.id);
    let tb = mk_track(b.id);
    idx.put_tracks(&[ta.clone(), tb.clone()]).await.unwrap();
    let span = |track: TrackId, t0: i64, text: &str| {
        Span::Transcript(TranscriptSpan {
            id: SpanId::new(),
            track_id: track,
            t0: Timestamp::from_secs(t0),
            t1: Timestamp::from_secs(t0 + 10),
            text: text.into(),
            speaker: None,
            language: None,
            confidence: None,
            words: None,
            provenance_id: prov.id,
        })
    };
    // Talk A mentions agents in five places (three chapters' worth of time);
    // Talk B once; Talk C never.
    idx.put_spans(&[
        span(ta.id, 10, "agents are the theme of this talk"),
        span(ta.id, 700, "our agents call tools in a loop"),
        span(ta.id, 1400, "evaluation of agents is hard"),
        span(ta.id, 2100, "agents again, and Anthropic's model"),
        span(ta.id, 2800, "closing thoughts on agents"),
        span(tb.id, 30, "one aside about agents and RAG"),
        span(tb.id, 900, "retrieval augmented generation, RAG for short"),
    ])
    .await
    .unwrap();
    (a.id, b.id)
}

fn ctx(idx: Arc<EmbeddedIndex>) -> ToolContext {
    let config = Arc::new(Config::default());
    ToolContext {
        storage: idx,
        providers: Arc::new(ProviderRegistry::new(
            config.clone(),
            CancellationToken::new(),
        )),
        config,
        videos: Vec::new(),
    }
}

async fn call(ctx: &ToolContext, name: &str, args: Value) -> (Value, String) {
    let out = tools::execute(
        ctx,
        &ToolCall {
            id: "t".into(),
            name: name.into(),
            args,
            signature: None,
        },
    )
    .await
    .unwrap();
    (serde_json::from_str(&out.content).unwrap(), out.summary)
}

#[tokio::test]
async fn find_mentions_lists_every_video_with_a_hit() {
    let dir = tempfile::tempdir().unwrap();
    let idx = Arc::new(EmbeddedIndex::create(&dir.path().join("t.vidx")).unwrap());
    let (a, b) = seed(&idx).await;
    let ctx = ctx(idx);

    let (v, summary) = call(
        &ctx,
        "find_mentions",
        json!({"terms": ["agents", "anthropic"], "per_video": 2}),
    )
    .await;
    assert_eq!(v["videos_searched"], 3, "{v}");
    assert_eq!(v["videos_with_hits"], 2);
    assert_eq!(v["total_hits"], 7);
    let rows = v["videos"].as_array().unwrap();
    assert_eq!(rows[0]["video_id"], a.to_string());
    assert_eq!(rows[0]["counts"]["transcript"], 6);
    assert_eq!(rows[0]["first"].as_array().unwrap().len(), 2);
    assert_eq!(rows[0]["first"][0]["t0"], 10.0);
    assert!(rows[0]["first"][0]["text"]
        .as_str()
        .unwrap()
        .contains("[agents]"));
    assert_eq!(rows[1]["video_id"], b.to_string());
    assert_eq!(rows[1]["total"], 1);
    assert!(
        summary.starts_with("2 of 3 videos mention agents / anthropic"),
        "{summary}"
    );

    // Scoped to one video, counts only.
    let (v, _) = call(
        &ctx,
        "find_mentions",
        json!({"terms": ["agents"], "video_ids": [b.to_string()], "per_video": 0}),
    )
    .await;
    assert_eq!(v["videos_searched"], 1);
    assert_eq!(v["videos_with_hits"], 1);
    assert!(v["videos"][0]["first"].as_array().unwrap().is_empty());

    // Bad arguments come back as content.
    let (v, summary) = call(&ctx, "find_mentions", json!({})).await;
    assert!(v["error"].as_str().unwrap().contains("terms"));
    assert!(summary.starts_with("error"));
}

#[tokio::test]
async fn count_mentions_ranks_terms_and_groups() {
    let dir = tempfile::tempdir().unwrap();
    let idx = Arc::new(EmbeddedIndex::create(&dir.path().join("t.vidx")).unwrap());
    let (a, _b) = seed(&idx).await;
    let ctx = ctx(idx);

    let (v, _) = call(
        &ctx,
        "count_mentions",
        json!({"terms": ["agents", "RAG", "MCP"], "group_by": "channel"}),
    )
    .await;
    let terms = v["terms"].as_array().unwrap();
    assert_eq!(terms[0]["term"], "agents");
    assert_eq!(terms[0]["total"], 6);
    assert_eq!(terms[0]["videos"], 2);
    assert_eq!(terms[1]["term"], "RAG");
    assert_eq!(terms[1]["total"], 2);
    assert_eq!(terms[1]["videos"], 1);
    assert_eq!(terms[2]["total"], 0);
    assert_eq!(terms[2]["videos"], 0);
    let chans = v["by_channel"].as_array().unwrap();
    // Two channels; MOOC has two videos but only Talk A has hits.
    assert_eq!(chans.len(), 2, "{v}");
    let mooc = chans.iter().find(|c| c["channel"] == "MOOC").unwrap();
    assert_eq!(mooc["videos"], 2);
    assert_eq!(mooc["videos_with_hits"], 1);
    assert_eq!(mooc["by_term"]["agents"], 5);

    let (v, _) = call(
        &ctx,
        "count_mentions",
        json!({"terms": ["agents"], "group_by": "video"}),
    )
    .await;
    let rows = v["by_video"].as_array().unwrap();
    assert_eq!(rows[0]["video_id"], a.to_string());
    assert_eq!(rows[0]["by_term"]["agents"], 5);
    assert!(v.get("by_channel").is_none());

    let (v, _) = call(
        &ctx,
        "count_mentions",
        json!({"terms": ["agents"], "group_by": "speaker"}),
    )
    .await;
    assert!(v["error"].as_str().unwrap().contains("group_by"));
}

#[tokio::test]
async fn library_stats_sums_durations_by_channel() {
    let dir = tempfile::tempdir().unwrap();
    let idx = Arc::new(EmbeddedIndex::create(&dir.path().join("t.vidx")).unwrap());
    seed(&idx).await;
    let ctx = ctx(idx);
    let (v, summary) = call(&ctx, "library_stats", json!({})).await;
    assert_eq!(v["videos"], 3);
    assert_eq!(v["total_duration_secs"], 6000.0);
    let chans = v["channels"].as_array().unwrap();
    assert_eq!(chans[0]["channel"], "MOOC");
    assert_eq!(chans[0]["videos"], 2);
    assert_eq!(chans[0]["duration_secs"], 4200.0);
    assert_eq!(chans[1]["channel"], "Workshop");
    assert_eq!(v["video_list"].as_array().unwrap().len(), 3);
    assert!(summary.contains("3 videos"), "{summary}");
}

#[tokio::test]
async fn search_spreads_hits_across_videos_unless_scoped() {
    let dir = tempfile::tempdir().unwrap();
    let idx = Arc::new(EmbeddedIndex::create(&dir.path().join("t.vidx")).unwrap());
    let (a, b) = seed(&idx).await;
    let ctx = ctx(idx);
    // Talk A alone has five matching windows; the default cap of three per
    // video leaves room for Talk B in a k=8 list.
    let (v, _) = call(&ctx, "search", json!({"query": "agents", "k": 8})).await;
    let hits = v["hits"].as_array().unwrap();
    let from_a = hits
        .iter()
        .filter(|h| h["video_id"] == a.to_string())
        .count();
    let from_b = hits
        .iter()
        .filter(|h| h["video_id"] == b.to_string())
        .count();
    assert_eq!(from_a, 3, "{v}");
    assert_eq!(from_b, 1);

    let (v, _) = call(
        &ctx,
        "search",
        json!({"query": "agents", "k": 8, "per_video_k": 1}),
    )
    .await;
    assert_eq!(v["hits"].as_array().unwrap().len(), 2);

    // A single-video search is not capped.
    let (v, _) = call(
        &ctx,
        "search",
        json!({"query": "agents", "k": 8, "video_id": a.to_string()}),
    )
    .await;
    assert_eq!(v["hits"].as_array().unwrap().len(), 5, "{v}");
}

#[tokio::test]
async fn get_transcript_reads_several_windows_in_one_call() {
    let dir = tempfile::tempdir().unwrap();
    let idx = Arc::new(EmbeddedIndex::create(&dir.path().join("t.vidx")).unwrap());
    let (a, _b) = seed(&idx).await;
    let ctx = ctx(idx);
    // Three windows given out of order; the last two overlap and merge.
    let (v, summary) = call(
        &ctx,
        "get_transcript",
        json!({"video_id": a.to_string(), "windows": [
            {"t0": 1390, "t1": 1420}, {"t0": 0, "t1": 30}, {"t0": 690, "t1": 720}, {"t0": 710, "t1": 800}
        ]}),
    )
    .await;
    assert_eq!(v["video_id"], a.to_string(), "{v}");
    assert_eq!(v["kind"], "transcript");
    assert!(
        v.get("t0").is_none(),
        "multi-window output has no top-level range: {v}"
    );
    let wins = v["windows"].as_array().unwrap();
    assert_eq!(wins.len(), 3, "{v}");
    assert_eq!(wins[0]["t0"], 0.0);
    assert_eq!(wins[1]["t0"], 690.0);
    assert_eq!(wins[1]["t1"], 800.0);
    assert_eq!(wins[2]["t0"], 1390.0);
    for w in wins {
        assert_eq!(w["count"], 1, "{w}");
    }
    assert!(wins[0]["text"]
        .as_str()
        .unwrap()
        .contains("theme of this talk"));
    assert!(wins[1]["text"].as_str().unwrap().contains("call tools"));
    assert!(wins[2]["text"].as_str().unwrap().contains("evaluation"));
    assert!(
        summary.starts_with("3 windows, 3 transcript lines"),
        "{summary}"
    );

    // The single form keeps its flat shape.
    let (v, summary) = call(
        &ctx,
        "get_transcript",
        json!({"video_id": a.to_string(), "t0": 0, "t1": 30}),
    )
    .await;
    assert_eq!(v["t0"], 0.0);
    assert_eq!(v["count"], 1);
    assert!(v.get("windows").is_none(), "{v}");
    assert!(summary.starts_with("1 transcript lines"), "{summary}");

    // Limits and bad windows come back as content.
    let many: Vec<Value> = (0..4)
        .map(|i| json!({"t0": i * 100, "t1": i * 100 + 10}))
        .collect();
    let (v, _) = call(
        &ctx,
        "get_ocr",
        json!({"video_id": a.to_string(), "windows": many}),
    )
    .await;
    assert!(
        v["error"].as_str().unwrap().contains("at most 3 windows"),
        "{v}"
    );
    let (v, _) = call(
        &ctx,
        "get_ocr",
        json!({"video_id": a.to_string(), "windows": [{"t0": 50, "t1": 40}]}),
    )
    .await;
    assert!(v["error"].as_str().unwrap().contains("windows[0]"), "{v}");
    let (v, _) = call(
        &ctx,
        "get_ocr",
        json!({"video_id": a.to_string(), "windows": []}),
    )
    .await;
    assert!(v["error"].as_str().unwrap().contains("at least one"), "{v}");
}
