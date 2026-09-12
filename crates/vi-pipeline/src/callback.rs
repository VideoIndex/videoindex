//! Support for operators implemented outside Rust (Python today, JS later):
//! items as JSON on the way out, rows as JSON on the way in. The bindings
//! only add the language-specific parts (frame pixels as arrays, calling
//! the user's function); everything that touches storage lives here so
//! each binding stores rows the same way the built-in operators do.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use vi_core::model::{
    BBox, Description, DescriptionKind, OcrSpan, Provenance, Segment, SegmentLevel, Span,
    TargetKind, TrackKind, TranscriptSpan,
};
use vi_core::{
    DescriptionId, FrameSampleId, Result, SegmentId, SpanId, Timestamp, TrackId, VideoId,
};

use crate::operator::{Item, ItemKind, OpContext};

/// Timebase for timestamps that arrive as float seconds.
const CALLBACK_TIMEBASE: u32 = 1_000_000;

/// An item as a plain JSON object, without pixels or audio samples (the
/// binding attaches those as native arrays). Every object has `kind`.
pub fn item_to_json(item: &Item) -> Value {
    let kind = serde_json::to_value(item.kind()).unwrap_or(Value::Null);
    let mut v = match item {
        Item::Media(m) => json!({
            "video": m.video,
            "tracks": m.tracks,
            "path": m.acquired.path,
            "duration_secs": m.probe.duration.as_secs_f64(),
            "expected_samples": m.expected_samples,
            "video_track_id": m.video_track().map(|t| t.id),
            "audio_track_id": m.audio_track().map(|t| t.id),
        }),
        Item::Frame(f) => json!({
            "sample": f.sample,
            "t": f.frame.t.as_secs_f64(),
            "width": f.frame.width,
            "height": f.frame.height,
        }),
        Item::Hashed {
            sample,
            phash,
            t,
            frame,
        } => json!({
            "sample_id": sample,
            "phash": phash,
            "t": t.as_secs_f64(),
            "width": frame.width,
            "height": frame.height,
        }),
        Item::Thumbnail { sample, blob } => json!({"sample_id": sample, "blob": blob}),
        Item::TranscriptSpan(s) => serde_json::to_value(s.as_ref()).unwrap_or(Value::Null),
        Item::SpeechRange(s) => json!({
            "t0": s.t0.as_secs_f64(),
            "t1": s.t1.as_secs_f64(),
            "sample_rate": s.sample_rate,
            "index": s.index,
        }),
        Item::Shot(s) | Item::Scene(s) | Item::Chapter(s) => {
            serde_json::to_value(s.as_ref()).unwrap_or(Value::Null)
        }
        Item::OcrSpan(s) => serde_json::to_value(s.as_ref()).unwrap_or(Value::Null),
        Item::Description(d) => serde_json::to_value(d.as_ref()).unwrap_or(Value::Null),
    };
    if let Value::Object(m) = &mut v {
        m.insert("kind".into(), kind);
        flatten_timestamps(&mut v);
    }
    v
}

/// `{"num": .., "den": ..}` objects become float seconds, recursively.
pub fn flatten_timestamps(v: &mut Value) {
    match v {
        Value::Object(m) => {
            if m.len() == 2 {
                if let (Some(num), Some(den)) = (
                    m.get("num").and_then(Value::as_i64),
                    m.get("den").and_then(Value::as_u64),
                ) {
                    if den > 0 {
                        *v = json!(num as f64 / den as f64);
                        return;
                    }
                }
            }
            for x in m.values_mut() {
                flatten_timestamps(x);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(flatten_timestamps),
        _ => {}
    }
}

/// A row a callback operator may produce. Ids and provenance are filled in
/// here; the callback supplies content and times in seconds.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CallbackRow {
    /// Timed speech text on the audio track.
    TranscriptSpan {
        t0: f64,
        t1: f64,
        text: String,
        #[serde(default)]
        speaker: Option<String>,
        #[serde(default)]
        language: Option<String>,
        #[serde(default)]
        confidence: Option<f32>,
        #[serde(default)]
        words: Option<Value>,
    },
    /// On-screen text on a frame sample.
    OcrSpan {
        frame_sample_id: String,
        t: f64,
        text: String,
        #[serde(default)]
        bbox: Option<BBox>,
        #[serde(default)]
        confidence: Option<f32>,
    },
    /// A shot segment.
    Shot(CallbackSegment),
    /// A scene segment.
    Scene(CallbackSegment),
    /// A chapter segment.
    Chapter(CallbackSegment),
    /// Text about a segment or a frame.
    Description {
        /// `segment` or `frame`.
        target_kind: String,
        target_id: String,
        text: String,
        /// `caption` (default), `summary`, `qa` or `structured`.
        #[serde(default)]
        description_kind: Option<String>,
        #[serde(default)]
        structured: Option<Value>,
    },
}

/// Segment fields a callback supplies.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CallbackSegment {
    pub t0: f64,
    pub t1: f64,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub keyframe_sample_id: Option<String>,
    #[serde(default)]
    pub parent_id: Option<String>,
}

