//! `@videoindex/core`: the Node.js binding. Methods are async and run on the
//! binding's tokio runtime; `ask` returns a stream object the JS wrapper turns
//! into an `AsyncIterable`. Timestamps come out as seconds.

use std::path::Path;
use std::sync::Arc;

use futures::stream::BoxStream;
use futures::StreamExt;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use vi_agent::{Agent, AskBudget, AskEvent, AskRequest, RetrievalOnlyPolicy};
use vi_core::config::Config;
use vi_core::time::flatten_timestamps_json;
use vi_core::VideoId;
use vi_index::{EmbeddedIndex, Kind, Storage};
use vi_providers::ProviderRegistry;

fn err(e: impl std::fmt::Display) -> Error {
    Error::from_reason(e.to_string())
}

fn json<T: serde::Serialize>(v: &T) -> Result<Value> {
    let mut value = serde_json::to_value(v).map_err(err)?;
    flatten_timestamps_json(&mut value);
    Ok(value)
}

fn parse_videos(v: Option<Vec<String>>) -> Result<Vec<VideoId>> {
    v.unwrap_or_default()
        .iter()
        .map(|s| VideoId::parse(s).map_err(|_| err(format!("'{s}' is not a video id"))))
        .collect()
}

/// How to open an index.
#[napi(object)]
#[derive(Debug, Default)]
pub struct OpenOptions {
    /// Path of a `videoindex.toml`.
    pub config_path: Option<String>,
    /// TOML text (takes precedence over `config_path`).
    pub config_toml: Option<String>,
}

/// Search options.
#[napi(object)]
#[derive(Debug, Default)]
pub struct SearchOptions {
    /// Results (default 10).
    pub k: Option<u32>,
    /// Restrict to video ids.
    pub videos: Option<Vec<String>>,
    /// `transcript`, `ocr`, `description`, `frame`.
    pub kinds: Option<Vec<String>>,
    /// BM25 only.
    pub text_only: Option<bool>,
}

/// Ask options.
#[napi(object)]
#[derive(Debug, Default)]
pub struct AskOptions {
    /// Restrict to video ids.
    pub videos: Option<Vec<String>>,
    /// Conversation to continue.
    pub session_id: Option<String>,
    /// `agent` (default) or `retrieval-only`.
    pub policy: Option<String>,
    /// Tokens across all calls.
    pub max_tokens: Option<u32>,
    /// USD.
    pub max_cost_usd: Option<f64>,
    /// Seconds.
    pub max_wallclock_secs: Option<f64>,
    /// Tool calls.
    pub max_tool_calls: Option<u32>,
}

fn load_config(opts: &OpenOptions) -> Result<Arc<Config>> {
    let cfg = if let Some(text) = &opts.config_toml {
        Config::from_toml_str(text).map_err(err)?
    } else {
        Config::load(opts.config_path.as_deref().map(Path::new)).map_err(err)?
    };
    Ok(Arc::new(cfg))
}

/// An open index.
#[napi]
pub struct Index {
    storage: Arc<EmbeddedIndex>,
    config: Arc<Config>,
    providers: Arc<ProviderRegistry>,
    path: String,
}

#[napi]
impl Index {
    fn with(storage: EmbeddedIndex, opts: Option<OpenOptions>, path: &str) -> Result<Self> {
        let config = load_config(&opts.unwrap_or_default())?;
        let providers = Arc::new(ProviderRegistry::new(
            config.clone(),
            CancellationToken::new(),
        ));
        vi_perceive::OnnxLocal::register(&providers);
        Ok(Self {
            storage: Arc::new(storage),
            config,
            providers,
            path: path.to_string(),
        })
    }

    /// Open an existing index directory.
    #[napi(factory)]
    pub fn open(path: String, opts: Option<OpenOptions>) -> Result<Index> {
        let storage = EmbeddedIndex::open(Path::new(&path)).map_err(err)?;
        Self::with(storage, opts, &path)
    }

    /// Create an empty index directory.
    #[napi(factory)]
    pub fn create(path: String, opts: Option<OpenOptions>) -> Result<Index> {
        let storage = EmbeddedIndex::create(Path::new(&path)).map_err(err)?;
        Self::with(storage, opts, &path)
    }

    /// Directory.
    #[napi(getter)]
    pub fn path(&self) -> String {
        self.path.clone()
    }

    /// Videos in the index.
    #[napi]
    pub async fn videos(&self) -> Result<Value> {
        json(&self.storage.list_videos().await.map_err(err)?)
    }

    /// Sizes, counts and jobs.
    #[napi]
    pub async fn status(&self) -> Result<Value> {
        json(&self.storage.stats().await.map_err(err)?)
    }

    /// Segments of a video: `chapter` (default), `scene` or `shot`.
    #[napi]
    pub async fn timeline(&self, video_id: String, level: Option<String>) -> Result<Value> {
        let vid = VideoId::parse(&video_id).map_err(|_| err("bad video id"))?;
        let level = vi_core::model::SegmentLevel::parse(level.as_deref().unwrap_or("chapter"))
            .ok_or_else(|| err("level must be chapter, scene or shot"))?;
        json(&self.storage.segments(vid, level).await.map_err(err)?)
    }

