# 04. Data model

## Time model

Every timestamp in the index is a `Timestamp { num: i64, den: u32 }` rational in the source Track's timebase, plus a derived `f64` seconds column for indexing and display. Rationals avoid drift over hour-long videos with odd frame rates (29.97, 23.976). Wall-clock time, when known from container metadata, is stored once per Video as `start_wallclock` so events can be placed on a calendar without touching every row.

Ranges are half-open: `[t0, t1)`.

## Entities

```mermaid
erDiagram
    VIDEO ||--o{ TRACK : has
    VIDEO ||--o{ SEGMENT : has
    SEGMENT ||--o{ SEGMENT : contains
    TRACK ||--o{ FRAME_SAMPLE : yields
    TRACK ||--o{ TRANSCRIPT_SPAN : yields
    FRAME_SAMPLE ||--o{ OCR_SPAN : yields
    SEGMENT ||--o{ DESCRIPTION : described_by
    FRAME_SAMPLE ||--o{ DESCRIPTION : described_by
    VIDEO ||--o{ ENTITY : mentions
    ENTITY ||--o{ ENTITY_MENTION : at
    VIDEO ||--o{ EVENT : has
    DESCRIPTION ||--o{ EMBEDDING : embedded_as
    TRANSCRIPT_SPAN ||--o{ EMBEDDING : embedded_as
    OCR_SPAN ||--o{ EMBEDDING : embedded_as
    FRAME_SAMPLE ||--o{ EMBEDDING : embedded_as
    PROVENANCE ||--o{ DESCRIPTION : produced
    PROVENANCE ||--o{ TRANSCRIPT_SPAN : produced
    PROVENANCE ||--o{ OCR_SPAN : produced
    PROVENANCE ||--o{ SEGMENT : produced
    PROVENANCE ||--o{ EMBEDDING : produced
```

### Video
| Field | Type | Notes |
|---|---|---|
| id | ULID | Stable across re-indexing of the same content |
| source_uri | text | Original location |
| content_hash | blake3 | The **identity hash**: of the media file for a file; for a stream, of `"live:" + source key + start time` (`vi_core::model::live_identity_hash`, start as RFC 3339 UTC at whole seconds), so re-attaching to a running stream finds its row. Drives caching, dedup and `find_video_by_hash` |
| title, description, channel, published_at | text/ts | From container or yt-dlp metadata |
| duration | Timestamp | For a live video, the head: the latest decoded time, which grows |
| start_wallclock | ts, nullable | Container metadata; for a stream, the programme date-time or ingest start |
| probe | JSON | ffprobe-equivalent output; `probe["live"]` names the source and store for a stream |
| index_state | enum | `acquired`, `coarse`, `fine`, `failed`, `live` (schema v3: coarse rows still arriving; readers treat it as coarse) |
| watermark | Timestamp, nullable | Live only: the time up to which every coarse operator has committed its rows; answers read below it. `NULL` for batch videos (schema v3) |
| live_ended_at | ts, nullable | Live only: when the stream ended; `NULL` while live (schema v3) |

A live video reports progress through `Event::LiveProgress { video, head, watermark, lag_by_stage }` on the event bus rather than a fraction; `vidx status` prints `live (head HH:MM:SS, watermark HH:MM:SS)` and its `--json` carries `head` and `watermark` per video. When the stream ends, the video moves to `coarse` and then `fine` like any other.

### Track
`id, video_id, kind (video|audio|subtitle), stream_index, codec, timebase, width, height, fps, sample_rate, channels, language`.

### Segment
The retrieval unit. A hierarchy: `shot` (visual cut boundaries), `scene` (grouped shots, typically 20 s to 3 min), `chapter` (from chapters metadata, slide titles, or topic shifts, typically minutes). Every level spans the whole video with no gaps.

`id, video_id, level, parent_id, t0, t1, keyframe_sample_id, title (nullable), summary (nullable), provenance_id`.

### FrameSample
`id, track_id, t, pts, is_keyframe, phash (u64), thumbnail_blob (nullable), width, height`. Only sampled frames exist here. Full frames are never stored; they are re-decoded on demand.

