//! Frame grids for VLM calls and the agent's `view` tool: tile frames into
//! rows and columns, label each tile with its timestamp in a built-in
//! bitmap font (no font files, no text-rendering dependency), and return
//! packed RGB.

use vi_media::{FrameBuffer, PixelFormat};

/// A tile's source.
#[derive(Debug, Clone)]
pub struct Tile<'a> {
    /// Frame pixels (RGB24).
    pub frame: &'a FrameBuffer,
    /// Label, usually `HH:MM:SS`.
    pub label: String,
}

/// Layout parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridLayout {
    /// Columns.
    pub cols: u32,
    /// Tile width in pixels; height follows the frame aspect.
    pub tile_width: u32,
    /// Gap between tiles.
    pub gap: u32,
    /// Font scale (pixels per font unit).
    pub font_scale: u32,
}

impl Default for GridLayout {
    fn default() -> Self {
        Self {
            cols: 3,
            tile_width: 448,
            gap: 4,
            font_scale: 3,
        }
    }
}

impl GridLayout {
    /// Parse `"3x3"` into columns (the row count follows the tile count).
    pub fn parse_cols(spec: &str) -> Option<u32> {
        spec.split('x').next()?.trim().parse().ok()
    }
}

/// A composed grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// Packed RGB, `width * height * 3`.
    pub rgb: Vec<u8>,
    /// Tile rectangles `(x, y, w, h)` in tile order.
    pub tiles: Vec<(u32, u32, u32, u32)>,
}

/// Compose tiles into a grid. Empty input gives an empty grid.
pub fn compose(tiles: &[Tile<'_>], layout: GridLayout) -> Grid {
    if tiles.is_empty() {
        return Grid {
            width: 0,
            height: 0,
            rgb: Vec::new(),
            tiles: Vec::new(),
        };
    }
    let cols = layout.cols.max(1);
    let rows = (tiles.len() as u32).div_ceil(cols);
    let tw = layout.tile_width.max(16);
    // Tile height from the first frame's aspect ratio.
    let f0 = tiles[0].frame;
    let th =
        ((u64::from(tw) * u64::from(f0.height.max(1))) / u64::from(f0.width.max(1))).max(9) as u32;
    let gap = layout.gap;
    let width = cols * tw + (cols + 1) * gap;
    let height = rows * th + (rows + 1) * gap;
    let mut rgb = vec![24u8; (width * height * 3) as usize];
    let mut rects = Vec::with_capacity(tiles.len());
    for (i, t) in tiles.iter().enumerate() {
        let c = i as u32 % cols;
        let r = i as u32 / cols;
        let x0 = gap + c * (tw + gap);
        let y0 = gap + r * (th + gap);
        blit_scaled(t.frame, &mut rgb, width, x0, y0, tw, th);
        draw_label(
            &mut rgb,
            width,
            height,
            x0 + 4,
            y0 + 4,
            &t.label,
            layout.font_scale,
        );
        rects.push((x0, y0, tw, th));
    }
    Grid {
        width,
        height,
        rgb,
        tiles: rects,
    }
}

/// Nearest-neighbour scale of an RGB24 frame into the grid.
fn blit_scaled(
    frame: &FrameBuffer,
    dst: &mut [u8],
    dst_w: u32,
    x0: u32,
    y0: u32,
    tw: u32,
    th: u32,
) {
    if frame.format != PixelFormat::Rgb24 || frame.width == 0 || frame.height == 0 {
        return;
    }
    for y in 0..th {
        let sy = (u64::from(y) * u64::from(frame.height) / u64::from(th)) as u32;
        let row = frame.row(sy.min(frame.height - 1));
        for x in 0..tw {
            let sx = ((u64::from(x) * u64::from(frame.width) / u64::from(tw)) as usize)
                .min(frame.width as usize - 1);
            let di = (((y0 + y) * dst_w + x0 + x) * 3) as usize;
            if di + 3 <= dst.len() && sx * 3 + 3 <= row.len() {
                dst[di..di + 3].copy_from_slice(&row[sx * 3..sx * 3 + 3]);
            }
        }
    }
}

/// 5x7 glyphs for `0-9`, `:` and `.`; each byte is a row, low 5 bits.
fn glyph(c: char) -> [u8; 7] {
    match c {
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        ':' => [0x00, 0x04, 0x04, 0x00, 0x04, 0x04, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        _ => [0x00; 7],
    }
}

/// Draw a label with a dark box behind it so it reads on any frame.
fn draw_label(dst: &mut [u8], w: u32, h: u32, x0: u32, y0: u32, text: &str, scale: u32) {
    let scale = scale.max(1);
    let cw = 6 * scale; // 5 px glyph + 1 px spacing
    let ch = 7 * scale;
    let pad = scale;
    let box_w = text.chars().count() as u32 * cw + 2 * pad;
    let box_h = ch + 2 * pad;
    for y in y0..(y0 + box_h).min(h) {
        for x in x0..(x0 + box_w).min(w) {
            let i = ((y * w + x) * 3) as usize;
            dst[i] = 0;
            dst[i + 1] = 0;
            dst[i + 2] = 0;
        }
    }
    for (ci, c) in text.chars().enumerate() {
        let g = glyph(c);
        for (gy, bits) in g.iter().enumerate() {
            for gx in 0..5u32 {
                if bits & (0x10 >> gx) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let x = x0 + pad + ci as u32 * cw + gx * scale + dx;
                        let y = y0 + pad + gy as u32 * scale + dy;
                        if x < w && y < h {
                            let i = ((y * w + x) * 3) as usize;
                            dst[i] = 255;
                            dst[i + 1] = 255;
                            dst[i + 2] = 255;
                        }
                    }
                }
            }
        }
    }
}

