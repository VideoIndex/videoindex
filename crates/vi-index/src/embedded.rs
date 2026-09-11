//! The embedded backend: one directory with SQLite metadata, FTS5, blobs,
//! job checkpoints, and a manifest.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use tracing::debug;
use vi_core::model::*;
use vi_core::*;

use crate::blob::{BlobKey, BlobStore};
use crate::error::{IndexError, Result};
use crate::manifest::{FileHash, Manifest, MANIFEST_FILE};
use crate::schema;
use crate::storage::*;
use crate::vectors::VectorStore;

/// SQLite file name.
pub const SQLITE_FILE: &str = "meta.sqlite";

/// The embedded index.
#[derive(Debug, Clone)]
pub struct EmbeddedIndex {
    dir: PathBuf,
    conn: Arc<Mutex<Connection>>,
    blobs: BlobStore,
    vectors: VectorStore,
}

impl EmbeddedIndex {
    /// Create a new index directory. Fails if a manifest already exists.
    pub fn create(dir: &Path) -> Result<Self> {
        if dir.join(MANIFEST_FILE).exists() {
            return Err(IndexError::AlreadyExists(dir.display().to_string()));
        }
        std::fs::create_dir_all(dir)?;
        for sub in ["blobs", "vectors", "cache/operators", "jobs"] {
            std::fs::create_dir_all(dir.join(sub))?;
        }
        let conn = open_conn(&dir.join(SQLITE_FILE))?;
        schema::migrate(&conn)?;
        let mut manifest = Manifest::new();
        manifest.files = file_hashes(dir)?;
        manifest.write(dir)?;
        Ok(Self::assemble(dir, conn))
    }

    /// Open an existing index, migrating older schemas in place.
    pub fn open(dir: &Path) -> Result<Self> {
        let manifest = Manifest::read(dir)?;
        let conn = open_conn(&dir.join(SQLITE_FILE))?;
        let before = schema::current_version(&conn)?;
        if before < crate::SCHEMA_VERSION {
            let backup = dir.join(format!("{SQLITE_FILE}.v{before}.bak"));
            std::fs::copy(dir.join(SQLITE_FILE), &backup)?;
            debug!(
                "migrating schema {before} -> {}; backup at {}",
                crate::SCHEMA_VERSION,
                backup.display()
            );
        }
        schema::migrate(&conn)?;
        if manifest.schema_version != crate::SCHEMA_VERSION {
            let mut m = manifest;
            m.schema_version = crate::SCHEMA_VERSION;
            m.updated_at = Utc::now();
            m.write(dir)?;
        }
        Ok(Self::assemble(dir, conn))
    }

    /// Open if present, else create.
    pub fn open_or_create(dir: &Path) -> Result<Self> {
        if dir.join(MANIFEST_FILE).is_file() {
            Self::open(dir)
        } else {
            Self::create(dir)
        }
    }

    fn assemble(dir: &Path, conn: Connection) -> Self {
        Self {
            dir: dir.to_path_buf(),
            conn: Arc::new(Mutex::new(conn)),
            blobs: BlobStore::new(dir.join("blobs")),
            vectors: VectorStore::new(dir.join("vectors")),
        }
    }

    /// Index directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The blob store.
    pub fn blobs(&self) -> &BlobStore {
        &self.blobs
    }

    /// The vector store.
    pub fn vectors(&self) -> &VectorStore {
        &self.vectors
    }

    /// Run a blocking closure against the connection on the blocking pool.
    async fn with_conn<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> Result<T> + Send + 'static,
    {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let guard = conn
                .lock()
                .map_err(|_| IndexError::Corrupt("connection mutex poisoned".into()))?;
            f(&guard)
        })
        .await?
    }

    fn jobs_dir(&self) -> PathBuf {
        self.dir.join("jobs")
    }
}

fn open_conn(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA temp_store = MEMORY;
         PRAGMA cache_size = -65536;",
    )?;
    Ok(conn)
}

fn file_hashes(dir: &Path) -> Result<std::collections::BTreeMap<String, FileHash>> {
    let mut files = std::collections::BTreeMap::new();
    let sqlite = dir.join(SQLITE_FILE);
    if sqlite.is_file() {
        let mut hasher = blake3::Hasher::new();
        let mut f = std::fs::File::open(&sqlite)?;
        let size = std::io::copy(&mut f, &mut hasher)?;
        files.insert(
            SQLITE_FILE.to_string(),
            FileHash {
                blake3: hasher.finalize().to_hex().to_string(),
                size,
            },
        );
    }
    Ok(files)
}

fn dir_size(dir: &Path) -> Result<u64> {
    let mut total = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            if ft.is_dir() {
                stack.push(entry.path());
            } else if ft.is_file() {
                total += entry.metadata()?.len();
            }
        }
    }
    Ok(total)
}

// ---------------------------------------------------------------- rows ---

fn ts(num: i64, den: i64) -> Timestamp {
    Timestamp::new(num, u32::try_from(den).unwrap_or(1).max(1))
}

fn parse_id<T: std::str::FromStr>(s: String, what: &str) -> Result<T> {
    s.parse()
        .map_err(|_| IndexError::Corrupt(format!("bad {what} id '{s}'")))
}

fn parse_dt(s: Option<String>) -> Result<Option<DateTime<Utc>>> {
    match s {
        None => Ok(None),
        Some(s) => DateTime::parse_from_rfc3339(&s)
            .map(|d| Some(d.with_timezone(&Utc)))
            .map_err(|e| IndexError::Corrupt(format!("bad timestamp '{s}': {e}"))),
    }
}

fn video_from_row(r: &Row<'_>) -> Result<Video> {
    let probe: String = r.get("probe")?;
    let state: String = r.get("index_state")?;
    Ok(Video {
        id: parse_id(r.get("id")?, "video")?,
        source_uri: r.get("source_uri")?,
        content_hash: r.get("content_hash")?,
        title: r.get("title")?,
        description: r.get("description")?,
        channel: r.get("channel")?,
        published_at: parse_dt(r.get("published_at")?)?,
        duration: ts(r.get("duration_num")?, r.get("duration_den")?),
        start_wallclock: parse_dt(r.get("start_wallclock")?)?,
        probe: serde_json::from_str(&probe)?,
        index_state: IndexState::parse(&state)
            .ok_or_else(|| IndexError::Corrupt(format!("bad index_state '{state}'")))?,
        created_at: parse_dt(r.get("created_at")?)?.unwrap_or_else(Utc::now),
    })
}

const VIDEO_COLS: &str = "id, source_uri, content_hash, title, description, channel, published_at, duration_num, duration_den, start_wallclock, probe, index_state, created_at";

