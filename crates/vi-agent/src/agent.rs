//! The loop: retrieve, decide, look, answer, under a budget.

use std::sync::Arc;
use std::time::Instant;

use futures::stream::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::mpsc;
use vi_core::config::{roles, Config};
use vi_core::{Result, VideoId};
use vi_index::Storage;
use vi_providers::{
    ContentPart, GenerateEvent, GenerateRequest, Message, ProviderRegistry, Role, ToolChoice, Vlm,
};

use crate::citations::{Piece, Scanner};
use crate::policy::{Policy, PolicyState, Step};
use crate::tools::{self, ToolCall, ToolContext};

/// Limits for one `ask`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AskBudget {
    /// Tokens across all provider calls (input plus output).
    pub max_tokens: u64,
    /// Provider spend in USD.
    pub max_cost_usd: f64,
    /// Wall clock in seconds.
    pub max_wallclock_secs: f64,
    /// Tool calls.
    pub max_tool_calls: u32,
}

impl Default for AskBudget {
    fn default() -> Self {
        Self {
            max_tokens: 50_000,
            max_cost_usd: 0.50,
            max_wallclock_secs: 120.0,
            max_tool_calls: 8,
        }
    }
}

/// Spend so far.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AskUsage {
    /// Input tokens.
    pub tokens_in: u64,
    /// Output tokens.
    pub tokens_out: u64,
    /// USD.
    pub cost_usd: f64,
    /// Tool calls made.
    pub tool_calls: u32,
    /// Provider calls made (LLM turns plus describe calls).
    pub provider_calls: u32,
    /// Wall clock.
    pub wallclock_ms: u64,
}

/// Streamed events (`docs/06-query-and-agents.md`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum AskEvent {
    /// Progress note.
    Status {
        /// Text.
        text: String,
    },
    /// The loop is calling a tool.
    ToolCall {
        /// Tool name.
        tool: String,
        /// Arguments.
        args: serde_json::Value,
    },
    /// A tool returned.
    ToolResult {
        /// Tool name.
        tool: String,
        /// One-line summary.
        summary: String,
    },
    /// Answer text.
    Token {
        /// Text.
        text: String,
    },
    /// A citation backing the preceding text.
    Citation {
        /// Video.
        video_id: VideoId,
        /// Start, seconds.
        t0: f64,
        /// End, seconds.
        t1: f64,
        /// Evidence kind when the citation matches a tool result: `transcript`, `ocr`, `frame`, `description`, else `range`.
        kind: String,
    },
    /// End of the answer.
    Done {
        /// Whether a budget cut the loop short.
        partial: bool,
        /// Why it stopped early, if it did.
        reason: Option<String>,
        /// Spend.
        usage: AskUsage,
    },
}

/// What to ask.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AskRequest {
    /// The question.
    pub question: String,
    /// Restrict to these videos; empty means the whole index.
    pub videos: Vec<VideoId>,
    /// Limits.
    pub budget: AskBudget,
    /// Conversation to continue.
    pub session_id: Option<String>,
}

impl AskRequest {
    /// A question with default budget over the whole index.
    pub fn new(question: impl Into<String>) -> Self {
        Self {
            question: question.into(),
            videos: Vec::new(),
            budget: AskBudget::default(),
            session_id: None,
        }
    }
}

/// Everything an `ask` produced, for non-streaming callers.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Collected {
    /// Answer text without citation markers.
    pub text: String,
    /// Citations in order.
    pub citations: Vec<(VideoId, f64, f64)>,
    /// Tool calls made.
    pub tool_calls: Vec<(String, serde_json::Value)>,
    /// Spend.
    pub usage: AskUsage,
    /// Cut short.
    pub partial: bool,
}

/// Session TTL.
const SESSION_TTL_SECS: u64 = 24 * 3600;

/// The agent.
pub struct Agent {
    storage: Arc<dyn Storage>,
    providers: Arc<ProviderRegistry>,
    config: Arc<Config>,
    policy: Option<Arc<dyn Policy>>,
}

impl std::fmt::Debug for Agent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Agent").finish()
    }
}

struct Turn {
    text: String,
    calls: Vec<ToolCall>,
    finish: String,
}

impl Agent {
    /// An agent using the `agent_llm` (and `agent_vlm`) roles.
    pub fn new(
        storage: Arc<dyn Storage>,
        providers: Arc<ProviderRegistry>,
        config: Arc<Config>,
    ) -> Self {
        Self {
            storage,
            providers,
            config,
            policy: None,
        }
    }

    /// Use a fixed policy for tool selection; the LLM still writes the
    /// answer.
    pub fn with_policy(mut self, policy: Arc<dyn Policy>) -> Self {
        self.policy = Some(policy);
        self
    }

