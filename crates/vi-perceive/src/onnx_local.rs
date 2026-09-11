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
    CallStats, EmbedResponse, ImageData, ImageEmbedder, Ocr, OcrBox, OcrLine, OcrResponse,
    ProviderError, ProviderRegistry, Result, TextEmbedder, Usage,
};

use crate::onnx::{resolve_device, Device};
use crate::{bge, ocr, siglip, PerceiveError};

/// Adapter kind name in config.
pub const KIND: &str = "onnx_local";

/// The adapter.
pub struct OnnxLocal {
    name: String,
    model_dir: PathBuf,
    image_model: String,
    text_model: String,
    ocr_model: String,
    device: Device,
    threads: usize,
    /// Loaded models, shared by every handle the registry hands out so a
    /// model loads once per process, not once per call.
    siglip: Arc<OnceLock<Arc<siglip::Siglip>>>,
    bge: Arc<OnceLock<Arc<bge::TextEmbedder>>>,
    ocr: Arc<OnceLock<Arc<ocr::RapidOcr>>>,
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
            ocr_model: get("ocr_model").unwrap_or_else(|| ocr::MODEL_DIR.to_string()),
            device,
            threads: cfg
                .extra
                .get("threads")
                .and_then(|v| v.as_integer())
                .map(|t| t.max(0) as usize)
                .unwrap_or(default_threads),
            siglip: Arc::new(OnceLock::new()),
            bge: Arc::new(OnceLock::new()),
            ocr: Arc::new(OnceLock::new()),
            load_error: Mutex::new(None),
        })
    }

    fn ocr(&self) -> Result<Arc<ocr::RapidOcr>> {
        if let Some(s) = self.ocr.get() {
            return Ok(s.clone());
        }
        let dir = self.model_dir.join(&self.ocr_model);
        let m = ocr::RapidOcr::load(&dir, self.device, self.threads, ocr::OcrConfig::default())
            .map_err(|e| perr(&self.name, e))?;
        let _ = self.ocr.set(Arc::new(m));
        self.ocr
            .get()
            .cloned()
            .ok_or_else(|| ProviderError::Inference {
                provider: self.name.clone(),
                message: "model cell empty after load".into(),
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

    fn as_ocr(&self) -> Option<Arc<dyn Ocr>> {
        Some(Arc::new(OcrHandle(self.shared())))
    }
}

impl OnnxLocal {
    /// The registry holds the adapter behind `ExternalAdapter`; trait
    /// objects need their own `Arc`, so hand out a copy that shares the
    /// same lazily loaded models (the `OnceLock`s are behind `Arc`s).
    fn shared(&self) -> Arc<OnnxLocal> {
        Arc::new(OnnxLocal {
            name: self.name.clone(),
            model_dir: self.model_dir.clone(),
            image_model: self.image_model.clone(),
            text_model: self.text_model.clone(),
            ocr_model: self.ocr_model.clone(),
            device: self.device,
            threads: self.threads,
            siglip: self.siglip.clone(),
            bge: self.bge.clone(),
            ocr: self.ocr.clone(),
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

/// `Ocr` over RapidOCR.
struct OcrHandle(Arc<OnnxLocal>);

#[async_trait]
impl Ocr for OcrHandle {
    fn provider_name(&self) -> &str {
        &self.0.name
    }

    fn model(&self) -> &str {
        &self.0.ocr_model
    }

    async fn read(&self, image: &ImageData) -> Result<OcrResponse> {
        let started = Instant::now();
        let name = self.0.name.clone();
        let model = self.0.ocr()?;
        let ImageData::Rgb8 {
            width,
            height,
            data,
        } = image
        else {
            return Err(ProviderError::Invalid(
                "onnx_local takes raw RGB images, not encoded ones".into(),
            ));
        };
        if data.len() < (*width as usize) * (*height as usize) * 3 {
            return Err(ProviderError::Invalid("RGB buffer too short".into()));
        }
        let (w, h, d) = (*width, *height, data.clone());
        let lines = cpu::run(move || model.read(&d, w, h, w as usize * 3))
            .await
            .map_err(|e| ProviderError::Inference {
                provider: name.clone(),
                message: e.to_string(),
            })?
            .map_err(|e| perr(&name, e))?;
        Ok(OcrResponse {
            lines: lines
                .into_iter()
                .map(|l| OcrLine {
                    text: l.text,
                    bbox: Some(OcrBox {
                        x: l.bbox[0],
                        y: l.bbox[1],
                        w: l.bbox[2],
                        h: l.bbox[3],
                    }),
                    confidence: Some(l.confidence),
                })
                .collect(),
            stats: self.0.stats(&self.0.ocr_model, 1, 1, started),
        })
    }
}
