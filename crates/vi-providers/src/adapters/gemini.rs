//! `gemini`: the Gemini API (`Vlm` with native video and audio as inline
//! data, `Llm` with function calling), streaming over SSE.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use base64::Engine;
use futures::StreamExt;
use serde_json::json;
use tokio_util::sync::CancellationToken;
use vi_core::config::{Pricing, ProviderConfig, RoleBinding};

use crate::adapters::common::{check_status, final_stats, http_client, image_base64, transport};
use crate::adapters::gemini_trace::{raw_dir_from_env, RawTrace};
use crate::cost::Usage;
use crate::error::{short_body, ProviderError, Result};
use crate::governor::Governor;
use crate::pricing::default_pricing;
use crate::retry;
use crate::sse;
use crate::traits::*;

/// Default endpoint.
pub const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";
/// Default model.
pub const DEFAULT_MODEL: &str = "gemini-2.5-flash";
/// Inline data limit (the API takes about 20 MB per request).
pub const MAX_INLINE_BYTES: usize = 19 * 1024 * 1024;

/// The adapter.
pub struct Gemini {
    name: String,
    base_url: String,
    api_key: String,
    model: String,
    pricing: Pricing,
    /// A configured temperature overrides the request's.
    temperature: Option<f32>,
    http: reqwest::Client,
    governor: Arc<Governor>,
    cancel: CancellationToken,
    /// `VI_GEMINI_RAW_DIR` at construction: where each request's shape and
    /// raw SSE lines go (see [`crate::adapters::gemini_trace`]).
    raw_dir: Option<std::path::PathBuf>,
}

impl std::fmt::Debug for Gemini {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gemini")
            .field("name", &self.name)
            .field("model", &self.model)
            .finish()
    }
}

