//! Decoded frames. A [`FrameBuffer`] is immutable once built and is shared
//! between operators as `Arc<FrameBuffer>`; nothing copies pixels after the
//! worker hands them over.

use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use vi_core::Timestamp;

use crate::shm::SlotGuard;

/// Pixel layout of a [`FrameBuffer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelFormat {
    /// Packed 8-bit RGB, 3 bytes per pixel.
    Rgb24,
    /// Planar Y followed by interleaved UV at half resolution.
    Nv12,
    /// Single 8-bit luma plane.
    Gray8,
}

impl PixelFormat {
    /// Bytes per pixel in the first plane.
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            Self::Rgb24 => 3,
            Self::Nv12 | Self::Gray8 => 1,
        }
    }

    /// Total bytes for a tightly packed frame of this size.
    pub fn frame_size(&self, width: u32, height: u32) -> usize {
        let (w, h) = (width as usize, height as usize);
        match self {
            Self::Rgb24 => w * h * 3,
            Self::Gray8 => w * h,
            Self::Nv12 => w * h + 2 * w.div_ceil(2) * h.div_ceil(2),
        }
    }
}

/// Where a frame's bytes live.
enum FrameData {
    /// Heap-owned copy.
    Owned(Vec<u8>),
    /// A slot in the worker's shared-memory region; released on drop.
    Shared(SlotGuard),
}

/// One decoded frame at a known time.
pub struct FrameBuffer {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Bytes per row of the first plane.
    pub stride: usize,
    /// Pixel layout.
    pub format: PixelFormat,
    /// Presentation timestamp in the track's timebase.
    pub pts: i64,
    /// Presentation time as a rational in seconds.
    pub t: Timestamp,
    /// Whether the decoder flagged the frame as a keyframe.
    pub is_keyframe: bool,
    /// Original (pre-scale) frame size, so thumbnails know the source size.
    pub source_width: u32,
    /// Original (pre-scale) frame height.
    pub source_height: u32,
    data: FrameData,
}

impl fmt::Debug for FrameBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FrameBuffer")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("stride", &self.stride)
            .field("format", &self.format)
            .field("pts", &self.pts)
            .field("t", &self.t.to_string())
            .field("is_keyframe", &self.is_keyframe)
            .field(
                "data",
                &match &self.data {
                    FrameData::Owned(v) => format!("owned({} bytes)", v.len()),
                    FrameData::Shared(s) => format!("shm slot {} ({} bytes)", s.slot(), s.len()),
                },
            )
            .finish()
    }
}

/// Timing and geometry that accompany pixel data.
#[derive(Debug, Clone, Copy)]
pub struct FrameMeta {
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Row stride in bytes.
    pub stride: usize,
    /// Pixel layout.
    pub format: PixelFormat,
    /// PTS in the track timebase.
    pub pts: i64,
    /// Time in seconds-rational.
    pub t: Timestamp,
    /// Keyframe flag.
    pub is_keyframe: bool,
    /// Source width.
    pub source_width: u32,
    /// Source height.
    pub source_height: u32,
}

impl FrameBuffer {
    /// Build a frame over heap memory. `data.len()` must be at least
    /// `stride * height` for the first plane.
    pub fn owned(meta: FrameMeta, data: Vec<u8>) -> Arc<Self> {
        Arc::new(Self::from_parts(meta, FrameData::Owned(data)))
    }

    /// Build a frame over a shared-memory slot.
    pub fn shared(meta: FrameMeta, slot: SlotGuard) -> Arc<Self> {
        Arc::new(Self::from_parts(meta, FrameData::Shared(slot)))
    }

    fn from_parts(meta: FrameMeta, data: FrameData) -> Self {
        Self {
            width: meta.width,
            height: meta.height,
            stride: meta.stride,
            format: meta.format,
            pts: meta.pts,
            t: meta.t,
            is_keyframe: meta.is_keyframe,
            source_width: meta.source_width,
            source_height: meta.source_height,
            data,
        }
    }

    /// All bytes of the frame, first plane first.
    pub fn data(&self) -> &[u8] {
        match &self.data {
            FrameData::Owned(v) => v.as_slice(),
            FrameData::Shared(s) => s.as_slice(),
        }
    }

    /// One row of the first plane, without padding.
    pub fn row(&self, y: u32) -> &[u8] {
        let start = y as usize * self.stride;
        let len = self.width as usize * self.format.bytes_per_pixel();
        let d = self.data();
        let end = (start + len).min(d.len());
        &d[start.min(end)..end]
    }

    /// True when the pixels live in shared memory (a slot is held).
    pub fn is_shared(&self) -> bool {
        matches!(self.data, FrameData::Shared(_))
    }

    /// Copy into heap memory, releasing any shared slot.
    pub fn to_owned_frame(&self) -> Arc<FrameBuffer> {
        Self::owned(self.meta(), self.data().to_vec())
    }

    /// Timing and geometry without the pixels.
    pub fn meta(&self) -> FrameMeta {
        FrameMeta {
            width: self.width,
            height: self.height,
            stride: self.stride,
            format: self.format,
            pts: self.pts,
            t: self.t,
            is_keyframe: self.is_keyframe,
            source_width: self.source_width,
            source_height: self.source_height,
        }
    }

    /// RGB triple at a pixel, for tests and hashing. `None` if out of range or
    /// not RGB24.
    pub fn rgb_at(&self, x: u32, y: u32) -> Option<[u8; 3]> {
        if self.format != PixelFormat::Rgb24 || x >= self.width || y >= self.height {
            return None;
        }
        let row = self.row(y);
        let i = x as usize * 3;
        Some([row[i], row[i + 1], row[i + 2]])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_frame_rows_respect_stride() {
        let meta = FrameMeta {
            width: 2,
            height: 2,
            stride: 8, // padded
            format: PixelFormat::Rgb24,
            pts: 0,
            t: Timestamp::ZERO,
            is_keyframe: true,
            source_width: 2,
            source_height: 2,
        };
        let mut data = vec![0u8; 16];
        data[8..14].copy_from_slice(&[1, 2, 3, 4, 5, 6]);
        let f = FrameBuffer::owned(meta, data);
        assert_eq!(f.row(1), &[1, 2, 3, 4, 5, 6]);
        assert_eq!(f.rgb_at(1, 1), Some([4, 5, 6]));
        assert_eq!(f.rgb_at(2, 1), None);
        assert!(!f.is_shared());
    }

    #[test]
    fn frame_sizes() {
        assert_eq!(PixelFormat::Rgb24.frame_size(4, 2), 24);
        assert_eq!(PixelFormat::Nv12.frame_size(4, 2), 8 + 4);
        assert_eq!(PixelFormat::Nv12.frame_size(3, 3), 9 + 2 * 2 * 2);
    }
}
