//! Role binding: `[roles] asr = { provider = "whisper" }` plus
//! `[providers.whisper]` become an `Arc<dyn Asr>`. Adapters are built lazily
//! and shared; every provider name has one [`Governor`], so a provider used
//! under two roles still respects one concurrency and rate limit.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use tokio_util::sync::CancellationToken;
use vi_core::config::{roles, Config, ProviderConfig, RoleBinding};

use crate::adapters::{Anthropic, Gemini, OpenAiCompat};
use crate::error::{ProviderError, Result};
use crate::governor::Governor;
use crate::traits::*;

/// Adapter families this build knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterKind {
    /// OpenAI HTTP shape (vLLM, Ollama, Whisper servers, OpenAI).
    OpenAiCompat,
    /// In-process ONNX Runtime models.
    OnnxLocal,
    /// Google Gemini API (M2).
    Gemini,
    /// Anthropic API (M2).
    Anthropic,
}

impl AdapterKind {
    /// Parse the config `adapter` string.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "openai_compat" => Some(Self::OpenAiCompat),
            "onnx_local" => Some(Self::OnnxLocal),
            "gemini" => Some(Self::Gemini),
            "anthropic" => Some(Self::Anthropic),
            _ => None,
        }
    }

    /// Config name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenAiCompat => "openai_compat",
            Self::OnnxLocal => "onnx_local",
            Self::Gemini => "gemini",
            Self::Anthropic => "anthropic",
        }
    }
}

/// A constructed adapter, whichever traits it implements.
enum Adapter {
    OpenAiCompat(Arc<OpenAiCompat>),
    Anthropic(Arc<Anthropic>),
    Gemini(Arc<Gemini>),
    /// Registered by the caller (the `onnx_local` adapter lives in
    /// `vi-perceive`, which this crate does not depend on).
    External(Arc<dyn ExternalAdapter>),
}

/// An adapter supplied from outside this crate. Implementors return the
/// trait objects they support.
pub trait ExternalAdapter: Send + Sync {
    /// Adapter kind name, for reports.
    fn kind(&self) -> &'static str;
    /// Model name, for reports.
    fn model(&self) -> String;
    /// ASR, if implemented.
    fn as_asr(&self) -> Option<Arc<dyn Asr>> {
        None
    }
    /// OCR, if implemented.
    fn as_ocr(&self) -> Option<Arc<dyn Ocr>> {
        None
    }
    /// Text embedder, if implemented.
    fn as_text_embedder(&self) -> Option<Arc<dyn TextEmbedder>> {
        None
    }
    /// Image embedder, if implemented.
    fn as_image_embedder(&self) -> Option<Arc<dyn ImageEmbedder>> {
        None
    }
    /// Reranker, if implemented.
    fn as_reranker(&self) -> Option<Arc<dyn Reranker>> {
        None
    }
}

/// Builds external adapters for a provider table. Registered per adapter
/// kind with [`ProviderRegistry::register_factory`].
pub type ExternalFactory = Arc<
    dyn Fn(
            &str,
            &ProviderConfig,
            Option<&RoleBinding>,
            Arc<Governor>,
        ) -> Result<Arc<dyn ExternalAdapter>>
        + Send
        + Sync,
>;

/// One line of the role report.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct RoleReport {
    /// Role name.
    pub role: String,
    /// Provider name.
    pub provider: String,
    /// Adapter kind.
    pub adapter: String,
    /// Model.
    pub model: Option<String>,
    /// Base URL for HTTP adapters.
    pub base_url: Option<String>,
    /// Why the binding cannot be used, if it cannot.
    pub problem: Option<String>,
}

/// The registry.
pub struct ProviderRegistry {
    config: Arc<Config>,
    governors: Mutex<BTreeMap<String, Arc<Governor>>>,
    adapters: Mutex<BTreeMap<String, Arc<Adapter>>>,
    factories: Mutex<BTreeMap<&'static str, ExternalFactory>>,
    cancel: CancellationToken,
}