impl Gemini {
    /// Build from config. `api_key_env` defaults to `GEMINI_API_KEY`.
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
            .unwrap_or_else(|| "GEMINI_API_KEY".into());
        let api_key = std::env::var(&var)
            .ok()
            .or_else(|| std::env::var("GOOGLE_API_KEY").ok())
            .filter(|k| !k.is_empty())
            .ok_or_else(|| {
                ProviderError::NotConfigured(format!(
                    "provider '{name}' (gemini) needs the API key in ${var}"
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
            temperature: cfg.temperature,
            model,
            http: http_client(name, governor.timeout)?,
            governor,
            cancel,
            raw_dir: raw_dir_from_env(),
        })
    }

    fn body(&self, req: &GenerateRequest) -> Result<serde_json::Value> {
        let mut system = Vec::new();
        let mut contents: Vec<serde_json::Value> = Vec::new();
        for m in &req.messages {
            // A system message anywhere is hoisted into `systemInstruction`;
            // one between two user messages makes them merge below. The
            // agent sends one, first, so this does not happen today.
            if m.role == Role::System {
                for p in &m.parts {
                    if let ContentPart::Text(t) = p {
                        system.push(json!({"text": t}));
                    }
                }
                continue;
            }
            let role = if m.role == Role::Assistant {
                "model"
            } else {
                "user"
            };
            let mut parts = Vec::new();
            for p in &m.parts {
                match p {
                    ContentPart::Text(t) => parts.push(json!({"text": t})),
                    ContentPart::Image(im) => {
                        let (mime, b64) = image_base64(im)?;
                        parts.push(json!({"inlineData": {"mimeType": mime, "data": b64}}));
                    }
                    ContentPart::Video { mime, bytes, .. } => {
                        if bytes.len() > MAX_INLINE_BYTES {
                            return Err(ProviderError::Invalid(format!(
                                "video clip of {} bytes exceeds the inline limit; file upload arrives later",
                                bytes.len()
                            )));
                        }
                        parts.push(json!({"inlineData": {"mimeType": mime, "data": base64::engine::general_purpose::STANDARD.encode(bytes)}}));
                    }
                    ContentPart::ToolCall {
                        name,
                        arguments,
                        signature,
                        ..
                    } => {
                        let args: serde_json::Value =
                            serde_json::from_str(arguments).unwrap_or_else(|_| json!({}));
                        let mut part = json!({"functionCall": {"name": name, "args": args}});
                        // Gemini 3 rejects a history whose function calls lack
                        // the thought signature it sent with them. The
                        // signature goes back on the `functionCall` part it
                        // came with (the parse keeps it per call, and with
                        // parallel calls only the first carries one); text
                        // parts never get one, because the parse does not
                        // keep a signature that arrives on a text part (the
                        // last part of a text-only answer, often an empty
                        // chunk when streaming). Google calls echoing those
                        // recommended, not required; a final answer is
                        // never sent back except by the `length`
                        // continuation.
                        if let Some(sig) = signature {
                            part["thoughtSignature"] = json!(sig);
                        }
                        parts.push(part);
                    }
                    ContentPart::ToolResult { name, content, .. } => {
                        let response: serde_json::Value = serde_json::from_str(content)
                            .map(|v: serde_json::Value| {
                                if v.is_object() {
                                    v
                                } else {
                                    json!({"result": v})
                                }
                            })
                            .unwrap_or_else(|_| json!({"result": content}));
                        parts.push(
                            json!({"functionResponse": {"name": name, "response": response}}),
                        );
                    }
                }
            }
            // Consecutive messages of one role merge into one entry. In the
            // agent loop that is what puts all the `functionResponse` parts
            // of a turn in one user entry (Gemini wants one response per call
            // in the turn after the calls), and it also appends to that same
            // entry the frames `view` / `zoom` returned (sibling `inlineData`
            // parts after the responses, not inside them) and, on the last
            // turn, the budget / last-turn instruction as a text part. Model
            // entries do not merge today: the loop always puts a tool or user
            // message between two assistant turns (and a session replays
            // alternating user / assistant text), so two turns' signatures
            // never end up in one model entry.
            match contents.last_mut() {
                Some(last) if last["role"] == role => {
                    if let Some(arr) = last["parts"].as_array_mut() {
                        arr.extend(parts);
                    }
                }
                _ => contents.push(json!({"role": role, "parts": parts})),
            }
        }
        // No `thinkingConfig`: the model's default thinking applies and its
        // thoughts are not returned (no `includeThoughts`). Thinking tokens
        // are billed as output and, as far as we know, count against
        // `maxOutputTokens` (a long think can end in `MAX_TOKENS` with little
        // text); `thoughtsTokenCount` in the raw trace's final event shows it.
        let mut gen = json!({
            "maxOutputTokens": req.max_tokens,
            "temperature": self.temperature.unwrap_or(req.temperature),
        });
        if let Some(schema) = &req.json_schema {
            gen["responseMimeType"] = json!("application/json");
            gen["responseSchema"] = strip_schema(schema);
        }
        let mut body = json!({"contents": contents, "generationConfig": gen});
        if !system.is_empty() {
            body["systemInstruction"] = json!({"parts": system});
        }
        if !req.tools.is_empty() {
            body["tools"] = json!([{"functionDeclarations": req.tools.iter().map(|t| json!({
                "name": t.name, "description": t.description, "parameters": strip_schema(&t.parameters)
            })).collect::<Vec<_>>()}]);
            // Tools withheld on the last turn: the declarations stay (the
            // history holds calls) and mode NONE forbids new calls. A model
            // that still wants to call writes the call as text (pseudo code)
            // or ends with `UNEXPECTED_TOOL_CALL` / `MALFORMED_FUNCTION_CALL`,
            // which the parse reports as `unexpected_tool_call` /
            // `malformed_function_call`; the raw trace shows which.
            if req.tool_choice == crate::traits::ToolChoice::None {
                body["toolConfig"] = json!({"functionCallingConfig": {"mode": "NONE"}});
            }
        }
        Ok(body)
    }

    async fn stream(&self, req: GenerateRequest) -> Result<EventStream> {
        let started = Instant::now();
        let permit = self.governor.acquire(req.estimated_tokens()).await?;
        let body = self.body(&req)?;
        let model = req.model.clone().unwrap_or_else(|| self.model.clone());
        let url = format!(
            "{}/models/{}:streamGenerateContent?alt=sse",
            self.base_url, model
        );
        // Raw tracing (no-op unless `vi_providers::gemini::raw` is enabled at
        // debug or finer, or `VI_GEMINI_RAW_DIR` is set). The key travels in
        // the header below, never in the URL, and neither is logged.
        let mut trace = RawTrace::start(&model, &body, self.raw_dir.as_deref());
        let timeout = self.governor.timeout.as_secs();
        let sent = retry::run(&self.governor.retry, &self.cancel, |_| {
            let body = body.clone();
            let url = url.clone();
            async move {
                let resp = self
                    .http
                    .post(&url)
                    .header("x-goog-api-key", &self.api_key)
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| transport(&self.name, timeout, e))?;
                check_status(&self.name, resp).await
            }
        })
        .await;
        let (resp, attempts) = match sent {
            Ok(r) => r,
            Err(e) => {
                trace.error(&e);
                return Err(e);
            }
        };
        let provider = self.name.clone();
        let pricing = self.pricing;
        let events = sse::events(resp.bytes_stream(), provider.clone());
        struct St {
            usage: Usage,
            finish: Option<String>,
            finish_message: Option<String>,
            calls: u64,
            done: bool,
            queue: std::collections::VecDeque<GenerateEvent>,
            trace: RawTrace,
        }
        let st = St {
            usage: Usage {
                calls: 1,
                ..Usage::default()
            },
            finish: None,
            finish_message: None,
            calls: 0,
            done: false,
            queue: Default::default(),
            trace,
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
                                st.trace.line(&ev);
                                let v: serde_json::Value = match serde_json::from_str(&ev.data) {
                                    Ok(v) => v,
                                    Err(e) => {
                                        st.done = true;
                                        let err = ProviderError::Decode {
                                            provider,
                                            message: format!("{e}: {}", short_body(&ev.data)),
                                        };
                                        st.trace.error(&err);
                                        return Some((Err(err), (events, st, permit)));
                                    }
                                };
                                if let Some(err) = v.get("error") {
                                    st.done = true;
                                    let err = ProviderError::Http {
                                        provider,
                                        status: err["code"].as_u64().unwrap_or(500) as u16,
                                        body: short_body(&err.to_string()),
                                        retry_after: None,
                                    };
                                    st.trace.error(&err);
                                    return Some((Err(err), (events, st, permit)));
                                }
                                st.trace.chunk(&v);
                                if let Some(u) = v.get("usageMetadata") {
                                    st.usage.tokens_in = u["promptTokenCount"]
                                        .as_u64()
                                        .unwrap_or(st.usage.tokens_in);
                                    // Thinking tokens are billed as output.
                                    st.usage.tokens_out = u["candidatesTokenCount"]
                                        .as_u64()
                                        .map(|n| n + u["thoughtsTokenCount"].as_u64().unwrap_or(0))
                                        .unwrap_or(st.usage.tokens_out);
                                }
                                // Every candidate is read and its parts go into
                                // one stream (the request never asks for more
                                // than one, so there is one).
                                for (ci, cand) in
                                    v["candidates"].as_array().into_iter().flatten().enumerate()
                                {
                                    let ci = cand["index"].as_u64().unwrap_or(ci as u64);
                                    for part in
                                        cand["content"]["parts"].as_array().into_iter().flatten()
                                    {
                                        let mut token_chars = None;
                                        let mut call_id = None;
                                        // Every non-empty `text` part becomes answer
                                        // text, wherever it sits in the turn: `thought`
                                        // is not read (a thought summary, sent only
                                        // with `includeThoughts`, which we never ask
                                        // for, would be emitted as answer text), and
                                        // text after a `functionCall` is kept too. An
                                        // empty text part is skipped, and with it any
                                        // `thoughtSignature` it carries.
                                        if let Some(t) = part["text"].as_str() {
                                            if !t.is_empty() {
                                                token_chars = Some(t.chars().count());
                                                st.queue.push_back(GenerateEvent::Token {
                                                    text: t.to_string(),
                                                });
                                            }
                                        }
                                        if let Some(fc) = part.get("functionCall") {
                                            st.calls += 1;
                                            let id = format!("call_{}", st.calls);
                                            if st.trace.is_active() {
                                                call_id = Some(id.clone());
                                            }
                                            // `fc["id"]`, when the API sends one, is
                                            // not kept.
                                            st.queue.push_back(GenerateEvent::ToolCall {
                                                id,
                                                name: fc["name"].as_str().unwrap_or("").to_string(),
                                                arguments: fc["args"].to_string(),
                                                signature: part["thoughtSignature"]
                                                    .as_str()
                                                    .map(str::to_string),
                                            });
                                        }
                                        // Parts with neither (`executableCode`, ...)
                                        // are dropped.
                                        st.trace.part(ci, part, token_chars, call_id.as_deref());
                                    }
                                    st.trace.candidate(ci, cand);
                                    if let Some(f) = cand["finishReason"].as_str() {
                                        st.finish = Some(f.to_string());
                                    }
                                    if let Some(m) = cand["finishMessage"].as_str() {
                                        st.finish_message = Some(m.to_string());
                                    }
                                }
                            }
                            Some(Err(e)) => {
                                st.done = true;
                                st.trace.error(&e);
                                return Some((Err(e), (events, st, permit)));
                            }
                            None => {
                                st.done = true;
                                // A tool call anywhere in the turn makes it
                                // `tool_use` whatever the reason; otherwise
                                // each reason keeps its own name (see
                                // `finish_string`), so a `SAFETY` cut or a
                                // `MALFORMED_FUNCTION_CALL` no longer reads as
                                // a normal `stop`.
                                let finish = if st.calls > 0 {
                                    "tool_use".to_string()
                                } else {
                                    finish_string(st.finish.as_deref())
                                };
                                if let Some(f) = st.finish.as_deref() {
                                    if f != "STOP" && f != "MAX_TOKENS" {
                                        tracing::warn!(
                                            provider = %provider,
                                            model = %model,
                                            finish = f,
                                            finish_message =
                                                st.finish_message.as_deref().unwrap_or(""),
                                            reported_as = %finish,
                                            tool_calls = st.calls,
                                            "gemini finishReason other than STOP or MAX_TOKENS"
                                        );
                                    }
                                }
                                st.trace.end(&finish, &st.usage);
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
            native_video: true,
            native_audio: true,
            max_images_per_request: 100,
            max_image_pixels: 3072 * 3072,
            supports_tools: true,
            supports_streaming: true,
            supports_json_schema: true,
            context_tokens: 1_000_000,
            price: self.pricing,
        }
    }
}

