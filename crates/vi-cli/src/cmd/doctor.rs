//! `vidx doctor`: machine facts and toolchain checks.

use std::path::Path;
use std::process::Command;

use anyhow::Result;
use serde::Serialize;
use vi_core::Config;

use crate::output::{bytes, Output};

/// Arguments.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Also print the effective configuration as TOML.
    #[arg(long)]
    pub show_config: bool,
}

#[derive(Debug, Serialize)]
struct Tool {
    name: String,
    path: Option<String>,
    version: Option<String>,
}

#[derive(Debug, Serialize)]
struct Disk {
    mount: String,
    total: u64,
    available: u64,
}

#[derive(Debug, Serialize)]
struct Gpu {
    present: bool,
    /// One line per GPU: name, VRAM, driver.
    detail: String,
    count: u32,
    driver_version: Option<String>,
    /// CUDA version the driver supports, from the `nvidia-smi` banner.
    cuda_driver_version: Option<String>,
    /// `nvcc --version` release, when a CUDA toolkit is installed.
    cuda_toolkit_version: Option<String>,
    /// CUDA runtime libraries ONNX Runtime's CUDA execution provider needs
    /// (`libcudart`, `libcublas`, `libcudnn`), as found by the dynamic
    /// linker.
    cuda_libs: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Report {
    vi_version: String,
    os: String,
    kernel: String,
    hostname: String,
    arch: String,
    cpu_model: String,
    cpus: usize,
    physical_cores: Option<usize>,
    rayon_threads: usize,
    memory_total: u64,
    memory_available: u64,
    swap_total: u64,
    gpu: Gpu,
    disks: Vec<Disk>,
    data_dir: String,
    data_dir_exists: bool,
    media_cache_dir: String,
    incoming_dir: String,
    incoming_files: Option<u64>,
    tools: Vec<Tool>,
    libav: Option<String>,
    decode_worker: String,
    sandbox: String,
    hwaccel: Vec<String>,
    etc_videoindex_exists: bool,
    config_source: String,
    onnx_runtime: String,
    onnx_device: String,
    models_dir: String,
    models: Vec<ModelFile>,
    roles: Vec<vi_providers::registry::RoleReport>,
}

#[derive(Debug, Serialize)]
struct ModelFile {
    name: String,
    path: String,
    present: bool,
    bytes: u64,
}

pub async fn run(args: Args, config: &Config, out: &Output) -> Result<()> {
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_list(sysinfo::CpuRefreshKind::nothing());

    let cpu_model = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into());

    let tools = [
        "ffmpeg",
        "ffprobe",
        "yt-dlp",
        "python3",
        "node",
        "npm",
        "docker",
        "caddy",
        "nginx",
        "rustc",
        "cargo",
        "nvidia-smi",
        "deno",
    ]
    .iter()
    .map(|name| tool(name))
    .collect::<Vec<_>>();

    let gpu = detect_gpu();

    let disks = sysinfo::Disks::new_with_refreshed_list()
        .iter()
        .filter(|d| {
            let m = d.mount_point().to_string_lossy();
            !m.starts_with("/boot") && !m.starts_with("/snap") && d.total_space() > 0
        })
        .map(|d| Disk {
            mount: d.mount_point().to_string_lossy().to_string(),
            total: d.total_space(),
            available: d.available_space(),
        })
        .collect();

    let hwaccel = Command::new("ffmpeg")
        .args(["-hide_banner", "-hwaccels"])
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .skip(1)
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let (libav, decode_worker) = match vi_media::worker_info(&config.media.worker).await {
        Ok(info) => (Some(info.libav), info.executable.display().to_string()),
        Err(e) => (None, format!("failed: {e}")),
    };

    let incoming = Path::new("/data/videoindex/videos/incoming");
    let incoming_files = incoming.is_dir().then(|| count_videos(incoming, 0));

