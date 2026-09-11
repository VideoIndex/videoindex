//! Configuration: one `videoindex.toml` shared by all surfaces, with `VI_`
//! environment overrides (`VI_MEDIA__CACHE_DIR=/x` sets `media.cache_dir`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use figment::providers::{Env, Format, Serialized, Toml};
use figment::Figment;
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Environment variable naming the config file when `--config` is absent.
pub const CONFIG_PATH_ENV: &str = "VI_CONFIG";
/// Prefix for environment overrides.
pub const ENV_PREFIX: &str = "VI_";
/// Separator between nested keys in environment overrides.
pub const ENV_SPLIT: &str = "__";

/// Top-level configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Media cache and decode worker settings.
    pub media: MediaConfig,
    /// Index storage settings.
    pub index: IndexConfig,
    /// Local model files and in-process inference settings.
    pub models: ModelsConfig,
    /// Named indexing policies.
    pub policy: BTreeMap<String, IndexPolicy>,
    /// Policy used when none is requested.
    pub default_policy: String,
    /// Provider definitions (opaque in M0; adapters arrive in M1/M2).
    pub providers: BTreeMap<String, ProviderConfig>,
    /// Role bindings (opaque in M0).
    pub roles: BTreeMap<String, RoleBinding>,
    /// Server settings.
    pub server: ServerConfig,
    /// Logging settings.
    pub log: LogConfig,
}

impl Default for Config {
    fn default() -> Self {
        let mut policy = BTreeMap::new();
        policy.insert(
            "lecture_default".to_string(),
            IndexPolicy::lecture_default(),
        );
        policy.insert("coarse_only".to_string(), IndexPolicy::coarse_only());
        policy.insert("m0".to_string(), IndexPolicy::m0());
        policy.insert("coarse_local".to_string(), IndexPolicy::coarse_local());
        Self {
            media: MediaConfig::default(),
            index: IndexConfig::default(),
            models: ModelsConfig::default(),
            policy,
            default_policy: "coarse_local".to_string(),
            providers: BTreeMap::new(),
            roles: BTreeMap::new(),
            server: ServerConfig::default(),
            log: LogConfig::default(),
        }
    }
}

/// Media cache and decode worker settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct MediaConfig {
    /// Content-addressed media cache directory.
    pub cache_dir: PathBuf,
    /// Decode worker limits.
    pub worker: WorkerConfig,
    /// Longest side of frames delivered to in-process operators, in pixels.
    /// Frames are scaled in the worker so only what operators need crosses
    /// the process boundary.
    pub sample_max_dim: u32,
}

impl Default for MediaConfig {
    fn default() -> Self {
        Self {
            cache_dir: default_cache_dir(),
            worker: WorkerConfig::default(),
            sample_max_dim: 640,
        }
    }
}

/// Decode worker process limits.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct WorkerConfig {
    /// Explicit worker executable. Defaults to re-executing the current
    /// binary with the hidden worker subcommand.
    pub path: Option<PathBuf>,
    /// Wall-clock limit per request (probe or decode of one range).
    pub timeout_secs: u64,
    /// Address-space limit for the worker, in MiB. 0 disables.
    pub memory_limit_mb: u64,
    /// Frames in flight between worker and parent (shared-memory slots).
    pub max_in_flight_frames: usize,
    /// Decoder threads. 0 means "number of CPUs".
    pub decode_threads: usize,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            path: None,
            timeout_secs: 900,
            memory_limit_mb: 4096,
            max_in_flight_frames: 16,
            decode_threads: 0,
        }
    }
}

