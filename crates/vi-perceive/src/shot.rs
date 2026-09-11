//! Shot boundary detection: HSV colour histogram distance plus edge change
//! ratio between consecutive sampled frames, with an adaptive threshold
//! over the recent distances so slow camera motion, slide builds and
//! burned-in timers do not register as cuts while hard cuts do.
//!
//! Works on frames at the sampling rate (1 fps by default), so a "shot"
//! here is a run of visually continuous samples; a cut between two samples
//! is placed at the second sample's time.

use vi_media::{FrameBuffer, PixelFormat};

/// Hue bins.
const H_BINS: usize = 8;
/// Saturation bins.
const S_BINS: usize = 4;
/// Value bins.
const V_BINS: usize = 4;
/// Histogram length.
pub const HIST_LEN: usize = H_BINS * S_BINS * V_BINS;
/// Edge map width; height follows the aspect ratio, capped at [`EDGE_H_MAX`].
const EDGE_W: usize = 96;
/// Edge map maximum height.
const EDGE_H_MAX: usize = 96;
/// Gradient magnitude at or above which a pixel is an edge (0-255 scale).
const EDGE_THRESHOLD: f32 = 40.0;

/// What one frame contributes to the comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameSignature {
    /// Normalised HSV histogram (sums to 1).
    pub hist: Vec<f32>,
    /// Binary edge map, row-major `edge_w * edge_h`.
    pub edges: Vec<bool>,
    /// Edge map width.
    pub edge_w: usize,
    /// Edge map height.
    pub edge_h: usize,
    /// Number of edge pixels.
    pub edge_count: usize,
}

impl FrameSignature {
    /// Signature of an RGB24 frame. `None` for other formats or empty
    /// frames.
    pub fn from_frame(frame: &FrameBuffer) -> Option<Self> {
        if frame.format != PixelFormat::Rgb24 || frame.width == 0 || frame.height == 0 {
            return None;
        }
        Some(Self::from_rgb(
            frame.data(),
            frame.width as usize,
            frame.height as usize,
            frame.stride,
        ))
    }

    /// Signature of packed RGB pixels.
    pub fn from_rgb(data: &[u8], width: usize, height: usize, stride: usize) -> Self {
        // Downsample to a small grey image and a histogram in one pass.
        let edge_w = EDGE_W.min(width).max(1);
        let edge_h = ((height * edge_w) / width.max(1)).clamp(1, EDGE_H_MAX);
        let mut hist = vec![0f32; HIST_LEN];
        let mut gray = vec![0f32; edge_w * edge_h];
        let mut n = 0f32;
        for gy in 0..edge_h {
            let y0 = gy * height / edge_h;
            let y1 = ((gy + 1) * height / edge_h).max(y0 + 1).min(height);
            for gx in 0..edge_w {
                let x0 = gx * width / edge_w;
                let x1 = ((gx + 1) * width / edge_w).max(x0 + 1).min(width);
                let mut acc = [0f32; 3];
                let mut cnt = 0f32;
                // Sample a sparse grid inside the cell instead of every pixel.
                let step_y = ((y1 - y0) / 2).max(1);
                let step_x = ((x1 - x0) / 2).max(1);
                let mut y = y0;
                while y < y1 {
                    let row = &data[y * stride..];
                    let mut x = x0;
                    while x < x1 {
                        let p = &row[x * 3..x * 3 + 3];
                        acc[0] += f32::from(p[0]);
                        acc[1] += f32::from(p[1]);
                        acc[2] += f32::from(p[2]);
                        cnt += 1.0;
                        x += step_x;
                    }
                    y += step_y;
                }
                let (r, g, b) = (acc[0] / cnt, acc[1] / cnt, acc[2] / cnt);
                gray[gy * edge_w + gx] = 0.299 * r + 0.587 * g + 0.114 * b;
                let (h, s, v) = rgb_to_hsv(r, g, b);
                let hi = ((h / 360.0) * H_BINS as f32) as usize % H_BINS;
                let si = ((s * S_BINS as f32) as usize).min(S_BINS - 1);
                let vi = ((v / 255.0 * V_BINS as f32) as usize).min(V_BINS - 1);
                hist[(hi * S_BINS + si) * V_BINS + vi] += 1.0;
                n += 1.0;
            }
        }
        if n > 0.0 {
            for h in &mut hist {
                *h /= n;
            }
        }
        // Sobel edges on the small grey image.
        let mut edges = vec![false; edge_w * edge_h];
        let mut edge_count = 0;
        if edge_w >= 3 && edge_h >= 3 {
            for y in 1..edge_h - 1 {
                for x in 1..edge_w - 1 {
                    let g = |dx: isize, dy: isize| {
                        gray[((y as isize + dy) as usize) * edge_w + (x as isize + dx) as usize]
                    };
                    let gx =
                        -g(-1, -1) - 2.0 * g(-1, 0) - g(-1, 1) + g(1, -1) + 2.0 * g(1, 0) + g(1, 1);
                    let gy =
                        -g(-1, -1) - 2.0 * g(0, -1) - g(1, -1) + g(-1, 1) + 2.0 * g(0, 1) + g(1, 1);
                    // Sobel sums 4 weights per axis; scale to the 0-255 range.
                    if (gx * gx + gy * gy).sqrt() / 4.0 >= EDGE_THRESHOLD {
                        edges[y * edge_w + x] = true;
                        edge_count += 1;
                    }
                }
            }
        }
        Self {
            hist,
            edges,
            edge_w,
            edge_h,
            edge_count,
        }
    }