### TranscriptSpan
`id, track_id, t0, t1, text, speaker (nullable), language, confidence, words (JSON, word-level timings when available), provenance_id`.

### OcrSpan
`id, frame_sample_id, t, text, bbox (x, y, w, h normalized), confidence, provenance_id`. Consecutive identical OCR text across frames is collapsed into a range on a derived view, not duplicated in storage.

### Description
Text a VLM produced about a Segment or a FrameSample. `id, target_kind (segment|frame), target_id, kind (caption|summary|qa|structured), text, structured (JSON, nullable), provenance_id`. Several Descriptions can exist for the same target from different providers, which is what enables A/B evaluation on one index.

### Entity and EntityMention
`Entity: id, video_id, kind (person|object|text|place|concept), name, canonical_name, attributes (JSON)`.
`EntityMention: entity_id, t0, t1, source_kind (transcript|ocr|description), source_id, confidence`.

### Event
`id, video_id, t0, t1, text, participants (entity ids), provenance_id`. Extracted from Descriptions and transcript by an LLM operator.

### Embedding
`id, target_kind, target_id, model, dim, vector`. Stored in the vector store, referenced from SQLite by `(target_kind, target_id, model)`. Multiple models can coexist.

### Provenance
`id, operator, operator_version, provider, model, model_version, prompt_hash, params (JSON), created_at, cost_usd, tokens_in, tokens_out, latency_ms`. Every derived row points at exactly one Provenance. Cost roll-ups per video and per configuration are a single aggregate over this table.

## Storage trait

```rust
#[async_trait]
pub trait Storage: Send + Sync {
    // metadata
    async fn put_video(&self, v: &Video) -> Result<()>;                 // upsert; VideoId is stable per content hash
    async fn get_video(&self, id: VideoId) -> Result<Option<Video>>;
    async fn find_video_by_hash(&self, content_hash: &str) -> Result<Option<Video>>;
    async fn list_videos(&self) -> Result<Vec<Video>>;
    async fn set_index_state(&self, id: VideoId, state: IndexState) -> Result<()>;
    async fn put_tracks(&self, t: &[Track]) -> Result<()>;
    async fn tracks(&self, video: VideoId) -> Result<Vec<Track>>;
    async fn put_segments(&self, s: &[Segment]) -> Result<()>;
    async fn put_frame_samples(&self, s: &[FrameSample]) -> Result<()>;
    async fn update_frame_phash(&self, updates: &[(FrameSampleId, u64)]) -> Result<()>;
    async fn update_frame_thumbnail(&self, updates: &[(FrameSampleId, BlobKey)]) -> Result<()>;
    async fn delete_frame_samples(&self, track: TrackId) -> Result<u64>;
    async fn frame_samples(&self, track: TrackId, range: Option<TimeRange>) -> Result<Vec<FrameSample>>;
    async fn put_spans(&self, s: &[Span]) -> Result<()>;               // transcript + ocr
    async fn delete_spans_by_operator(&self, track: TrackId, operator: &str) -> Result<u64>; // re-runs replace their own output
    async fn spans_by_operator(&self, video: VideoId, operator: &str) -> Result<Vec<Span>>;  // replay of cached stages
    async fn put_descriptions(&self, d: &[Description]) -> Result<()>;
    async fn put_embeddings(&self, e: &[Embedding]) -> Result<()>;
    async fn put_provenance(&self, p: &Provenance) -> Result<ProvenanceId>;
    async fn get_embeddings(&self, model: &str, targets: &[(TargetKind, String)]) -> Result<Vec<Option<Vec<f32>>>>;
    async fn descriptions(&self, video: VideoId) -> Result<Vec<Description>>;
    async fn put_entities(&self, e: &[Entity], m: &[EntityMention]) -> Result<()>;
    async fn entities(&self, video: VideoId) -> Result<Vec<Entity>>;
    async fn put_events(&self, e: &[Event]) -> Result<()>;
    async fn events(&self, video: VideoId) -> Result<Vec<Event>>;
    async fn delete_extractions(&self, video: VideoId) -> Result<u64>;
    // search
    async fn text_search(&self, q: &TextQuery) -> Result<Vec<Hit>>;    // BM25 / FTS
    async fn vector_search(&self, q: &VectorQuery) -> Result<Vec<Hit>>;
    async fn time_window(&self, video: VideoId, t0: Ts, t1: Ts, kinds: &[Kind]) -> Result<Window>;
    // blobs
    async fn put_blob(&self, key: &BlobKey, bytes: Bytes) -> Result<()>;
    async fn get_blob(&self, key: &BlobKey) -> Result<Option<Bytes>>;
    // sessions (agent conversations, TTL)
    async fn put_session(&self, id: &str, state: &serde_json::Value, ttl_secs: u64) -> Result<()>;
    async fn get_session(&self, id: &str) -> Result<Option<serde_json::Value>>;
    // jobs
    async fn checkpoint(&self, job: JobId, state: &JobState) -> Result<()>;
    async fn load_checkpoint(&self, job: JobId) -> Result<Option<JobState>>;
    async fn list_jobs(&self) -> Result<Vec<JobState>>;
    // maintenance
    async fn manifest(&self) -> Result<Manifest>;
    async fn stats(&self) -> Result<IndexStats>;                        // sizes and per-video counts for `vidx status`
    async fn compact(&self) -> Result<()>;                              // VACUUM + refresh manifest hashes
}
```

