//! The agent's tools (`docs/06-query-and-agents.md`). Every tool is
//! read-only against the index and the media and is exposed identically to
//! the loop's LLM, to SDK callers, and over MCP.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use vi_core::config::{roles, Config};
use vi_core::model::{Description, DescriptionKind, SegmentLevel, TargetKind, Video};
use vi_core::{DescriptionId, Result, VideoId};
use vi_index::{BlobKey, Kind, MentionQuery, Storage};
use vi_providers::{
    provenance_for, ContentPart, GenerateRequest, ImageData, Message, ProviderRegistry, Role,
    ToolSpec,
};
use vi_query::SearchRequest;

use crate::view::{media_path, render_view, ts, ViewRequest};

/// Longest text a tool returns before truncation, characters; a multi-window
/// call shares it across the windows.
pub const MAX_TEXT_CHARS: usize = 8000;
/// Most windows one `get_transcript` / `get_ocr` / `get_descriptions` call reads.
pub const MAX_TEXT_WINDOWS: usize = 6;
/// Most windows one `view` call renders (one grid each).
pub const MAX_VIEW_WINDOWS: usize = 3;
/// Frames per grid when a `view` covers several windows.
pub const MULTI_VIEW_FRAMES: usize = 12;
/// Longest `find_mentions` result, characters; samples are dropped until the
/// per-video table fits.
pub const MAX_MENTION_CHARS: usize = 24_000;
/// `search` returns at most this many hits from one video unless told
/// otherwise (or when the search is scoped to a single video).
pub const DEFAULT_PER_VIDEO_K: usize = 3;

/// What tools need.
pub struct ToolContext {
    /// Storage.
    pub storage: Arc<dyn Storage>,
    /// Providers (query embedders, VLM for `describe`).
    pub providers: Arc<ProviderRegistry>,
    /// Config (decode worker, grid layout).
    pub config: Arc<Config>,
    /// Restrict searches to these videos; empty means all.
    pub videos: Vec<VideoId>,
}

impl std::fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolContext")
            .field("videos", &self.videos)
            .finish()
    }
}

/// A call the model (or a policy) makes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Call id from the model, or generated.
    pub id: String,
    /// Tool name.
    pub name: String,
    /// Arguments.
    pub args: Value,
    /// Opaque provider state returned with the call (Gemini 3 thought
    /// signatures); echoed back when the call enters the history.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

/// What a tool produced.
#[derive(Debug, Clone, Default)]
pub struct ToolOutput {
    /// JSON or text result for the model.
    pub content: String,
    /// One-line summary for the event stream.
    pub summary: String,
    /// Images to show the model after the result (frame grids, one per
    /// window), in order.
    pub images: Vec<ImageData>,
    /// Provider spend inside the tool (`describe`).
    pub cost_usd: f64,
    /// Tokens used by the tool's own provider calls.
    pub tokens_in: u64,
    /// Output tokens used by the tool's own provider calls.
    pub tokens_out: u64,
    /// Whether decoding happened.
    pub decoded: bool,
}