    /// Histogram distance in `[0, 1]`: one minus the histogram intersection.
    pub fn hist_distance(&self, other: &Self) -> f32 {
        let inter: f32 = self
            .hist
            .iter()
            .zip(other.hist.iter())
            .map(|(a, b)| a.min(*b))
            .sum();
        (1.0 - inter).clamp(0.0, 1.0)
    }

    /// Edge change ratio in `[0, 1]`: the larger of the fraction of edges
    /// that appeared and the fraction that disappeared, with one pixel of
    /// tolerance for motion. Frames with almost no edges compare as 0.
    pub fn edge_change_ratio(&self, other: &Self) -> f32 {
        if self.edge_w != other.edge_w || self.edge_h != other.edge_h {
            return 1.0;
        }
        let min_edges = (self.edge_w * self.edge_h) / 200; // 0.5% of pixels
        if self.edge_count < min_edges && other.edge_count < min_edges {
            return 0.0;
        }
        let dil_a = dilate(&self.edges, self.edge_w, self.edge_h);
        let dil_b = dilate(&other.edges, other.edge_w, other.edge_h);
        let mut exited = 0usize; // in self, not near anything in other
        let mut entered = 0usize; // in other, not near anything in self
        for i in 0..self.edges.len() {
            if self.edges[i] && !dil_b[i] {
                exited += 1;
            }
            if other.edges[i] && !dil_a[i] {
                entered += 1;
            }
        }
        let out = if self.edge_count > 0 {
            exited as f32 / self.edge_count as f32
        } else {
            1.0
        };
        let inn = if other.edge_count > 0 {
            entered as f32 / other.edge_count as f32
        } else {
            1.0
        };
        out.max(inn).clamp(0.0, 1.0)
    }

    /// Combined distance used for cut detection.
    pub fn distance(&self, other: &Self) -> f32 {
        0.5 * self.hist_distance(other) + 0.5 * self.edge_change_ratio(other)
    }
}

fn dilate(e: &[bool], w: usize, h: usize) -> Vec<bool> {
    let mut out = e.to_vec();
    for y in 0..h {
        for x in 0..w {
            if e[y * w + x] {
                for dy in -1isize..=1 {
                    for dx in -1isize..=1 {
                        let (nx, ny) = (x as isize + dx, y as isize + dy);
                        if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                            out[ny as usize * w + nx as usize] = true;
                        }
                    }
                }
            }
        }
    }
    out
}

/// RGB (0-255) to HSV (h in degrees, s in 0-1, v in 0-255).
pub fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let h = if delta < 1e-6 {
        0.0
    } else if (max - r).abs() < 1e-6 {
        60.0 * (((g - b) / delta) % 6.0)
    } else if (max - g).abs() < 1e-6 {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    let h = if h < 0.0 { h + 360.0 } else { h };
    let s = if max < 1e-6 { 0.0 } else { delta / max };
    (h, s, max)
}

/// Adaptive-threshold parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShotParams {
    /// A distance below this is never a cut.
    pub min_distance: f32,
    /// A distance at or above this is always a cut.
    pub hard_distance: f32,
    /// Between those, a cut needs `distance > mean + k * std` over the
    /// recent window and `distance > ratio * mean`.
    pub k_std: f32,
    /// Ratio to the recent mean distance.
    pub ratio: f32,
    /// Recent distances remembered.
    pub window: usize,
    /// Minimum samples per shot (cuts closer than this are ignored).
    pub min_shot_samples: usize,
}

impl Default for ShotParams {
    fn default() -> Self {
        Self {
            min_distance: 0.25,
            hard_distance: 0.7,
            k_std: 3.0,
            ratio: 2.5,
            window: 20,
            min_shot_samples: 2,
        }
    }
}

/// Streaming detector over sampled frames in time order.
#[derive(Debug)]
pub struct ShotDetector {
    params: ShotParams,
    prev: Option<FrameSignature>,
    recent: std::collections::VecDeque<f32>,
    samples: usize,
    since_cut: usize,
    /// Indices (sample ordinals) where a new shot starts.
    cuts: Vec<usize>,
    /// Distance at each processed sample (0 for the first).
    distances: Vec<f32>,
}

impl ShotDetector {
    /// New detector.
    pub fn new(params: ShotParams) -> Self {
        Self {
            params,
            prev: None,
            recent: std::collections::VecDeque::with_capacity(params.window + 1),
            samples: 0,
            since_cut: 0,
            cuts: Vec::new(),
            distances: Vec::new(),
        }
    }

