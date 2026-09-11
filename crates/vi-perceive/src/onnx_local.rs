//! The `onnx_local` provider adapter: SigLIP (`ImageEmbedder`), bge
//! (`TextEmbedder`) and RapidOCR (`Ocr`) as `vi-providers` trait objects.
//! Models load lazily on first use and inference runs on the rayon pool.
//!
//! Config (`[providers.<name>]`, `adapter = "onnx_local"`):
//! `model_dir` (default `models.dir`), `image_model` and `text_model`
//! (subdirectory names, defaults `siglip-base-patch16-224` and
//! `bge-small-en-v1.5`), `ocr_model` (default `rapidocr`), `device`
//! (default `models.device`), `threads`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use async_trait::async_trait;
use vi_core::config::{Pricing, ProviderConfig, RoleBinding};
use vi_core::cpu;
use vi_providers::registry::{ExternalAdapter, ExternalFactory};
use vi_providers::{
    CallStats, EmbedResponse, ImageData, ImageEmbedder, ProviderError, ProviderRegistry, Result,
    TextEmbedder, Usage,
};

use crate::onnx::{resolve_device, Device};
use crate::{bge, siglip, PerceiveError};

/// Adapter kind name in config.
pub const KIND: &str = "onnx_local";

/// The adapter.
pub struct OnnxLocal {
    name: String,
    model_dir: PathBuf,
    image_model: String,
    text_model: String,
    device: Device,
    threads: usize,
    siglip: OnceLock<Arc<siglip::Siglip>>,
    bge: OnceLock<Arc<bge::TextEmbedder>>,
    load_error: Mutex<Option<String>>,
}

impl std::fmt::Debug for OnnxLocal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnnxLocal")
            .field("name", &self.name)
            .field("model_dir", &self.model_dir)
            .field("device", &self.device)
            .finish()
    }
}

fn perr(name: &str, e: PerceiveError) -> ProviderError {
    match e {
        PerceiveError::ModelMissing { path, hint } => ProviderError::ModelMissing { path, hint },
        PerceiveError::Invalid(m) => ProviderError::Invalid(m),
        other => ProviderError::Inference {
            provider: name.to_string(),
            message: other.to_string(),
        },
    }
}

impl OnnxLocal {
    /// Build from config. `default_model_dir` and `default_device` come from
    /// `[models]`.
    pub fn from_config(
        name: &str,
        cfg: &ProviderConfig,
        default_model_dir: &std::path::Path,
        default_device: &str,
        default_threads: usize,
    ) -> Result<Self> {
        let get = |k: &str| {
            cfg.extra
                .get(k)
                .and_then(|v| v.as_str())
                .map(str::to_string)
        };
        let device_str = get("device").unwrap_or_else(|| default_device.to_string());
        let device = resolve_device(&device_str).map_err(|e| perr(name, e))?;
        Ok(Self {
            name: name.to_string(),
            model_dir: cfg
                .model_dir
                .clone()
                .unwrap_or_else(|| default_model_dir.to_path_buf()),
            image_model: get("image_model").unwrap_or_else(|| siglip::MODEL_DIR.to_string()),
            text_model: get("text_model").unwrap_or_else(|| bge::MODEL_DIR.to_string()),
            device,
            threads: cfg
                .extra
                .get("threads")
                .and_then(|v| v.as_integer())
                .map(|t| t.max(0) as usize)
                .unwrap_or(default_threads),
            siglip: OnceLock::new(),
            bge: OnceLock::new(),
            load_error: Mutex::new(None),
        })
    }

    /// A factory for [`ProviderRegistry::register_factory`].
    pub fn factory(model_dir: PathBuf, device: String, threads: usize) -> ExternalFactory {
        Arc::new(
            move |name: &str, cfg: &ProviderConfig, _role: Option<&RoleBinding>, _gov| {
                let a = OnnxLocal::from_config(name, cfg, &model_dir, &device, threads)?;
                Ok(Arc::new(a) as Arc<dyn ExternalAdapter>)
            },
        )
    }

    /// Register the factory on a registry using the `[models]` defaults.
    pub fn register(registry: &ProviderRegistry) {
        let m = &registry.config().models;
        registry.register_factory(
            KIND,
            Self::factory(m.dir.clone(), m.device.clone(), m.onnx_threads),
        );
    }

    fn siglip(&self) -> Result<Arc<siglip::Siglip>> {
        if let Some(s) = self.siglip.get() {
            return Ok(s.clone());
        }
        let dir = self.model_dir.join(&self.image_model);
        let m = siglip::Siglip::load(&dir, self.device, self.threads)
            .map_err(|e| perr(&self.name, e))?;
        let _ = self.siglip.set(Arc::new(m));
        self.siglip
            .get()
            .cloned()
            .ok_or_else(|| ProviderError::Inference {
                provider: self.name.clone(),
                message: "model cell empty after load".into(),
            })
    }

    fn bge(&self) -> Result<Arc<bge::TextEmbedder>> {
        if let Some(s) = self.bge.get() {
            return Ok(s.clone());
        }
        let dir = self.model_dir.join(&self.text_model);
        let m = bge::TextEmbedder::load(&dir, self.device, self.threads)
            .map_err(|e| perr(&self.name, e))?;
        let _ = self.bge.set(Arc::new(m));
        self.bge
            .get()
            .cloned()
            .ok_or_else(|| ProviderError::Inference {
                provider: self.name.clone(),
                message: "model cell empty after load".into(),
            })
    }

