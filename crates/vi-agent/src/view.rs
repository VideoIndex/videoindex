//! The `view` primitive: decode a window of one video at a chosen rate,
//! compose a labelled frame grid, encode it as PNG.

use std::path::PathBuf;
use std::sync::Arc;

use vi_core::config::WorkerConfig;
use vi_core::model::Video;
use vi_core::{Error, Result, Timestamp};
use vi_media::{FrameBuffer, VideoDecodeRequest};
use vi_perceive::grid::{compose, hms, GridLayout, Tile};

/// Longest window a single `view` may cover, seconds.
pub const MAX_WINDOW_SECS: f64 = 120.0;
/// Most frames in one grid.
pub const MAX_FRAMES: usize = 16;

/// What to look at.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewRequest {
    /// Start, seconds.
    pub t0: f64,
    /// End, seconds.
    pub t1: f64,
    /// Frames per second to sample.
    pub fps: f64,
    /// Longest side of each decoded frame before tiling.
    pub max_dim: u32,
    /// Grid columns.
    pub cols: u32,
    /// Tile width in the grid.
    pub tile_width: u32,
    /// Most frames in this grid (at most [`MAX_FRAMES`]); fps is lowered to
    /// fit. Every grid of a multi-window `view` gets the full count too.
    pub max_frames: usize,
}

impl Default for ViewRequest {
    fn default() -> Self {
        Self {
            t0: 0.0,
            t1: 30.0,
            fps: 1.0,
            max_dim: 640,
            cols: 3,
            tile_width: 448,
            max_frames: MAX_FRAMES,
        }
    }
}

impl ViewRequest {
    /// Clamp to the limits: window at most [`MAX_WINDOW_SECS`], frame count
    /// at most `max_frames` (never above [`MAX_FRAMES`]; lowering fps), inside
    /// the video.
    pub fn clamped(mut self, duration_secs: f64) -> Self {
        self.max_frames = self.max_frames.clamp(1, MAX_FRAMES);
        self.t0 = self.t0.clamp(0.0, duration_secs.max(0.0));
        self.t1 = self
            .t1
            .clamp(self.t0 + 0.5, duration_secs.max(self.t0 + 0.5));
        if self.t1 - self.t0 > MAX_WINDOW_SECS {
            self.t1 = self.t0 + MAX_WINDOW_SECS;
        }
        if self.fps.is_nan() || self.fps <= 0.0 {
            self.fps = 1.0;
        }
        let frames = (self.t1 - self.t0) * self.fps;
        if frames > self.max_frames as f64 {
            self.fps = self.max_frames as f64 / (self.t1 - self.t0);
        }
        self
    }
}

/// A rendered grid.
#[derive(Debug, Clone)]
pub struct ViewResult {
    /// PNG bytes.
    pub png: Vec<u8>,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Timestamps of the tiles, seconds.
    pub timestamps: Vec<f64>,
    /// pHash-distinct frame count (frames that differ from their predecessor).
    pub distinct: usize,
}

/// Media file for a video, from its stored probe.
pub fn media_path(video: &Video) -> Option<PathBuf> {
    video
        .probe
        .get("path")
        .and_then(|p| p.as_str())
        .map(PathBuf::from)
}

/// Decode and render.
pub async fn render_view(
    worker: &WorkerConfig,
    video: &Video,
    req: ViewRequest,
) -> Result<ViewResult> {
    let path = media_path(video).filter(|p| p.is_file()).ok_or_else(|| {
        Error::NotFound(format!(
            "media file for video {} is not on this machine",
            video.id
        ))
    })?;
    let req = req.clamped(video.duration.as_secs_f64());
    let decode = VideoDecodeRequest::new(&path, req.fps, req.max_dim).range(req.t0, Some(req.t1));
    let mut stream = vi_media::decode_video(worker, decode).await?;
    let mut frames: Vec<Arc<FrameBuffer>> = Vec::new();
    while let Some(f) = stream.next().await? {
        frames.push(f.to_owned_frame());
        if frames.len() >= req.max_frames {
            break;
        }
    }
    if frames.is_empty() {
        return Err(Error::media(format!(
            "no frames decoded in [{:.1}, {:.1}) of {}",
            req.t0,
            req.t1,
            path.display()
        )));
    }
    let mut distinct = 1;
    let mut prev: Option<u64> = None;
    for f in &frames {
        let h = vi_perceive::phash::phash_frame(f).unwrap_or(0);
        if let Some(p) = prev {
            if vi_perceive::hamming(p, h) > vi_perceive::PHASH_DEDUP_DISTANCE {
                distinct += 1;
            }
        }
        prev = Some(h);
    }
    let tiles: Vec<Tile<'_>> = frames
        .iter()
        .map(|f| Tile {
            frame: f,
            label: hms(f.t.as_secs_f64()),
        })
        .collect();
    let layout = GridLayout {
        cols: req.cols.max(1),
        tile_width: req.tile_width,
        ..GridLayout::default()
    };
    let grid = compose(&tiles, layout);
    let img = image::RgbImage::from_raw(grid.width, grid.height, grid.rgb)
        .ok_or_else(|| Error::Other("grid buffer size mismatch".into()))?;
    let mut png = std::io::Cursor::new(Vec::new());
    img.write_to(&mut png, image::ImageFormat::Png)
        .map_err(|e| Error::Other(format!("png encode: {e}")))?;
    Ok(ViewResult {
        png: png.into_inner(),
        width: grid.width,
        height: grid.height,
        timestamps: frames.iter().map(|f| f.t.as_secs_f64()).collect(),
        distinct,
    })
}

/// Seconds as a `Timestamp` at millisecond precision.
pub fn ts(secs: f64) -> Timestamp {
    Timestamp::from_secs_f64(secs, 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamping_keeps_windows_and_frame_counts_bounded() {
        let r = ViewRequest {
            t0: 100.0,
            t1: 400.0,
            fps: 2.0,
            ..ViewRequest::default()
        }
        .clamped(3600.0);
        assert_eq!(r.t1 - r.t0, MAX_WINDOW_SECS);
        assert!((r.t1 - r.t0) * r.fps <= MAX_FRAMES as f64 + 1e-9);
        let r = ViewRequest {
            t0: 50.0,
            t1: 10.0,
            fps: -1.0,
            ..ViewRequest::default()
        }
        .clamped(60.0);
        assert!(r.t1 > r.t0 && r.fps > 0.0);
        let r = ViewRequest {
            t0: 5000.0,
            t1: 6000.0,
            ..ViewRequest::default()
        }
        .clamped(100.0);
        assert!(r.t0 <= 100.0 && r.t1 > r.t0);
        // A lower frame cap lowers fps the same way.
        let r = ViewRequest {
            t0: 0.0,
            t1: 60.0,
            fps: 1.0,
            max_frames: 12,
            ..ViewRequest::default()
        }
        .clamped(3600.0);
        assert!((r.t1 - r.t0) * r.fps <= 12.0 + 1e-9, "{r:?}");
        let r = ViewRequest {
            max_frames: 500,
            ..ViewRequest::default()
        }
        .clamped(3600.0);
        assert_eq!(r.max_frames, MAX_FRAMES);
    }
}