    /// Ask, streaming events. The stream ends with [`AskEvent::Done`].
    pub fn ask(&self, req: AskRequest) -> impl Stream<Item = AskEvent> + Send + 'static {
        let (tx, rx) = mpsc::channel::<AskEvent>(64);
        let storage = self.storage.clone();
        let providers = self.providers.clone();
        let config = self.config.clone();
        let policy = self.policy.clone();
        tokio::spawn(async move {
            let started = Instant::now();
            let mut usage = AskUsage::default();
            let result = run(
                storage,
                providers,
                config,
                policy,
                req,
                tx.clone(),
                &mut usage,
                started,
            )
            .await;
            usage.wallclock_ms = started.elapsed().as_millis() as u64;
            let done = match result {
                Ok((partial, reason)) => AskEvent::Done {
                    partial,
                    reason,
                    usage,
                },
                Err(e) => {
                    let _ = tx
                        .send(AskEvent::Status {
                            text: format!("error: {e}"),
                        })
                        .await;
                    AskEvent::Done {
                        partial: true,
                        reason: Some(e.to_string()),
                        usage,
                    }
                }
            };
            let _ = tx.send(done).await;
        });
        tokio_stream_from(rx)
    }

    /// Ask and collect.
    pub async fn ask_collect(&self, req: AskRequest) -> Collected {
        let mut out = Collected::default();
        let mut s = Box::pin(self.ask(req));
        while let Some(ev) = s.next().await {
            match ev {
                AskEvent::Token { text } => out.text.push_str(&text),
                AskEvent::Citation {
                    video_id, t0, t1, ..
                } => out.citations.push((video_id, t0, t1)),
                AskEvent::ToolCall { tool, args } => out.tool_calls.push((tool, args)),
                AskEvent::Done { partial, usage, .. } => {
                    out.partial = partial;
                    out.usage = usage;
                }
                _ => {}
            }
        }
        out
    }
}

fn tokio_stream_from(
    rx: mpsc::Receiver<AskEvent>,
) -> impl Stream<Item = AskEvent> + Send + 'static {
    futures::stream::unfold(
        rx,
        |mut rx| async move { rx.recv().await.map(|ev| (ev, rx)) },
    )
}

/// Session state persisted between asks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Session {
    turns: Vec<(String, String)>,
}

