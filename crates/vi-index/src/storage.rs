//! The [`Storage`] trait: everything the pipeline and query layers may ask
//! of a backend. See `docs/04-data-model.md`.

use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use vi_core::model::*;
use vi_core::*;

use crate::blob::BlobKey;
use crate::error::Result;
use crate::manifest::Manifest;

/// Which kinds of rows a query touches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Transcript spans.
    Transcript,
    /// OCR spans.
    Ocr,
    /// Descriptions.
    Description,
    /// Segments.
    Segment,
    /// Frame samples.
    Frame,
}

/// Kind of a search hit.
pub type HitKind = Kind;

/// A ranked text or vector hit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hit {
    /// What matched.
    pub kind: HitKind,
    /// Row id as ULID string.
    pub id: String,
    /// Owning video.
    pub video_id: VideoId,
    /// Start of the evidence.
    pub t0: Timestamp,
    /// End of the evidence (equals `t0` for point evidence).
    pub t1: Timestamp,
    /// Matched text.
    pub text: String,
    /// Higher is better.
    pub score: f64,
}

/// Full-text query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextQuery {
    /// FTS5 query string (already escaped by the caller or plain words).
    pub query: String,
    /// Restrict to these videos; empty means all.
    pub videos: Vec<VideoId>,
    /// Which row kinds to search; empty means transcript, ocr, description.
    pub kinds: Vec<Kind>,
    /// Max hits.
    pub k: usize,
}

impl TextQuery {
    /// Search everything for `query`, top `k`.
    pub fn new(query: impl Into<String>, k: usize) -> Self {
        Self {
            query: query.into(),
            videos: Vec::new(),
            kinds: Vec::new(),
            k,
        }
    }
}

/// Vector query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorQuery {
    /// Embedding model the vector came from.
    pub model: String,
    /// The query vector.
    pub vector: Vec<f32>,
    /// Restrict to these videos.
    pub videos: Vec<VideoId>,
    /// Target kinds to search.
    pub kinds: Vec<TargetKind>,
    /// Max hits.
    pub k: usize,
}

/// Everything known about a time window of one video.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Window {
    /// Segments overlapping the window.
    pub segments: Vec<Segment>,
    /// Frame samples in the window.
    pub frames: Vec<FrameSample>,
    /// Transcript spans overlapping the window.
    pub transcript: Vec<TranscriptSpan>,
    /// OCR spans in the window.
    pub ocr: Vec<OcrSpan>,
    /// Descriptions of the returned segments and frames.
    pub descriptions: Vec<Description>,
}

/// Per-video counts for `vi status`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoStats {
    /// The video.
    pub video: Video,
    /// Tracks.
    pub tracks: u64,
    /// Frame samples.
    pub frame_samples: u64,
    /// Frame samples with a pHash.
    pub hashed: u64,
    /// Frame samples with a thumbnail.
    pub thumbnails: u64,
    /// Segments.
    pub segments: u64,
    /// Transcript spans.
    pub transcript_spans: u64,
    /// OCR spans.
    pub ocr_spans: u64,
    /// Descriptions.
    pub descriptions: u64,
    /// Cost in USD from provenance rows tied to this video (M0: 0).
    pub cost_usd: f64,
}

/// Index-wide numbers for `vi status`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexStats {
    /// Manifest.
    pub manifest: Manifest,
    /// Index directory.
    pub path: String,
    /// Total bytes on disk.
    pub dir_bytes: u64,
    /// `meta.sqlite` bytes.
    pub sqlite_bytes: u64,
    /// Blob count.
    pub blob_count: u64,
    /// Blob bytes.
    pub blob_bytes: u64,
    /// Per-video stats.
    pub videos: Vec<VideoStats>,
    /// Jobs on disk.
    pub jobs: Vec<JobState>,
}

/// A storage backend.
#[async_trait]
pub trait Storage: Send + Sync {
    // ---- metadata --------------------------------------------------------