/// The finish string for a Gemini `finishReason` in a turn without tool
/// calls: `STOP` (or none) is `stop`, `MAX_TOKENS` is `length`, anything else
/// its own name lower-cased (`safety`, `recitation`,
/// `malformed_function_call`, `unexpected_tool_call`, ...).
fn finish_string(reason: Option<&str>) -> String {
    match reason {
        None | Some("STOP") => "stop".to_string(),
        Some("MAX_TOKENS") => "length".to_string(),
        Some(other) => other.to_ascii_lowercase(),
    }
}

/// Gemini's schema dialect rejects some JSON Schema keywords; drop them.
fn strip_schema(v: &serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(m) => serde_json::Value::Object(
            m.iter()
                .filter(|(k, _)| {
                    !matches!(
                        k.as_str(),
                        "$schema" | "additionalProperties" | "default" | "examples" | "title"
                    )
                })
                .map(|(k, v)| (k.clone(), strip_schema(v)))
                .collect(),
        ),
        serde_json::Value::Array(a) => {
            serde_json::Value::Array(a.iter().map(strip_schema).collect())
        }
        other => other.clone(),
    }
}

#[async_trait]
impl Llm for Gemini {
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
        self.stream(req).await
    }
}

#[async_trait]
impl Vlm for Gemini {
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
        self.stream(req).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::openai_compat::collect_stream;

    /// A `temperature` in the provider's config wins over the request's:
    /// the loop asks every model for 0, which makes the Gemini 3 Pro models
    /// loop (2026-10-02).
    #[test]
    fn a_configured_temperature_overrides_the_request() {
        std::env::set_var("VI_TEST_GEMINI_KEY_T", "g-key");
        let mut cfg = ProviderConfig {
            adapter: "gemini".into(),
            api_key_env: Some("VI_TEST_GEMINI_KEY_T".into()),
            ..ProviderConfig::default()
        };
        let req = GenerateRequest::new(vec![Message::text(Role::User, "hi")]);
        let gov = Arc::new(Governor::from_config("g", &cfg));
        let g =
            Gemini::from_config("g", &cfg, None, gov.clone(), CancellationToken::new()).unwrap();
        assert_eq!(
            g.body(&req).unwrap()["generationConfig"]["temperature"],
            json!(0.0)
        );
        cfg.temperature = Some(1.0);
        let g = Gemini::from_config("g", &cfg, None, gov, CancellationToken::new()).unwrap();
        assert_eq!(
            g.body(&req).unwrap()["generationConfig"]["temperature"],
            json!(1.0)
        );
    }

    #[test]
    fn schema_is_stripped() {
        let s = json!({"type":"object","additionalProperties":false,"properties":{"a":{"type":"string","title":"A"}}});
        let out = strip_schema(&s);
        assert!(out.get("additionalProperties").is_none());
        assert!(out["properties"]["a"].get("title").is_none());
        assert_eq!(out["properties"]["a"]["type"], "string");
    }

