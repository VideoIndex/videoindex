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
    /// Only rows whose start time lies in this half-open range. `None`
    /// means every row. Ranking among the remaining rows is unchanged (the
    /// filter is applied before the limit, and BM25 scores rows
    /// independently). This is how a live index answers "as of `until`".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_range: Option<TimeRange>,
}

/// Exhaustive term search: every row whose text contains one of the
/// terms, grouped by video (the agent's `find_mentions` and
/// `count_mentions`). Unlike [`TextQuery`] nothing is ranked or truncated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MentionQuery {
    /// Terms; each is matched as a phrase on token boundaries, case- and
    /// diacritic-insensitively.
    pub terms: Vec<String>,
    /// Restrict to these videos; empty means all.
    pub videos: Vec<VideoId>,
    /// Which row kinds to search; empty means transcript, ocr, description.
    pub kinds: Vec<Kind>,
    /// Match the last word of each term as a token prefix, so `agent` also
    /// finds `agents` and `agentic`.
    pub prefix: bool,
    /// Earliest matching rows to return per video, term and kind; 0 for
    /// counts only.
    pub samples_per_video: usize,
    /// Only rows whose start time lies in this half-open range; counts and
    /// samples both respect it. `None` means every row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_range: Option<TimeRange>,
}

/// Rows of one kind in one video that contain a term.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MentionCount {
    /// The term as given.
    pub term: String,
    /// Row kind.
    pub kind: Kind,
    /// Matching rows (transcript spans, distinct on-screen texts per
    /// minute, descriptions).
    pub count: u64,
}

/// One matching row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MentionHit {
    /// The term as given.
    pub term: String,
    /// Row kind.
    pub kind: Kind,
    /// Start.
    pub t0: Timestamp,
    /// End (equals `t0` for point evidence).
    pub t1: Timestamp,
    /// Snippet around the match (the match in square brackets) or the row
    /// text.
    pub text: String,
}

/// A video's mentions of the queried terms.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoMentions {
    /// Video.
    pub video_id: VideoId,
    /// Title.
    pub title: Option<String>,
    /// Channel.
    pub channel: Option<String>,
    /// Counts per term and kind (only non-zero entries).
    pub counts: Vec<MentionCount>,
    /// Earliest hits, by time.
    pub samples: Vec<MentionHit>,
}

impl VideoMentions {
    /// Matching rows over every term and kind.
    pub fn total(&self) -> u64 {
        self.counts.iter().map(|c| c.count).sum()
    }
}

