//! Data-model records from `docs/04-data-model.md`. These are plain data:
//! storage backends persist them, operators produce them.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::*;
use crate::time::Timestamp;

/// How far indexing has progressed for a video.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexState {
    /// Media acquired and probed, nothing derived yet.
    Acquired,
    /// Coarse pass done: queries work.
    Coarse,
    /// Fine pass done.
    Fine,
    /// Indexing failed.
    Failed,
    /// A stream that is still being indexed while it runs: coarse rows are
    /// committed up to [`Video::watermark`], `duration` is the head and grows.
    /// Readers treat it as "coarse, still growing"; when the stream ends the
    /// video moves to `coarse` and then `fine` like a batch video.
    Live,
}

impl IndexState {
    /// Stable string form used in SQLite.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Acquired => "acquired",
            Self::Coarse => "coarse",
            Self::Fine => "fine",
            Self::Failed => "failed",
            Self::Live => "live",
        }
    }

    /// Parse the stable string form.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "acquired" => Some(Self::Acquired),
            "coarse" => Some(Self::Coarse),
            "fine" => Some(Self::Fine),
            "failed" => Some(Self::Failed),
            "live" => Some(Self::Live),
            _ => None,
        }
    }

    /// True for a video whose rows are still arriving.
    pub fn is_live(&self) -> bool {
        matches!(self, Self::Live)
    }
}

/// The identity hash of a stream: blake3 of `"live:" + source_key + start`,
/// hex, where `start` is the stream's start time as RFC 3339 in UTC at
/// whole-second precision (`2026-09-26T19:00:00Z`). It fills
/// [`Video::content_hash`] for a live video, so re-attaching to a running
/// stream with the same source key and start time finds the same row
/// through `find_video_by_hash`.
pub fn live_identity_hash(source_key: &str, start: DateTime<Utc>) -> String {
    let mut h = blake3::Hasher::new();
    h.update(b"live:");
    h.update(source_key.as_bytes());
    h.update(
        start
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
            .as_bytes(),
    );
    h.finalize().to_hex().to_string()
}

/// The indexed record of one Source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Video {
    /// Stable across re-indexing of the same content.
    pub id: VideoId,
    /// Original location.
    pub source_uri: String,
    /// The identity hash, hex: blake3 of the media file for a file, or
    /// [`live_identity_hash`] of the source key and start time for a stream
    /// (`probe["live"]` says which). Drives caching, dedup and re-attaching.
    pub content_hash: String,
    /// Title from container or sidecar metadata.
    pub title: Option<String>,
    /// Description text.
    pub description: Option<String>,
    /// Channel or uploader.
    pub channel: Option<String>,
    /// Publication time.
    pub published_at: Option<DateTime<Utc>>,
    /// Total duration; for a live video, the head (latest decoded time),
    /// which grows.
    pub duration: Timestamp,
    /// Wall-clock start when the container knows it, or the stream's
    /// programme date-time or ingest start for a live video.
    pub start_wallclock: Option<DateTime<Utc>>,
    /// ffprobe-equivalent output.
    pub probe: serde_json::Value,
    /// Indexing state.
    pub index_state: IndexState,
    /// Row creation time.
    pub created_at: DateTime<Utc>,
    /// For a live video, the media time up to which every coarse operator
    /// has committed its rows; answers read only below it. `None` for batch
    /// videos and before the first live tick.
    #[serde(default)]
    pub watermark: Option<Timestamp>,
    /// When the stream ended, once it has; `None` while live and for batch
    /// videos.
    #[serde(default)]
    pub live_ended_at: Option<DateTime<Utc>>,
}

/// Elementary stream kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    /// Video stream.
    Video,
    /// Audio stream.
    Audio,
    /// Subtitle stream (embedded or sidecar).
    Subtitle,
}

impl TrackKind {
    /// Stable string form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Subtitle => "subtitle",
        }
    }

    /// Parse the stable string form.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "video" => Some(Self::Video),
            "audio" => Some(Self::Audio),
            "subtitle" => Some(Self::Subtitle),
            _ => None,
        }
    }
}

/// One elementary stream of a Video.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    /// Id.
    pub id: TrackId,
    /// Owning video.
    pub video_id: VideoId,
    /// Kind.
    pub kind: TrackKind,
    /// Stream index in the container.
    pub stream_index: u32,
    /// Codec name.
    pub codec: String,
    /// Timebase denominator: PTS values are `pts / timebase_den * timebase_num`.
    pub timebase_num: u32,
    /// Timebase denominator.
    pub timebase_den: u32,
    /// Width in pixels (video).
    pub width: Option<u32>,
    /// Height in pixels (video).
    pub height: Option<u32>,
    /// Frames per second (video).
    pub fps: Option<f64>,
    /// Sample rate (audio).
    pub sample_rate: Option<u32>,
    /// Channel count (audio).
    pub channels: Option<u32>,
    /// BCP-47 language when known.
    pub language: Option<String>,
}

