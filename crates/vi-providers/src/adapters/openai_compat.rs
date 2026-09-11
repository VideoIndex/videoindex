//! `openai_compat`: the OpenAI HTTP API shape, which vLLM, Ollama, LM
//! Studio, llama.cpp, Groq, Together and Whisper servers all speak.
//!
//! M1 implements the audio transcription endpoint (`Asr`). Chat completions
//! and embeddings arrive in M2; until then those traits return
//! [`ProviderError::Unsupported`].

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use reqwest::multipart;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;
use vi_core::config::{Pricing, ProviderConfig, RoleBinding};

use crate::cost::{CallStats, Usage};
use crate::error::{redact, short_body, ProviderError, Result};
use crate::governor::Governor;
use crate::retry;
use crate::traits::*;

/// Longest clip the transcription endpoint takes; OpenAI's limit is 25 MB
/// (about 13 minutes of 16 kHz PCM). Local servers accept more, but the
/// operator chunks well below this anyway.
pub const MAX_AUDIO_SECS: f64 = 30.0 * 60.0;

/// An OpenAI-compatible endpoint.
pub struct OpenAiCompat {
    name: String,
    base_url: String,
    api_key: Option<String>,
    model: String,
    pricing: Pricing,
    http: reqwest::Client,
    governor: Arc<Governor>,
    cancel: CancellationToken,
}

impl std::fmt::Debug for OpenAiCompat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAiCompat")
            .field("name", &self.name)
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("api_key", &self.api_key.as_ref().map(|_| "[set]"))
            .finish()
    }
}

