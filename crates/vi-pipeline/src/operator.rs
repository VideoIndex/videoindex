//! The operator contract from `docs/05-indexing-pipeline.md`.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use vi_core::config::{Config, IndexPolicy, WorkerConfig};
use vi_core::model::{FailedRange, FrameSample, Track, Video};
use vi_core::{Error, Event, EventBus, JobId, Progress, Result, Timestamp, VideoId};
use vi_index::{BlobKey, Storage};
use vi_media::{Acquired, FrameBuffer, Probe};
use vi_providers::ProviderRegistry;

/// Kinds of items that flow between operators. An operator's `inputs` and
/// `outputs` are sets of these; the DAG is derived from them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    /// The acquired, probed media: the root of every job.
    Media,
    /// A sampled frame with its `FrameSample` row.
    Frame,
    /// A frame sample with its pHash filled in.
    Hashed,
    /// A frame sample with its thumbnail stored.
    Thumbnail,
    /// 16 kHz mono PCM chunk.
    AudioChunk,
    /// Speech ranges.
    SpeechRange,
    /// Transcript spans.
    TranscriptSpan,
    /// Shot segments.
    Shot,
    /// OCR spans.
    OcrSpan,
    /// Image embeddings.
    ImageEmbedding,
    /// Text embeddings.
    TextEmbedding,
    /// Scene segments.
    Scene,
    /// Chapter segments.
    Chapter,
    /// VLM descriptions.
    Description,
    /// Entities and events.
    Extraction,
}

/// Input kinds, same enum.
pub type InputKind = ItemKind;
/// Output kinds, same enum.
pub type OutputKind = ItemKind;

/// The acquired media plus everything probing learned.
#[derive(Debug, Clone)]
pub struct MediaItem {
    /// Local file and hashes.
    pub acquired: Acquired,
    /// libav probe.
    pub probe: Probe,
    /// The Video row.
    pub video: Video,
    /// Its tracks.
    pub tracks: Vec<Track>,
    /// Expected number of frame samples at the policy rate.
    pub expected_samples: u64,
}

impl MediaItem {
    /// The video track chosen for sampling.
    pub fn video_track(&self) -> Option<&Track> {
        let best = self.probe.video_stream()?;
        self.tracks
            .iter()
            .find(|t| t.kind == vi_core::model::TrackKind::Video && t.stream_index == best.index)
    }

    /// The audio track chosen for transcription.
    pub fn audio_track(&self) -> Option<&Track> {
        let best = self.probe.audio_stream()?;
        self.tracks
            .iter()
            .find(|t| t.kind == vi_core::model::TrackKind::Audio && t.stream_index == best.index)
    }
}

/// A run of speech with its PCM, what the ASR operator transcribes.
#[derive(Debug, Clone)]
pub struct SpeechItem {
    /// Start in the video.
    pub t0: Timestamp,
    /// End in the video.
    pub t1: Timestamp,
    /// 16 kHz mono samples covering `[t0, t1)`.
    pub samples: Arc<[i16]>,
    /// Sample rate.
    pub sample_rate: u32,
    /// Ordinal within the job, for logs and progress.
    pub index: u64,
}

/// A decoded frame with its sample row.
#[derive(Debug, Clone)]
pub struct FrameItem {
    /// The row as persisted by `sample` (pHash and thumbnail not yet set).
    pub sample: FrameSample,
    /// Pixels.
    pub frame: Arc<FrameBuffer>,
}

