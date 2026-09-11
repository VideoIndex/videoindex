//! `shot_boundary`: cut detection over the sampled frames, written as
//! `shot` Segments that cover the whole video with no gaps.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use vi_core::model::{Provenance, Segment, SegmentLevel};
use vi_core::{cpu, FrameSampleId, Result, SegmentId, Timestamp};
use vi_perceive::shot::{FrameSignature, ShotDetector, ShotParams};

use crate::operator::*;

/// Shot boundary operator.
#[derive(Debug)]
pub struct ShotBoundary {
    params: ShotParams,
    state: Mutex<Option<State>>,
}

#[derive(Debug)]
struct State {
    media: Arc<MediaItem>,
    detector: ShotDetector,
    /// Time and sample id of every frame seen, in order.
    samples: Vec<(Timestamp, FrameSampleId)>,
}

impl Default for ShotBoundary {
    fn default() -> Self {
        Self::new()
    }
}

impl ShotBoundary {
    /// New operator with default parameters.
    pub fn new() -> Self {
        Self {
            params: ShotParams::default(),
            state: Mutex::new(None),
        }
    }
}

#[async_trait]
impl Operator for ShotBoundary {
    fn id(&self) -> &'static str {
        "shot_boundary"
    }

    fn version(&self) -> u32 {
        1
    }

    fn inputs(&self) -> &[InputKind] {
        &[ItemKind::Media, ItemKind::Frame]
    }

    fn outputs(&self) -> &[OutputKind] {
        &[ItemKind::Shot]
    }

    fn cost_estimate(&self, input: &InputSummary) -> CostEstimate {
        CostEstimate {
            cpu_secs: input.expected_samples as f64 * 0.0005,
            usd: 0.0,
            provider_calls: 0,
        }
    }

    async fn run(&self, ctx: &OpContext, input: OpInput) -> Result<OpOutput> {
        match input.item {
            Item::Media(media) => {
                *self.state.lock().await = Some(State {
                    media,
                    detector: ShotDetector::new(self.params),
                    samples: Vec::new(),
                });
                Ok(OpOutput::default())
            }
            Item::Frame(FrameItem { sample, frame }) => {
                let f = frame.clone();
                let sig = cpu::run(move || FrameSignature::from_frame(&f))
                    .await?
                    .ok_or_else(|| ctx.err("frame is not RGB24"))?;
                drop(frame);
                let mut guard = self.state.lock().await;
                let st = guard
                    .as_mut()
                    .ok_or_else(|| ctx.err("frame arrived before the media item"))?;
                st.detector.push(sig);
                st.samples.push((sample.t, sample.id));
                Ok(OpOutput::default())
            }
            _ => Err(ctx.err("expected media or a frame")),
        }
    }

    async fn finish(&self, ctx: &OpContext) -> Result<OpOutput> {
        let Some(st) = self.state.lock().await.take() else {
            return Ok(OpOutput::default());
        };
        let video = &st.media.video;
        // Idempotent: replace this level.
        ctx.storage
            .delete_segments(video.id, SegmentLevel::Shot)
            .await?;
        if st.samples.is_empty() {
            return Ok(OpOutput::default());
        }
        let duration = video.duration;
        let prov = Provenance::local(
            self.id(),
            self.version(),
            serde_json::json!({
                "samples": st.samples.len(),
                "cuts": st.detector.cuts().len(),
                "min_distance": self.params.min_distance,
                "hard_distance": self.params.hard_distance,
                "k_std": self.params.k_std,
                "ratio": self.params.ratio,
                "window": self.params.window,
                "sample_fps": ctx.policy.sample_fps,
            }),
        );
        ctx.storage.put_provenance(&prov).await?;
        // Shot i spans [start_i, start_{i+1}); the first starts at 0 and the
        // last ends at the video duration, so the level has no gaps.
        let mut starts: Vec<usize> = vec![0];
        starts.extend(st.detector.cuts().iter().copied());
        let mut segments = Vec::with_capacity(starts.len());
        for (i, &s) in starts.iter().enumerate() {
            let t0 = if i == 0 {
                Timestamp::ZERO
            } else {
                st.samples[s].0
            };
            let t1 = match starts.get(i + 1) {
                Some(&next) => st.samples[next].0,
                None => duration.max(st.samples[st.samples.len() - 1].0),
            };
            if t1 <= t0 {
                continue;
            }
            segments.push(Segment {
                id: SegmentId::new(),
                video_id: video.id,
                level: SegmentLevel::Shot,
                parent_id: None,
                t0,
                t1,
                keyframe_sample_id: Some(st.samples[s].1),
                title: None,
                summary: None,
                provenance_id: prov.id,
            });
        }
        ctx.storage.put_segments(&segments).await?;
        let stored = segments.len() as u64;
        let mut emitted = 0;
        for seg in segments {
            ctx.emit(Item::Shot(Arc::new(seg))).await?;
            emitted += 1;
        }
        tracing::info!(video = %video.id, shots = stored, "shot boundaries written");
        ctx.progress(ctx.expected_items.unwrap_or(st.samples.len() as u64));
        Ok(OpOutput { emitted, stored })
    }
}