#[allow(clippy::too_many_arguments)]
async fn run(
    storage: Arc<dyn Storage>,
    providers: Arc<ProviderRegistry>,
    config: Arc<Config>,
    policy: Option<Arc<dyn Policy>>,
    req: AskRequest,
    tx: mpsc::Sender<AskEvent>,
    usage: &mut AskUsage,
    started: Instant,
) -> Result<(bool, Option<String>)> {
    let llm: Arc<dyn Vlm> = providers.agent_vlm()?;
    let caps = llm.vlm_capabilities();
    let with_describe = providers.has_role(roles::VLM_DESCRIBE);
    let ctx = ToolContext {
        storage: storage.clone(),
        providers: providers.clone(),
        config: config.clone(),
        videos: req.videos.clone(),
    };
    let videos = storage.list_videos().await?;
    let known: std::collections::BTreeSet<VideoId> = videos.iter().map(|v| v.id).collect();

    // System prompt with the video list (compact) and the date.
    let prompt = vi_providers::prompts::get("agent_system")
        .ok_or_else(|| vi_core::Error::Other("agent_system prompt missing".into()))?;
    let mut system = prompt.text.clone();
    let listed: Vec<&vi_core::model::Video> = videos
        .iter()
        .filter(|v| req.videos.is_empty() || req.videos.contains(&v.id))
        .collect();
    if listed.len() <= 40 {
        system.push_str("\n\nVideos in this index (id | duration | title):\n");
        for v in &listed {
            system.push_str(&format!(
                "- {} | {} | {}\n",
                v.id,
                vi_perceive::grid::hms(v.duration.as_secs_f64()),
                v.title.clone().unwrap_or_default()
            ));
        }
    } else {
        system.push_str(&format!(
            "\n\nThe index holds {} videos; call list_videos when you need ids.\n",
            listed.len()
        ));
    }
    if caps.max_images_per_request == 0 {
        system.push_str("\nYou cannot see images: do not call view.\n");
    }
    let session: Session = match &req.session_id {
        Some(id) => storage
            .get_session(id)
            .await?
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default(),
        None => Session::default(),
    };
    let mut messages = vec![Message::text(Role::System, system)];
    for (q, a) in &session.turns {
        messages.push(Message::text(Role::User, q.clone()));
        messages.push(Message::text(Role::Assistant, a.clone()));
    }
    messages.push(Message::text(Role::User, req.question.clone()));

    let mut specs = tools::specs(with_describe);
    if caps.max_images_per_request == 0 {
        specs.retain(|t| t.name != "view");
    }
    let mut scanner = Scanner::default();
    let mut answer = String::new();
    let mut partial = false;
    let mut retried_empty = false;
    let mut reason: Option<String> = None;
    let mut steps: Vec<(ToolCall, String)> = Vec::new();
    let mut evidence: Vec<(VideoId, f64, f64, String)> = Vec::new();

    let _ = tx
        .send(AskEvent::Status {
            text: "searching".into(),
        })
        .await;

    loop {
        // Budget check before each provider call.
        let over = budget_exceeded(&req.budget, usage, started);
        let tools_left = req.budget.max_tool_calls.saturating_sub(usage.tool_calls);
        if let Some(why) = &over {
            partial = true;
            reason = Some(why.clone());
        }
        // A fixed policy chooses tools; otherwise the LLM does.
        let policy_step = match (&policy, over.is_some()) {
            (Some(p), false) => Some(
                p.next_step(&PolicyState {
                    question: req.question.clone(),
                    steps: steps.clone(),
                    tool_calls_left: tools_left,
                })
                .await?,
            ),
            _ => None,
        };
        if let Some(Step::Tool(call)) = policy_step {
            let out = run_tool(&ctx, &call, &tx, usage, &mut evidence).await?;
            messages.push(Message::text(
                Role::User,
                format!("Result of {}({}):\n{}", call.name, call.args, out.content),
            ));
            if let Some(img) = out.image {
                messages.push(Message {
                    role: Role::User,
                    parts: vec![ContentPart::Image(img)],
                });
            }
            steps.push((call, out.summary));
            continue;
        }
        // Generation turn: tools only when the LLM chooses and budget allows.
        let allow_tools = policy.is_none() && over.is_none() && tools_left > 0;
        let mut greq = GenerateRequest::new(messages.clone());
        // Tools stay defined whenever the history holds tool calls (some APIs
        // require that and a model shown call syntax with no tools tends to
        // return nothing or to write pseudo calls); `tool_choice: None`
        // forces a text answer.
        let history_has_tools = !steps.is_empty();
        if allow_tools {
            greq.tools = specs.clone();
        } else if history_has_tools {
            greq.tools = specs.clone();
            greq.tool_choice = ToolChoice::None;
        }
        greq.max_tokens = 1500;
        if over.is_some() {
            greq.messages.push(Message::text(
                Role::User,
                "The budget for looking is exhausted. Answer now with what you have, and say what could not be checked.",
            ));
        } else if !allow_tools && history_has_tools {
            greq.messages.push(Message::text(
                Role::User,
                "No further tool calls are available in this turn. Write the answer now from the observations above, citing the timestamps you used; do not write tool calls.",
            ));
        }
        let turn = generate_turn(
            llm.as_ref(),
            greq,
            &tx,
            &mut scanner,
            &known,
            &evidence,
            &mut answer,
            usage,
        )
        .await?;
        usage.provider_calls += 1;
        if !allow_tools && !turn.calls.is_empty() {
            // Tools were withheld (`tool_choice: none`) but the model called
            // one anyway: some OpenAI-compatible servers ignore the flag.
            // Never execute past the budget; ask once for a text answer,
            // then stop with what there is.
            if !retried_empty {
                retried_empty = true;
                tracing::debug!(
                    calls = turn.calls.len(),
                    "tool calls after the budget; asking for text"
                );
                messages.push(Message::text(
                    Role::User,
                    "Do not call tools. Write the final answer now as text from the observations above.",
                ));
                continue;
            }
            partial = true;
            reason.get_or_insert_with(|| {
                "model kept calling tools after the budget was exhausted".into()
            });
            break;
        }
        if turn.calls.is_empty() {
            if turn.finish == "length" {
                partial = true;
                reason.get_or_insert_with(|| "answer hit the output token limit".into());
            } else if turn.text.trim().is_empty() && answer.trim().is_empty() {
                // Seen after several tool turns with tools withheld: the
                // model ends its turn with no content. Ask once more,
                // explicitly, before giving up.
                if !retried_empty {
                    retried_empty = true;
                    tracing::debug!(finish = %turn.finish, "empty answer turn; asking again");
                    messages.push(Message::text(
                        Role::User,
                        "Write the answer now from the observations above, citing the timestamps you used. If they do not settle the question, say what is known and what is not.",
                    ));
                    continue;
                }
                // Last resort: a fresh, compact request built from the
                // notes the tools produced, with no tool history at all.
                tracing::warn!(finish = %turn.finish, steps = steps.len(), "two empty answer turns; answering from notes");
                let notes = notes_from_steps(&steps, &evidence);
                let system_text = messages
                    .first()
                    .filter(|m| m.role == Role::System)
                    .and_then(|m| {
                        m.parts.iter().find_map(|p| match p {
                            ContentPart::Text(t) => Some(t.clone()),
                            _ => None,
                        })
                    })
                    .unwrap_or_default();
                let mut greq2 = GenerateRequest::new(vec![
                    Message::text(Role::System, system_text),
                    Message::text(
                        Role::User,
                        format!(
                            "{}\n\nNotes gathered from the index (tool: result):\n{notes}\n\nWrite the final answer from these notes, citing timestamps as [[cite:VIDEO_ID:T0-T1]]. If the notes do not settle the question, give the most likely answer and say what is uncertain.",
                            req.question
                        ),
                    ),
                ]);
                greq2.max_tokens = 1500;
                let turn2 = generate_turn(
                    llm.as_ref(),
                    greq2,
                    &tx,
                    &mut scanner,
                    &known,
                    &evidence,
                    &mut answer,
                    usage,
                )
                .await?;
                usage.provider_calls += 1;
                if turn2.text.trim().is_empty() {
                    partial = true;
                    reason.get_or_insert_with(|| {
                        format!("model returned an empty answer (finish: {})", turn.finish)
                    });
                } else {
                    reason.get_or_insert_with(|| {
                        "answered from tool notes after two empty turns".into()
                    });
                }
            }
            break;
        }
        // Record the assistant turn (text plus calls), run the tools.
        let mut parts = Vec::new();
        if !turn.text.is_empty() {
            parts.push(ContentPart::Text(turn.text.clone()));
        }
        for c in &turn.calls {
            parts.push(ContentPart::ToolCall {
                id: c.id.clone(),
                name: c.name.clone(),
                arguments: c.args.to_string(),
            });
        }
        messages.push(Message {
            role: Role::Assistant,
            parts,
        });
        let mut images = Vec::new();
        for call in &turn.calls {
            let out = run_tool(&ctx, call, &tx, usage, &mut evidence).await?;
            messages.push(Message {
                role: Role::Tool,
                parts: vec![ContentPart::ToolResult {
                    call_id: call.id.clone(),
                    name: call.name.clone(),
                    content: out.content,
                }],
            });
            if let Some(img) = out.image {
                images.push(img);
            }
            steps.push((call.clone(), out.summary));
        }
        if !images.is_empty() {
            messages.push(Message {
                role: Role::User,
                parts: images.into_iter().map(ContentPart::Image).collect(),
            });
        }
    }
    for piece in scanner.finish() {
        if let Piece::Text(t) = piece {
            answer.push_str(&t);
            let _ = tx.send(AskEvent::Token { text: t }).await;
        }
    }
    if let Some(id) = &req.session_id {
        let mut s = session;
        s.turns.push((req.question.clone(), answer.clone()));
        if s.turns.len() > 20 {
            let n = s.turns.len() - 20;
            s.turns.drain(..n);
        }
        let _ = storage
            .put_session(
                id,
                &serde_json::to_value(&s).unwrap_or(json!({})),
                SESSION_TTL_SECS,
            )
            .await;
    }
    Ok((partial, reason))
}

