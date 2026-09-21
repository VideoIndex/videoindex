//! `videoindex._core`: the Python binding. Every method releases the GIL
//! while the core runs; `ask` streams events synchronously (`for ev in
//! idx.ask(...)`) or asynchronously (`async for ev in idx.aask(...)`).
//! Frames come back as NumPy arrays (a copy of the decoded RGB buffer).

#![allow(clippy::useless_conversion)]

use std::sync::{Arc, Mutex, OnceLock};

use futures::StreamExt;
use numpy::PyArrayMethods;
use pyo3::exceptions::{PyRuntimeError, PyStopAsyncIteration, PyStopIteration, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};
use tokio_util::sync::CancellationToken;
use vi_agent::{Agent, AskBudget, AskEvent, AskRequest, RetrievalOnlyPolicy};
use vi_core::config::Config as CoreConfig;
use vi_core::{EventBus, VideoId};
use vi_index::{EmbeddedIndex, Kind, Storage};
use vi_pipeline::{JobOptions, Scheduler};

mod callback;
use callback::{PyOperatorSpec, PyPolicy};
use vi_providers::ProviderRegistry;

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("videoindex")
            .build()
            .expect("tokio runtime")
    })
}

fn err<E: std::fmt::Display>(e: E) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

/// JSON to Python objects.
fn to_py<'py>(py: Python<'py>, v: &serde_json::Value) -> PyResult<Bound<'py, PyAny>> {
    Ok(match v {
        serde_json::Value::Null => py.None().into_bound(py),
        serde_json::Value::Bool(b) => b.into_pyobject(py)?.to_owned().into_any(),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_pyobject(py)?.into_any()
            } else if let Some(u) = n.as_u64() {
                u.into_pyobject(py)?.into_any()
            } else {
                n.as_f64().unwrap_or(0.0).into_pyobject(py)?.into_any()
            }
        }
        serde_json::Value::String(s) => s.into_pyobject(py)?.into_any(),
        serde_json::Value::Array(a) => {
            let list = PyList::empty(py);
            for x in a {
                list.append(to_py(py, x)?)?;
            }
            list.into_any()
        }
        serde_json::Value::Object(m) => {
            let d = PyDict::new(py);
            for (k, x) in m {
                d.set_item(k, to_py(py, x)?)?;
            }
            d.into_any()
        }
    })
}

fn json_py<'py, T: serde::Serialize>(py: Python<'py>, v: &T) -> PyResult<Bound<'py, PyAny>> {
    let mut value = serde_json::to_value(v).map_err(err)?;
    flatten_timestamps(&mut value);
    to_py(py, &value)
}

/// `{"num": .., "den": ..}` timestamps become float seconds for Python.
fn flatten_timestamps(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(m) => {
            if m.len() == 2 {
                if let (Some(n), Some(d)) = (
                    m.get("num").and_then(|x| x.as_i64()),
                    m.get("den").and_then(|x| x.as_u64()),
                ) {
                    *v = serde_json::json!(n as f64 / d.max(1) as f64);
                    return;
                }
            }
            for x in m.values_mut() {
                flatten_timestamps(x);
            }
        }
        serde_json::Value::Array(a) => a.iter_mut().for_each(flatten_timestamps),
        _ => {}
    }
}

