//! `vidx ask <index-dir> "<question>" [--budget-usd ..] [--video ID] [--session ID] [--model NAME] [--json]`

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use futures::StreamExt;
use std::io::Write;
use vi_agent::{Agent, AskBudget, AskEvent, AskRequest, RetrievalOnlyPolicy};
use vi_core::{Config, VideoId};
use vi_index::EmbeddedIndex;

use crate::output::Output;

/// Arguments.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Index directory.
    pub index_dir: PathBuf,
    /// The question.
    pub question: String,
    /// Restrict to a video id (repeatable).
    #[arg(long = "video")]
    pub videos: Vec<VideoId>,
    /// Token budget (input plus output across all calls).
    #[arg(long, default_value_t = 50_000)]
    pub budget_tokens: u64,
    /// Cost budget in USD.
    #[arg(long, default_value_t = 0.5)]
    pub budget_usd: f64,
    /// Wall-clock budget in seconds.
    #[arg(long, default_value_t = 120.0)]
    pub budget_secs: f64,
    /// Max tool calls.
    #[arg(long, default_value_t = 8)]
    pub max_tool_calls: u32,
    /// Max output tokens per model turn (length of the answer).
    #[arg(long, default_value_t = 4_000)]
    pub max_answer_tokens: u64,
    /// Conversation id to continue.
    #[arg(long)]
    pub session: Option<String>,
    /// Tool policy: `agent` (the LLM decides) or `retrieval-only` (one
    /// search, then answer).
    #[arg(long, default_value = "agent")]
    pub policy: String,
    /// Chat model: a `[providers.*]` name or its model id (default: the
    /// `agent_llm` role).
    #[arg(long)]
    pub model: Option<String>,
}

pub async fn run(args: Args, config: &Config, out: &Output) -> Result<()> {
    let idx = Arc::new(
        EmbeddedIndex::open(&args.index_dir)
            .with_context(|| format!("opening index at {}", args.index_dir.display()))?,
    );
    let config = Arc::new(config.clone());
    let providers = Arc::new(vi_providers::ProviderRegistry::new(
        config.clone(),
        tokio_util::sync::CancellationToken::new(),
    ));
    vi_perceive::OnnxLocal::register(&providers);
    let mut agent = Agent::new(idx.clone(), providers.clone(), config);
    if matches!(args.policy.as_str(), "retrieval-only" | "retrieval_only") {
        agent = agent.with_policy(Arc::new(RetrievalOnlyPolicy { k: 8 }));
    } else if args.policy != "agent" {
        anyhow::bail!(
            "unknown policy '{}'; expected agent or retrieval-only",
            args.policy
        );
    }
    if let Some(model) = &args.model {
        let provider = providers.find_llm_provider(model).with_context(|| {
            let known: Vec<String> = providers
                .llm_providers()
                .iter()
                .map(|p| format!("{} ({})", p.provider, p.model))
                .collect();
            format!("unknown model '{model}'; configured: {}", known.join(", "))
        })?;
        agent = agent.with_provider(provider);
    }
    let req = AskRequest {
        question: args.question.clone(),
        videos: args.videos.clone(),
        budget: AskBudget {
            max_tokens: args.budget_tokens,
            max_cost_usd: args.budget_usd,
            max_wallclock_secs: args.budget_secs,
            max_tool_calls: args.max_tool_calls,
            max_answer_tokens: args.max_answer_tokens,
        },
        session_id: args.session.clone(),
    };
    // Titles for citation display.
    use vi_index::Storage;
    let titles: std::collections::BTreeMap<VideoId, String> = idx
        .list_videos()
        .await?
        .into_iter()
        .map(|v| (v.id, v.title.unwrap_or_default()))
        .collect();
    let multi = titles.len() > 1;
    let mut stream = Box::pin(agent.ask(req));
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    let mut printed_text = false;
    while let Some(ev) = stream.next().await {
        if out.json() {
            writeln!(lock, "{}", serde_json::to_string(&ev)?)?;
            continue;
        }
        match &ev {
            AskEvent::Status { text } => eprintln!("· {text}"),
            AskEvent::ToolCall { tool, args } => eprintln!("→ {tool} {args}"),
            AskEvent::ToolResult { tool, summary } => eprintln!("← {tool}: {summary}"),
            AskEvent::Token { text } => {
                write!(lock, "{text}")?;
                lock.flush()?;
                printed_text = true;
            }
            AskEvent::Citation {
                video_id, t0, t1, ..
            } => {
                let t = vi_perceive::grid::hms(*t0);
                let label = if multi {
                    let title = titles.get(video_id).cloned().unwrap_or_default();
                    let short: String = title.chars().take(40).collect();
                    if (t1 - t0) > 1.0 {
                        format!(" [{short} {t}–{}]", vi_perceive::grid::hms(*t1))
                    } else {
                        format!(" [{short} {t}]")
                    }
                } else if (t1 - t0) > 1.0 {
                    format!(" [{t}–{}]", vi_perceive::grid::hms(*t1))
                } else {
                    format!(" [{t}]")
                };
                write!(lock, "{label}")?;
                lock.flush()?;
            }
            AskEvent::Done {
                partial,
                reason,
                usage,
            } => {
                if printed_text {
                    writeln!(lock)?;
                }
                eprintln!(
                    "— {}{} tokens {}+{}, ${:.4}, {} tool calls, {} provider calls, {:.1}s",
                    if *partial { "partial answer" } else { "done" },
                    reason
                        .as_deref()
                        .map(|r| format!(" ({r})"))
                        .unwrap_or_default(),
                    usage.tokens_in,
                    usage.tokens_out,
                    usage.cost_usd,
                    usage.tool_calls,
                    usage.provider_calls,
                    usage.wallclock_ms as f64 / 1000.0
                );
            }
            _ => {}
        }
    }
    Ok(())
}