impl std::fmt::Debug for ProviderRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderRegistry")
            .field(
                "providers",
                &self.config.providers.keys().collect::<Vec<_>>(),
            )
            .field("roles", &self.config.roles.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl ProviderRegistry {
    /// Registry over a config. Nothing is built until a role is asked for.
    pub fn new(config: Arc<Config>, cancel: CancellationToken) -> Self {
        Self {
            config,
            governors: Mutex::new(BTreeMap::new()),
            adapters: Mutex::new(BTreeMap::new()),
            factories: Mutex::new(BTreeMap::new()),
            cancel,
        }
    }

    /// Register a constructor for an adapter kind implemented elsewhere
    /// (`onnx_local`).
    pub fn register_factory(&self, kind: &'static str, factory: ExternalFactory) {
        if let Ok(mut f) = self.factories.lock() {
            f.insert(kind, factory);
        }
    }

    /// The configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// The governor for a provider name (created on first use).
    pub fn governor(&self, provider: &str) -> Arc<Governor> {
        let mut g = match self.governors.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        g.entry(provider.to_string())
            .or_insert_with(|| {
                let cfg = self.config.providers.get(provider);
                Arc::new(match cfg {
                    Some(c) => Governor::from_config(provider, c),
                    None => Governor::unlimited(provider),
                })
            })
            .clone()
    }

    /// Whether a role is bound in the config (not whether it works).
    pub fn has_role(&self, role: &str) -> bool {
        self.config.roles.contains_key(role)
    }

    fn adapter_for_role(&self, role: &str) -> Result<Arc<Adapter>> {
        let (binding, cfg) = self
            .config
            .provider_for_role(role)
            .map_err(|e| ProviderError::NotConfigured(e.to_string()))?;
        // Overrides make a distinct instance; the governor is per provider.
        let key = format!(
            "{}|{}|{}",
            binding.provider,
            binding.model.as_deref().unwrap_or(""),
            binding.base_url.as_deref().unwrap_or("")
        );
        if let Some(a) = self.adapters.lock().ok().and_then(|m| m.get(&key).cloned()) {
            return Ok(a);
        }
        let governor = self.governor(&binding.provider);
        let adapter = match AdapterKind::parse(&cfg.adapter) {
            Some(AdapterKind::OpenAiCompat) => Adapter::OpenAiCompat(Arc::new(
                OpenAiCompat::from_config(
                    &binding.provider,
                    cfg,
                    Some(binding),
                    governor,
                    self.cancel.clone(),
                )?,
            )),
            Some(AdapterKind::Anthropic) => Adapter::Anthropic(Arc::new(Anthropic::from_config(
                &binding.provider,
                cfg,
                Some(binding),
                governor,
                self.cancel.clone(),
            )?)),
            Some(AdapterKind::Gemini) => Adapter::Gemini(Arc::new(Gemini::from_config(
                &binding.provider,
                cfg,
                Some(binding),
                governor,
                self.cancel.clone(),
            )?)),
            Some(kind) => {
                let factory = self
                    .factories
                    .lock()
                    .ok()
                    .and_then(|f| f.get(kind.as_str()).cloned());
                match factory {
                    Some(f) => Adapter::External(f(&binding.provider, cfg, Some(binding), governor)?),
                    None => {
                        return Err(ProviderError::NotConfigured(format!(
                            "provider '{}' uses adapter '{}', which is not available in this build yet",
                            binding.provider, cfg.adapter
                        )))
                    }
                }
            }
            None => {
                return Err(ProviderError::NotConfigured(format!(
                    "provider '{}' names unknown adapter '{}'; known: openai_compat, onnx_local, gemini, anthropic",
                    binding.provider, cfg.adapter
                )))
            }
        };
        let adapter = Arc::new(adapter);
        if let Ok(mut m) = self.adapters.lock() {
            m.insert(key, adapter.clone());
        }
        Ok(adapter)
    }

    fn unsupported(&self, role: &str, capability: &'static str) -> ProviderError {
        let provider = self
            .config
            .roles
            .get(role)
            .map(|b| b.provider.clone())
            .unwrap_or_default();
        ProviderError::Unsupported {
            provider,
            capability,
        }
    }

    /// The ASR provider for the `asr` role.
    pub fn asr(&self) -> Result<Arc<dyn Asr>> {
        match &*self.adapter_for_role(roles::ASR)? {
            Adapter::OpenAiCompat(a) => Ok(a.clone()),
            Adapter::External(e) => e
                .as_asr()
                .ok_or_else(|| self.unsupported(roles::ASR, "asr")),
            _ => Err(self.unsupported(roles::ASR, "asr")),
        }
    }

    /// The OCR provider for the `ocr` role.
    pub fn ocr(&self) -> Result<Arc<dyn Ocr>> {
        match &*self.adapter_for_role(roles::OCR)? {
            Adapter::External(e) => e
                .as_ocr()
                .ok_or_else(|| self.unsupported(roles::OCR, "ocr")),
            _ => Err(self.unsupported(roles::OCR, "ocr")),
        }
    }

    /// The image embedder for the `image_embed` role.
    pub fn image_embedder(&self) -> Result<Arc<dyn ImageEmbedder>> {
        match &*self.adapter_for_role(roles::IMAGE_EMBED)? {
            Adapter::External(e) => e
                .as_image_embedder()
                .ok_or_else(|| self.unsupported(roles::IMAGE_EMBED, "image_embed")),
            _ => Err(self.unsupported(roles::IMAGE_EMBED, "image_embed")),
        }
    }

    /// The text embedder for the `text_embed` role.
    pub fn text_embedder(&self) -> Result<Arc<dyn TextEmbedder>> {
        match &*self.adapter_for_role(roles::TEXT_EMBED)? {
            Adapter::OpenAiCompat(a) => Ok(a.clone()),
            Adapter::External(e) => e
                .as_text_embedder()
                .ok_or_else(|| self.unsupported(roles::TEXT_EMBED, "text_embed")),
            _ => Err(self.unsupported(roles::TEXT_EMBED, "text_embed")),
        }
    }

    /// The reranker for the `reranker` role.
    pub fn reranker(&self) -> Result<Arc<dyn Reranker>> {
        match &*self.adapter_for_role(roles::RERANKER)? {
            Adapter::External(e) => e
                .as_reranker()
                .ok_or_else(|| self.unsupported(roles::RERANKER, "reranker")),
            _ => Err(self.unsupported(roles::RERANKER, "reranker")),
        }
    }

    /// The LLM for a role (`agent_llm`, `extract_llm`).
    pub fn llm(&self, role: &str) -> Result<Arc<dyn Llm>> {
        match &*self.adapter_for_role(role)? {
            Adapter::OpenAiCompat(a) => Ok(a.clone()),
            Adapter::Anthropic(a) => Ok(a.clone()),
            Adapter::Gemini(a) => Ok(a.clone()),
            Adapter::External(_) => Err(self.unsupported(role, "llm")),
        }
    }

    /// The VLM for a role (`vlm_describe`, `agent_vlm`).
    pub fn vlm(&self, role: &str) -> Result<Arc<dyn Vlm>> {
        match &*self.adapter_for_role(role)? {
            Adapter::OpenAiCompat(a) => Ok(a.clone()),
            Adapter::Anthropic(a) => Ok(a.clone()),
            Adapter::Gemini(a) => Ok(a.clone()),
            Adapter::External(_) => Err(self.unsupported(role, "vlm")),
        }
    }

    /// The agent's multimodal model: `agent_vlm`, else `agent_llm`.
    pub fn agent_vlm(&self) -> Result<Arc<dyn Vlm>> {
        if self.has_role(roles::AGENT_VLM) {
            self.vlm(roles::AGENT_VLM)
        } else {
            self.vlm(roles::AGENT_LLM)
        }
    }

    /// One line per configured role, with the problem if it cannot be
    /// built. Used by `vi doctor`.
    pub fn report(&self) -> Vec<RoleReport> {
        let mut out = Vec::new();
        for (role, binding) in &self.config.roles {
            let cfg = self.config.providers.get(&binding.provider);
            let problem = match cfg {
                None => Some(format!("no [providers.{}] table", binding.provider)),
                Some(_) => self.adapter_for_role(role).err().map(|e| e.to_string()),
            };
            out.push(RoleReport {
                role: role.clone(),
                provider: binding.provider.clone(),
                adapter: cfg.map(|c| c.adapter.clone()).unwrap_or_default(),
                model: binding
                    .model
                    .clone()
                    .or_else(|| cfg.and_then(|c| c.model.clone())),
                base_url: binding
                    .base_url
                    .clone()
                    .or_else(|| cfg.and_then(|c| c.base_url.clone())),
                problem,
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(toml: &str) -> Arc<Config> {
        Arc::new(Config::from_toml_str(toml).unwrap())
    }

    #[test]
    fn binds_asr_role_to_openai_compat() {
        let reg = ProviderRegistry::new(
            config(
                r#"
                [providers.whisper]
                adapter = "openai_compat"
                base_url = "http://127.0.0.1:9000/v1"
                model = "large-v3"
                [roles]
                asr = { provider = "whisper" }
                text_embed = { provider = "whisper", model = "bge" }
                "#,
            ),
            CancellationToken::new(),
        );
        let asr = reg.asr().unwrap();
        assert_eq!(asr.model(), "large-v3");
        assert!(asr.asr_capabilities().word_timestamps);
        // Same provider under another role with a model override is a
        // different instance sharing the governor.
        let emb = reg.text_embedder().unwrap();
        assert_eq!(emb.model(), "bge");
        assert!(Arc::ptr_eq(
            &reg.governor("whisper"),
            &reg.governor("whisper")
        ));
        // Not bound.
        assert!(matches!(reg.ocr(), Err(ProviderError::NotConfigured(_))));
        let report = reg.report();
        assert_eq!(report.len(), 2);
        assert!(report.iter().all(|r| r.problem.is_none()));
    }

    #[test]
    fn reports_problems() {
        let reg = ProviderRegistry::new(
            config(
                r#"
                [providers.nokey]
                adapter = "openai_compat"
                [providers.weird]
                adapter = "quantum"
                [roles]
                asr = { provider = "nokey" }
                ocr = { provider = "weird" }
                text_embed = { provider = "missing" }
                "#,
            ),
            CancellationToken::new(),
        );
        let r = reg.report();
        assert!(r[0].problem.as_deref().unwrap().contains("base_url"));
        assert!(r[1].problem.as_deref().unwrap().contains("unknown adapter"));
        assert!(r[2]
            .problem
            .as_deref()
            .unwrap()
            .contains("no [providers.missing]"));
    }
}
