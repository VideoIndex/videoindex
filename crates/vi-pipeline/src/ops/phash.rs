//! `phash`: 64-bit perceptual hash per frame sample.

use async_trait::async_trait;
use tokio::sync::Mutex;
use vi_core::{cpu, FrameSampleId, Result};

use crate::operator::*;

const BATCH: usize = 64;

/// Perceptual hasher.
#[derive(Debug, Default)]
pub struct PHash {
    pending: Mutex<Vec<(FrameSampleId, u64)>>,
}

impl PHash {
    /// New hasher.
    pub fn new() -> Self {
        Self::default()
    }

    async fn flush(&self, ctx: &OpContext, force: bool) -> Result<u64> {
        let mut pending = self.pending.lock().await;
        if pending.is_empty() || (!force && pending.len() < BATCH) {
            return Ok(0);
        }
        let batch = std::mem::take(&mut *pending);
        drop(pending);
        ctx.storage.update_frame_phash(&batch).await?;
        Ok(batch.len() as u64)
    }
}

#[async_trait]
impl Operator for PHash {
    fn id(&self) -> &'static str {
        "phash"
    }

    fn version(&self) -> u32 {
        1
    }

    fn inputs(&self) -> &[InputKind] {
        &[ItemKind::Frame]
    }

    fn outputs(&self) -> &[OutputKind] {
        &[ItemKind::Hashed]
    }

    fn cost_estimate(&self, input: &InputSummary) -> CostEstimate {
        CostEstimate {
            cpu_secs: input.expected_samples as f64 * 0.001,
            usd: 0.0,
            provider_calls: 0,
        }
    }

    async fn run(&self, ctx: &OpContext, input: OpInput) -> Result<OpOutput> {
        let Item::Frame(FrameItem { sample, frame }) = input.item else {
            return Err(ctx.err("expected a frame"));
        };
        let f = frame.clone();
        let hash = cpu::run(move || vi_perceive::phash::phash_frame(&f))
            .await?
            .ok_or_else(|| ctx.err("frame is not RGB24"))?;
        drop(frame);
        self.pending.lock().await.push((sample.id, hash));
        let stored = self.flush(ctx, false).await?;
        ctx.emit(Item::Hashed {
            sample: sample.id,
            phash: hash,
            t: sample.t,
        })
        .await?;
        Ok(OpOutput { emitted: 1, stored })
    }

    async fn finish(&self, ctx: &OpContext) -> Result<OpOutput> {
        let stored = self.flush(ctx, true).await?;
        Ok(OpOutput { emitted: 0, stored })
    }
}