/// `policy=` for `add`: a policy name, or a dict defining one inline
/// (`{"coarse": [...], "fine": [...], "sample_fps": 1.0, "name": "..."}`).
fn parse_policy(
    policy: Option<Bound<'_, PyAny>>,
) -> PyResult<(Option<String>, Option<vi_core::config::IndexPolicy>)> {
    let Some(p) = policy else {
        return Ok((None, None));
    };
    if let Ok(name) = p.extract::<String>() {
        return Ok((Some(name), None));
    }
    let d = p.cast::<PyDict>().map_err(|_| {
        PyValueError::new_err("policy must be a name or a dict with coarse/fine operator lists")
    })?;
    let mut v = callback::py_to_json(d.as_any())?;
    let name = v
        .as_object_mut()
        .and_then(|m| m.remove("name"))
        .and_then(|n| n.as_str().map(str::to_string))
        .unwrap_or_else(|| "inline".to_string());
    // Unset fields take the defaults of an empty policy: no operators
    // rather than `lecture_default`'s full list.
    let base = vi_core::config::IndexPolicy {
        coarse: Vec::new(),
        fine: Vec::new(),
        ..vi_core::config::IndexPolicy::default()
    };
    let mut merged = serde_json::to_value(&base).map_err(err)?;
    match (&mut merged, v) {
        (serde_json::Value::Object(m), serde_json::Value::Object(given)) => {
            for (k, x) in given {
                m.insert(k, x);
            }
        }
        _ => return Err(PyValueError::new_err("policy dict must be a mapping")),
    }
    let inline: vi_core::config::IndexPolicy = serde_json::from_value(merged)
        .map_err(|e| PyValueError::new_err(format!("bad inline policy: {e}")))?;
    Ok((Some(name), Some(inline)))
}

fn parse_videos(videos: Option<Vec<String>>) -> PyResult<Vec<VideoId>> {
    videos
        .unwrap_or_default()
        .iter()
        .map(|s| {
            VideoId::parse(s).map_err(|_| PyValueError::new_err(format!("'{s}' is not a video id")))
        })
        .collect()
}

/// Configuration (`videoindex.toml`).
#[pyclass(module = "videoindex._core", from_py_object)]
#[derive(Clone)]
struct Config {
    inner: Arc<CoreConfig>,
}

#[pymethods]
impl Config {
    /// Built-in defaults plus `VI_*` environment overrides.
    #[staticmethod]
    fn default() -> PyResult<Self> {
        Ok(Self {
            inner: Arc::new(CoreConfig::load(None).map_err(err)?),
        })
    }

    /// Load a TOML file (plus `VI_*` environment overrides).
    #[staticmethod]
    fn from_file(path: &str) -> PyResult<Self> {
        Ok(Self {
            inner: Arc::new(CoreConfig::load(Some(std::path::Path::new(path))).map_err(err)?),
        })
    }

    /// Parse TOML text over the defaults.
    #[staticmethod]
    fn from_toml(text: &str) -> PyResult<Self> {
        Ok(Self {
            inner: Arc::new(CoreConfig::from_toml_str(text).map_err(err)?),
        })
    }

    /// The effective configuration as TOML.
    fn to_toml(&self) -> PyResult<String> {
        self.inner.to_toml().map_err(err)
    }

    fn __repr__(&self) -> String {
        format!("Config(default_policy={:?})", self.inner.default_policy)
    }
}

/// Limits for one `ask`.
#[pyclass(module = "videoindex._core", from_py_object)]
#[derive(Clone)]
struct Budget {
    #[pyo3(get, set)]
    max_tokens: u64,
    #[pyo3(get, set)]
    max_cost_usd: f64,
    #[pyo3(get, set)]
    max_wallclock_secs: f64,
    #[pyo3(get, set)]
    max_tool_calls: u32,
}

#[pymethods]
impl Budget {
    #[new]
    #[pyo3(signature = (max_tokens = 50_000, max_cost_usd = 0.5, max_wallclock_secs = 120.0, max_tool_calls = 8))]
    fn new(
        max_tokens: u64,
        max_cost_usd: f64,
        max_wallclock_secs: f64,
        max_tool_calls: u32,
    ) -> Self {
        Self {
            max_tokens,
            max_cost_usd,
            max_wallclock_secs,
            max_tool_calls,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Budget(max_tokens={}, max_cost_usd={}, max_wallclock_secs={}, max_tool_calls={})",
            self.max_tokens, self.max_cost_usd, self.max_wallclock_secs, self.max_tool_calls
        )
    }
}

impl From<&Budget> for AskBudget {
    fn from(b: &Budget) -> Self {
        AskBudget {
            max_tokens: b.max_tokens,
            max_cost_usd: b.max_cost_usd,
            max_wallclock_secs: b.max_wallclock_secs,
            max_tool_calls: b.max_tool_calls,
            ..AskBudget::default()
        }
    }
}