/// One unit of work flowing along a DAG edge.
#[derive(Debug, Clone)]
pub enum Item {
    /// Root item.
    Media(Arc<MediaItem>),
    /// A frame.
    Frame(FrameItem),
    /// A hashed frame sample, still carrying its pixels so consumers that
    /// need them (embeddings, OCR) can read the frame once and drop it.
    Hashed {
        /// Sample id.
        sample: vi_core::FrameSampleId,
        /// The hash.
        phash: u64,
        /// Time, for downstream grouping.
        t: vi_core::Timestamp,
        /// Pixels; holding this keeps a decoder slot busy, so consumers
        /// copy what they need and drop it promptly.
        frame: Arc<FrameBuffer>,
    },
    /// A stored thumbnail.
    Thumbnail {
        /// Sample id.
        sample: vi_core::FrameSampleId,
        /// Blob key.
        blob: BlobKey,
    },
    /// A transcript span that has been persisted.
    TranscriptSpan(Arc<vi_core::model::TranscriptSpan>),
    /// Speech audio for ASR.
    SpeechRange(Arc<SpeechItem>),
    /// A shot segment that has been persisted.
    Shot(Arc<vi_core::model::Segment>),
    /// An OCR span that has been persisted.
    OcrSpan(Arc<vi_core::model::OcrSpan>),
    /// A scene segment that has been persisted.
    Scene(Arc<vi_core::model::Segment>),
    /// A chapter segment that has been persisted.
    Chapter(Arc<vi_core::model::Segment>),
    /// A description that has been persisted.
    Description(Arc<vi_core::model::Description>),
}

impl Item {
    /// Kind tag.
    pub fn kind(&self) -> ItemKind {
        match self {
            Item::Media(_) => ItemKind::Media,
            Item::Frame(_) => ItemKind::Frame,
            Item::Hashed { .. } => ItemKind::Hashed,
            Item::Thumbnail { .. } => ItemKind::Thumbnail,
            Item::TranscriptSpan(_) => ItemKind::TranscriptSpan,
            Item::SpeechRange(_) => ItemKind::SpeechRange,
            Item::Shot(_) => ItemKind::Shot,
            Item::OcrSpan(_) => ItemKind::OcrSpan,
            Item::Scene(_) => ItemKind::Scene,
            Item::Chapter(_) => ItemKind::Chapter,
            Item::Description(_) => ItemKind::Description,
        }
    }
}

/// Facts about the input that cost estimation can use.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InputSummary {
    /// Duration in seconds.
    pub duration_secs: f64,
    /// Sampling rate.
    pub sample_fps: f64,
    /// Expected sample count.
    pub expected_samples: u64,
    /// Source width.
    pub width: u32,
    /// Source height.
    pub height: u32,
    /// Whether an audio stream exists.
    pub has_audio: bool,
}

/// Estimated cost of running an operator over an input.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CostEstimate {
    /// CPU seconds.
    pub cpu_secs: f64,
    /// Provider spend.
    pub usd: f64,
    /// Provider calls.
    pub provider_calls: u64,
}

/// Sends items to every downstream consumer with backpressure.
#[derive(Debug, Clone)]
pub struct Emitter {
    senders: Vec<mpsc::Sender<Item>>,
}

impl Emitter {
    /// Emitter over the given consumer channels.
    pub fn new(senders: Vec<mpsc::Sender<Item>>) -> Self {
        Self { senders }
    }

    /// An emitter with no consumers.
    pub fn none() -> Self {
        Self {
            senders: Vec::new(),
        }
    }

    /// Whether anyone is listening.
    pub fn has_consumers(&self) -> bool {
        !self.senders.is_empty()
    }

    /// Send to all consumers; waits when any consumer's channel is full.
    /// A consumer that has gone away (its task ended) is skipped.
    pub async fn emit(&self, item: Item) -> Result<()> {
        match self.senders.len() {
            0 => Ok(()),
            1 => {
                let _ = self.senders[0].send(item).await;
                Ok(())
            }
            _ => {
                for s in &self.senders {
                    let _ = s.send(item.clone()).await;
                }
                Ok(())
            }
        }
    }
}

/// The job's spending limits (`docs/05-indexing-pipeline.md`, budgets).
/// Provider-backed operators call [`Budget::allow_call`] before each
/// provider call; once a limit is hit no new calls are issued, what exists
/// is written, and the report lists what was skipped.
#[derive(Debug)]
pub struct Budget {
    /// Cost ceiling in USD for this job; `None` means unlimited.
    pub max_cost_usd: Option<f64>,
    /// Wall-clock deadline for issuing provider calls.
    pub deadline: Option<Instant>,
    /// Wall-clock limit in seconds, for reports.
    pub max_wallclock_secs: Option<f64>,
    spent_micro_usd: AtomicU64,
    exhausted: AtomicBool,
    started: Instant,
}

