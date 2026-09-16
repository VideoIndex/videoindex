//! Python-implemented operators and agent policies. A `PyOperator` stands
//! in for the Rust `Operator` trait: it hands each item to a Python object
//! as a dict (frames as NumPy arrays), and stores whatever rows come back
//! through `vi_pipeline::callback`. The GIL is held only for the call, which
//! runs on a blocking thread so a slow Python function never stalls the
//! runtime.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use numpy::PyArrayMethods;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple};
use serde_json::{json, Value};
use vi_agent::policy::{Policy, PolicyState, Step};
use vi_agent::tools::ToolCall;
use vi_core::model::Provenance;
use vi_core::{Error, Result};
use vi_pipeline::callback::{
    item_to_json, parse_kind, provenance, store_rows, CallbackRow, RowTarget,
};
use vi_pipeline::{
    CostEstimate, InputSummary, Item, ItemKind, OpContext, OpInput, OpOutput, Operator,
};

/// Python objects to JSON. Dicts need string keys; tuples become arrays;
/// anything else goes through `str()`.
pub fn py_to_json(obj: &Bound<'_, PyAny>) -> PyResult<Value> {
    if obj.is_none() {
        return Ok(Value::Null);
    }
    if let Ok(b) = obj.cast::<PyBool>() {
        return Ok(Value::Bool(b.is_true()));
    }
    if let Ok(i) = obj.cast::<PyInt>() {
        if let Ok(v) = i.extract::<i64>() {
            return Ok(json!(v));
        }
        return Ok(json!(i.extract::<u64>()?));
    }
    if let Ok(f) = obj.cast::<PyFloat>() {
        return Ok(json!(f.value()));
    }
    if let Ok(s) = obj.cast::<PyString>() {
        return Ok(Value::String(s.to_str()?.to_string()));
    }
    if let Ok(d) = obj.cast::<PyDict>() {
        let mut m = serde_json::Map::new();
        for (k, v) in d.iter() {
            m.insert(k.str()?.to_str()?.to_string(), py_to_json(&v)?);
        }
        return Ok(Value::Object(m));
    }
    if let Ok(l) = obj.cast::<PyList>() {
        return l
            .iter()
            .map(|x| py_to_json(&x))
            .collect::<PyResult<Vec<_>>>()
            .map(Value::Array);
    }
    if let Ok(t) = obj.cast::<PyTuple>() {
        return t
            .iter()
            .map(|x| py_to_json(&x))
            .collect::<PyResult<Vec<_>>>()
            .map(Value::Array);
    }
    // NumPy scalars and other numerics.
    if let Ok(f) = obj.extract::<f64>() {
        return Ok(json!(f));
    }
    Ok(Value::String(obj.str()?.to_str()?.to_string()))
}

fn kinds(obj: &Bound<'_, PyAny>, attr: &str, required: bool) -> PyResult<Vec<ItemKind>> {
    let names: Vec<String> = match obj.getattr(attr) {
        Ok(v) if !v.is_none() => v.extract()?,
        Ok(_) => Vec::new(),
        Err(e) if required => return Err(e),
        Err(_) => Vec::new(),
    };
    names
        .iter()
        .map(|n| {
            parse_kind(n).ok_or_else(|| {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "unknown item kind '{n}' in {attr}; use snake_case names such as 'hashed', 'transcript_span', 'shot', 'scene', 'description'"
                ))
            })
        })
        .collect()
}

/// What `register_operator` reads from the Python object once. Each job
/// gets its own `PyOperator` from `instantiate`, so per-job state (the media
/// item, the provenance row) never leaks between jobs.
pub struct PyOperatorSpec {
    obj: Py<PyAny>,
    id: &'static str,
    version: u32,
    inputs: Vec<ItemKind>,
    optional_inputs: Vec<ItemKind>,
    outputs: Vec<ItemKind>,
    params: Value,
    has_finish: bool,
}

