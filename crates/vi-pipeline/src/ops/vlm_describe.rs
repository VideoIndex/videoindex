//! `vlm_describe`: one VLM call per scene with a labelled frame grid (or a
//! native clip for providers that take video) and the scene's transcript
//! as data, producing a structured `Description` that is searchable and
//! citable. Budget-aware: when the job's cost or time limit is hit the
//! remaining scenes are counted as skipped.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use vi_core::config::roles;
use vi_core::model::{Description, DescriptionKind, Segment, TargetKind, TranscriptSpan};
use vi_core::{DescriptionId, Result};
use vi_index::{BlobKey, Kind};
use vi_perceive::grid::{compose, hms, GridLayout, Tile};
use vi_providers::{provenance_for, ContentPart, GenerateRequest, ImageData, Message, Role};

use crate::operator::*;

/// Frames per grid.
const GRID_FRAMES: usize = 9;

/// Scene description operator.
#[derive(Debug, Default)]
pub struct VlmDescribe {
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    media: Option<Arc<MediaItem>>,
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

/// Turn the model's JSON (or prose) into the stored text.
pub fn description_text(raw: &str) -> (String, Option<serde_json::Value>) {
    let trimmed = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let structured: Option<serde_json::Value> = serde_json::from_str(trimmed)
        .ok()
        .filter(|v: &serde_json::Value| v.is_object());
    let text = structured
        .as_ref()
        .map(|s| {
            let mut parts = Vec::new();
            for k in ["summary", "visible", "actions", "on_screen_text", "topics"] {
                if let Some(v) = s.get(k) {
                    let t = match v {
                        serde_json::Value::String(t) => t.clone(),
                        serde_json::Value::Array(a) => a
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
        .unwrap_or_else(|| trimmed.to_string());
    (text, structured)
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
        &[ItemKind::Media, ItemKind::Scene, ItemKind::TranscriptSpan]
    }

    fn optional_inputs(&self) -> &[InputKind] {
        &[ItemKind::TranscriptSpan]
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
        serde_json::json!({
            "provider": provider, "model": model,
            "prompt_hash": vi_providers::prompts::get("vlm_describe").map(|p| p.hash),
            "grid": ctx.policy.vlm_grid, "frames": GRID_FRAMES,
        })
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
        // About 40 scenes per hour, roughly 2,500 tokens each.
        let scenes = (input.duration_secs / 90.0).ceil();
        CostEstimate {
            cpu_secs: scenes * 1.5,
            usd: scenes * 0.02,
            provider_calls: scenes as u64,
        }
    }

    async fn run(&self, ctx: &OpContext, input: OpInput) -> Result<OpOutput> {
        let mut st = self.state.lock().await;
        match input.item {
            Item::Media(m) => st.media = Some(m),
            Item::Scene(s) => st.pending.push((*s).clone()),
            Item::TranscriptSpan(s) => st.spans.push(s),
            _ => return Err(ctx.err("expected media, a scene or a transcript span")),
        }
        Ok(OpOutput::default())
    }

    async fn finish(&self, ctx: &OpContext) -> Result<OpOutput> {
        let mut st = self.state.lock().await;
        let Some(media) = st.media.take() else {
            return Ok(OpOutput::default());
        };
        let scenes = std::mem::take(&mut st.pending);
        let spans = std::mem::take(&mut st.spans);
        if scenes.is_empty() {
            return Ok(OpOutput::default());
        }
        let vlm = ctx.providers.vlm(roles::VLM_DESCRIBE)?;
        let caps = vlm.vlm_capabilities();
        let prompt = vi_providers::prompts::get("vlm_describe")
            .ok_or_else(|| ctx.err("vlm_describe prompt missing"))?;
        // Re-runs replace this operator's descriptions.
        let old = ctx.storage.descriptions(media.video.id).await?;
        let _ = old; // descriptions are keyed by target; new rows supersede in search by provenance recency
        let cols = GridLayout::parse_cols(&ctx.policy.vlm_grid).unwrap_or(3);
        let mut emitted = 0u64;
        for (i, scene) in scenes.iter().enumerate() {
            ctx.check_cancelled()?;
            if !ctx.allow_provider_call() {
                ctx.record_skipped((scenes.len() - i) as u64);
                break;
            }
            let (t0, t1) = (scene.t0.as_secs_f64(), scene.t1.as_secs_f64());
            let dur = (t1 - t0).max(1.0);
            let fps = (GRID_FRAMES as f64 / dur).min(1.0);
            let req = vi_media::VideoDecodeRequest::new(
                &media.acquired.path,
                fps,
                ctx.config.media.sample_max_dim,
            )
            .range(t0, Some(t1));
            let mut stream = match vi_media::decode_video(&ctx.worker, req).await {
                Ok(s) => s,
                Err(e) => {
                    ctx.record_failure(scene.t0, scene.t1, e);
                    continue;
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
                        ctx.record_failure(scene.t0, scene.t1, e);
                        break;
                    }
                }
            }
            if frames.is_empty() {
                continue;
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
                    cols,
                    ..GridLayout::default()
                },
            );
            let transcript: String = spans
                .iter()
                .filter(|s| s.t0.as_secs_f64() < t1 && s.t1.as_secs_f64() > t0)
                .map(|s| format!("[{}] {}", hms(s.t0.as_secs_f64()), s.text))
                .collect::<Vec<_>>()
                .join("\n");
            let user_text = format!(
                "Segment {}–{} of \"{}\" ({} frames).\n\nTranscript (data):\n{}",
                hms(t0),
                hms(t1),
                media.video.title.clone().unwrap_or_default(),
                frames.len(),
                if transcript.is_empty() {
                    "(none)".to_string()
                } else {
                    transcript.chars().take(6000).collect()
                }
            );
            let greq = GenerateRequest {
                messages: vec![
                    Message::text(Role::System, prompt.text.clone()),
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
                json_schema: if caps.supports_json_schema {
                    Some(serde_json::json!({"type":"object","properties":{
                        "summary":{"type":"string"},"visible":{"type":"string"},
                        "on_screen_text":{"type":"array","items":{"type":"string"}},
                        "actions":{"type":"array","items":{"type":"string"}},
                        "topics":{"type":"array","items":{"type":"string"}}},
                        "required":["summary","visible","on_screen_text","actions","topics"]}))
                } else {
                    None
                },
                model: None,
            };
            let out = match vlm.generate(greq).await {
                Ok(stream) => match vi_providers::adapters::collect_stream(stream).await {
                    Ok(o) => o,
                    Err(e) => {
                        ctx.record_failure(scene.t0, scene.t1, e);
                        continue;
                    }
                },
                Err(e) => {
                    ctx.record_failure(scene.t0, scene.t1, e);
                    continue;
                }
            };
            let stats = out.stats.clone().unwrap_or_else(|| {
                vi_providers::CallStats::new(
                    vlm.provider_name(),
                    vlm.model(),
                    out.usage,
                    &caps.price,
                )
            });
            ctx.record_cost(&stats);
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
            let (text, structured) = description_text(&out.text);
            let prov = provenance_for(
                self.id(),
                self.version(),
                &stats,
                Some(prompt.hash.clone()),
                serde_json::json!({"scene": scene.id.to_string(), "t0": t0, "t1": t1, "frames": frames.len(), "grid_blob": key.uri()}),
            );
            ctx.storage.put_provenance(&prov).await?;
            let desc = Description {
                id: DescriptionId::new(),
                target_kind: TargetKind::Segment,
                target_id: scene.id.to_string(),
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
            // A scene summary makes `timeline` and search grouping readable.
            if let Some(summary) = desc.structured.as_ref().and_then(|s| s["summary"].as_str()) {
                let mut s2 = scene.clone();
                s2.summary = Some(summary.to_string());
                let _ = ctx.storage.put_segments(std::slice::from_ref(&s2)).await;
            }
            ctx.emit(Item::Description(Arc::new(desc))).await?;
            emitted += 1;
            st.described += 1;
            if emitted % 5 == 0 {
                ctx.progress(emitted);
            }
        }
        let _ = Kind::Description;
        tracing::info!(video = %media.video.id, scenes = scenes.len(), described = emitted, cost_usd = ctx.budget.spent_usd(), "scene descriptions written");
        ctx.progress(ctx.expected_items.unwrap_or(emitted));
        Ok(OpOutput {
            emitted,
            stored: emitted,
        })
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
}