/// Segment hierarchy level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentLevel {
    /// Visual cut boundaries.
    Shot,
    /// Grouped shots, 20 s to 3 min.
    Scene,
    /// Minutes; from chapter metadata or topic shifts.
    Chapter,
}

impl SegmentLevel {
    /// Stable string form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Shot => "shot",
            Self::Scene => "scene",
            Self::Chapter => "chapter",
        }
    }

    /// Parse the stable string form.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "shot" => Some(Self::Shot),
            "scene" => Some(Self::Scene),
            "chapter" => Some(Self::Chapter),
            _ => None,
        }
    }
}

/// The retrieval unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    /// Id.
    pub id: SegmentId,
    /// Owning video.
    pub video_id: VideoId,
    /// Level.
    pub level: SegmentLevel,
    /// Parent at the next level up.
    pub parent_id: Option<SegmentId>,
    /// Start, inclusive.
    pub t0: Timestamp,
    /// End, exclusive.
    pub t1: Timestamp,
    /// Representative frame.
    pub keyframe_sample_id: Option<FrameSampleId>,
    /// Title.
    pub title: Option<String>,
    /// Summary.
    pub summary: Option<String>,
    /// Who produced it.
    pub provenance_id: ProvenanceId,
}

/// A decoded frame at a known timestamp, kept as a hash and a thumbnail.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameSample {
    /// Id.
    pub id: FrameSampleId,
    /// Owning video track.
    pub track_id: TrackId,
    /// Presentation time in seconds-rational.
    pub t: Timestamp,
    /// Raw PTS in the track timebase.
    pub pts: i64,
    /// Whether the decoder flagged it a keyframe.
    pub is_keyframe: bool,
    /// 64-bit perceptual hash.
    pub phash: Option<u64>,
    /// Blob key of the WebP thumbnail.
    pub thumbnail_blob: Option<String>,
    /// Source frame width.
    pub width: u32,
    /// Source frame height.
    pub height: u32,
}

/// A timed piece of text from speech.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscriptSpan {
    /// Id.
    pub id: SpanId,
    /// Audio or subtitle track.
    pub track_id: TrackId,
    /// Start.
    pub t0: Timestamp,
    /// End.
    pub t1: Timestamp,
    /// Text.
    pub text: String,
    /// Speaker label.
    pub speaker: Option<String>,
    /// Language.
    pub language: Option<String>,
    /// 0-1.
    pub confidence: Option<f32>,
    /// Word-level timings when available.
    pub words: Option<serde_json::Value>,
    /// Who produced it.
    pub provenance_id: ProvenanceId,
}

/// Normalised bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BBox {
    /// Left, 0-1.
    pub x: f32,
    /// Top, 0-1.
    pub y: f32,
    /// Width, 0-1.
    pub w: f32,
    /// Height, 0-1.
    pub h: f32,
}

/// A timed piece of on-screen text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrSpan {
    /// Id.
    pub id: SpanId,
    /// Frame it was read from.
    pub frame_sample_id: FrameSampleId,
    /// Time of that frame.
    pub t: Timestamp,
    /// Text.
    pub text: String,
    /// Where on the frame.
    pub bbox: Option<BBox>,
    /// 0-1.
    pub confidence: Option<f32>,
    /// Who produced it.
    pub provenance_id: ProvenanceId,
}

/// Either kind of span; what `Storage::put_spans` accepts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Span {
    /// Speech.
    Transcript(TranscriptSpan),
    /// On-screen text.
    Ocr(OcrSpan),
}

/// What a Description is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    /// A segment.
    Segment,
    /// A frame sample.
    Frame,
    /// A transcript span (embeddings only).
    TranscriptSpan,
    /// An OCR span (embeddings only).
    OcrSpan,
    /// A description (embeddings only).
    Description,
}

impl TargetKind {
    /// Stable string form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Segment => "segment",
            Self::Frame => "frame",
            Self::TranscriptSpan => "transcript_span",
            Self::OcrSpan => "ocr_span",
            Self::Description => "description",
        }
    }

    /// Parse the stable string form.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "segment" => Some(Self::Segment),
            "frame" => Some(Self::Frame),
            "transcript_span" => Some(Self::TranscriptSpan),
            "ocr_span" => Some(Self::OcrSpan),
            "description" => Some(Self::Description),
            _ => None,
        }
    }
}

