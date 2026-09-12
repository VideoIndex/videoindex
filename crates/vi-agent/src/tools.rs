//! The agent's tools (`docs/06-query-and-agents.md`). Every tool is
//! read-only against the index and the media and is exposed identically to
//! the loop's LLM, to SDK callers, and over MCP.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use vi_core::config::{roles, Config};
use vi_core::model::{Description, DescriptionKind, SegmentLevel, TargetKind, Video};
use vi_core::{DescriptionId, Result, VideoId};
use vi_index::{BlobKey, Kind, Storage};
use vi_providers::{
    provenance_for, ContentPart, GenerateRequest, ImageData, Message, ProviderRegistry, Role,
    ToolSpec,
};
use vi_query::SearchRequest;

use crate::view::{media_path, render_view, ts, ViewRequest};

/// Longest text a tool returns before truncation, characters.
pub const MAX_TEXT_CHARS: usize = 8000;

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
}

/// What a tool produced.
#[derive(Debug, Clone, Default)]
pub struct ToolOutput {
    /// JSON or text result for the model.
    pub content: String,
    /// One-line summary for the event stream.
    pub summary: String,
    /// An image to show the model after the result (frame grids).
    pub image: Option<ImageData>,
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
            description: "Hybrid search over transcripts, on-screen text, descriptions and frames. Returns ranked time ranges with evidence. Use precise queries; call several times with different phrasings.".into(),
            parameters: json!({"type":"object","properties":{
                "query":{"type":"string","description":"What to look for."},
                "k":{"type":"integer","minimum":1,"maximum":20,"default":8},
                "video_id":{"type":"string","description":"Restrict to one video id."},
                "kind":{"type":"string","enum":["transcript","ocr","description","frame"],"description":"Restrict to one evidence kind. 'frame' searches pixels with the query text."}
            },"required":["query"]}),
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
            description: "Transcript text with timestamps for a time range of one video (seconds).".into(),
            parameters: json!({"type":"object","properties":{
                "video_id":{"type":"string"},"t0":{"type":"number"},"t1":{"type":"number"}
            },"required":["video_id","t0","t1"]}),
        },
        ToolSpec {
            name: "get_ocr".into(),
            description: "On-screen text (slides, captions, code) with timestamps for a time range of one video.".into(),
            parameters: json!({"type":"object","properties":{
                "video_id":{"type":"string"},"t0":{"type":"number"},"t1":{"type":"number"}
            },"required":["video_id","t0","t1"]}),
        },
        ToolSpec {
            name: "get_descriptions".into(),
            description: "Stored visual descriptions for a time range of one video, when any exist.".into(),
            parameters: json!({"type":"object","properties":{
                "video_id":{"type":"string"},"t0":{"type":"number"},"t1":{"type":"number"}
            },"required":["video_id","t0","t1"]}),
        },
        ToolSpec {
            name: "view".into(),
            description: "Look at the pixels: decodes a time range (at most 120 s) at the given frame rate and returns a labelled frame grid image plus the transcript of the window. Costs decoding and image tokens; use after search has narrowed the range.".into(),
            parameters: json!({"type":"object","properties":{
                "video_id":{"type":"string"},"t0":{"type":"number"},"t1":{"type":"number"},
                "fps":{"type":"number","default":1,"description":"Frames per second; at most 16 frames per view."}
            },"required":["video_id","t0","t1"]}),
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
    let req = SearchRequest {
        query: query.clone(),
        videos,
        kinds,
        k,
        text_only: false,
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

async fn tool_window(ctx: &ToolContext, args: &Value, kind: Kind) -> ToolResult {
    let id = video_arg(ctx, args)?;
    let v = video(ctx, id).await?;
    let (t0, t1) = range_args(args, v.duration.as_secs_f64())?;
    let w = ctx
        .storage
        .time_window(id, ts(t0), ts(t1), &[kind])
        .await
        .map_err(|e| e.to_string())?;
    let (lines, label): (Vec<String>, &str) = match kind {
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
    };
    let n = lines.len();
    let text = truncate(lines.join("\n"), MAX_TEXT_CHARS);
    Ok(ToolOutput {
        content: json!({
            "video_id": id.to_string(), "t0": t0, "t1": t1, "kind": label,
            "count": n,
            "text": if text.is_empty() { format!("(no {label} in this range)") } else { text },
        })
        .to_string(),
        summary: format!("{n} {label} lines in [{}, {}]", hms(t0), hms(t1)),
        ..ToolOutput::default()
    })
}

async fn grid_for(
    ctx: &ToolContext,
    v: &Video,
    t0: f64,
    t1: f64,
    fps: f64,
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

async fn transcript_text(ctx: &ToolContext, id: VideoId, t0: f64, t1: f64) -> String {
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
            3000,
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
    let (t0, t1) = range_args(args, v.duration.as_secs_f64())?;
    let fps = arg_f64(args, "fps").unwrap_or(1.0);
    let (view, key) = grid_for(ctx, &v, t0, t1, fps).await?;
    let transcript = transcript_text(ctx, id, t0, t1).await;
    Ok(ToolOutput {
        content: json!({
            "video_id": id.to_string(), "t0": t0, "t1": t1,
            "frames": view.timestamps.len(), "distinct_frames": view.distinct,
            "timestamps": view.timestamps.iter().map(|t| (t * 10.0).round() / 10.0).collect::<Vec<_>>(),
            "grid_blob": key.uri(),
            "transcript": transcript,
            "note": "The frame grid follows as an image; tiles are labelled HH:MM:SS.",
        })
        .to_string(),
        summary: format!("{} frames ({} distinct) in [{}, {}]", view.timestamps.len(), view.distinct, hms(t0), hms(t1)),
        image: Some(ImageData::Encoded {
            mime: "image/png",
            bytes: bytes::Bytes::from(view.png),
        }),
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
    let (view, key) = grid_for(ctx, &v, t0, t1, 1.0).await?;
    let transcript = transcript_text(ctx, id, t0, t1).await;
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
        image: None,
        cost_usd: stats.cost_usd,
        tokens_in: stats.usage.tokens_in,
        tokens_out: stats.usage.tokens_out,
        decoded: true,
    })
}
