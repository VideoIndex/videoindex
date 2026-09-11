//! Container and stream facts, libav-based. Runs inside the worker.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use vi_core::model::{Track, TrackKind};
use vi_core::{Timestamp, TrackId, VideoId};

/// ffprobe-equivalent output for one file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Probe {
    /// Path probed.
    pub path: String,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Short container name, e.g. `mov,mp4,m4a,3gp,3g2,mj2`.
    pub format_name: String,
    /// Long container name.
    pub format_long_name: String,
    /// Duration of the container.
    pub duration: Timestamp,
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

impl StreamInfo {
    /// Timebase as a Timestamp with `num = 1`, for converting PTS values.
    pub fn pts_to_timestamp(&self, pts: i64) -> Timestamp {
        Timestamp::new(
            pts.saturating_mul(i64::from(self.time_base_num)),
            self.time_base_den,
        )
    }
}