/// Kind of description text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DescriptionKind {
    /// Short caption.
    Caption,
    /// Longer summary.
    Summary,
    /// Answer to a question.
    Qa,
    /// Structured JSON.
    Structured,
}

impl DescriptionKind {
    /// Stable string form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Caption => "caption",
            Self::Summary => "summary",
            Self::Qa => "qa",
            Self::Structured => "structured",
        }
    }

    /// Parse the stable string form.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "caption" => Some(Self::Caption),
            "summary" => Some(Self::Summary),
            "qa" => Some(Self::Qa),
            "structured" => Some(Self::Structured),
            _ => None,
        }
    }
}

/// Text a VLM produced about a Segment or a FrameSample.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Description {
    /// Id.
    pub id: DescriptionId,
    /// Segment or frame.
    pub target_kind: TargetKind,
    /// Id of the target, as a ULID string.
    pub target_id: String,
    /// Kind.
    pub kind: DescriptionKind,
    /// Text.
    pub text: String,
    /// Structured payload.
    pub structured: Option<serde_json::Value>,
    /// Who produced it.
    pub provenance_id: ProvenanceId,
}

/// Entity kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    /// Person.
    Person,
    /// Object.
    Object,
    /// Text.
    Text,
    /// Place.
    Place,
    /// Concept.
    Concept,
}

impl EntityKind {
    /// Stable string form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Object => "object",
            Self::Text => "text",
            Self::Place => "place",
            Self::Concept => "concept",
        }
    }
}

/// A named thing mentioned in a video.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    /// Id.
    pub id: EntityId,
    /// Owning video.
    pub video_id: VideoId,
    /// Kind.
    pub kind: EntityKind,
    /// Surface name.
    pub name: String,
    /// Canonical name after de-duplication.
    pub canonical_name: String,
    /// Attributes.
    pub attributes: serde_json::Value,
}

/// Where an entity was mentioned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityMention {
    /// Entity.
    pub entity_id: EntityId,
    /// Start.
    pub t0: Timestamp,
    /// End.
    pub t1: Timestamp,
    /// Source kind: transcript, ocr, description.
    pub source_kind: String,
    /// Source row id.
    pub source_id: String,
    /// 0-1.
    pub confidence: Option<f32>,
}

/// Something that happened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// Id.
    pub id: EventId,
    /// Owning video.
    pub video_id: VideoId,
    /// Start.
    pub t0: Timestamp,
    /// End.
    pub t1: Timestamp,
    /// Text.
    pub text: String,
    /// Participating entities.
    pub participants: Vec<EntityId>,
    /// Who produced it.
    pub provenance_id: ProvenanceId,
}

/// Reference to a vector in the vector store.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Embedding {
    /// Id.
    pub id: EmbeddingId,
    /// What it embeds.
    pub target_kind: TargetKind,
    /// Id of the target.
    pub target_id: String,
    /// Embedding model name.
    pub model: String,
    /// Dimensionality.
    pub dim: u32,
    /// The vector.
    pub vector: Vec<f32>,
    /// Who produced it.
    pub provenance_id: ProvenanceId,
}

/// Which Operator, Provider, model version, and prompt produced a fact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    /// Id.
    pub id: ProvenanceId,
    /// Operator id.
    pub operator: String,
    /// Operator version.
    pub operator_version: u32,
    /// Provider name, if any.
    pub provider: Option<String>,
    /// Model name, if any.
    pub model: Option<String>,
    /// Model version, if any.
    pub model_version: Option<String>,
    /// Hash of the prompt, if any.
    pub prompt_hash: Option<String>,
    /// Parameters.
    pub params: serde_json::Value,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Cost in USD.
    pub cost_usd: f64,
    /// Input tokens.
    pub tokens_in: u64,
    /// Output tokens.
    pub tokens_out: u64,
    /// Latency in milliseconds.
    pub latency_ms: u64,
}

impl Provenance {
    /// A provenance row for an in-process operator with no provider.
    pub fn local(operator: &str, operator_version: u32, params: serde_json::Value) -> Self {
        Self {
            id: ProvenanceId::new(),
            operator: operator.to_string(),
            operator_version,
            provider: None,
            model: None,
            model_version: None,
            prompt_hash: None,
            params,
            created_at: Utc::now(),
            cost_usd: 0.0,
            tokens_in: 0,
            tokens_out: 0,
            latency_ms: 0,
        }
    }
}

/// Status of one stage inside a job checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    /// Not started.
    Pending,
    /// In progress.
    Running,
    /// Finished.
    Complete,
    /// Failed.
    Failed,
    /// Skipped (budget, policy, or unavailable).
    Skipped,
}

