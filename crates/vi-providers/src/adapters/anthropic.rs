//! `anthropic`: the Messages API (`Vlm` with images, `Llm` with tools),
//! streaming over SSE. No native video: frame grids go in as images.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use futures::StreamExt;
use serde_json::json;
use tokio_util::sync::CancellationToken;
use vi_core::config::{Pricing, ProviderConfig, RoleBinding};

use crate::adapters::common::{
    check_status, final_stats, http_client, image_base64, schema_instruction, transport,
    ToolCallBuilder,
};
use crate::cost::Usage;
use crate::error::{short_body, ProviderError, Result};
use crate::governor::Governor;
use crate::pricing::default_pricing;
use crate::retry;
use crate::sse;
use crate::traits::*;

/// API version header value.
pub const API_VERSION: &str = "2023-06-01";
/// Default endpoint.
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
/// Default model.
pub const DEFAULT_MODEL: &str = "claude-sonnet-5";

/// The adapter.
pub struct Anthropic {
    name: String,
    base_url: String,
    api_key: String,
    model: String,
    pricing: Pricing,
    http: reqwest::Client,
    governor: Arc<Governor>,
    cancel: CancellationToken,
}

impl std::fmt::Debug for Anthropic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Anthropic")
            .field("name", &self.name)
            .field("model", &self.model)
            .finish()
    }
}

impl Anthropic {
    /// Build from config. `api_key_env` defaults to `ANTHROPIC_API_KEY`.
    pub fn from_config(
        name: &str,
        cfg: &ProviderConfig,
        role: Option<&RoleBinding>,
        governor: Arc<Governor>,
        cancel: CancellationToken,
    ) -> Result<Self> {
        let var = cfg
            .api_key_env
            .clone()
            .unwrap_or_else(|| "ANTHROPIC_API_KEY".into());
        let api_key = std::env::var(&var)
            .ok()
            .filter(|k| !k.is_empty())
            .ok_or_else(|| {
                ProviderError::NotConfigured(format!(
                    "provider '{name}' (anthropic) needs the API key in ${var}"
                ))
            })?;
        let model = role
            .and_then(|r| r.model.clone())
            .or_else(|| cfg.model.clone())
            .unwrap_or_else(|| DEFAULT_MODEL.to_string());
        let base_url = role
            .and_then(|r| r.base_url.clone())
            .or_else(|| cfg.base_url.clone())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
            .trim_end_matches('/')
            .to_string();
        Ok(Self {
            name: name.to_string(),
            base_url,
            api_key,
            pricing: cfg.pricing.unwrap_or_else(|| default_pricing(&model)),
            model,
            http: http_client(name, governor.timeout)?,
            governor,
            cancel,
        })
    }

    fn body(&self, req: &GenerateRequest) -> Result<serde_json::Value> {
        let mut system = Vec::new();
        let mut messages: Vec<serde_json::Value> = Vec::new();
        for m in &req.messages {
            if m.role == Role::System {
                for p in &m.parts {
                    if let ContentPart::Text(t) = p {
                        system.push(t.clone());
                    }
                }
                continue;
            }
            let role = if m.role == Role::Assistant {
                "assistant"
            } else {
                "user"
            };
            let mut content = Vec::new();
            for p in &m.parts {
                match p {
                    ContentPart::Text(t) => content.push(json!({"type": "text", "text": t})),
                    ContentPart::Image(im) => {
                        let (mime, b64) = image_base64(im)?;
                        content.push(json!({"type": "image", "source": {"type": "base64", "media_type": mime, "data": b64}}));
                    }
                    ContentPart::Video { .. } => {
                        return Err(ProviderError::Invalid(
                            "anthropic has no native video input; send a frame grid".into(),
                        ))
                    }
                    ContentPart::ToolCall {
                        id,
                        name,
                        arguments,
                        ..
                    } => {
                        let input: serde_json::Value =
                            serde_json::from_str(arguments).unwrap_or_else(|_| json!({}));
                        content.push(
                            json!({"type": "tool_use", "id": id, "name": name, "input": input}),
                        );
                    }
                    ContentPart::ToolResult {
                        call_id,
                        content: c,
                        ..
                    } => {
                        content.push(
                            json!({"type": "tool_result", "tool_use_id": call_id, "content": c}),
                        );
                    }
                }
            }
            // Consecutive same-role messages must merge (tool results
            // follow the assistant's tool_use in a user turn).
            match messages.last_mut() {
                Some(last) if last["role"] == role => {
                    if let Some(arr) = last["content"].as_array_mut() {
                        arr.extend(content);
                    }
                }
                _ => messages.push(json!({"role": role, "content": content})),
            }
        }
        if let Some(schema) = &req.json_schema {
            system.push(schema_instruction(schema));
        }
        let mut body = json!({
            "model": req.model.clone().unwrap_or_else(|| self.model.clone()),
            "max_tokens": req.max_tokens,
            "messages": messages,
            "stream": true,
        });
        // Claude 5 models reject the `temperature` field outright; older
        // ones default to 1.0. Send it only when a non-default value is
        // asked for, and let the API refuse where it must.
        if req.temperature > 0.0 {
            body["temperature"] = json!(req.temperature);
        }
        if !system.is_empty() {
            body["system"] = json!(system.join("\n\n"));
        }
        if !req.tools.is_empty() {
            body["tools"] = serde_json::Value::Array(
                req.tools
                    .iter()
                    .map(|t| json!({"name": t.name, "description": t.description, "input_schema": t.parameters}))
                    .collect(),
            );
            if req.tool_choice == ToolChoice::None {
                body["tool_choice"] = json!({"type": "none"});
            }
        }
        Ok(body)
    }

