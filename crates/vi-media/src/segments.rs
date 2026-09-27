//! The segment index of a live store: a directory of MPEG-TS segments and
//! the JSON file that says where each one sits on the media timeline.
//!
//! This is the shared definition the live store (writer) and the decoder
//! (reader) agree on. The writer appends a segment by writing it to a
//! temporary name, renaming it into place and then rewriting `index.json`
//! atomically; a reader only ever sees complete segments. See
//! `videoindex-live/docs/04-realtime-core.md`, "The live store".
//!
//! ```text
//! <store>/
//!   index.json        this file
//!   seg/000001.ts …   2 s MPEG-TS segments (audio + video, codec copy)
//! ```

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use vi_core::{TimeRange, Timestamp};

use crate::error::{MediaError, Result};

/// File name of the segment index inside a store directory.
pub const INDEX_FILE: &str = "index.json";

/// Sub-directory holding the segments.
pub const SEGMENT_DIR: &str = "seg";

/// Schema version this build reads and writes.
pub const SEGMENT_INDEX_SCHEMA: u32 = 1;

/// What a decode or probe request reads from: one file, or a live store's
/// segment feed. `From<PathBuf>` (and `&Path`, `&PathBuf`) keeps every
/// file-based caller as it was.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MediaInput {
    /// A media file.
    File {
        /// The file.
        path: PathBuf,
    },
    /// A segmented recording in the live store layout.
    Segments(SegmentFeed),
}

/// A directory of MPEG-TS segments and their `index.json` (see
/// [`SegmentIndex`]). With `follow` set the recording may still be growing:
/// probes report no duration and live decodes wait for more segments.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SegmentFeed {
    /// The store directory; `index.json` and `seg/` are inside it.
    pub dir: PathBuf,
    /// True while the writer may still append segments.
    pub follow: bool,
}

impl SegmentFeed {
    /// A feed over a finished recording.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            follow: false,
        }
    }

    /// A feed over a recording that may still grow.
    pub fn following(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            follow: true,
        }
    }

    /// Path of the feed's `index.json`.
    pub fn index_path(&self) -> PathBuf {
        SegmentIndex::path_in(&self.dir)
    }

    /// Read the feed's index.
    pub fn load_index(&self) -> Result<SegmentIndex> {
        SegmentIndex::load(&self.index_path())
    }

    /// Absolute path of a segment named in the index.
    pub fn segment_path(&self, entry: &SegmentEntry) -> PathBuf {
        self.dir.join(&entry.file)
    }
}

impl MediaInput {
    /// A file input.
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self::File { path: path.into() }
    }

    /// The file path for a file input, `None` for a feed.
    pub fn as_file(&self) -> Option<&Path> {
        match self {
            Self::File { path } => Some(path),
            Self::Segments(_) => None,
        }
    }

    /// The feed for a segments input, `None` for a file.
    pub fn as_segments(&self) -> Option<&SegmentFeed> {
        match self {
            Self::File { .. } => None,
            Self::Segments(feed) => Some(feed),
        }
    }

    /// Human-readable location: the file path or the store directory.
    pub fn display(&self) -> String {
        match self {
            Self::File { path } => path.display().to_string(),
            Self::Segments(feed) => feed.dir.display().to_string(),
        }
    }
}

impl From<PathBuf> for MediaInput {
    fn from(path: PathBuf) -> Self {
        Self::File { path }
    }
}

impl From<&PathBuf> for MediaInput {
    fn from(path: &PathBuf) -> Self {
        Self::File { path: path.clone() }
    }
}

impl From<&Path> for MediaInput {
    fn from(path: &Path) -> Self {
        Self::File {
            path: path.to_path_buf(),
        }
    }
}

impl From<SegmentFeed> for MediaInput {
    fn from(feed: SegmentFeed) -> Self {
        Self::Segments(feed)
    }
}

impl From<&str> for MediaInput {
    fn from(path: &str) -> Self {
        Self::File {
            path: PathBuf::from(path),
        }
    }
}

impl From<String> for MediaInput {
    fn from(path: String) -> Self {
        Self::File {
            path: PathBuf::from(path),
        }
    }
}