/// Tool specifications for the LLM. `describe` is included only when a
/// `vlm_describe` role is bound.
pub fn specs(with_describe: bool) -> Vec<ToolSpec> {
    let mut v = vec![
        ToolSpec {
            name: "search".into(),
            description: "Hybrid search over transcripts, on-screen text, descriptions and frames. Returns ranked time ranges with evidence, spread across videos (at most per_video_k hits per video). Use precise queries; call several times with different phrasings. For 'which videos mention X' questions use find_mentions instead: it is exhaustive, search is not.".into(),
            parameters: json!({"type":"object","properties":{
                "query":{"type":"string","description":"What to look for."},
                "k":{"type":"integer","minimum":1,"maximum":20,"default":8},
                "video_id":{"type":"string","description":"Restrict to one video id."},
                "kind":{"type":"string","enum":["transcript","ocr","description","frame"],"description":"Restrict to one evidence kind. 'frame' searches pixels with the query text."},
                "per_video_k":{"type":"integer","minimum":1,"maximum":20,"default":3,"description":"Max hits from any one video; raise it to go deep into one video."}
            },"required":["query"]}),
        },
        ToolSpec {
            name: "find_mentions".into(),
            description: "Exhaustive scan of the whole index (or the given videos) for words or phrases: every transcript segment, on-screen text and description containing a term, grouped by video with counts per kind and the earliest hits with timestamps. Deterministic and cheap; the right tool for 'which videos mention X', 'find all', 'list every', 'how many videos'. Speech recognition misspells names, so pass spelling variants as separate terms (e.g. [\"LoRA\", \"Laura\"], [\"DeepSeek\", \"deep seek\"]). Every video listed has a real hit; confirm context with get_transcript when a term is ambiguous.".into(),
            parameters: json!({"type":"object","properties":{
                "terms":{"type":"array","items":{"type":"string"},"minItems":1,"maxItems":12,"description":"Words or phrases; each is matched exactly on word boundaries, case-insensitively."},
                "prefix":{"type":"boolean","default":false,"description":"Also match longer words starting with the term's last word (agent -> agents, agentic)."},
                "kinds":{"type":"array","items":{"type":"string","enum":["transcript","ocr","description"]},"description":"Restrict to evidence kinds; default all three."},
                "video_ids":{"type":"array","items":{"type":"string"},"description":"Restrict to these video ids."},
                "per_video":{"type":"integer","minimum":0,"maximum":10,"default":3,"description":"Earliest hits to return per video (0 = counts only)."}
            },"required":["terms"]}),
        },
        ToolSpec {
            name: "count_mentions".into(),
            description: "Count how often terms occur across the library, from the index's transcripts and on-screen text: per term, the number of matching transcript segments, on-screen lines and descriptions, and the number of videos with at least one hit; optionally broken down per video or per channel. Use for 'which topic is discussed most', 'how often', rankings and totals. Quote counts as approximate and say they come from transcripts.".into(),
            parameters: json!({"type":"object","properties":{
                "terms":{"type":"array","items":{"type":"string"},"minItems":1,"maxItems":12,"description":"Words or phrases to count; add spelling variants as separate terms."},
                "group_by":{"type":"string","enum":["library","video","channel"],"default":"library"},
                "prefix":{"type":"boolean","default":false,"description":"Also count longer words starting with the term's last word."},
                "kinds":{"type":"array","items":{"type":"string","enum":["transcript","ocr","description"]}},
                "video_ids":{"type":"array","items":{"type":"string"}}
            },"required":["terms"]}),
        },
        ToolSpec {
            name: "library_stats".into(),
            description: "Facts about the library as a whole: number of videos, total duration, channels with their video counts and durations, and every video's id, title, channel, duration and publication date. Use for 'how many videos/talks', 'which channels', 'how long'.".into(),
            parameters: json!({"type":"object","properties":{}}),
        },
        ToolSpec {
            name: "list_videos".into(),
            description: "List the videos in the index with ids, titles and durations.".into(),
            parameters: json!({"type":"object","properties":{}}),
        },
        ToolSpec {
            name: "timeline".into(),
            description: "Segments of one video: chapters (with titles), scenes, or shots.".into(),
            parameters: json!({"type":"object","properties":{
                "video_id":{"type":"string"},
                "level":{"type":"string","enum":["chapter","scene","shot"],"default":"chapter"}
            },"required":["video_id"]}),
        },
        ToolSpec {
            name: "get_transcript".into(),
            description: format!("Transcript text with timestamps for one or more time ranges of one video (seconds). {WINDOWS_RULE}"),
            parameters: text_window_params(),
        },
        ToolSpec {
            name: "get_ocr".into(),
            description: format!("On-screen text (slides, captions, code) with timestamps for one or more time ranges of one video. {WINDOWS_RULE}"),
            parameters: text_window_params(),
        },
        ToolSpec {
            name: "get_descriptions".into(),
            description: format!("Stored visual descriptions for one or more time ranges of one video, when any exist. {WINDOWS_RULE}"),
            parameters: text_window_params(),
        },
        ToolSpec {
            name: "view".into(),
            description: "Look at the pixels: decodes a time range (at most 120 s) at the given frame rate and returns a labelled frame grid image plus the transcript of the window. Several candidate moments in one call cost one tool call: pass windows=[{t0,t1},...] (up to 3, same video) instead of t0/t1 and get one grid per window (at most 12 frames each), images in window order. Costs decoding and image tokens; use after search has narrowed the range.".into(),
            parameters: json!({"type":"object","properties":{
                "video_id":{"type":"string"},"t0":{"type":"number"},"t1":{"type":"number"},
                "windows":{"type":"array","minItems":1,"maxItems":3,"items":{"type":"object","properties":{"t0":{"type":"number"},"t1":{"type":"number"}},"required":["t0","t1"]},"description":"Up to 3 time ranges of the same video, one grid each; use instead of t0/t1."},
                "fps":{"type":"number","default":1,"description":"Frames per second; at most 16 frames per view (12 per window when several)."}
            },"required":["video_id"]}),
        },
    ];
    if with_describe {
        v.push(ToolSpec {
            name: "describe".into(),
            description: "Ask the vision model to describe a time range (at most 120 s), optionally answering a question about it. The description is stored in the index. Use when you need a detailed reading of the visuals.".into(),
            parameters: json!({"type":"object","properties":{
                "video_id":{"type":"string"},"t0":{"type":"number"},"t1":{"type":"number"},
                "question":{"type":"string"}
            },"required":["video_id","t0","t1"]}),
        });
    }
    v
}

const WINDOWS_RULE: &str = "Several ranges in one call cost one tool call: pass windows=[{t0,t1},...] (up to 6, same video) instead of t0/t1 and get one entry per window, in time order.";

fn text_window_params() -> Value {
    json!({"type":"object","properties":{
        "video_id":{"type":"string"},"t0":{"type":"number"},"t1":{"type":"number"},
        "windows":{"type":"array","minItems":1,"maxItems":6,"items":{"type":"object","properties":{"t0":{"type":"number"},"t1":{"type":"number"}},"required":["t0","t1"]},"description":"Up to 6 time ranges of the same video; use instead of t0/t1."}
    },"required":["video_id"]})
}

fn arg_str<'a>(args: &'a Value, k: &str) -> Option<&'a str> {
    args.get(k)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
}

fn arg_f64(args: &Value, k: &str) -> Option<f64> {
    args.get(k).and_then(|v| {
        v.as_f64()
            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
    })
}

fn video_arg(ctx: &ToolContext, args: &Value) -> std::result::Result<VideoId, String> {
    match arg_str(args, "video_id") {
        Some(s) => VideoId::parse(s).map_err(|_| format!("'{s}' is not a video id")),
        None => match ctx.videos.as_slice() {
            [one] => Ok(*one),
            _ => Err("video_id is required".into()),
        },
    }
}