    async fn messages(&self, req: GenerateRequest) -> Result<EventStream> {
        let started = Instant::now();
        let permit = self.governor.acquire(req.estimated_tokens()).await?;
        let body = self.body(&req)?;
        let model = body["model"].as_str().unwrap_or(&self.model).to_string();
        let url = format!("{}/v1/messages", self.base_url);
        let timeout = self.governor.timeout.as_secs();
        let (resp, attempts) = retry::run(&self.governor.retry, &self.cancel, |_| {
            let body = body.clone();
            let url = url.clone();
            async move {
                let resp = self
                    .http
                    .post(&url)
                    .header("x-api-key", &self.api_key)
                    .header("anthropic-version", API_VERSION)
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| transport(&self.name, timeout, e))?;
                check_status(&self.name, resp).await
            }
        })
        .await?;
        let provider = self.name.clone();
        let pricing = self.pricing;
        let events = sse::events(resp.bytes_stream(), provider.clone());
        struct St {
            blocks: std::collections::BTreeMap<u64, ToolCallBuilder>,
            usage: Usage,
            stop: Option<String>,
            done: bool,
            queue: std::collections::VecDeque<GenerateEvent>,
            /// Content blocks seen, by type (`text`, `tool_use`, `thinking`,
            /// ...), and text characters streamed: the diagnostic for a
            /// `max_tokens` stop that carried neither text nor a call.
            block_kinds: std::collections::BTreeMap<String, usize>,
            text_chars: usize,
            tool_blocks_closed: usize,
        }
        let st = St {
            blocks: Default::default(),
            usage: Usage {
                calls: 1,
                ..Usage::default()
            },
            stop: None,
            done: false,
            queue: Default::default(),
            block_kinds: Default::default(),
            text_chars: 0,
            tool_blocks_closed: 0,
        };
        let stream =
            futures::stream::unfold((events, st, permit), move |(mut events, mut st, permit)| {
                let provider = provider.clone();
                let model = model.clone();
                async move {
                    loop {
                        if let Some(ev) = st.queue.pop_front() {
                            return Some((Ok(ev), (events, st, permit)));
                        }
                        if st.done {
                            return None;
                        }
                        match events.next().await {
                            Some(Ok(ev)) => {
                                let v: serde_json::Value = match serde_json::from_str(&ev.data) {
                                    Ok(v) => v,
                                    Err(e) => {
                                        st.done = true;
                                        return Some((
                                            Err(ProviderError::Decode {
                                                provider,
                                                message: format!("{e}: {}", short_body(&ev.data)),
                                            }),
                                            (events, st, permit),
                                        ));
                                    }
                                };
                                let kind = v["type"].as_str().unwrap_or(ev.event.as_str());
                                match kind {
                                    "message_start" => {
                                        st.usage.tokens_in = v["message"]["usage"]["input_tokens"]
                                            .as_u64()
                                            .unwrap_or(0);
                                    }
                                    "content_block_start" => {
                                        let idx = v["index"].as_u64().unwrap_or(0);
                                        let cb = &v["content_block"];
                                        *st.block_kinds
                                            .entry(cb["type"].as_str().unwrap_or("?").to_string())
                                            .or_default() += 1;
                                        if cb["type"] == "tool_use" {
                                            st.blocks.insert(
                                                idx,
                                                ToolCallBuilder {
                                                    id: cb["id"].as_str().unwrap_or("").to_string(),
                                                    name: cb["name"]
                                                        .as_str()
                                                        .unwrap_or("")
                                                        .to_string(),
                                                    arguments: String::new(),
                                                },
                                            );
                                        } else if let Some(t) = cb["text"].as_str() {
                                            if !t.is_empty() {
                                                st.text_chars += t.chars().count();
                                                st.queue.push_back(GenerateEvent::Token {
                                                    text: t.to_string(),
                                                });
                                            }
                                        }
                                    }
                                    "content_block_delta" => {
                                        let idx = v["index"].as_u64().unwrap_or(0);
                                        let d = &v["delta"];
                                        if let Some(t) = d["text"].as_str() {
                                            st.text_chars += t.chars().count();
                                            st.queue.push_back(GenerateEvent::Token {
                                                text: t.to_string(),
                                            });
                                        }
                                        if let Some(j) = d["partial_json"].as_str() {
                                            st.blocks.entry(idx).or_default().arguments.push_str(j);
                                        }
                                    }
                                    "content_block_stop" => {
                                        let idx = v["index"].as_u64().unwrap_or(0);
                                        if let Some(b) = st.blocks.remove(&idx) {
                                            st.tool_blocks_closed += 1;
                                            st.queue.push_back(b.finish());
                                        }
                                    }
                                    "message_delta" => {
                                        if let Some(s) = v["delta"]["stop_reason"].as_str() {
                                            st.stop = Some(s.to_string());
                                        }
                                        if let Some(o) = v["usage"]["output_tokens"].as_u64() {
                                            st.usage.tokens_out = o;
                                        }
                                    }
                                    "error" => {
                                        st.done = true;
                                        return Some((
                                            Err(ProviderError::Http {
                                                provider,
                                                status: 500,
                                                body: short_body(&v["error"].to_string()),
                                                retry_after: None,
                                            }),
                                            (events, st, permit),
                                        ));
                                    }
                                    _ => {}
                                }
                            }
                            Some(Err(e)) => {
                                st.done = true;
                                return Some((Err(e), (events, st, permit)));
                            }
                            None => {
                                st.done = true;
                                let open: Vec<(String, usize)> = st
                                    .blocks
                                    .values()
                                    .map(|b| (b.name.clone(), b.arguments.len()))
                                    .collect();
                                for (_, b) in std::mem::take(&mut st.blocks) {
                                    st.queue.push_back(b.finish());
                                }
                                let finish = match st.stop.as_deref() {
                                    Some("tool_use") => "tool_use",
                                    Some("max_tokens") => "length",
                                    _ => "stop",
                                }
                                .to_string();
                                if finish == "length"
                                    && st.text_chars == 0
                                    && st.tool_blocks_closed == 0
                                {
                                    // What did the output tokens go to? Seen
                                    // once (Sonnet 5, 4,000 tokens, corpus-26,
                                    // 2026-09-21) without a record of the
                                    // stream; this names the block types and
                                    // any tool call left open at the cut.
                                    tracing::warn!(
                                        provider = %provider,
                                        model = %model,
                                        output_tokens = st.usage.tokens_out,
                                        blocks = ?st.block_kinds,
                                        open_tool_blocks = ?open,
                                        "max_tokens stop with no text and no finished tool call"
                                    );
                                }
                                st.queue.push_back(GenerateEvent::Usage(st.usage));
                                st.queue.push_back(GenerateEvent::Done {
                                    finish_reason: finish,
                                    stats: final_stats(
                                        &provider, &model, st.usage, &pricing, started, attempts,
                                    ),
                                });
                            }
                        }
                    }
                }
            });
        Ok(Box::pin(stream))
    }

    fn caps(&self) -> VlmCapabilities {
        VlmCapabilities {
            native_video: false,
            native_audio: false,
            max_images_per_request: 20,
            max_image_pixels: 1_568 * 1_568,
            supports_tools: true,
            supports_streaming: true,
            supports_json_schema: false,
            context_tokens: 200_000,
            price: self.pricing,
        }
    }
}

