//! RapidOCR (PaddleOCR models exported to ONNX): DB text detection plus a
//! CTC recogniser, all in-process. Tuned for slides and on-screen text:
//! axis-aligned boxes, English dictionary by default.
//!
//! Model directory layout (`SWHL/RapidOCR` files): a detector
//! (`ch_PP-OCRv4_det_infer.onnx` by default), a recogniser
//! (`en_PP-OCRv3_rec_infer.onnx`) and its dictionary (`en_dict.txt`).

use std::path::{Path, PathBuf};

use crate::onnx::{extract_f32, tensor_f32, Device, OnnxSession};
use crate::PerceiveError;

/// Default model directory name under `models.dir`.
pub const MODEL_DIR: &str = "rapidocr";
/// Where to get the files.
pub const MODEL_HINT: &str = "download PP-OCRv4/ch_PP-OCRv4_det_infer.onnx and PP-OCRv3/en_PP-OCRv3_rec_infer.onnx from huggingface.co/SWHL/RapidOCR and ppocr/utils/en_dict.txt from github.com/PaddlePaddle/PaddleOCR";

/// Files and thresholds.
#[derive(Debug, Clone, PartialEq)]
pub struct OcrConfig {
    /// Detector file name.
    pub det_model: String,
    /// Recogniser file name.
    pub rec_model: String,
    /// Dictionary file name (one character per line).
    pub dict: String,
    /// Longest side the detector sees.
    pub det_max_side: u32,
    /// Probability threshold for the binary map.
    pub det_threshold: f32,
    /// Mean probability a region needs to become a box.
    pub box_threshold: f32,
    /// Box expansion (DB unclip ratio).
    pub unclip_ratio: f32,
    /// Recogniser input height.
    pub rec_height: u32,
    /// Drop recognised lines below this mean confidence.
    pub min_confidence: f32,
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            det_model: "ch_PP-OCRv4_det_infer.onnx".into(),
            rec_model: "en_PP-OCRv3_rec_infer.onnx".into(),
            dict: "en_dict.txt".into(),
            det_max_side: 960,
            det_threshold: 0.3,
            box_threshold: 0.5,
            unclip_ratio: 1.6,
            rec_height: 48,
            min_confidence: 0.5,
        }
    }
}

/// A recognised line.
#[derive(Debug, Clone, PartialEq)]
pub struct OcrLine {
    /// Text.
    pub text: String,
    /// Box in normalised image coordinates: left, top, width, height.
    pub bbox: [f32; 4],
    /// Mean per-character confidence.
    pub confidence: f32,
}

/// A detected region in source pixel coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextBox {
    /// Left.
    pub x0: u32,
    /// Top.
    pub y0: u32,
    /// Right (exclusive).
    pub x1: u32,
    /// Bottom (exclusive).
    pub y1: u32,
    /// Mean detector probability inside.
    pub score: f32,
}

/// Detector + recogniser.
pub struct RapidOcr {
    det: OnnxSession,
    rec: OnnxSession,
    chars: Vec<String>,
    cfg: OcrConfig,
    dir: PathBuf,
}

impl std::fmt::Debug for RapidOcr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RapidOcr")
            .field("dir", &self.dir)
            .field("chars", &self.chars.len())
            .finish()
    }
}

const DET_MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const DET_STD: [f32; 3] = [0.229, 0.224, 0.225];

impl RapidOcr {
    /// Load from a model directory.
    pub fn load(
        dir: &Path,
        device: Device,
        threads: usize,
        cfg: OcrConfig,
    ) -> Result<Self, PerceiveError> {
        let det = OnnxSession::load(&dir.join(&cfg.det_model), device, threads, MODEL_HINT)?;
        let rec = OnnxSession::load(&dir.join(&cfg.rec_model), device, threads, MODEL_HINT)?;
        let dict_path = dir.join(&cfg.dict);
        if !dict_path.is_file() {
            return Err(PerceiveError::ModelMissing {
                path: dict_path.display().to_string(),
                hint: MODEL_HINT.to_string(),
            });
        }
        let text = std::fs::read_to_string(&dict_path)?;
        // CTC classes: blank, the dictionary, then space.
        let mut chars = vec![String::new()];
        chars.extend(text.lines().map(|l| l.trim_end_matches('\r').to_string()));
        chars.push(" ".to_string());
        Ok(Self {
            det,
            rec,
            chars,
            cfg,
            dir: dir.to_path_buf(),
        })
    }