fn range_args(args: &Value, duration: f64) -> std::result::Result<(f64, f64), String> {
    let t0 = arg_f64(args, "t0").ok_or("t0 is required")?;
    let t1 = arg_f64(args, "t1").ok_or("t1 is required")?;
    if t1 <= t0 {
        return Err(format!("t1 ({t1}) must be greater than t0 ({t0})"));
    }
    Ok((t0.clamp(0.0, duration), t1.min(duration.max(t0 + 0.5))))
}

/// Time windows of one call: `windows: [{t0, t1}]` (1 to `max`), else the
/// single `t0`/`t1`. Sorted by start, overlapping or touching windows
/// merged, each clamped to the video.
fn windows_arg(
    args: &Value,
    duration: f64,
    max: usize,
) -> std::result::Result<Vec<(f64, f64)>, String> {
    let Some(list) = args.get("windows").filter(|w| !w.is_null()) else {
        return Ok(vec![range_args(args, duration)?]);
    };
    let list = list
        .as_array()
        .ok_or("windows must be a list of {t0, t1} objects")?;
    if list.is_empty() {
        return Err("windows must hold at least one {t0, t1}".into());
    }
    if list.len() > max {
        return Err(format!("at most {max} windows per call (got {})", list.len()));
    }
    let mut wins = Vec::with_capacity(list.len());
    for (i, w) in list.iter().enumerate() {
        wins.push(range_args(w, duration).map_err(|e| format!("windows[{i}]: {e}"))?);
    }
    wins.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut merged: Vec<(f64, f64)> = Vec::with_capacity(wins.len());
    for (t0, t1) in wins {
        match merged.last_mut() {
            Some(last) if t0 <= last.1 => last.1 = last.1.max(t1),
            _ => merged.push((t0, t1)),
        }
    }
    Ok(merged)
}

fn hms(secs: f64) -> String {
    vi_perceive::grid::hms(secs)
}

fn truncate(mut s: String, max: usize) -> String {
    if s.chars().count() > max {
        let cut: String = s.chars().take(max).collect();
        s = format!("{cut}\n[truncated; narrow the range]");
    }
    s
}

async fn video(ctx: &ToolContext, id: VideoId) -> std::result::Result<Video, String> {
    match ctx.storage.get_video(id).await {
        Ok(Some(v)) => Ok(v),
        Ok(None) => Err(format!("no video {id} in this index")),
        Err(e) => Err(e.to_string()),
    }
}

/// Execute one call. Errors that the model can act on (bad arguments,
/// missing video) come back as content, not as `Err`.
pub async fn execute(ctx: &ToolContext, call: &ToolCall) -> Result<ToolOutput> {
    let r = match call.name.as_str() {
        "search" => tool_search(ctx, &call.args).await,
        "find_mentions" => tool_find_mentions(ctx, &call.args).await,
        "count_mentions" => tool_count_mentions(ctx, &call.args).await,
        "library_stats" => tool_library_stats(ctx).await,
        "list_videos" => tool_list_videos(ctx).await,
        "timeline" => tool_timeline(ctx, &call.args).await,
        "get_transcript" => tool_window(ctx, &call.args, Kind::Transcript).await,
        "get_ocr" => tool_window(ctx, &call.args, Kind::Ocr).await,
        "get_descriptions" => tool_window(ctx, &call.args, Kind::Description).await,
        "view" => tool_view(ctx, &call.args).await,
        "describe" => tool_describe(ctx, &call.args).await,
        other => Err(format!("unknown tool '{other}'")),
    };
    Ok(match r {
        Ok(out) => out,
        Err(msg) => ToolOutput {
            content: json!({"error": msg}).to_string(),
            summary: format!("error: {msg}"),
            ..ToolOutput::default()
        },
    })
}

type ToolResult = std::result::Result<ToolOutput, String>;