fn budget_exceeded(b: &AskBudget, u: &AskUsage, started: Instant) -> Option<String> {
    if u.tokens_in + u.tokens_out >= b.max_tokens {
        return Some(format!("token budget ({}) reached", b.max_tokens));
    }
    if b.max_cost_usd > 0.0 && u.cost_usd >= b.max_cost_usd {
        return Some(format!("cost budget (${:.2}) reached", b.max_cost_usd));
    }
    if b.max_wallclock_secs > 0.0 && started.elapsed().as_secs_f64() >= b.max_wallclock_secs {
        return Some(format!(
            "wall-clock budget ({:.0}s) reached",
            b.max_wallclock_secs
        ));
    }
    None
}

async fn run_tool(
    ctx: &ToolContext,
    call: &ToolCall,
    tx: &mpsc::Sender<AskEvent>,
    usage: &mut AskUsage,
    evidence: &mut Vec<(VideoId, f64, f64, String)>,
) -> Result<tools::ToolOutput> {
    let _ = tx
        .send(AskEvent::ToolCall {
            tool: call.name.clone(),
            args: call.args.clone(),
        })
        .await;
    let out = tools::execute(ctx, call).await?;
    usage.tool_calls += 1;
    usage.cost_usd += out.cost_usd;
    usage.tokens_in += out.tokens_in;
    usage.tokens_out += out.tokens_out;
    if out.cost_usd > 0.0 || out.tokens_in > 0 {
        usage.provider_calls += 1;
    }
    // Remember evidence ranges so citations can be typed.
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&out.content) {
        for h in v["hits"].as_array().into_iter().flatten() {
            if let (Some(id), Some(t0), Some(t1)) = (
                h["video_id"].as_str().and_then(|s| VideoId::parse(s).ok()),
                h["t0"].as_f64(),
                h["t1"].as_f64(),
            ) {
                let kind = h["evidence"][0]["kind"]
                    .as_str()
                    .unwrap_or("range")
                    .to_string();
                evidence.push((id, t0, t1, kind));
            }
        }
        if let (Some(id), Some(t0), Some(t1)) = (
            v["video_id"].as_str().and_then(|s| VideoId::parse(s).ok()),
            v["t0"].as_f64(),
            v["t1"].as_f64(),
        ) {
            let kind = v["kind"]
                .as_str()
                .unwrap_or(if call.name == "view" {
                    "frame"
                } else {
                    "range"
                })
                .to_string();
            evidence.push((id, t0, t1, kind));
        }
    }
    let _ = tx
        .send(AskEvent::ToolResult {
            tool: call.name.clone(),
            summary: out.summary.clone(),
        })
        .await;
    Ok(out)
}