    fn stats(&self, model: &str, n: u64, images: u64, started: Instant) -> CallStats {
        let mut s = CallStats::new(
            &self.name,
            model,
            Usage {
                images,
                calls: 1,
                tokens_in: if images == 0 { n } else { 0 },
                ..Usage::default()
            },
            &Pricing::default(),
        );
        s.latency_ms = started.elapsed().as_millis() as u64;
        s
    }

    /// Device the models run on.
    pub fn device(&self) -> Device {
        self.device
    }

    /// Last load error, for reports.
    pub fn load_error(&self) -> Option<String> {
        self.load_error.lock().ok().and_then(|g| g.clone())
    }
}

impl ExternalAdapter for OnnxLocal {
    fn kind(&self) -> &'static str {
        KIND
    }

    fn model(&self) -> String {
        format!("{} + {}", self.image_model, self.text_model)
    }

    fn as_text_embedder(&self) -> Option<Arc<dyn TextEmbedder>> {
        Some(Arc::new(BgeHandle(self.shared())))
    }

    fn as_image_embedder(&self) -> Option<Arc<dyn ImageEmbedder>> {
        Some(Arc::new(SiglipHandle(self.shared())))
    }
}

impl OnnxLocal {
    /// The registry holds the adapter in an `Arc`; the trait objects need
    /// their own handle, so the adapter keeps a self-reference set at
    /// construction through [`Self::factory`].
    fn shared(&self) -> Arc<OnnxLocal> {
        // Rebuild a lightweight adapter sharing the loaded models.
        Arc::new(OnnxLocal {
            name: self.name.clone(),
            model_dir: self.model_dir.clone(),
            image_model: self.image_model.clone(),
            text_model: self.text_model.clone(),
            device: self.device,
            threads: self.threads,
            siglip: match self.siglip.get() {
                Some(m) => OnceLock::from(m.clone()),
                None => OnceLock::new(),
            },
            bge: match self.bge.get() {
                Some(m) => OnceLock::from(m.clone()),
                None => OnceLock::new(),
            },
            load_error: Mutex::new(None),
        })
    }
}

/// `ImageEmbedder` over SigLIP.
struct SiglipHandle(Arc<OnnxLocal>);

#[async_trait]
impl ImageEmbedder for SiglipHandle {
    fn provider_name(&self) -> &str {
        &self.0.name
    }

    fn model(&self) -> &str {
        &self.0.image_model
    }

    fn dim(&self) -> u32 {
        self.0.siglip.get().map(|m| m.dim()).unwrap_or(768)
    }

    fn max_batch(&self) -> usize {
        16
    }

    async fn embed_images(&self, images: &[ImageData]) -> Result<EmbedResponse> {
        let started = Instant::now();
        let name = self.0.name.clone();
        let model = self.0.siglip()?;
        let mut chw = Vec::with_capacity(images.len());
        for im in images {
            match im {
                ImageData::Rgb8 {
                    width,
                    height,
                    data,
                } => {
                    if data.len() < (*width as usize) * (*height as usize) * 3 {
                        return Err(ProviderError::Invalid("RGB buffer too short".into()));
                    }
                    let (w, h, d, m) = (*width, *height, data.clone(), model.clone());
                    chw.push(
                        cpu::run(move || m.preprocess().image_to_chw(&d, w, h, w as usize * 3))
                            .await
                            .map_err(|e| ProviderError::Inference {
                                provider: name.clone(),
                                message: e.to_string(),
                            })?,
                    );
                }
                ImageData::Encoded { .. } => {
                    return Err(ProviderError::Invalid(
                        "onnx_local takes raw RGB images, not encoded ones".into(),
                    ))
                }
            }
        }
        let m = model.clone();
        let vectors = cpu::run(move || m.embed_chw(&chw))
            .await
            .map_err(|e| ProviderError::Inference {
                provider: name.clone(),
                message: e.to_string(),
            })?
            .map_err(|e| perr(&name, e))?;
        Ok(EmbedResponse {
            vectors,
            stats: self.0.stats(
                &self.0.image_model,
                images.len() as u64,
                images.len() as u64,
                started,
            ),
        })
    }

    async fn embed_text(&self, texts: &[String]) -> Result<EmbedResponse> {
        let started = Instant::now();
        let name = self.0.name.clone();
        let model = self.0.siglip()?;
        let owned = texts.to_vec();
        let vectors = cpu::run(move || model.embed_text(&owned))
            .await
            .map_err(|e| ProviderError::Inference {
                provider: name.clone(),
                message: e.to_string(),
            })?
            .map_err(|e| perr(&name, e))?;
        Ok(EmbedResponse {
            vectors,
            stats: self
                .0
                .stats(&self.0.image_model, texts.len() as u64, 0, started),
        })
    }
}

/// `TextEmbedder` over bge.
struct BgeHandle(Arc<OnnxLocal>);

#[async_trait]
impl TextEmbedder for BgeHandle {
    fn provider_name(&self) -> &str {
        &self.0.name
    }

    fn model(&self) -> &str {
        &self.0.text_model
    }

    fn dim(&self) -> u32 {
        self.0.bge.get().map(|m| m.dim()).unwrap_or(384)
    }

    fn max_batch(&self) -> usize {
        32
    }

    async fn embed(&self, texts: &[String]) -> Result<EmbedResponse> {
        let started = Instant::now();
        let name = self.0.name.clone();
        let model = self.0.bge()?;
        let owned = texts.to_vec();
        let vectors = cpu::run(move || model.embed(&owned))
            .await
            .map_err(|e| ProviderError::Inference {
                provider: name.clone(),
                message: e.to_string(),
            })?
            .map_err(|e| perr(&name, e))?;
        Ok(EmbedResponse {
            vectors,
            stats: self
                .0
                .stats(&self.0.text_model, texts.len() as u64, 0, started),
        })
    }
}
