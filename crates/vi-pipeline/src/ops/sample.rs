//! `sample`: decode video at the policy rate and persist `FrameSample` rows.

use std::sync::Arc;

use async_trait::async_trait;
use vi_core::model::FrameSample;
use vi_core::{FrameSampleId, Result};
use vi_media::VideoDecodeRequest;

use crate::operator::*;

/// Rows buffered before a storage write.
const BATCH: usize = 64;

/// Frame sampler.
#[derive(Debug, Default)]
pub struct Sample;

impl Sample {
    /// New sampler.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Operator for Sample {
    fn id(&self) -> &'static str {
        "sample"
    }

    fn version(&self) -> u32 {
        1
    }

    fn inputs(&self) -> &[InputKind] {
        &[ItemKind::Media]
    }

    fn outputs(&self) -> &[OutputKind] {
        &[ItemKind::Frame]
    }

    fn cost_estimate(&self, input: &InputSummary) -> CostEstimate {
        // Decoding dominates: roughly 0.04 CPU-seconds per second of 720p
        // H.264 with non-reference frames skipped, on a 2.3 GHz core.
        CostEstimate {
            cpu_secs: input.duration_secs
                * 0.04
                * (input.width as f64 * input.height as f64 / (1280.0 * 720.0)).max(0.1),
            usd: 0.0,
            provider_calls: 0,
        }
    }

    async fn run(&self, ctx: &OpContext, input: OpInput) -> Result<OpOutput> {
        let Item::Media(media) = input.item else {
            return Err(ctx.err("expected the media item"));
        };
        let track = media
            .video_track()
            .ok_or_else(|| ctx.err("no video track"))?
            .clone();
        // Idempotent re-runs: drop any samples from an earlier attempt.
        ctx.storage.delete_frame_samples(track.id).await?;

        let fps = ctx.policy.sample_fps;
        let req = VideoDecodeRequest {
            stream_index: Some(track.stream_index),
            ..VideoDecodeRequest::new(&media.acquired.path, fps, ctx.config.media.sample_max_dim)
        };
        let mut stream = vi_media::decode_video(&ctx.worker, req).await?;
        let mut batch: Vec<FrameSample> = Vec::with_capacity(BATCH);
        let mut emitted = 0u64;
        let mut stored = 0u64;
        loop {
            ctx.check_cancelled()?;
            let Some(frame) = stream.next().await? else {
                break;
            };
            let sample = FrameSample {
                id: FrameSampleId::new(),
                track_id: track.id,
                t: frame.t,
                pts: frame.pts,
                is_keyframe: frame.is_keyframe,
                phash: None,
                thumbnail_blob: None,
                width: frame.source_width,
                height: frame.source_height,
            };
            batch.push(sample.clone());
            if batch.len() >= BATCH {
                ctx.storage.put_frame_samples(&batch).await?;
                stored += batch.len() as u64;
                batch.clear();
            }
            ctx.emit(Item::Frame(FrameItem { sample, frame })).await?;
            emitted += 1;
            if emitted % 32 == 0 {
                ctx.progress(emitted);
            }
        }
        if !batch.is_empty() {
            ctx.storage.put_frame_samples(&batch).await?;
            stored += batch.len() as u64;
        }
        ctx.progress(emitted);
        let _ = Arc::strong_count(&media);
        Ok(OpOutput { emitted, stored })
    }
}
