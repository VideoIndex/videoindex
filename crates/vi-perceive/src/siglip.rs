//! SigLIP image-text embeddings through ONNX Runtime. Images and text land
//! in one 768-dimensional space, so a text query can rank frames directly.
//!
//! Model directory layout (`Xenova/siglip-base-patch16-224` export):
//! `vision_model.onnx`, `text_model.onnx`, `tokenizer.json`,
//! `preprocessor_config.json` (optional; 224 px and mean/std 0.5 assumed).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tokenizers::Tokenizer;

use crate::onnx::{extract_f32, tensor_f32, tensor_i64, Device, OnnxSession};
use crate::PerceiveError;

/// Default model directory name under `models.dir`.
pub const MODEL_DIR: &str = "siglip-base-patch16-224";
/// Where to get the files.
pub const MODEL_HINT: &str =
    "download onnx/vision_model.onnx, onnx/text_model.onnx and tokenizer.json from huggingface.co/Xenova/siglip-base-patch16-224";
/// Text sequence length SigLIP was trained with.
pub const TEXT_LEN: usize = 64;
/// Pad token id (`</s>`).
const PAD_ID: i64 = 1;

/// Image preprocessing parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Preprocess {
    /// Side of the square input.
    pub size: u32,
    /// Per-channel mean (0-1 scale).
    pub mean: [f32; 3],
    /// Per-channel std.
    pub std: [f32; 3],
}

impl Default for Preprocess {
    fn default() -> Self {
        Self {
            size: 224,
            mean: [0.5, 0.5, 0.5],
            std: [0.5, 0.5, 0.5],
        }
    }
}

impl Preprocess {
    /// Read `preprocessor_config.json` if present.
    pub fn from_dir(dir: &Path) -> Self {
        let mut p = Self::default();
        let Ok(text) = std::fs::read_to_string(dir.join("preprocessor_config.json")) else {
            return p;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
            return p;
        };
        if let Some(h) = v["size"]["height"].as_u64() {
            p.size = h as u32;
        }
        for (i, m) in v["image_mean"]
            .as_array()
            .into_iter()
            .flatten()
            .take(3)
            .enumerate()
        {
            if let Some(f) = m.as_f64() {
                p.mean[i] = f as f32;
            }
        }
        for (i, m) in v["image_std"]
            .as_array()
            .into_iter()
            .flatten()
            .take(3)
            .enumerate()
        {
            if let Some(f) = m.as_f64() {
                p.std[i] = f as f32;
            }
        }
        p
    }

    /// Resize (bilinear, no aspect preservation, as the HF processor does)
    /// and normalise packed RGB into CHW `f32`.
    pub fn image_to_chw(&self, rgb: &[u8], width: u32, height: u32, stride: usize) -> Vec<f32> {
        let s = self.size as usize;
        let mut out = vec![0f32; 3 * s * s];
        let (w, h) = (width as f32, height as f32);
        for y in 0..s {
            // Sample position in source coordinates (pixel centres).
            let sy = ((y as f32 + 0.5) * h / s as f32 - 0.5).clamp(0.0, h - 1.0);
            let y0 = sy.floor() as usize;
            let y1 = (y0 + 1).min(height as usize - 1);
            let fy = sy - y0 as f32;
            for x in 0..s {
                let sx = ((x as f32 + 0.5) * w / s as f32 - 0.5).clamp(0.0, w - 1.0);
                let x0 = sx.floor() as usize;
                let x1 = (x0 + 1).min(width as usize - 1);
                let fx = sx - x0 as f32;
                for c in 0..3 {
                    let p = |yy: usize, xx: usize| f32::from(rgb[yy * stride + xx * 3 + c]);
                    let top = p(y0, x0) * (1.0 - fx) + p(y0, x1) * fx;
                    let bot = p(y1, x0) * (1.0 - fx) + p(y1, x1) * fx;
                    let v = (top * (1.0 - fy) + bot * fy) / 255.0;
                    out[c * s * s + y * s + x] = (v - self.mean[c]) / self.std[c];
                }
            }
        }
        out
    }
}

/// L2-normalise in place.
pub fn normalize(v: &mut [f32]) {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        for x in v.iter_mut() {
            *x /= n;
        }
    }
}

