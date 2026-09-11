//! The `Policy` trait: who decides the next step. The default is the
//! tool-using LLM inside [`crate::Agent`]; fixed strategies are useful eval
//! baselines and can be supplied from Python or JS later.

use async_trait::async_trait;
use serde_json::json;
use vi_core::Result;

use crate::tools::ToolCall;

/// What the loop knows when it asks a policy for the next step.
#[derive(Debug, Clone)]
pub struct PolicyState {
    /// The question.
    pub question: String,
    /// Tool calls made so far with their result summaries.
    pub steps: Vec<(ToolCall, String)>,
    /// Tool calls still allowed.
    pub tool_calls_left: u32,
}

/// A policy's decision.
#[derive(Debug, Clone)]
pub enum Step {
    /// Run this tool.
    Tool(ToolCall),
    /// Compose the answer now (the LLM writes it from the observations).
    Answer,
}

/// Decides the next step.
#[async_trait]
pub trait Policy: Send + Sync {
    /// Next step given the state so far.
    async fn next_step(&self, state: &PolicyState) -> Result<Step>;
    /// Name for reports.
    fn name(&self) -> &str;
}

/// Baseline: one search with the question, then answer. Measures what the
/// index alone gives before any looking.
#[derive(Debug, Default)]
pub struct RetrievalOnlyPolicy {
    /// Hits to fetch.
    pub k: u64,
}

#[async_trait]
impl Policy for RetrievalOnlyPolicy {
    async fn next_step(&self, state: &PolicyState) -> Result<Step> {
        if state.steps.is_empty() && state.tool_calls_left > 0 {
            return Ok(Step::Tool(ToolCall {
                id: "policy_search".into(),
                name: "search".into(),
                args: json!({"query": state.question, "k": self.k.max(1)}),
            }));
        }
        Ok(Step::Answer)
    }

    fn name(&self) -> &str {
        "retrieval_only"
    }
}
