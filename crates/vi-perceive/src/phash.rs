//! 64-bit perceptual hash (pHash): grayscale, 32x32 box downsample, 2-D
//! DCT-II, the 8x8 low-frequency block thresholded at its median.
//!
//! Frames within Hamming distance [`PHASH_DEDUP_DISTANCE`] are treated as
//! the same picture by downstream operators.

use vi_media::{FrameBuffer, PixelFormat};

/// Hamming distance at or below which two frames count as duplicates.
pub const PHASH_DEDUP_DISTANCE: u32 = 6;

const N: usize = 32;
const LOW: usize = 8;

/// Hamming distance between two hashes.
pub fn hamming(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// pHash of an RGB24 frame. Returns `None` for other pixel formats or empty
/// frames.
pub fn phash_frame(frame: &FrameBuffer) -> Option<u64> {
    if frame.format != PixelFormat::Rgb24 || frame.width == 0 || frame.height == 0 {
        return None;
    }
    Some(phash_rgb(
        frame.data(),
        frame.width as usize,
        frame.height as usize,
        frame.stride,
    ))
}

/// pHash of packed RGB pixels with the given row stride.
pub fn phash_rgb(data: &[u8], width: usize, height: usize, stride: usize) -> u64 {
    let gray = downsample_gray(data, width, height, stride);
    let dct = dct2d(&gray);
    let mut low = [0f32; LOW * LOW];
    for y in 0..LOW {
        for x in 0..LOW {
            low[y * LOW + x] = dct[y * N + x];
        }
    }
    let mut sorted = low;
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = (sorted[LOW * LOW / 2 - 1] + sorted[LOW * LOW / 2]) * 0.5;
    let mut bits = 0u64;
    for (i, v) in low.iter().enumerate() {
        if *v > median {
            bits |= 1 << i;
        }
    }
    bits
}

/// Box-filter the luma down to 32x32.
fn downsample_gray(data: &[u8], width: usize, height: usize, stride: usize) -> [f32; N * N] {
    let mut out = [0f32; N * N];
    for gy in 0..N {
        let y0 = gy * height / N;
        let y1 = ((gy + 1) * height / N).max(y0 + 1).min(height);
        for gx in 0..N {
            let x0 = gx * width / N;
            let x1 = ((gx + 1) * width / N).max(x0 + 1).min(width);
            let mut acc = 0f32;
            let mut n = 0f32;
            for y in y0..y1 {
                let row = &data[y * stride..];
                for x in x0..x1 {
                    let p = &row[x * 3..x * 3 + 3];
                    acc +=
                        0.299 * f32::from(p[0]) + 0.587 * f32::from(p[1]) + 0.114 * f32::from(p[2]);
                    n += 1.0;
                }
            }
            out[gy * N + gx] = if n > 0.0 { acc / n } else { 0.0 };
        }
    }
    out
}

fn cos_table() -> [f32; N * N] {
    let mut t = [0f32; N * N];
    for k in 0..N {
        for n in 0..N {
            t[k * N + n] = ((std::f32::consts::PI / N as f32) * (n as f32 + 0.5) * k as f32).cos();
        }
    }
    t
}

/// Separable 2-D DCT-II (unnormalised; normalisation does not affect the
/// median threshold).
fn dct2d(input: &[f32; N * N]) -> [f32; N * N] {
    let c = cos_table();
    let mut rows = [0f32; N * N];
    for y in 0..N {
        for k in 0..N {
            let mut s = 0f32;
            for n in 0..N {
                s += input[y * N + n] * c[k * N + n];
            }
            rows[y * N + k] = s;
        }
    }
    let mut out = [0f32; N * N];
    for x in 0..N {
        for k in 0..N {
            let mut s = 0f32;
            for n in 0..N {
                s += rows[n * N + x] * c[k * N + n];
            }
            out[k * N + x] = s;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(width: usize, height: usize, f: impl Fn(usize, usize) -> [u8; 3]) -> Vec<u8> {
        let mut v = vec![0u8; width * height * 3];
        for y in 0..height {
            for x in 0..width {
                let p = f(x, y);
                v[(y * width + x) * 3..(y * width + x) * 3 + 3].copy_from_slice(&p);
            }
        }
        v
    }

    fn box_pattern(bx: usize, by: usize) -> impl Fn(usize, usize) -> [u8; 3] {
        move |x, y| {
            if x >= bx && x < bx + 200 && y >= by && y < by + 120 {
                [255, 255, 255]
            } else {
                [40, 40, 160]
            }
        }
    }

    #[test]
    fn identical_frames_hash_equal() {
        let a = pattern(640, 360, box_pattern(0, 0));
        let h1 = phash_rgb(&a, 640, 360, 640 * 3);
        let h2 = phash_rgb(&a, 640, 360, 640 * 3);
        assert_eq!(h1, h2);
        assert_eq!(hamming(h1, h2), 0);
    }

    #[test]
    fn scale_and_noise_invariant() {
        let big = pattern(640, 360, box_pattern(120, 60));
        // 2x2 box downsample of `big`.
        let mut small = vec![0u8; 320 * 180 * 3];
        for y in 0..180 {
            for x in 0..320 {
                for c in 0..3 {
                    let mut acc = 0u32;
                    for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                        acc += u32::from(big[((2 * y + dy) * 640 + 2 * x + dx) * 3 + c]);
                    }
                    small[(y * 320 + x) * 3 + c] = (acc / 4) as u8;
                }
            }
        }
        let hb = phash_rgb(&big, 640, 360, 640 * 3);
        let hs = phash_rgb(&small, 320, 180, 320 * 3);
        assert!(
            hamming(hb, hs) <= PHASH_DEDUP_DISTANCE,
            "scale: {}",
            hamming(hb, hs)
        );

        // Per-pixel sensor-style noise (deterministic LCG, +-4) does not move
        // the hash beyond the dedup threshold.
        let mut seed = 12345u32;
        let noisy: Vec<u8> = big
            .iter()
            .map(|p| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let d = (seed >> 24) as i16 % 9 - 4;
                (i16::from(*p) + d).clamp(0, 255) as u8
            })
            .collect();
        let hn = phash_rgb(&noisy, 640, 360, 640 * 3);
        assert!(
            hamming(hb, hn) <= PHASH_DEDUP_DISTANCE,
            "noise: {}",
            hamming(hb, hn)
        );

        // A small high-contrast overlay (burned-in timestamp) on this flat
        // synthetic picture moves a few bits; it must stay well under the
        // distance between genuinely different layouts (tested below).
        let mut overlaid = big.clone();
        for y in 10..34 {
            for x in 520..620 {
                let i = (y * 640 + x) * 3;
                let v = if (x / 4 + y / 4) % 2 == 0 { 255 } else { 0 };
                overlaid[i..i + 3].copy_from_slice(&[v, v, v]);
            }
        }
        let ho = phash_rgb(&overlaid, 640, 360, 640 * 3);
        assert!(hamming(hb, ho) <= 12, "overlay: {}", hamming(hb, ho));
    }

    #[test]
    fn different_layouts_hash_far_apart() {
        let a = pattern(640, 360, box_pattern(0, 0));
        let b = pattern(640, 360, box_pattern(400, 200));
        let c = pattern(640, 360, |x, y| {
            let v = ((x / 20 + y / 20) % 2 * 255) as u8;
            [v, v, v]
        });
        let (ha, hb, hc) = (
            phash_rgb(&a, 640, 360, 640 * 3),
            phash_rgb(&b, 640, 360, 640 * 3),
            phash_rgb(&c, 640, 360, 640 * 3),
        );
        assert!(
            hamming(ha, hb) > PHASH_DEDUP_DISTANCE * 2,
            "{}",
            hamming(ha, hb)
        );
        assert!(
            hamming(ha, hc) > PHASH_DEDUP_DISTANCE * 2,
            "{}",
            hamming(ha, hc)
        );
        assert!(
            hamming(hb, hc) > PHASH_DEDUP_DISTANCE * 2,
            "{}",
            hamming(hb, hc)
        );
    }

    #[test]
    fn stride_padding_is_ignored() {
        let tight = pattern(64, 32, box_pattern(10, 5));
        let mut padded = vec![0u8; 32 * 256];
        for y in 0..32 {
            padded[y * 256..y * 256 + 64 * 3].copy_from_slice(&tight[y * 192..(y + 1) * 192]);
        }
        assert_eq!(
            phash_rgb(&tight, 64, 32, 192),
            phash_rgb(&padded, 64, 32, 256)
        );
    }
}