impl Budget {
    /// A budget from a policy's per-hour limits and the video's duration.
    pub fn for_policy(policy: &IndexPolicy, duration_secs: f64) -> Result<Self> {
        let hours = (duration_secs / 3600.0).max(1.0 / 60.0); // at least a minute's worth
        let max_cost_usd =
            (policy.max_cost_usd_per_hour > 0.0).then_some(policy.max_cost_usd_per_hour * hours);
        let wall = policy.max_wallclock_per_hour_secs()?;
        let max_wallclock_secs = (wall > 0.0).then_some(wall * hours);
        let started = Instant::now();
        Ok(Self {
            max_cost_usd,
            deadline: max_wallclock_secs.map(|s| started + std::time::Duration::from_secs_f64(s)),
            max_wallclock_secs,
            spent_micro_usd: AtomicU64::new(0),
            exhausted: AtomicBool::new(false),
            started,
        })
    }

    /// No limits.
    pub fn unlimited() -> Self {
        Self {
            max_cost_usd: None,
            deadline: None,
            max_wallclock_secs: None,
            spent_micro_usd: AtomicU64::new(0),
            exhausted: AtomicBool::new(false),
            started: Instant::now(),
        }
    }

    /// Record spend.
    pub fn add_cost(&self, usd: f64) {
        if usd > 0.0 {
            self.spent_micro_usd
                .fetch_add((usd * 1e6).round() as u64, Ordering::Relaxed);
        }
    }

    /// Spend so far.
    pub fn spent_usd(&self) -> f64 {
        self.spent_micro_usd.load(Ordering::Relaxed) as f64 / 1e6
    }

    /// Whether another provider call may be issued. Once this returns
    /// false it stays false.
    pub fn allow_call(&self) -> bool {
        if self.exhausted.load(Ordering::Relaxed) {
            return false;
        }
        let over_cost = self.max_cost_usd.is_some_and(|max| self.spent_usd() >= max);
        let over_time = self.deadline.is_some_and(|d| Instant::now() >= d);
        if over_cost || over_time {
            self.exhausted.store(true, Ordering::Relaxed);
            tracing::warn!(
                spent_usd = self.spent_usd(),
                elapsed_secs = self.started.elapsed().as_secs_f64(),
                reason = if over_cost { "cost" } else { "wallclock" },
                "budget exhausted; no further provider calls"
            );
            return false;
        }
        true
    }

    /// Whether a limit was hit.
    pub fn is_exhausted(&self) -> bool {
        self.exhausted.load(Ordering::Relaxed)
    }

    /// Which limit was hit, for reports.
    pub fn exhausted_reason(&self) -> Option<&'static str> {
        if !self.is_exhausted() {
            return None;
        }
        if self.max_cost_usd.is_some_and(|m| self.spent_usd() >= m) {
            Some("cost")
        } else {
            Some("wallclock")
        }
    }
}

/// Failure bookkeeping for one stage, shared with the scheduler.
#[derive(Debug, Default)]
pub struct StageFailures {
    count: AtomicU64,
    skipped: AtomicU64,
    first: std::sync::Mutex<Vec<FailedRange>>,
}

/// How many failed ranges a report keeps in full.
pub const MAX_REPORTED_FAILURES: usize = 50;