    /// Model directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Number of recogniser classes (dictionary + blank + space).
    pub fn classes(&self) -> usize {
        self.chars.len()
    }

    /// Detect text regions in a packed RGB image.
    pub fn detect(
        &self,
        rgb: &[u8],
        width: u32,
        height: u32,
        stride: usize,
    ) -> Result<Vec<TextBox>, PerceiveError> {
        if width == 0 || height == 0 {
            return Ok(Vec::new());
        }
        // Resize so the longest side is at most det_max_side and both sides
        // are multiples of 32.
        let scale = (f64::from(self.cfg.det_max_side) / f64::from(width.max(height))).min(1.0);
        let rw = (((f64::from(width) * scale) / 32.0).round().max(1.0) * 32.0) as u32;
        let rh = (((f64::from(height) * scale) / 32.0).round().max(1.0) * 32.0) as u32;
        let chw = resize_normalize(rgb, width, height, stride, rw, rh, &DET_MEAN, &DET_STD);
        let input = tensor_f32(&[1, 3, rh as usize, rw as usize], chw)?;
        let name = self
            .det
            .input_names()
            .first()
            .cloned()
            .unwrap_or_else(|| "x".into());
        let out_name = self.det.output_names().first().cloned().unwrap_or_default();
        let (shape, prob) = self.det.run(ort::inputs![name.as_str() => input], |out| {
            extract_f32(out, &out_name)
        })?;
        if shape.len() != 4 {
            return Err(PerceiveError::Onnx(format!(
                "detector output shape {shape:?}"
            )));
        }
        let (ph, pw) = (shape[2], shape[3]);
        let boxes = boxes_from_prob_map(&prob, pw, ph, &self.cfg);
        // Map back to source coordinates.
        let sx = f64::from(width) / pw as f64;
        let sy = f64::from(height) / ph as f64;
        Ok(boxes
            .into_iter()
            .map(|b| TextBox {
                x0: ((f64::from(b.x0) * sx).floor() as u32).min(width - 1),
                y0: ((f64::from(b.y0) * sy).floor() as u32).min(height - 1),
                x1: ((f64::from(b.x1) * sx).ceil() as u32).clamp(1, width),
                y1: ((f64::from(b.y1) * sy).ceil() as u32).clamp(1, height),
                score: b.score,
            })
            .filter(|b| b.x1 > b.x0 + 2 && b.y1 > b.y0 + 2)
            .collect())
    }

