//! ONNX Runtime sessions: loading with the right execution provider,
//! thread settings, and clear errors when a model file is missing.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ort::session::builder::GraphOptimizationLevel;
use ort::session::{Session, SessionInputs, SessionOutputs};
use ort::value::Tensor;

use crate::PerceiveError;

/// Where inference runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    /// CPU execution provider.
    Cpu,
    /// CUDA execution provider.
    Cuda,
}

impl Device {
    /// Name for reports.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Cuda => "cuda",
        }
    }
}

/// Resolve the configured device string. `auto` picks CUDA when this build
/// has the `cuda` feature and the runtime reports it usable.
pub fn resolve_device(requested: &str) -> Result<Device, PerceiveError> {
    match requested {
        "cpu" => Ok(Device::Cpu),
        "cuda" => {
            if cuda_available() {
                Ok(Device::Cuda)
            } else {
                Err(PerceiveError::Onnx(
                    "CUDA execution provider requested but unavailable (build with `--features cuda` and install libcudart/cuBLAS/cuDNN)".into(),
                ))
            }
        }
        "auto" | "" => Ok(if cuda_available() {
            Device::Cuda
        } else {
            Device::Cpu
        }),
        other => Err(PerceiveError::Onnx(format!(
            "unknown device '{other}'; expected auto, cpu, or cuda"
        ))),
    }
}

/// Whether the CUDA execution provider can be used by this build.
pub fn cuda_available() -> bool {
    #[cfg(feature = "cuda")]
    {
        use ort::ep::ExecutionProvider;
        ort::ep::CUDA::default().is_available().unwrap_or(false)
    }
    #[cfg(not(feature = "cuda"))]
    {
        false
    }
}

/// The ONNX Runtime API version this binary was built against.
pub fn runtime_version() -> String {
    format!("ONNX Runtime API {}", ort::sys::ORT_API_VERSION)
}

/// A session plus the facts operators need about it. `run` takes `&self`
/// and serialises calls; batch inside one call for throughput.
pub struct OnnxSession {
    session: Mutex<Session>,
    path: PathBuf,
    device: Device,
    inputs: Vec<String>,
    outputs: Vec<String>,
}

impl std::fmt::Debug for OnnxSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnnxSession")
            .field("path", &self.path)
            .field("device", &self.device)
            .field("inputs", &self.inputs)
            .field("outputs", &self.outputs)
            .finish()
    }
}

/// Threads for one CPU session when the config says 0.
pub fn default_threads() -> usize {
    (std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2)
        / 2)
    .max(1)
}

impl OnnxSession {
    /// Load a model. `threads` of 0 means [`default_threads`].
    pub fn load(
        path: &Path,
        device: Device,
        threads: usize,
        hint: &str,
    ) -> Result<Self, PerceiveError> {
        if !path.is_file() {
            return Err(PerceiveError::ModelMissing {
                path: path.display().to_string(),
                hint: hint.to_string(),
            });
        }
        let threads = if threads == 0 {
            default_threads()
        } else {
            threads
        };
        let mut builder = Session::builder()
            .map_err(|e| PerceiveError::Onnx(e.to_string()))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| PerceiveError::Onnx(e.to_string()))?
            .with_intra_threads(threads)
            .map_err(|e| PerceiveError::Onnx(e.to_string()))?;
        #[cfg(feature = "cuda")]
        if device == Device::Cuda {
            builder = builder
                .with_execution_providers([ort::ep::CUDA::default().build().error_on_failure()])
                .map_err(|e| PerceiveError::Onnx(e.to_string()))?;
        }
        #[cfg(not(feature = "cuda"))]
        if device == Device::Cuda {
            return Err(PerceiveError::Onnx(
                "this build has no CUDA execution provider".into(),
            ));
        }
        let session = builder
            .commit_from_file(path)
            .map_err(|e| PerceiveError::Onnx(format!("{}: {e}", path.display())))?;
        let inputs = session
            .inputs()
            .iter()
            .map(|i| i.name().to_string())
            .collect();
        let outputs = session
            .outputs()
            .iter()
            .map(|o| o.name().to_string())
            .collect();
        tracing::info!(model = %path.display(), device = device.as_str(), threads, "loaded ONNX model");
        Ok(Self {
            session: Mutex::new(session),
            path: path.to_path_buf(),
            device,
            inputs,
            outputs,
        })
    }

    /// Model file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Device in use.
    pub fn device(&self) -> Device {
        self.device
    }

    /// Input names in model order.
    pub fn input_names(&self) -> &[String] {
        &self.inputs
    }

    /// Output names in model order.
    pub fn output_names(&self) -> &[String] {
        &self.outputs
    }

    /// Run the model. The closure receives the outputs while the session
    /// lock is held; copy out what you need.
    pub fn run<'i, 'v: 'i, const N: usize, T>(
        &self,
        inputs: impl Into<SessionInputs<'i, 'v, N>>,
        f: impl FnOnce(&SessionOutputs<'_>) -> Result<T, PerceiveError>,
    ) -> Result<T, PerceiveError> {
        let mut s = self
            .session
            .lock()
            .map_err(|_| PerceiveError::Onnx("session mutex poisoned".into()))?;
        let out = s
            .run(inputs)
            .map_err(|e| PerceiveError::Onnx(format!("{}: {e}", self.path.display())))?;
        f(&out)
    }
}

/// Build an `f32` tensor from a shape and data.
pub fn tensor_f32(shape: &[usize], data: Vec<f32>) -> Result<Tensor<f32>, PerceiveError> {
    Tensor::from_array((shape.to_vec(), data)).map_err(|e| PerceiveError::Onnx(e.to_string()))
}

/// Build an `i64` tensor from a shape and data.
pub fn tensor_i64(shape: &[usize], data: Vec<i64>) -> Result<Tensor<i64>, PerceiveError> {
    Tensor::from_array((shape.to_vec(), data)).map_err(|e| PerceiveError::Onnx(e.to_string()))
}

/// Read an `f32` output tensor into an owned vector with its shape.
pub fn extract_f32(
    outputs: &SessionOutputs<'_>,
    name: &str,
) -> Result<(Vec<usize>, Vec<f32>), PerceiveError> {
    let v = outputs
        .get(name)
        .ok_or_else(|| PerceiveError::Onnx(format!("model has no output '{name}'")))?;
    let (shape, data) = v
        .try_extract_tensor::<f32>()
        .map_err(|e| PerceiveError::Onnx(format!("output '{name}': {e}")))?;
    Ok((
        shape.iter().map(|d| (*d).max(0) as usize).collect(),
        data.to_vec(),
    ))
}