impl TextQuery {
    /// Search everything for `query`, top `k`.
    pub fn new(query: impl Into<String>, k: usize) -> Self {
        Self {
            query: query.into(),
            videos: Vec::new(),
            kinds: Vec::new(),
            k,
            time_range: None,
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
    /// Only targets whose start time lies in this half-open range. The
    /// vector files carry no time column, so the store over-fetches three
    /// times `k` nearest rows and drops those outside the range after
    /// resolving them; a later vector-file format may add the column. `None`
    /// means every row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_range: Option<TimeRange>,
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

/// Per-video counts for `vidx status`.
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
    /// For a live video, the head (latest decoded time); `None` otherwise.
    #[serde(default)]
    pub head: Option<Timestamp>,
    /// For a live video, the watermark; `None` otherwise.
    #[serde(default)]
    pub watermark: Option<Timestamp>,
}

/// Index-wide numbers for `vidx status`.
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
    /// Record a live video's progress: `head` (the latest decoded media
    /// time) becomes the video's `duration`, which is the head while a video
    /// is live, and `watermark` the time up to which every coarse stage has
    /// committed its rows. Called once per tick by a live indexer. Fails
    /// with `Invalid` when the video does not exist.
    async fn set_live_progress(
        &self,
        video: VideoId,
        head: Timestamp,
        watermark: Timestamp,
    ) -> Result<()> {
        let Some(mut v) = self.get_video(video).await? else {
            return Err(crate::error::IndexError::Invalid(format!(
                "no video {video}"
            )));
        };
        v.duration = head;
        v.watermark = Some(watermark);
        self.put_video(&v).await
    }
    /// Videos whose `index_state` is `live`, oldest first.
    async fn live_videos(&self) -> Result<Vec<Video>> {
        Ok(self
            .list_videos()
            .await?
            .into_iter()
            .filter(|v| v.index_state.is_live())
            .collect())
    }
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
    /// Remove the transcript spans of a track that a given operator produced
    /// (so a re-run replaces its own output and leaves imported subtitles).
    async fn delete_spans_by_operator(&self, track: TrackId, operator: &str) -> Result<u64>;
    /// Transcript and OCR spans of a video produced by an operator, in time
    /// order (used to replay a cached stage's outputs to its consumers).
    async fn spans_by_operator(&self, video: VideoId, operator: &str) -> Result<Vec<Span>>;
    /// Insert or replace descriptions.
    async fn put_descriptions(&self, d: &[Description]) -> Result<()>;
    /// Insert embedding metadata (and vectors, once a vector store exists).
    async fn put_embeddings(&self, e: &[Embedding]) -> Result<()>;
    /// Insert a provenance row.
    async fn put_provenance(&self, p: &Provenance) -> Result<ProvenanceId>;
    /// Fetch a provenance row (how a caller checks what produced a fact, or
    /// which prompt an answer was given).
    async fn get_provenance(&self, id: ProvenanceId) -> Result<Option<Provenance>> {
        Err(crate::error::IndexError::Unsupported(format!(
            "get_provenance({id}) is not implemented by this backend"
        )))
    }
    /// Stored vectors for targets under one model, in the order given
    /// (`None` where no embedding exists).
    async fn get_embeddings(
        &self,
        model: &str,
        targets: &[(TargetKind, String)],
    ) -> Result<Vec<Option<Vec<f32>>>>;
    /// Descriptions of a video's segments and frames, by time.
    async fn descriptions(&self, video: VideoId) -> Result<Vec<Description>>;
    /// Insert or replace entities with their mentions (mentions of the
    /// given entities are replaced).
    async fn put_entities(&self, entities: &[Entity], mentions: &[EntityMention]) -> Result<()>;
    /// Entities of a video.
    async fn entities(&self, video: VideoId) -> Result<Vec<Entity>>;
    /// Insert or replace events.
    async fn put_events(&self, events: &[vi_core::model::Event]) -> Result<()>;
    /// Events of a video, by time.
    async fn events(&self, video: VideoId) -> Result<Vec<vi_core::model::Event>>;
    /// Remove a video's entities (and mentions) and events, before a re-run.
    async fn delete_extractions(&self, video: VideoId) -> Result<u64>;

    // ---- search ----------------------------------------------------------

    /// BM25 full-text search.
    async fn text_search(&self, q: &TextQuery) -> Result<Vec<Hit>>;
    /// Every row containing one of the query's terms, grouped by video and
    /// ordered by total matches, most first.
    async fn find_mentions(&self, q: &MentionQuery) -> Result<Vec<VideoMentions>>;
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

    // ---- live feeds ------------------------------------------------------

    /// Transcript and OCR spans of a video from `since` on, in time order:
    /// the tail of [`Storage::time_window`] over `[since, ∞)`, so a
    /// transcript span still running at `since` is included and one that
    /// ended at or before it is not. A live events feed polls this with the
    /// previous tick's watermark as the marker. `kinds` empty means
    /// transcript and OCR; other kinds are ignored.
    async fn spans_since(
        &self,
        video: VideoId,
        kinds: &[Kind],
        since: Timestamp,
    ) -> Result<Vec<Span>> {
        let kinds: Vec<Kind> = if kinds.is_empty() {
            vec![Kind::Transcript, Kind::Ocr]
        } else {
            kinds
                .iter()
                .copied()
                .filter(|k| matches!(k, Kind::Transcript | Kind::Ocr))
                .collect()
        };
        let w = self
            .time_window(video, since, Timestamp::new(i64::MAX, 1), &kinds)
            .await?;
        let mut out: Vec<(Timestamp, Span)> = w
            .transcript
            .into_iter()
            .map(|t| (t.t0, Span::Transcript(t)))
            .chain(w.ocr.into_iter().map(|o| (o.t, Span::Ocr(o))))
            .collect();
        out.sort_by_key(|(t, _)| *t);
        Ok(out.into_iter().map(|(_, s)| s).collect())
    }
    /// Segments of a video at one level that end after `since`, by time:
    /// closed segments written since the marker and the open one still being
    /// extended.
    async fn segments_since(
        &self,
        video: VideoId,
        level: SegmentLevel,
        since: Timestamp,
    ) -> Result<Vec<Segment>> {
        Ok(self
            .segments(video, level)
            .await?
            .into_iter()
            .filter(|s| s.t1 > since)
            .collect())
    }

    // ---- blobs -----------------------------------------------------------

    /// Store a blob under a key.
    async fn put_blob(&self, key: &BlobKey, bytes: Bytes) -> Result<()>;
    /// Fetch a blob.
    async fn get_blob(&self, key: &BlobKey) -> Result<Option<Bytes>>;

    // ---- sessions ---------------------------------------------------------

    /// Store (or replace) an agent session's state, expiring after `ttl_secs`.
    async fn put_session(&self, id: &str, state: &serde_json::Value, ttl_secs: u64) -> Result<()>;
    /// Load a session's state if it exists and has not expired.
    async fn get_session(&self, id: &str) -> Result<Option<serde_json::Value>>;

    // ---- jobs ------------------------------------------------------------

    /// Persist a job checkpoint.
    async fn checkpoint(&self, job: JobId, state: &JobState) -> Result<()>;
    /// Load a job checkpoint.
    async fn load_checkpoint(&self, job: JobId) -> Result<Option<JobState>>;
    /// All job checkpoints.
    async fn list_jobs(&self) -> Result<Vec<JobState>>;

    // ---- maintenance -----------------------------------------------------

    /// Directory for operator output cache markers (`cache/` in the
    /// embedded layout); `None` for backends without a local directory.
    fn cache_dir(&self) -> Option<std::path::PathBuf> {
        None
    }
    /// The manifest.
    async fn manifest(&self) -> Result<Manifest>;
    /// Sizes and counts.
    async fn stats(&self) -> Result<IndexStats>;
    /// Vacuum, refresh manifest hashes.
    async fn compact(&self) -> Result<()>;
}
