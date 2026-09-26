//! Container and stream facts, libav-based. Runs inside the worker.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use vi_core::model::{Track, TrackKind};
use vi_core::{Timestamp, TrackId, VideoId};

use crate::segments::{MediaInput, SegmentFeed};

/// ffprobe-equivalent output for one file or one segment feed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Probe {
    /// Path probed: the file, or the store directory of a segment feed.
    pub path: String,
    /// File size in bytes; for a feed, the bytes of every segment listed.
    pub size_bytes: u64,
    /// Short container name, e.g. `mov,mp4,m4a,3gp,3g2,mj2`.
    pub format_name: String,
    /// Long container name.
    pub format_long_name: String,
    /// Duration of the container; `None` when it is not known: a segment
    /// feed that is still being written (`follow`), or a container without
    /// a duration. Batch callers treat `None` as zero.
    pub duration: Option<Timestamp>,
    /// Container start time.
    pub start_time: Timestamp,
    /// Overall bit rate.
    pub bit_rate: i64,
    /// Streams.
    pub streams: Vec<StreamInfo>,
    /// Chapters.
    pub chapters: Vec<ChapterInfo>,
    /// Container metadata.
    pub metadata: BTreeMap<String, String>,
    /// Median gap between keyframes of the best video stream, seconds.
    pub keyframe_interval_secs: Option<f64>,
    /// libav version string.
    pub libav: String,
}

/// One elementary stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamInfo {
    /// Stream index.
    pub index: u32,
    /// Kind.
    pub kind: TrackKind,
    /// Codec short name.
    pub codec: String,
    /// Codec long name.
    pub codec_long_name: String,
    /// Timebase numerator.
    pub time_base_num: u32,
    /// Timebase denominator.
    pub time_base_den: u32,
    /// Start time in timebase units.
    pub start_time: i64,
    /// Duration.
    pub duration: Option<Timestamp>,
    /// Frame count when the container knows it.
    pub frames: Option<i64>,
    /// Width.
    pub width: Option<u32>,
    /// Height.
    pub height: Option<u32>,
    /// Pixel format name.
    pub pix_fmt: Option<String>,
    /// Real frame rate.
    pub fps: Option<f64>,
    /// Average frame rate.
    pub avg_fps: Option<f64>,
    /// Sample rate.
    pub sample_rate: Option<u32>,
    /// Channels.
    pub channels: Option<u32>,
    /// Sample format name.
    pub sample_fmt: Option<String>,
    /// Bit rate.
    pub bit_rate: Option<i64>,
    /// Language tag.
    pub language: Option<String>,
    /// Title tag.
    pub title: Option<String>,
    /// Default disposition.
    pub is_default: bool,
}

/// A chapter marker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChapterInfo {
    /// Chapter id.
    pub id: i64,
    /// Start.
    pub t0: Timestamp,
    /// End.
    pub t1: Timestamp,
    /// Title.
    pub title: Option<String>,
}

impl Probe {
    /// The best video stream, if any.
    pub fn video_stream(&self) -> Option<&StreamInfo> {
        self.streams
            .iter()
            .filter(|s| s.kind == TrackKind::Video)
            .max_by_key(|s| (s.is_default, s.width.unwrap_or(0) * s.height.unwrap_or(0)))
    }

    /// The best audio stream, if any.
    pub fn audio_stream(&self) -> Option<&StreamInfo> {
        self.streams
            .iter()
            .filter(|s| s.kind == TrackKind::Audio)
            .max_by_key(|s| (s.is_default, s.channels.unwrap_or(0)))
    }

    /// Build Track records for a Video from this probe.
    pub fn tracks(&self, video_id: VideoId) -> Vec<Track> {
        self.streams
            .iter()
            .map(|s| Track {
                id: TrackId::new(),
                video_id,
                kind: s.kind,
                stream_index: s.index,
                codec: s.codec.clone(),
                timebase_num: s.time_base_num,
                timebase_den: s.time_base_den,
                width: s.width,
                height: s.height,
                fps: s.fps.or(s.avg_fps),
                sample_rate: s.sample_rate,
                channels: s.channels,
                language: s.language.clone(),
            })
            .collect()
    }

    /// Title from container metadata.
    pub fn title(&self) -> Option<String> {
        self.metadata.get("title").cloned()
    }
}

/// Probe a file with libav. Blocking; call inside the worker.
pub(crate) fn probe_file(path: &Path) -> crate::Result<Probe> {
    crate::decode::probe_impl(path)
}

/// Probe a file or a segment feed. Blocking; call inside the worker.
pub(crate) fn probe_input(input: &MediaInput) -> crate::Result<Probe> {
    match input {
        MediaInput::File { path } => probe_file(path),
        MediaInput::Segments(feed) => probe_segments(feed),
    }
}

/// Probe a segment feed: read `index.json` and the first segment. No
/// single file is `stat`ed; the size is the sum of the listed segments and
/// the duration comes from the index, or is `None` while the feed is
/// followed.
fn probe_segments(feed: &SegmentFeed) -> crate::Result<Probe> {
    let index = feed.load_index()?;
    let first = index
        .segments
        .first()
        .ok_or_else(|| crate::MediaError::NoStream("segment", feed.dir.display().to_string()))?;
    let mut probe = probe_file(&feed.segment_path(first))?;
    probe.path = feed.dir.display().to_string();
    probe.size_bytes = index.segments.iter().map(|s| s.bytes).sum();
    probe.start_time = first.t0;
    probe.duration = if feed.follow {
        None
    } else {
        Some(index.duration())
    };
    // One segment says nothing about the whole recording's length.
    for s in &mut probe.streams {
        s.duration = None;
        s.frames = None;
    }
    probe.chapters.clear();
    probe
        .metadata
        .insert("live_store".to_string(), feed.dir.display().to_string());
    probe.metadata.insert(
        "live_ended".to_string(),
        if index.ended { "true" } else { "false" }.to_string(),
    );
    Ok(probe)
}

impl StreamInfo {
    /// Timebase as a Timestamp with `num = 1`, for converting PTS values.
    pub fn pts_to_timestamp(&self, pts: i64) -> Timestamp {
        Timestamp::new(
            pts.saturating_mul(i64::from(self.time_base_num)),
            self.time_base_den,
        )
    }
}