/// The two towers. Each loads on first use: a query needs only the text
/// tower (441 MB), indexing only the vision tower (372 MB).
pub struct Siglip {
    vision: OnceLock<OnnxSession>,
    text: OnceLock<OnnxSession>,
    tokenizer: Tokenizer,
    pre: Preprocess,
    dim: u32,
    dir: PathBuf,
    device: Device,
    threads: usize,
}

impl std::fmt::Debug for Siglip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Siglip")
            .field("dir", &self.dir)
            .field("dim", &self.dim)
            .field("device", &self.device)
            .finish()
    }
}

impl Siglip {
    /// Load from `<models_dir>/<MODEL_DIR>` (or an explicit directory).
    pub fn load(dir: &Path, device: Device, threads: usize) -> Result<Self, PerceiveError> {
        let tok_path = dir.join("tokenizer.json");
        if !tok_path.is_file() {
            return Err(PerceiveError::ModelMissing {
                path: tok_path.display().to_string(),
                hint: MODEL_HINT.to_string(),
            });
        }
        let tokenizer = Tokenizer::from_file(&tok_path)
            .map_err(|e| PerceiveError::Onnx(format!("tokenizer: {e}")))?;
        let pre = Preprocess::from_dir(dir);
        let dim = std::fs::read_to_string(dir.join("config.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|v| v["text_config"]["hidden_size"].as_u64())
            .unwrap_or(768) as u32;
        Ok(Self {
            vision: OnceLock::new(),
            text: OnceLock::new(),
            tokenizer,
            pre,
            dim,
            dir: dir.to_path_buf(),
            device,
            threads,
        })
    }

    fn tower<'a>(
        &'a self,
        cell: &'a OnceLock<OnnxSession>,
        file: &str,
    ) -> Result<&'a OnnxSession, PerceiveError> {
        if let Some(s) = cell.get() {
            return Ok(s);
        }
        let s = OnnxSession::load(&self.dir.join(file), self.device, self.threads, MODEL_HINT)?;
        let _ = cell.set(s);
        cell.get()
            .ok_or_else(|| PerceiveError::Onnx("tower cell empty after load".into()))
    }

    /// Embedding size.
    pub fn dim(&self) -> u32 {
        self.dim
    }

    /// Preprocessing parameters.
    pub fn preprocess(&self) -> &Preprocess {
        &self.pre
    }

    /// Device in use.
    pub fn device(&self) -> Device {
        self.device
    }

    /// Embed preprocessed CHW images (each `3 * size * size` floats).
    pub fn embed_chw(&self, images: &[Vec<f32>]) -> Result<Vec<Vec<f32>>, PerceiveError> {
        if images.is_empty() {
            return Ok(Vec::new());
        }
        let s = self.pre.size as usize;
        let per = 3 * s * s;
        let mut flat = Vec::with_capacity(images.len() * per);
        for im in images {
            if im.len() != per {
                return Err(PerceiveError::Invalid(format!(
                    "preprocessed image has {} values, expected {per}",
                    im.len()
                )));
            }
            flat.extend_from_slice(im);
        }
        let input = tensor_f32(&[images.len(), 3, s, s], flat)?;
        let (shape, data) = self
            .tower(&self.vision, "vision_model.onnx")?
            .run(ort::inputs!["pixel_values" => input], |out| {
                extract_f32(out, "pooler_output")
            })?;
        split_rows(shape, data, images.len(), self.dim as usize)
    }

    /// Embed RGB images given as `(rgb, width, height, stride)`.
    pub fn embed_images(
        &self,
        images: &[(&[u8], u32, u32, usize)],
    ) -> Result<Vec<Vec<f32>>, PerceiveError> {
        let chw: Vec<Vec<f32>> = images
            .iter()
            .map(|(d, w, h, st)| self.pre.image_to_chw(d, *w, *h, *st))
            .collect();
        self.embed_chw(&chw)
    }