async fn tool_search(ctx: &ToolContext, args: &Value) -> ToolResult {
    let query = arg_str(args, "query")
        .ok_or("query is required")?
        .to_string();
    let k = args
        .get("k")
        .and_then(|v| v.as_u64())
        .unwrap_or(8)
        .clamp(1, 20) as usize;
    let mut videos = ctx.videos.clone();
    if let Some(v) = arg_str(args, "video_id") {
        videos = vec![VideoId::parse(v).map_err(|_| format!("'{v}' is not a video id"))?];
    }
    let kinds = match arg_str(args, "kind") {
        Some("transcript") => vec![Kind::Transcript],
        Some("ocr") => vec![Kind::Ocr],
        Some("description") => vec![Kind::Description],
        Some("frame") => vec![Kind::Frame],
        _ => vec![],
    };
    // A single-video search wants depth, not spread.
    let per_video_k = if videos.len() == 1 {
        None
    } else {
        Some(
            args.get("per_video_k")
                .and_then(|v| v.as_u64())
                .map_or(DEFAULT_PER_VIDEO_K, |v| v.clamp(1, 20) as usize),
        )
    };
    let req = SearchRequest {
        query: query.clone(),
        videos,
        kinds,
        k,
        text_only: false,
        per_video_k,
    };
    let resp = vi_query::search(ctx.storage.as_ref(), Some(&ctx.providers), &req)
        .await
        .map_err(|e| e.to_string())?;
    let hits: Vec<Value> = resp
        .hits
        .iter()
        .map(|h| {
            json!({
                "video_id": h.video_id.to_string(),
                "title": h.title,
                "t0": (h.t0.as_secs_f64() * 10.0).round() / 10.0,
                "t1": (h.t1.as_secs_f64() * 10.0).round() / 10.0,
                "chapter": h.segment_title,
                "score": (h.score * 1000.0).round() / 1000.0,
                "evidence": h.evidence.iter().take(4).map(|e| json!({
                    "kind": format!("{:?}", e.kind).to_lowercase(),
                    "t": (e.t0.as_secs_f64() * 10.0).round() / 10.0,
                    "text": e.text.chars().take(300).collect::<String>(),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    Ok(ToolOutput {
        content: json!({"query": query, "hits": hits, "index_state": resp.index_state}).to_string(),
        summary: format!("{} hits for \"{query}\"", resp.hits.len()),
        ..ToolOutput::default()
    })
}


fn terms_arg(args: &Value) -> std::result::Result<Vec<String>, String> {
    let terms: Vec<String> = match args.get("terms") {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|t| t.as_str())
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        Some(Value::String(t)) if !t.trim().is_empty() => vec![t.trim().to_string()],
        _ => Vec::new(),
    };
    if terms.is_empty() {
        return Err("terms is required: a list of words or phrases (add spelling variants)".into());
    }
    if terms.len() > 12 {
        return Err("at most 12 terms per call".into());
    }
    Ok(terms)
}

fn text_kinds_arg(args: &Value) -> std::result::Result<Vec<Kind>, String> {
    let mut out = Vec::new();
    for k in args
        .get("kinds")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        match k.as_str() {
            Some("transcript") => out.push(Kind::Transcript),
            Some("ocr") => out.push(Kind::Ocr),
            Some("description") => out.push(Kind::Description),
            other => {
                return Err(format!(
                    "unknown kind {other:?}; expected transcript, ocr or description"
                ))
            }
        }
    }
    Ok(out)
}

/// Videos a library-wide tool looks at: `video_ids` (or `video_id`) from the
/// arguments, else the context's restriction, else the whole index.
fn scope_arg(ctx: &ToolContext, args: &Value) -> std::result::Result<Vec<VideoId>, String> {
    let mut ids = Vec::new();
    for v in args
        .get("video_ids")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .chain(arg_str(args, "video_id"))
    {
        ids.push(VideoId::parse(v).map_err(|_| format!("'{v}' is not a video id"))?);
    }
    if ids.is_empty() {
        ids = ctx.videos.clone();
    }
    Ok(ids)
}

/// Videos in scope, for "M of K videos" figures and channel sizes.
async fn scoped_videos(
    ctx: &ToolContext,
    scope: &[VideoId],
) -> std::result::Result<Vec<Video>, String> {
    let all = ctx.storage.list_videos().await.map_err(|e| e.to_string())?;
    Ok(all
        .into_iter()
        .filter(|v| scope.is_empty() || scope.contains(&v.id))
        .collect())
}

fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Transcript => "transcript",
        Kind::Ocr => "ocr",
        Kind::Description => "description",
        Kind::Segment => "segment",
        Kind::Frame => "frame",
    }
}

fn round1(secs: f64) -> f64 {
    (secs * 10.0).round() / 10.0
}

const MENTION_NOTE: &str = "Counts are index rows containing the term: transcript segments (speech, as recognised by ASR), distinct on-screen text lines per minute (OCR) and stored descriptions. ASR misspells names; add variants as extra terms. Cite hits as [[cite:VIDEO_ID:T0-T1]].";

async fn tool_find_mentions(ctx: &ToolContext, args: &Value) -> ToolResult {
    let terms = terms_arg(args)?;
    let scope = scope_arg(ctx, args)?;
    let kinds = text_kinds_arg(args)?;
    let per_video = args
        .get("per_video")
        .and_then(|v| v.as_u64())
        .unwrap_or(3)
        .clamp(0, 10) as usize;
    let prefix = args
        .get("prefix")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let found = ctx
        .storage
        .find_mentions(&MentionQuery {
            terms: terms.clone(),
            videos: scope.clone(),
            kinds,
            prefix,
            samples_per_video: per_video,
        })
        .await
        .map_err(|e| e.to_string())?;
    let searched = scoped_videos(ctx, &scope).await?.len();
    let total: u64 = found.iter().map(|v| v.total()).sum();
    // Shrink the samples until the table fits the size cap.
    let mut per = per_video;
    loop {
        let rows: Vec<Value> = found
            .iter()
            .map(|v| {
                let mut counts = serde_json::Map::new();
                for c in &v.counts {
                    let e = counts
                        .entry(kind_name(c.kind))
                        .or_insert(Value::from(0u64));
                    *e = Value::from(e.as_u64().unwrap_or(0) + c.count);
                }
                let first: Vec<Value> = v
                    .samples
                    .iter()
                    .take(per)
                    .map(|h| {
                        json!({
                            "kind": kind_name(h.kind),
                            "t0": round1(h.t0.as_secs_f64()),
                            "t1": round1(h.t1.as_secs_f64()),
                            "term": h.term,
                            "text": h.text.chars().take(160).collect::<String>(),
                        })
                    })
                    .collect();
                json!({
                    "video_id": v.video_id.to_string(),
                    "title": v.title,
                    "counts": counts,
                    "total": v.total(),
                    "first": first,
                })
            })
            .collect();
        let content = json!({
            "terms": terms,
            "prefix": prefix,
            "videos_searched": searched,
            "videos_with_hits": found.len(),
            "total_hits": total,
            "videos": rows,
            "note": MENTION_NOTE,
        })
        .to_string();
        if content.len() <= MAX_MENTION_CHARS || per == 0 {
            return Ok(ToolOutput {
                content,
                summary: format!(
                    "{} of {} videos mention {} ({} hits)",
                    found.len(),
                    searched,
                    terms.join(" / "),
                    total
                ),
                ..ToolOutput::default()
            });
        }
        per -= 1;
    }
}

async fn tool_count_mentions(ctx: &ToolContext, args: &Value) -> ToolResult {
    let terms = terms_arg(args)?;
    let scope = scope_arg(ctx, args)?;
    let kinds = text_kinds_arg(args)?;
    let prefix = args
        .get("prefix")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let group_by = arg_str(args, "group_by").unwrap_or("library");
    if !matches!(group_by, "library" | "video" | "channel") {
        return Err(format!(
            "group_by must be library, video or channel, not '{group_by}'"
        ));
    }
    let found = ctx
        .storage
        .find_mentions(&MentionQuery {
            terms: terms.clone(),
            videos: scope.clone(),
            kinds,
            prefix,
            samples_per_video: 0,
        })
        .await
        .map_err(|e| e.to_string())?;
    let videos = scoped_videos(ctx, &scope).await?;
    let channel_of = |c: &Option<String>| c.clone().unwrap_or_else(|| "unknown".into());

    // Per term: rows per kind, total, and videos with at least one hit.
    let per_term: Vec<Value> = terms
        .iter()
        .map(|t| {
            let mut by_kind = serde_json::Map::new();
            let mut total = 0u64;
            let mut with = 0usize;
            for v in &found {
                let mut any = false;
                for c in v.counts.iter().filter(|c| &c.term == t) {
                    any = true;
                    total += c.count;
                    let e = by_kind
                        .entry(kind_name(c.kind))
                        .or_insert(Value::from(0u64));
                    *e = Value::from(e.as_u64().unwrap_or(0) + c.count);
                }
                if any {
                    with += 1;
                }
            }
            json!({"term": t, "counts": by_kind, "total": total, "videos": with})
        })
        .collect();

    let groups: Value = match group_by {
        "video" => Value::Array(
            found
                .iter()
                .map(|v| {
                    let mut by_term = serde_json::Map::new();
                    for c in &v.counts {
                        let e = by_term
                            .entry(c.term.clone())
                            .or_insert(Value::from(0u64));
                        *e = Value::from(e.as_u64().unwrap_or(0) + c.count);
                    }
                    json!({
                        "video_id": v.video_id.to_string(),
                        "title": v.title,
                        "channel": v.channel,
                        "total": v.total(),
                        "by_term": by_term,
                    })
                })
                .collect(),
        ),
        "channel" => {
            let mut chans: std::collections::BTreeMap<String, (usize, usize, u64, serde_json::Map<String, Value>)> =
                std::collections::BTreeMap::new();
            for v in &videos {
                chans.entry(channel_of(&v.channel)).or_default().0 += 1;
            }
            for v in &found {
                let e = chans.entry(channel_of(&v.channel)).or_default();
                e.1 += 1;
                e.2 += v.total();
                for c in &v.counts {
                    let t = e.3.entry(c.term.clone()).or_insert(Value::from(0u64));
                    *t = Value::from(t.as_u64().unwrap_or(0) + c.count);
                }
            }
            let mut rows: Vec<(u64, Value)> = chans
                .into_iter()
                .map(|(name, (n, with, total, by_term))| {
                    (
                        total,
                        json!({"channel": name, "videos": n, "videos_with_hits": with, "total": total, "by_term": by_term}),
                    )
                })
                .collect();
            rows.sort_by_key(|a| std::cmp::Reverse(a.0));
            Value::Array(rows.into_iter().map(|(_, v)| v).collect())
        }
        _ => Value::Null,
    };
    let total: u64 = found.iter().map(|v| v.total()).sum();
    let mut content = json!({
        "terms": per_term,
        "prefix": prefix,
        "videos_searched": videos.len(),
        "videos_with_hits": found.len(),
        "total_hits": total,
        "note": MENTION_NOTE,
    });
    if !groups.is_null() {
        content[format!("by_{group_by}")] = groups;
    }
    Ok(ToolOutput {
        content: content.to_string(),
        summary: format!(
            "{} hits for {} in {} of {} videos",
            total,
            terms.join(" / "),
            found.len(),
            videos.len()
        ),
        ..ToolOutput::default()
    })
}

async fn tool_library_stats(ctx: &ToolContext) -> ToolResult {
    let videos = scoped_videos(ctx, &ctx.videos).await?;
    let total_secs: f64 = videos.iter().map(|v| v.duration.as_secs_f64()).sum();
    let mut chans: std::collections::BTreeMap<String, (usize, f64)> =
        std::collections::BTreeMap::new();
    for v in &videos {
        let e = chans
            .entry(v.channel.clone().unwrap_or_else(|| "unknown".into()))
            .or_default();
        e.0 += 1;
        e.1 += v.duration.as_secs_f64();
    }
    let mut channels: Vec<Value> = chans
        .into_iter()
        .map(|(name, (n, secs))| json!({"channel": name, "videos": n, "duration_secs": secs.round()}))
        .collect();
    channels.sort_by(|a, b| b["videos"].as_u64().cmp(&a["videos"].as_u64()));
    let rows: Vec<Value> = videos
        .iter()
        .map(|v| {
            json!({
                "video_id": v.id.to_string(),
                "title": v.title,
                "channel": v.channel,
                "duration_secs": v.duration.as_secs_f64().round(),
                "published_at": v.published_at.map(|d| d.format("%Y-%m-%d").to_string()),
                "index_state": v.index_state,
            })
        })
        .collect();
    Ok(ToolOutput {
        content: json!({
            "videos": videos.len(),
            "total_duration_secs": total_secs.round(),
            "total_duration": hms(total_secs),
            "channels": channels,
            "video_list": rows,
        })
        .to_string(),
        summary: format!(
            "{} videos, {} in {} channel(s)",
            videos.len(),
            hms(total_secs),
            channels.len()
        ),
        ..ToolOutput::default()
    })
}

async fn tool_list_videos(ctx: &ToolContext) -> ToolResult {
    let videos = ctx.storage.list_videos().await.map_err(|e| e.to_string())?;
    let rows: Vec<Value> = videos
        .iter()
        .filter(|v| ctx.videos.is_empty() || ctx.videos.contains(&v.id))
        .map(|v| {
            json!({
                "video_id": v.id.to_string(),
                "title": v.title,
                "duration_secs": v.duration.as_secs_f64().round(),
                "index_state": v.index_state,
            })
        })
        .collect();
    Ok(ToolOutput {
        content: json!({"videos": rows}).to_string(),
        summary: format!("{} videos", rows.len()),
        ..ToolOutput::default()
    })
}

async fn tool_timeline(ctx: &ToolContext, args: &Value) -> ToolResult {
    let id = video_arg(ctx, args)?;
    let v = video(ctx, id).await?;
    let level = match arg_str(args, "level").unwrap_or("chapter") {
        "shot" => SegmentLevel::Shot,
        "scene" => SegmentLevel::Scene,
        _ => SegmentLevel::Chapter,
    };
    let segs = ctx
        .storage
        .segments(id, level)
        .await
        .map_err(|e| e.to_string())?;
    let rows: Vec<Value> = segs
        .iter()
        .take(300)
        .map(|s| {
            json!({
                "t0": s.t0.as_secs_f64().round(),
                "t1": s.t1.as_secs_f64().round(),
                "title": s.title,
                "summary": s.summary,
            })
        })
        .collect();
    Ok(ToolOutput {
        content: json!({
            "video_id": id.to_string(),
            "title": v.title,
            "duration_secs": v.duration.as_secs_f64().round(),
            "level": level.as_str(),
            "segments": rows,
            "note": if segs.is_empty() { Some("no segments at this level; try another level") } else { None }
        })
        .to_string(),
        summary: format!("{} {} segments", rows.len(), level.as_str()),
        ..ToolOutput::default()
    })
}

/// Lines of one kind in one window, timestamped, in time order.
async fn window_lines(
    ctx: &ToolContext,
    id: VideoId,
    t0: f64,
    t1: f64,
    kind: Kind,
) -> std::result::Result<(Vec<String>, &'static str), String> {
    let w = ctx
        .storage
        .time_window(id, ts(t0), ts(t1), &[kind])
        .await
        .map_err(|e| e.to_string())?;
    Ok(match kind {
        Kind::Transcript => (
            w.transcript
                .iter()
                .map(|s| format!("[{}] {}", hms(s.t0.as_secs_f64()), s.text))
                .collect(),
            "transcript",
        ),
        Kind::Ocr => {
            let mut seen = std::collections::BTreeSet::new();
            (
                w.ocr
                    .iter()
                    .filter(|o| seen.insert(o.text.clone()))
                    .map(|o| format!("[{}] {}", hms(o.t.as_secs_f64()), o.text))
                    .collect(),
                "ocr",
            )
        }
        _ => (
            w.descriptions.iter().map(|d| d.text.clone()).collect(),
            "descriptions",
        ),
    })
}

/// `get_transcript`, `get_ocr`, `get_descriptions`. A single `t0`/`t1` keeps
/// the flat `{video_id, t0, t1, kind, count, text}` shape; `windows` returns
/// `{video_id, kind, windows: [{t0, t1, count, text}]}` with the text cap
/// shared across windows. Either way it is one tool call.
async fn tool_window(ctx: &ToolContext, args: &Value, kind: Kind) -> ToolResult {
    let id = video_arg(ctx, args)?;
    let v = video(ctx, id).await?;
    let wins = windows_arg(args, v.duration.as_secs_f64(), MAX_TEXT_WINDOWS)?;
    let multi = args.get("windows").is_some_and(|w| !w.is_null());
    let cap = MAX_TEXT_CHARS / wins.len();
    let mut rows = Vec::with_capacity(wins.len());
    let mut total = 0usize;
    let mut label = "transcript";
    for &(t0, t1) in &wins {
        let (lines, l) = window_lines(ctx, id, t0, t1, kind).await?;
        label = l;
        let n = lines.len();
        total += n;
        let text = truncate(lines.join("\n"), cap);
        rows.push(json!({
            "t0": t0, "t1": t1, "count": n,
            "text": if text.is_empty() { format!("(no {label} in this range)") } else { text },
        }));
    }
    if !multi {
        let mut row = rows.pop().unwrap_or_default();
        row["video_id"] = Value::String(id.to_string());
        row["kind"] = Value::String(label.into());
        let (t0, t1) = wins[0];
        return Ok(ToolOutput {
            content: row.to_string(),
            summary: format!("{total} {label} lines in [{}, {}]", hms(t0), hms(t1)),
            ..ToolOutput::default()
        });
    }
    Ok(ToolOutput {
        content: json!({
            "video_id": id.to_string(), "kind": label,
            "windows": rows,
        })
        .to_string(),
        summary: format!(
            "{} windows, {total} {label} lines ({})",
            wins.len(),
            wins.iter()
                .map(|(a, b)| format!("[{}, {}]", hms(*a), hms(*b)))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        ..ToolOutput::default()
    })
}

async fn grid_for(
    ctx: &ToolContext,
    v: &Video,
    t0: f64,
    t1: f64,
    fps: f64,
    max_frames: usize,
) -> std::result::Result<(crate::view::ViewResult, BlobKey), String> {
    let cols = ctx
        .config
        .policy
        .values()
        .next()
        .and_then(|p| vi_perceive::grid::GridLayout::parse_cols(&p.vlm_grid))
        .unwrap_or(3);
    let req = ViewRequest {
        t0,
        t1,
        fps,
        cols,
        max_frames,
        ..ViewRequest::default()
    };
    let view = render_view(&ctx.config.media.worker, v, req)
        .await
        .map_err(|e| e.to_string())?;
    let key = BlobKey::for_bytes(&view.png);
    ctx.storage
        .put_blob(&key, bytes::Bytes::from(view.png.clone()))
        .await
        .map_err(|e| e.to_string())?;
    Ok((view, key))
}

async fn transcript_text(ctx: &ToolContext, id: VideoId, t0: f64, t1: f64, cap: usize) -> String {
    match ctx
        .storage
        .time_window(id, ts(t0), ts(t1), &[Kind::Transcript])
        .await
    {
        Ok(w) => truncate(
            w.transcript
                .iter()
                .map(|s| format!("[{}] {}", hms(s.t0.as_secs_f64()), s.text))
                .collect::<Vec<_>>()
                .join("\n"),
            cap,
        ),
        Err(_) => String::new(),
    }
}

async fn tool_view(ctx: &ToolContext, args: &Value) -> ToolResult {
    let id = video_arg(ctx, args)?;
    let v = video(ctx, id).await?;
    if media_path(&v).filter(|p| p.is_file()).is_none() {
        return Err("the media file for this video is not on this machine; use get_transcript, get_ocr and search instead".into());
    }
    let wins = windows_arg(args, v.duration.as_secs_f64(), MAX_VIEW_WINDOWS)?;
    let multi = args.get("windows").is_some_and(|w| !w.is_null());
    let fps = arg_f64(args, "fps").unwrap_or(1.0);
    let max_frames = if wins.len() > 1 {
        MULTI_VIEW_FRAMES
    } else {
        crate::view::MAX_FRAMES
    };
    // Decode the windows together (each is its own worker call), then keep
    // window order for the text, the images and the summary.
    let grids = futures::future::join_all(
        wins.iter()
            .map(|&(t0, t1)| grid_for(ctx, &v, t0, t1, fps, max_frames)),
    )
    .await;
    let mut rows = Vec::with_capacity(wins.len());
    let mut images = Vec::with_capacity(wins.len());
    let mut frames = 0usize;
    let mut distinct = 0usize;
    for (&(t0, t1), grid) in wins.iter().zip(grids) {
        let (view, key) = grid?;
        let transcript = transcript_text(ctx, id, t0, t1, 3000 / wins.len()).await;
        frames += view.timestamps.len();
        distinct += view.distinct;
        rows.push(json!({
            "t0": t0, "t1": t1,
            "frames": view.timestamps.len(), "distinct_frames": view.distinct,
            "timestamps": view.timestamps.iter().map(|t| (t * 10.0).round() / 10.0).collect::<Vec<_>>(),
            "grid_blob": key.uri(),
            "transcript": transcript,
        }));
        images.push(ImageData::Encoded {
            mime: "image/png",
            bytes: bytes::Bytes::from(view.png),
        });
    }
    let content = if multi {
        json!({
            "video_id": id.to_string(),
            "windows": rows,
            "note": format!("{} frame grids follow as images, one per window in this order; tiles are labelled HH:MM:SS.", wins.len()),
        })
    } else {
        let mut row = rows.pop().unwrap_or_default();
        row["video_id"] = Value::String(id.to_string());
        row["note"] = Value::String("The frame grid follows as an image; tiles are labelled HH:MM:SS.".into());
        row
    };
    let summary = if multi {
        format!(
            "{} windows, {frames} frames ({distinct} distinct): {}",
            wins.len(),
            wins.iter()
                .map(|(a, b)| format!("[{}, {}]", hms(*a), hms(*b)))
                .collect::<Vec<_>>()
                .join(" ")
        )
    } else {
        format!("{frames} frames ({distinct} distinct) in [{}, {}]", hms(wins[0].0), hms(wins[0].1))
    };
    Ok(ToolOutput {
        content: content.to_string(),
        summary,
        images,
        decoded: true,
        ..ToolOutput::default()
    })
}

async fn tool_describe(ctx: &ToolContext, args: &Value) -> ToolResult {
    let id = video_arg(ctx, args)?;
    let v = video(ctx, id).await?;
    if media_path(&v).filter(|p| p.is_file()).is_none() {
        return Err("the media file for this video is not on this machine".into());
    }
    let (t0, t1) = range_args(args, v.duration.as_secs_f64())?;
    let vlm = ctx
        .providers
        .vlm(roles::VLM_DESCRIBE)
        .map_err(|e| e.to_string())?;
    let (view, key) = grid_for(ctx, &v, t0, t1, 1.0, crate::view::MAX_FRAMES).await?;
    let transcript = transcript_text(ctx, id, t0, t1, 3000).await;
    let prompt = vi_providers::prompts::get("vlm_describe").ok_or("prompt missing")?;
    let mut user_text = format!(
        "Segment {}–{} of \"{}\".\n\nTranscript (data):\n{}",
        hms(t0),
        hms(t1),
        v.title.clone().unwrap_or_default(),
        if transcript.is_empty() {
            "(none)".to_string()
        } else {
            transcript
        }
    );
    if let Some(q) = arg_str(args, "question") {
        user_text.push_str(&format!(
            "\n\nAlso answer this question about the segment, in a field \"answer\": {q}"
        ));
    }
    let req = GenerateRequest {
        messages: vec![
            Message::text(Role::System, prompt.text.clone()),
            Message {
                role: Role::User,
                parts: vec![
                    ContentPart::Text(user_text),
                    ContentPart::Image(ImageData::Encoded {
                        mime: "image/png",
                        bytes: bytes::Bytes::from(view.png.clone()),
                    }),
                ],
            },
        ],
        tools: vec![],
        tool_choice: Default::default(),
        max_tokens: 1200,
        temperature: 0.0,
        json_schema: None,
        model: None,
    };
    let out =
        vi_providers::adapters::collect_stream(vlm.generate(req).await.map_err(|e| e.to_string())?)
            .await
            .map_err(|e| e.to_string())?;
    let stats = out.stats.clone().unwrap_or_else(|| {
        vi_providers::CallStats::new(
            vlm.provider_name(),
            vlm.model(),
            out.usage,
            &vi_core::config::Pricing::default(),
        )
    });
    // Store the description against the shot containing the midpoint, else
    // the nearest frame sample, so it is searchable and citable.
    let mid = ts((t0 + t1) / 2.0);
    let shots = ctx
        .storage
        .segments(id, SegmentLevel::Shot)
        .await
        .unwrap_or_default();
    let target = match shots.iter().find(|s| mid >= s.t0 && mid < s.t1) {
        Some(s) => Some((TargetKind::Segment, s.id.to_string())),
        None => ctx
            .storage
            .time_window(id, ts(t0), ts(t1), &[Kind::Frame])
            .await
            .ok()
            .and_then(|w| {
                w.frames
                    .first()
                    .map(|f| (TargetKind::Frame, f.id.to_string()))
            }),
    };
    let structured: Option<Value> = serde_json::from_str(out.text.trim()).ok();
    let text = structured
        .as_ref()
        .map(|s| {
            let mut parts = Vec::new();
            for k in [
                "summary",
                "visible",
                "actions",
                "on_screen_text",
                "topics",
                "answer",
            ] {
                if let Some(v) = s.get(k) {
                    let t = match v {
                        Value::String(t) => t.clone(),
                        Value::Array(a) => a
                            .iter()
                            .map(|x| {
                                x.as_str()
                                    .map(str::to_string)
                                    .unwrap_or_else(|| x.to_string())
                            })
                            .collect::<Vec<_>>()
                            .join("; "),
                        other => other.to_string(),
                    };
                    if !t.is_empty() {
                        parts.push(t);
                    }
                }
            }
            parts.join(" ")
        })
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| out.text.trim().to_string());
    if let Some((kind, target_id)) = target {
        let prov = provenance_for(
            "describe",
            1,
            &stats,
            Some(prompt.hash.clone()),
            json!({"t0": t0, "t1": t1, "grid_blob": key.uri(), "frames": view.timestamps.len()}),
        );
        let _ = ctx.storage.put_provenance(&prov).await;
        let _ = ctx
            .storage
            .put_descriptions(&[Description {
                id: DescriptionId::new(),
                target_kind: kind,
                target_id,
                kind: if structured.is_some() {
                    DescriptionKind::Structured
                } else {
                    DescriptionKind::Caption
                },
                text: text.clone(),
                structured: structured.clone(),
                provenance_id: prov.id,
            }])
            .await;
    }
    Ok(ToolOutput {
        content: json!({
            "video_id": id.to_string(), "t0": t0, "t1": t1,
            "description": structured.unwrap_or(Value::String(text.clone())),
            "grid_blob": key.uri(),
        })
        .to_string(),
        summary: format!(
            "described [{}, {}] with {} ({} tokens)",
            hms(t0),
            hms(t1),
            vlm.model(),
            stats.usage.tokens_in + stats.usage.tokens_out
        ),
        images: Vec::new(),
        cost_usd: stats.cost_usd,
        tokens_in: stats.usage.tokens_in,
        tokens_out: stats.usage.tokens_out,
        decoded: true,
    })
}
