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
/// Least longest side of a `zoom` frame after scaling, pixels: a small crop
/// is enlarged to this so the model reads it at full size.
pub const ZOOM_MIN_SIDE: u32 = 1024;
/// Most, so a source-size frame of a 4K video stays inside image limits.
pub const ZOOM_MAX_SIDE: u32 = 1600;
/// Least side of a `zoom` crop in source pixels; a smaller region is widened
/// around its centre.
pub const ZOOM_MIN_CROP_PX: u32 = 64;

/// A region of a frame in fractions of its width and height (0–1), from the
/// top-left corner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub w: f64,
    /// Height.
    pub h: f64,
}

impl Region {
    /// Inside the unit square with a positive size.
    pub fn clamped(self) -> Self {
        let coord = |v: f64| {
            if v.is_finite() {
                v.clamp(0.0, 0.999)
            } else {
                0.0
            }
        };
        let x = coord(self.x);
        let y = coord(self.y);
        let size = |v: f64, from: f64| {
            if v.is_finite() && v > 0.0 {
                v.min(1.0 - from)
            } else {
                1.0 - from
            }
        };
        Self {
            x,
            y,
            w: size(self.w, x),
            h: size(self.h, y),
        }
    }

    /// Pixel rectangle `(x, y, w, h)` inside a `width × height` frame, at
    /// least [`ZOOM_MIN_CROP_PX`] a side where the frame allows.
    pub fn pixels(self, width: u32, height: u32) -> (u32, u32, u32, u32) {
        let r = self.clamped();
        let (fw, fh) = (f64::from(width.max(1)), f64::from(height.max(1)));
        let min = f64::from(ZOOM_MIN_CROP_PX);
        let axis = |from: f64, size: f64, full: f64| -> (u32, u32) {
            let mut a = from * full;
            let mut b = (from + size) * full;
            if b - a < min {
                let c = (a + b) / 2.0;
                a = c - min / 2.0;
                b = c + min / 2.0;
            }
            if a < 0.0 {
                b -= a;
                a = 0.0;
            }
            if b > full {
                a -= b - full;
                b = full;
            }
            let a = a.max(0.0);
            let start = a.floor() as u32;
            let end = (b.ceil() as u32).min(full as u32);
            (start, end.saturating_sub(start).max(1))
        };
        let (x, w) = axis(r.x, r.w, fw);
        let (y, h) = axis(r.y, r.h, fh);
        (x, y, w, h)
    }
}

/// One frame as PNG, after an optional crop and the scaling `zoom` applies.
#[derive(Debug, Clone)]
pub struct FramePng {
    /// PNG bytes.
    pub png: Vec<u8>,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// The crop taken from the source frame, pixels `(x, y, w, h)`.
    pub crop: Option<(u32, u32, u32, u32)>,
}

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

/// Decode the frame nearest `t` at source size (`max_dim` 0) or scaled to
/// `max_dim`.
pub async fn decode_frame(
    worker: &WorkerConfig,
    video: &Video,
    t: f64,
    max_dim: u32,
) -> Result<Arc<FrameBuffer>> {
    let path = media_path(video).filter(|p| p.is_file()).ok_or_else(|| {
        Error::NotFound(format!(
            "media file for video {} is not on this machine",
            video.id
        ))
    })?;
    let duration = video.duration.as_secs_f64();
    let t = if t.is_finite() {
        t.clamp(0.0, (duration - 0.1).max(0.0))
    } else {
        0.0
    };
    let decode = VideoDecodeRequest::new(&path, 2.0, max_dim).range(t, Some(t + 0.6));
    let mut stream = vi_media::decode_video(worker, decode).await?;
    match stream.next().await? {
        Some(f) => Ok(f.to_owned_frame()),
        None => Err(Error::media(format!(
            "no frame decoded at {t:.1} s of {}",
            path.display()
        ))),
    }
}

/// Crop `region` out of the frame (when given), scale so the longest side is
/// at least `min_side` and at most [`ZOOM_MAX_SIDE`] (bilinear), encode PNG.
pub fn frame_to_png(
    img: image::RgbImage,
    region: Option<Region>,
    min_side: u32,
) -> Result<FramePng> {
    let (fw, fh) = (img.width(), img.height());
    let (img, crop) = match region {
        Some(r) => {
            let (x, y, w, h) = r.pixels(fw, fh);
            (
                image::imageops::crop_imm(&img, x, y, w, h).to_image(),
                Some((x, y, w, h)),
            )
        }
        None => (img, None),
    };
    let (w, h) = (img.width().max(1), img.height().max(1));
    let longest = w.max(h);
    let target = longest.clamp(min_side.min(ZOOM_MAX_SIDE), ZOOM_MAX_SIDE);
    let img = if target != longest {
        let scale = f64::from(target) / f64::from(longest);
        let nw = ((f64::from(w) * scale).round() as u32).max(1);
        let nh = ((f64::from(h) * scale).round() as u32).max(1);
        image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let (width, height) = (img.width(), img.height());
    let mut png = std::io::Cursor::new(Vec::new());
    img.write_to(&mut png, image::ImageFormat::Png)
        .map_err(|e| Error::Other(format!("png encode: {e}")))?;
    Ok(FramePng {
        png: png.into_inner(),
        width,
        height,
        crop,
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

    #[test]
    fn regions_clamp_into_the_frame_and_keep_a_least_size() {
        let r = Region {
            x: 0.9,
            y: -0.2,
            w: 0.5,
            h: 0.5,
        }
        .clamped();
        assert!(r.x + r.w <= 1.0 + 1e-9 && r.y == 0.0 && r.h <= 0.5, "{r:?}");
        // A tiny region is widened to 64 px a side inside the frame.
        let (x, y, w, h) = Region {
            x: 0.99,
            y: 0.99,
            w: 0.005,
            h: 0.005,
        }
        .pixels(1280, 720);
        assert!(w >= 64 && h >= 64, "{w}x{h}");
        assert!(x + w <= 1280 && y + h <= 720, "{x},{y},{w},{h}");
        // The whole frame maps to the whole frame.
        assert_eq!(
            Region {
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0
            }
            .pixels(640, 360),
            (0, 0, 640, 360)
        );
    }

    #[test]
    fn small_frames_and_crops_are_enlarged_to_the_least_side() {
        let img = image::RgbImage::from_fn(640, 360, |x, _| image::Rgb([(x % 256) as u8, 0, 0]));
        let full = frame_to_png(img.clone(), None, ZOOM_MIN_SIDE).unwrap();
        assert_eq!((full.width, full.height), (1024, 576));
        assert!(full.crop.is_none());
        let crop = frame_to_png(
            img,
            Some(Region {
                x: 0.7,
                y: 0.0,
                w: 0.3,
                h: 0.15,
            }),
            ZOOM_MIN_SIDE,
        )
        .unwrap();
        assert_eq!(crop.width, 1024, "{crop:?}");
        assert!(crop.height > 100 && crop.height < 400, "{crop:?}");
        assert_eq!(crop.crop.map(|c| c.0), Some(448));
        // A huge frame is brought down to the ceiling.
        let big = image::RgbImage::new(3840, 2160);
        let out = frame_to_png(big, None, ZOOM_MIN_SIDE).unwrap();
        assert_eq!(out.width, ZOOM_MAX_SIDE);
    }
}