    /// Token ids padded to [`TEXT_LEN`].
    pub fn tokenize(&self, text: &str) -> Result<Vec<i64>, PerceiveError> {
        let enc = self
            .tokenizer
            .encode(text.to_lowercase(), false)
            .map_err(|e| PerceiveError::Onnx(format!("tokenizer: {e}")))?;
        let mut ids: Vec<i64> = enc.get_ids().iter().map(|i| i64::from(*i)).collect();
        // SigLIP appends the eos token and pads to a fixed length.
        ids.truncate(TEXT_LEN - 1);
        ids.push(PAD_ID);
        ids.resize(TEXT_LEN, PAD_ID);
        Ok(ids)
    }

    /// Embed texts with the text tower.
    pub fn embed_text(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, PerceiveError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let mut flat = Vec::with_capacity(texts.len() * TEXT_LEN);
        for t in texts {
            flat.extend(self.tokenize(t)?);
        }
        let input = tensor_i64(&[texts.len(), TEXT_LEN], flat)?;
        let (shape, data) = self
            .tower(&self.text, "text_model.onnx")?
            .run(ort::inputs!["input_ids" => input], |out| {
                extract_f32(out, "pooler_output")
            })?;
        split_rows(shape, data, texts.len(), self.dim as usize)
    }
}

/// Split a `[n, dim]` output into normalised rows.
pub(crate) fn split_rows(
    shape: Vec<usize>,
    data: Vec<f32>,
    n: usize,
    dim: usize,
) -> Result<Vec<Vec<f32>>, PerceiveError> {
    if shape.len() != 2 || shape[0] != n || shape[1] != dim || data.len() != n * dim {
        return Err(PerceiveError::Onnx(format!(
            "unexpected embedding output shape {shape:?} for {n} inputs of {dim} dims"
        )));
    }
    Ok(data
        .chunks_exact(dim)
        .map(|c| {
            let mut v = c.to_vec();
            normalize(&mut v);
            v
        })
        .collect())
}

/// Dot product of two normalised vectors.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preprocess_shapes_and_range() {
        let pre = Preprocess::default();
        let (w, h) = (64u32, 32u32);
        let mut rgb = vec![0u8; (w * h * 3) as usize];
        for (i, p) in rgb.chunks_exact_mut(3).enumerate() {
            let x = (i as u32 % w) as u8;
            p.copy_from_slice(&[x * 4, 255 - x * 4, 128]);
        }
        let chw = pre.image_to_chw(&rgb, w, h, (w * 3) as usize);
        assert_eq!(chw.len(), 3 * 224 * 224);
        assert!(chw.iter().all(|v| (-1.0..=1.0).contains(v)));
        // Red rises left to right in the first channel.
        assert!(chw[0] < chw[223]);
        // Blue 128 maps to about 0.
        assert!(chw[2 * 224 * 224].abs() < 0.02);
    }

    #[test]
    fn model_embeds_and_ranks_when_available() {
        let dir = PathBuf::from("/data/videoindex/models").join(MODEL_DIR);
        if !dir.join("vision_model.onnx").is_file() {
            eprintln!("skipping: SigLIP model not present");
            return;
        }
        let m = Siglip::load(&dir, Device::Cpu, 4).unwrap();
        assert_eq!(m.dim(), 768);
        let ids = m.tokenize("a photo of a cat").unwrap();
        assert_eq!(ids.len(), TEXT_LEN);
        assert_eq!(ids[TEXT_LEN - 1], PAD_ID);
        // A red image and a blue image.
        let (w, h) = (96u32, 96u32);
        let red: Vec<u8> = (0..w * h).flat_map(|_| [220u8, 30, 30]).collect();
        let blue: Vec<u8> = (0..w * h).flat_map(|_| [30u8, 30, 220]).collect();
        let st = (w * 3) as usize;
        let im = m
            .embed_images(&[(&red, w, h, st), (&blue, w, h, st)])
            .unwrap();
        assert_eq!(im.len(), 2);
        assert!((im[0].iter().map(|x| x * x).sum::<f32>() - 1.0).abs() < 1e-3);
        let tx = m
            .embed_text(&["a red image".into(), "a blue image".into()])
            .unwrap();
        // Each text prefers its own colour.
        assert!(cosine(&im[0], &tx[0]) > cosine(&im[0], &tx[1]), "red");
        assert!(cosine(&im[1], &tx[1]) > cosine(&im[1], &tx[0]), "blue");
    }
}