    /// Feed the next sample's signature. Returns `true` when a new shot
    /// starts at this sample.
    pub fn push(&mut self, sig: FrameSignature) -> bool {
        let idx = self.samples;
        self.samples += 1;
        self.since_cut += 1;
        let Some(prev) = &self.prev else {
            self.prev = Some(sig);
            self.distances.push(0.0);
            return false;
        };
        let d = prev.distance(&sig);
        self.distances.push(d);
        let (mean, std) = stats(&self.recent);
        let p = &self.params;
        let adaptive = if self.recent.len() >= 3 {
            d > mean + p.k_std * std && d > p.ratio * mean.max(0.02)
        } else {
            // Not enough history: rely on the absolute band.
            d >= (p.min_distance + p.hard_distance) / 2.0
        };
        let is_cut = d >= p.min_distance
            && (d >= p.hard_distance || adaptive)
            && self.since_cut > p.min_shot_samples;
        if is_cut {
            self.cuts.push(idx);
            self.since_cut = 0;
            // A cut is not "recent motion"; keep the window about continuity.
            self.recent.clear();
        } else {
            self.recent.push_back(d);
            while self.recent.len() > p.window {
                self.recent.pop_front();
            }
        }
        self.prev = Some(sig);
        is_cut
    }

    /// Sample ordinals at which new shots start (never includes 0).
    pub fn cuts(&self) -> &[usize] {
        &self.cuts
    }

    /// Distances per sample, for diagnostics.
    pub fn distances(&self) -> &[f32] {
        &self.distances
    }

    /// Samples seen.
    pub fn samples(&self) -> usize {
        self.samples
    }
}

fn stats(v: &std::collections::VecDeque<f32>) -> (f32, f32) {
    if v.is_empty() {
        return (0.0, 0.0);
    }
    let n = v.len() as f32;
    let mean = v.iter().sum::<f32>() / n;
    let var = v.iter().map(|x| (x - mean) * (x - mean)).sum::<f32>() / n;
    (mean, var.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: usize, h: usize, rgb: [u8; 3], box_x: usize) -> Vec<u8> {
        let mut d = vec![0u8; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                let inside = x >= box_x && x < box_x + w / 4 && y >= h / 4 && y < h / 2;
                let c = if inside { [255, 255, 255] } else { rgb };
                d[i..i + 3].copy_from_slice(&c);
            }
        }
        d
    }

    #[test]
    fn hsv_conversion() {
        assert_eq!(rgb_to_hsv(255.0, 0.0, 0.0).0, 0.0);
        assert!((rgb_to_hsv(0.0, 255.0, 0.0).0 - 120.0).abs() < 1e-3);
        assert!((rgb_to_hsv(0.0, 0.0, 255.0).0 - 240.0).abs() < 1e-3);
        let (_, s, v) = rgb_to_hsv(128.0, 128.0, 128.0);
        assert_eq!((s, v), (0.0, 128.0));
    }

    #[test]
    fn identical_frames_have_zero_distance_and_cuts_are_far() {
        let (w, h) = (320, 180);
        let a = FrameSignature::from_rgb(&solid(w, h, [200, 40, 40], 20), w, h, w * 3);
        let b = FrameSignature::from_rgb(&solid(w, h, [200, 40, 40], 20), w, h, w * 3);
        assert!(a.hist_distance(&b) < 1e-6);
        assert_eq!(a.edge_change_ratio(&b), 0.0);
        // Same colour, box moved by a few pixels: small distance.
        let c = FrameSignature::from_rgb(&solid(w, h, [200, 40, 40], 24), w, h, w * 3);
        assert!(a.distance(&c) < 0.25, "{}", a.distance(&c));
        // Different colour and box far away: large distance.
        let d = FrameSignature::from_rgb(&solid(w, h, [40, 40, 200], 200), w, h, w * 3);
        assert!(a.distance(&d) > 0.6, "{}", a.distance(&d));
    }

    #[test]
    fn detector_finds_hard_cuts_and_ignores_jitter() {
        let (w, h) = (320, 180);
        let mut det = ShotDetector::new(ShotParams::default());
        let colours = [
            [200u8, 40, 40],
            [40, 200, 40],
            [40, 40, 200],
            [200, 200, 40],
        ];
        let mut expected = Vec::new();
        let mut idx = 0;
        for (seg, c) in colours.iter().enumerate() {
            if seg > 0 {
                expected.push(idx);
            }
            for j in 0..10 {
                // Jitter the box by a pixel or two each sample.
                let sig = FrameSignature::from_rgb(
                    &solid(w, h, *c, 20 + seg * 40 + (j % 3)),
                    w,
                    h,
                    w * 3,
                );
                det.push(sig);
                idx += 1;
            }
        }
        assert_eq!(det.cuts(), expected.as_slice(), "{:?}", det.distances());
    }
}