/// Local model files and ONNX Runtime settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ModelsConfig {
    /// Directory holding one subdirectory per model (`silero-vad/`,
    /// `siglip-base-patch16-224/`, `rapidocr/`, ...).
    pub dir: PathBuf,
    /// `auto` (CUDA when the runtime and libraries are present, else CPU),
    /// `cpu`, or `cuda`.
    pub device: String,
    /// Threads per ONNX Runtime session on the CPU. 0 means half the CPUs.
    pub onnx_threads: usize,
    /// Voice activity detection.
    pub vad: VadConfig,
    /// Speech recognition chunking.
    pub asr: AsrConfig,
    /// On-screen text.
    pub ocr: OcrGateConfig,
}

impl Default for ModelsConfig {
    fn default() -> Self {
        Self {
            dir: default_models_dir(),
            device: "auto".to_string(),
            onnx_threads: 0,
            vad: VadConfig::default(),
            asr: AsrConfig::default(),
            ocr: OcrGateConfig::default(),
        }
    }
}

/// When the `ocr` operator reads a frame. A frame is read when it is the
/// first, or its pHash differs from the last frame read by more than the
/// dedup distance, or its pixel change against the last frame read is at
/// least `min_pixel_change`, or `max_gap_secs` have passed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct OcrGateConfig {
    /// Fraction of the small grey image (0-1) that must change.
    pub min_pixel_change: f32,
    /// Read a frame at least this often, seconds.
    pub max_gap_secs: f64,
    /// Drop lines shorter than this many characters.
    pub min_chars: usize,
}

impl Default for OcrGateConfig {
    fn default() -> Self {
        Self {
            min_pixel_change: 0.02,
            max_gap_secs: 30.0,
            min_chars: 2,
        }
    }
}

/// Silero VAD parameters (`docs/05-indexing-pipeline.md`, audio operators).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct VadConfig {
    /// Speech probability at or above which a window starts speech.
    pub threshold: f32,
    /// Probability below which speech ends (hysteresis).
    pub neg_threshold: f32,
    /// Speech shorter than this is dropped, milliseconds.
    pub min_speech_ms: u32,
    /// Silence shorter than this joins its neighbours into one segment,
    /// milliseconds.
    pub min_silence_ms: u32,
    /// Padding added on both sides of a segment, milliseconds.
    pub pad_ms: u32,
    /// Segments longer than this are split at their longest internal pause.
    /// Long segments let a batched Whisper server fill its GPU batch; the
    /// server cuts them into 30 s pieces at pauses itself.
    pub max_segment_secs: f64,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            threshold: 0.5,
            neg_threshold: 0.35,
            min_speech_ms: 250,
            min_silence_ms: 1000,
            pad_ms: 200,
            max_segment_secs: 120.0,
        }
    }
}

/// ASR operator settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct AsrConfig {
    /// Language hint passed to the provider; `None` lets it detect.
    pub language: Option<String>,
    /// Skip ASR when the video already has human-authored subtitles.
    pub skip_if_human_subtitles: bool,
    /// Target length of stored transcript spans, seconds (short ASR
    /// segments are grouped up to this).
    pub span_secs: f64,
    /// Concurrent ASR requests per job.
    pub concurrency: usize,
}

impl Default for AsrConfig {
    fn default() -> Self {
        Self {
            language: None,
            skip_if_human_subtitles: false,
            span_secs: 15.0,
            concurrency: 4,
        }
    }
}

fn default_models_dir() -> PathBuf {
    if Path::new("/data/videoindex").is_dir() {
        return PathBuf::from("/data/videoindex/models");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".cache")
            .join("videoindex")
            .join("models");
    }
    std::env::temp_dir().join("videoindex").join("models")
}

/// Index storage settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct IndexConfig {
    /// Thumbnail longest side in pixels.
    pub thumbnail_px: u32,
    /// WebP quality for thumbnails, 0-100.
    pub thumbnail_quality: u8,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            thumbnail_px: 320,
            thumbnail_quality: 75,
        }
    }
}

