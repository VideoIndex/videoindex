//! The operator contract from `docs/05-indexing-pipeline.md`.

use std::collections::VecDeque;
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
    /// Live jobs: the stream has been decoded up to a head time. The root
    /// operator of a live policy emits one every `tick_secs`; operators
    /// that list it among their inputs flush their batches, close what is
    /// open and record `last_t = head`. Batch policies have no producer of
    /// it, so no batch behaviour changes.
    Tick,
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
    /// Expected number of frame samples at the policy rate; `None` for a
    /// live recording, whose length is not known (progress then reports
    /// the head time rather than a fraction).
    pub expected_samples: Option<u64>,
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
    /// 16 kHz mono PCM from a live decode, what `vad_stream` consumes.
    AudioChunk(Arc<vi_media::AudioChunk>),
    /// Live jobs: everything up to `head` has been decoded.
    Tick {
        /// Stream time decoded so far.
        head: Timestamp,
    },
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
            Item::AudioChunk(_) => ItemKind::AudioChunk,
            Item::Tick { .. } => ItemKind::Tick,
        }
    }

    /// The stream time the item stands for: the end of a range, the time
    /// of a frame, the head of a tick; `None` for items without one (the
    /// media item, thumbnails, descriptions). Live jobs record it as a
    /// stage's `last_t`.
    pub fn time(&self) -> Option<Timestamp> {
        match self {
            Item::Media(_) | Item::Thumbnail { .. } | Item::Description(_) => None,
            Item::Frame(f) => Some(f.sample.t),
            Item::Hashed { t, .. } => Some(*t),
            Item::TranscriptSpan(s) => Some(s.t1),
            Item::SpeechRange(s) => Some(s.t1),
            Item::Shot(s) | Item::Scene(s) | Item::Chapter(s) => Some(s.t1),
            Item::OcrSpan(s) => Some(s.t),
            Item::AudioChunk(c) => Some(c.t1),
            Item::Tick { head } => Some(*head),
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

/// Sends items to every downstream consumer with backpressure. A consumer
/// may be limited to the item kinds it declares, so a root that emits
/// frames, audio and ticks reaches each consumer with its own kinds only.
#[derive(Debug, Clone)]
pub struct Emitter {
    senders: Vec<(mpsc::Sender<Item>, Option<Vec<ItemKind>>)>,
}

impl Emitter {
    /// Emitter over the given consumer channels; every item goes to every
    /// consumer.
    pub fn new(senders: Vec<mpsc::Sender<Item>>) -> Self {
        Self {
            senders: senders.into_iter().map(|s| (s, None)).collect(),
        }
    }

    /// Emitter whose consumers each receive only the kinds listed for them.
    pub fn with_kinds(senders: Vec<(mpsc::Sender<Item>, Vec<ItemKind>)>) -> Self {
        Self {
            senders: senders
                .into_iter()
                .map(|(s, kinds)| (s, Some(kinds)))
                .collect(),
        }
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

    /// Send to every consumer that takes the item's kind; waits when any
    /// consumer's channel is full. A consumer that has gone away (its task
    /// ended) is skipped.
    pub async fn emit(&self, item: Item) -> Result<()> {
        let kind = item.kind();
        let targets: Vec<&mpsc::Sender<Item>> = self
            .senders
            .iter()
            .filter(|(_, kinds)| kinds.as_ref().is_none_or(|k| k.contains(&kind)))
            .map(|(s, _)| s)
            .collect();
        let last = targets.len().saturating_sub(1);
        let mut item = Some(item);
        for (i, s) in targets.into_iter().enumerate() {
            let it = if i == last { item.take() } else { item.clone() };
            if let Some(it) = it {
                let _ = s.send(it).await;
            }
        }
        Ok(())
    }
}

/// The stream time a stage has processed up to, shared between the stage's
/// task and the scheduler, which writes it into the checkpoint as `last_t`
/// (live jobs). Only ever moves forward.
#[derive(Debug, Default)]
pub struct StageClock {
    t: std::sync::Mutex<Option<Timestamp>>,
}

impl StageClock {
    /// Move the clock to `t` if that is later.
    pub fn advance(&self, t: Timestamp) {
        if let Ok(mut cur) = self.t.lock() {
            if cur.is_none_or(|c| c < t) {
                *cur = Some(t);
            }
        }
    }

    /// The time reached so far.
    pub fn get(&self) -> Option<Timestamp> {
        self.t.lock().ok().and_then(|c| *c)
    }
}

/// The per-hour limits of a live job, applied over the trailing hour of
/// stream time (see [`Budget::for_live`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RollingLimits {
    /// Provider spend allowed per hour of stream time; `None` is unlimited.
    pub max_cost_usd_per_hour: Option<f64>,
    /// Wall-clock seconds allowed per hour of stream time; `None` is
    /// unlimited.
    pub max_wallclock_secs_per_hour: Option<f64>,
}

/// Seconds of stream time the rolling window covers.
const ROLLING_WINDOW_SECS: f64 = 3600.0;

/// Bookkeeping for a rolling budget: where the stream head is, when it got
/// there, and what was spent at which stream time.
#[derive(Debug, Default)]
struct LiveSpend {
    /// Stream time reached, seconds.
    head_secs: f64,
    /// Spend entries `(stream secs, usd)`, oldest first.
    costs: VecDeque<(f64, f64)>,
    /// `(stream secs, wall secs since start)` each time the head advanced,
    /// oldest first.
    heads: VecDeque<(f64, f64)>,
}

/// The job's spending limits (`docs/05-indexing-pipeline.md`, budgets).
/// Provider-backed operators call [`Budget::allow_call`] before each
/// provider call; once a limit is hit no new calls are issued, what exists
/// is written, and the report lists what was skipped. A batch budget is
/// fixed from the video's duration and stays exhausted; a live budget
/// ([`Budget::for_live`]) is a rolling allowance per hour of stream time
/// and opens again as the window moves on.
#[derive(Debug)]
pub struct Budget {
    /// Cost ceiling in USD for this job; `None` means unlimited (and for a
    /// live budget, see `rolling`).
    pub max_cost_usd: Option<f64>,
    /// Wall-clock deadline for issuing provider calls.
    pub deadline: Option<Instant>,
    /// Wall-clock limit in seconds, for reports.
    pub max_wallclock_secs: Option<f64>,
    /// Live jobs: the per-hour limits applied over the trailing hour of
    /// stream time. `None` for batch budgets.
    pub rolling: Option<RollingLimits>,
    spent_micro_usd: AtomicU64,
    exhausted: AtomicBool,
    started: Instant,
    live: std::sync::Mutex<LiveSpend>,
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
            rolling: None,
            spent_micro_usd: AtomicU64::new(0),
            exhausted: AtomicBool::new(false),
            started,
            live: std::sync::Mutex::new(LiveSpend::default()),
        })
    }

    /// A budget for a live job (live C3): the policy's `max_cost_usd_per_hour`
    /// and wall-clock ceiling per hour apply to the trailing hour of stream
    /// time, so a stream is never cut off for good; a burst of spend closes
    /// the budget until enough of the stream has passed. The scheduler
    /// moves the stream head with [`Budget::advance`] on every tick. The
    /// wall-clock rule compares wall time spent with stream time covered in
    /// the same window, so a live policy sets `max_wallclock_per_hour` to
    /// `0` (unlimited) or above an hour; anything lower closes the budget
    /// whenever indexing lags the stream.
    pub fn for_live(policy: &IndexPolicy) -> Result<Self> {
        let wall = policy.max_wallclock_per_hour_secs()?;
        Ok(Self {
            max_cost_usd: None,
            deadline: None,
            max_wallclock_secs: None,
            rolling: Some(RollingLimits {
                max_cost_usd_per_hour: (policy.max_cost_usd_per_hour > 0.0)
                    .then_some(policy.max_cost_usd_per_hour),
                max_wallclock_secs_per_hour: (wall > 0.0).then_some(wall),
            }),
            spent_micro_usd: AtomicU64::new(0),
            exhausted: AtomicBool::new(false),
            started: Instant::now(),
            live: std::sync::Mutex::new(LiveSpend::default()),
        })
    }

    /// No limits.
    pub fn unlimited() -> Self {
        Self {
            max_cost_usd: None,
            deadline: None,
            max_wallclock_secs: None,
            rolling: None,
            spent_micro_usd: AtomicU64::new(0),
            exhausted: AtomicBool::new(false),
            started: Instant::now(),
            live: std::sync::Mutex::new(LiveSpend::default()),
        }
    }

    /// Record spend (at the current stream head for a live budget).
    pub fn add_cost(&self, usd: f64) {
        if usd > 0.0 {
            self.spent_micro_usd
                .fetch_add((usd * 1e6).round() as u64, Ordering::Relaxed);
            if self.rolling.is_some() {
                if let Ok(mut l) = self.live.lock() {
                    let at = l.head_secs;
                    l.costs.push_back((at, usd));
                }
            }
        }
    }

    /// Spend so far.
    pub fn spent_usd(&self) -> f64 {
        self.spent_micro_usd.load(Ordering::Relaxed) as f64 / 1e6
    }

    /// Live budgets: the stream has been decoded up to `head`. Moves the
    /// rolling window; never moves it back.
    pub fn advance(&self, head: Timestamp) {
        if self.rolling.is_none() {
            return;
        }
        let secs = head.as_secs_f64();
        let wall = self.started.elapsed().as_secs_f64();
        if let Ok(mut l) = self.live.lock() {
            if secs <= l.head_secs && !l.heads.is_empty() {
                return;
            }
            l.head_secs = secs;
            l.heads.push_back((secs, wall));
            // Keep one entry at or before the window's start for lookups.
            let floor = secs - ROLLING_WINDOW_SECS;
            while l.heads.len() > 1 && l.heads[1].0 <= floor {
                l.heads.pop_front();
            }
            while l.costs.front().is_some_and(|(t, _)| *t <= floor) {
                l.costs.pop_front();
            }
        }
    }

    /// Live budgets: the stream head as last advanced.
    pub fn stream_head_secs(&self) -> f64 {
        self.live.lock().map(|l| l.head_secs).unwrap_or(0.0)
    }

    /// Spend within the trailing window of a live budget.
    pub fn spent_in_window_usd(&self) -> f64 {
        self.live
            .lock()
            .map(|l| {
                let floor = l.head_secs - ROLLING_WINDOW_SECS;
                l.costs
                    .iter()
                    .filter(|(t, _)| *t > floor)
                    .map(|(_, c)| c)
                    .sum()
            })
            .unwrap_or(0.0)
    }

    /// Which rolling limit is exceeded right now, if any.
    fn rolling_over(&self) -> Option<&'static str> {
        let limits = self.rolling?;
        let l = self.live.lock().ok()?;
        let floor = l.head_secs - ROLLING_WINDOW_SECS;
        if let Some(max) = limits.max_cost_usd_per_hour {
            let spent: f64 = l
                .costs
                .iter()
                .filter(|(t, _)| *t > floor)
                .map(|(_, c)| c)
                .sum();
            if spent >= max {
                return Some("cost");
            }
        }
        if let Some(max) = limits.max_wallclock_secs_per_hour {
            if let Some(&(t_start, wall_start)) = l.heads.front() {
                let stream_secs = (l.head_secs - t_start).max(60.0);
                let wall_secs = self.started.elapsed().as_secs_f64() - wall_start;
                if wall_secs / stream_secs > max / ROLLING_WINDOW_SECS {
                    return Some("wallclock");
                }
            }
        }
        None
    }

    /// Whether another provider call may be issued. For a batch budget,
    /// once this returns false it stays false; a live budget opens again
    /// when the rolling window has moved past its spend.
    pub fn allow_call(&self) -> bool {
        if self.rolling.is_some() {
            return match self.rolling_over() {
                Some(reason) => {
                    if !self.exhausted.swap(true, Ordering::Relaxed) {
                        tracing::warn!(
                            spent_in_window_usd = self.spent_in_window_usd(),
                            head_secs = self.stream_head_secs(),
                            reason,
                            "live budget exhausted for the trailing hour; provider calls paused"
                        );
                    }
                    false
                }
                None => {
                    self.exhausted.store(false, Ordering::Relaxed);
                    true
                }
            };
        }
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

    /// Whether a limit is hit (for a live budget: right now).
    pub fn is_exhausted(&self) -> bool {
        if self.rolling.is_some() {
            return self.rolling_over().is_some();
        }
        self.exhausted.load(Ordering::Relaxed)
    }

    /// Which limit was hit, for reports.
    pub fn exhausted_reason(&self) -> Option<&'static str> {
        if self.rolling.is_some() {
            return self.rolling_over();
        }
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
    /// Whether the job is live (`JobOptions.live`): no cache, no duration,
    /// ticks from the root operator.
    pub live: bool,
    /// The stream time this stage has processed up to; the scheduler
    /// writes it into the checkpoint as `last_t` (live jobs). Operators
    /// may advance it themselves when they commit rows.
    pub clock: Arc<StageClock>,
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

    /// Report progress for this stage: a fraction when the total is known,
    /// the head time reached when it is not (live jobs).
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
            head: if self.live { self.clock.get() } else { None },
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
    fn live_budget_rolls_over_an_hour_of_stream_time() {
        let mut p = IndexPolicy::m0();
        p.max_cost_usd_per_hour = 2.0;
        p.max_wallclock_per_hour = "0".into();
        let b = Budget::for_live(&p).unwrap();
        assert!(b.max_cost_usd.is_none() && b.deadline.is_none());
        assert_eq!(
            b.rolling.unwrap().max_cost_usd_per_hour,
            Some(2.0),
            "{:?}",
            b.rolling
        );
        let s = |secs: i64| Timestamp::from_secs(secs);
        b.advance(s(600));
        b.add_cost(1.5);
        assert!(b.allow_call(), "under the hourly ceiling");
        b.advance(s(1200));
        b.add_cost(0.6);
        assert!(!b.allow_call(), "2.1 USD within the first simulated hour");
        assert!(b.is_exhausted());
        assert_eq!(b.exhausted_reason(), Some("cost"));
        assert!((b.spent_in_window_usd() - 2.1).abs() < 1e-9);
        assert!((b.spent_usd() - 2.1).abs() < 1e-9, "total spend is kept");
        // Still closed while the first spend is inside the trailing hour.
        b.advance(s(4100));
        assert!(!b.allow_call());
        // An hour after the first spend it drops out of the window.
        b.advance(s(4300));
        assert!(b.allow_call(), "the window moved past the 1.5 USD");
        assert!(!b.is_exhausted());
        assert!((b.spent_in_window_usd() - 0.6).abs() < 1e-9);
        assert!((b.stream_head_secs() - 4300.0).abs() < 1e-9);
        // The head never moves back.
        b.advance(s(100));
        assert!((b.stream_head_secs() - 4300.0).abs() < 1e-9);

        // No limits: always open.
        let mut free = IndexPolicy::m0();
        free.max_cost_usd_per_hour = 0.0;
        free.max_wallclock_per_hour = "0".into();
        let b = Budget::for_live(&free).unwrap();
        b.advance(s(10));
        b.add_cost(100.0);
        assert!(b.allow_call());
        assert_eq!(b.exhausted_reason(), None);

        // A wall-clock ceiling under an hour per hour closes as soon as
        // wall time outruns stream time in the window.
        let mut slow = IndexPolicy::m0();
        slow.max_cost_usd_per_hour = 0.0;
        slow.max_wallclock_per_hour = "1s".into();
        let b = Budget::for_live(&slow).unwrap();
        b.advance(s(0));
        std::thread::sleep(std::time::Duration::from_millis(30));
        b.advance(s(60));
        assert!(!b.allow_call(), "{:?}", b.exhausted_reason());
        assert_eq!(b.exhausted_reason(), Some("wallclock"));

        let clock = StageClock::default();
        assert_eq!(clock.get(), None);
        clock.advance(s(5));
        clock.advance(s(3));
        assert_eq!(clock.get(), Some(s(5)));
        clock.advance(s(9));
        assert_eq!(clock.get(), Some(s(9)));
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