Implementations:

| Backend | Metadata + FTS | Vectors | Blobs | Use |
|---|---|---|---|---|
| **Embedded** (default) | SQLite + FTS5 | Flat memory-mapped files per model (exact search); Lance or usearch later for approximate search | Files under `blobs/` | Local SDK, CLI, single-node server |
| **Postgres** | Postgres + tsvector | pgvector | S3/GCS/R2 | Hosted, multi-tenant |
| **Qdrant** | Postgres or SQLite | Qdrant | S3/GCS/R2 | Hosted at larger scale |

Only the embedded backend ships in v1. The trait exists from day one so the pipeline and query layers never touch SQLite directly. The trait is implemented in `vi-index`; the record types live in `vi-core::model` so `vi-media` and `vi-pipeline` share them without depending on the storage crate.

Writes to parent tables (videos, tracks, frame samples, segments) are upserts. `INSERT OR REPLACE` would delete and re-insert the row and the `ON DELETE CASCADE` constraints would silently drop every child row (see the decisions log in `vi_internal`).

## Embedded index directory layout

```
myindex.vidx/
  manifest.json           schema_version, created_by, index ids, content hashes, optional signature
  meta.sqlite             all tables above, FTS5 virtual tables for spans and descriptions
  vectors/
    <model-name>.vec      normalised f32 rows, append-only, memory-mapped for exact search
    <model-name>.meta     per row: embedding id, video id, target kind, alive flag
  blobs/
    ab/cd/abcdef...       content-addressed: thumbnails (WebP), audio chunks (Opus), frame grids
  cache/
    operators/            operator output cache keyed by (content_hash, operator, version, provider, model, prompt_hash)
  jobs/
    <job-id>.json         checkpoints
```

### The live store, the index's neighbour

A stream is recorded, while it runs, into a segmented local store next to the index (`<media-cache>/live/<video-id>/`; the live modules in `videoindex-live` write it, the decoder in `vi-media` reads it). Its layout and the `index.json` schema are defined once, in `vi_media::segments::SegmentIndex`, so writer and reader cannot drift:

```
<media-cache>/live/<video-id>/
  index.json            { schema: 1, timebase: {num, den}, segments: [{ seq, file, t0, t1, bytes, wallclock?, discontinuity? }], gaps: [{t0, t1, reason?}], ended }
  seg/000001.ts …       2 s MPEG-TS segments (audio + video, codec copy)
```