/// A named indexing policy (`docs/05-indexing-pipeline.md`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct IndexPolicy {
    /// Coarse-pass sampling rate.
    pub sample_fps: f64,
    /// Operators in the coarse pass.
    pub coarse: Vec<String>,
    /// Operators in the fine pass.
    pub fine: Vec<String>,
    /// Frame grid layout for VLM calls, e.g. `"3x3"`.
    pub vlm_grid: String,
    /// Cost ceiling per hour of video.
    pub max_cost_usd_per_hour: f64,
    /// Wall-clock ceiling per hour of video, humantime-style (`"20m"`).
    pub max_wallclock_per_hour: String,
}

impl Default for IndexPolicy {
    fn default() -> Self {
        Self::lecture_default()
    }
}

impl IndexPolicy {
    /// The default policy from the design docs.
    pub fn lecture_default() -> Self {
        Self {
            sample_fps: 1.0,
            coarse: [
                "vad",
                "asr",
                "shot_boundary",
                "phash",
                "image_embed",
                "ocr",
                "thumbnail",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            fine: [
                "scenes",
                "chapters",
                "vlm_describe",
                "entities_events",
                "text_embed",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            vlm_grid: "3x3".to_string(),
            max_cost_usd_per_hour: 2.0,
            max_wallclock_per_hour: "20m".to_string(),
        }
    }

    /// Coarse pass only.
    pub fn coarse_only() -> Self {
        Self {
            fine: Vec::new(),
            ..Self::lecture_default()
        }
    }

    /// The M0 policy: sampling, perceptual hashes, thumbnails.
    pub fn m0() -> Self {
        Self {
            coarse: ["sample", "phash", "thumbnail"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            fine: Vec::new(),
            max_cost_usd_per_hour: 0.0,
            max_wallclock_per_hour: "5m".to_string(),
            ..Self::lecture_default()
        }
    }

    /// Everything that runs without a model provider: sidecar subtitles,
    /// sampling, perceptual hashes, thumbnails. The default until the
    /// provider-backed operators exist.
    pub fn coarse_local() -> Self {
        Self {
            coarse: ["subtitle_import", "sample", "phash", "thumbnail"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            fine: Vec::new(),
            max_cost_usd_per_hour: 0.0,
            max_wallclock_per_hour: "5m".to_string(),
            ..Self::lecture_default()
        }
    }

    /// All operators in run order.
    pub fn operators(&self) -> impl Iterator<Item = &str> {
        self.coarse
            .iter()
            .chain(self.fine.iter())
            .map(String::as_str)
    }
}

/// One `[providers.<name>]` table (`docs/07-model-providers.md`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct ProviderConfig {
    /// Adapter name: `openai_compat`, `onnx_local`, `gemini`, `anthropic`.
    pub adapter: String,
    /// Environment variable holding the API key. Keys are read once at
    /// start-up and never logged.
    pub api_key_env: Option<String>,
    /// Base URL for HTTP adapters, e.g. `http://127.0.0.1:9000/v1`.
    pub base_url: Option<String>,
    /// Default model.
    pub model: Option<String>,
    /// Max in-flight requests to this provider.
    pub concurrency: Option<u32>,
    /// Request rate limit (token bucket refilled at this rate).
    pub requests_per_minute: Option<u32>,
    /// Token rate limit for LLM/VLM/embedding calls.
    pub tokens_per_minute: Option<u32>,
    /// Per-request timeout in seconds.
    pub timeout_secs: Option<u64>,
    /// Retries after the first attempt on 429, 5xx and transport errors.
    pub max_retries: Option<u32>,
    /// Price table override for cost accounting.
    pub pricing: Option<Pricing>,
    /// Directory holding model files for local adapters.
    pub model_dir: Option<PathBuf>,
    /// Anything adapter-specific.
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}

/// Prices in USD used for cost accounting. All default to zero, which is
/// right for local servers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Pricing {
    /// Per million input tokens.
    pub input_per_mtok: f64,
    /// Per million output tokens.
    pub output_per_mtok: f64,
    /// Per input image.
    pub per_image: f64,
    /// Per second of audio or video sent.
    pub per_media_second: f64,
    /// Flat fee per call.
    pub per_call: f64,
}

/// One `[roles]` entry: which provider (and optionally which model or base
/// URL) serves a capability role.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct RoleBinding {
    /// Provider name (a key of `[providers]`).
    pub provider: String,
    /// Model override.
    pub model: Option<String>,
    /// Base URL override.
    pub base_url: Option<String>,
}

/// Role names the pipeline and agent look up in `[roles]`.
pub mod roles {
    /// Speech to text.
    pub const ASR: &str = "asr";
    /// On-screen text.
    pub const OCR: &str = "ocr";
    /// Frame embeddings (and the matching text tower).
    pub const IMAGE_EMBED: &str = "image_embed";
    /// Text embeddings.
    pub const TEXT_EMBED: &str = "text_embed";
    /// Scene descriptions.
    pub const VLM_DESCRIBE: &str = "vlm_describe";
    /// Entity and event extraction.
    pub const EXTRACT_LLM: &str = "extract_llm";
    /// The agent's reasoning model.
    pub const AGENT_LLM: &str = "agent_llm";
    /// The agent's multimodal model (defaults to `agent_llm`).
    pub const AGENT_VLM: &str = "agent_vlm";
    /// Cross-encoder reranking.
    pub const RERANKER: &str = "reranker";
    /// Every role name, for validation.
    pub const ALL: &[&str] = &[
        ASR,
        OCR,
        IMAGE_EMBED,
        TEXT_EMBED,
        VLM_DESCRIBE,
        EXTRACT_LLM,
        AGENT_LLM,
        AGENT_VLM,
        RERANKER,
    ];
}

/// Server settings (used from M3).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ServerConfig {
    /// Bind address.
    pub bind: String,
    /// Directory holding indexes.
    pub index_root: PathBuf,
    /// Serve MCP too.
    pub mcp: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8090".to_string(),
            index_root: PathBuf::from("/data/videoindex/indexes"),
            mcp: true,
        }
    }
}