fn track_from_row(r: &Row<'_>) -> Result<Track> {
    let kind: String = r.get("kind")?;
    Ok(Track {
        id: parse_id(r.get("id")?, "track")?,
        video_id: parse_id(r.get("video_id")?, "video")?,
        kind: TrackKind::parse(&kind)
            .ok_or_else(|| IndexError::Corrupt(format!("bad track kind '{kind}'")))?,
        stream_index: r.get("stream_index")?,
        codec: r.get("codec")?,
        timebase_num: r.get("timebase_num")?,
        timebase_den: r.get("timebase_den")?,
        width: r.get("width")?,
        height: r.get("height")?,
        fps: r.get("fps")?,
        sample_rate: r.get("sample_rate")?,
        channels: r.get("channels")?,
        language: r.get("language")?,
    })
}

fn frame_from_row(r: &Row<'_>) -> Result<FrameSample> {
    let phash: Option<i64> = r.get("phash")?;
    Ok(FrameSample {
        id: parse_id(r.get("id")?, "frame_sample")?,
        track_id: parse_id(r.get("track_id")?, "track")?,
        t: ts(r.get("t_num")?, r.get("t_den")?),
        pts: r.get("pts")?,
        is_keyframe: r.get::<_, i64>("is_keyframe")? != 0,
        phash: phash.map(|v| v as u64),
        thumbnail_blob: r.get("thumbnail_blob")?,
        width: r.get("width")?,
        height: r.get("height")?,
    })
}

fn segment_from_row(r: &Row<'_>) -> Result<Segment> {
    let level: String = r.get("level")?;
    let parent: Option<String> = r.get("parent_id")?;
    let kf: Option<String> = r.get("keyframe_sample_id")?;
    Ok(Segment {
        id: parse_id(r.get("id")?, "segment")?,
        video_id: parse_id(r.get("video_id")?, "video")?,
        level: SegmentLevel::parse(&level)
            .ok_or_else(|| IndexError::Corrupt(format!("bad level '{level}'")))?,
        parent_id: parent.map(|p| parse_id(p, "segment")).transpose()?,
        t0: ts(r.get("t0_num")?, r.get("t0_den")?),
        t1: ts(r.get("t1_num")?, r.get("t1_den")?),
        keyframe_sample_id: kf.map(|p| parse_id(p, "frame_sample")).transpose()?,
        title: r.get("title")?,
        summary: r.get("summary")?,
        provenance_id: parse_id(r.get("provenance_id")?, "provenance")?,
    })
}

fn transcript_from_row(r: &Row<'_>) -> Result<TranscriptSpan> {
    let words: Option<String> = r.get("words")?;
    Ok(TranscriptSpan {
        id: parse_id(r.get("id")?, "span")?,
        track_id: parse_id(r.get("track_id")?, "track")?,
        t0: ts(r.get("t0_num")?, r.get("t0_den")?),
        t1: ts(r.get("t1_num")?, r.get("t1_den")?),
        text: r.get("text")?,
        speaker: r.get("speaker")?,
        language: r.get("language")?,
        confidence: r.get("confidence")?,
        words: words.map(|w| serde_json::from_str(&w)).transpose()?,
        provenance_id: parse_id(r.get("provenance_id")?, "provenance")?,
    })
}

fn ocr_from_row(r: &Row<'_>) -> Result<OcrSpan> {
    let bx: Option<f32> = r.get("bbox_x")?;
    let bbox = match bx {
        Some(x) => Some(BBox {
            x,
            y: r.get::<_, Option<f32>>("bbox_y")?.unwrap_or(0.0),
            w: r.get::<_, Option<f32>>("bbox_w")?.unwrap_or(0.0),
            h: r.get::<_, Option<f32>>("bbox_h")?.unwrap_or(0.0),
        }),
        None => None,
    };
    Ok(OcrSpan {
        id: parse_id(r.get("id")?, "span")?,
        frame_sample_id: parse_id(r.get("frame_sample_id")?, "frame_sample")?,
        t: ts(r.get("t_num")?, r.get("t_den")?),
        text: r.get("text")?,
        bbox,
        confidence: r.get("confidence")?,
        provenance_id: parse_id(r.get("provenance_id")?, "provenance")?,
    })
}

fn description_from_row(r: &Row<'_>) -> Result<Description> {
    let tk: String = r.get("target_kind")?;
    let kind: String = r.get("kind")?;
    let structured: Option<String> = r.get("structured")?;
    Ok(Description {
        id: parse_id(r.get("id")?, "description")?,
        target_kind: TargetKind::parse(&tk)
            .ok_or_else(|| IndexError::Corrupt(format!("bad target_kind '{tk}'")))?,
        target_id: r.get("target_id")?,
        kind: DescriptionKind::parse(&kind)
            .ok_or_else(|| IndexError::Corrupt(format!("bad description kind '{kind}'")))?,
        text: r.get("text")?,
        structured: structured.map(|s| serde_json::from_str(&s)).transpose()?,
        provenance_id: parse_id(r.get("provenance_id")?, "provenance")?,
    })
}

fn video_filter(videos: &[VideoId], column: &str) -> (String, Vec<String>) {
    if videos.is_empty() {
        return (String::new(), Vec::new());
    }
    let placeholders = std::iter::repeat_n("?", videos.len())
        .collect::<Vec<_>>()
        .join(",");
    (
        format!(" AND {column} IN ({placeholders})"),
        videos.iter().map(ToString::to_string).collect(),
    )
}