    #[tokio::test]
    async fn streams_content_from_a_fake_server() {
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
            assert!(
                req.starts_with(
                    "POST /v1beta/models/gemini-2.5-flash:streamGenerateContent?alt=sse"
                ),
                "{req}"
            );
            assert!(req.contains("x-goog-api-key: g-key"));
            assert!(req.contains("\"inlineData\""));
            assert!(req.contains("\"functionDeclarations\""));
            let chunks = [
                r#"{"candidates":[{"content":{"parts":[{"text":"The "}],"role":"model"}}]}"#,
                r#"{"candidates":[{"content":{"parts":[{"text":"slide"}],"role":"model"},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":100,"candidatesTokenCount":2}}"#,
            ];
            let mut body = String::new();
            for c in chunks {
                body.push_str(&format!("data: {c}\r\n\r\n"));
            }
            let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            sock.write_all(resp.as_bytes()).await.unwrap();
            sock.shutdown().await.unwrap();
        });
        std::env::set_var("VI_TEST_GEMINI_KEY", "g-key");
        let cfg = ProviderConfig {
            adapter: "gemini".into(),
            base_url: Some(format!("http://{addr}/v1beta")),
            api_key_env: Some("VI_TEST_GEMINI_KEY".into()),
            ..ProviderConfig::default()
        };
        let gov = Arc::new(Governor::from_config("g", &cfg));
        let g = Gemini::from_config("g", &cfg, None, gov, CancellationToken::new()).unwrap();
        let mut req = GenerateRequest::new(vec![Message {
            role: Role::User,
            parts: vec![
                ContentPart::Text("describe".into()),
                ContentPart::Video {
                    mime: "video/mp4",
                    bytes: bytes::Bytes::from_static(b"\x00\x00\x00\x18ftyp"),
                    duration_secs: 3.0,
                },
            ],
        }]);
        req.tools.push(ToolSpec {
            name: "search".into(),
            description: "s".into(),
            parameters: json!({"type":"object","additionalProperties":false}),
        });
        let out = collect_stream(Vlm::generate(&g, req).await.unwrap())
            .await
            .unwrap();
        assert_eq!(out.text, "The slide");
        assert_eq!(out.finish_reason, "stop");
        assert_eq!((out.usage.tokens_in, out.usage.tokens_out), (100, 2));
        assert!(g.vlm_capabilities().native_video);
        assert!(out.stats.unwrap().cost_usd > 0.0);
    }

    /// Gemini 3 sends a thought signature with each function call and
    /// rejects a later turn whose history lacks it, so the signature rides
    /// on the tool call and comes back on the `functionCall` part.
    #[test]
    fn thought_signatures_round_trip_through_the_history() {
        std::env::set_var("VI_TEST_GEMINI_KEY2", "g-key");
        let cfg = ProviderConfig {
            adapter: "gemini".into(),
            api_key_env: Some("VI_TEST_GEMINI_KEY2".into()),
            ..ProviderConfig::default()
        };
        let gov = Arc::new(Governor::from_config("g", &cfg));
        let g = Gemini::from_config("g", &cfg, None, gov, CancellationToken::new()).unwrap();
        let req = GenerateRequest::new(vec![
            Message::text(Role::User, "find it"),
            Message {
                role: Role::Assistant,
                parts: vec![
                    ContentPart::ToolCall {
                        id: "call_1".into(),
                        name: "search".into(),
                        arguments: "{\"query\":\"x\"}".into(),
                        signature: Some("sig-abc".into()),
                    },
                    ContentPart::ToolCall {
                        id: "call_2".into(),
                        name: "list_videos".into(),
                        arguments: "{}".into(),
                        signature: None,
                    },
                ],
            },
        ]);
        let body = g.body(&req).unwrap();
        let parts = body["contents"][1]["parts"].as_array().unwrap();
        assert_eq!(parts[0]["thoughtSignature"], "sig-abc");
        assert_eq!(parts[0]["functionCall"]["name"], "search");
        assert!(parts[1].get("thoughtSignature").is_none());
    }

    #[tokio::test]
    async fn function_calls_carry_their_signature_and_thinking_counts_as_output() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 65536];
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
            let chunk = r#"{"candidates":[{"content":{"parts":[{"functionCall":{"name":"search","args":{"query":"x"}},"thoughtSignature":"sig-xyz"}],"role":"model"},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":100,"candidatesTokenCount":5,"thoughtsTokenCount":40}}"#;
            let body = format!("data: {chunk}\r\n\r\n");
            let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            sock.write_all(resp.as_bytes()).await.unwrap();
            sock.shutdown().await.unwrap();
        });
        std::env::set_var("VI_TEST_GEMINI_KEY3", "g-key");
        let cfg = ProviderConfig {
            adapter: "gemini".into(),
            base_url: Some(format!("http://{addr}/v1beta")),
            api_key_env: Some("VI_TEST_GEMINI_KEY3".into()),
            model: Some("gemini-3.8-flash".into()),
            ..ProviderConfig::default()
        };
        let gov = Arc::new(Governor::from_config("g", &cfg));
        let g = Gemini::from_config("g", &cfg, None, gov, CancellationToken::new()).unwrap();
        let req = GenerateRequest::new(vec![Message::text(Role::User, "find it")]);
        let mut stream = Llm::generate(&g, req).await.unwrap();
        let mut signature = None;
        let mut usage = Usage::default();
        while let Some(ev) = stream.next().await {
            match ev.unwrap() {
                GenerateEvent::ToolCall {
                    name, signature: s, ..
                } => {
                    assert_eq!(name, "search");
                    signature = s;
                }
                GenerateEvent::Usage(u) => usage = u,
                _ => {}
            }
        }
        assert_eq!(signature.as_deref(), Some("sig-xyz"));
        assert_eq!((usage.tokens_in, usage.tokens_out), (100, 45));
    }

    // ---------------------------------------------------------------------
    // Raw tracing (`gemini_trace`): captured events and the raw file.

    use crate::adapters::gemini_trace::{self, TARGET};
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    /// The key every tracing test sends; it must never show up in a log line
    /// or a raw file.
    const SECRET: &str = "g-key-secret-7Qx9";

    /// One captured event: target, level and fields rendered as text.
    #[derive(Debug, Clone)]
    struct Ev {
        target: String,
        level: tracing::Level,
        fields: BTreeMap<String, String>,
    }

    impl Ev {
        fn msg(&self) -> &str {
            self.fields.get("message").map_or("", String::as_str)
        }
        fn json(&self, field: &str) -> serde_json::Value {
            serde_json::from_str(&self.fields[field]).unwrap()
        }
    }

    #[derive(Clone, Default)]
    struct Captured(Arc<Mutex<Vec<Ev>>>);

    impl Captured {
        fn raw(&self) -> Vec<Ev> {
            self.0
                .lock()
                .unwrap()
                .iter()
                .filter(|e| e.target == TARGET)
                .cloned()
                .collect()
        }
        fn named(&self, message: &str) -> Vec<Ev> {
            self.raw()
                .into_iter()
                .filter(|e| e.msg() == message)
                .collect()
        }
        fn debug_containing(&self, needle: &str) -> Vec<Ev> {
            self.raw()
                .into_iter()
                .filter(|e| e.level == tracing::Level::DEBUG && e.msg().contains(needle))
                .collect()
        }
        fn all_text(&self) -> String {
            format!("{:?}", self.0.lock().unwrap())
        }
    }

    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Captured {
        fn on_event(
            &self,
            event: &tracing::Event<'_>,
            _ctx: tracing_subscriber::layer::Context<'_, S>,
        ) {
            struct V<'a>(&'a mut BTreeMap<String, String>);
            impl tracing::field::Visit for V<'_> {
                fn record_str(&mut self, f: &tracing::field::Field, v: &str) {
                    self.0.insert(f.name().to_string(), v.to_string());
                }
                fn record_debug(&mut self, f: &tracing::field::Field, v: &dyn std::fmt::Debug) {
                    self.0.insert(f.name().to_string(), format!("{v:?}"));
                }
            }
            let mut fields = BTreeMap::new();
            event.record(&mut V(&mut fields));
            self.0.lock().unwrap().push(Ev {
                target: event.metadata().target().to_string(),
                level: *event.metadata().level(),
                fields,
            });
        }
    }

    /// A thread-local subscriber built the way `vidx --log FILTER` builds
    /// its own (`crates/vi-cli/src/main.rs`), plus a capturing layer.
    fn capture(filter: &str) -> (Captured, tracing::subscriber::DefaultGuard) {
        use tracing_subscriber::layer::SubscriberExt;
        let cap = Captured::default();
        let filter = tracing_subscriber::EnvFilter::try_new(filter).unwrap();
        let sub = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::sink)
            .finish()
            .with(cap.clone());
        (cap, tracing::subscriber::set_default(sub))
    }

    fn sse_body(chunks: &[&str]) -> String {
        chunks
            .iter()
            .map(|c| format!("data: {c}\r\n\r\n"))
            .collect()
    }

    /// A one-shot HTTP server: reads one request, answers `status` with
    /// `body`, returns the request text.
    async fn fake_gemini(
        status: &'static str,
        body: String,
    ) -> (std::net::SocketAddr, tokio::task::JoinHandle<String>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0u8; 16384];
            loop {
                let r = sock.read(&mut chunk).await.unwrap();
                if r == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..r]);
                let head = String::from_utf8_lossy(&buf).to_string();
                if let Some(pos) = head.find("\r\n\r\n") {
                    let len: usize = head[..pos]
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .map(|v| v.trim().to_string())
                        })
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                    if buf.len() >= pos + 4 + len {
                        break;
                    }
                }
            }
            let resp = format!(
                "HTTP/1.1 {status}\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
            sock.shutdown().await.unwrap();
            String::from_utf8_lossy(&buf).to_string()
        });
        (addr, handle)
    }

    fn adapter_at(addr: std::net::SocketAddr, key_env: &str) -> Gemini {
        std::env::set_var(key_env, SECRET);
        let cfg = ProviderConfig {
            adapter: "gemini".into(),
            base_url: Some(format!("http://{addr}/v1beta")),
            api_key_env: Some(key_env.into()),
            model: Some("gemini-3.1-pro-preview".into()),
            ..ProviderConfig::default()
        };
        let gov = Arc::new(Governor::from_config("g", &cfg));
        Gemini::from_config("g", &cfg, None, gov, CancellationToken::new()).unwrap()
    }

    /// 3000 bytes that are not a real JPEG; only their length is logged.
    fn fake_jpeg() -> ContentPart {
        let bytes: Vec<u8> = (0..3000u32).map(|i| (i * 7 % 251) as u8).collect();
        ContentPart::Image(ImageData::Encoded {
            mime: "image/jpeg",
            bytes: bytes::Bytes::from(bytes),
        })
    }

    fn fake_jpeg_base64() -> String {
        match fake_jpeg() {
            ContentPart::Image(im) => image_base64(&im).unwrap().1,
            _ => unreachable!(),
        }
    }

    /// A Gemini 3 text answer as it streams: text chunks, then an empty
    /// text part carrying the thought signature with the finish reason.
    const TEXT_ANSWER: [&str; 3] = [
        r#"{"candidates":[{"content":{"parts":[{"text":"The "}],"role":"model"},"index":0}],"modelVersion":"gemini-3.1-pro-preview"}"#,
        r#"{"candidates":[{"content":{"parts":[{"text":"slide"}],"role":"model"},"index":0}]}"#,
        r#"{"candidates":[{"content":{"parts":[{"text":"","thoughtSignature":"c2lnLXRleHQ="}],"role":"model"},"finishReason":"STOP","index":0}],"usageMetadata":{"promptTokenCount":100,"candidatesTokenCount":2,"thoughtsTokenCount":30}}"#,
    ];

    /// The directive `vidx --log vi_providers::gemini::raw=trace` enables
    /// the raw events (with or without a default level before it);
    /// `vi_agent=debug` alone enables none and leaves the tracer off;
    /// `vi_providers::gemini::raw=debug` gives the parse decisions only.
    #[tokio::test]
    async fn the_raw_events_need_their_own_directive() {
        for (filter, want_trace, want_debug) in [
            ("vi_providers::gemini::raw=trace", true, true),
            ("info,vi_providers::gemini::raw=trace", true, true),
            ("vi_agent=debug", false, false),
            ("info", false, false),
            ("vi_providers::gemini::raw=debug", false, true),
        ] {
            let (cap, _guard) = capture(filter);
            let (addr, server) = fake_gemini("200 OK", sse_body(&TEXT_ANSWER)).await;
            let g = adapter_at(addr, "VI_TEST_GEMINI_KEY_F");
            let out = collect_stream(
                Llm::generate(
                    &g,
                    GenerateRequest::new(vec![Message::text(Role::User, "q")]),
                )
                .await
                .unwrap(),
            )
            .await
            .unwrap();
            server.await.unwrap();
            assert_eq!(out.text, "The slide", "{filter}");
            let raw = cap.raw();
            let traces = raw
                .iter()
                .filter(|e| e.level == tracing::Level::TRACE)
                .count();
            let debugs = raw
                .iter()
                .filter(|e| e.level == tracing::Level::DEBUG)
                .count();
            assert_eq!(traces > 0, want_trace, "{filter}: {raw:?}");
            assert_eq!(debugs > 0, want_debug, "{filter}: {raw:?}");
            if want_trace {
                assert_eq!(cap.named("gemini raw request").len(), 1, "{filter}");
                assert_eq!(cap.named("gemini raw sse").len(), 3, "{filter}");
                assert_eq!(cap.named("gemini raw final").len(), 1, "{filter}");
            }
        }
    }

    /// (a) A plain answer: tokens stream as before; the final event lists
    /// one text part (three chunks merged, the last an empty carrier of the
    /// signature, which is not kept) and what was emitted.
    #[tokio::test]
    async fn a_normal_answer_streams_its_tokens_and_the_final_lists_one_text_part() {
        let (cap, _guard) = capture("vi_providers::gemini::raw=trace");
        let (addr, server) = fake_gemini("200 OK", sse_body(&TEXT_ANSWER)).await;
        let g = adapter_at(addr, "VI_TEST_GEMINI_KEY_A");
        let out = collect_stream(
            Llm::generate(
                &g,
                GenerateRequest::new(vec![Message::text(Role::User, "q")]),
            )
            .await
            .unwrap(),
        )
        .await
        .unwrap();
        let req = server.await.unwrap();
        assert!(req.contains(&format!("x-goog-api-key: {SECRET}")));
        assert!(
            !req.lines().next().unwrap().contains(SECRET),
            "key not in the URL"
        );
        assert_eq!(out.text, "The slide");
        assert_eq!(out.finish_reason, "stop");
        // The SSE lines are logged verbatim.
        let lines: Vec<String> = cap
            .named("gemini raw sse")
            .iter()
            .map(|e| e.fields["line"].clone())
            .collect();
        assert_eq!(lines, TEXT_ANSWER.map(str::to_string).to_vec());
        let parts = cap.named("gemini raw part");
        assert_eq!(parts.len(), 1, "{parts:?}");
        let p = parts[0].json("part");
        assert_eq!(p["kind"], "text");
        assert_eq!(p["chars"], 9);
        assert_eq!(p["chunks"], 3);
        assert_eq!(p["empty_chunks"], 1);
        assert_eq!(p["thought"], false);
        assert_eq!(p["thoughtSignature"], 12);
        assert_eq!(p["preview"], "The slide");
        assert_eq!(p["emitted"]["tokens"], 2);
        let f = cap.named("gemini raw final")[0].json("summary");
        assert_eq!(f["emitted"]["tokens"], 2);
        assert_eq!(f["emitted"]["chars"], 9);
        assert_eq!(f["emitted"]["tool_calls"], 0);
        assert_eq!(f["emitted"]["finish_reason"], "stop");
        assert_eq!(f["finishReason"]["0"], "STOP");
        assert_eq!(f["usageMetadata"]["thoughtsTokenCount"], 30);
        assert_eq!(f["modelVersion"], "gemini-3.1-pro-preview");
        assert_eq!(f["sse_lines"], 3);
        assert!(f["dropped"][0]
            .as_str()
            .unwrap()
            .contains("thoughtSignature on a text part"));
        assert_eq!(
            cap.debug_containing("thoughtSignature on a text part")
                .len(),
            1
        );
        let shape = cap.named("gemini raw request")[0].json("shape");
        assert_eq!(shape["model"], "gemini-3.1-pro-preview");
        assert_eq!(shape["contents"][0]["parts"][0], "text(1)");
        assert!(!cap.all_text().contains(SECRET));
    }

    /// (b) A function call followed by a text part: one tool call, the text
    /// still becomes answer text, and the trace lists both parts.
    #[tokio::test]
    async fn a_call_then_text_yields_one_tool_call_and_the_trace_lists_both_parts() {
        let (cap, _guard) = capture("vi_providers::gemini::raw=trace");
        let chunks = [
            r#"{"candidates":[{"content":{"parts":[{"functionCall":{"name":"search","args":{"query":"x"}},"thoughtSignature":"c2lnLWNhbGw="}],"role":"model"},"index":0}]}"#,
            r#"{"candidates":[{"content":{"parts":[{"text":"Looking it up."}],"role":"model"},"finishReason":"STOP","index":0}],"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":9}}"#,
        ];
        let (addr, server) = fake_gemini("200 OK", sse_body(&chunks)).await;
        let g = adapter_at(addr, "VI_TEST_GEMINI_KEY_B");
        let out = collect_stream(
            Llm::generate(
                &g,
                GenerateRequest::new(vec![Message::text(Role::User, "q")]),
            )
            .await
            .unwrap(),
        )
        .await
        .unwrap();
        server.await.unwrap();
        assert_eq!(out.tool_calls.len(), 1);
        assert_eq!(out.tool_calls[0].1, "search");
        assert_eq!(out.text, "Looking it up.");
        assert_eq!(out.finish_reason, "tool_use");
        let parts: Vec<serde_json::Value> = cap
            .named("gemini raw part")
            .iter()
            .map(|e| e.json("part"))
            .collect();
        assert_eq!(parts.len(), 2, "{parts:?}");
        assert_eq!(parts[0]["kind"], "functionCall:search");
        assert_eq!(parts[0]["thoughtSignature"], 12);
        assert_eq!(parts[0]["emitted"]["tool_call"], "call_1");
        assert_eq!(parts[1]["kind"], "text");
        assert_eq!(parts[1]["emitted"]["tokens"], 1);
        assert_eq!(cap.debug_containing("after a functionCall").len(), 1);
        let f = cap.named("gemini raw final")[0].json("summary");
        assert_eq!(f["emitted"]["tool_calls"], 1);
        assert_eq!(f["emitted"]["finish_reason"], "tool_use");
    }

    /// (c) CURRENT BEHAVIOUR, NOT A SPEC: a text part marked `thought: true`
    /// is emitted as answer text, glued to the answer. The API sends thought
    /// parts only when `thinkingConfig.includeThoughts` is set, which `body()`
    /// never does; if a response ever carries one, it lands in the answer.
    #[tokio::test]
    async fn a_thought_part_is_emitted_as_answer_text_today() {
        let (cap, _guard) = capture("vi_providers::gemini::raw=trace");
        let chunks = [
            r#"{"candidates":[{"content":{"parts":[{"text":"Weighing the options, B fits.","thought":true}],"role":"model"},"index":0}]}"#,
            r#"{"candidates":[{"content":{"parts":[{"text":"The answer is B."}],"role":"model"},"finishReason":"STOP","index":0}]}"#,
        ];
        let (addr, server) = fake_gemini("200 OK", sse_body(&chunks)).await;
        let g = adapter_at(addr, "VI_TEST_GEMINI_KEY_C");
        let out = collect_stream(
            Llm::generate(
                &g,
                GenerateRequest::new(vec![Message::text(Role::User, "q")]),
            )
            .await
            .unwrap(),
        )
        .await
        .unwrap();
        server.await.unwrap();
        assert_eq!(out.text, "Weighing the options, B fits.The answer is B.");
        let parts: Vec<serde_json::Value> = cap
            .named("gemini raw part")
            .iter()
            .map(|e| e.json("part"))
            .collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0]["thought"], true);
        assert_eq!(parts[0]["emitted"]["tokens"], 1);
        assert_eq!(parts[1]["thought"], false);
        assert_eq!(cap.debug_containing("thought:true").len(), 1);
    }

    /// (d) The request shape of a multi-turn history with a frame and a
    /// thought signature: MIME type and byte length, signature length, the
    /// merged user turn, mode NONE; no base64 bytes and no key anywhere,
    /// neither in the shape nor in the elided body the raw file gets.
    #[test]
    fn the_request_shape_names_parts_without_bytes_or_key() {
        std::env::set_var("VI_TEST_GEMINI_KEY_D", SECRET);
        let cfg = ProviderConfig {
            adapter: "gemini".into(),
            api_key_env: Some("VI_TEST_GEMINI_KEY_D".into()),
            model: Some("gemini-3.1-pro-preview".into()),
            ..ProviderConfig::default()
        };
        let gov = Arc::new(Governor::from_config("g", &cfg));
        let g = Gemini::from_config("g", &cfg, None, gov, CancellationToken::new()).unwrap();
        let mut req = GenerateRequest::new(vec![
            Message::text(Role::System, "You answer questions about videos."),
            Message::text(Role::User, "What is on the slide?"),
            Message {
                role: Role::Assistant,
                parts: vec![
                    ContentPart::Text("Let me look.".into()),
                    ContentPart::ToolCall {
                        id: "call_1".into(),
                        name: "view".into(),
                        arguments: "{\"t0\":10,\"t1\":20}".into(),
                        signature: Some("SIGNATURE-0123456789".into()),
                    },
                ],
            },
            Message {
                role: Role::Tool,
                parts: vec![ContentPart::ToolResult {
                    call_id: "call_1".into(),
                    name: "view".into(),
                    content: "{\"frames\":6}".into(),
                }],
            },
            Message {
                role: Role::User,
                parts: vec![fake_jpeg()],
            },
            Message::text(
                Role::User,
                "You have used 1 of 1 tool calls; no further calls are available.",
            ),
        ]);
        req.tools.push(ToolSpec {
            name: "view".into(),
            description: "v".into(),
            parameters: json!({"type":"object"}),
        });
        req.tool_choice = ToolChoice::None;
        let body = g.body(&req).unwrap();
        let shape = gemini_trace::request_shape("gemini-3.1-pro-preview", &body);
        assert_eq!(shape["tools"], 1);
        assert_eq!(shape["functionCallingConfig"], "NONE");
        assert_eq!(shape["generationConfig"]["maxOutputTokens"], 1024);
        assert_eq!(shape["system_chars"], 34);
        let contents = shape["contents"].as_array().unwrap();
        // The tool result, the frame and the last-turn text share one entry.
        assert_eq!(contents.len(), 3, "{shape}");
        assert_eq!(contents[0]["role"], "user");
        assert_eq!(contents[1]["role"], "model");
        assert_eq!(
            contents[1]["parts"],
            json!(["text(12)", "functionCall:view sig(20)"])
        );
        assert_eq!(contents[2]["role"], "user");
        assert_eq!(contents[2]["n"], 3);
        assert_eq!(
            contents[2]["parts"],
            json!([
                "functionResponse:view(12)",
                "inlineData:image/jpeg(3000B)",
                "text(64)"
            ])
        );
        let b64 = fake_jpeg_base64();
        assert!(
            body.to_string().contains(&b64),
            "the request itself carries the bytes"
        );
        let elided = gemini_trace::elide_inline_data(&body);
        assert_eq!(
            elided["contents"][2]["parts"][1]["inlineData"]["data"],
            "<3000 bytes elided>"
        );
        for text in [shape.to_string(), elided.to_string()] {
            assert!(!text.contains(&b64[..40]));
            assert!(!text.contains(SECRET));
        }
    }

    /// (e) With `VI_GEMINI_RAW_DIR` set, a request writes
    /// `<dir>/<timestamp>-<n>.jsonl`: the request line (shape and the body
    /// with inline data elided), one line per SSE event, the final line.
    #[tokio::test]
    async fn the_raw_dir_gets_request_sse_and_final_lines() {
        let dir = tempfile::tempdir().unwrap();
        let chunks = [
            r#"{"candidates":[{"content":{"parts":[{"text":"A red "}],"role":"model"},"index":0}]}"#,
            r#"{"candidates":[{"content":{"parts":[{"text":"car."}],"role":"model"},"finishReason":"MAX_TOKENS","index":0}],"usageMetadata":{"promptTokenCount":1200,"candidatesTokenCount":3,"thoughtsTokenCount":3990}}"#,
        ];
        let (addr, server) = fake_gemini("200 OK", sse_body(&chunks)).await;
        std::env::set_var(gemini_trace::RAW_DIR_ENV, dir.path());
        let g = adapter_at(addr, "VI_TEST_GEMINI_KEY_E");
        std::env::remove_var(gemini_trace::RAW_DIR_ENV);
        assert_eq!(g.raw_dir.as_deref(), Some(dir.path()));
        let req = GenerateRequest::new(vec![Message {
            role: Role::User,
            parts: vec![ContentPart::Text("what colour?".into()), fake_jpeg()],
        }]);
        let out = collect_stream(Llm::generate(&g, req).await.unwrap())
            .await
            .unwrap();
        server.await.unwrap();
        assert_eq!(out.text, "A red car.");
        assert_eq!(out.finish_reason, "length");
        // Other tests may build an adapter while the variable is set; pick
        // this test's file by its first user text length and image.
        let files: Vec<std::path::PathBuf> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                std::fs::read_to_string(p)
                    .unwrap()
                    .contains("inlineData:image/jpeg(3000B)")
            })
            .collect();
        assert_eq!(files.len(), 1, "{files:?}");
        let name = files[0].file_name().unwrap().to_string_lossy().to_string();
        assert!(
            name.ends_with(".jsonl") && name.contains('T') && name.contains('-'),
            "{name}"
        );
        let text = std::fs::read_to_string(&files[0]).unwrap();
        assert!(!text.contains(SECRET));
        assert!(!text.contains(&fake_jpeg_base64()[..40]));
        let lines: Vec<serde_json::Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 4, "{text}");
        assert_eq!(lines[0]["kind"], "request");
        assert_eq!(lines[0]["model"], "gemini-3.1-pro-preview");
        assert_eq!(
            lines[0]["shape"]["contents"][0]["parts"],
            json!(["text(12)", "inlineData:image/jpeg(3000B)"])
        );
        assert_eq!(
            lines[0]["body"]["contents"][0]["parts"][1]["inlineData"]["data"],
            "<3000 bytes elided>"
        );
        assert_eq!(lines[1]["kind"], "sse");
        assert_eq!(lines[1]["line"], chunks[0]);
        assert_eq!(lines[2]["line"], chunks[1]);
        assert_eq!(lines[3]["kind"], "final");
        assert_eq!(lines[3]["finishReason"]["0"], "MAX_TOKENS");
        assert_eq!(lines[3]["emitted"]["finish_reason"], "length");
        assert_eq!(lines[3]["emitted"]["tokens"], 2);
        assert_eq!(lines[3]["usageMetadata"]["thoughtsTokenCount"], 3990);
        assert_eq!(lines[3]["parts"].as_array().unwrap().len(), 1);
        assert_eq!(lines[3]["parts"][0]["preview"], "A red car.");
    }

    /// A refused request writes the redacted error and a final line.
    #[tokio::test]
    async fn an_http_error_is_written_to_the_raw_file() {
        let dir = tempfile::tempdir().unwrap();
        let (addr, server) = fake_gemini(
            "400 Bad Request",
            r#"{"error":{"code":400,"message":"Function call is missing a thought_signature","status":"INVALID_ARGUMENT"}}"#.into(),
        )
        .await;
        let mut g = adapter_at(addr, "VI_TEST_GEMINI_KEY_H");
        g.raw_dir = Some(dir.path().to_path_buf());
        let err = Llm::generate(
            &g,
            GenerateRequest::new(vec![Message::text(Role::User, "q")]),
        )
        .await
        .err()
        .unwrap();
        server.await.unwrap();
        assert!(err.to_string().contains("thought_signature"));
        let file = std::fs::read_dir(dir.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let text = std::fs::read_to_string(file).unwrap();
        let kinds: Vec<String> = text
            .lines()
            .map(|l| {
                serde_json::from_str::<serde_json::Value>(l).unwrap()["kind"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(kinds, ["request", "error", "final"]);
        assert!(text.contains("missing a thought_signature"));
        assert!(!text.contains(SECRET));
    }

    /// A turn that ends in `MALFORMED_FUNCTION_CALL` or `UNEXPECTED_TOOL_CALL`
    /// with no parts reports that reason (lower-cased) and no tokens; it no
    /// longer reads as a normal `stop`.
    #[tokio::test]
    async fn a_malformed_or_unexpected_call_keeps_its_finish_reason() {
        for (reason, content, want) in [
            (
                "MALFORMED_FUNCTION_CALL",
                r#""content":{"role":"model"},"finishMessage":"Malformed function call: print(default_api.search(query='x'))","#,
                "malformed_function_call",
            ),
            ("UNEXPECTED_TOOL_CALL", "", "unexpected_tool_call"),
        ] {
            let chunk = format!(
                r#"{{"candidates":[{{{content}"finishReason":"{reason}","index":0}}],"usageMetadata":{{"promptTokenCount":5000,"candidatesTokenCount":0,"thoughtsTokenCount":700}}}}"#
            );
            let (addr, server) = fake_gemini("200 OK", sse_body(&[&chunk])).await;
            let g = adapter_at(addr, "VI_TEST_GEMINI_KEY_M");
            let out = collect_stream(
                Llm::generate(
                    &g,
                    GenerateRequest::new(vec![Message::text(Role::User, "q")]),
                )
                .await
                .unwrap(),
            )
            .await
            .unwrap();
            server.await.unwrap();
            assert_eq!(out.finish_reason, want, "{reason}");
            assert_eq!(out.text, "", "{reason}");
            assert!(out.tool_calls.is_empty(), "{reason}");
            assert_eq!(out.usage.tokens_out, 700, "{reason}");
        }
    }

    #[test]
    fn finish_reasons_keep_their_names() {
        assert_eq!(finish_string(None), "stop");
        assert_eq!(finish_string(Some("STOP")), "stop");
        assert_eq!(finish_string(Some("MAX_TOKENS")), "length");
        assert_eq!(finish_string(Some("SAFETY")), "safety");
        assert_eq!(finish_string(Some("RECITATION")), "recitation");
        assert_eq!(
            finish_string(Some("MALFORMED_FUNCTION_CALL")),
            "malformed_function_call"
        );
        assert_eq!(
            finish_string(Some("UNEXPECTED_TOOL_CALL")),
            "unexpected_tool_call"
        );
        assert_eq!(finish_string(Some("OTHER")), "other");
    }

    /// `vidx` loads its config with `VI_*` overrides that reject unknown
    /// keys; the raw-dir variable is on the list they skip.
    #[test]
    fn the_raw_dir_variable_is_not_read_as_a_config_key() {
        let key = gemini_trace::RAW_DIR_ENV
            .strip_prefix(vi_core::config::ENV_PREFIX)
            .unwrap();
        assert!(vi_core::config::ENV_NOT_CONFIG.contains(&key));
    }

    /// An SSE line longer than 20,000 characters is cut and flagged.
    #[test]
    fn a_long_line_is_cut_and_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let mut t = gemini_trace::RawTrace::start("m", &json!({"contents": []}), Some(dir.path()));
        assert!(t.is_active());
        let path = t.path().unwrap().to_path_buf();
        t.line(&crate::sse::SseEvent {
            event: String::new(),
            data: "é".repeat(25_000),
        });
        drop(t);
        let text = std::fs::read_to_string(path).unwrap();
        let lines: Vec<serde_json::Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines[1]["kind"], "sse");
        assert_eq!(lines[1]["truncated"], true);
        assert_eq!(lines[1]["chars"], 25_000);
        assert_eq!(lines[1]["line"].as_str().unwrap().chars().count(), 20_000);
        // Dropped before its end: the final line says so.
        assert_eq!(lines[2]["kind"], "final");
        assert!(lines[2]["emitted"]["finish_reason"]
            .as_str()
            .unwrap()
            .starts_with("abandoned"));
    }
}
