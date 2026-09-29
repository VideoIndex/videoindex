//! `vlm_describe`: one VLM call per scene (the default) or per shot
//! (`describe_level = "shot"`) with a labelled frame grid and the range's
//! transcript as data, producing a structured `Description` that is
//! searchable and citable. Budget-aware: when the job's cost or time limit
//! is hit the remaining scenes or shots are counted as skipped.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use serde_json::Value;
use tokio::sync::Mutex;
use vi_core::config::{describe_level, roles, IndexPolicy};
use vi_core::model::{
    Description, DescriptionKind, Segment, SegmentLevel, TargetKind, TranscriptSpan,
};
use vi_core::{DescriptionId, Result, SegmentId, VideoId};
use vi_index::{BlobKey, Kind};
use vi_perceive::grid::{compose, hms, Grid, GridLayout, Tile};
use vi_providers::prompts::Prompt;
use vi_providers::{
    provenance_for, CallStats, ContentPart, GenerateRequest, ImageData, Message, Role, Vlm,
    VlmCapabilities,
};

use crate::operator::*;

/// Frames per grid.
const GRID_FRAMES: usize = 9;

/// What one pass describes (`IndexPolicy::describe_level`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    Scene,
    Shot,
}

impl Level {
    fn of(policy: &IndexPolicy) -> Self {
        if policy.describes_shots() {
            Self::Shot
        } else {
            Self::Scene
        }
    }

    /// The policy value, also the provenance key naming the target.
    fn name(self) -> &'static str {
        match self {
            Self::Scene => describe_level::SCENE,
            Self::Shot => describe_level::SHOT,
        }
    }

    fn prompt(self) -> &'static str {
        match self {
            Self::Scene => "vlm_describe",
            Self::Shot => "vlm_describe_shot",
        }
    }

    /// How the user message names the range.
    fn label(self) -> &'static str {
        match self {
            Self::Scene => "Segment",
            Self::Shot => "Shot",
        }
    }

    /// Transcript characters sent with one range. A shot's schema is about
    /// what is visible, and one long take under a speaker should not cost
    /// more in transcript than in frames.
    fn transcript_chars(self) -> usize {
        match self {
            Self::Scene => 6000,
            Self::Shot => 2000,
        }
    }

    /// The response schema for providers that take one (no integer enums:
    /// Gemini rejects them).
    fn json_schema(self) -> Value {
        match self {
            Self::Scene => serde_json::json!({"type":"object","properties":{
                "summary":{"type":"string"},"visible":{"type":"string"},
                "on_screen_text":{"type":"array","items":{"type":"string"}},
                "actions":{"type":"array","items":{"type":"string"}},
                "topics":{"type":"array","items":{"type":"string"}}},
                "required":["summary","visible","on_screen_text","actions","topics"]}),
            Self::Shot => serde_json::json!({"type":"object","properties":{
                "people":{"type":"array","items":{"type":"string"}},
                "objects":{"type":"array","items":{"type":"object","properties":{
                    "name":{"type":"string"},"count":{"type":"integer"}},
                    "required":["name","count"]}},
                "actions":{"type":"array","items":{"type":"string"}},
                "on_screen_text":{"type":"array","items":{"type":"string"}},
                "summary":{"type":"string"}},
                "required":["people","objects","actions","on_screen_text","summary"]}),
        }
    }
}

/// Scene (or shot) description operator.
#[derive(Debug, Default)]
pub struct VlmDescribe {
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    media: Option<Arc<MediaItem>>,
    /// Scenes or shots, whichever the policy's `describe_level` names.
    pending: Vec<Segment>,
    spans: Vec<Arc<TranscriptSpan>>,
    described: u64,
}

impl VlmDescribe {
    /// New operator.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Turn the model's JSON (or prose) into the stored, searchable text. A
/// scene description flattens summary, visible, actions, on-screen text
/// and topics; a shot description (it has `people` or `objects`) reads as
/// sentences: summary, people, objects with their counts ("2 chairs"),
/// actions, on-screen text, so `search` and `find_mentions` match on each.
pub fn description_text(raw: &str) -> (String, Option<Value>) {
    let trimmed = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let structured: Option<Value> = serde_json::from_str(trimmed)
        .ok()
        .filter(|v: &Value| v.is_object());
    let text = structured
        .as_ref()
        .map(|s| {
            if s.get("people").is_some() || s.get("objects").is_some() {
                shot_text(s)
            } else {
                scene_text(s)
            }
        })
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| trimmed.to_string());
    (text, structured)
}