    /// Hybrid search.
    #[napi]
    pub async fn search(&self, query: String, opts: Option<SearchOptions>) -> Result<Value> {
        let opts = opts.unwrap_or_default();
        let kinds = opts
            .kinds
            .unwrap_or_default()
            .iter()
            .map(|k| match k.as_str() {
                "transcript" => Ok(Kind::Transcript),
                "ocr" => Ok(Kind::Ocr),
                "description" => Ok(Kind::Description),
                "frame" => Ok(Kind::Frame),
                other => Err(err(format!("unknown kind '{other}'"))),
            })
            .collect::<Result<Vec<_>>>()?;
        let req = vi_query::SearchRequest {
            query,
            videos: parse_videos(opts.videos)?,
            kinds,
            k: opts.k.unwrap_or(10).clamp(1, 100) as usize,
            text_only: opts.text_only.unwrap_or(false),
        };
        let resp = vi_query::search(self.storage.as_ref(), Some(&self.providers), &req)
            .await
            .map_err(err)?;
        json(&resp)
    }

    /// Index a source (file, directory, URL) to completion with a policy;
    /// returns the job reports.
    #[napi]
    pub async fn add(
        &self,
        source: String,
        policy: Option<String>,
        force: Option<bool>,
    ) -> Result<Value> {
        let sched = vi_pipeline::Scheduler::with_providers(
            self.storage.clone(),
            self.config.clone(),
            vi_core::EventBus::default(),
            self.providers.clone(),
        );
        let src = vi_media::Source::parse(&source);
        let mut reports = Vec::new();
        // Like `vi index`: one failing file does not abort the rest; it is
        // reported as `{ok: false, error}` in its slot.
        for s in sched.expand(&src).await.map_err(err)? {
            let uri = s.uri();
            match sched
                .run(
                    s,
                    vi_pipeline::JobOptions {
                        policy: policy.clone(),
                        force: force.unwrap_or(false),
                        ..vi_pipeline::JobOptions::default()
                    },
                    CancellationToken::new(),
                )
                .await
            {
                Ok(r) => reports.push(json(&r)?),
                Err(e) => reports
                    .push(serde_json::json!({"ok": false, "source": uri, "error": e.to_string()})),
            }
        }
        Ok(Value::Array(reports))
    }

    /// Ask a question; the JS wrapper turns the returned stream into an
    /// `AsyncIterable` of event objects.
    #[napi]
    pub fn ask(&self, question: String, opts: Option<AskOptions>) -> Result<AskStream> {
        let opts = opts.unwrap_or_default();
        let mut agent = Agent::new(
            self.storage.clone(),
            self.providers.clone(),
            self.config.clone(),
        );
        match opts.policy.as_deref().unwrap_or("agent") {
            "agent" => {}
            "retrieval-only" | "retrieval_only" => {
                agent = agent.with_policy(Arc::new(RetrievalOnlyPolicy { k: 8 }));
            }
            other => return Err(err(format!("unknown policy '{other}'"))),
        }
        let d = AskBudget::default();
        let req = AskRequest {
            question,
            videos: parse_videos(opts.videos)?,
            budget: AskBudget {
                max_tokens: opts.max_tokens.map(u64::from).unwrap_or(d.max_tokens),
                max_cost_usd: opts.max_cost_usd.unwrap_or(d.max_cost_usd),
                max_wallclock_secs: opts.max_wallclock_secs.unwrap_or(d.max_wallclock_secs),
                max_tool_calls: opts.max_tool_calls.unwrap_or(d.max_tool_calls),
            },
            session_id: opts.session_id,
        };
        // The stream is created on first `next()`, inside the async
        // runtime, because `Agent::ask` spawns tasks.
        Ok(AskStream {
            inner: Arc::new(tokio::sync::Mutex::new(Pending::NotStarted(Box::new((
                agent, req,
            ))))),
        })
    }
}

enum Pending {
    NotStarted(Box<(Agent, AskRequest)>),
    Running(BoxStream<'static, AskEvent>),
    Finished,
}

/// A stream of ask events; `next()` resolves to an event or `null` at the end.
#[napi]
pub struct AskStream {
    inner: Arc<tokio::sync::Mutex<Pending>>,
}

#[napi]
impl AskStream {
    /// Next event, or `null` when the stream is finished.
    #[napi]
    pub async fn next(&self) -> Result<Option<Value>> {
        let mut guard = self.inner.lock().await;
        if let Pending::NotStarted(_) = &*guard {
            let taken = std::mem::replace(&mut *guard, Pending::Finished);
            if let Pending::NotStarted(boxed) = taken {
                let (agent, req) = *boxed;
                *guard = Pending::Running(Box::pin(agent.ask(req)));
            }
        }
        let Pending::Running(stream) = &mut *guard else {
            return Ok(None);
        };
        match stream.next().await {
            Some(ev) => {
                let done = matches!(ev, AskEvent::Done { .. });
                let v = json(&ev)?;
                if done {
                    *guard = Pending::Finished;
                }
                Ok(Some(v))
            }
            None => {
                *guard = Pending::Finished;
                Ok(None)
            }
        }
    }
}

/// Library version.
#[napi]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

impl std::fmt::Debug for Index {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Index")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for AskStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AskStream").finish_non_exhaustive()
    }
}