/// `HH:MM:SS` for a time in seconds.
pub fn hms(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vi_media::frame::FrameMeta;

    fn frame(w: u32, h: u32, rgb: [u8; 3]) -> std::sync::Arc<FrameBuffer> {
        let mut data = Vec::with_capacity((w * h * 3) as usize);
        for _ in 0..w * h {
            data.extend_from_slice(&rgb);
        }
        FrameBuffer::owned(
            FrameMeta {
                width: w,
                height: h,
                stride: (w * 3) as usize,
                format: PixelFormat::Rgb24,
                pts: 0,
                t: vi_core::Timestamp::ZERO,
                is_keyframe: true,
                source_width: w,
                source_height: h,
            },
            data,
        )
    }

    #[test]
    fn composes_tiles_with_labels() {
        let a = frame(64, 36, [200, 30, 30]);
        let b = frame(64, 36, [30, 200, 30]);
        let tiles = vec![
            Tile {
                frame: &a,
                label: hms(61.0),
            },
            Tile {
                frame: &b,
                label: hms(3725.4),
            },
        ];
        let g = compose(
            &tiles,
            GridLayout {
                cols: 2,
                tile_width: 128,
                gap: 2,
                font_scale: 2,
            },
        );
        assert_eq!(g.width, 2 * 128 + 3 * 2);
        assert_eq!(g.height, 72 + 2 * 2);
        assert_eq!(g.tiles.len(), 2);
        // Bottom-right pixel of the second tile is green.
        let (x, y, w, h) = g.tiles[1];
        let i = (((y + h - 1) * g.width + x + w - 1) * 3) as usize;
        assert_eq!(&g.rgb[i..i + 3], &[30, 200, 30]);
        // The label box is black at its top-left corner and has white glyph pixels.
        let (x, y, _, _) = g.tiles[0];
        let i = (((y + 4) * g.width + x + 4) * 3) as usize;
        assert_eq!(&g.rgb[i..i + 3], &[0, 0, 0]);
        let whites = g
            .rgb
            .chunks_exact(3)
            .filter(|p| p == &[255, 255, 255])
            .count();
        assert!(whites > 50, "{whites}");
        assert_eq!(hms(3725.4), "01:02:05");
        assert_eq!(GridLayout::parse_cols("3x3"), Some(3));
        assert!(compose(&[], GridLayout::default()).rgb.is_empty());
    }
}