    /// Recognise text in boxes of a packed RGB image.
    pub fn recognize(
        &self,
        rgb: &[u8],
        width: u32,
        height: u32,
        stride: usize,
        boxes: &[TextBox],
    ) -> Result<Vec<(String, f32)>, PerceiveError> {
        if boxes
            .iter()
            .any(|b| b.x1 > width || b.y1 > height || b.x1 <= b.x0 || b.y1 <= b.y0)
        {
            return Err(PerceiveError::Invalid("text box outside the image".into()));
        }
        let h = self.cfg.rec_height as usize;
        let mut results: Vec<(String, f32)> = vec![(String::new(), 0.0); boxes.len()];
        // Sort by aspect ratio so batches pad little.
        let mut order: Vec<usize> = (0..boxes.len()).collect();
        let ratio = |b: &TextBox| f64::from(b.x1 - b.x0) / f64::from((b.y1 - b.y0).max(1));
        order.sort_by(|a, b| ratio(&boxes[*a]).total_cmp(&ratio(&boxes[*b])));
        for chunk in order.chunks(8) {
            let widths: Vec<usize> = chunk
                .iter()
                .map(|i| ((ratio(&boxes[*i]) * h as f64).ceil() as usize).clamp(16, 1600))
                .collect();
            let w = *widths.iter().max().unwrap_or(&16);
            let mut flat = vec![0f32; chunk.len() * 3 * h * w];
            for (k, i) in chunk.iter().enumerate() {
                let b = &boxes[*i];
                let crop = crop_rgb(rgb, stride, b);
                let cw = (b.x1 - b.x0) as usize;
                let ch = (b.y1 - b.y0) as usize;
                let resized = resize_normalize(
                    &crop,
                    cw as u32,
                    ch as u32,
                    cw * 3,
                    widths[k] as u32,
                    h as u32,
                    &[0.5; 3],
                    &[0.5; 3],
                );
                // Place into the padded batch tensor (pad right with zeros).
                for c in 0..3 {
                    for y in 0..h {
                        let src = &resized[c * h * widths[k] + y * widths[k]
                            ..c * h * widths[k] + (y + 1) * widths[k]];
                        let dst_off = k * 3 * h * w + c * h * w + y * w;
                        flat[dst_off..dst_off + widths[k]].copy_from_slice(src);
                    }
                }
            }
            let input = tensor_f32(&[chunk.len(), 3, h, w], flat)?;
            let name = self
                .rec
                .input_names()
                .first()
                .cloned()
                .unwrap_or_else(|| "x".into());
            let out_name = self.rec.output_names().first().cloned().unwrap_or_default();
            let (shape, logits) = self.rec.run(ort::inputs![name.as_str() => input], |out| {
                extract_f32(out, &out_name)
            })?;
            if shape.len() != 3 || shape[0] != chunk.len() || shape[2] != self.chars.len() {
                return Err(PerceiveError::Onnx(format!(
                    "recogniser output shape {shape:?}, expected [{}, T, {}]",
                    chunk.len(),
                    self.chars.len()
                )));
            }
            let (t, c) = (shape[1], shape[2]);
            for (k, i) in chunk.iter().enumerate() {
                let steps = &logits[k * t * c..(k + 1) * t * c];
                results[*i] = ctc_decode(steps, t, c, &self.chars);
            }
        }
        Ok(results)
    }

    /// Detect and recognise; lines in reading order (top to bottom, then
    /// left to right), with normalised boxes.
    pub fn read(
        &self,
        rgb: &[u8],
        width: u32,
        height: u32,
        stride: usize,
    ) -> Result<Vec<OcrLine>, PerceiveError> {
        let mut boxes = self.detect(rgb, width, height, stride)?;
        if boxes.is_empty() {
            return Ok(Vec::new());
        }
        // Reading order: sort by centre y (a total order), then group into
        // rows greedily (a box joins the current row when its centre is
        // within half the row's box height) and sort each row by x. A
        // tolerance inside a comparator is not a total order and the
        // standard sort panics on one.
        boxes.sort_by_key(|b| b.y0 + b.y1);
        let mut rows: Vec<Vec<TextBox>> = Vec::new();
        for b in boxes.drain(..) {
            let cy = f64::from(b.y0 + b.y1) / 2.0;
            let joins = rows.last().is_some_and(|row| {
                let r = &row[0];
                let rcy = f64::from(r.y0 + r.y1) / 2.0;
                (cy - rcy).abs() <= f64::from((r.y1 - r.y0).min(b.y1 - b.y0)) / 2.0
            });
            if joins {
                if let Some(row) = rows.last_mut() {
                    row.push(b);
                }
            } else {
                rows.push(vec![b]);
            }
        }
        for row in &mut rows {
            row.sort_by_key(|b| b.x0);
        }
        let boxes: Vec<TextBox> = rows.into_iter().flatten().collect();
        let texts = self.recognize(rgb, width, height, stride, &boxes)?;
        let (fw, fh) = (width as f32, height as f32);
        Ok(boxes
            .iter()
            .zip(texts)
            .filter(|(_, (text, conf))| !text.trim().is_empty() && *conf >= self.cfg.min_confidence)
            .map(|(b, (text, conf))| OcrLine {
                text: text.trim().to_string(),
                bbox: [
                    b.x0 as f32 / fw,
                    b.y0 as f32 / fh,
                    (b.x1 - b.x0) as f32 / fw,
                    (b.y1 - b.y0) as f32 / fh,
                ],
                confidence: conf,
            })
            .collect())
    }
}