/// Compact record of what the tools found, for the answer-from-notes fallback.
fn notes_from_steps(
    steps: &[(ToolCall, String)],
    evidence: &[(VideoId, f64, f64, String)],
) -> String {
    let mut out = String::new();
    for (call, summary) in steps.iter().take(24) {
        let args = call.args.to_string();
        let args = if args.len() > 160 {
            format!("{}…", &args[..160])
        } else {
            args
        };
        out.push_str(&format!(
            "- {}({args}): {}\n",
            call.name,
            truncate(summary, 300)
        ));
    }
    if !evidence.is_empty() {
        out.push_str("\nEvidence seen (video, seconds, text):\n");
        for (vid, t0, t1, text) in evidence.iter().take(40) {
            out.push_str(&format!(
                "- {vid} {t0:.0}-{t1:.0}: {}\n",
                truncate(text, 240)
            ));
        }
    }
    out
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

#[allow(clippy::too_many_arguments)]
async fn generate_turn(
    llm: &dyn Vlm,
    req: GenerateRequest,
    tx: &mpsc::Sender<AskEvent>,
    scanner: &mut Scanner,
    known: &std::collections::BTreeSet<VideoId>,
    evidence: &[(VideoId, f64, f64, String)],
    answer: &mut String,
    usage: &mut AskUsage,
) -> Result<Turn> {
    let mut stream = llm.generate(req).await?;
    let mut turn = Turn {
        text: String::new(),
        calls: Vec::new(),
        finish: "stop".into(),
    };
    while let Some(ev) = stream.next().await {
        match ev? {
            GenerateEvent::Token { text } => {
                turn.text.push_str(&text);
                for piece in scanner.push(&text) {
                    match piece {
                        Piece::Text(t) => {
                            answer.push_str(&t);
                            let _ = tx.send(AskEvent::Token { text: t }).await;
                        }
                        Piece::Cite(c) => {
                            if known.contains(&c.video_id) {
                                let kind = evidence
                                    .iter()
                                    .filter(|(v, a, b, _)| {
                                        *v == c.video_id && c.t0 <= *b + 1.0 && c.t1 >= *a - 1.0
                                    })
                                    .map(|e| e.3.clone())
                                    .next()
                                    .unwrap_or_else(|| "range".into());
                                let _ = tx
                                    .send(AskEvent::Citation {
                                        video_id: c.video_id,
                                        t0: c.t0,
                                        t1: c.t1,
                                        kind,
                                    })
                                    .await;
                            }
                        }
                    }
                }
            }
            GenerateEvent::ToolCall {
                id,
                name,
                arguments,
            } => {
                let args: serde_json::Value = serde_json::from_str(&arguments).unwrap_or(json!({}));
                turn.calls.push(ToolCall { id, name, args });
            }
            GenerateEvent::Usage(_) => {}
            GenerateEvent::Done {
                finish_reason,
                stats,
            } => {
                turn.finish = finish_reason;
                usage.tokens_in += stats.usage.tokens_in;
                usage.tokens_out += stats.usage.tokens_out;
                usage.cost_usd += stats.cost_usd;
            }
        }
    }
    Ok(turn)
}
