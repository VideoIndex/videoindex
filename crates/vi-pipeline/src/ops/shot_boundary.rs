//! `shot_boundary`: cut detection over the sampled frames, written as
//! `shot` Segments that cover the whole video with no gaps. A batch job
//! writes them all in `finish`; a live job (ticks) upserts the open shot
//! with `t1 = head` at every tick and emits shots as they close.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use vi_core::model::{Provenance, Segment, SegmentLevel};
use vi_core::{cpu, FrameSampleId, ProvenanceId, Result, SegmentId, Timestamp};
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
    /// Live: what the ticks have written so far.
    live: Option<LiveShots>,
}

/// The shots a live job has upserted: every one but the last is closed;
/// the last ends at the latest head.
#[derive(Debug)]
struct LiveShots {
    provenance: ProvenanceId,
    shots: Vec<Segment>,
    /// How many of `shots` were emitted downstream as closed.
    emitted: usize,
    head: Timestamp,
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

    /// Live: bring the stored shots up to `head`. Shot `i` spans from the
    /// sample at its cut to the sample at the next cut; the last shot is
    /// open and ends at `head`. Closed shots are upserted once and emitted
    /// once; the open shot is upserted at every tick with its new `t1`
    /// (same id), and emitted when `close_all` is set (the stream ended).
    async fn sync_live(
        &self,
        ctx: &OpContext,
        st: &mut State,
        head: Timestamp,
        close_all: bool,
    ) -> Result<OpOutput> {
        if st.samples.is_empty() {
            return Ok(OpOutput::default());
        }
        let video_id = st.media.video.id;
        if st.live.is_none() {
            let prov = Provenance::local(
                self.id(),
                self.version(),
                serde_json::json!({
                    "live": true,
                    "min_distance": self.params.min_distance,
                    "hard_distance": self.params.hard_distance,
                    "k_std": self.params.k_std,
                    "ratio": self.params.ratio,
                    "window": self.params.window,
                    "sample_fps": ctx.policy.sample_fps,
                }),
            );
            ctx.storage.put_provenance(&prov).await?;
            st.live = Some(LiveShots {
                provenance: prov.id,
                shots: Vec::new(),
                emitted: 0,
                head,
            });
        }
        let live = st
            .live
            .as_mut()
            .ok_or_else(|| ctx.err("live shot state missing"))?;
        let mut starts: Vec<usize> = vec![0];
        starts.extend(st.detector.cuts().iter().copied());
        let last_sample = st.samples[st.samples.len() - 1].0;
        let mut changed: Vec<Segment> = Vec::new();
        for (i, &s) in starts.iter().enumerate() {
            let t0 = st.samples[s].0;
            let t1 = match starts.get(i + 1) {
                Some(&next) => st.samples[next].0,
                None => head.max(last_sample),
            };
            if t1 <= t0 {
                continue;
            }
            match live.shots.get_mut(i) {
                Some(seg) => {
                    if seg.t1 != t1 {
                        seg.t1 = t1;
                        changed.push(seg.clone());
                    }
                }
                None => {
                    let seg = Segment {
                        id: SegmentId::new(),
                        video_id,
                        level: SegmentLevel::Shot,
                        parent_id: None,
                        t0,
                        t1,
                        keyframe_sample_id: Some(st.samples[s].1),
                        title: None,
                        summary: None,
                        provenance_id: live.provenance,
                    };
                    live.shots.push(seg.clone());
                    changed.push(seg);
                }
            }
        }
        live.head = head;
        if !changed.is_empty() {
            ctx.storage.put_segments(&changed).await?;
        }
        let closed = if close_all {
            live.shots.len()
        } else {
            live.shots.len().saturating_sub(1)
        };
        let mut emitted = 0;
        for i in live.emitted..closed {
            ctx.emit(Item::Shot(Arc::new(live.shots[i].clone())))
                .await?;
            emitted += 1;
        }
        live.emitted = live.emitted.max(closed);
        Ok(OpOutput {
            emitted,
            stored: changed.len() as u64,
        })
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
        &[ItemKind::Media, ItemKind::Frame, ItemKind::Tick]
    }

    fn optional_inputs(&self) -> &[InputKind] {
        // Live jobs tick; batch jobs have no producer of ticks and write
        // every shot in `finish`.
        &[ItemKind::Tick]
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

    fn cache_params(&self, ctx: &OpContext) -> serde_json::Value {
        serde_json::json!({
            "min_distance": self.params.min_distance,
            "hard_distance": self.params.hard_distance,
            "k_std": self.params.k_std,
            "ratio": self.params.ratio,
            "window": self.params.window,
            "sample_fps": ctx.policy.sample_fps,
        })
    }

    fn replay_supported(&self) -> bool {
        true
    }

    async fn replay(&self, ctx: &OpContext) -> Result<Option<u64>> {
        // Replay must not run `finish`, which would delete the segments.
        let _ = self.state.lock().await.take();
        let shots = ctx.storage.segments(ctx.video, SegmentLevel::Shot).await?;
        let mut n = 0;
        for seg in shots {
            ctx.emit(Item::Shot(Arc::new(seg))).await?;
            n += 1;
        }
        Ok(Some(n))
    }

    async fn run(&self, ctx: &OpContext, input: OpInput) -> Result<OpOutput> {
        match input.item {
            Item::Media(media) => {
                *self.state.lock().await = Some(State {
                    media,
                    detector: ShotDetector::new(self.params),
                    samples: Vec::new(),
                    live: None,
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
            Item::Tick { head } => {
                let mut guard = self.state.lock().await;
                let st = guard
                    .as_mut()
                    .ok_or_else(|| ctx.err("tick arrived before the media item"))?;
                self.sync_live(ctx, st, head, false).await
            }
            _ => Err(ctx.err("expected media, a frame or a tick")),
        }
    }

    async fn finish(&self, ctx: &OpContext) -> Result<OpOutput> {
        let Some(mut st) = self.state.lock().await.take() else {
            return Ok(OpOutput::default());
        };
        if st.live.is_some() {
            // A live job that ended: the open shot closes at the head, and
            // what the ticks wrote stays (ids included).
            let head = st
                .live
                .as_ref()
                .map(|l| l.head)
                .unwrap_or(Timestamp::ZERO)
                .max(st.samples.last().map(|s| s.0).unwrap_or(Timestamp::ZERO));
            return self.sync_live(ctx, &mut st, head, true).await;
        }
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