/// A string as is, a list joined with `sep`, anything else as JSON.
fn flatten(v: &Value, sep: &str) -> String {
    match v {
        Value::String(t) => t.trim().to_string(),
        Value::Array(a) => a
            .iter()
            .map(|x| {
                x.as_str()
                    .map(|t| t.trim().to_string())
                    .unwrap_or_else(|| x.to_string())
            })
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(sep),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn scene_text(s: &Value) -> String {
    let mut parts = Vec::new();
    for k in ["summary", "visible", "actions", "on_screen_text", "topics"] {
        if let Some(v) = s.get(k) {
            let t = flatten(v, "; ");
            if !t.is_empty() {
                parts.push(t);
            }
        }
    }
    parts.join(" ")
}

/// `{"name": "chairs", "count": 2}` as "2 chairs"; a bare string as is; a
/// count of zero drops the entry.
fn object_phrase(o: &Value) -> Option<String> {
    match o {
        Value::String(t) => Some(t.trim().to_string()).filter(|t| !t.is_empty()),
        Value::Object(m) => {
            let name = m
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|n| !n.is_empty())?;
            let count = m.get("count").and_then(|c| {
                c.as_u64()
                    .or_else(|| c.as_f64().map(|f| f.max(0.0).round() as u64))
                    .or_else(|| c.as_str().and_then(|t| t.trim().parse().ok()))
            });
            match count {
                Some(0) => None,
                Some(n) => Some(format!("{n} {name}")),
                None => Some(name.to_string()),
            }
        }
        _ => None,
    }
}

fn shot_text(s: &Value) -> String {
    let field = |k: &str, sep: &str| s.get(k).map(|v| flatten(v, sep)).unwrap_or_default();
    let objects = s
        .get("objects")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(object_phrase)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let parts = [
        field("summary", " "),
        field("people", "; "),
        objects,
        field("actions", "; "),
        field("on_screen_text", "; "),
    ];
    // One sentence per field.
    let mut out = String::new();
    for p in parts.iter().map(|p| p.trim()).filter(|p| !p.is_empty()) {
        if !out.is_empty() {
            if !out.ends_with(['.', '!', '?']) {
                out.push('.');
            }
            out.push(' ');
        }
        out.push_str(p);
    }
    out
}

/// Descriptions of this video's segments that this operator already wrote
/// with the same prompt, provider and model, by target id. A shot pass the
/// budget stopped leaves no cache marker, so the next pass runs the stage
/// again: the shots described then are re-emitted from these instead of
/// being paid for twice and stored twice (a duplicate would count twice in
/// `find_mentions`).
async fn already_described(
    ctx: &OpContext,
    video: VideoId,
    prompt_hash: &str,
    vlm: &dyn Vlm,
) -> Result<HashMap<String, Description>> {
    let mut out = HashMap::new();
    for d in ctx.storage.descriptions(video).await? {
        if d.target_kind != TargetKind::Segment || out.contains_key(&d.target_id) {
            continue;
        }
        let same = ctx
            .storage
            .get_provenance(d.provenance_id)
            .await?
            .is_some_and(|p| {
                p.operator == "vlm_describe"
                    && p.prompt_hash.as_deref() == Some(prompt_hash)
                    && p.provider.as_deref() == Some(vlm.provider_name())
                    && p.model.as_deref() == Some(vlm.model())
            });
        if same {
            out.insert(d.target_id.clone(), d);
        }
    }
    Ok(out)
}

/// What describing one scene or shot came to.
enum Outcome {
    /// The budget was spent before its call; counted as skipped.
    Skipped,
    /// No frames, or decoding or the call failed (failures are recorded).
    Nothing,
    /// The model's answer and the grid it was shown.
    Described(Box<Described>),
}

/// One call's result.
struct Described {
    grid: Grid,
    frames: usize,
    text: String,
    stats: CallStats,
}

/// What every call of one pass shares.
struct Call<'a> {
    ctx: &'a OpContext,
    media: &'a MediaItem,
    spans: &'a [Arc<TranscriptSpan>],
    level: Level,
    prompt: &'a Prompt,
    vlm: &'a dyn Vlm,
    caps: &'a VlmCapabilities,
    cols: u32,
}