impl From<&String> for MediaInput {
    fn from(path: &String) -> Self {
        Self::File {
            path: PathBuf::from(path),
        }
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// One complete segment on disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SegmentEntry {
    /// Monotonic sequence number; the first segment of a store is 1.
    pub seq: u64,
    /// File name relative to the store directory, e.g. `seg/000001.ts`.
    pub file: String,
    /// Media time of the first frame.
    pub t0: Timestamp,
    /// Media time just after the last frame.
    pub t1: Timestamp,
    /// Size on disk.
    pub bytes: u64,
    /// Wall-clock time of `t0` when the source carried one (HLS programme
    /// date-time, RTMP arrival time); `None` for replays and fixtures.
    #[serde(default)]
    pub wallclock: Option<DateTime<Utc>>,
    /// True when the segment's in-file PTS do not continue the previous
    /// segment's (an HLS `EXT-X-DISCONTINUITY`, an encoder restart). The
    /// decoder re-anchors its audio clock there; the timeline itself comes
    /// from `t0`, so nothing else changes. Absent in the JSON when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub discontinuity: bool,
}

/// A range of media time with no segments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentGap {
    /// Inclusive start.
    pub t0: Timestamp,
    /// Exclusive end.
    pub t1: Timestamp,
    /// Why the range is missing, as the writer names it (`discontinuity`,
    /// `window_overrun`, `fetch`, `expired`, `recovered`, `other`); absent
    /// in the JSON when the writer gave none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl SegmentGap {
    /// A gap with an optional reason.
    pub fn new(t0: Timestamp, t1: Timestamp, reason: Option<&str>) -> Self {
        Self {
            t0,
            t1,
            reason: reason.map(str::to_string),
        }
    }

    /// The gap as a half-open range, `None` if it is empty or inverted.
    pub fn range(&self) -> Option<TimeRange> {
        TimeRange::new(self.t0, self.t1)
    }
}

impl From<TimeRange> for SegmentGap {
    fn from(r: TimeRange) -> Self {
        Self {
            t0: r.t0,
            t1: r.t1,
            reason: None,
        }
    }
}

/// The index of a segmented recording.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SegmentIndex {
    /// Schema version, [`SEGMENT_INDEX_SCHEMA`].
    pub schema: u32,
    /// Timebase segment PTS values are expressed in, as seconds per tick
    /// (`1/90000` for MPEG-TS). Entry times are rationals in their own
    /// denominators; this records the container clock for tools that need it.
    pub timebase: Timestamp,
    /// Segments in `seq` order.
    pub segments: Vec<SegmentEntry>,
    /// Ranges of media time with no segments (source discontinuities,
    /// expired retention), each with the writer's reason when it gave one.
    #[serde(default)]
    pub gaps: Vec<SegmentGap>,
    /// True once the writer has closed the recording; no further segments
    /// will appear.
    #[serde(default)]
    pub ended: bool,
}

impl Default for SegmentIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl SegmentIndex {
    /// An empty index with the MPEG-TS timebase.
    pub fn new() -> Self {
        Self {
            schema: SEGMENT_INDEX_SCHEMA,
            timebase: Timestamp::new(1, 90_000),
            segments: Vec::new(),
            gaps: Vec::new(),
            ended: false,
        }
    }

    /// Path of the index file inside a store directory.
    pub fn path_in(dir: &Path) -> PathBuf {
        dir.join(INDEX_FILE)
    }