impl CallbackRow {
    /// The item kind this row becomes.
    pub fn kind(&self) -> ItemKind {
        match self {
            CallbackRow::TranscriptSpan { .. } => ItemKind::TranscriptSpan,
            CallbackRow::OcrSpan { .. } => ItemKind::OcrSpan,
            CallbackRow::Shot(_) => ItemKind::Shot,
            CallbackRow::Scene(_) => ItemKind::Scene,
            CallbackRow::Chapter(_) => ItemKind::Chapter,
            CallbackRow::Description { .. } => ItemKind::Description,
        }
    }
}

/// Parse an item kind from its snake_case name (`"ocr_span"`).
pub fn parse_kind(name: &str) -> Option<ItemKind> {
    serde_json::from_value(Value::String(name.to_string())).ok()
}

fn ts(secs: f64) -> Timestamp {
    Timestamp::from_secs_f64(secs.max(0.0), CALLBACK_TIMEBASE)
}

fn parse_id<T>(
    s: &str,
    what: &str,
    ctx: &OpContext,
    parse: impl Fn(&str) -> Option<T>,
) -> Result<T> {
    parse(s).ok_or_else(|| ctx.err(format!("{what} '{s}' is not a valid id")))
}

/// Where a callback's rows go: the video and the track transcript spans
/// attach to. Built from storage so an operator need not consume the
/// media item to produce rows.
#[derive(Debug, Clone)]
pub struct RowTarget {
    /// The video being indexed.
    pub video: VideoId,
    /// Track for transcript spans: the audio track, else the video track.
    pub transcript_track: Option<TrackId>,
}

impl RowTarget {
    /// Look the video's tracks up.
    pub async fn load(ctx: &OpContext) -> Result<Self> {
        let tracks = ctx.storage.tracks(ctx.video).await?;
        let pick = |kind: TrackKind| {
            tracks
                .iter()
                .filter(|t| t.kind == kind)
                .min_by_key(|t| t.stream_index)
                .map(|t| t.id)
        };
        Ok(Self {
            video: ctx.video,
            transcript_track: pick(TrackKind::Audio).or_else(|| pick(TrackKind::Video)),
        })
    }
}