/// Logging.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct LogConfig {
    /// `tracing` filter directive, e.g. `info` or `vi_media=debug`.
    pub level: String,
    /// Emit JSON lines instead of human text.
    pub json: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            json: false,
        }
    }
}

fn default_cache_dir() -> PathBuf {
    if Path::new("/data/videoindex").is_dir() {
        return PathBuf::from("/data/videoindex/videos");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".cache")
            .join("videoindex")
            .join("videos");
    }
    std::env::temp_dir().join("videoindex").join("videos")
}

impl Config {
    /// Defaults, then the TOML file (if any), then `VI_*` environment overrides.
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let mut fig = Figment::from(Serialized::defaults(Config::default()));
        let path = path
            .map(Path::to_path_buf)
            .or_else(|| std::env::var_os(CONFIG_PATH_ENV).map(PathBuf::from));
        if let Some(p) = path {
            if !p.is_file() {
                return Err(Error::Config(format!(
                    "config file not found: {}",
                    p.display()
                )));
            }
            fig = fig.merge(Toml::file(p));
        }
        fig = fig.merge(
            Env::prefixed(ENV_PREFIX)
                .split(ENV_SPLIT)
                .ignore(&["CONFIG"]),
        );
        fig.extract().map_err(|e| Error::Config(e.to_string()))
    }

    /// Parse from a TOML string on top of defaults. No environment.
    pub fn from_toml_str(s: &str) -> Result<Self> {
        Figment::from(Serialized::defaults(Config::default()))
            .merge(Toml::string(s))
            .extract()
            .map_err(|e| Error::Config(e.to_string()))
    }

    /// The role binding for a role name, if configured.
    pub fn role(&self, name: &str) -> Option<&RoleBinding> {
        self.roles.get(name)
    }

    /// The provider table a role points at.
    pub fn provider_for_role(&self, role: &str) -> Result<(&RoleBinding, &ProviderConfig)> {
        let binding = self.roles.get(role).ok_or_else(|| {
            Error::Provider(format!(
                "no provider bound to role '{role}'; add `[roles] {role} = {{ provider = \"...\" }}` to the config"
            ))
        })?;
        let provider = self.providers.get(&binding.provider).ok_or_else(|| {
            Error::Config(format!(
                "role '{role}' names provider '{}' but no [providers.{}] table exists",
                binding.provider, binding.provider
            ))
        })?;
        Ok((binding, provider))
    }

    /// Look up a policy by name.
    pub fn policy(&self, name: &str) -> Result<&IndexPolicy> {
        self.policy.get(name).ok_or_else(|| {
            Error::Config(format!(
                "unknown policy '{name}'; known: {}",
                self.policy.keys().cloned().collect::<Vec<_>>().join(", ")
            ))
        })
    }

    /// Serialise to TOML (for `vi doctor --show-config` and tests).
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).map_err(|e| Error::Config(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_have_builtin_policies() {
        let c = Config::default();
        assert!(c.policy.contains_key("lecture_default"));
        assert_eq!(
            c.policy("m0").unwrap().coarse,
            ["sample", "phash", "thumbnail"]
        );
        assert!(c.policy("nope").is_err());
    }

    #[test]
    fn toml_overrides_defaults_and_keeps_others() {
        let c = Config::from_toml_str(
            r#"
            [media]
            sample_max_dim = 320
            [policy.mine]
            sample_fps = 0.5
            coarse = ["sample"]
            "#,
        )
        .unwrap();
        assert_eq!(c.media.sample_max_dim, 320);
        assert_eq!(c.index.thumbnail_px, 320);
        assert_eq!(c.policy("mine").unwrap().sample_fps, 0.5);
        assert!(c.policy.contains_key("lecture_default"));
    }

    #[test]
    fn unknown_keys_are_rejected() {
        assert!(Config::from_toml_str("[media]\nbogus = 1").is_err());
    }

    #[test]
    #[allow(clippy::result_large_err)] // figment::Jail's closure signature
    fn env_overrides_apply() {
        // figment::Jail isolates environment mutation.
        figment::Jail::expect_with(|jail| {
            jail.create_file(
                "videoindex.toml",
                "[media]\nsample_max_dim = 500\n[index]\nthumbnail_px = 200",
            )?;
            jail.set_env("VI_MEDIA__SAMPLE_MAX_DIM", "777");
            jail.set_env("VI_LOG__LEVEL", "debug");
            let c = Config::load(Some(Path::new("videoindex.toml"))).unwrap();
            assert_eq!(c.media.sample_max_dim, 777);
            assert_eq!(c.index.thumbnail_px, 200);
            assert_eq!(c.log.level, "debug");
            Ok(())
        });
    }

    #[test]
    fn roles_resolve_to_providers() {
        let c = Config::from_toml_str(
            r#"
            [providers.whisper]
            adapter = "openai_compat"
            base_url = "http://127.0.0.1:9000/v1"
            model = "large-v3"
            concurrency = 2
            [roles]
            asr = { provider = "whisper" }
            ocr = { provider = "nope" }
            "#,
        )
        .unwrap();
        let (b, p) = c.provider_for_role(roles::ASR).unwrap();
        assert_eq!(b.provider, "whisper");
        assert_eq!(p.adapter, "openai_compat");
        assert_eq!(p.concurrency, Some(2));
        assert!(matches!(
            c.provider_for_role(roles::OCR),
            Err(Error::Config(_))
        ));
        assert!(matches!(
            c.provider_for_role(roles::TEXT_EMBED),
            Err(Error::Provider(_))
        ));
    }

    #[test]
    fn roundtrips_through_toml() {
        let c = Config::default();
        let s = c.to_toml().unwrap();
        let back = Config::from_toml_str(&s).unwrap();
        assert_eq!(back, c);
    }
}