    /// Read an index file. A missing, truncated or otherwise unparsable file
    /// is a [`MediaError::Protocol`] error naming the path; a newer schema is
    /// refused the same way.
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path)?;
        let idx: Self = serde_json::from_slice(&bytes)
            .map_err(|e| MediaError::Protocol(format!("segment index {}: {e}", path.display())))?;
        if idx.schema > SEGMENT_INDEX_SCHEMA {
            return Err(MediaError::Protocol(format!(
                "segment index {} has schema {} newer than supported {}",
                path.display(),
                idx.schema,
                SEGMENT_INDEX_SCHEMA
            )));
        }
        Ok(idx)
    }

    /// Write atomically: to `<path>.tmp`, then rename over `path`.
    pub fn save(&self, path: &Path) -> Result<()> {
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// The segments overlapping `[t0, t1)`, in order. Segments are contiguous
    /// in `seq` order, so this is a slice of `segments`.
    pub fn covering(&self, t0: Timestamp, t1: Timestamp) -> &[SegmentEntry] {
        if t1 <= t0 {
            return &[];
        }
        let start = self.segments.partition_point(|s| s.t1 <= t0);
        let end = self.segments.partition_point(|s| s.t0 < t1);
        if start >= end {
            &[]
        } else {
            &self.segments[start..end]
        }
    }

    /// The end of the last segment, or zero for an empty index. This is the
    /// head of a live recording as far as the store knows it.
    pub fn head(&self) -> Timestamp {
        self.segments
            .last()
            .map(|s| s.t1)
            .unwrap_or(Timestamp::ZERO)
    }

    /// Total duration covered by segments, `head - first.t0`.
    pub fn duration(&self) -> Timestamp {
        match self.segments.first() {
            Some(first) => self.head().sub(first.t0),
            None => Timestamp::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(seq: u64, t0: i64, t1: i64) -> SegmentEntry {
        SegmentEntry {
            seq,
            file: format!("seg/{seq:06}.ts"),
            t0: Timestamp::from_secs(t0),
            t1: Timestamp::from_secs(t1),
            bytes: 1,
            wallclock: None,
            discontinuity: false,
        }
    }

    fn index() -> SegmentIndex {
        let mut idx = SegmentIndex::new();
        idx.segments = (0..5)
            .map(|i| entry(i + 1, 2 * i as i64, 2 * i as i64 + 2))
            .collect();
        idx
    }

    #[test]
    fn covering_selects_overlapping_segments() {
        let idx = index();
        let s = Timestamp::from_secs;
        let seqs = |c: &[SegmentEntry]| c.iter().map(|e| e.seq).collect::<Vec<_>>();
        assert_eq!(seqs(idx.covering(s(0), s(2))), vec![1]);
        assert_eq!(seqs(idx.covering(s(1), s(5))), vec![1, 2, 3]);
        assert_eq!(seqs(idx.covering(s(2), s(4))), vec![2]);
        assert_eq!(seqs(idx.covering(s(9), s(20))), vec![5]);
        assert!(idx.covering(s(10), s(12)).is_empty());
        assert!(idx.covering(s(4), s(4)).is_empty());
        assert!(idx.covering(s(4), s(3)).is_empty());
        assert_eq!(idx.head(), s(10));
        assert_eq!(idx.duration(), s(10));
        assert!(SegmentIndex::new().covering(s(0), s(1)).is_empty());
    }

    #[test]
    fn save_load_roundtrip_and_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let p = SegmentIndex::path_in(dir.path());
        let mut idx = index();
        idx.gaps.push(SegmentGap::from(
            TimeRange::new(Timestamp::from_secs(10), Timestamp::from_secs(14)).unwrap(),
        ));
        idx.save(&p).unwrap();
        assert!(!dir.path().join("index.json.tmp").exists());
        let back = SegmentIndex::load(&p).unwrap();
        assert_eq!(back, idx);
        assert!(!back.ended);
        assert_eq!(
            back.gaps[0].range(),
            TimeRange::new(Timestamp::from_secs(10), Timestamp::from_secs(14))
        );
        // A gap without a reason and a segment without a discontinuity keep
        // the old JSON shape: neither key is written.
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(!text.contains("reason"), "{text}");
        assert!(!text.contains("discontinuity"), "{text}");
        // `ended`, `gaps` and `wallclock` default when absent.
        std::fs::write(
            &p,
            r#"{"schema":1,"timebase":{"num":1,"den":90000},"segments":[{"seq":1,"file":"seg/000001.ts","t0":{"num":0,"den":1},"t1":{"num":2,"den":1},"bytes":3}]}"#,
        )
        .unwrap();
        let back = SegmentIndex::load(&p).unwrap();
        assert_eq!(back.segments.len(), 1);
        assert!(back.gaps.is_empty() && !back.ended);
        assert_eq!(back.segments[0].wallclock, None);
        assert!(!back.segments[0].discontinuity);
    }

    #[test]
    fn discontinuity_and_gap_reason_survive_a_save() {
        // The shape the live store writes (realtime-core, S0): a per-segment
        // `discontinuity` flag and a `reason` per gap. Both must round-trip
        // through `load` and `save`, so the decoder can rewrite an index
        // without losing what the writer recorded.
        let dir = tempfile::tempdir().unwrap();
        let p = SegmentIndex::path_in(dir.path());
        std::fs::write(
            &p,
            r#"{"schema":1,"timebase":{"num":1,"den":90000},"ended":false,
                "segments":[
                  {"seq":1,"file":"seg/000001.ts","t0":{"num":0,"den":1},"t1":{"num":2,"den":1},"bytes":3},
                  {"seq":2,"file":"seg/000002.ts","t0":{"num":6,"den":1},"t1":{"num":8,"den":1},"bytes":3,"discontinuity":true}],
                "gaps":[{"t0":{"num":2,"den":1},"t1":{"num":6,"den":1},"reason":"window_overrun"}],
                "retention":{"max_hours":12,"max_gb":10}}"#,
        )
        .unwrap();
        let idx = SegmentIndex::load(&p).unwrap();
        assert!(!idx.segments[0].discontinuity);
        assert!(idx.segments[1].discontinuity);
        assert_eq!(idx.gaps.len(), 1);
        assert_eq!(idx.gaps[0].reason.as_deref(), Some("window_overrun"));
        assert_eq!(
            idx.gaps[0].range(),
            TimeRange::new(Timestamp::from_secs(2), Timestamp::from_secs(6))
        );
        idx.save(&p).unwrap();
        let back = SegmentIndex::load(&p).unwrap();
        assert_eq!(back, idx);
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(text.contains(r#""discontinuity": true"#), "{text}");
        assert!(text.contains(r#""reason": "window_overrun""#), "{text}");
        assert_eq!(
            SegmentGap::new(Timestamp::from_secs(1), Timestamp::from_secs(1), Some("x")).range(),
            None
        );
    }

    #[test]
    fn media_input_conversions_and_serde() {
        let p = PathBuf::from("/tmp/a.mp4");
        assert_eq!(MediaInput::from(p.clone()).as_file(), Some(p.as_path()));
        assert_eq!(MediaInput::from(&p), MediaInput::file(&p));
        assert_eq!(MediaInput::from(p.as_path()).display(), "/tmp/a.mp4");
        assert!(MediaInput::from(&p).as_segments().is_none());
        let feed = SegmentFeed::following("/store");
        assert!(feed.follow);
        assert_eq!(feed.index_path(), PathBuf::from("/store/index.json"));
        assert_eq!(
            feed.segment_path(&entry(1, 0, 2)),
            PathBuf::from("/store/seg/000001.ts")
        );
        let input = MediaInput::from(feed.clone());
        assert_eq!(input.as_segments(), Some(&feed));
        assert!(input.as_file().is_none());
        assert_eq!(input.display(), "/store");
        assert!(!SegmentFeed::new("/store").follow);
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(json["kind"], "segments");
        assert_eq!(json["dir"], "/store");
        assert_eq!(json["follow"], true);
        let back: MediaInput = serde_json::from_value(json).unwrap();
        assert_eq!(back, input);
        let json = serde_json::to_value(MediaInput::from(&p)).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"kind": "file", "path": "/tmp/a.mp4"})
        );
        assert_eq!(MediaInput::from("/tmp/a.mp4"), MediaInput::from(&p));
        assert_eq!(
            MediaInput::from(String::from("/tmp/a.mp4")),
            MediaInput::from(&p)
        );
    }

    #[test]
    fn truncated_index_is_a_protocol_error() {
        let dir = tempfile::tempdir().unwrap();
        let p = SegmentIndex::path_in(dir.path());
        index().save(&p).unwrap();
        let bytes = std::fs::read(&p).unwrap();
        std::fs::write(&p, &bytes[..bytes.len() / 2]).unwrap();
        let err = SegmentIndex::load(&p).unwrap_err();
        assert!(matches!(err, MediaError::Protocol(_)), "{err:?}");
        assert!(err.to_string().contains("index.json"), "{err}");
        // Missing file is an io error, not a panic.
        assert!(matches!(
            SegmentIndex::load(&dir.path().join("nope.json")),
            Err(MediaError::Io(_))
        ));
        // A newer schema is refused.
        std::fs::write(
            &p,
            r#"{"schema":9,"timebase":{"num":1,"den":90000},"segments":[]}"#,
        )
        .unwrap();
        assert!(matches!(
            SegmentIndex::load(&p),
            Err(MediaError::Protocol(_))
        ));
    }
}