/// Store rows a callback produced and emit them downstream. Returns
/// `(emitted, stored)`. Rows whose kind the operator did not declare in
/// `outputs` are an error: the DAG was built from that declaration.
pub async fn store_rows(
    ctx: &OpContext,
    target: &RowTarget,
    prov: &Provenance,
    outputs: &[ItemKind],
    rows: Vec<CallbackRow>,
) -> Result<(u64, u64)> {
    let mut emitted = 0u64;
    let mut stored = 0u64;
    let mut spans: Vec<Span> = Vec::new();
    let mut segments: Vec<Segment> = Vec::new();
    let mut descriptions: Vec<Description> = Vec::new();
    let mut items: Vec<Item> = Vec::new();
    for row in rows {
        let kind = row.kind();
        if !outputs.contains(&kind) {
            return Err(ctx.err(format!(
                "operator emitted a {kind:?} row but declares outputs {outputs:?}"
            )));
        }
        match row {
            CallbackRow::TranscriptSpan {
                t0,
                t1,
                text,
                speaker,
                language,
                confidence,
                words,
            } => {
                let track = target
                    .transcript_track
                    .ok_or_else(|| ctx.err("video has no track for transcript spans"))?;
                let span = TranscriptSpan {
                    id: SpanId::new(),
                    track_id: track,
                    t0: ts(t0),
                    t1: ts(t1.max(t0)),
                    text,
                    speaker,
                    language,
                    confidence,
                    words,
                    provenance_id: prov.id,
                };
                items.push(Item::TranscriptSpan(Arc::new(span.clone())));
                spans.push(Span::Transcript(span));
            }
            CallbackRow::OcrSpan {
                frame_sample_id,
                t,
                text,
                bbox,
                confidence,
            } => {
                let fid = parse_id(&frame_sample_id, "frame_sample_id", ctx, |s| {
                    FrameSampleId::parse(s).ok()
                })?;
                let span = OcrSpan {
                    id: SpanId::new(),
                    frame_sample_id: fid,
                    t: ts(t),
                    text,
                    bbox,
                    confidence,
                    provenance_id: prov.id,
                };
                items.push(Item::OcrSpan(Arc::new(span.clone())));
                spans.push(Span::Ocr(span));
            }
            CallbackRow::Shot(s) | CallbackRow::Scene(s) | CallbackRow::Chapter(s) => {
                let level = match kind {
                    ItemKind::Shot => SegmentLevel::Shot,
                    ItemKind::Scene => SegmentLevel::Scene,
                    _ => SegmentLevel::Chapter,
                };
                let keyframe = match &s.keyframe_sample_id {
                    Some(k) => Some(parse_id(k, "keyframe_sample_id", ctx, |s| {
                        FrameSampleId::parse(s).ok()
                    })?),
                    None => None,
                };
                let parent = match &s.parent_id {
                    Some(p) => Some(parse_id(p, "parent_id", ctx, |s| SegmentId::parse(s).ok())?),
                    None => None,
                };
                let seg = Segment {
                    id: SegmentId::new(),
                    video_id: target.video,
                    level,
                    parent_id: parent,
                    t0: ts(s.t0),
                    t1: ts(s.t1.max(s.t0)),
                    keyframe_sample_id: keyframe,
                    title: s.title,
                    summary: s.summary,
                    provenance_id: prov.id,
                };
                let arc = Arc::new(seg.clone());
                items.push(match level {
                    SegmentLevel::Shot => Item::Shot(arc),
                    SegmentLevel::Scene => Item::Scene(arc),
                    SegmentLevel::Chapter => Item::Chapter(arc),
                });
                segments.push(seg);
            }
            CallbackRow::Description {
                target_kind,
                target_id,
                text,
                description_kind,
                structured,
            } => {
                let target_kind = match target_kind.as_str() {
                    "segment" => TargetKind::Segment,
                    "frame" => TargetKind::Frame,
                    other => {
                        return Err(ctx.err(format!(
                            "description target_kind must be 'segment' or 'frame', got '{other}'"
                        )))
                    }
                };
                let dkind = match description_kind.as_deref().unwrap_or("caption") {
                    "caption" => DescriptionKind::Caption,
                    "summary" => DescriptionKind::Summary,
                    "qa" => DescriptionKind::Qa,
                    "structured" => DescriptionKind::Structured,
                    other => return Err(ctx.err(format!("unknown description kind '{other}'"))),
                };
                let d = Description {
                    id: DescriptionId::new(),
                    target_kind,
                    target_id,
                    kind: dkind,
                    text,
                    structured,
                    provenance_id: prov.id,
                };
                items.push(Item::Description(Arc::new(d.clone())));
                descriptions.push(d);
            }
        }
    }
    if !spans.is_empty() {
        ctx.storage.put_spans(&spans).await?;
        stored += spans.len() as u64;
    }
    if !segments.is_empty() {
        ctx.storage.put_segments(&segments).await?;
        stored += segments.len() as u64;
    }
    if !descriptions.is_empty() {
        ctx.storage.put_descriptions(&descriptions).await?;
        stored += descriptions.len() as u64;
    }
    for item in items {
        ctx.emit(item).await?;
        emitted += 1;
    }
    Ok((emitted, stored))
}

/// A provenance row for a callback operator's stage.
pub fn provenance(id: &str, version: u32, params: Value) -> Provenance {
    Provenance {
        id: vi_core::ProvenanceId::new(),
        operator: id.to_string(),
        operator_version: version,
        provider: None,
        model: None,
        model_version: None,
        prompt_hash: None,
        params,
        created_at: chrono::Utc::now(),
        cost_usd: 0.0,
        tokens_in: 0,
        tokens_out: 0,
        latency_ms: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_parse_by_kind() {
        let rows: Vec<CallbackRow> = serde_json::from_value(json!([
            {"kind": "transcript_span", "t0": 1.0, "t1": 2.5, "text": "hi"},
            {"kind": "scene", "t0": 0.0, "t1": 30.0, "title": "Intro"},
            {"kind": "description", "target_kind": "frame", "target_id": "x", "text": "a slide"},
        ]))
        .unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].kind(), ItemKind::TranscriptSpan);
        assert_eq!(rows[1].kind(), ItemKind::Scene);
        assert_eq!(rows[2].kind(), ItemKind::Description);
        assert!(serde_json::from_value::<CallbackRow>(json!({"kind": "frame"})).is_err());
    }

    #[test]
    fn kinds_parse_from_snake_case() {
        assert_eq!(parse_kind("ocr_span"), Some(ItemKind::OcrSpan));
        assert_eq!(parse_kind("hashed"), Some(ItemKind::Hashed));
        assert_eq!(parse_kind("Frame"), None);
    }

    #[test]
    fn timestamps_flatten() {
        let mut v =
            json!({"t0": {"num": 1500, "den": 1000}, "nested": [{"t": {"num": 2, "den": 1}}]});
        flatten_timestamps(&mut v);
        assert_eq!(v, json!({"t0": 1.5, "nested": [{"t": 2.0}]}));
    }
}