    let onnx_device = match vi_perceive::onnx::resolve_device(&config.models.device) {
        Ok(d) => format!(
            "{} (config: {}{})",
            d.as_str(),
            config.models.device,
            if cfg!(feature = "cuda") {
                ", cuda feature on"
            } else {
                ", built without the cuda feature"
            }
        ),
        Err(e) => format!("error: {e}"),
    };
    let models = [
        ("silero-vad", "silero-vad/silero_vad.onnx"),
        ("siglip vision", "siglip-base-patch16-224/vision_model.onnx"),
        ("siglip text", "siglip-base-patch16-224/text_model.onnx"),
        ("siglip tokenizer", "siglip-base-patch16-224/tokenizer.json"),
        ("bge-small", "bge-small-en-v1.5/model.onnx"),
        ("rapidocr det", "rapidocr/ch_PP-OCRv4_det_infer.onnx"),
        ("rapidocr rec (en)", "rapidocr/en_PP-OCRv3_rec_infer.onnx"),
        ("rapidocr dict (en)", "rapidocr/en_dict.txt"),
    ]
    .iter()
    .map(|(name, rel)| {
        let path = config.models.dir.join(rel);
        let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        ModelFile {
            name: name.to_string(),
            path: path.display().to_string(),
            present: path.is_file(),
            bytes,
        }
    })
    .collect();
    let registry = vi_providers::ProviderRegistry::new(
        std::sync::Arc::new(config.clone()),
        tokio_util::sync::CancellationToken::new(),
    );
    let roles = registry.report();