impl PyOperatorSpec {
    /// Read the operator's declaration from its attributes.
    pub fn from_object(obj: &Bound<'_, PyAny>) -> PyResult<Arc<Self>> {
        let id: String = obj.getattr("id")?.extract()?;
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "operator id must be non-empty and use only [A-Za-z0-9_]",
            ));
        }
        let version: u32 = match obj.getattr("version") {
            Ok(v) if !v.is_none() => v.extract()?,
            _ => 1,
        };
        let mut inputs = kinds(obj, "inputs", true)?;
        let outputs = kinds(obj, "outputs", true)?;
        let optional_inputs = kinds(obj, "optional_inputs", false)?;
        // Optional inputs are inputs that may have no producer; the DAG reads
        // them from `inputs`, so declaring one only as optional still works.
        for k in &optional_inputs {
            if !inputs.contains(k) {
                inputs.push(*k);
            }
        }
        if inputs.is_empty() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "operator must declare at least one input kind",
            ));
        }
        let params = match obj.getattr("params") {
            Ok(p) if !p.is_none() => py_to_json(&p)?,
            _ => json!({}),
        };
        let has_finish = obj.hasattr("finish")?;
        if !obj.hasattr("run")? {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "operator needs a run(ctx, item) method",
            ));
        }
        // Operator ids are `&'static str` in the trait; an operator is
        // registered a handful of times per process, so leaking the name
        // is the simplest correct choice.
        let id: &'static str = Box::leak(id.into_boxed_str());
        Ok(Arc::new(Self {
            obj: obj.clone().unbind(),
            id,
            version,
            inputs,
            optional_inputs,
            outputs,
            params,
            has_finish,
        }))
    }

    /// The policy name this operator is registered under.
    pub fn name(&self) -> &'static str {
        self.id
    }

    /// A fresh operator for one job.
    pub fn instantiate(self: &Arc<Self>) -> Box<dyn Operator> {
        Box::new(PyOperator {
            spec: self.clone(),
            state: Mutex::new(None),
        })
    }
}

struct JobState {
    target: RowTarget,
    prov: Provenance,
}

/// One job's instance of a Python operator.
pub struct PyOperator {
    spec: Arc<PyOperatorSpec>,
    state: Mutex<Option<JobState>>,
}

/// The parts of the context a Python operator sees, copied out so the call
/// can run on a blocking thread without borrowing the `OpContext`.
struct CtxSnapshot {
    job: String,
    video: String,
    stage: String,
    sample_fps: f64,
    cost_usd: f64,
}

impl CtxSnapshot {
    fn of(ctx: &OpContext) -> Self {
        Self {
            job: ctx.job.to_string(),
            video: ctx.video.to_string(),
            stage: ctx.stage.clone(),
            sample_fps: ctx.policy.sample_fps,
            cost_usd: ctx.budget.spent_usd(),
        }
    }

    fn dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new(py);
        d.set_item("job", &self.job)?;
        d.set_item("video_id", &self.video)?;
        d.set_item("stage", &self.stage)?;
        d.set_item("sample_fps", self.sample_fps)?;
        d.set_item("cost_usd", self.cost_usd)?;
        Ok(d)
    }
}