impl StageFailures {
    /// Record a failed input range.
    pub fn record(&self, t0: Timestamp, t1: Timestamp, error: impl std::fmt::Display) {
        self.count.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut v) = self.first.lock() {
            if v.len() < MAX_REPORTED_FAILURES {
                v.push(FailedRange {
                    t0,
                    t1,
                    error: error.to_string(),
                });
            }
        }
    }

    /// Record inputs skipped because the budget ran out.
    pub fn skipped(&self, n: u64) {
        self.skipped.fetch_add(n, Ordering::Relaxed);
    }

    /// Failures so far.
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Skips so far.
    pub fn skipped_count(&self) -> u64 {
        self.skipped.load(Ordering::Relaxed)
    }

    /// The recorded ranges.
    pub fn ranges(&self) -> Vec<FailedRange> {
        self.first.lock().map(|v| v.clone()).unwrap_or_default()
    }
}

/// What an operator sees while running.
pub struct OpContext {
    /// Job id.
    pub job: JobId,
    /// Video being indexed.
    pub video: VideoId,
    /// Stage name (operator id).
    pub stage: String,
    /// Storage.
    pub storage: Arc<dyn Storage>,
    /// Global config.
    pub config: Arc<Config>,
    /// Model providers bound to roles.
    pub providers: Arc<ProviderRegistry>,
    /// The active policy.
    pub policy: IndexPolicy,
    /// Decode worker settings.
    pub worker: WorkerConfig,
    /// Downstream sink.
    pub emitter: Emitter,
    /// Cancellation.
    pub cancel: CancellationToken,
    /// Event bus.
    pub events: EventBus,
    /// Expected item count for progress, when known.
    pub expected_items: Option<u64>,
    /// The job's budget.
    pub budget: Arc<Budget>,
    /// This stage's failure record.
    pub failures: Arc<StageFailures>,
}

impl std::fmt::Debug for OpContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpContext")
            .field("job", &self.job)
            .field("video", &self.video)
            .field("stage", &self.stage)
            .finish()
    }
}

impl OpContext {
    /// Emit an item downstream.
    pub async fn emit(&self, item: Item) -> Result<()> {
        if self.cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        self.emitter.emit(item).await
    }

    /// Whether a provider call may be issued now (budget not exhausted).
    pub fn allow_provider_call(&self) -> bool {
        self.budget.allow_call()
    }

    /// Record a provider call's cost against the budget.
    pub fn record_cost(&self, stats: &vi_providers::CallStats) {
        self.budget.add_cost(stats.cost_usd);
    }

    /// Record an input range that failed after retries; the stage goes on.
    pub fn record_failure(&self, t0: Timestamp, t1: Timestamp, error: impl std::fmt::Display) {
        tracing::warn!(stage = %self.stage, t0 = %t0, t1 = %t1, error = %error, "input failed; continuing");
        self.failures.record(t0, t1, error);
    }

    /// Record inputs skipped because the budget ran out.
    pub fn record_skipped(&self, n: u64) {
        self.failures.skipped(n);
    }

    /// Report progress for this stage.
    pub fn progress(&self, items_done: u64) {
        let fraction = match self.expected_items {
            Some(total) if total > 0 => (items_done as f64 / total as f64).min(1.0),
            _ => 0.0,
        };
        self.events.emit(Event::Progress(Progress {
            job: self.job,
            video: Some(self.video),
            stage: self.stage.clone(),
            fraction,
            items_done,
            items_total: self.expected_items,
            cost_usd: self.budget.spent_usd(),
            eta_secs: None,
        }));
    }

    /// Error if cancelled.
    pub fn check_cancelled(&self) -> Result<()> {
        if self.cancel.is_cancelled() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }

    /// Build an operator error.
    pub fn err(&self, msg: impl std::fmt::Display) -> Error {
        Error::operator(&self.stage, msg.to_string())
    }
}

/// Input to one `run` call.
#[derive(Debug, Clone)]
pub struct OpInput {
    /// The item.
    pub item: Item,
}

/// Result of one `run` call. Items themselves stream through
/// [`OpContext::emit`] so an hour of frames never sits in one `Vec`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OpOutput {
    /// Items emitted downstream during this call.
    pub emitted: u64,
    /// Rows written to storage during this call.
    pub stored: u64,
}

