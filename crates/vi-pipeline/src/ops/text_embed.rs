//! `text_embed`: vectors for transcript and OCR spans through the
//! `text_embed` role (bge-small locally by default), batched.

use async_trait::async_trait;
use tokio::sync::Mutex;
use vi_core::config::roles;
use vi_core::model::{Embedding, TargetKind};
use vi_core::{EmbeddingId, Result};
use vi_providers::provenance_for;

use crate::operator::*;

/// Text embedding operator.
#[derive(Debug, Default)]
pub struct TextEmbed {
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    pending: Vec<(TargetKind, String, String, vi_core::Timestamp)>,
    embedded: u64,
}

impl TextEmbed {
    /// New operator.
    pub fn new() -> Self {
        Self::default()
    }

    async fn flush(&self, ctx: &OpContext, st: &mut State, force: bool) -> Result<u64> {
        let embedder = ctx.providers.text_embedder()?;
        let batch = embedder.max_batch().max(1);
        if st.pending.is_empty() || (!force && st.pending.len() < batch) {
            return Ok(0);
        }
        let items = std::mem::take(&mut st.pending);
        let mut stored = 0;
        for chunk in items.chunks(batch) {
            if !ctx.allow_provider_call() {
                ctx.record_skipped(chunk.len() as u64);
                continue;
            }
            let texts: Vec<String> = chunk.iter().map(|(_, _, t, _)| t.clone()).collect();
            let resp = match embedder.embed(&texts).await {
                Ok(r) => r,
                Err(e) => {
                    let t0 = chunk
                        .iter()
                        .map(|c| c.3)
                        .min()
                        .unwrap_or(vi_core::Timestamp::ZERO);
                    let t1 = chunk.iter().map(|c| c.3).max().unwrap_or(t0);
                    for _ in 0..chunk.len() {
                        ctx.failures.record(t0, t1, &e);
                    }
                    tracing::warn!(stage = %ctx.stage, batch = chunk.len(), error = %e, "batch failed; continuing");
                    continue;
                }
            };
            ctx.record_cost(&resp.stats);
            if resp.vectors.len() != chunk.len() {
                return Err(ctx.err(format!(
                    "provider returned {} vectors for {} texts",
                    resp.vectors.len(),
                    chunk.len()
                )));
            }
            let prov = provenance_for(
                self.id(),
                self.version(),
                &resp.stats,
                None,
                serde_json::json!({ "texts": chunk.len(), "dim": embedder.dim() }),
            );
            ctx.storage.put_provenance(&prov).await?;
            let dim = resp.vectors.first().map(|v| v.len()).unwrap_or(0) as u32;
            let rows: Vec<Embedding> = chunk
                .iter()
                .zip(resp.vectors)
                .map(|((kind, id, _, _), vector)| Embedding {
                    id: EmbeddingId::new(),
                    target_kind: *kind,
                    target_id: id.clone(),
                    model: embedder.model().to_string(),
                    dim,
                    vector,
                    provenance_id: prov.id,
                })
                .collect();
            ctx.storage.put_embeddings(&rows).await?;
            stored += rows.len() as u64;
        }
        st.embedded += stored;
        Ok(stored)
    }
}

#[async_trait]
impl Operator for TextEmbed {
    fn id(&self) -> &'static str {
        "text_embed"
    }

    fn version(&self) -> u32 {
        1
    }

    fn inputs(&self) -> &[InputKind] {
        &[ItemKind::TranscriptSpan, ItemKind::OcrSpan]
    }

    fn outputs(&self) -> &[OutputKind] {
        &[ItemKind::TextEmbedding]
    }

    fn optional_inputs(&self) -> &[InputKind] {
        &[ItemKind::TranscriptSpan, ItemKind::OcrSpan]
    }

    fn required_roles(&self) -> &[&'static str] {
        &[roles::TEXT_EMBED]
    }

    fn cache_params(&self, ctx: &OpContext) -> serde_json::Value {
        let (provider, model) = ctx
            .providers
            .text_embedder()
            .map(|a| (a.provider_name().to_string(), a.model().to_string()))
            .unwrap_or_default();
        serde_json::json!({ "provider": provider, "model": model })
    }

    fn cost_estimate(&self, input: &InputSummary) -> CostEstimate {
        // About one span per 12 s of speech, 5 ms each on the CPU.
        let spans = input.duration_secs / 12.0;
        CostEstimate {
            cpu_secs: spans * 0.005,
            usd: 0.0,
            provider_calls: (spans / 32.0).ceil() as u64,
        }
    }

    async fn run(&self, ctx: &OpContext, input: OpInput) -> Result<OpOutput> {
        let (kind, id, text, t) = match input.item {
            Item::TranscriptSpan(s) => (
                TargetKind::TranscriptSpan,
                s.id.to_string(),
                s.text.clone(),
                s.t0,
            ),
            Item::OcrSpan(s) => (TargetKind::OcrSpan, s.id.to_string(), s.text.clone(), s.t),
            Item::Media(_) => return Ok(OpOutput::default()),
            _ => return Err(ctx.err("expected a transcript or OCR span")),
        };
        if text.trim().is_empty() {
            return Ok(OpOutput::default());
        }
        let mut st = self.state.lock().await;
        st.pending.push((kind, id, text, t));
        let stored = self.flush(ctx, &mut st, false).await?;
        Ok(OpOutput { emitted: 0, stored })
    }

    async fn finish(&self, ctx: &OpContext) -> Result<OpOutput> {
        let mut st = self.state.lock().await;
        let stored = self.flush(ctx, &mut st, true).await?;
        tracing::info!(embedded = st.embedded, "text embeddings written");
        ctx.progress(ctx.expected_items.unwrap_or(st.embedded));
        Ok(OpOutput { emitted: 0, stored })
    }
}
