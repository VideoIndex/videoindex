//! `vi search <index-dir> "<query>" [--k N] [--video ID] [--kind K]`

use std::path::PathBuf;

use anyhow::{Context, Result};
use vi_core::{Config, VideoId};
use vi_index::{EmbeddedIndex, Kind};
use vi_query::{search, SearchRequest};

use crate::output::Output;

/// Arguments.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Index directory.
    pub index_dir: PathBuf,
    /// Query text. Words are ANDed; the last word matches as a prefix.
    pub query: String,
    /// Number of results.
    #[arg(long, short, default_value_t = 10)]
    pub k: usize,
    /// Restrict to a video id (repeatable).
    #[arg(long = "video")]
    pub videos: Vec<VideoId>,
    /// Restrict to evidence kinds: transcript, ocr, description, frame (repeatable).
    #[arg(long = "kind", value_parser = parse_kind)]
    pub kinds: Vec<Kind>,
    /// BM25 only: skip the text-vector and image-vector lists.
    #[arg(long)]
    pub text_only: bool,
    /// At most this many hits per video (spreads results across a library).
    #[arg(long)]
    pub per_video_k: Option<usize>,
}

fn parse_kind(s: &str) -> std::result::Result<Kind, String> {
    match s {
        "transcript" => Ok(Kind::Transcript),
        "ocr" => Ok(Kind::Ocr),
        "description" => Ok(Kind::Description),
        "frame" => Ok(Kind::Frame),
        other => Err(format!(
            "unknown kind '{other}'; expected transcript, ocr, description, or frame"
        )),
    }
}

pub async fn run(args: Args, config: &Config, out: &Output) -> Result<()> {
    let idx = EmbeddedIndex::open(&args.index_dir)
        .with_context(|| format!("opening index at {}", args.index_dir.display()))?;
    let providers = vi_providers::ProviderRegistry::new(
        std::sync::Arc::new(config.clone()),
        tokio_util::sync::CancellationToken::new(),
    );
    vi_perceive::OnnxLocal::register(&providers);
    let req = SearchRequest {
        query: args.query.clone(),
        videos: args.videos.clone(),
        kinds: args.kinds.clone(),
        k: args.k,
        text_only: args.text_only,
        per_video_k: args.per_video_k,
    };
    let started = std::time::Instant::now();
    let resp = search(&idx, Some(&providers), &req).await?;
    let elapsed_ms = started.elapsed().as_millis();
    out.emit(&resp, || {
        if resp.hits.is_empty() {
            return format!("no results for \"{}\"", args.query);
        }
        let mut s = String::new();
        for (i, h) in resp.hits.iter().enumerate() {
            s.push_str(&format!(
                "{:>2}. [{} - {}]  {}  score {:.3}\n    {}{}\n",
                i + 1,
                h.t0,
                h.t1,
                h.title.as_deref().unwrap_or("(untitled)"),
                h.score,
                h.video_id,
                h.segment_title
                    .as_deref()
                    .map(|t| format!("  chapter: {t}"))
                    .unwrap_or_default()
            ));
            for e in h.evidence.iter().take(3) {
                let text: String = e.text.chars().take(160).collect();
                let ellipsis = if e.text.chars().count() > 160 {
                    "…"
                } else {
                    ""
                };
                s.push_str(&format!(
                    "    {:<10} [{}] {}{text}{ellipsis}\n",
                    format!("{:?}", e.kind).to_lowercase(),
                    e.t0,
                    if e.sources.len() > 1 || e.kind == Kind::Frame {
                        format!("({}) ", e.sources.join("+"))
                    } else {
                        String::new()
                    }
                ));
            }
            if let Some(t) = &h.thumbnail {
                s.push_str(&format!("    thumbnail {t}\n"));
            }
        }
        s.push_str(&format!(
            "{} result(s) from {} candidate(s) in {elapsed_ms} ms; lists {}; grouped by {}; index state {}",
            resp.hits.len(),
            resp.candidates,
            if resp.lists.is_empty() { "none".to_string() } else { resp.lists.join(", ") },
            if resp.grouping.is_empty() { "-" } else { &resp.grouping },
            resp.index_state
                .map(|st| st.as_str().to_string())
                .unwrap_or_else(|| "unknown".into())
        ));
        s
    });
    Ok(())
}
