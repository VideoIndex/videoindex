//! `thumbnail`: WebP thumbnail per frame sample, stored as a blob.

use async_trait::async_trait;
use bytes::Bytes;
use tokio::sync::Mutex;
use vi_core::{cpu, FrameSampleId, Result};
use vi_index::BlobKey;

use crate::operator::*;

const BATCH: usize = 64;

/// Thumbnail encoder.
#[derive(Debug)]
pub struct Thumbnail {
    max_dim: u32,
    quality: u8,
    pending: Mutex<Vec<(FrameSampleId, BlobKey)>>,
}

impl Thumbnail {
    /// Thumbnails with the longest side `max_dim` at WebP `quality`.
    pub fn new(max_dim: u32, quality: u8) -> Self {
        Self {
            max_dim,
            quality,
            pending: Mutex::new(Vec::new()),
        }
    }

    async fn flush(&self, ctx: &OpContext, force: bool) -> Result<u64> {
        let mut pending = self.pending.lock().await;
        if pending.is_empty() || (!force && pending.len() < BATCH) {
            return Ok(0);
        }
        let batch = std::mem::take(&mut *pending);
        drop(pending);
        ctx.storage.update_frame_thumbnail(&batch).await?;
        Ok(batch.len() as u64)
    }
}

#[async_trait]
impl Operator for Thumbnail {
    fn id(&self) -> &'static str {
        "thumbnail"
    }

    fn version(&self) -> u32 {
        1
    }

    fn inputs(&self) -> &[InputKind] {
        &[ItemKind::Frame]
    }

    fn outputs(&self) -> &[OutputKind] {
        &[ItemKind::Thumbnail]
    }

    fn cost_estimate(&self, input: &InputSummary) -> CostEstimate {
        CostEstimate {
            cpu_secs: input.expected_samples as f64 * 0.008,
            usd: 0.0,
            provider_calls: 0,
        }
    }

    async fn run(&self, ctx: &OpContext, input: OpInput) -> Result<OpOutput> {
        let Item::Frame(FrameItem { sample, frame }) = input.item else {
            return Err(ctx.err("expected a frame"));
        };
        let (max_dim, quality) = (self.max_dim, self.quality);
        let f = frame.clone();
        let (bytes, _w, _h) =
            cpu::run(move || vi_perceive::thumbnail::encode_webp_thumbnail(&f, max_dim, quality))
                .await?
                .map_err(|e| ctx.err(e))?;
        drop(frame);
        let key = BlobKey::for_bytes(&bytes);
        ctx.storage.put_blob(&key, Bytes::from(bytes)).await?;
        self.pending.lock().await.push((sample.id, key.clone()));
        let stored = self.flush(ctx, false).await?;
        ctx.emit(Item::Thumbnail {
            sample: sample.id,
            blob: key,
        })
        .await?;
        Ok(OpOutput {
            emitted: 1,
            stored: stored + 1,
        })
    }

    async fn finish(&self, ctx: &OpContext) -> Result<OpOutput> {
        let stored = self.flush(ctx, true).await?;
        Ok(OpOutput { emitted: 0, stored })
    }
}