- `segments` are in `seq` order and contiguous unless a range appears in `gaps`; `t0`/`t1` are `Timestamp` rationals on the stream's media timeline, `wallclock` is the source's programme date-time for `t0` when it has one, `ended` is set once the writer has closed the recording.
- **The index is the timeline.** libav reports a segment's times from that segment's own start, so the decoder places a frame or sample at in-segment PTS `p` at `t0 + (p − base)`, where `base` is the segment's first video PTS (its first audio PTS when it has no video). The in-file PTS therefore need not continue across segments; when they do not (an HLS `EXT-X-DISCONTINUITY`, an encoder restart) the writer sets `discontinuity: true` on the first segment after the break and the decoder re-anchors its audio clock there. A hole between consecutive segments (`t0` after the previous `t1`) is a gap: the times of everything after it skip the hole, and a live decode reports it as a `Gap`. `gaps` records the same holes for readers with the writer's `reason` (`discontinuity`, `window_overrun`, `fetch`, `expired`, `recovered`, `other`); both new fields are optional in the JSON and absent when false or unset, so an index written before them reads unchanged.
- Writers append a segment by writing `seg/NNNNNN.ts.tmp`, renaming it, then rewriting `index.json` through a temporary file and rename, so a reader never lists a partial segment. `SegmentIndex::covering(t0, t1)` names the segments a window decode must open.
- A truncated or unreadable `index.json` is a `protocol` error naming the file; a newer `schema` is refused like a newer index schema.
- Probes and decodes name their source through `vi_media::MediaInput`: `File { path }` or `Segments(SegmentFeed { dir, follow })`. A probe over a feed reads `index.json` and the first segment, never `stat`s a single file, and reports `duration: None` while `follow` is set (`Probe.duration` is optional for that reason; batch callers treat `None` as zero). Every decode request (`VideoDecodeRequest`, `AudioDecodeRequest`, `LiveDecodeRequest`) carries a `MediaInput`; paths convert into one, so file callers are unchanged. How the recording is decoded is in [05-indexing-pipeline](05-indexing-pipeline.md#the-recording-as-the-decode-source).
- The test fixture exists in this layout too: `vi_testkit::fixture_segments_dir()` is the 2-minute fixture cut into 60 segments, and `vi_testkit::PacedWriter` replays it into a fresh directory at any speed.

Properties:
- Copy the directory anywhere and open it. No absolute paths inside.
- `manifest.json` is the only file a reader must parse before deciding whether it can open the index. Unknown newer schema versions are refused with a clear error; older ones are migrated in place with a backup.
- `cache/` and `blobs/thumbnails` are safe to delete; they regenerate on demand from the source media if it is still reachable.
- Sizes: roughly 30 to 80 MB per hour of video, dominated by thumbnails and embeddings. Tunable by thumbnail resolution and sampling rate.

## Full-text and vector search details

- FTS5 with the `unicode61` tokenizer and prefix indexes for spans and descriptions. BM25 ranking. Tantivy is an alternative if FTS5 ranking quality proves limiting; the trait hides the choice.
- Vectors live in flat per-model files searched exactly (brute force, parallel); the `embeddings` table maps each row back to its target and the `.meta` file carries the video id and target kind so filters apply before scoring. An approximate index (Lance IVF-PQ or usearch HNSW) can replace the file behind the trait when tables grow past a few million rows.
- Temporal fusion, described in [06-query-and-agents](06-query-and-agents.md), happens above the storage layer.

## Schema versioning

The schema version is a single integer in `manifest.json` and in a `schema_meta` table. Migrations are Rust functions registered in order (`vi_index::schema::MIGRATIONS`); `schema::migrate_to` builds an index at an older version for tests. Every migration is tested against fixture indexes produced by the previous version.

| Version | Change |
|---|---|
| 1 | Initial schema |
| 2 | `embeddings.row`: an embedding knows its row in the model's vector file |
| 3 | Live videos: `videos.watermark_num`, `watermark_den`, `watermark_secs`, `live_ended_at`, all nullable; `index_state` may be `live`. A v2 index opens as v3 after a copy to `meta.sqlite.v2.bak`; a build older than v3 refuses a v3 index (`SchemaTooNew`), so hosts upgrade `vidx` before receiving one |