impl Call<'_> {
    /// Decode the range's grid and ask the model about it. The budget is
    /// checked before the frames are decoded and again before the call;
    /// the cost is recorded as soon as the call returns, so calls started
    /// later see it.
    async fn describe(&self, seg: &Segment) -> Result<Outcome> {
        let ctx = self.ctx;
        ctx.check_cancelled()?;
        if !ctx.allow_provider_call() {
            return Ok(Outcome::Skipped);
        }
        let (t0, t1) = (seg.t0.as_secs_f64(), seg.t1.as_secs_f64());
        let dur = (t1 - t0).max(1.0);
        let fps = (GRID_FRAMES as f64 / dur).min(1.0);
        // At most 1 fps, so a range shorter than a second still yields its
        // first frame (and only that one).
        let req = vi_media::VideoDecodeRequest::new(
            &self.media.acquired.path,
            fps,
            ctx.config.media.sample_max_dim,
        )
        .range(t0, Some(t1.max(t0 + 1.0)));
        let mut stream = match vi_media::decode_video(&ctx.worker, req).await {
            Ok(s) => s,
            Err(e) => {
                ctx.record_failure(seg.t0, seg.t1, e);
                return Ok(Outcome::Nothing);
            }
        };
        let mut frames = Vec::new();
        loop {
            match stream.next().await {
                Ok(Some(f)) => {
                    frames.push(f.to_owned_frame());
                    if frames.len() >= GRID_FRAMES {
                        break;
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    ctx.record_failure(seg.t0, seg.t1, e);
                    break;
                }
            }
        }
        drop(stream);
        if frames.is_empty() {
            return Ok(Outcome::Nothing);
        }
        let tiles: Vec<Tile<'_>> = frames
            .iter()
            .map(|f| Tile {
                frame: f,
                label: hms(f.t.as_secs_f64()),
            })
            .collect();
        let grid = compose(
            &tiles,
            GridLayout {
                cols: self.cols,
                ..GridLayout::default()
            },
        );
        let transcript: String = self
            .spans
            .iter()
            .filter(|s| s.t0.as_secs_f64() < t1 && s.t1.as_secs_f64() > t0)
            .map(|s| format!("[{}] {}", hms(s.t0.as_secs_f64()), s.text))
            .collect::<Vec<_>>()
            .join("\n");
        let user_text = format!(
            "{} {}–{} of \"{}\" ({} frames).\n\nTranscript (data):\n{}",
            self.level.label(),
            hms(t0),
            hms(t1),
            self.media.video.title.clone().unwrap_or_default(),
            frames.len(),
            if transcript.is_empty() {
                "(none)".to_string()
            } else {
                transcript
                    .chars()
                    .take(self.level.transcript_chars())
                    .collect()
            }
        );
        let greq = GenerateRequest {
            messages: vec![
                Message::text(Role::System, self.prompt.text.clone()),
                Message {
                    role: Role::User,
                    parts: vec![
                        ContentPart::Text(user_text),
                        ContentPart::Image(ImageData::Rgb8 {
                            width: grid.width,
                            height: grid.height,
                            data: Arc::from(grid.rgb.clone()),
                        }),
                    ],
                },
            ],
            tools: vec![],
            tool_choice: Default::default(),
            max_tokens: 1000,
            temperature: 0.0,
            json_schema: self
                .caps
                .supports_json_schema
                .then(|| self.level.json_schema()),
            model: None,
        };
        // The budget may have closed while the frames were decoded.
        if !ctx.allow_provider_call() {
            return Ok(Outcome::Skipped);
        }
        let out = match self.vlm.generate(greq).await {
            Ok(stream) => match vi_providers::adapters::collect_stream(stream).await {
                Ok(o) => o,
                Err(e) => {
                    ctx.record_failure(seg.t0, seg.t1, e);
                    return Ok(Outcome::Nothing);
                }
            },
            Err(e) => {
                ctx.record_failure(seg.t0, seg.t1, e);
                return Ok(Outcome::Nothing);
            }
        };
        let stats = out.stats.clone().unwrap_or_else(|| {
            CallStats::new(
                self.vlm.provider_name(),
                self.vlm.model(),
                out.usage,
                &self.caps.price,
            )
        });
        ctx.record_cost(&stats);
        Ok(Outcome::Described(Box::new(Described {
            grid,
            frames: frames.len(),
            text: out.text,
            stats,
        })))
    }
}