/// Bilinear resize of packed RGB to `(rw, rh)`, normalised CHW.
#[allow(clippy::too_many_arguments)] // an image, a size, and a normalisation: no struct would read better
fn resize_normalize(
    rgb: &[u8],
    width: u32,
    height: u32,
    stride: usize,
    rw: u32,
    rh: u32,
    mean: &[f32; 3],
    std: &[f32; 3],
) -> Vec<f32> {
    let (rw_us, rh_us) = (rw as usize, rh as usize);
    let mut out = vec![0f32; 3 * rw_us * rh_us];
    let (w, h) = (width as f32, height as f32);
    for y in 0..rh_us {
        let sy = ((y as f32 + 0.5) * h / rh as f32 - 0.5).clamp(0.0, h - 1.0);
        let y0 = sy.floor() as usize;
        let y1 = (y0 + 1).min(height as usize - 1);
        let fy = sy - y0 as f32;
        for x in 0..rw_us {
            let sx = ((x as f32 + 0.5) * w / rw as f32 - 0.5).clamp(0.0, w - 1.0);
            let x0 = sx.floor() as usize;
            let x1 = (x0 + 1).min(width as usize - 1);
            let fx = sx - x0 as f32;
            for c in 0..3 {
                let p = |yy: usize, xx: usize| f32::from(rgb[yy * stride + xx * 3 + c]);
                let top = p(y0, x0) * (1.0 - fx) + p(y0, x1) * fx;
                let bot = p(y1, x0) * (1.0 - fx) + p(y1, x1) * fx;
                let v = (top * (1.0 - fy) + bot * fy) / 255.0;
                out[c * rw_us * rh_us + y * rw_us + x] = (v - mean[c]) / std[c];
            }
        }
    }
    out
}

/// Copy a box out of a packed RGB image (tight stride).
fn crop_rgb(rgb: &[u8], stride: usize, b: &TextBox) -> Vec<u8> {
    let w = (b.x1 - b.x0) as usize;
    let mut out = Vec::with_capacity(w * (b.y1 - b.y0) as usize * 3);
    for y in b.y0..b.y1 {
        let row =
            &rgb[y as usize * stride + b.x0 as usize * 3..y as usize * stride + b.x1 as usize * 3];
        out.extend_from_slice(row);
    }
    out
}

/// DB post-processing: threshold, connected components, box score, unclip.
/// Boxes are in probability-map coordinates.
pub fn boxes_from_prob_map(prob: &[f32], w: usize, h: usize, cfg: &OcrConfig) -> Vec<TextBox> {
    if prob.len() < w * h || w == 0 || h == 0 {
        return Vec::new();
    }
    let mut labels = vec![0u32; w * h];
    let mut boxes = Vec::new();
    let mut next = 1u32;
    let mut stack: Vec<usize> = Vec::new();
    for start in 0..w * h {
        if labels[start] != 0 || prob[start] < cfg.det_threshold {
            continue;
        }
        let label = next;
        next += 1;
        labels[start] = label;
        stack.push(start);
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0usize, 0usize);
        let mut sum = 0f32;
        let mut n = 0usize;
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
            sum += prob[i];
            n += 1;
            let mut visit = |j: usize| {
                if labels[j] == 0 && prob[j] >= cfg.det_threshold {
                    labels[j] = label;
                    stack.push(j);
                }
            };
            if x > 0 {
                visit(i - 1);
            }
            if x + 1 < w {
                visit(i + 1);
            }
            if y > 0 {
                visit(i - w);
            }
            if y + 1 < h {
                visit(i + w);
            }
        }
        let bw = (x1 - x0 + 1) as f32;
        let bh = (y1 - y0 + 1) as f32;
        if bw < 3.0 || bh < 3.0 {
            continue;
        }
        // Score over the bounding box (as PaddleOCR's `box_score_mode=fast`).
        let mut bsum = 0f32;
        for y in y0..=y1 {
            for x in x0..=x1 {
                bsum += prob[y * w + x];
            }
        }
        let score = bsum / (bw * bh);
        let _ = (sum, n);
        if score < cfg.box_threshold {
            continue;
        }
        // Unclip: grow by area * ratio / perimeter, as DB does.
        let d = (bw * bh * cfg.unclip_ratio / (2.0 * (bw + bh))).round() as usize;
        boxes.push(TextBox {
            x0: x0.saturating_sub(d) as u32,
            y0: y0.saturating_sub(d) as u32,
            x1: (x1 + 1 + d).min(w) as u32,
            y1: (y1 + 1 + d).min(h) as u32,
            score,
        });
    }
    boxes
}