/// A range of input an operator could not process (provider failure after
/// retries); the job continued past it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FailedRange {
    /// Start.
    pub t0: Timestamp,
    /// End.
    pub t1: Timestamp,
    /// Error text.
    pub error: String,
}

/// Checkpoint for one stage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StageState {
    /// Status.
    pub status: StageStatus,
    /// Items emitted so far.
    pub items_done: u64,
    /// Inputs that failed after retries; the stage continued.
    pub items_failed: u64,
    /// Inputs skipped because a budget was exhausted.
    pub items_skipped: u64,
    /// The first failures, for the report (capped).
    pub failures: Vec<FailedRange>,
    /// Whether the stage was satisfied from the operator output cache.
    pub cached: bool,
    /// Whether the stage replayed stored outputs instead of recomputing.
    pub replayed: bool,
    /// Last timestamp processed, for resumable stream operators.
    pub last_t: Option<Timestamp>,
    /// Error text if failed.
    pub error: Option<String>,
}

impl Default for StageState {
    fn default() -> Self {
        Self {
            status: StageStatus::Pending,
            items_done: 0,
            items_failed: 0,
            items_skipped: 0,
            failures: Vec::new(),
            cached: false,
            replayed: false,
            last_t: None,
            error: None,
        }
    }
}

/// Persistent state of an indexing job; what `Storage::checkpoint` stores.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobState {
    /// Job id.
    pub job_id: JobId,
    /// Video id.
    pub video_id: VideoId,
    /// Source URI.
    pub source_uri: String,
    /// Policy name.
    pub policy: String,
    /// Per-stage state.
    pub stages: BTreeMap<String, StageState>,
    /// Start time.
    pub started_at: DateTime<Utc>,
    /// Last checkpoint time.
    pub updated_at: DateTime<Utc>,
    /// Whether the job has ended.
    pub finished: bool,
}

impl JobState {
    /// A fresh job state with every stage pending.
    pub fn new(
        job_id: JobId,
        video_id: VideoId,
        source_uri: impl Into<String>,
        policy: impl Into<String>,
        stages: impl IntoIterator<Item = String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            job_id,
            video_id,
            source_uri: source_uri.into(),
            policy: policy.into(),
            stages: stages
                .into_iter()
                .map(|s| (s, StageState::default()))
                .collect(),
            started_at: now,
            updated_at: now,
            finished: false,
        }
    }

    /// Stage state, creating a pending one if missing.
    pub fn stage_mut(&mut self, stage: &str) -> &mut StageState {
        self.stages.entry(stage.to_string()).or_default()
    }

    /// Whether the given stage is recorded complete.
    pub fn is_complete(&self, stage: &str) -> bool {
        self.stages
            .get(stage)
            .is_some_and(|s| s.status == StageStatus::Complete)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_state_strings_roundtrip() {
        for s in [
            IndexState::Acquired,
            IndexState::Coarse,
            IndexState::Fine,
            IndexState::Failed,
            IndexState::Live,
        ] {
            assert_eq!(IndexState::parse(s.as_str()), Some(s));
        }
        assert_eq!(IndexState::parse("live"), Some(IndexState::Live));
        assert!(IndexState::Live.is_live() && !IndexState::Coarse.is_live());
        assert_eq!(
            serde_json::to_string(&IndexState::Live).unwrap(),
            "\"live\""
        );
    }

    #[test]
    fn live_identity_hash_is_stable_and_keyed() {
        let start = DateTime::parse_from_rfc3339("2026-09-26T19:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let a = live_identity_hash("youtube:abc", start);
        let b = live_identity_hash("youtube:abc", start);
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
        assert_ne!(a, live_identity_hash("youtube:abd", start));
        assert_ne!(
            a,
            live_identity_hash("youtube:abc", start + chrono::Duration::seconds(1))
        );
        // Sub-second differences do not change the identity.
        assert_eq!(
            a,
            live_identity_hash("youtube:abc", start + chrono::Duration::milliseconds(400))
        );
        assert_eq!(
            a,
            blake3::hash(b"live:youtube:abc2026-09-26T19:00:00Z")
                .to_hex()
                .to_string()
        );
    }

    #[test]
    fn video_live_fields_default_when_absent() {
        let v: Video = serde_json::from_value(serde_json::json!({
            "id": VideoId::new(),
            "source_uri": "file:///a.mp4",
            "content_hash": "h",
            "title": null, "description": null, "channel": null, "published_at": null,
            "duration": {"num": 10, "den": 1},
            "start_wallclock": null,
            "probe": {},
            "index_state": "coarse",
            "created_at": Utc::now(),
        }))
        .unwrap();
        assert_eq!(v.watermark, None);
        assert_eq!(v.live_ended_at, None);
    }
}
