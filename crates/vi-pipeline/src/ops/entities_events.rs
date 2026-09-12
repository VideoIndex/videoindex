//! `entities_events`: an LLM reads scene descriptions plus the transcript in
//! sliding windows and emits Entities (with mentions) and Events. Entities
//! are canonicalised across the video by lower-cased name.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use vi_core::config::roles;
use vi_core::model::{
    Description, Entity, EntityKind, EntityMention, Event as VideoEvent, TranscriptSpan,
};
use vi_core::{EntityId, EventId, Result, Timestamp};
use vi_perceive::grid::hms;
use vi_providers::{provenance_for, GenerateRequest, Message, Role};

use crate::operator::*;

/// Window length, seconds.
pub const WINDOW_SECS: f64 = 300.0;

/// Extraction operator.
#[derive(Debug, Default)]
pub struct EntitiesEvents {
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    media: Option<Arc<MediaItem>>,
    descriptions: Vec<(f64, f64, String)>,
    spans: Vec<Arc<TranscriptSpan>>,
}

impl EntitiesEvents {
    /// New operator.
    pub fn new() -> Self {
        Self::default()
    }
}

/// The first JSON object in a model reply, tolerating prose and code
/// fences around it.
pub fn extract_json(text: &str) -> Option<serde_json::Value> {
    let t = text.trim();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(t) {
        if v.is_object() {
            return Some(v);
        }
    }
    let start = t.find('{')?;
    let end = t.rfind('}')?;
    if end <= start {
        return None;
    }
    let slice = &t[start..=end];
    if let Some(v) = serde_json::from_str::<serde_json::Value>(slice)
        .ok()
        .filter(|v| v.is_object())
    {
        return Some(v);
    }
    // Models occasionally leave a trailing comma before a closing bracket
    // or brace; strip those (outside strings) and try once more.
    let repaired = strip_trailing_commas(slice);
    serde_json::from_str(&repaired)
        .ok()
        .filter(|v: &serde_json::Value| v.is_object())
}