/// An indexing job: iterate `progress()` for events, `wait()` for the report.
#[pyclass(module = "videoindex._core")]
struct Job {
    events: Mutex<Option<tokio::sync::broadcast::Receiver<vi_core::Event>>>,
    report: Arc<Mutex<Option<PyResult<serde_json::Value>>>>,
    handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    cancel: CancellationToken,
}

#[pymethods]
impl Job {
    /// Blocking iterator over progress events (dicts) until the job ends.
    fn progress(slf: PyRef<'_, Self>) -> PyResult<JobEvents> {
        let rx = slf
            .events
            .lock()
            .map_err(|_| err("job poisoned"))?
            .take()
            .ok_or_else(|| err("progress() can be iterated once"))?;
        Ok(JobEvents {
            rx: Arc::new(tokio::sync::Mutex::new(rx)),
            done: Mutex::new(false),
        })
    }

    /// Wait for the job and return its report.
    fn wait<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let handle = self.handle.lock().map_err(|_| err("job poisoned"))?.take();
        if let Some(h) = handle {
            py.detach(|| {
                let _ = runtime().block_on(h);
            });
        }
        let report = self.report.lock().map_err(|_| err("job poisoned"))?;
        match report.as_ref() {
            Some(Ok(v)) => to_py(py, v),
            Some(Err(e)) => Err(err(e)),
            None => Err(err("job produced no report")),
        }
    }

    /// Cancel the job.
    fn cancel(&self) {
        self.cancel.cancel();
    }
}

/// Iterator of job events.
#[pyclass(module = "videoindex._core")]
struct JobEvents {
    rx: Arc<tokio::sync::Mutex<tokio::sync::broadcast::Receiver<vi_core::Event>>>,
    done: Mutex<bool>,
}

#[pymethods]
impl JobEvents {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        if *self.done.lock().map_err(|_| err("poisoned"))? {
            return Err(PyStopIteration::new_err(()));
        }
        let rx = self.rx.clone();
        let ev = py.detach(|| {
            runtime().block_on(async move {
                let mut rx = rx.lock().await;
                loop {
                    match rx.recv().await {
                        Ok(ev) => return Some(ev),
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => return None,
                    }
                }
            })
        });
        match ev {
            Some(ev) => {
                if matches!(ev, vi_core::Event::JobFinished { .. }) {
                    *self.done.lock().map_err(|_| err("poisoned"))? = true;
                }
                json_py(py, &ev)
            }
            None => {
                *self.done.lock().map_err(|_| err("poisoned"))? = true;
                Err(PyStopIteration::new_err(()))
            }
        }
    }
}

/// Streamed answer events; a sync and an async iterator.
#[pyclass(module = "videoindex._core")]
struct AskStream {
    rx: Arc<tokio::sync::Mutex<futures::stream::BoxStream<'static, AskEvent>>>,
    done: Arc<std::sync::atomic::AtomicBool>,
}

#[pymethods]
impl AskStream {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        if self.done.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(PyStopIteration::new_err(()));
        }
        let rx = self.rx.clone();
        let ev = py.detach(|| runtime().block_on(async move { rx.lock().await.next().await }));
        match ev {
            Some(ev) => {
                if matches!(ev, AskEvent::Done { .. }) {
                    self.done.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                json_py(py, &ev)
            }
            None => {
                self.done.store(true, std::sync::atomic::Ordering::Relaxed);
                Err(PyStopIteration::new_err(()))
            }
        }
    }

    fn __aiter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __anext__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rx = self.rx.clone();
        let done = self.done.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            if done.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(PyStopAsyncIteration::new_err(()));
            }
            let ev = rx.lock().await.next().await;
            match ev {
                Some(ev) => {
                    if matches!(ev, AskEvent::Done { .. }) {
                        done.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    Python::attach(|py| json_py(py, &ev).map(|b| b.unbind()))
                }
                None => Err(PyStopAsyncIteration::new_err(())),
            }
        })
    }

    /// Drain the stream: `{"text", "citations", "tool_calls", "usage", "partial"}`.
    fn collect<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rx = self.rx.clone();
        let out = py.detach(|| {
            runtime().block_on(async move {
                let mut s = rx.lock().await;
                let mut c = vi_agent::Collected::default();
                while let Some(ev) = s.next().await {
                    match ev {
                        AskEvent::Token { text } => c.text.push_str(&text),
                        AskEvent::Citation {
                            video_id, t0, t1, ..
                        } => c.citations.push((video_id, t0, t1)),
                        AskEvent::ToolCall { tool, args, .. } => c.tool_calls.push((tool, args)),
                        AskEvent::Done { partial, usage, .. } => {
                            c.partial = partial;
                            c.usage = usage;
                        }
                        _ => {}
                    }
                }
                c
            })
        });
        self.done.store(true, std::sync::atomic::Ordering::Relaxed);
        json_py(py, &out)
    }
}