/// Turn free text into a safe FTS5 query: each word becomes a quoted phrase
/// token, joined by implicit AND, with a prefix match on the last word.
pub fn fts_query(input: &str) -> String {
    let words: Vec<String> = input
        .split_whitespace()
        .map(|w| w.replace('"', "\"\""))
        .filter(|w| !w.is_empty())
        .collect();
    let n = words.len();
    words
        .iter()
        .enumerate()
        .map(|(i, w)| {
            if i + 1 == n {
                format!("\"{w}\"*")
            } else {
                format!("\"{w}\"")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ------------------------------------------------------------- storage ---

#[async_trait]
impl Storage for EmbeddedIndex {
    async fn put_video(&self, v: &Video) -> Result<()> {
        let v = v.clone();
        self.with_conn(move |c| {
            c.execute(
                &format!("INSERT INTO videos({VIDEO_COLS}, duration_secs) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)
                     ON CONFLICT(id) DO UPDATE SET
                       source_uri = excluded.source_uri, content_hash = excluded.content_hash,
                       title = excluded.title, description = excluded.description, channel = excluded.channel,
                       published_at = excluded.published_at, duration_num = excluded.duration_num,
                       duration_den = excluded.duration_den, duration_secs = excluded.duration_secs,
                       start_wallclock = excluded.start_wallclock, probe = excluded.probe,
                       index_state = excluded.index_state"),
                params![
                    v.id.to_string(),
                    v.source_uri,
                    v.content_hash,
                    v.title,
                    v.description,
                    v.channel,
                    v.published_at.map(|d| d.to_rfc3339()),
                    v.duration.num,
                    v.duration.den,
                    v.start_wallclock.map(|d| d.to_rfc3339()),
                    serde_json::to_string(&v.probe)?,
                    v.index_state.as_str(),
                    v.created_at.to_rfc3339(),
                    v.duration.as_secs_f64(),
                ],
            )?;
            Ok(())
        })
        .await
    }

    async fn get_video(&self, id: VideoId) -> Result<Option<Video>> {
        self.with_conn(move |c| {
            c.query_row(
                &format!("SELECT {VIDEO_COLS} FROM videos WHERE id = ?1"),
                [id.to_string()],
                |r| Ok(video_from_row(r)),
            )
            .optional()?
            .transpose()
        })
        .await
    }

    async fn find_video_by_hash(&self, content_hash: &str) -> Result<Option<Video>> {
        let h = content_hash.to_string();
        self.with_conn(move |c| {
            c.query_row(
                &format!("SELECT {VIDEO_COLS} FROM videos WHERE content_hash = ?1 ORDER BY created_at LIMIT 1"),
                [h],
                |r| Ok(video_from_row(r)),
            )
            .optional()?
            .transpose()
        })
        .await
    }

    async fn list_videos(&self) -> Result<Vec<Video>> {
        self.with_conn(|c| {
            let mut st = c.prepare(&format!(
                "SELECT {VIDEO_COLS} FROM videos ORDER BY created_at, id"
            ))?;
            let rows = st.query_map([], |r| Ok(video_from_row(r)))?;
            rows.map(|r| r?).collect()
        })
        .await
    }

    async fn set_index_state(&self, id: VideoId, state: IndexState) -> Result<()> {
        self.with_conn(move |c| {
            c.execute(
                "UPDATE videos SET index_state = ?2 WHERE id = ?1",
                params![id.to_string(), state.as_str()],
            )?;
            Ok(())
        })
        .await
    }

    async fn put_tracks(&self, t: &[Track]) -> Result<()> {
        let t = t.to_vec();
        self.with_conn(move |c| {
            let tx = c.unchecked_transaction()?;
            {
                let mut st = tx.prepare_cached(
                    "INSERT INTO tracks(id, video_id, kind, stream_index, codec, timebase_num, timebase_den, width, height, fps, sample_rate, channels, language)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
                     ON CONFLICT(id) DO UPDATE SET
                       video_id = excluded.video_id, kind = excluded.kind, stream_index = excluded.stream_index,
                       codec = excluded.codec, timebase_num = excluded.timebase_num, timebase_den = excluded.timebase_den,
                       width = excluded.width, height = excluded.height, fps = excluded.fps,
                       sample_rate = excluded.sample_rate, channels = excluded.channels, language = excluded.language",
                )?;
                for tr in &t {
                    st.execute(params![
                        tr.id.to_string(),
                        tr.video_id.to_string(),
                        tr.kind.as_str(),
                        tr.stream_index,
                        tr.codec,
                        tr.timebase_num,
                        tr.timebase_den,
                        tr.width,
                        tr.height,
                        tr.fps,
                        tr.sample_rate,
                        tr.channels,
                        tr.language,
                    ])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
        .await
    }

    async fn tracks(&self, video: VideoId) -> Result<Vec<Track>> {
        self.with_conn(move |c| {
            let mut st =
                c.prepare_cached("SELECT * FROM tracks WHERE video_id = ?1 ORDER BY stream_index")?;
            let rows = st.query_map([video.to_string()], |r| Ok(track_from_row(r)))?;
            rows.map(|r| r?).collect()
        })
        .await
    }

    async fn put_segments(&self, s: &[Segment]) -> Result<()> {
        let s = s.to_vec();
        self.with_conn(move |c| {
            let tx = c.unchecked_transaction()?;
            {
                let mut st = tx.prepare_cached(
                    "INSERT INTO segments(id, video_id, level, parent_id, t0_num, t0_den, t0_secs, t1_num, t1_den, t1_secs, keyframe_sample_id, title, summary, provenance_id)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)
                     ON CONFLICT(id) DO UPDATE SET
                       video_id = excluded.video_id, level = excluded.level, parent_id = excluded.parent_id,
                       t0_num = excluded.t0_num, t0_den = excluded.t0_den, t0_secs = excluded.t0_secs,
                       t1_num = excluded.t1_num, t1_den = excluded.t1_den, t1_secs = excluded.t1_secs,
                       keyframe_sample_id = excluded.keyframe_sample_id, title = excluded.title,
                       summary = excluded.summary, provenance_id = excluded.provenance_id",
                )?;
                for seg in &s {
                    st.execute(params![
                        seg.id.to_string(),
                        seg.video_id.to_string(),
                        seg.level.as_str(),
                        seg.parent_id.map(|p| p.to_string()),
                        seg.t0.num,
                        seg.t0.den,
                        seg.t0.as_secs_f64(),
                        seg.t1.num,
                        seg.t1.den,
                        seg.t1.as_secs_f64(),
                        seg.keyframe_sample_id.map(|p| p.to_string()),
                        seg.title,
                        seg.summary,
                        seg.provenance_id.to_string(),
                    ])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
        .await
    }

    async fn put_frame_samples(&self, s: &[FrameSample]) -> Result<()> {
        let s = s.to_vec();
        self.with_conn(move |c| {
            let tx = c.unchecked_transaction()?;
            {
                let mut st = tx.prepare_cached(
                    "INSERT INTO frame_samples(id, track_id, t_num, t_den, t_secs, pts, is_keyframe, phash, thumbnail_blob, width, height)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
                     ON CONFLICT(id) DO UPDATE SET
                       track_id = excluded.track_id, t_num = excluded.t_num, t_den = excluded.t_den, t_secs = excluded.t_secs,
                       pts = excluded.pts, is_keyframe = excluded.is_keyframe,
                       phash = COALESCE(excluded.phash, frame_samples.phash),
                       thumbnail_blob = COALESCE(excluded.thumbnail_blob, frame_samples.thumbnail_blob),
                       width = excluded.width, height = excluded.height",
                )?;
                for f in &s {
                    st.execute(params![
                        f.id.to_string(),
                        f.track_id.to_string(),
                        f.t.num,
                        f.t.den,
                        f.t.as_secs_f64(),
                        f.pts,
                        i64::from(f.is_keyframe),
                        f.phash.map(|h| h as i64),
                        f.thumbnail_blob,
                        f.width,
                        f.height,
                    ])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
        .await
    }

    async fn update_frame_phash(&self, updates: &[(FrameSampleId, u64)]) -> Result<()> {
        let u = updates.to_vec();
        self.with_conn(move |c| {
            let tx = c.unchecked_transaction()?;
            {
                let mut st =
                    tx.prepare_cached("UPDATE frame_samples SET phash = ?2 WHERE id = ?1")?;
                for (id, h) in &u {
                    st.execute(params![id.to_string(), *h as i64])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
        .await
    }

    async fn update_frame_thumbnail(&self, updates: &[(FrameSampleId, BlobKey)]) -> Result<()> {
        let u = updates.to_vec();
        self.with_conn(move |c| {
            let tx = c.unchecked_transaction()?;
            {
                let mut st = tx
                    .prepare_cached("UPDATE frame_samples SET thumbnail_blob = ?2 WHERE id = ?1")?;
                for (id, k) in &u {
                    st.execute(params![id.to_string(), k.0])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
        .await
    }

    async fn delete_frame_samples(&self, track: TrackId) -> Result<u64> {
        self.with_conn(move |c| {
            let n = c.execute(
                "DELETE FROM frame_samples WHERE track_id = ?1",
                [track.to_string()],
            )?;
            Ok(n as u64)
        })
        .await
    }

    async fn frame_samples(
        &self,
        track: TrackId,
        range: Option<TimeRange>,
    ) -> Result<Vec<FrameSample>> {
        self.with_conn(move |c| {
            let (t0, t1) = match range {
                Some(r) => (r.t0.as_secs_f64(), r.t1.as_secs_f64()),
                None => (f64::NEG_INFINITY, f64::INFINITY),
            };
            let mut st = c.prepare_cached(
                "SELECT * FROM frame_samples WHERE track_id = ?1 AND t_secs >= ?2 AND t_secs < ?3 ORDER BY t_secs",
            )?;
            let rows = st.query_map(params![track.to_string(), t0, t1], |r| Ok(frame_from_row(r)))?;
            rows.map(|r| r?).collect()
        })
        .await
    }

    async fn put_spans(&self, s: &[Span]) -> Result<()> {
        let s = s.to_vec();
        self.with_conn(move |c| {
            let tx = c.unchecked_transaction()?;
            {
                let mut ts_st = tx.prepare_cached(
                    "INSERT OR REPLACE INTO transcript_spans(id, track_id, t0_num, t0_den, t0_secs, t1_num, t1_den, t1_secs, text, speaker, language, confidence, words, provenance_id)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
                )?;
                let mut ocr_st = tx.prepare_cached(
                    "INSERT OR REPLACE INTO ocr_spans(id, frame_sample_id, t_num, t_den, t_secs, text, bbox_x, bbox_y, bbox_w, bbox_h, confidence, provenance_id)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                )?;
                for span in &s {
                    match span {
                        Span::Transcript(t) => {
                            ts_st.execute(params![
                                t.id.to_string(),
                                t.track_id.to_string(),
                                t.t0.num,
                                t.t0.den,
                                t.t0.as_secs_f64(),
                                t.t1.num,
                                t.t1.den,
                                t.t1.as_secs_f64(),
                                t.text,
                                t.speaker,
                                t.language,
                                t.confidence,
                                t.words.as_ref().map(serde_json::to_string).transpose()?,
                                t.provenance_id.to_string(),
                            ])?;
                        }
                        Span::Ocr(o) => {
                            ocr_st.execute(params![
                                o.id.to_string(),
                                o.frame_sample_id.to_string(),
                                o.t.num,
                                o.t.den,
                                o.t.as_secs_f64(),
                                o.text,
                                o.bbox.map(|b| b.x),
                                o.bbox.map(|b| b.y),
                                o.bbox.map(|b| b.w),
                                o.bbox.map(|b| b.h),
                                o.confidence,
                                o.provenance_id.to_string(),
                            ])?;
                        }
                    }
                }
            }
            tx.commit()?;
            Ok(())
        })
        .await
    }

    async fn put_descriptions(&self, d: &[Description]) -> Result<()> {
        let d = d.to_vec();
        self.with_conn(move |c| {
            let tx = c.unchecked_transaction()?;
            {
                let mut st = tx.prepare_cached(
                    "INSERT OR REPLACE INTO descriptions(id, target_kind, target_id, kind, text, structured, provenance_id)
                     VALUES (?1,?2,?3,?4,?5,?6,?7)",
                )?;
                for desc in &d {
                    st.execute(params![
                        desc.id.to_string(),
                        desc.target_kind.as_str(),
                        desc.target_id,
                        desc.kind.as_str(),
                        desc.text,
                        desc.structured.as_ref().map(serde_json::to_string).transpose()?,
                        desc.provenance_id.to_string(),
                    ])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
        .await
    }

    async fn put_embeddings(&self, e: &[Embedding]) -> Result<()> {
        let e = e.to_vec();
        debug!(
            "vector store is a stub; storing metadata for {} embeddings only",
            e.len()
        );
        self.with_conn(move |c| {
            let tx = c.unchecked_transaction()?;
            {
                let mut st = tx.prepare_cached(
                    "INSERT OR REPLACE INTO embeddings(id, target_kind, target_id, model, dim, provenance_id)
                     VALUES (?1,?2,?3,?4,?5,?6)",
                )?;
                for emb in &e {
                    st.execute(params![
                        emb.id.to_string(),
                        emb.target_kind.as_str(),
                        emb.target_id,
                        emb.model,
                        emb.dim,
                        emb.provenance_id.to_string(),
                    ])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
        .await
    }

    async fn put_provenance(&self, p: &Provenance) -> Result<ProvenanceId> {
        let p = p.clone();
        self.with_conn(move |c| {
            c.execute(
                "INSERT OR REPLACE INTO provenance(id, operator, operator_version, provider, model, model_version, prompt_hash, params, created_at, cost_usd, tokens_in, tokens_out, latency_ms)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
                params![
                    p.id.to_string(),
                    p.operator,
                    p.operator_version,
                    p.provider,
                    p.model,
                    p.model_version,
                    p.prompt_hash,
                    serde_json::to_string(&p.params)?,
                    p.created_at.to_rfc3339(),
                    p.cost_usd,
                    i64::try_from(p.tokens_in).unwrap_or(i64::MAX),
                    i64::try_from(p.tokens_out).unwrap_or(i64::MAX),
                    i64::try_from(p.latency_ms).unwrap_or(i64::MAX),
                ],
            )?;
            Ok(p.id)
        })
        .await
    }

    async fn text_search(&self, q: &TextQuery) -> Result<Vec<Hit>> {
        let q = q.clone();
        self.with_conn(move |c| {
            let fts = fts_query(&q.query);
            if fts.is_empty() {
                return Ok(Vec::new());
            }
            let kinds: Vec<Kind> = if q.kinds.is_empty() {
                vec![Kind::Transcript, Kind::Ocr, Kind::Description]
            } else {
                q.kinds.clone()
            };
            let k = q.k.max(1) as i64;
            let mut hits = Vec::new();
            for kind in kinds {
                let (filter, ids) = video_filter(&q.videos, "v.id");
                let sql = match kind {
                    Kind::Transcript => format!(
                        "SELECT s.id, v.id AS video_id, s.t0_num, s.t0_den, s.t1_num, s.t1_den, s.text, bm25(transcript_fts) AS score
                         FROM transcript_fts JOIN transcript_spans s ON s.rowid = transcript_fts.rowid
                         JOIN tracks tr ON tr.id = s.track_id JOIN videos v ON v.id = tr.video_id
                         WHERE transcript_fts MATCH ?{filter} ORDER BY score LIMIT ?"
                    ),
                    Kind::Ocr => format!(
                        "SELECT s.id, v.id AS video_id, s.t_num AS t0_num, s.t_den AS t0_den, s.t_num AS t1_num, s.t_den AS t1_den, s.text, bm25(ocr_fts) AS score
                         FROM ocr_fts JOIN ocr_spans s ON s.rowid = ocr_fts.rowid
                         JOIN frame_samples f ON f.id = s.frame_sample_id JOIN tracks tr ON tr.id = f.track_id JOIN videos v ON v.id = tr.video_id
                         WHERE ocr_fts MATCH ?{filter} ORDER BY score LIMIT ?"
                    ),
                    Kind::Description => format!(
                        "SELECT d.id, v.id AS video_id, seg.t0_num, seg.t0_den, seg.t1_num, seg.t1_den, d.text, bm25(descriptions_fts) AS score
                         FROM descriptions_fts JOIN descriptions d ON d.rowid = descriptions_fts.rowid
                         JOIN segments seg ON seg.id = d.target_id AND d.target_kind = 'segment' JOIN videos v ON v.id = seg.video_id
                         WHERE descriptions_fts MATCH ?{filter} ORDER BY score LIMIT ?"
                    ),
                    Kind::Segment | Kind::Frame => continue,
                };
                let mut st = c.prepare(&sql)?;
                let mut bound: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(fts.clone())];
                for id in ids {
                    bound.push(Box::new(id));
                }
                bound.push(Box::new(k));
                let params_ref: Vec<&dyn rusqlite::ToSql> = bound.iter().map(|b| b.as_ref()).collect();
                let rows = st.query_map(params_ref.as_slice(), |r| {
                    let id: String = r.get("id")?;
                    let vid: String = r.get("video_id")?;
                    let score: f64 = r.get("score")?;
                    Ok((
                        id,
                        vid,
                        ts(r.get("t0_num")?, r.get("t0_den")?),
                        ts(r.get("t1_num")?, r.get("t1_den")?),
                        r.get::<_, String>("text")?,
                        score,
                    ))
                })?;
                for row in rows {
                    let (id, vid, t0, t1, text, score) = row?;
                    hits.push(Hit {
                        kind,
                        id,
                        video_id: parse_id(vid, "video")?,
                        t0,
                        t1,
                        text,
                        // bm25() is lower-is-better and negative; flip it.
                        score: -score,
                    });
                }
            }
            hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
            hits.truncate(q.k.max(1));
            Ok(hits)
        })
        .await
    }

    async fn vector_search(&self, _q: &VectorQuery) -> Result<Vec<Hit>> {
        // M0 stub: no vectors are stored.
        Ok(Vec::new())
    }

    async fn time_window(
        &self,
        video: VideoId,
        t0: Timestamp,
        t1: Timestamp,
        kinds: &[Kind],
    ) -> Result<Window> {
        let kinds: Vec<Kind> = if kinds.is_empty() {
            vec![
                Kind::Segment,
                Kind::Frame,
                Kind::Transcript,
                Kind::Ocr,
                Kind::Description,
            ]
        } else {
            kinds.to_vec()
        };
        self.with_conn(move |c| {
            let (a, b) = (t0.as_secs_f64(), t1.as_secs_f64());
            let vid = video.to_string();
            let mut w = Window::default();
            if kinds.contains(&Kind::Segment) {
                let mut st = c.prepare_cached(
                    "SELECT * FROM segments WHERE video_id = ?1 AND t0_secs < ?3 AND t1_secs > ?2 ORDER BY level, t0_secs",
                )?;
                w.segments = st
                    .query_map(params![vid, a, b], |r| Ok(segment_from_row(r)))?
                    .map(|r| r?)
                    .collect::<Result<_>>()?;
            }
            if kinds.contains(&Kind::Frame) {
                let mut st = c.prepare_cached(
                    "SELECT f.* FROM frame_samples f JOIN tracks tr ON tr.id = f.track_id
                     WHERE tr.video_id = ?1 AND f.t_secs >= ?2 AND f.t_secs < ?3 ORDER BY f.t_secs",
                )?;
                w.frames = st
                    .query_map(params![vid, a, b], |r| Ok(frame_from_row(r)))?
                    .map(|r| r?)
                    .collect::<Result<_>>()?;
            }
            if kinds.contains(&Kind::Transcript) {
                let mut st = c.prepare_cached(
                    "SELECT s.* FROM transcript_spans s JOIN tracks tr ON tr.id = s.track_id
                     WHERE tr.video_id = ?1 AND s.t0_secs < ?3 AND s.t1_secs > ?2 ORDER BY s.t0_secs",
                )?;
                w.transcript = st
                    .query_map(params![vid, a, b], |r| Ok(transcript_from_row(r)))?
                    .map(|r| r?)
                    .collect::<Result<_>>()?;
            }
            if kinds.contains(&Kind::Ocr) {
                let mut st = c.prepare_cached(
                    "SELECT o.* FROM ocr_spans o JOIN frame_samples f ON f.id = o.frame_sample_id JOIN tracks tr ON tr.id = f.track_id
                     WHERE tr.video_id = ?1 AND o.t_secs >= ?2 AND o.t_secs < ?3 ORDER BY o.t_secs",
                )?;
                w.ocr = st
                    .query_map(params![vid, a, b], |r| Ok(ocr_from_row(r)))?
                    .map(|r| r?)
                    .collect::<Result<_>>()?;
            }
            if kinds.contains(&Kind::Description) {
                let mut st = c.prepare_cached(
                    "SELECT d.* FROM descriptions d
                     WHERE (d.target_kind = 'segment' AND d.target_id IN (SELECT id FROM segments WHERE video_id = ?1 AND t0_secs < ?3 AND t1_secs > ?2))
                        OR (d.target_kind = 'frame' AND d.target_id IN (SELECT f.id FROM frame_samples f JOIN tracks tr ON tr.id = f.track_id WHERE tr.video_id = ?1 AND f.t_secs >= ?2 AND f.t_secs < ?3))",
                )?;
                w.descriptions = st
                    .query_map(params![vid, a, b], |r| Ok(description_from_row(r)))?
                    .map(|r| r?)
                    .collect::<Result<_>>()?;
            }
            Ok(w)
        })
        .await
    }

    async fn put_blob(&self, key: &BlobKey, bytes: Bytes) -> Result<()> {
        self.blobs.put(key, bytes).await
    }

    async fn get_blob(&self, key: &BlobKey) -> Result<Option<Bytes>> {
        self.blobs.get(key).await
    }

    async fn checkpoint(&self, job: JobId, state: &JobState) -> Result<()> {
        let dir = self.jobs_dir();
        tokio::fs::create_dir_all(&dir).await?;
        let path = dir.join(format!("{job}.json"));
        let tmp = dir.join(format!("{job}.json.tmp"));
        tokio::fs::write(&tmp, serde_json::to_vec_pretty(state)?).await?;
        tokio::fs::rename(tmp, path).await?;
        Ok(())
    }

    async fn load_checkpoint(&self, job: JobId) -> Result<Option<JobState>> {
        match tokio::fs::read(self.jobs_dir().join(format!("{job}.json"))).await {
            Ok(v) => Ok(Some(serde_json::from_slice(&v)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    async fn list_jobs(&self) -> Result<Vec<JobState>> {
        let mut out = Vec::new();
        let mut rd = match tokio::fs::read_dir(self.jobs_dir()).await {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(e) => return Err(e.into()),
        };
        while let Some(entry) = rd.next_entry().await? {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("json") {
                match serde_json::from_slice::<JobState>(&tokio::fs::read(&p).await?) {
                    Ok(j) => out.push(j),
                    Err(e) => debug!("skipping unreadable job file {}: {e}", p.display()),
                }
            }
        }
        out.sort_by_key(|j| j.job_id);
        Ok(out)
    }

    async fn manifest(&self) -> Result<Manifest> {
        let dir = self.dir.clone();
        tokio::task::spawn_blocking(move || Manifest::read(&dir)).await?
    }

    async fn stats(&self) -> Result<IndexStats> {
        let manifest = self.manifest().await?;
        let jobs = self.list_jobs().await?;
        let dir = self.dir.clone();
        let blobs = self.blobs.clone();
        let sizes = tokio::task::spawn_blocking(move || -> Result<(u64, u64, u64, u64)> {
            let dir_bytes = dir_size(&dir)?;
            let sqlite_bytes = std::fs::metadata(dir.join(SQLITE_FILE))
                .map(|m| m.len())
                .unwrap_or(0)
                + std::fs::metadata(dir.join(format!("{SQLITE_FILE}-wal")))
                    .map(|m| m.len())
                    .unwrap_or(0);
            let (bc, bb) = blobs.stats_blocking()?;
            Ok((dir_bytes, sqlite_bytes, bc, bb))
        })
        .await??;
        let videos = self
            .with_conn(|c| {
                let mut st = c.prepare(&format!("SELECT {VIDEO_COLS} FROM videos ORDER BY created_at, id"))?;
                let videos: Vec<Video> = st
                    .query_map([], |r| Ok(video_from_row(r)))?
                    .map(|r| r?)
                    .collect::<Result<_>>()?;
                let mut out = Vec::with_capacity(videos.len());
                for v in videos {
                    let vid = v.id.to_string();
                    let count = |sql: &str| -> Result<u64> {
                        Ok(c.query_row(sql, [&vid], |r| r.get::<_, i64>(0))? as u64)
                    };
                    let tracks = count("SELECT COUNT(*) FROM tracks WHERE video_id = ?1")?;
                    let frame_samples = count("SELECT COUNT(*) FROM frame_samples f JOIN tracks t ON t.id = f.track_id WHERE t.video_id = ?1")?;
                    let hashed = count("SELECT COUNT(*) FROM frame_samples f JOIN tracks t ON t.id = f.track_id WHERE t.video_id = ?1 AND f.phash IS NOT NULL")?;
                    let thumbnails = count("SELECT COUNT(*) FROM frame_samples f JOIN tracks t ON t.id = f.track_id WHERE t.video_id = ?1 AND f.thumbnail_blob IS NOT NULL")?;
                    let segments = count("SELECT COUNT(*) FROM segments WHERE video_id = ?1")?;
                    let transcript_spans = count("SELECT COUNT(*) FROM transcript_spans s JOIN tracks t ON t.id = s.track_id WHERE t.video_id = ?1")?;
                    let ocr_spans = count("SELECT COUNT(*) FROM ocr_spans o JOIN frame_samples f ON f.id = o.frame_sample_id JOIN tracks t ON t.id = f.track_id WHERE t.video_id = ?1")?;
                    let descriptions = count("SELECT COUNT(*) FROM descriptions d WHERE (d.target_kind = 'segment' AND d.target_id IN (SELECT id FROM segments WHERE video_id = ?1)) OR (d.target_kind = 'frame' AND d.target_id IN (SELECT f.id FROM frame_samples f JOIN tracks t ON t.id = f.track_id WHERE t.video_id = ?1))")?;
                    out.push(VideoStats {
                        video: v,
                        tracks,
                        frame_samples,
                        hashed,
                        thumbnails,
                        segments,
                        transcript_spans,
                        ocr_spans,
                        descriptions,
                        cost_usd: 0.0,
                    });
                }
                Ok(out)
            })
            .await?;
        Ok(IndexStats {
            manifest,
            path: self.dir.display().to_string(),
            dir_bytes: sizes.0,
            sqlite_bytes: sizes.1,
            blob_count: sizes.2,
            blob_bytes: sizes.3,
            videos,
            jobs,
        })
    }

    async fn compact(&self) -> Result<()> {
        self.with_conn(|c| {
            c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;")?;
            Ok(())
        })
        .await?;
        let dir = self.dir.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut m = Manifest::read(&dir)?;
            m.files = file_hashes(&dir)?;
            m.updated_at = Utc::now();
            m.write(&dir)
        })
        .await?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(hash: &str) -> Video {
        Video {
            id: VideoId::new(),
            source_uri: "file:///tmp/a.mp4".into(),
            content_hash: hash.into(),
            title: Some("A talk".into()),
            description: None,
            channel: None,
            published_at: None,
            duration: Timestamp::new(3600 * 1000, 1000),
            start_wallclock: None,
            probe: serde_json::json!({"format": "mp4"}),
            index_state: IndexState::Acquired,
            created_at: Utc::now(),
        }
    }

    fn track(video_id: VideoId) -> Track {
        Track {
            id: TrackId::new(),
            video_id,
            kind: TrackKind::Video,
            stream_index: 0,
            codec: "h264".into(),
            timebase_num: 1,
            timebase_den: 15360,
            width: Some(1280),
            height: Some(720),
            fps: Some(30.0),
            sample_rate: None,
            channels: None,
            language: None,
        }
    }

    #[tokio::test]
    async fn create_open_and_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.vidx");
        let idx = EmbeddedIndex::create(&p).unwrap();
        assert!(EmbeddedIndex::create(&p).is_err());
        let m = idx.manifest().await.unwrap();
        assert_eq!(m.schema_version, crate::SCHEMA_VERSION);
        assert!(m.files.contains_key(SQLITE_FILE));
        drop(idx);
        let idx = EmbeddedIndex::open(&p).unwrap();
        assert_eq!(idx.list_videos().await.unwrap().len(), 0);
        assert!(EmbeddedIndex::open(dir.path()).is_err());
    }

    #[tokio::test]
    async fn refuses_newer_schema() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.vidx");
        EmbeddedIndex::create(&p).unwrap();
        let mut m = Manifest::read(&p).unwrap();
        m.schema_version = 99;
        m.write(&p).unwrap();
        assert!(matches!(
            EmbeddedIndex::open(&p),
            Err(IndexError::SchemaTooNew { found: 99, .. })
        ));
    }

    #[tokio::test]
    async fn video_track_frame_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let idx = EmbeddedIndex::create(&dir.path().join("x.vidx")).unwrap();
        let v = video("abc");
        idx.put_video(&v).await.unwrap();
        assert_eq!(idx.get_video(v.id).await.unwrap().unwrap(), v);
        assert_eq!(
            idx.find_video_by_hash("abc").await.unwrap().unwrap().id,
            v.id
        );
        assert!(idx.find_video_by_hash("zzz").await.unwrap().is_none());
        idx.set_index_state(v.id, IndexState::Coarse).await.unwrap();
        assert_eq!(
            idx.get_video(v.id).await.unwrap().unwrap().index_state,
            IndexState::Coarse
        );

        let t = track(v.id);
        idx.put_tracks(std::slice::from_ref(&t)).await.unwrap();
        assert_eq!(idx.tracks(v.id).await.unwrap(), vec![t.clone()]);
        // Re-putting the video is an update, not delete + insert: children survive.
        idx.put_video(&Video {
            title: Some("renamed".into()),
            ..v.clone()
        })
        .await
        .unwrap();
        assert_eq!(idx.tracks(v.id).await.unwrap(), vec![t.clone()]);
        assert_eq!(
            idx.get_video(v.id).await.unwrap().unwrap().title.as_deref(),
            Some("renamed")
        );

        let frames: Vec<FrameSample> = (0..10)
            .map(|i| FrameSample {
                id: FrameSampleId::new(),
                track_id: t.id,
                t: Timestamp::new(i * 15360, 15360),
                pts: i * 15360,
                is_keyframe: i % 5 == 0,
                phash: None,
                thumbnail_blob: None,
                width: 640,
                height: 360,
            })
            .collect();
        idx.put_frame_samples(&frames).await.unwrap();
        let got = idx.frame_samples(t.id, None).await.unwrap();
        assert_eq!(got, frames);
        let ranged = idx
            .frame_samples(
                t.id,
                TimeRange::new(Timestamp::from_secs(3), Timestamp::from_secs(6)),
            )
            .await
            .unwrap();
        assert_eq!(ranged.len(), 3);

        let big = u64::MAX - 5;
        idx.update_frame_phash(&[(frames[0].id, big), (frames[1].id, 7)])
            .await
            .unwrap();
        let key = BlobKey::for_bytes(b"thumb");
        idx.update_frame_thumbnail(&[(frames[0].id, key.clone())])
            .await
            .unwrap();
        let got = idx.frame_samples(t.id, None).await.unwrap();
        assert_eq!(got[0].phash, Some(big), "u64 survives the i64 column");
        assert_eq!(got[1].phash, Some(7));
        assert_eq!(got[0].thumbnail_blob.as_deref(), Some(key.0.as_str()));

        let stats = idx.stats().await.unwrap();
        assert_eq!(stats.videos.len(), 1);
        assert_eq!(stats.videos[0].frame_samples, 10);
        assert_eq!(stats.videos[0].hashed, 2);
        assert_eq!(stats.videos[0].thumbnails, 1);
        assert!(stats.dir_bytes > 0);

        assert_eq!(idx.delete_frame_samples(t.id).await.unwrap(), 10);
        assert!(idx.frame_samples(t.id, None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn spans_fts_and_time_window() {
        let dir = tempfile::tempdir().unwrap();
        let idx = EmbeddedIndex::create(&dir.path().join("x.vidx")).unwrap();
        let v = video("h1");
        idx.put_video(&v).await.unwrap();
        let t = track(v.id);
        idx.put_tracks(std::slice::from_ref(&t)).await.unwrap();
        let prov = Provenance::local("asr", 1, serde_json::json!({}));
        idx.put_provenance(&prov).await.unwrap();
        let span = |t0: i64, text: &str| {
            Span::Transcript(TranscriptSpan {
                id: SpanId::new(),
                track_id: t.id,
                t0: Timestamp::from_secs(t0),
                t1: Timestamp::from_secs(t0 + 10),
                text: text.into(),
                speaker: None,
                language: Some("en".into()),
                confidence: Some(0.9),
                words: None,
                provenance_id: prov.id,
            })
        };
        idx.put_spans(&[
            span(0, "welcome to the workshop on hybrid retrieval"),
            span(10, "the retriever returns candidate segments"),
            span(20, "lunch is at noon"),
        ])
        .await
        .unwrap();
        let frame = FrameSample {
            id: FrameSampleId::new(),
            track_id: t.id,
            t: Timestamp::from_secs(12),
            pts: 12 * 15360,
            is_keyframe: true,
            phash: Some(1),
            thumbnail_blob: None,
            width: 640,
            height: 360,
        };
        idx.put_frame_samples(std::slice::from_ref(&frame))
            .await
            .unwrap();
        idx.put_spans(&[Span::Ocr(OcrSpan {
            id: SpanId::new(),
            frame_sample_id: frame.id,
            t: frame.t,
            text: "Hybrid retrieval: BM25 + dense".into(),
            bbox: Some(BBox {
                x: 0.1,
                y: 0.1,
                w: 0.5,
                h: 0.1,
            }),
            confidence: Some(0.8),
            provenance_id: prov.id,
        })])
        .await
        .unwrap();

        let hits = idx
            .text_search(&TextQuery::new("retriev", 10))
            .await
            .unwrap();
        assert_eq!(hits.len(), 3, "{hits:?}");
        assert!(hits.iter().any(|h| h.kind == Kind::Ocr));
        assert!(hits.iter().all(|h| h.video_id == v.id));
        assert!(hits.iter().all(|h| h.score > 0.0));

        let only_ocr = idx
            .text_search(&TextQuery {
                kinds: vec![Kind::Ocr],
                ..TextQuery::new("hybrid", 10)
            })
            .await
            .unwrap();
        assert_eq!(only_ocr.len(), 1);
        assert_eq!(only_ocr[0].t0, Timestamp::from_secs(12));

        let other = idx
            .text_search(&TextQuery {
                videos: vec![VideoId::new()],
                ..TextQuery::new("retrieval", 10)
            })
            .await
            .unwrap();
        assert!(other.is_empty());

        assert!(idx
            .text_search(&TextQuery::new("\"unbalanced", 10))
            .await
            .is_ok());

        let w = idx
            .time_window(v.id, Timestamp::from_secs(5), Timestamp::from_secs(15), &[])
            .await
            .unwrap();
        assert_eq!(w.transcript.len(), 2, "spans overlapping [5,15)");
        assert_eq!(w.frames.len(), 1);
        assert_eq!(w.ocr.len(), 1);
    }

    #[tokio::test]
    async fn segments_descriptions_embeddings() {
        let dir = tempfile::tempdir().unwrap();
        let idx = EmbeddedIndex::create(&dir.path().join("x.vidx")).unwrap();
        let v = video("h2");
        idx.put_video(&v).await.unwrap();
        let prov = Provenance::local("shot_boundary", 1, serde_json::json!({"threshold": 0.3}));
        idx.put_provenance(&prov).await.unwrap();
        let seg = Segment {
            id: SegmentId::new(),
            video_id: v.id,
            level: SegmentLevel::Scene,
            parent_id: None,
            t0: Timestamp::from_secs(0),
            t1: Timestamp::from_secs(60),
            keyframe_sample_id: None,
            title: Some("Intro".into()),
            summary: None,
            provenance_id: prov.id,
        };
        idx.put_segments(std::slice::from_ref(&seg)).await.unwrap();
        let desc = Description {
            id: DescriptionId::new(),
            target_kind: TargetKind::Segment,
            target_id: seg.id.to_string(),
            kind: DescriptionKind::Caption,
            text: "Speaker at a podium beside an architecture diagram".into(),
            structured: Some(serde_json::json!({"people": 1})),
            provenance_id: prov.id,
        };
        idx.put_descriptions(std::slice::from_ref(&desc))
            .await
            .unwrap();
        let hits = idx
            .text_search(&TextQuery::new("architecture diagram", 5))
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, Kind::Description);
        assert_eq!(hits[0].t1, Timestamp::from_secs(60));

        let w = idx
            .time_window(
                v.id,
                Timestamp::from_secs(10),
                Timestamp::from_secs(20),
                &[Kind::Segment, Kind::Description],
            )
            .await
            .unwrap();
        assert_eq!(w.segments, vec![seg]);
        assert_eq!(w.descriptions, vec![desc]);

        idx.put_embeddings(&[Embedding {
            id: EmbeddingId::new(),
            target_kind: TargetKind::Description,
            target_id: "x".into(),
            model: "bge-small".into(),
            dim: 3,
            vector: vec![0.1, 0.2, 0.3],
            provenance_id: prov.id,
        }])
        .await
        .unwrap();
        let hits = idx
            .vector_search(&VectorQuery {
                model: "bge-small".into(),
                vector: vec![0.1, 0.2, 0.3],
                videos: vec![],
                kinds: vec![],
                k: 5,
            })
            .await
            .unwrap();
        assert!(hits.is_empty(), "vector store is a stub in M0");
    }

    #[tokio::test]
    async fn blobs_checkpoints_compact() {
        let dir = tempfile::tempdir().unwrap();
        let idx = EmbeddedIndex::create(&dir.path().join("x.vidx")).unwrap();
        let key = BlobKey::for_bytes(b"webp bytes");
        idx.put_blob(&key, Bytes::from_static(b"webp bytes"))
            .await
            .unwrap();
        assert_eq!(
            idx.get_blob(&key).await.unwrap().unwrap(),
            Bytes::from_static(b"webp bytes")
        );

        let job = JobId::new();
        let mut state = JobState::new(
            job,
            VideoId::new(),
            "file:///a",
            "m0",
            ["sample".to_string()],
        );
        state.stage_mut("sample").status = StageStatus::Complete;
        idx.checkpoint(job, &state).await.unwrap();
        assert_eq!(idx.load_checkpoint(job).await.unwrap().unwrap(), state);
        assert!(idx.load_checkpoint(JobId::new()).await.unwrap().is_none());
        assert_eq!(idx.list_jobs().await.unwrap().len(), 1);

        let before = idx.manifest().await.unwrap();
        idx.put_video(&video("h3")).await.unwrap();
        idx.compact().await.unwrap();
        let after = idx.manifest().await.unwrap();
        assert_ne!(
            before.files[SQLITE_FILE].blake3,
            after.files[SQLITE_FILE].blake3
        );
        assert!(after.updated_at >= before.updated_at);
        let stats = idx.stats().await.unwrap();
        assert_eq!(stats.blob_count, 1);
        assert_eq!(stats.jobs.len(), 1);
    }

    #[test]
    fn fts_query_escaping() {
        assert_eq!(fts_query("hybrid retrieval"), "\"hybrid\" \"retrieval\"*");
        assert_eq!(fts_query("say \"hi\""), "\"say\" \"\"\"hi\"\"\"*");
        assert_eq!(fts_query("   "), "");
    }
}