fn strip_trailing_commas(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_str = false;
    let mut escaped = false;
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if in_str {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        if c == '"' {
            in_str = true;
            out.push(c);
        } else if c == ',' {
            // Drop the comma when the next non-space char closes a container.
            let next = chars[i + 1..].iter().find(|x| !x.is_whitespace());
            if !matches!(next, Some('}') | Some(']')) {
                out.push(c);
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn canonical(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn kind_of(s: &str) -> EntityKind {
    match s {
        "person" => EntityKind::Person,
        "object" => EntityKind::Object,
        "text" => EntityKind::Text,
        "place" => EntityKind::Place,
        _ => EntityKind::Concept,
    }
}

fn ts(secs: f64) -> Timestamp {
    Timestamp::from_secs_f64(secs.max(0.0), 1000)
}

#[async_trait]
impl Operator for EntitiesEvents {
    fn id(&self) -> &'static str {
        "entities_events"
    }

    fn version(&self) -> u32 {
        1
    }

    fn inputs(&self) -> &[InputKind] {
        &[
            ItemKind::Media,
            ItemKind::Description,
            ItemKind::TranscriptSpan,
        ]
    }

    fn optional_inputs(&self) -> &[InputKind] {
        &[ItemKind::Description, ItemKind::TranscriptSpan]
    }

    fn outputs(&self) -> &[OutputKind] {
        &[ItemKind::Extraction]
    }

    fn required_roles(&self) -> &[&'static str] {
        &[roles::EXTRACT_LLM]
    }

    fn cache_params(&self, ctx: &OpContext) -> serde_json::Value {
        let (provider, model) = ctx
            .providers
            .llm(roles::EXTRACT_LLM)
            .map(|v| (v.provider_name().to_string(), v.model().to_string()))
            .unwrap_or_default();
        serde_json::json!({
            "provider": provider, "model": model,
            "prompt_hash": vi_providers::prompts::get("entities_events").map(|p| p.hash),
            "window_secs": WINDOW_SECS,
        })
    }

    fn cost_estimate(&self, input: &InputSummary) -> CostEstimate {
        let windows = (input.duration_secs / WINDOW_SECS).ceil();
        CostEstimate {
            cpu_secs: windows * 0.1,
            usd: windows * 0.03,
            provider_calls: windows as u64,
        }
    }

    async fn run(&self, ctx: &OpContext, input: OpInput) -> Result<OpOutput> {
        let mut st = self.state.lock().await;
        match input.item {
            Item::Media(m) => st.media = Some(m),
            Item::Description(d) => {
                let d: &Description = &d;
                // Time comes from the segment; look it up lazily at finish.
                st.descriptions
                    .push((-1.0, -1.0, format!("{}|{}", d.target_id, d.text)));
            }
            Item::TranscriptSpan(s) => st.spans.push(s),
            _ => return Err(ctx.err("expected media, a description or a transcript span")),
        }
        Ok(OpOutput::default())
    }

    async fn finish(&self, ctx: &OpContext) -> Result<OpOutput> {
        let mut st = self.state.lock().await;
        let Some(media) = st.media.take() else {
            return Ok(OpOutput::default());
        };
        let video = media.video.id;
        let llm = ctx.providers.llm(roles::EXTRACT_LLM)?;
        let prompt = vi_providers::prompts::get("entities_events")
            .ok_or_else(|| ctx.err("entities_events prompt missing"))?;
        // Resolve description times through their scene segments.
        let scenes = ctx
            .storage
            .segments(video, vi_core::model::SegmentLevel::Scene)
            .await?;
        let by_id: BTreeMap<String, (f64, f64)> = scenes
            .iter()
            .map(|s| (s.id.to_string(), (s.t0.as_secs_f64(), s.t1.as_secs_f64())))
            .collect();
        let mut lines: Vec<(f64, String)> = Vec::new();
        for (_, _, packed) in std::mem::take(&mut st.descriptions) {
            if let Some((target, text)) = packed.split_once('|') {
                if let Some((a, _)) = by_id.get(target) {
                    lines.push((
                        *a,
                        format!(
                            "[{}] (scene) {}",
                            hms(*a),
                            text.chars().take(1200).collect::<String>()
                        ),
                    ));
                }
            }
        }
        for s in std::mem::take(&mut st.spans) {
            lines.push((
                s.t0.as_secs_f64(),
                format!("[{}] {}", hms(s.t0.as_secs_f64()), s.text),
            ));
        }
        if lines.is_empty() {
            return Ok(OpOutput::default());
        }
        lines.sort_by(|a, b| a.0.total_cmp(&b.0));
        ctx.storage.delete_extractions(video).await?;
        let total = media.video.duration.as_secs_f64();
        let mut entities: BTreeMap<String, Entity> = BTreeMap::new();
        let mut mentions: Vec<EntityMention> = Vec::new();
        let mut events: Vec<VideoEvent> = Vec::new();
        let mut windows = 0u64;
        let mut ok_windows = 0u64;
        let mut w0 = 0.0;
        while w0 < total.max(1.0) {
            let w1 = (w0 + WINDOW_SECS).min(total.max(w0 + 1.0));
            let window: Vec<&str> = lines
                .iter()
                .filter(|(t, _)| *t >= w0 && *t < w1)
                .map(|(_, l)| l.as_str())
                .collect();
            if window.is_empty() {
                w0 = w1;
                continue;
            }
            if !ctx.allow_provider_call() {
                ctx.record_skipped(((total - w0) / WINDOW_SECS).ceil() as u64);
                break;
            }
            let text: String = window.join("\n").chars().take(24_000).collect();
            let req = GenerateRequest {
                messages: vec![
                    Message::text(Role::System, prompt.text.clone()),
                    Message::text(
                        Role::User,
                        format!(
                            "Video: \"{}\". Window {}–{}.\n\n{text}",
                            media.video.title.clone().unwrap_or_default(),
                            hms(w0),
                            hms(w1)
                        ),
                    ),
                ],
                tools: vec![],
                tool_choice: Default::default(),
                max_tokens: 4000,
                temperature: 0.0,
                json_schema: None,
                model: None,
            };
            let out = match llm.generate(req).await {
                Ok(s) => match vi_providers::adapters::collect_stream(s).await {
                    Ok(o) => o,
                    Err(e) => {
                        ctx.record_failure(ts(w0), ts(w1), e);
                        w0 = w1;
                        continue;
                    }
                },
                Err(e) => {
                    ctx.record_failure(ts(w0), ts(w1), e);
                    w0 = w1;
                    continue;
                }
            };
            windows += 1;
            let stats = out.stats.clone().unwrap_or_else(|| {
                vi_providers::CallStats::new(
                    llm.provider_name(),
                    llm.model(),
                    out.usage,
                    &llm.llm_capabilities().price,
                )
            });
            ctx.record_cost(&stats);
            let prov = provenance_for(
                self.id(),
                self.version(),
                &stats,
                Some(prompt.hash.clone()),
                serde_json::json!({"t0": w0, "t1": w1, "lines": window.len()}),
            );
            ctx.storage.put_provenance(&prov).await?;
            let Some(v) = extract_json(&out.text) else {
                ctx.record_failure(
                    ts(w0),
                    ts(w1),
                    format!(
                        "extraction was not JSON ({}): {}",
                        out.finish_reason,
                        out.text.chars().take(120).collect::<String>()
                    ),
                );
                w0 = w1;
                continue;
            };
            ok_windows += 1;
            for e in v["entities"].as_array().into_iter().flatten() {
                let Some(name) = e["name"].as_str().filter(|n| !n.trim().is_empty()) else {
                    continue;
                };
                let key = canonical(name);
                let ent = entities.entry(key.clone()).or_insert_with(|| Entity {
                    id: EntityId::new(),
                    video_id: video,
                    kind: kind_of(e["kind"].as_str().unwrap_or("concept")),
                    name: name.trim().to_string(),
                    canonical_name: key.clone(),
                    attributes: serde_json::json!({}),
                });
                for m in e["mentions"].as_array().into_iter().flatten() {
                    let (Some(a), Some(b)) = (m["t0"].as_f64(), m["t1"].as_f64()) else {
                        continue;
                    };
                    mentions.push(EntityMention {
                        entity_id: ent.id,
                        t0: ts(a),
                        t1: ts(b.max(a)),
                        source_kind: "extraction".into(),
                        source_id: prov.id.to_string(),
                        confidence: None,
                    });
                }
            }
            for ev in v["events"].as_array().into_iter().flatten() {
                let (Some(a), Some(b), Some(text)) =
                    (ev["t0"].as_f64(), ev["t1"].as_f64(), ev["text"].as_str())
                else {
                    continue;
                };
                let participants: Vec<EntityId> = ev["participants"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|p| p.as_str())
                    .filter_map(|p| entities.get(&canonical(p)).map(|e| e.id))
                    .collect();
                events.push(VideoEvent {
                    id: EventId::new(),
                    video_id: video,
                    t0: ts(a),
                    t1: ts(b.max(a)),
                    text: text.trim().to_string(),
                    participants,
                    provenance_id: prov.id,
                });
            }
            w0 = w1;
        }
        let ents: Vec<Entity> = entities.into_values().collect();
        ctx.storage.put_entities(&ents, &mentions).await?;
        ctx.storage.put_events(&events).await?;
        tracing::info!(video = %video, windows, entities = ents.len(), mentions = mentions.len(), events = events.len(), "entities and events written");
        ctx.progress(ctx.expected_items.unwrap_or(windows));
        Ok(OpOutput {
            emitted: ok_windows,
            stored: (ents.len() + events.len()) as u64,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_json_repairs_trailing_commas() {
        let text = "```json\n{\"entities\": [{\"name\": \"a\", \"kind\": \"person\",}, ], \"events\": [],}\n```";
        let v = extract_json(text).expect("repaired");
        assert_eq!(v["entities"][0]["name"], "a");
        assert!(extract_json("no json here").is_none());
        let ok = extract_json("prefix {\"a\": \"x, }\"} suffix").unwrap();
        assert_eq!(ok["a"], "x, }");
    }

    #[test]
    fn json_is_found_inside_prose_and_fences() {
        let v = extract_json("Here you go:\n```json\n{\"entities\": [], \"events\": [{\"t0\": 1, \"t1\": 2, \"text\": \"x\"}]}\n```\nDone.").unwrap();
        assert_eq!(v["events"][0]["text"], "x");
        assert!(extract_json("no json here").is_none());
        assert!(extract_json("[1,2]").is_none());
    }
}