/// An index directory.
#[pyclass(module = "videoindex._core")]
struct Index {
    storage: Arc<EmbeddedIndex>,
    config: Mutex<Arc<CoreConfig>>,
    providers: Mutex<Arc<ProviderRegistry>>,
    operators: Mutex<Vec<Arc<PyOperatorSpec>>>,
    path: String,
}

impl Index {
    fn make_providers(config: &Arc<CoreConfig>) -> Arc<ProviderRegistry> {
        let p = Arc::new(ProviderRegistry::new(
            config.clone(),
            CancellationToken::new(),
        ));
        vi_perceive::OnnxLocal::register(&p);
        p
    }

    fn with(storage: EmbeddedIndex, config: Option<Config>, path: &str) -> PyResult<Self> {
        let config = match config {
            Some(c) => c.inner,
            None => Arc::new(CoreConfig::load(None).map_err(err)?),
        };
        Ok(Self {
            storage: Arc::new(storage),
            providers: Mutex::new(Self::make_providers(&config)),
            config: Mutex::new(config),
            operators: Mutex::new(Vec::new()),
            path: path.to_string(),
        })
    }

    fn cfg(&self) -> Arc<CoreConfig> {
        self.config
            .lock()
            .map(|c| c.clone())
            .unwrap_or_else(|p| p.into_inner().clone())
    }

    fn prov(&self) -> Arc<ProviderRegistry> {
        self.providers
            .lock()
            .map(|p| p.clone())
            .unwrap_or_else(|p| p.into_inner().clone())
    }

    fn operator_specs(&self) -> Vec<Arc<PyOperatorSpec>> {
        self.operators
            .lock()
            .map(|o| o.clone())
            .unwrap_or_else(|p| p.into_inner().clone())
    }

