//! `image_embed`: one embedding per pHash-distinct frame through the
//! `image_embed` role (SigLIP locally by default). Frames within
//! `PHASH_DEDUP_DISTANCE` of the last embedded frame are skipped; the
//! embedding of the group's first frame stands for the group.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use vi_core::config::roles;
use vi_core::model::{Embedding, Provenance, TargetKind};
use vi_core::{EmbeddingId, FrameSampleId, Result};
use vi_perceive::{hamming, PHASH_DEDUP_DISTANCE};
use vi_providers::{provenance_for, ImageData, ImageEmbedder};

use crate::operator::*;

/// Frames per provider call.
const BATCH: usize = 16;

/// Image embedding operator.
#[derive(Debug, Default)]
pub struct ImageEmbed {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    last_hash: Option<u64>,
    /// Frames waiting for a batch: sample id, owned RGB pixels, time.
    pending: Vec<(FrameSampleId, ImageData, vi_core::Timestamp)>,
    embedded: u64,
    skipped: u64,
    seen: u64,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("pending", &self.pending.len())
            .field("embedded", &self.embedded)
            .field("skipped", &self.skipped)
            .finish()
    }
}

impl ImageEmbed {
    /// New operator.
    pub fn new() -> Self {
        Self::default()
    }

    async fn flush(&self, ctx: &OpContext, st: &mut State, force: bool) -> Result<u64> {
        let embedder: Arc<dyn ImageEmbedder> = ctx.providers.image_embedder()?;
        let batch = embedder.max_batch().clamp(1, BATCH);
        if st.pending.is_empty() || (!force && st.pending.len() < batch) {
            return Ok(0);
        }
        let items: Vec<(FrameSampleId, ImageData, vi_core::Timestamp)> =
            std::mem::take(&mut st.pending);
        if !ctx.allow_provider_call() {
            ctx.record_skipped(items.len() as u64);
            return Ok(0);
        }
        let images: Vec<ImageData> = items.iter().map(|(_, im, _)| im.clone()).collect();
        let resp = match embedder.embed_images(&images).await {
            Ok(r) => r,
            Err(e) => {
                let t0 = items
                    .iter()
                    .map(|i| i.2)
                    .min()
                    .unwrap_or(vi_core::Timestamp::ZERO);
                let t1 = items.iter().map(|i| i.2).max().unwrap_or(t0);
                for _ in 0..items.len() {
                    ctx.failures.record(t0, t1, &e);
                }
                tracing::warn!(stage = %ctx.stage, batch = items.len(), error = %e, "batch failed; continuing");
                return Ok(0);
            }
        };
        ctx.record_cost(&resp.stats);
        if resp.vectors.len() != items.len() {
            return Err(ctx.err(format!(
                "provider returned {} vectors for {} images",
                resp.vectors.len(),
                items.len()
            )));
        }
        let prov = provenance_for(
            self.id(),
            self.version(),
            &resp.stats,
            None,
            serde_json::json!({ "images": items.len(), "dim": embedder.dim() }),
        );
        ctx.storage.put_provenance(&prov).await?;
        let dim = resp.vectors.first().map(|v| v.len()).unwrap_or(0) as u32;
        let rows: Vec<Embedding> = items
            .iter()
            .zip(resp.vectors)
            .map(|((sample, _, _), vector)| Embedding {
                id: EmbeddingId::new(),
                target_kind: TargetKind::Frame,
                target_id: sample.to_string(),
                model: embedder.model().to_string(),
                dim,
                vector,
                provenance_id: prov.id,
            })
            .collect();
        ctx.storage.put_embeddings(&rows).await?;
        st.embedded += rows.len() as u64;
        Ok(rows.len() as u64)
    }
}

#[async_trait]
impl Operator for ImageEmbed {
    fn id(&self) -> &'static str {
        "image_embed"
    }

    fn version(&self) -> u32 {
        1
    }

    fn inputs(&self) -> &[InputKind] {
        &[ItemKind::Hashed]
    }

    fn outputs(&self) -> &[OutputKind] {
        &[ItemKind::ImageEmbedding]
    }

    fn required_roles(&self) -> &[&'static str] {
        &[roles::IMAGE_EMBED]
    }

    fn cache_params(&self, ctx: &OpContext) -> serde_json::Value {
        let (provider, model) = ctx
            .providers
            .image_embedder()
            .map(|a| (a.provider_name().to_string(), a.model().to_string()))
            .unwrap_or_default();
        serde_json::json!({
            "provider": provider,
            "model": model,
            "dedup_distance": PHASH_DEDUP_DISTANCE,
            "sample_fps": ctx.policy.sample_fps,
        })
    }

    fn cost_estimate(&self, input: &InputSummary) -> CostEstimate {
        // About a third of lecture frames are pHash-distinct; SigLIP base
        // on the CPU takes about 40 ms per image.
        CostEstimate {
            cpu_secs: input.expected_samples as f64 * 0.3 * 0.04,
            usd: 0.0,
            provider_calls: (input.expected_samples as f64 * 0.3 / BATCH as f64).ceil() as u64,
        }
    }

    async fn run(&self, ctx: &OpContext, input: OpInput) -> Result<OpOutput> {
        let Item::Hashed {
            sample,
            phash,
            frame,
            t,
        } = input.item
        else {
            return Err(ctx.err("expected a hashed frame"));
        };
        let mut st = self.state.lock().await;
        st.seen += 1;
        let duplicate = st
            .last_hash
            .is_some_and(|h| hamming(h, phash) <= PHASH_DEDUP_DISTANCE);
        if duplicate {
            st.skipped += 1;
            drop(frame);
            return Ok(OpOutput::default());
        }
        st.last_hash = Some(phash);
        // Copy the pixels out (unpadded) and release the decoder slot.
        let (w, h) = (frame.width, frame.height);
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            rgb.extend_from_slice(frame.row(y));
        }
        drop(frame);
        st.pending.push((
            sample,
            ImageData::Rgb8 {
                width: w,
                height: h,
                data: Arc::from(rgb),
            },
            t,
        ));
        let stored = self.flush(ctx, &mut st, false).await?;
        if st.seen % 64 == 0 {
            ctx.progress(st.seen);
        }
        Ok(OpOutput { emitted: 0, stored })
    }

    async fn finish(&self, ctx: &OpContext) -> Result<OpOutput> {
        let mut st = self.state.lock().await;
        let stored = self.flush(ctx, &mut st, true).await?;
        tracing::info!(
            frames = st.seen,
            embedded = st.embedded,
            skipped_duplicates = st.skipped,
            "image embeddings written"
        );
        ctx.progress(ctx.expected_items.unwrap_or(st.seen));
        let _ = Provenance::local;
        Ok(OpOutput { emitted: 0, stored })
    }
}