    let report = Report {
        vi_version: env!("CARGO_PKG_VERSION").into(),
        os: format!(
            "{} {}",
            System::name().unwrap_or_default(),
            System::os_version().unwrap_or_default()
        ),
        kernel: System::kernel_version().unwrap_or_default(),
        hostname: System::host_name().unwrap_or_default(),
        arch: std::env::consts::ARCH.into(),
        cpu_model,
        cpus: std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1),
        physical_cores: System::physical_core_count(),
        rayon_threads: vi_core::cpu::threads(),
        memory_total: sys.total_memory(),
        memory_available: sys.available_memory(),
        swap_total: sys.total_swap(),
        gpu,
        disks,
        data_dir: "/data/videoindex".into(),
        data_dir_exists: Path::new("/data/videoindex").is_dir(),
        media_cache_dir: config.media.cache_dir.display().to_string(),
        incoming_dir: incoming.display().to_string(),
        incoming_files,
        tools,
        libav,
        decode_worker,
        sandbox: vi_media::sandbox::describe().into(),
        hwaccel,
        etc_videoindex_exists: Path::new("/etc/videoindex").is_dir(),
        config_source: std::env::var("VI_CONFIG").unwrap_or_else(|_| "defaults + VI_* env".into()),
        onnx_runtime: vi_perceive::onnx::runtime_version(),
        onnx_device,
        models_dir: config.models.dir.display().to_string(),
        models,
        roles,
    };

    out.emit(&report, || {
        let mut s = String::new();
        s.push_str(&format!("vidx {}\n", report.vi_version));
        s.push_str(&format!("host: {} ({} {}, kernel {}, {})\n", report.hostname, report.os, report.arch, report.kernel, report.config_source));
        s.push_str(&format!(
            "cpu: {}  {} logical{}  rayon threads {}\n",
            report.cpu_model,
            report.cpus,
            report.physical_cores.map(|p| format!(", {p} physical")).unwrap_or_default(),
            report.rayon_threads
        ));
        s.push_str(&format!(
            "memory: {} total, {} available, swap {}\n",
            bytes(report.memory_total),
            bytes(report.memory_available),
            bytes(report.swap_total)
        ));
        s.push_str(&format!("gpu: {}\n", report.gpu.detail));
        if report.gpu.present {
            s.push_str(&format!(
                "  count {}  driver {}  cuda (driver) {}  cuda toolkit {}  cuda libs: {}\n",
                report.gpu.count,
                report.gpu.driver_version.as_deref().unwrap_or("?"),
                report.gpu.cuda_driver_version.as_deref().unwrap_or("?"),
                report.gpu.cuda_toolkit_version.as_deref().unwrap_or("missing"),
                if report.gpu.cuda_libs.is_empty() { "none (ONNX Runtime CUDA EP unavailable)".to_string() } else { report.gpu.cuda_libs.join(" ") }
            ));
        }
        s.push_str("disks:\n");
        for d in &report.disks {
            s.push_str(&format!("  {:<16} {} free of {}\n", d.mount, bytes(d.available), bytes(d.total)));
        }
        s.push_str(&format!(
            "data dir: {} ({})\n",
            report.data_dir,
            if report.data_dir_exists { "exists" } else { "missing" }
        ));
        s.push_str(&format!("media cache: {}\n", report.media_cache_dir));
        s.push_str(&format!(
            "incoming: {} ({})\n",
            report.incoming_dir,
            match report.incoming_files {
                Some(n) => format!("{n} video files"),
                None => "missing".into(),
            }
        ));
        s.push_str(&format!(
            "/etc/videoindex: {}\n",
            if report.etc_videoindex_exists { "exists" } else { "missing" }
        ));
        s.push_str("tools:\n");
        for t in &report.tools {
            match (&t.path, &t.version) {
                (Some(p), Some(v)) => s.push_str(&format!("  {:<11} {v}  ({p})\n", t.name)),
                (Some(p), None) => s.push_str(&format!("  {:<11} present ({p})\n", t.name)),
                _ => s.push_str(&format!("  {:<11} missing\n", t.name)),
            }
        }
        s.push_str(&format!(
            "libav (via decode worker): {}\n",
            report.libav.as_deref().unwrap_or("worker failed to start")
        ));
        s.push_str(&format!("decode worker: {}\n", report.decode_worker));
        s.push_str(&format!("sandbox: {}\n", report.sandbox));
        s.push_str(&format!(
            "ffmpeg hwaccel methods: {}\n",
            if report.hwaccel.is_empty() { "none".to_string() } else { report.hwaccel.join(" ") }
        ));
        s.push_str(&format!("onnx runtime: {}; device {}\n", report.onnx_runtime, report.onnx_device));
        s.push_str(&format!("models dir: {}\n", report.models_dir));
        for m in &report.models {
            if m.present {
                s.push_str(&format!("  {:<20} {}\n", m.name, bytes(m.bytes)));
            } else {
                s.push_str(&format!("  {:<20} MISSING ({})\n", m.name, m.path));
            }
        }
        if report.roles.is_empty() {
            s.push_str("provider roles: none configured (add [providers] and [roles]; see config/gcp-a100.toml)\n");
        } else {
            s.push_str("provider roles:\n");
            for r in &report.roles {
                s.push_str(&format!(
                    "  {:<13} {} ({}{}{}){}\n",
                    r.role,
                    r.provider,
                    r.adapter,
                    r.model.as_deref().map(|m| format!(", {m}")).unwrap_or_default(),
                    r.base_url.as_deref().map(|u| format!(", {u}")).unwrap_or_default(),
                    r.problem.as_deref().map(|p| format!("  PROBLEM: {p}")).unwrap_or_default()
                ));
            }
        }
        s.push_str("youtube: yt-dlp is blocked from datacenter IPs; transfer videos with sidecars into the incoming dir, or run yt-dlp with a JS runtime and retries (see eval/README.md)");
        s.trim_end().to_string()
    });
    if args.show_config {
        println!("\n# effective configuration\n{}", config.to_toml()?);
    }
    Ok(())
}

fn tool(name: &str) -> Tool {
    let path = which::which(name).ok().or_else(|| {
        // rustup installs into ~/.cargo/bin, which non-login shells may
        // not have on PATH.
        let home = std::env::var_os("HOME")?;
        let p = Path::new(&home).join(".cargo").join("bin").join(name);
        p.is_file().then_some(p)
    });
    let version = path.as_ref().and_then(|p| {
        let args: &[&str] = match name {
            "ffmpeg" | "ffprobe" => &["-version"],
            "nginx" => &["-v"],
            "caddy" => &["version"],
            _ => &["--version"],
        };
        let o = Command::new(p).args(args).output().ok()?;
        let text = if o.stdout.is_empty() {
            o.stderr
        } else {
            o.stdout
        };
        let first = String::from_utf8_lossy(&text)
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        (!first.is_empty()).then_some(shorten_version(name, &first))
    });
    Tool {
        name: name.into(),
        path: path.map(|p| p.display().to_string()),
        version,
    }
}