    fn ask_stream(
        &self,
        question: String,
        budget: Option<&Budget>,
        videos: Option<Vec<String>>,
        session_id: Option<String>,
        policy: Option<&Bound<'_, PyAny>>,
        model: Option<String>,
    ) -> PyResult<AskStream> {
        let videos = parse_videos(videos)?;
        let mut agent = Agent::new(self.storage.clone(), self.prov(), self.cfg());
        if let Some(model) = model.as_deref() {
            let provider = self.prov().find_llm_provider(model).ok_or_else(|| {
                PyValueError::new_err(format!(
                    "unknown model '{model}'; pass a [providers.*] name or its model id"
                ))
            })?;
            agent = agent.with_provider(provider);
        }
        match policy {
            None => {}
            Some(p) if p.is_instance_of::<pyo3::types::PyString>() => {
                match p.extract::<String>()?.as_str() {
                    "agent" => {}
                    "retrieval-only" | "retrieval_only" => {
                        agent = agent.with_policy(Arc::new(RetrievalOnlyPolicy { k: 8 }))
                    }
                    other => {
                        return Err(PyValueError::new_err(format!(
                            "unknown policy '{other}'; use 'agent', 'retrieval_only' or an object with next_step(state)"
                        )))
                    }
                }
            }
            Some(p) => agent = agent.with_policy(Arc::new(PyPolicy::new(p)?)),
        }
        let req = AskRequest {
            question,
            videos,
            budget: budget.map(AskBudget::from).unwrap_or_default(),
            session_id,
        };
        let stream = {
            let _guard = runtime().enter();
            agent.ask(req)
        };
        Ok(AskStream {
            rx: Arc::new(tokio::sync::Mutex::new(Box::pin(stream))),
            done: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }
}

#[pymethods]
impl Index {
    /// Create a new index directory.
    #[staticmethod]
    #[pyo3(signature = (path, config = None))]
    fn create(path: &str, config: Option<Config>) -> PyResult<Self> {
        let s = EmbeddedIndex::create(std::path::Path::new(path)).map_err(err)?;
        Self::with(s, config, path)
    }

    /// Open an existing index directory.
    #[staticmethod]
    #[pyo3(signature = (path, config = None))]
    fn open(path: &str, config: Option<Config>) -> PyResult<Self> {
        let s = EmbeddedIndex::open(std::path::Path::new(path)).map_err(err)?;
        Self::with(s, config, path)
    }

    /// Replace the configuration (providers, policies, media cache).
    fn configure(&self, config: Config) -> PyResult<()> {
        let providers = Self::make_providers(&config.inner);
        *self.config.lock().map_err(|_| err("poisoned"))? = config.inner;
        *self.providers.lock().map_err(|_| err("poisoned"))? = providers;
        Ok(())
    }

    /// Index directory.
    #[getter]
    fn path(&self) -> &str {
        &self.path
    }

    /// Index a source (path, directory, URL) with a policy. Returns a `Job`.
    #[pyo3(signature = (source, policy = None, force = false))]
    fn add(
        &self,
        py: Python<'_>,
        source: &str,
        policy: Option<Bound<'_, PyAny>>,
        force: bool,
    ) -> PyResult<Job> {
        let config = self.cfg();
        let events = EventBus::default();
        let rx = events.subscribe();
        let mut sched =
            Scheduler::with_providers(self.storage.clone(), config, events, self.prov());
        for spec in self.operator_specs() {
            let s = spec.clone();
            sched.register_operator(spec.name(), Arc::new(move |_cfg| s.instantiate()));
        }
        let (policy, inline_policy) = parse_policy(policy)?;
        let cancel = CancellationToken::new();
        let report: Arc<Mutex<Option<PyResult<serde_json::Value>>>> = Arc::new(Mutex::new(None));
        let src = vi_media::Source::parse(source);
        let report2 = report.clone();
        let cancel2 = cancel.clone();
        let handle = py.detach(|| {
            runtime().spawn(async move {
                let mut reports = Vec::new();
                let mut error: Option<String> = None;
                match sched.expand(&src).await {
                    Ok(sources) => {
                        for s in sources {
                            match sched
                                .run(
                                    s,
                                    JobOptions {
                                        policy: policy.clone(),
                                        force,
                                        inline_policy: inline_policy.clone(),
                                        ..JobOptions::default()
                                    },
                                    cancel2.clone(),
                                )
                                .await
                            {
                                Ok(r) => reports.push(r),
                                Err(e) => {
                                    error = Some(e.to_string());
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => error = Some(e.to_string()),
                }
                let mut value = serde_json::json!({"reports": reports, "error": error});
                flatten_timestamps(&mut value);
                if let Ok(mut r) = report2.lock() {
                    *r = Some(Ok(value));
                }
            })
        });
        Ok(Job {
            events: Mutex::new(Some(rx)),
            report,
            handle: Mutex::new(Some(handle)),
            cancel,
        })
    }

    /// Register a Python operator for this index's jobs. The object needs
    /// `id`, `inputs`, `outputs` and `run(ctx, item)`; optional `version`,
    /// `optional_inputs`, `params`, `finish(ctx)`. Policies then name it
    /// like a built-in operator.
    fn register_operator(&self, op: &Bound<'_, PyAny>) -> PyResult<()> {
        let spec = PyOperatorSpec::from_object(op)?;
        let mut ops = self.operators.lock().unwrap_or_else(|p| p.into_inner());
        ops.retain(|o| o.name() != spec.name());
        ops.push(spec);
        Ok(())
    }

    /// Names of registered Python operators.
    fn operators(&self) -> Vec<String> {
        self.operator_specs()
            .iter()
            .map(|s| s.name().to_string())
            .collect()
    }

    /// Hybrid search. Returns hit dicts.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (query, k = 10, videos = None, kinds = None, text_only = false, per_video_k = None))]
    fn search<'py>(
        &self,
        py: Python<'py>,
        query: &str,
        k: usize,
        videos: Option<Vec<String>>,
        kinds: Option<Vec<String>>,
        text_only: bool,
        per_video_k: Option<usize>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let videos = parse_videos(videos)?;
        let kinds: Vec<Kind> = kinds
            .unwrap_or_default()
            .iter()
            .map(|k| match k.as_str() {
                "transcript" => Ok(Kind::Transcript),
                "ocr" => Ok(Kind::Ocr),
                "description" => Ok(Kind::Description),
                "frame" => Ok(Kind::Frame),
                other => Err(PyValueError::new_err(format!("unknown kind '{other}'"))),
            })
            .collect::<PyResult<_>>()?;
        let req = vi_query::SearchRequest {
            query: query.to_string(),
            videos,
            kinds,
            k,
            text_only,
            per_video_k,
        };
        let storage = self.storage.clone();
        let providers = self.prov();
        let resp = py
            .detach(|| {
                runtime().block_on(vi_query::search(storage.as_ref(), Some(&providers), &req))
            })
            .map_err(err)?;
        json_py(py, &resp.hits)
    }

    /// Ask a question; iterate the returned stream for events.
    #[pyo3(signature = (question, budget = None, videos = None, session_id = None, policy = None, model = None))]
    fn ask(
        &self,
        question: &str,
        budget: Option<Budget>,
        videos: Option<Vec<String>>,
        session_id: Option<String>,
        policy: Option<&Bound<'_, PyAny>>,
        model: Option<String>,
    ) -> PyResult<AskStream> {
        self.ask_stream(
            question.to_string(),
            budget.as_ref(),
            videos,
            session_id,
            policy,
            model,
        )
    }

    /// Ask a question; `async for ev in idx.aask(...)`.
    #[pyo3(signature = (question, budget = None, videos = None, session_id = None, policy = None, model = None))]
    fn aask(
        &self,
        question: &str,
        budget: Option<Budget>,
        videos: Option<Vec<String>>,
        session_id: Option<String>,
        policy: Option<&Bound<'_, PyAny>>,
        model: Option<String>,
    ) -> PyResult<AskStream> {
        self.ask_stream(
            question.to_string(),
            budget.as_ref(),
            videos,
            session_id,
            policy,
            model,
        )
    }

    /// Labelled frame grid of a time range as PNG bytes.
    #[pyo3(signature = (video_id, t0, t1, fps = 1.0, cols = 3))]
    fn view<'py>(
        &self,
        py: Python<'py>,
        video_id: &str,
        t0: f64,
        t1: f64,
        fps: f64,
        cols: u32,
    ) -> PyResult<Bound<'py, PyAny>> {
        let vid = VideoId::parse(video_id).map_err(|_| PyValueError::new_err("bad video id"))?;
        let storage = self.storage.clone();
        let cfg = self.cfg();
        let view = py
            .detach(|| {
                runtime().block_on(async move {
                    let video = storage
                        .get_video(vid)
                        .await
                        .map_err(vi_core::Error::from)?
                        .ok_or_else(|| vi_core::Error::NotFound(format!("video {vid}")))?;
                    vi_agent::render_view(
                        &cfg.media.worker,
                        &video,
                        vi_agent::ViewRequest {
                            t0,
                            t1,
                            fps,
                            cols,
                            ..vi_agent::ViewRequest::default()
                        },
                    )
                    .await
                })
            })
            .map_err(err)?;
        let d = PyDict::new(py);
        d.set_item("png", PyBytes::new(py, &view.png))?;
        d.set_item("width", view.width)?;
        d.set_item("height", view.height)?;
        d.set_item("timestamps", view.timestamps)?;
        d.set_item("distinct", view.distinct)?;
        Ok(d.into_any())
    }

    /// One decoded frame at time `t` as an HxWx3 uint8 NumPy array.
    #[pyo3(signature = (video_id, t, max_dim = 640))]
    fn frame<'py>(
        &self,
        py: Python<'py>,
        video_id: &str,
        t: f64,
        max_dim: u32,
    ) -> PyResult<Bound<'py, PyAny>> {
        let vid = VideoId::parse(video_id).map_err(|_| PyValueError::new_err("bad video id"))?;
        let storage = self.storage.clone();
        let cfg = self.cfg();
        let frame = py
            .detach(|| {
                runtime().block_on(async move {
                    let video = storage
                        .get_video(vid)
                        .await
                        .map_err(vi_core::Error::from)?
                        .ok_or_else(|| vi_core::Error::NotFound(format!("video {vid}")))?;
                    let path = vi_agent::view::media_path(&video)
                        .filter(|p| p.is_file())
                        .ok_or_else(|| {
                            vi_core::Error::NotFound("media file not on this machine".into())
                        })?;
                    let req = vi_media::VideoDecodeRequest::new(&path, 2.0, max_dim)
                        .range(t.max(0.0), Some(t.max(0.0) + 2.0));
                    let mut s = vi_media::decode_video(&cfg.media.worker, req).await?;
                    let f = s
                        .next()
                        .await?
                        .ok_or_else(|| vi_core::Error::media("no frame decoded"))?;
                    Ok::<_, vi_core::Error>(f.to_owned_frame())
                })
            })
            .map_err(err)?;
        let (w, h) = (frame.width as usize, frame.height as usize);
        let mut data = Vec::with_capacity(w * h * 3);
        for y in 0..frame.height {
            data.extend_from_slice(frame.row(y));
        }
        let arr = numpy::PyArray1::from_vec(py, data);
        let arr = arr.reshape([h, w, 3]).map_err(err)?;
        Ok(arr.into_any())
    }

    /// Segments of a video at a level: `chapter`, `scene` or `shot`.
    #[pyo3(signature = (video_id, level = "chapter"))]
    fn timeline<'py>(
        &self,
        py: Python<'py>,
        video_id: &str,
        level: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let vid = VideoId::parse(video_id).map_err(|_| PyValueError::new_err("bad video id"))?;
        let lvl = vi_core::model::SegmentLevel::parse(level)
            .ok_or_else(|| PyValueError::new_err("level must be chapter, scene or shot"))?;
        let storage = self.storage.clone();
        let segs = py
            .detach(|| runtime().block_on(storage.segments(vid, lvl)))
            .map_err(err)?;
        json_py(py, &segs)
    }