impl PyOperator {
    /// The item as a dict; pixels and audio samples become NumPy arrays.
    fn item_dict<'py>(py: Python<'py>, item: &Item) -> PyResult<Bound<'py, PyDict>> {
        let v = item_to_json(item);
        let d = crate::to_py(py, &v)?;
        let d = d.cast_into::<PyDict>()?;
        match item {
            Item::Frame(f) => d.set_item("frame", frame_array(py, &f.frame)?)?,
            Item::Hashed { frame, .. } => d.set_item("frame", frame_array(py, frame)?)?,
            Item::SpeechRange(s) => {
                let arr = numpy::PyArray1::from_slice(py, &s.samples);
                d.set_item("samples", arr)?;
            }
            _ => {}
        }
        Ok(d)
    }

    /// Call `run` (with an item) or `finish` (without) under the GIL and
    /// parse the rows it returns.
    fn call(
        spec: &PyOperatorSpec,
        ctx: &CtxSnapshot,
        item: Option<Item>,
    ) -> std::result::Result<Vec<CallbackRow>, String> {
        Python::attach(|py| -> PyResult<Vec<CallbackRow>> {
            let obj = spec.obj.bind(py);
            let ctxd = ctx.dict(py)?;
            let out = match item {
                Some(item) => obj.call_method1("run", (ctxd, Self::item_dict(py, &item)?))?,
                None => obj.call_method1("finish", (ctxd,))?,
            };
            if out.is_none() {
                return Ok(Vec::new());
            }
            let v = py_to_json(&out)?;
            let rows: Vec<CallbackRow> = match v {
                Value::Array(_) => serde_json::from_value(v),
                Value::Object(_) => serde_json::from_value(v).map(|r| vec![r]),
                other => {
                    return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                    "operator {} must return None, a row dict or a list of row dicts, got {other}",
                    spec.id
                )))
                }
            }
            .map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "operator {} returned a row the core does not understand: {e}",
                    spec.id
                ))
            })?;
            Ok(rows)
        })
        .map_err(|e| Python::attach(|py| format_py_err(py, &e)))
    }

    async fn call_blocking(&self, ctx: &OpContext, item: Option<Item>) -> Result<OpOutput> {
        let spec = self.spec.clone();
        let snap = CtxSnapshot::of(ctx);
        let rows = tokio::task::spawn_blocking(move || Self::call(&spec, &snap, item))
            .await
            .map_err(|e| ctx.err(format!("python operator task failed: {e}")))?
            .map_err(|e| ctx.err(e))?;
        if rows.is_empty() {
            return Ok(OpOutput::default());
        }
        let (target, prov) = self.job_state(ctx).await?;
        let (emitted, stored) = store_rows(ctx, &target, &prov, &self.spec.outputs, rows).await?;
        Ok(OpOutput { emitted, stored })
    }

    /// The job's row target and provenance row, created on first use.
    async fn job_state(&self, ctx: &OpContext) -> Result<(RowTarget, Provenance)> {
        if let Some(st) = self
            .state
            .lock()
            .map_err(|_| ctx.err("operator state poisoned"))?
            .as_ref()
        {
            return Ok((st.target.clone(), st.prov.clone()));
        }
        let target = RowTarget::load(ctx).await?;
        let prov = provenance(self.spec.id, self.spec.version, self.spec.params.clone());
        ctx.storage.put_provenance(&prov).await?;
        let mut guard = self
            .state
            .lock()
            .map_err(|_| ctx.err("operator state poisoned"))?;
        let st = guard.get_or_insert(JobState { target, prov });
        Ok((st.target.clone(), st.prov.clone()))
    }
}

/// Python exception with its type name, and the traceback when there is one.
pub fn format_py_err(py: Python<'_>, e: &PyErr) -> String {
    let tb = e
        .traceback(py)
        .and_then(|t| t.format().ok())
        .unwrap_or_default();
    format!("{e}\n{tb}").trim_end().to_string()
}

/// HxWx3 uint8 for RGB frames, HxW for single-plane formats.
fn frame_array<'py>(py: Python<'py>, frame: &vi_media::FrameBuffer) -> PyResult<Bound<'py, PyAny>> {
    let (w, h) = (frame.width as usize, frame.height as usize);
    let channels = match frame.format {
        vi_media::PixelFormat::Rgb24 => 3,
        _ => 1,
    };
    let mut data = Vec::with_capacity(w * h * channels);
    for y in 0..frame.height {
        data.extend_from_slice(&frame.row(y)[..w * channels]);
    }
    let arr = numpy::PyArray1::from_vec(py, data);
    let arr = if channels == 3 {
        arr.reshape([h, w, 3])?.into_any()
    } else {
        arr.reshape([h, w])?.into_any()
    };
    Ok(arr)
}

