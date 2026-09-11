//! Small text embedder (`bge-small-en-v1.5` and other BERT-style models
//! exported to ONNX): CLS pooling, L2-normalised.

use std::path::{Path, PathBuf};

use tokenizers::Tokenizer;

use crate::onnx::{extract_f32, tensor_i64, Device, OnnxSession};
use crate::siglip::split_rows;
use crate::PerceiveError;

/// Default model directory name under `models.dir`.
pub const MODEL_DIR: &str = "bge-small-en-v1.5";
/// Where to get the files.
pub const MODEL_HINT: &str =
    "download onnx/model.onnx and tokenizer.json from huggingface.co/Xenova/bge-small-en-v1.5";
/// Longest input in tokens.
pub const MAX_LEN: usize = 512;
/// Prefix bge recommends for short queries against passages.
pub const QUERY_INSTRUCTION: &str = "Represent this sentence for searching relevant passages: ";

/// A BERT-style text embedder.
pub struct TextEmbedder {
    session: OnnxSession,
    tokenizer: Tokenizer,
    dim: u32,
    dir: PathBuf,
    needs_token_types: bool,
}

impl std::fmt::Debug for TextEmbedder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextEmbedder")
            .field("dir", &self.dir)
            .field("dim", &self.dim)
            .finish()
    }
}

impl TextEmbedder {
    /// Load from a model directory.
    pub fn load(dir: &Path, device: Device, threads: usize) -> Result<Self, PerceiveError> {
        let session = OnnxSession::load(&dir.join("model.onnx"), device, threads, MODEL_HINT)?;
        let tok_path = dir.join("tokenizer.json");
        if !tok_path.is_file() {
            return Err(PerceiveError::ModelMissing {
                path: tok_path.display().to_string(),
                hint: MODEL_HINT.to_string(),
            });
        }
        let tokenizer = Tokenizer::from_file(&tok_path)
            .map_err(|e| PerceiveError::Onnx(format!("tokenizer: {e}")))?;
        let dim = std::fs::read_to_string(dir.join("config.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|v| v["hidden_size"].as_u64())
            .unwrap_or(384) as u32;
        let needs_token_types = session.input_names().iter().any(|n| n == "token_type_ids");
        Ok(Self {
            session,
            tokenizer,
            dim,
            dir: dir.to_path_buf(),
            needs_token_types,
        })
    }

    /// Embedding size.
    pub fn dim(&self) -> u32 {
        self.dim
    }

    /// Device in use.
    pub fn device(&self) -> Device {
        self.session.device()
    }

    /// Embed a batch. Inputs are padded to the longest in the batch.
    pub fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, PerceiveError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let encs = self
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| PerceiveError::Onnx(format!("tokenizer: {e}")))?;
        let len = encs
            .iter()
            .map(|e| e.get_ids().len())
            .max()
            .unwrap_or(1)
            .clamp(1, MAX_LEN);
        let n = texts.len();
        let mut ids = vec![0i64; n * len];
        let mut mask = vec![0i64; n * len];
        for (i, e) in encs.iter().enumerate() {
            for (j, id) in e.get_ids().iter().take(len).enumerate() {
                ids[i * len + j] = i64::from(*id);
                mask[i * len + j] = 1;
            }
        }
        let ids_t = tensor_i64(&[n, len], ids)?;
        let mask_t = tensor_i64(&[n, len], mask)?;
        let dim = self.dim as usize;
        let run = |out: &ort::session::SessionOutputs<'_>| -> Result<Vec<Vec<f32>>, PerceiveError> {
            let (shape, data) = extract_f32(out, "last_hidden_state")?;
            if shape.len() != 3 || shape[0] != n || shape[2] != dim {
                return Err(PerceiveError::Onnx(format!(
                    "unexpected hidden state shape {shape:?}"
                )));
            }
            let seq = shape[1];
            // CLS pooling: token 0 of every row.
            let mut rows = Vec::with_capacity(n * dim);
            for i in 0..n {
                rows.extend_from_slice(&data[i * seq * dim..i * seq * dim + dim]);
            }
            split_rows(vec![n, dim], rows, n, dim)
        };
        if self.needs_token_types {
            let types_t = tensor_i64(&[n, len], vec![0i64; n * len])?;
            self.session.run(
                ort::inputs!["input_ids" => ids_t, "attention_mask" => mask_t, "token_type_ids" => types_t],
                run,
            )
        } else {
            self.session.run(
                ort::inputs!["input_ids" => ids_t, "attention_mask" => mask_t],
                run,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::siglip::cosine;

    #[test]
    fn model_embeds_and_ranks_when_available() {
        let dir = PathBuf::from("/data/videoindex/models").join(MODEL_DIR);
        if !dir.join("model.onnx").is_file() {
            eprintln!("skipping: bge model not present");
            return;
        }
        let m = TextEmbedder::load(&dir, Device::Cpu, 4).unwrap();
        assert_eq!(m.dim(), 384);
        let v = m
            .embed(&[
                format!("{QUERY_INSTRUCTION}how do I evaluate a language model"),
                "We ran the evaluation harness over three LLM benchmarks.".into(),
                "The pitch deck should open with the team slide.".into(),
            ])
            .unwrap();
        assert_eq!(v.len(), 3);
        assert!((v[0].iter().map(|x| x * x).sum::<f32>() - 1.0).abs() < 1e-3);
        assert!(cosine(&v[0], &v[1]) > cosine(&v[0], &v[2]));
    }
}