    /// Videos in the index.
    fn videos<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let storage = self.storage.clone();
        let v = py
            .detach(|| runtime().block_on(storage.list_videos()))
            .map_err(err)?;
        json_py(py, &v)
    }

    /// Sizes, counts and jobs (what `vidx status` prints).
    fn status<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let storage = self.storage.clone();
        let s = py
            .detach(|| runtime().block_on(storage.stats()))
            .map_err(err)?;
        json_py(py, &s)
    }

    fn __repr__(&self) -> String {
        format!("Index({:?})", self.path)
    }
}

/// Replace a built-in prompt for this process.
#[pyfunction]
fn override_prompt(name: &str, text: &str) {
    vi_providers::prompts::override_prompt(name, text);
}

/// Core version.
#[pyfunction]
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Route the core's tracing to stderr at the level of RUST_LOG (default warn).
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .try_init();
    pyo3_async_runtimes::tokio::init_with_runtime(runtime())
        .map_err(|_| err("async runtime already initialised"))?;
    m.add_class::<Config>()?;
    m.add_class::<Budget>()?;
    m.add_class::<Index>()?;
    m.add_class::<Job>()?;
    m.add_class::<JobEvents>()?;
    m.add_class::<AskStream>()?;
    m.add_function(wrap_pyfunction!(override_prompt, m)?)?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    Ok(())
}