#[async_trait]
impl Operator for PyOperator {
    fn id(&self) -> &'static str {
        self.spec.id
    }

    fn version(&self) -> u32 {
        self.spec.version
    }

    fn inputs(&self) -> &[ItemKind] {
        &self.spec.inputs
    }

    fn optional_inputs(&self) -> &[ItemKind] {
        &self.spec.optional_inputs
    }

    fn outputs(&self) -> &[ItemKind] {
        &self.spec.outputs
    }

    fn cost_estimate(&self, _input: &InputSummary) -> CostEstimate {
        CostEstimate::default()
    }

    fn cache_params(&self, _ctx: &OpContext) -> Value {
        json!({"python": true, "params": self.spec.params})
    }

    async fn run(&self, ctx: &OpContext, input: OpInput) -> Result<OpOutput> {
        self.call_blocking(ctx, Some(input.item)).await
    }

    async fn finish(&self, ctx: &OpContext) -> Result<OpOutput> {
        if !self.spec.has_finish {
            return Ok(OpOutput::default());
        }
        self.call_blocking(ctx, None).await
    }
}

/// A Python object deciding the agent's next step: `next_step(state)`
/// returns `None` (or `"answer"`) to answer, or `{"tool": name, "args": {...}}`.
pub struct PyPolicy {
    obj: Py<PyAny>,
    name: String,
    calls: std::sync::atomic::AtomicU64,
}

impl PyPolicy {
    /// Wrap a Python policy object.
    pub fn new(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        if !obj.hasattr("next_step")? {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "policy needs a next_step(state) method",
            ));
        }
        let name = match obj.getattr("name") {
            Ok(n) if !n.is_none() => n.extract::<String>()?,
            _ => obj.get_type().name()?.to_string(),
        };
        Ok(Self {
            obj: obj.clone().unbind(),
            name,
            calls: std::sync::atomic::AtomicU64::new(0),
        })
    }
}

#[async_trait]
impl Policy for PyPolicy {
    async fn next_step(&self, state: &PolicyState) -> Result<Step> {
        let obj = Python::attach(|py| self.obj.clone_ref(py));
        let state_json = json!({
            "question": state.question,
            "steps": state.steps.iter().map(|(call, result)| json!({
                "tool": call.name, "args": call.args, "result": result,
            })).collect::<Vec<_>>(),
            "tool_calls_left": state.tool_calls_left,
        });
        let n = self
            .calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let decision = tokio::task::spawn_blocking(move || {
            Python::attach(|py| -> PyResult<Value> {
                let st = crate::to_py(py, &state_json)?;
                let out = obj.bind(py).call_method1("next_step", (st,))?;
                py_to_json(&out)
            })
            .map_err(|e| Python::attach(|py| format_py_err(py, &e)))
        })
        .await
        .map_err(|e| Error::Other(format!("policy task failed: {e}")))?
        .map_err(|e| Error::Other(format!("python policy: {e}")))?;
        match decision {
            Value::Null => Ok(Step::Answer),
            Value::String(s) if s == "answer" => Ok(Step::Answer),
            Value::Object(m) if m.get("answer").and_then(Value::as_bool) == Some(true) => {
                Ok(Step::Answer)
            }
            Value::Object(mut m) => {
                let name = m
                    .remove("tool")
                    .and_then(|v| v.as_str().map(str::to_string))
                    .ok_or_else(|| {
                        Error::Other("python policy returned a dict without 'tool'".into())
                    })?;
                let args = m.remove("args").unwrap_or_else(|| json!({}));
                Ok(Step::Tool(ToolCall {
                    id: format!("policy_{n}"),
                    name,
                    args,
                    signature: None,
                }))
            }
            other => Err(Error::Other(format!(
                "python policy must return None, 'answer' or {{'tool': .., 'args': ..}}, got {other}"
            ))),
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}
