//! `vi serve [--bind ADDR] [--index-root DIR] [--api-key KEY]... [--no-mcp]`
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use vi_core::Config;

use crate::output::Output;

/// Arguments.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Bind address (config `server.bind`).
    #[arg(long)]
    pub bind: Option<String>,
    /// Directory holding `<id>.vidx` indexes (config `server.index_root`).
    #[arg(long)]
    pub index_root: Option<PathBuf>,
    /// Bearer API key; repeatable. None means open access on the bind address.
    #[arg(long = "api-key")]
    pub api_keys: Vec<String>,
    /// Serve MCP (the default; kept so documented invocations work).
    #[arg(long, conflicts_with = "no_mcp")]
    pub mcp: bool,
    /// Do not serve MCP.
    #[arg(long)]
    pub no_mcp: bool,
    /// Per-key daily spend cap in USD (0 = unlimited).
    #[arg(long)]
    pub daily_cost_cap_usd: Option<f64>,
    /// Index used by `/v1/mcp` and unqualified requests.
    #[arg(long)]
    pub default_index: Option<String>,
}

pub async fn run(args: Args, config: &Config, out: &Output) -> Result<()> {
    let mut config = config.clone();
    if let Some(b) = args.bind {
        config.server.bind = b;
    }
    if let Some(r) = args.index_root {
        config.server.index_root = r;
    }
    if !args.api_keys.is_empty() {
        config.server.api_keys = args.api_keys;
    }
    if args.mcp {
        config.server.mcp = true;
    }
    if args.no_mcp {
        config.server.mcp = false;
    }
    if let Some(c) = args.daily_cost_cap_usd {
        config.server.daily_cost_cap_usd = c;
    }
    if let Some(d) = args.default_index {
        config.server.default_index = Some(d);
    }
    let summary = serde_json::json!({
        "bind": config.server.bind,
        "index_root": config.server.index_root,
        "mcp": config.server.mcp,
        "auth": !config.server.api_keys.is_empty(),
    });
    out.emit(&summary, || {
        format!(
            "serving http://{} (indexes under {}, mcp {}, auth {})",
            config.server.bind,
            config.server.index_root.display(),
            if config.server.mcp { "on" } else { "off" },
            if config.server.api_keys.is_empty() {
                "off"
            } else {
                "on"
            }
        )
    });
    let cancel = tokio_util::sync::CancellationToken::new();
    let c2 = cancel.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        c2.cancel();
    });
    vi_server::serve(Arc::new(config), cancel).await?;
    Ok(())
}