/// One processing step. Typed, versioned, cacheable.
#[async_trait]
pub trait Operator: Send + Sync {
    /// Stable id, e.g. `"phash"`. Also the stage name in checkpoints.
    fn id(&self) -> &'static str;
    /// Bump when output semantics change.
    fn version(&self) -> u32;
    /// What it consumes.
    fn inputs(&self) -> &[InputKind];
    /// What it produces.
    fn outputs(&self) -> &[OutputKind];
    /// Inputs the operator can use but does not need: the DAG builds when
    /// nothing in the policy produces them (e.g. `text_embed` embeds OCR
    /// spans when `ocr` runs and transcript spans otherwise).
    fn optional_inputs(&self) -> &[InputKind] {
        &[]
    }
    /// Provider roles (`vi_core::config::roles`) that must be bound for the
    /// operator to run; checked at plan time so a missing provider fails
    /// before any decoding.
    fn required_roles(&self) -> &[&'static str] {
        &[]
    }
    /// Cost estimate.
    fn cost_estimate(&self, input: &InputSummary) -> CostEstimate;
    /// Parameters that change the output and so belong in the cache key
    /// (provider, model, thresholds). The scheduler adds the content hash,
    /// operator id and version itself.
    fn cache_params(&self, _ctx: &OpContext) -> serde_json::Value {
        serde_json::Value::Null
    }
    /// Re-emit stored outputs of an earlier run to consumers without
    /// recomputing. Returns the number of items emitted, or `None` when the
    /// operator cannot replay (its outputs are not stored as items). Called
    /// instead of `run`/`finish` when the stage is cached but a consumer
    /// still needs its items.
    async fn replay(&self, _ctx: &OpContext) -> Result<Option<u64>> {
        Ok(None)
    }
    /// Whether [`Operator::replay`] is implemented (decided at plan time).
    fn replay_supported(&self) -> bool {
        false
    }
    /// Process one input item, emitting through `ctx`.
    async fn run(&self, ctx: &OpContext, input: OpInput) -> Result<OpOutput>;
    /// Called once after the last input; flush buffered writes.
    async fn finish(&self, _ctx: &OpContext) -> Result<OpOutput> {
        Ok(OpOutput::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_limits_from_policy() {
        let mut p = IndexPolicy::m0();
        p.max_cost_usd_per_hour = 2.0;
        p.max_wallclock_per_hour = "20m".into();
        let b = Budget::for_policy(&p, 1800.0).unwrap();
        assert_eq!(b.max_cost_usd, Some(1.0));
        assert_eq!(b.max_wallclock_secs, Some(600.0));
        assert!(b.allow_call());
        b.add_cost(0.6);
        assert!(b.allow_call());
        b.add_cost(0.5);
        assert!(!b.allow_call(), "over the cost ceiling");
        assert!(b.is_exhausted());
        assert_eq!(b.exhausted_reason(), Some("cost"));
        assert!((b.spent_usd() - 1.1).abs() < 1e-9);

        let mut free = IndexPolicy::m0();
        free.max_cost_usd_per_hour = 0.0;
        free.max_wallclock_per_hour = "0".into();
        let b = Budget::for_policy(&free, 10.0).unwrap();
        assert!(b.max_cost_usd.is_none() && b.deadline.is_none());
        assert!(b.allow_call());

        let mut bad = IndexPolicy::m0();
        bad.max_wallclock_per_hour = "soon".into();
        assert!(Budget::for_policy(&bad, 10.0).is_err());
    }

    #[test]
    fn failures_are_capped_but_counted() {
        let f = StageFailures::default();
        for i in 0..(MAX_REPORTED_FAILURES + 10) {
            f.record(
                Timestamp::from_secs(i as i64),
                Timestamp::from_secs(i as i64 + 1),
                "boom",
            );
        }
        f.skipped(3);
        assert_eq!(f.count() as usize, MAX_REPORTED_FAILURES + 10);
        assert_eq!(f.ranges().len(), MAX_REPORTED_FAILURES);
        assert_eq!(f.skipped_count(), 3);
    }
}
