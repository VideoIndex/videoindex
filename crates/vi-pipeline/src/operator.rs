//! The operator contract from `docs/05-indexing-pipeline.md`.

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use vi_core::config::{Config, IndexPolicy, WorkerConfig};
use vi_core::model::{FrameSample, Track, Video};
use vi_core::{Error, Event, EventBus, JobId, Progress, Result, VideoId};
use vi_index::{BlobKey, Storage};
use vi_media::{Acquired, FrameBuffer, Probe};

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
    /// A hashed frame sample.
    Hashed {
        /// Sample id.
        sample: vi_core::FrameSampleId,
        /// The hash.
        phash: u64,
        /// Time, for downstream grouping.
        t: vi_core::Timestamp,
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
            cost_usd: 0.0,
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
    /// Cost estimate.
    fn cost_estimate(&self, input: &InputSummary) -> CostEstimate;
    /// Process one input item, emitting through `ctx`.
    async fn run(&self, ctx: &OpContext, input: OpInput) -> Result<OpOutput>;
    /// Called once after the last input; flush buffered writes.
    async fn finish(&self, _ctx: &OpContext) -> Result<OpOutput> {
        Ok(OpOutput::default())
    }
}
