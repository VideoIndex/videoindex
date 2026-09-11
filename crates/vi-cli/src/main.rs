//! The `vi` binary. See `docs/09-sdk-and-apis.md` for the command surface.
//! M0 shipped `init`, `probe`, `index`, `status`, `doctor`; M1 adds `search`.

#![cfg_attr(test, allow(clippy::unwrap_used))]

mod cmd;
mod output;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

/// VideoIndex: turn long videos into a queryable knowledge base.
#[derive(Debug, Parser)]
#[command(name = "vi", version, about, long_about = None)]
struct Cli {
    /// Config file (default: $VI_CONFIG or built-in defaults). `VI_*`
    /// environment variables override individual keys.
    #[arg(long, global = true, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Machine-readable JSON output.
    #[arg(long, global = true)]
    json: bool,

    /// Log filter, e.g. `debug` or `vi_media=trace` (default from config).
    #[arg(long, global = true, value_name = "FILTER")]
    log: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create an empty index directory.
    Init(cmd::init::Args),
    /// Probe a media file: container, streams, chapters, keyframe interval.
    Probe(cmd::probe::Args),
    /// Index one or more local video files into an index directory.
    Index(cmd::index::Args),
    /// Full-text search over transcripts, on-screen text and descriptions.
    Search(cmd::search::Args),
    /// Show videos, states, sample counts, sizes, and jobs of an index.
    Status(cmd::status::Args),
    /// Report machine facts: CPU, RAM, GPU, disks, toolchain, network paths.
    Doctor(cmd::doctor::Args),
}

fn main() {
    // The decode worker re-executes this binary with a hidden first argument;
    // dispatch before clap sees anything.
    if std::env::args().nth(1).as_deref() == Some(vi_media::WORKER_ARG) {
        std::process::exit(vi_media::worker::main());
    }
    let code = match run() {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("error: {e:#}");
            exit_code(&e)
        }
    };
    std::process::exit(code);
}

/// Stable exit codes: 1 generic, 2 usage/invalid, 3 not found, 4 media,
/// 5 storage, 6 unsupported, 7 cancelled/timeout.
fn exit_code(e: &anyhow::Error) -> i32 {
    if let Some(core) = e.downcast_ref::<vi_core::Error>() {
        return core_exit_code(core);
    }
    if let Some(m) = e.downcast_ref::<vi_media::MediaError>() {
        return match m {
            vi_media::MediaError::Invalid(_) => 2,
            vi_media::MediaError::Cancelled | vi_media::MediaError::Timeout(_) => 7,
            _ => 4,
        };
    }
    if let Some(i) = e.downcast_ref::<vi_index::IndexError>() {
        return match i {
            vi_index::IndexError::Invalid(_) => 2,
            vi_index::IndexError::NotAnIndex(_) => 3,
            vi_index::IndexError::Unsupported(_) => 6,
            _ => 5,
        };
    }
    1
}

fn core_exit_code(e: &vi_core::Error) -> i32 {
    match e {
        vi_core::Error::Invalid(_) | vi_core::Error::Config(_) => 2,
        vi_core::Error::NotFound(_) => 3,
        vi_core::Error::Media(_) => 4,
        vi_core::Error::Storage(_) | vi_core::Error::SchemaTooNew { .. } => 5,
        vi_core::Error::Unsupported(_) => 6,
        vi_core::Error::Cancelled | vi_core::Error::Timeout(_) => 7,
        _ => 1,
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let config = vi_core::Config::load(cli.config.as_deref())?;
    init_logging(
        cli.log.as_deref().unwrap_or(&config.log.level),
        config.log.json || cli.json,
    );
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let out = output::Output::new(cli.json);
    rt.block_on(async move {
        match cli.command {
            Command::Init(a) => cmd::init::run(a, &config, &out).await,
            Command::Probe(a) => cmd::probe::run(a, &config, &out).await,
            Command::Index(a) => cmd::index::run(a, config, &out).await,
            Command::Search(a) => cmd::search::run(a, &config, &out).await,
            Command::Status(a) => cmd::status::run(a, &config, &out).await,
            Command::Doctor(a) => cmd::doctor::run(a, &config, &out).await,
        }
    })
}

fn init_logging(filter: &str, json: bool) {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_new(filter).unwrap_or_else(|_| EnvFilter::new("info"));
    let builder = fmt().with_env_filter(filter).with_writer(std::io::stderr);
    if json {
        let _ = builder.json().try_init();
    } else {
        let _ = builder.with_target(false).try_init();
    }
}
