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
    /// expired retention).
    #[serde(default)]
    pub gaps: Vec<TimeRange>,
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
        idx.gaps
            .push(TimeRange::new(Timestamp::from_secs(10), Timestamp::from_secs(14)).unwrap());
        idx.save(&p).unwrap();
        assert!(!dir.path().join("index.json.tmp").exists());
        let back = SegmentIndex::load(&p).unwrap();
        assert_eq!(back, idx);
        assert!(!back.ended);
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