/// Greedy CTC decoding over `[t, c]` probabilities with blank at 0.
pub fn ctc_decode(steps: &[f32], t: usize, c: usize, chars: &[String]) -> (String, f32) {
    let mut text = String::new();
    let mut confs = Vec::new();
    let mut prev = 0usize;
    for i in 0..t {
        let row = &steps[i * c..(i + 1) * c];
        let (best, p) =
            row.iter().enumerate().fold(
                (0usize, f32::MIN),
                |acc, (j, v)| if *v > acc.1 { (j, *v) } else { acc },
            );
        if best != 0 && best != prev {
            if let Some(ch) = chars.get(best) {
                text.push_str(ch);
                confs.push(p);
            }
        }
        prev = best;
    }
    let conf = if confs.is_empty() {
        0.0
    } else {
        confs.iter().sum::<f32>() / confs.len() as f32
    };
    (text, conf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctc_collapses_repeats_and_blanks() {
        let chars: Vec<String> = ["", "a", "b", " "].iter().map(|s| s.to_string()).collect();
        // t=6, c=4: a a blank b b blank -> "ab"
        let steps = [
            [0.1, 0.8, 0.05, 0.05],
            [0.1, 0.7, 0.1, 0.1],
            [0.9, 0.05, 0.05, 0.0],
            [0.1, 0.1, 0.7, 0.1],
            [0.1, 0.0, 0.9, 0.0],
            [0.9, 0.0, 0.1, 0.0],
        ]
        .concat();
        let (text, conf) = ctc_decode(&steps, 6, 4, &chars);
        assert_eq!(text, "ab");
        assert!((conf - 0.75).abs() < 1e-6);
    }

    #[test]
    fn prob_map_regions_become_boxes() {
        let (w, h) = (64, 32);
        let mut prob = vec![0.0f32; w * h];
        for y in 5..10 {
            for x in 10..40 {
                prob[y * w + x] = 0.9;
            }
        }
        for y in 20..24 {
            for x in 50..60 {
                prob[y * w + x] = 0.4; // below box threshold
            }
        }
        let boxes = boxes_from_prob_map(&prob, w, h, &OcrConfig::default());
        assert_eq!(boxes.len(), 1, "{boxes:?}");
        let b = boxes[0];
        assert!(b.x0 <= 10 && b.x1 >= 40 && b.y0 <= 5 && b.y1 >= 10, "{b:?}");
        assert!(b.score > 0.85);
    }

    #[test]
    fn model_reads_rendered_text_when_available() {
        let dir = PathBuf::from("/data/videoindex/models").join(MODEL_DIR);
        if !dir.join("en_PP-OCRv3_rec_infer.onnx").is_file() {
            eprintln!("skipping: RapidOCR models not present");
            return;
        }
        let m = RapidOcr::load(&dir, Device::Cpu, 4, OcrConfig::default()).unwrap();
        assert_eq!(m.classes(), 97);
        // Render text with ffmpeg's drawtext into a PNG, decode it, read it.
        let png = std::env::temp_dir().join(format!("vi-ocr-test-{}.png", std::process::id()));
        let status = std::process::Command::new("ffmpeg")
            .args(["-y", "-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i",
                "color=c=white:size=640x360:rate=1:duration=1",
                "-vf", "drawtext=fontfile=/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf:text='Hybrid Retrieval':fontsize=56:fontcolor=black:x=60:y=80,drawtext=fontfile=/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf:text='BM25 and dense vectors':fontsize=36:fontcolor=black:x=60:y=200",
                "-frames:v", "1"])
            .arg(&png)
            .status();
        if !matches!(status, Ok(s) if s.success()) {
            eprintln!("skipping: ffmpeg drawtext unavailable");
            return;
        }
        let img = image::open(&png).unwrap().to_rgb8();
        let _ = std::fs::remove_file(&png);
        let (w, h) = img.dimensions();
        let lines = m.read(img.as_raw(), w, h, w as usize * 3).unwrap();
        let joined = lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(joined.to_lowercase().contains("hybrid"), "{joined}");
        assert!(joined.to_lowercase().contains("vectors"), "{joined}");
        assert!(
            lines[0].bbox[1] < lines[lines.len() - 1].bbox[1],
            "reading order"
        );
        assert!(lines.iter().all(|l| l.confidence > 0.5));
    }
}
