//! Shared server state: config, one provider registry (rate limits are
//! global), lazily opened indexes, the job registry, metrics and per-key
//! daily spend.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use chrono::{NaiveDate, Utc};
use tokio_util::sync::CancellationToken;
use vi_core::config::Config;
use vi_index::EmbeddedIndex;
use vi_providers::ProviderRegistry;

use crate::error::{ApiError, ApiResult};
use crate::jobs::JobRegistry;
use crate::metrics::Metrics;

/// An index the server has opened.
pub struct OpenIndex {
    /// Id (`<id>.vidx` under the index root).
    pub id: String,
    /// Directory.
    pub path: PathBuf,
    /// Storage.
    pub storage: Arc<EmbeddedIndex>,
}

/// Server state.
pub struct AppState {
    /// Config.
    pub config: Arc<Config>,
    /// Providers bound to roles.
    pub providers: Arc<ProviderRegistry>,
    indexes: RwLock<HashMap<String, Arc<OpenIndex>>>,
    /// Indexing jobs.
    pub jobs: JobRegistry,
    /// Counters.
    pub metrics: Metrics,
    quota: Mutex<HashMap<String, (NaiveDate, f64)>>,
    /// Start time, for `/healthz`.
    pub started: Instant,
}

impl AppState {
    /// State over a config.
    pub fn new(config: Arc<Config>) -> Self {
        let providers = Arc::new(ProviderRegistry::new(
            config.clone(),
            CancellationToken::new(),
        ));
        vi_perceive::OnnxLocal::register(&providers);
        Self {
            config,
            providers,
            indexes: RwLock::new(HashMap::new()),
            jobs: JobRegistry::default(),
            metrics: Metrics::default(),
            quota: Mutex::new(HashMap::new()),
            started: Instant::now(),
        }
    }

    fn valid_id(id: &str) -> ApiResult<()> {
        let stem = id.strip_suffix(".vidx").unwrap_or(id);
        if stem.is_empty()
            || !stem
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(ApiError::bad_request(
                "index id must match [A-Za-z0-9_-]+ (an optional .vidx suffix is allowed)",
            ));
        }
        Ok(())
    }

    /// Directory for an index id.
    pub fn index_path(&self, id: &str) -> PathBuf {
        let stem = id.strip_suffix(".vidx").unwrap_or(id);
        self.config.server.index_root.join(format!("{stem}.vidx"))
    }

    /// Ids of the indexes under the root (opened or not).
    pub fn index_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = std::fs::read_dir(&self.config.server.index_root)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter(|e| e.path().is_dir() && e.path().join("manifest.json").is_file())
                    .filter_map(|e| {
                        e.file_name()
                            .to_str()
                            .map(|s| s.strip_suffix(".vidx").unwrap_or(s).to_string())
                    })
                    .collect()
            })
            .unwrap_or_default();
        ids.sort();
        ids
    }

    /// Open (once) and return an index.
    pub fn index(&self, id: &str) -> ApiResult<Arc<OpenIndex>> {
        Self::valid_id(id)?;
        let key = id.strip_suffix(".vidx").unwrap_or(id).to_string();
        if let Some(ix) = self.indexes.read().ok().and_then(|m| m.get(&key).cloned()) {
            return Ok(ix);
        }
        let path = self.index_path(&key);
        if !path.join("manifest.json").is_file() {
            return Err(ApiError::not_found(format!("no index '{key}'")));
        }
        let storage = Arc::new(EmbeddedIndex::open(&path)?);
        let ix = Arc::new(OpenIndex {
            id: key.clone(),
            path,
            storage,
        });
        if let Ok(mut m) = self.indexes.write() {
            m.entry(key).or_insert_with(|| ix.clone());
        }
        Ok(ix)
    }

    /// Create an empty index.
    pub fn create_index(&self, id: &str) -> ApiResult<Arc<OpenIndex>> {
        Self::valid_id(id)?;
        let key = id.strip_suffix(".vidx").unwrap_or(id).to_string();
        let path = self.index_path(&key);
        if path.exists() {
            return Err(ApiError::new(
                axum::http::StatusCode::CONFLICT,
                "exists",
                format!("index '{key}' already exists"),
            ));
        }
        std::fs::create_dir_all(&self.config.server.index_root)
            .map_err(|e| ApiError::internal(format!("index root: {e}")))?;
        let storage = Arc::new(EmbeddedIndex::create(&path)?);
        let ix = Arc::new(OpenIndex {
            id: key.clone(),
            path,
            storage,
        });
        if let Ok(mut m) = self.indexes.write() {
            m.insert(key, ix.clone());
        }
        Ok(ix)
    }

    /// The index `/v1/mcp` and unqualified requests use.
    pub fn default_index(&self) -> ApiResult<Arc<OpenIndex>> {
        if let Some(id) = &self.config.server.default_index {
            return self.index(id);
        }
        let ids = self.index_ids();
        match ids.as_slice() {
            [one] => self.index(one),
            [] => Err(ApiError::not_found("no index under the index root")),
            _ => Err(ApiError::bad_request(
                "several indexes exist; set server.default_index or address one by id",
            )),
        }
    }

    /// Record provider spend against a key; 429 once the daily cap is hit.
    pub fn charge(&self, key: &str, usd: f64) -> ApiResult<()> {
        let cap = self.config.server.daily_cost_cap_usd;
        self.metrics.add_cost(usd);
        if cap <= 0.0 {
            return Ok(());
        }
        let today = Utc::now().date_naive();
        let mut q = self
            .quota
            .lock()
            .map_err(|_| ApiError::internal("quota lock"))?;
        let entry = q.entry(key.to_string()).or_insert((today, 0.0));
        if entry.0 != today {
            *entry = (today, 0.0);
        }
        entry.1 += usd;
        if entry.1 > cap {
            return Err(ApiError::quota(format!(
                "this key has spent ${:.2} today; the daily cap is ${cap:.2}",
                entry.1
            )));
        }
        Ok(())
    }

    /// Spend so far today for a key.
    pub fn spent_today(&self, key: &str) -> f64 {
        let today = Utc::now().date_naive();
        self.quota
            .lock()
            .ok()
            .and_then(|q| q.get(key).filter(|(d, _)| *d == today).map(|(_, v)| *v))
            .unwrap_or(0.0)
    }
}

impl std::fmt::Debug for OpenIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenIndex")
            .field("id", &self.id)
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("started", &self.started)
            .finish_non_exhaustive()
    }
}