#[async_trait]
impl Operator for VlmDescribe {
    fn id(&self) -> &'static str {
        "vlm_describe"
    }

    fn version(&self) -> u32 {
        1
    }

    fn inputs(&self) -> &[InputKind] {
        &[
            ItemKind::Media,
            ItemKind::Scene,
            ItemKind::Shot,
            ItemKind::TranscriptSpan,
        ]
    }

    /// Scenes or shots, by `describe_level`: the DAG cannot see the policy,
    /// so both are optional and `IndexPolicy::validate` checks at plan time
    /// that the level's producer (`scenes` or `shot_boundary`) is in the
    /// policy. A policy with `fine = ["vlm_describe"]` and shot level runs
    /// over shots replayed from the coarse pass.
    fn optional_inputs(&self) -> &[InputKind] {
        &[ItemKind::Scene, ItemKind::Shot, ItemKind::TranscriptSpan]
    }

    fn outputs(&self) -> &[OutputKind] {
        &[ItemKind::Description]
    }

    fn required_roles(&self) -> &[&'static str] {
        &[roles::VLM_DESCRIBE]
    }

    fn cache_params(&self, ctx: &OpContext) -> serde_json::Value {
        let (provider, model) = ctx
            .providers
            .vlm(roles::VLM_DESCRIBE)
            .map(|v| (v.provider_name().to_string(), v.model().to_string()))
            .unwrap_or_default();
        let level = Level::of(&ctx.policy);
        let mut params = serde_json::json!({
            "provider": provider, "model": model,
            "prompt_hash": vi_providers::prompts::get(level.prompt()).map(|p| p.hash),
            "grid": ctx.policy.vlm_grid, "frames": GRID_FRAMES,
        });
        // The level joins the key only when it is not the default, so the
        // markers of scene passes written before it existed still hold and
        // a shot pass is never taken for a cached scene pass.
        if level == Level::Shot {
            params["level"] = Value::from(level.name());
        }
        params
    }

    fn replay_supported(&self) -> bool {
        true
    }

    async fn replay(&self, ctx: &OpContext) -> Result<Option<u64>> {
        let _ = self.state.lock().await.media.take();
        let descs = ctx.storage.descriptions(ctx.video).await?;
        let mut n = 0;
        for d in descs
            .into_iter()
            .filter(|d| d.target_kind == TargetKind::Segment)
        {
            ctx.emit(Item::Description(Arc::new(d))).await?;
            n += 1;
        }
        Ok(Some(n))
    }

    fn cost_estimate(&self, input: &InputSummary) -> CostEstimate {
        // About 40 scenes per hour, roughly 2,500 tokens each. With
        // `describe_level = "shot"` it is one call per shot instead: about
        // 2.5 shots a minute (150 an hour; MINERVA's median, LVBench's is
        // about 250), roughly 800 tokens each. `InputSummary` carries only
        // facts about the media, not the policy, so the estimate cannot
        // tell the levels apart (and nothing calls it yet); the job budget
        // (`max_cost_usd_per_hour`) is what bounds a shot pass.
        let scenes = (input.duration_secs / 90.0).ceil();
        CostEstimate {
            cpu_secs: scenes * 1.5,
            usd: scenes * 0.02,
            provider_calls: scenes as u64,
        }
    }

    async fn run(&self, ctx: &OpContext, input: OpInput) -> Result<OpOutput> {
        let level = Level::of(&ctx.policy);
        let mut st = self.state.lock().await;
        match input.item {
            Item::Media(m) => st.media = Some(m),
            // Only the policy's level is kept; the other arrives when its
            // producer is in the policy too.
            Item::Scene(s) if level == Level::Scene => st.pending.push((*s).clone()),
            Item::Shot(s) if level == Level::Shot => st.pending.push((*s).clone()),
            Item::Scene(_) | Item::Shot(_) => {}
            Item::TranscriptSpan(s) => st.spans.push(s),
            _ => return Err(ctx.err("expected media, a scene, a shot or a transcript span")),
        }
        Ok(OpOutput::default())
    }

    async fn finish(&self, ctx: &OpContext) -> Result<OpOutput> {
        let level = Level::of(&ctx.policy);
        let mut st = self.state.lock().await;
        let Some(media) = st.media.take() else {
            return Ok(OpOutput::default());
        };
        let segments = std::mem::take(&mut st.pending);
        let spans = std::mem::take(&mut st.spans);
        if segments.is_empty() {
            return Ok(OpOutput::default());
        }
        let vlm = ctx.providers.vlm(roles::VLM_DESCRIBE)?;
        let caps = vlm.vlm_capabilities();
        let prompt = vi_providers::prompts::get(level.prompt())
            .ok_or_else(|| ctx.err(format!("{} prompt missing", level.prompt())))?;
        let mut emitted = 0u64;
        let mut stored = 0u64;
        // A scene re-run adds rows next to the old ones, as it always has;
        // a shot pass re-emits the shots already described with this
        // prompt and model and describes only the rest.
        let (todo, reused): (Vec<Segment>, u64) = if level == Level::Shot {
            let mut done =
                already_described(ctx, media.video.id, &prompt.hash, vlm.as_ref()).await?;
            let mut todo = Vec::with_capacity(segments.len());
            let mut reused = 0u64;
            for seg in segments {
                match done.remove(&seg.id.to_string()) {
                    Some(d) => {
                        ctx.emit(Item::Description(Arc::new(d))).await?;
                        reused += 1;
                    }
                    None => todo.push(seg),
                }
            }
            (todo, reused)
        } else {
            (segments, 0)
        };
        emitted += reused;
        // Shot rows as stored now: `scenes` may have set their parent since
        // `shot_boundary` emitted them, and the summary update must keep it.
        let mut stored_shots: HashMap<SegmentId, Segment> = if level == Level::Shot {
            ctx.storage
                .segments(media.video.id, SegmentLevel::Shot)
                .await?
                .into_iter()
                .map(|s| (s.id, s))
                .collect()
        } else {
            HashMap::new()
        };
        let call = Call {
            ctx,
            media: &media,
            spans: &spans,
            level,
            prompt: &prompt,
            vlm: vlm.as_ref(),
            caps: &caps,
            cols: GridLayout::parse_cols(&ctx.policy.vlm_grid).unwrap_or(3),
        };
        // Shots are described as many at a time as the provider takes
        // calls at once (its `concurrency`, default 4 as in the governor,
        // which still enforces it and the rate limits); scenes one at a
        // time. Results are written in order. Once the budget is spent no
        // call starts; calls already in flight finish and are kept.
        let in_flight = match level {
            Level::Scene => 1,
            Level::Shot => ctx
                .config
                .provider_for_role(roles::VLM_DESCRIBE)
                .ok()
                .and_then(|(_, p)| p.concurrency)
                .unwrap_or(4)
                .max(1) as usize,
        };
        let call = &call;
        // Boxed so the stream's type names no closure: the operator's
        // future must be provably `Send`.
        type Pending<'a> = std::pin::Pin<
            Box<dyn std::future::Future<Output = (&'a Segment, Result<Outcome>)> + Send + 'a>,
        >;
        let pending: Vec<Pending<'_>> = todo
            .iter()
            .map(|seg| Box::pin(async move { (seg, call.describe(seg).await) }) as Pending<'_>)
            .collect();
        let mut results = futures::stream::iter(pending).buffered(in_flight);
        let mut skipped = 0u64;
        while let Some((seg, outcome)) = results.next().await {
            let Described {
                grid,
                frames,
                text: raw,
                stats,
            } = match outcome? {
                Outcome::Skipped => {
                    skipped += 1;
                    continue;
                }
                Outcome::Nothing => continue,
                Outcome::Described(d) => *d,
            };
            let (t0, t1) = (seg.t0.as_secs_f64(), seg.t1.as_secs_f64());
            // Keep the grid so the description can be shown with its frames.
            let png = {
                let img = image::RgbImage::from_raw(grid.width, grid.height, grid.rgb)
                    .ok_or_else(|| ctx.err("grid buffer mismatch"))?;
                let mut buf = std::io::Cursor::new(Vec::new());
                let _ = img.write_to(&mut buf, image::ImageFormat::Png);
                buf.into_inner()
            };
            let key = BlobKey::for_bytes(&png);
            ctx.storage.put_blob(&key, bytes::Bytes::from(png)).await?;
            let (text, structured) = description_text(&raw);
            let mut params =
                serde_json::json!({"t0": t0, "t1": t1, "frames": frames, "grid_blob": key.uri()});
            params[level.name()] = Value::from(seg.id.to_string());
            let prov = provenance_for(
                self.id(),
                self.version(),
                &stats,
                Some(prompt.hash.clone()),
                params,
            );
            ctx.storage.put_provenance(&prov).await?;
            let desc = Description {
                id: DescriptionId::new(),
                target_kind: TargetKind::Segment,
                target_id: seg.id.to_string(),
                kind: if structured.is_some() {
                    DescriptionKind::Structured
                } else {
                    DescriptionKind::Caption
                },
                text,
                structured,
                provenance_id: prov.id,
            };
            ctx.storage
                .put_descriptions(std::slice::from_ref(&desc))
                .await?;
            // A summary on the scene or shot makes `timeline` and search
            // grouping readable.
            if let Some(summary) = desc.structured.as_ref().and_then(|s| s["summary"].as_str()) {
                let row = match level {
                    Level::Scene => Some(seg.clone()),
                    Level::Shot => stored_shots.remove(&seg.id),
                };
                if let Some(mut row) = row {
                    row.summary = Some(summary.to_string());
                    let _ = ctx.storage.put_segments(std::slice::from_ref(&row)).await;
                }
            }
            ctx.emit(Item::Description(Arc::new(desc))).await?;
            emitted += 1;
            stored += 1;
            st.described += 1;
            if emitted % 5 == 0 {
                ctx.progress(emitted);
            }
        }
        drop(results);
        if skipped > 0 {
            // One per scene or shot not described.
            ctx.record_skipped(skipped);
        }
        let _ = Kind::Description;
        tracing::info!(
            video = %media.video.id,
            level = level.name(),
            segments = todo.len() as u64 + reused,
            described = stored,
            reused,
            cost_usd = ctx.budget.spent_usd(),
            "descriptions written"
        );
        ctx.progress(ctx.expected_items.unwrap_or(emitted));
        Ok(OpOutput { emitted, stored })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn description_text_flattens_json_and_keeps_prose() {
        let (t, s) = description_text("```json\n{\"summary\":\"A talk.\",\"visible\":\"Two speakers.\",\"on_screen_text\":[\"Evals 101\"],\"actions\":[],\"topics\":[\"evals\"]}\n```");
        assert!(s.is_some());
        assert_eq!(t, "A talk. Two speakers. Evals 101 evals");
        let (t, s) = description_text("Just prose.");
        assert!(s.is_none());
        assert_eq!(t, "Just prose.");
    }

    #[test]
    fn description_text_renders_the_shot_schema_with_counts() {
        let raw = r#"{"people":["3 people: a host, two guests"],
            "objects":[{"name":"chairs","count":2},{"name":"laptop","count":1},
                       {"name":"cups","count":"4"},{"name":"ghosts","count":0},{"name":"plant"}],
            "actions":["host waves","guest sits down"],
            "on_screen_text":["LIVE","Episode 12"],
            "summary":"A host greets two guests at a desk."}"#;
        let (t, s) = description_text(raw);
        assert!(s.is_some());
        assert_eq!(
            t,
            "A host greets two guests at a desk. 3 people: a host, two guests. \
             2 chairs, 1 laptop, 4 cups, plant. host waves; guest sits down. LIVE; Episode 12"
        );
        // Empty lists drop out; nobody visible still renders the rest.
        let (t, _) = description_text(
            r#"{"people":[],"objects":[],"actions":[],"on_screen_text":["EXIT"],"summary":"An empty corridor."}"#,
        );
        assert_eq!(t, "An empty corridor. EXIT");
    }

    #[test]
    fn shot_schema_has_the_fixed_fields_and_no_enums() {
        let schema = Level::Shot.json_schema();
        let required: Vec<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(
            required,
            ["people", "objects", "actions", "on_screen_text", "summary"]
        );
        assert_eq!(
            schema["properties"]["objects"]["items"]["properties"]["count"]["type"],
            "integer"
        );
        assert!(!schema.to_string().contains("enum"));
        assert_eq!(Level::Scene.prompt(), "vlm_describe");
        assert_eq!(Level::Shot.prompt(), "vlm_describe_shot");
    }
}