impl OpenAiCompat {
    /// Build from a provider table and an optional role binding (model and
    /// base URL overrides). The API key is read from `api_key_env` now and
    /// kept only in memory.
    pub fn from_config(
        name: &str,
        cfg: &ProviderConfig,
        role: Option<&RoleBinding>,
        governor: Arc<Governor>,
        cancel: CancellationToken,
    ) -> Result<Self> {
        let base_url = role
            .and_then(|r| r.base_url.clone())
            .or_else(|| cfg.base_url.clone())
            .ok_or_else(|| {
                ProviderError::NotConfigured(format!(
                    "provider '{name}' (openai_compat) needs base_url"
                ))
            })?;
        let model = role
            .and_then(|r| r.model.clone())
            .or_else(|| cfg.model.clone())
            .unwrap_or_else(|| "default".to_string());
        let api_key = cfg
            .api_key_env
            .as_ref()
            .and_then(|var| std::env::var(var).ok())
            .filter(|k| !k.is_empty());
        let http = reqwest::Client::builder()
            .timeout(governor.timeout)
            .connect_timeout(std::time::Duration::from_secs(20))
            .user_agent(concat!("videoindex/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| ProviderError::Transport {
                provider: name.to_string(),
                message: e.to_string(),
            })?;
        Ok(Self {
            name: name.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            model,
            pricing: cfg.pricing.unwrap_or_default(),
            http,
            governor,
            cancel,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    fn transport(&self, e: reqwest::Error) -> ProviderError {
        if e.is_timeout() {
            ProviderError::Timeout {
                provider: self.name.clone(),
                secs: self.governor.timeout.as_secs(),
            }
        } else {
            ProviderError::Transport {
                provider: self.name.clone(),
                message: redact(&e.without_url().to_string()),
            }
        }
    }

    async fn check_status(&self, resp: reqwest::Response) -> Result<reqwest::Response> {
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let retry_after = resp
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(std::time::Duration::from_secs);
        let body = resp.text().await.unwrap_or_default();
        Err(ProviderError::Http {
            provider: self.name.clone(),
            status: status.as_u16(),
            body: short_body(&body),
            retry_after,
        })
    }

    /// `GET /models`, used by `vi doctor` to check a server is up.
    pub async fn list_models(&self) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct Models {
            data: Vec<ModelRow>,
        }
        #[derive(Deserialize)]
        struct ModelRow {
            id: String,
        }
        let mut req = self.http.get(self.url("models"));
        if let Some(k) = &self.api_key {
            req = req.bearer_auth(k);
        }
        let resp = req.send().await.map_err(|e| self.transport(e))?;
        let resp = self.check_status(resp).await?;
        let m: Models = resp.json().await.map_err(|e| ProviderError::Decode {
            provider: self.name.clone(),
            message: e.to_string(),
        })?;
        Ok(m.data.into_iter().map(|r| r.id).collect())
    }
}

// ---- verbose_json shapes ---------------------------------------------------

#[derive(Debug, Deserialize)]
struct VerboseJson {
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    text: String,
    #[serde(default)]
    segments: Vec<JsonSegment>,
    #[serde(default)]
    words: Vec<JsonWord>,
}

#[derive(Debug, Deserialize)]
struct JsonSegment {
    start: f64,
    end: f64,
    #[serde(default)]
    text: String,
    #[serde(default)]
    words: Vec<JsonWord>,
    #[serde(default)]
    avg_logprob: Option<f64>,
    #[serde(default)]
    no_speech_prob: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct JsonWord {
    word: String,
    start: f64,
    end: f64,
    #[serde(default)]
    probability: Option<f64>,
}

impl From<JsonWord> for AsrWord {
    fn from(w: JsonWord) -> Self {
        AsrWord {
            word: w.word,
            start: w.start,
            end: w.end,
            probability: w.probability.map(|p| p as f32),
        }
    }
}

/// Convert a verbose_json body into segments. OpenAI itself returns
/// top-level `words` (and no `segments`) when word granularity is
/// requested; Whisper servers return segments with nested words. Both shapes
/// are handled: words are attached to the segment whose range contains
/// them, and when there are no segments at all, words are grouped into
/// segments at pauses.
fn segments_from_verbose(v: VerboseJson) -> (Vec<AsrSegment>, Option<String>, f64) {
    let VerboseJson {
        language,
        duration,
        text,
        segments,
        words,
    } = v;
    let mut out: Vec<AsrSegment> = segments
        .into_iter()
        .map(|s| AsrSegment {
            start: s.start,
            end: s.end.max(s.start),
            text: s.text.trim().to_string(),
            words: s.words.into_iter().map(AsrWord::from).collect(),
            confidence: s.avg_logprob.map(|l| l.exp().clamp(0.0, 1.0) as f32),
            no_speech_prob: s.no_speech_prob.map(|p| p as f32),
        })
        .filter(|s| !s.text.is_empty())
        .collect();
    if out.is_empty() && !words.is_empty() {
        // Group words into segments at gaps over 1 s or every ~15 s.
        let mut cur: Vec<AsrWord> = Vec::new();
        let flush = |cur: &mut Vec<AsrWord>, out: &mut Vec<AsrSegment>| {
            if cur.is_empty() {
                return;
            }
            let start = cur[0].start;
            let end = cur[cur.len() - 1].end;
            let text: String = cur.iter().map(|w| w.word.as_str()).collect::<String>();
            out.push(AsrSegment {
                start,
                end,
                text: text.trim().to_string(),
                words: std::mem::take(cur),
                confidence: None,
                no_speech_prob: None,
            });
        };
        for w in words.into_iter().map(AsrWord::from) {
            let split = cur
                .last()
                .map(|p| w.start - p.end > 1.0 || w.end - cur[0].start > 15.0)
                .unwrap_or(false);
            if split {
                flush(&mut cur, &mut out);
            }
            cur.push(w);
        }
        flush(&mut cur, &mut out);
    } else if out.is_empty() && !text.trim().is_empty() {
        out.push(AsrSegment {
            start: 0.0,
            end: duration.unwrap_or(0.0),
            text: text.trim().to_string(),
            words: Vec::new(),
            confidence: None,
            no_speech_prob: None,
        });
    } else if !words.is_empty() && out.iter().all(|s| s.words.is_empty()) {
        // Top-level words alongside segments: attach by time.
        for w in words.into_iter().map(AsrWord::from) {
            let mid = (w.start + w.end) / 2.0;
            if let Some(seg) = out
                .iter_mut()
                .find(|s| mid >= s.start && mid < s.end.max(s.start + 1e-6))
            {
                seg.words.push(w);
            }
        }
    }
    let dur = duration.unwrap_or_else(|| out.last().map(|s| s.end).unwrap_or(0.0));
    (out, language, dur)
}

#[async_trait]
impl Asr for OpenAiCompat {
    fn provider_name(&self) -> &str {
        &self.name
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn asr_capabilities(&self) -> AsrCapabilities {
        AsrCapabilities {
            word_timestamps: true,
            max_audio_secs: MAX_AUDIO_SECS,
            price: self.pricing,
        }
    }

    async fn transcribe(&self, audio: &AudioData, opts: &AsrOptions) -> Result<AsrResponse> {
        if audio.samples.is_empty() {
            return Err(ProviderError::Invalid("empty audio".into()));
        }
        if audio.duration_secs() > MAX_AUDIO_SECS {
            return Err(ProviderError::Invalid(format!(
                "clip of {:.0}s exceeds {MAX_AUDIO_SECS}s",
                audio.duration_secs()
            )));
        }
        let started = Instant::now();
        let _permit = self.governor.acquire(0).await?;
        let wav = bytes::Bytes::from(audio.to_wav());
        let url = self.url("audio/transcriptions");
        let (body, attempts) = retry::run(&self.governor.retry, &self.cancel, |_| {
            let wav = wav.clone();
            let url = url.clone();
            async move {
                let mut form = multipart::Form::new()
                    .part(
                        "file",
                        multipart::Part::stream(wav)
                            .file_name("audio.wav")
                            .mime_str("audio/wav")
                            .map_err(|e| ProviderError::Invalid(e.to_string()))?,
                    )
                    .text("model", self.model.clone())
                    .text("response_format", "verbose_json")
                    .text("temperature", opts.temperature.to_string());
                if opts.word_timestamps {
                    form = form
                        .text("timestamp_granularities[]", "word")
                        .text("timestamp_granularities[]", "segment");
                }
                if let Some(l) = &opts.language {
                    form = form.text("language", l.clone());
                }
                if let Some(p) = &opts.prompt {
                    form = form.text("prompt", p.clone());
                }
                let mut req = self.http.post(&url).multipart(form);
                if let Some(k) = &self.api_key {
                    req = req.bearer_auth(k);
                }
                let resp = req.send().await.map_err(|e| self.transport(e))?;
                let resp = self.check_status(resp).await?;
                let text = resp.text().await.map_err(|e| self.transport(e))?;
                serde_json::from_str::<VerboseJson>(&text).map_err(|e| ProviderError::Decode {
                    provider: self.name.clone(),
                    message: format!("{e}: {}", short_body(&text)),
                })
            }
        })
        .await?;
        let (segments, language, duration) = segments_from_verbose(body);
        let usage = Usage {
            media_secs: if duration > 0.0 {
                duration
            } else {
                audio.duration_secs()
            },
            calls: 1,
            ..Usage::default()
        };
        let mut stats = CallStats::new(&self.name, &self.model, usage, &self.pricing);
        stats.latency_ms = started.elapsed().as_millis() as u64;
        stats.attempts = attempts;
        Ok(AsrResponse {
            segments,
            language,
            stats,
        })
    }
}

#[async_trait]
impl TextEmbedder for OpenAiCompat {
    fn provider_name(&self) -> &str {
        &self.name
    }
    fn model(&self) -> &str {
        &self.model
    }
    fn dim(&self) -> u32 {
        0
    }
    fn max_batch(&self) -> usize {
        1
    }
    async fn embed(&self, _texts: &[String]) -> Result<EmbedResponse> {
        Err(ProviderError::Unsupported {
            provider: self.name.clone(),
            capability: "text_embed (openai_compat embeddings arrive in M2)",
        })
    }
}

#[async_trait]
impl Llm for OpenAiCompat {
    fn provider_name(&self) -> &str {
        &self.name
    }
    fn model(&self) -> &str {
        &self.model
    }
    fn llm_capabilities(&self) -> VlmCapabilities {
        VlmCapabilities {
            native_video: false,
            native_audio: false,
            max_images_per_request: 16,
            max_image_pixels: 1_048_576,
            supports_tools: true,
            supports_streaming: true,
            supports_json_schema: false,
            context_tokens: 32_768,
            price: self.pricing,
        }
    }
    async fn generate(&self, _req: GenerateRequest) -> Result<EventStream> {
        Err(ProviderError::Unsupported {
            provider: self.name.clone(),
            capability: "llm (openai_compat chat completions arrive in M2)",
        })
    }
}

#[async_trait]
impl Vlm for OpenAiCompat {
    fn provider_name(&self) -> &str {
        &self.name
    }
    fn model(&self) -> &str {
        &self.model
    }
    fn vlm_capabilities(&self) -> VlmCapabilities {
        Llm::llm_capabilities(self)
    }
    async fn generate(&self, _req: GenerateRequest) -> Result<EventStream> {
        Err(ProviderError::Unsupported {
            provider: self.name.clone(),
            capability: "vlm (openai_compat chat completions arrive in M2)",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_whisper_server_shape() {
        let body = r#"{"language":"en","duration":60.0,"text":"hello world",
            "segments":[{"id":0,"seek":0,"start":0.0,"end":1.5,"text":" hello world","avg_logprob":-0.2,"no_speech_prob":0.01,
              "words":[{"word":" hello","start":0.0,"end":0.5,"probability":0.9},{"word":" world","start":0.6,"end":1.5,"probability":0.8}]}],
            "words":[]}"#;
        let v: VerboseJson = serde_json::from_str(body).unwrap();
        let (segs, lang, dur) = segments_from_verbose(v);
        assert_eq!(lang.as_deref(), Some("en"));
        assert_eq!(dur, 60.0);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].text, "hello world");
        assert_eq!(segs[0].words.len(), 2);
        assert!((segs[0].confidence.unwrap() - (-0.2f64).exp() as f32).abs() < 1e-6);
    }

    #[test]
    fn parses_openai_words_only_shape() {
        let body = r#"{"task":"transcribe","language":"english","duration":40.0,"text":"a b c d",
            "words":[{"word":"a","start":0.0,"end":0.2},{"word":"b","start":0.3,"end":0.5},
                     {"word":"c","start":5.0,"end":5.2},{"word":"d","start":5.3,"end":5.4}]}"#;
        let v: VerboseJson = serde_json::from_str(body).unwrap();
        let (segs, _, _) = segments_from_verbose(v);
        assert_eq!(segs.len(), 2, "{segs:?}");
        assert_eq!(segs[0].text, "ab");
        assert_eq!(segs[1].start, 5.0);
        assert_eq!(segs[1].words.len(), 2);
    }

    #[test]
    fn text_only_becomes_one_segment() {
        let v: VerboseJson =
            serde_json::from_str(r#"{"text":"just text","duration":3.0}"#).unwrap();
        let (segs, _, dur) = segments_from_verbose(v);
        assert_eq!(segs.len(), 1);
        assert_eq!((segs[0].start, segs[0].end), (0.0, 3.0));
        assert_eq!(dur, 3.0);
    }

    #[tokio::test]
    async fn transcribes_against_a_fake_server() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            // First request: 503 (retryable). Second: success.
            for i in 0..2 {
                let (mut sock, _) = listener.accept().await.unwrap();
                let mut buf = vec![0u8; 1 << 20];
                let mut n = 0;
                // Read until the multipart body has arrived (blank line then content-length bytes).
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
                assert!(req.starts_with("POST /v1/audio/transcriptions"), "{req}");
                assert!(req.contains("name=\"model\""));
                assert!(req.contains("RIFF"));
                let body = if i == 0 {
                    "{\"error\":\"busy\"}"
                } else {
                    r#"{"language":"en","duration":1.0,"text":"hi","segments":[{"start":0.0,"end":1.0,"text":" hi"}]}"#
                };
                let status = if i == 0 {
                    "503 Service Unavailable\r\nRetry-After: 0"
                } else {
                    "200 OK"
                };
                let resp = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                sock.write_all(resp.as_bytes()).await.unwrap();
                sock.shutdown().await.unwrap();
            }
        });
        let cfg = ProviderConfig {
            adapter: "openai_compat".into(),
            base_url: Some(format!("http://{addr}/v1")),
            model: Some("whisper-1".into()),
            max_retries: Some(2),
            pricing: Some(Pricing {
                per_media_second: 0.0001,
                ..Pricing::default()
            }),
            ..ProviderConfig::default()
        };
        let gov = Arc::new(Governor::from_config("t", &cfg));
        let a = OpenAiCompat::from_config("t", &cfg, None, gov, CancellationToken::new()).unwrap();
        let audio = AudioData {
            samples: Arc::from(vec![0i16; 16_000]),
            sample_rate: 16_000,
        };
        let r = a
            .transcribe(
                &audio,
                &AsrOptions {
                    word_timestamps: true,
                    ..AsrOptions::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(r.segments.len(), 1);
        assert_eq!(r.segments[0].text, "hi");
        assert_eq!(r.stats.attempts, 2);
        assert_eq!(r.stats.usage.calls, 1);
        assert!((r.stats.cost_usd - 0.0001).abs() < 1e-12);
        assert_eq!(r.stats.provider, "t");
    }
}
