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
    http: reqwest::Client,
    governor: Arc<Governor>,
    cancel: CancellationToken,
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
            model,
            http: http_client(name, governor.timeout)?,
            governor,
            cancel,
        })
    }

    fn body(&self, req: &GenerateRequest) -> Result<serde_json::Value> {
        let mut system = Vec::new();
        let mut contents: Vec<serde_json::Value> = Vec::new();
        for m in &req.messages {
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
                        name, arguments, ..
                    } => {
                        let args: serde_json::Value =
                            serde_json::from_str(arguments).unwrap_or_else(|_| json!({}));
                        parts.push(json!({"functionCall": {"name": name, "args": args}}));
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
            match contents.last_mut() {
                Some(last) if last["role"] == role => {
                    if let Some(arr) = last["parts"].as_array_mut() {
                        arr.extend(parts);
                    }
                }
                _ => contents.push(json!({"role": role, "parts": parts})),
            }
        }
        let mut gen = json!({
            "maxOutputTokens": req.max_tokens,
            "temperature": req.temperature,
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
        let timeout = self.governor.timeout.as_secs();
        let (resp, attempts) = retry::run(&self.governor.retry, &self.cancel, |_| {
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
        .await?;
        let provider = self.name.clone();
        let pricing = self.pricing;
        let events = sse::events(resp.bytes_stream(), provider.clone());
        struct St {
            usage: Usage,
            finish: Option<String>,
            calls: u64,
            done: bool,
            queue: std::collections::VecDeque<GenerateEvent>,
        }
        let st = St {
            usage: Usage {
                calls: 1,
                ..Usage::default()
            },
            finish: None,
            calls: 0,
            done: false,
            queue: Default::default(),
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
                                if let Some(err) = v.get("error") {
                                    st.done = true;
                                    return Some((
                                        Err(ProviderError::Http {
                                            provider,
                                            status: err["code"].as_u64().unwrap_or(500) as u16,
                                            body: short_body(&err.to_string()),
                                            retry_after: None,
                                        }),
                                        (events, st, permit),
                                    ));
                                }
                                if let Some(u) = v.get("usageMetadata") {
                                    st.usage.tokens_in = u["promptTokenCount"]
                                        .as_u64()
                                        .unwrap_or(st.usage.tokens_in);
                                    st.usage.tokens_out = u["candidatesTokenCount"]
                                        .as_u64()
                                        .unwrap_or(st.usage.tokens_out);
                                }
                                for cand in v["candidates"].as_array().into_iter().flatten() {
                                    for part in
                                        cand["content"]["parts"].as_array().into_iter().flatten()
                                    {
                                        if let Some(t) = part["text"].as_str() {
                                            if !t.is_empty() {
                                                st.queue.push_back(GenerateEvent::Token {
                                                    text: t.to_string(),
                                                });
                                            }
                                        }
                                        if let Some(fc) = part.get("functionCall") {
                                            st.calls += 1;
                                            st.queue.push_back(GenerateEvent::ToolCall {
                                                id: format!("call_{}", st.calls),
                                                name: fc["name"].as_str().unwrap_or("").to_string(),
                                                arguments: fc["args"].to_string(),
                                            });
                                        }
                                    }
                                    if let Some(f) = cand["finishReason"].as_str() {
                                        st.finish = Some(f.to_string());
                                    }
                                }
                            }
                            Some(Err(e)) => {
                                st.done = true;
                                return Some((Err(e), (events, st, permit)));
                            }
                            None => {
                                st.done = true;
                                let finish = if st.calls > 0 {
                                    "tool_use".to_string()
                                } else {
                                    match st.finish.as_deref() {
                                        Some("MAX_TOKENS") => "length".to_string(),
                                        _ => "stop".to_string(),
                                    }
                                };
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
}