    /// Insert or replace a video.
    async fn put_video(&self, v: &Video) -> Result<()>;
    /// Fetch a video.
    async fn get_video(&self, id: VideoId) -> Result<Option<Video>>;
    /// Find a video by media content hash.
    async fn find_video_by_hash(&self, content_hash: &str) -> Result<Option<Video>>;
    /// All videos, oldest first.
    async fn list_videos(&self) -> Result<Vec<Video>>;
    /// Update a video's index state.
    async fn set_index_state(&self, id: VideoId, state: IndexState) -> Result<()>;
    /// Insert or replace tracks.
    async fn put_tracks(&self, t: &[Track]) -> Result<()>;
    /// Tracks of a video.
    async fn tracks(&self, video: VideoId) -> Result<Vec<Track>>;
    /// Remove tracks of one kind (and, by cascade, their samples and spans).
    async fn delete_tracks(&self, video: VideoId, kind: TrackKind) -> Result<u64>;
    /// Insert or replace segments.
    async fn put_segments(&self, s: &[Segment]) -> Result<()>;
    /// Remove all segments of a video at one level.
    async fn delete_segments(&self, video: VideoId, level: SegmentLevel) -> Result<u64>;
    /// Segments of a video at one level, by time.
    async fn segments(&self, video: VideoId, level: SegmentLevel) -> Result<Vec<Segment>>;
    /// Insert or replace frame samples.
    async fn put_frame_samples(&self, s: &[FrameSample]) -> Result<()>;
    /// Set pHashes.
    async fn update_frame_phash(&self, updates: &[(FrameSampleId, u64)]) -> Result<()>;
    /// Set thumbnail blob keys.
    async fn update_frame_thumbnail(&self, updates: &[(FrameSampleId, BlobKey)]) -> Result<()>;
    /// Remove all frame samples of a track (before re-sampling).
    async fn delete_frame_samples(&self, track: TrackId) -> Result<u64>;
    /// Frame samples of a track, optionally within a range, by time.
    async fn frame_samples(
        &self,
        track: TrackId,
        range: Option<TimeRange>,
    ) -> Result<Vec<FrameSample>>;
    /// Insert or replace transcript and OCR spans.
    async fn put_spans(&self, s: &[Span]) -> Result<()>;
    /// Insert or replace descriptions.
    async fn put_descriptions(&self, d: &[Description]) -> Result<()>;
    /// Insert embedding metadata (and vectors, once a vector store exists).
    async fn put_embeddings(&self, e: &[Embedding]) -> Result<()>;
    /// Insert a provenance row.
    async fn put_provenance(&self, p: &Provenance) -> Result<ProvenanceId>;

    // ---- search ----------------------------------------------------------

    /// BM25 full-text search.
    async fn text_search(&self, q: &TextQuery) -> Result<Vec<Hit>>;
    /// Nearest-neighbour search.
    async fn vector_search(&self, q: &VectorQuery) -> Result<Vec<Hit>>;
    /// Everything in `[t0, t1)` of a video.
    async fn time_window(
        &self,
        video: VideoId,
        t0: Timestamp,
        t1: Timestamp,
        kinds: &[Kind],
    ) -> Result<Window>;

    // ---- blobs -----------------------------------------------------------

    /// Store a blob under a key.
    async fn put_blob(&self, key: &BlobKey, bytes: Bytes) -> Result<()>;
    /// Fetch a blob.
    async fn get_blob(&self, key: &BlobKey) -> Result<Option<Bytes>>;

    // ---- jobs ------------------------------------------------------------

    /// Persist a job checkpoint.
    async fn checkpoint(&self, job: JobId, state: &JobState) -> Result<()>;
    /// Load a job checkpoint.
    async fn load_checkpoint(&self, job: JobId) -> Result<Option<JobState>>;
    /// All job checkpoints.
    async fn list_jobs(&self) -> Result<Vec<JobState>>;

    // ---- maintenance -----------------------------------------------------

    /// The manifest.
    async fn manifest(&self) -> Result<Manifest>;
    /// Sizes and counts.
    async fn stats(&self) -> Result<IndexStats>;
    /// Vacuum, refresh manifest hashes.
    async fn compact(&self) -> Result<()>;
}