fn shorten_version(name: &str, line: &str) -> String {
    match name {
        "ffmpeg" | "ffprobe" => line.split_whitespace().nth(2).unwrap_or(line).to_string(),
        "nginx" => line.rsplit('/').next().unwrap_or(line).to_string(),
        "docker" => line
            .trim_start_matches("Docker version ")
            .split(',')
            .next()
            .unwrap_or(line)
            .to_string(),
        "nvidia-smi" => line.rsplit(':').next().unwrap_or(line).trim().to_string(),
        _ => line.to_string(),
    }
}

/// Count video files under `dir`, descending into subdirectories (playlists
/// arrive as one directory each).
fn count_videos(dir: &Path, depth: u32) -> u64 {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut n = 0;
    for e in rd.filter_map(|e| e.ok()) {
        let p = e.path();
        if p.is_dir() {
            if depth < 3 {
                n += count_videos(&p, depth + 1);
            }
        } else if vi_media::acquire::is_video_file(&p) {
            n += 1;
        }
    }
    n
}

fn detect_gpu() -> Gpu {
    let none = |detail: String| Gpu {
        present: false,
        detail,
        count: 0,
        driver_version: None,
        cuda_driver_version: None,
        cuda_toolkit_version: None,
        cuda_libs: Vec::new(),
    };
    if which::which("nvidia-smi").is_err() {
        return none("none (nvidia-smi not installed)".into());
    }
    let query = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total,driver_version",
            "--format=csv,noheader",
        ])
        .output();
    let rows: Vec<String> = match query {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        Ok(o) => {
            return none(format!(
                "nvidia-smi present but failed: {}",
                String::from_utf8_lossy(&o.stderr).trim()
            ))
        }
        Err(e) => return none(format!("nvidia-smi failed to run: {e}")),
    };
    if rows.is_empty() {
        return none("nvidia-smi reports no GPU".into());
    }
    let driver_version = rows[0].rsplit(',').next().map(|s| s.trim().to_string());
    // The banner is the only place nvidia-smi prints the CUDA version the
    // driver supports.
    let cuda_driver_version = Command::new("nvidia-smi").output().ok().and_then(|o| {
        let text = String::from_utf8_lossy(&o.stdout).to_string();
        let i = text.find("CUDA Version:")?;
        let rest = &text[i + "CUDA Version:".len()..];
        let v: String = rest
            .trim_start()
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        (!v.is_empty()).then_some(v)
    });
    let cuda_toolkit_version = ["nvcc", "/usr/local/cuda/bin/nvcc"]
        .iter()
        .find_map(|c| Command::new(c).arg("--version").output().ok())
        .and_then(|o| {
            let text = String::from_utf8_lossy(&o.stdout).to_string();
            let i = text.find("release ")?;
            let v: String = text[i + "release ".len()..]
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            (!v.is_empty()).then_some(v)
        });
    let cuda_libs = ["libcudart.so", "libcublas.so", "libcudnn.so"]
        .iter()
        .filter(|lib| ldconfig_has(lib))
        .map(|s| s.to_string())
        .collect();
    Gpu {
        present: true,
        detail: rows.join("; "),
        count: rows.len() as u32,
        driver_version,
        cuda_driver_version,
        cuda_toolkit_version,
        cuda_libs,
    }
}

/// Whether the dynamic linker cache lists a library (Linux only).
fn ldconfig_has(lib: &str) -> bool {
    if !cfg!(target_os = "linux") {
        return false;
    }
    Command::new("ldconfig")
        .arg("-p")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains(lib))
        .unwrap_or(false)
}
