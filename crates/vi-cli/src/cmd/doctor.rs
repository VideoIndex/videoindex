//! `vi doctor`: machine facts and toolchain checks.

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
    detail: String,
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

    let gpu = match which::which("nvidia-smi") {
        Ok(_) => match Command::new("nvidia-smi")
            .args([
                "--query-gpu=name,memory.total,driver_version",
                "--format=csv,noheader",
            ])
            .output()
        {
            Ok(o) if o.status.success() => Gpu {
                present: true,
                detail: String::from_utf8_lossy(&o.stdout).trim().to_string(),
            },
            Ok(o) => Gpu {
                present: false,
                detail: format!(
                    "nvidia-smi present but failed: {}",
                    String::from_utf8_lossy(&o.stderr).trim()
                ),
            },
            Err(e) => Gpu {
                present: false,
                detail: format!("nvidia-smi failed to run: {e}"),
            },
        },
        Err(_) => Gpu {
            present: false,
            detail: "none (nvidia-smi not installed)".into(),
        },
    };

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
    let incoming_files = std::fs::read_dir(incoming).ok().map(|rd| {
        rd.filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|x| x == "mp4" || x == "mkv" || x == "webm")
            })
            .count() as u64
    });

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
    };

    out.emit(&report, || {
        let mut s = String::new();
        s.push_str(&format!("vi {}\n", report.vi_version));
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
        s.push_str("youtube: yt-dlp is blocked from datacenter IPs; transfer videos with sidecars into the incoming dir (docs/11-deployment.md)");
        s.trim_end().to_string()
    });
    if args.show_config {
        println!("\n# effective configuration\n{}", config.to_toml()?);
    }
    Ok(())
}

fn tool(name: &str) -> Tool {
    let path = which::which(name).ok();
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
        _ => line.to_string(),
    }
}