#[async_trait]
impl Llm for Anthropic {
    fn provider_name(&self) -> &str {
        &self.name
    }
    fn model(&self) -> &str {
        &self.model
    }
    fn llm_capabilities(&self) -> VlmCapabilities {
        self.caps()
    }
    async fn generate(&self, req: GenerateRequest) -> Result<EventStream> {
        self.messages(req).await
    }
}

#[async_trait]
impl Vlm for Anthropic {
    fn provider_name(&self) -> &str {
        &self.name
    }
    fn model(&self) -> &str {
        &self.model
    }
    fn vlm_capabilities(&self) -> VlmCapabilities {
        self.caps()
    }
    async fn generate(&self, req: GenerateRequest) -> Result<EventStream> {
        self.messages(req).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::openai_compat::collect_stream;

    #[tokio::test]
    async fn streams_messages_from_a_fake_server() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 1 << 16];
            let mut n = 0;
            loop {
                let r = sock.read(&mut buf[n..]).await.unwrap();
                if r == 0 {
                    break;
                }
                n += r;
                let head = String::from_utf8_lossy(&buf[..n]).to_string();
                if let Some(pos) = head.find("\r\n\r\n") {
                    let len: usize = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length: "))
                        .and_then(|v| v.trim().parse().ok())
                        .unwrap_or(0);
                    if n >= pos + 4 + len {
                        break;
                    }
                }
            }
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            assert!(req.starts_with("POST /v1/messages"), "{req}");
            assert!(req.contains("x-api-key: test-key"));
            assert!(req.contains("\"system\":\"be brief"));
            assert!(req.contains("\"tool_result\""));
            let evs = [
                (
                    "message_start",
                    r#"{"type":"message_start","message":{"usage":{"input_tokens":30}}}"#,
                ),
                (
                    "content_block_start",
                    r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
                ),
                (
                    "content_block_delta",
                    r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Sure"}}"#,
                ),
                (
                    "content_block_stop",
                    r#"{"type":"content_block_stop","index":0}"#,
                ),
                (
                    "content_block_start",
                    r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_1","name":"view","input":{}}}"#,
                ),
                (
                    "content_block_delta",
                    r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"t0\":1"}}"#,
                ),
                (
                    "content_block_delta",
                    r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"0}"}}"#,
                ),
                (
                    "content_block_stop",
                    r#"{"type":"content_block_stop","index":1}"#,
                ),
                (
                    "message_delta",
                    r#"{"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":9}}"#,
                ),
                ("message_stop", r#"{"type":"message_stop"}"#),
            ];
            let mut body = String::new();
            for (e, d) in evs {
                body.push_str(&format!("event: {e}\ndata: {d}\n\n"));
            }
            let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            sock.write_all(resp.as_bytes()).await.unwrap();
            sock.shutdown().await.unwrap();
        });
        std::env::set_var("VI_TEST_ANTHROPIC_KEY", "test-key");
        let cfg = ProviderConfig {
            adapter: "anthropic".into(),
            base_url: Some(format!("http://{addr}")),
            model: Some("claude-sonnet-5".into()),
            api_key_env: Some("VI_TEST_ANTHROPIC_KEY".into()),
            ..ProviderConfig::default()
        };
        let gov = Arc::new(Governor::from_config("a", &cfg));
        let a = Anthropic::from_config("a", &cfg, None, gov, CancellationToken::new()).unwrap();
        let req = GenerateRequest::new(vec![
            Message::text(Role::System, "be brief"),
            Message::text(Role::User, "look at 10s"),
            Message {
                role: Role::Assistant,
                parts: vec![ContentPart::ToolCall {
                    id: "toolu_0".into(),
                    name: "search".into(),
                    arguments: "{\"q\":\"x\"}".into(),
                    signature: None,
                }],
            },
            Message {
                role: Role::Tool,
                parts: vec![ContentPart::ToolResult {
                    call_id: "toolu_0".into(),
                    name: "search".into(),
                    content: "[]".into(),
                }],
            },
        ]);
        let out = collect_stream(Llm::generate(&a, req).await.unwrap())
            .await
            .unwrap();
        assert_eq!(out.text, "Sure");
        assert_eq!(
            out.tool_calls,
            vec![(
                "toolu_1".to_string(),
                "view".to_string(),
                "{\"t0\":10}".to_string()
            )]
        );
        assert_eq!(out.finish_reason, "tool_use");
        assert_eq!((out.usage.tokens_in, out.usage.tokens_out), (30, 9));
        assert!(out.stats.unwrap().cost_usd > 0.0);
    }
}
