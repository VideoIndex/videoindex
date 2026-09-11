//! Live smoke test of a chat adapter: `chat <adapter> [model] [prompt]`.
//! Reads the API key from the adapter's default environment variable.
#![allow(clippy::unwrap_used)]
use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use vi_core::config::ProviderConfig;
use vi_providers::adapters::collect_stream;
use vi_providers::{Anthropic, Gemini, Governor, Llm, Message, OpenAiCompat, Role};
use vi_providers::{GenerateRequest, ToolSpec};

#[tokio::main]
async fn main() {
    let adapter = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "anthropic".into());
    let model = std::env::args().nth(2);
    let prompt = std::env::args()
        .nth(3)
        .unwrap_or_else(|| "In one short sentence, what is a frame grid? Then call the `search` tool with the query \"frame grid\".".into());
    let cfg = ProviderConfig {
        adapter: adapter.clone(),
        model: model.clone(),
        base_url: std::env::var("VI_CHAT_BASE_URL").ok(),
        ..ProviderConfig::default()
    };
    let gov = Arc::new(Governor::from_config(&adapter, &cfg));
    let llm: Arc<dyn Llm> = match adapter.as_str() {
        "anthropic" => Arc::new(
            Anthropic::from_config("anthropic", &cfg, None, gov, CancellationToken::new()).unwrap(),
        ),
        "gemini" => Arc::new(
            Gemini::from_config("gemini", &cfg, None, gov, CancellationToken::new()).unwrap(),
        ),
        _ => Arc::new(
            OpenAiCompat::from_config("openai_compat", &cfg, None, gov, CancellationToken::new())
                .unwrap(),
        ),
    };
    let mut req = GenerateRequest::new(vec![
        Message::text(Role::System, "You are terse."),
        Message::text(Role::User, prompt),
    ]);
    req.tools.push(ToolSpec {
        name: "search".into(),
        description: "Search the video index.".into(),
        parameters: serde_json::json!({"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}),
    });
    req.max_tokens = 200;
    let started = std::time::Instant::now();
    let out = collect_stream(llm.generate(req).await.unwrap())
        .await
        .unwrap();
    println!("model: {}", llm.model());
    println!("text: {}", out.text.trim());
    println!("tool_calls: {:?}", out.tool_calls);
    println!("finish: {}  usage: {:?}", out.finish_reason, out.usage);
    println!(
        "stats: {:?}",
        out.stats.map(|s| (s.cost_usd, s.latency_ms, s.attempts))
    );
    println!("wall: {:.2}s", started.elapsed().as_secs_f64());
}
