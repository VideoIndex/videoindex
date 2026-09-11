//! Thumbnails: resize an RGB frame so its longest side is `max_dim` pixels
//! and encode it as lossy WebP.

use image::imageops::FilterType;
use image::{ImageBuffer, Rgb, RgbImage};
use vi_media::{FrameBuffer, PixelFormat};

/// Thumbnail errors.
#[derive(Debug, thiserror::Error)]
pub enum ThumbnailError {
    /// Only RGB24 frames can be thumbnailed.
    #[error("unsupported pixel format {0:?}")]
    Format(PixelFormat),
    /// Frame has zero size.
    #[error("empty frame")]
    Empty,
    /// The encoder failed.
    #[error("webp encode failed: {0}")]
    Encode(String),
}

/// Build an `RgbImage` view (copy) of a frame, dropping row padding.
pub fn frame_to_rgb(frame: &FrameBuffer) -> Result<RgbImage, ThumbnailError> {
    if frame.format != PixelFormat::Rgb24 {
        return Err(ThumbnailError::Format(frame.format));
    }
    if frame.width == 0 || frame.height == 0 {
        return Err(ThumbnailError::Empty);
    }
    let (w, h) = (frame.width as usize, frame.height as usize);
    let mut buf = Vec::with_capacity(w * h * 3);
    for y in 0..frame.height {
        buf.extend_from_slice(frame.row(y));
    }
    ImageBuffer::<Rgb<u8>, Vec<u8>>::from_raw(frame.width, frame.height, buf)
        .ok_or(ThumbnailError::Empty)
}

/// Target size with the longest side at `max_dim`, never upscaling.
pub fn fit(width: u32, height: u32, max_dim: u32) -> (u32, u32) {
    if max_dim == 0 || (width <= max_dim && height <= max_dim) {
        return (width.max(1), height.max(1));
    }
    let scale = f64::from(max_dim) / f64::from(width.max(height));
    (
        ((f64::from(width) * scale).round() as u32).max(1),
        ((f64::from(height) * scale).round() as u32).max(1),
    )
}

/// Encode a WebP thumbnail. Returns the bytes and the thumbnail size.
pub fn encode_webp_thumbnail(
    frame: &FrameBuffer,
    max_dim: u32,
    quality: u8,
) -> Result<(Vec<u8>, u32, u32), ThumbnailError> {
    let img = frame_to_rgb(frame)?;
    let (tw, th) = fit(img.width(), img.height(), max_dim);
    let small = if (tw, th) == (img.width(), img.height()) {
        img
    } else {
        image::imageops::resize(&img, tw, th, FilterType::Triangle)
    };
    let enc = webp::Encoder::from_rgb(small.as_raw(), tw, th);
    let mem = enc
        .encode_simple(false, f32::from(quality.min(100)))
        .map_err(|e| ThumbnailError::Encode(format!("{e:?}")))?;
    Ok((mem.to_vec(), tw, th))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vi_core::Timestamp;
    use vi_media::frame::FrameMeta;

    fn frame(w: u32, h: u32) -> std::sync::Arc<FrameBuffer> {
        let mut data = vec![0u8; (w * h * 3) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 3) as usize;
                data[i] = (x * 255 / w) as u8;
                data[i + 1] = (y * 255 / h) as u8;
                data[i + 2] = 128;
            }
        }
        FrameBuffer::owned(
            FrameMeta {
                width: w,
                height: h,
                stride: (w * 3) as usize,
                format: PixelFormat::Rgb24,
                pts: 0,
                t: Timestamp::ZERO,
                is_keyframe: true,
                source_width: w,
                source_height: h,
            },
            data,
        )
    }

    #[test]
    fn fit_keeps_aspect_and_never_upscales() {
        assert_eq!(fit(1280, 720, 320), (320, 180));
        assert_eq!(fit(720, 1280, 320), (180, 320));
        assert_eq!(fit(200, 100, 320), (200, 100));
    }

    #[test]
    fn encodes_valid_webp() {
        let f = frame(640, 360);
        let (bytes, w, h) = encode_webp_thumbnail(&f, 320, 75).unwrap();
        assert_eq!((w, h), (320, 180));
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WEBP");
        assert!(bytes.len() < 640 * 360 * 3 / 10, "{} bytes", bytes.len());
        // Higher quality is not smaller.
        let (hq, _, _) = encode_webp_thumbnail(&f, 320, 95).unwrap();
        assert!(hq.len() >= bytes.len());
    }
}
