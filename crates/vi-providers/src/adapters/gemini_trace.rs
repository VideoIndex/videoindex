//! Raw tracing for the [`Gemini`](super::gemini::Gemini) adapter: the shape
//! of each request, every SSE data line as received, and after the stream
//! the parts the model returned with what the adapter made of each.
//!
//! Every event uses the target `vi_providers::gemini::raw`. It is a custom
//! target (this module's path is `vi_providers::adapters::gemini_trace`),
//! and `EnvFilter` matches a directive against the event's target, so
//! `vidx --log vi_providers::gemini::raw=trace` enables the request, SSE,
//! part and final events and `vi_providers::gemini::raw=debug` only the
//! one-line parse decisions (a `thought` part emitted as answer text, text
//! after a function call, a finish reason other than `STOP`, ...).
//!
//! With `VI_GEMINI_RAW_DIR` set, each request is also appended to
//! `<dir>/<timestamp>-<n>.jsonl`, one JSON object per line:
//! `{"kind":"request",...}`, then one `{"kind":"sse",...}` per data line,
//! then `{"kind":"final",...}` (or `{"kind":"error",...}` first when the call
//! failed). Neither the log nor the file ever carries the API key (it travels
//! in the `x-goog-api-key` header, which is never logged, and the URL has no
//! `?key=`) or inline data bytes (only their MIME type and decoded length).

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use serde_json::{json, Value};

use crate::cost::Usage;
use crate::error::ProviderError;
use crate::sse::SseEvent;

/// The `tracing` target of every event here.
pub const TARGET: &str = "vi_providers::gemini::raw";
/// The directory for the per-request JSON-lines files. A `VI_*` name, so
/// `vi_core::config::ENV_NOT_CONFIG` lists it (the config's `VI_*`
/// overrides would otherwise reject it as an unknown key).
pub const RAW_DIR_ENV: &str = "VI_GEMINI_RAW_DIR";
/// Longest SSE line logged or written; longer lines are cut and flagged.
pub const MAX_LINE_CHARS: usize = 20_000;
/// Characters of a text part shown in its final event.
const PREVIEW_CHARS: usize = 80;

/// Per-process request counter for the file names.
static SEQ: AtomicU64 = AtomicU64::new(0);

/// `VI_GEMINI_RAW_DIR`, when set and non-empty.
pub fn raw_dir_from_env() -> Option<PathBuf> {
    std::env::var_os(RAW_DIR_ENV)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Decoded length of a standard base64 string.
fn b64_decoded_len(s: &str) -> usize {
    let pad = s.bytes().rev().take_while(|&b| b == b'=').count();
    (s.len() * 3 / 4).saturating_sub(pad)
}

/// The first `n` characters of `s` and whether it was cut.
pub(crate) fn cut_chars(s: &str, n: usize) -> (&str, bool) {
    match s.char_indices().nth(n) {
        Some((i, _)) => (&s[..i], true),
        None => (s, false),
    }
}

/// One part of a request `contents` entry in a few characters:
/// `text(312)`, `functionCall:search sig(88)`, `functionCall:view nosig`,
/// `functionResponse:search(1840)`, `inlineData:image/jpeg(84211B)`.
/// Lengths of text are in characters; of inline data, decoded bytes.
fn part_shape(p: &Value) -> String {
    let mut s = if let Some(t) = p["text"].as_str() {
        format!("text({})", t.chars().count())
    } else if let Some(fc) = p.get("functionCall") {
        format!("functionCall:{}", fc["name"].as_str().unwrap_or("?"))
    } else if let Some(fr) = p.get("functionResponse") {
        format!(
            "functionResponse:{}({})",
            fr["name"].as_str().unwrap_or("?"),
            fr["response"].to_string().chars().count()
        )
    } else if let Some(d) = p.get("inlineData") {
        format!(
            "inlineData:{}({}B)",
            d["mimeType"].as_str().unwrap_or("?"),
            d["data"].as_str().map_or(0, b64_decoded_len)
        )
    } else {
        let keys: Vec<&str> = p
            .as_object()
            .map(|m| m.keys().map(String::as_str).collect())
            .unwrap_or_default();
        format!("other:{}", keys.join("+"))
    };
    if p["thought"].as_bool() == Some(true) {
        s.push_str(" thought");
    }
    match p["thoughtSignature"].as_str() {
        Some(sig) => s.push_str(&format!(" sig({})", sig.len())),
        None if p.get("functionCall").is_some() => s.push_str(" nosig"),
        None => {}
    }
    s
}

/// The shape of a request body: model, generation settings, tool count and
/// calling mode, and per `contents` entry its role and parts. Never carries
/// text, data bytes or the key.
pub fn request_shape(model: &str, body: &Value) -> Value {
    let gen = &body["generationConfig"];
    let mut g = serde_json::Map::new();
    for k in [
        "temperature",
        "maxOutputTokens",
        "thinkingConfig",
        "candidateCount",
        "topP",
        "topK",
        "responseMimeType",
    ] {
        if let Some(v) = gen.get(k) {
            g.insert(k.to_string(), v.clone());
        }
    }
    if gen.get("responseSchema").is_some() {
        g.insert("responseSchema".into(), json!(true));
    }
    let tools: usize = body["tools"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|t| t["functionDeclarations"].as_array().map_or(0, Vec::len))
                .sum()
        })
        .unwrap_or(0);
    let mode = match body["toolConfig"]["functionCallingConfig"]["mode"].as_str() {
        Some(m) => m.to_string(),
        None if tools > 0 => "AUTO (default)".into(),
        None => "-".into(),
    };
    let system_chars: usize = body["systemInstruction"]["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p["text"].as_str())
        .map(|t| t.chars().count())
        .sum();
    let contents: Vec<Value> = body["contents"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| {
            let parts: Vec<String> = c["parts"]
                .as_array()
                .into_iter()
                .flatten()
                .map(part_shape)
                .collect();
            json!({"role": c["role"], "n": parts.len(), "parts": parts})
        })
        .collect();
    json!({
        "model": model,
        "generationConfig": g,
        "system_chars": system_chars,
        "tools": tools,
        "functionCallingConfig": mode,
        "contents": contents,
    })
}

/// A copy of the request body with every `inlineData.data` replaced by
/// `"<N bytes elided>"`, for the raw file (the body carries no key).
pub fn elide_inline_data(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, v)| {
                    if k == "inlineData" {
                        let mut d = v.clone();
                        if let Some(data) = v["data"].as_str() {
                            d["data"] = json!(format!("<{} bytes elided>", b64_decoded_len(data)));
                        }
                        (k.clone(), d)
                    } else {
                        (k.clone(), elide_inline_data(v))
                    }
                })
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(elide_inline_data).collect()),
        other => other.clone(),
    }
}

/// What one logical part of the response was and what the adapter did with
/// it. Consecutive text chunks of one candidate with the same `thought` flag
/// merge into one record until one of them carries a thought signature (the
/// streaming form of one text part).
#[derive(Debug, Default)]
struct PartRec {
    candidate: u64,
    kind: String,
    thought: bool,
    signature: Option<usize>,
    chars: usize,
    preview: String,
    chunks: usize,
    empty_chunks: usize,
    tokens: usize,
    emitted_chars: usize,
    tool_call: Option<String>,
    api_call_id: Option<String>,
    notes: Vec<String>,
}

impl PartRec {
    fn note(&mut self, s: impl Into<String>) {
        let s = s.into();
        if !self.notes.contains(&s) {
            self.notes.push(s);
        }
    }

    fn to_json(&self, index: usize) -> Value {
        let mut v = json!({
            "index": index,
            "candidate": self.candidate,
            "kind": self.kind,
            "thought": self.thought,
            "thoughtSignature": self.signature,
            "chunks": self.chunks,
            "emitted": {"tokens": self.tokens, "chars": self.emitted_chars, "tool_call": self.tool_call},
        });
        if self.kind == "text" {
            v["chars"] = json!(self.chars);
            v["preview"] = json!(self.preview);
            v["empty_chunks"] = json!(self.empty_chunks);
        }
        if let Some(id) = &self.api_call_id {
            v["api_call_id"] = json!(id);
        }
        if !self.notes.is_empty() {
            v["notes"] = json!(self.notes);
        }
        v
    }
}

#[derive(Debug)]
struct Inner {
    started: Instant,
    model: String,
    file: Option<std::fs::File>,
    path: Option<PathBuf>,
    lines: u64,
    parts: Vec<PartRec>,
    finish: BTreeMap<u64, String>,
    finish_message: BTreeMap<u64, String>,
    usage_metadata: Option<Value>,
    prompt_feedback: Option<Value>,
    model_version: Option<String>,
    response_id: Option<String>,
    candidates_seen: u64,
    saw_call: bool,
    warned_multi: bool,
    warned_replacement: bool,
    warned_thought: bool,
    warned_after_call: bool,
    tokens: u64,
    chars: u64,
    calls: u64,
    error: Option<String>,
    finished: bool,
}

/// The tracer of one request. Inactive (every hook returns at once) unless
/// the target is enabled at `debug` or finer, or a raw directory is set.
#[derive(Debug)]
pub struct RawTrace {
    inner: Option<Box<Inner>>,
}

impl RawTrace {
    /// Start tracing a request: log its shape and open the raw file.
    pub fn start(model: &str, body: &Value, raw_dir: Option<&Path>) -> Self {
        let enabled = tracing::enabled!(target: TARGET, tracing::Level::DEBUG);
        if !enabled && raw_dir.is_none() {
            return Self { inner: None };
        }
        let (file, path) = match raw_dir.map(open_raw_file) {
            Some(Ok((f, p))) => (Some(f), Some(p)),
            Some(Err(e)) => {
                tracing::debug!(target: TARGET, error = %e, "gemini raw: cannot open a file in {RAW_DIR_ENV}; not writing one");
                (None, None)
            }
            None => (None, None),
        };
        let shape = request_shape(model, body);
        tracing::trace!(
            target: TARGET,
            file = ?path,
            shape = %shape,
            "gemini raw request"
        );
        let mut inner = Box::new(Inner {
            started: Instant::now(),
            model: model.to_string(),
            file,
            path,
            lines: 0,
            parts: Vec::new(),
            finish: BTreeMap::new(),
            finish_message: BTreeMap::new(),
            usage_metadata: None,
            prompt_feedback: None,
            model_version: None,
            response_id: None,
            candidates_seen: 0,
            saw_call: false,
            warned_multi: false,
            warned_replacement: false,
            warned_thought: false,
            warned_after_call: false,
            tokens: 0,
            chars: 0,
            calls: 0,
            error: None,
            finished: false,
        });
        inner.write(&json!({
            "kind": "request",
            "ts": chrono::Utc::now().to_rfc3339(),
            "model": model,
            "shape": shape,
            "body": elide_inline_data(body),
        }));
        Self { inner: Some(inner) }
    }

    /// Whether the tracer records anything.
    pub fn is_active(&self) -> bool {
        self.inner.is_some()
    }

    /// Path of the raw file, when one is being written.
    pub fn path(&self) -> Option<&Path> {
        self.inner.as_ref().and_then(|i| i.path.as_deref())
    }

    /// One SSE event as received (its `data:` payload; Gemini sends one
    /// single-line JSON object per event).
    pub fn line(&mut self, ev: &SseEvent) {
        let Some(t) = self.inner.as_mut() else {
            return;
        };
        t.lines += 1;
        let chars = ev.data.chars().count();
        let (shown, truncated) = cut_chars(&ev.data, MAX_LINE_CHARS);
        tracing::trace!(
            target: TARGET,
            n = t.lines,
            chars,
            truncated,
            event = %ev.event,
            line = %shown,
            "gemini raw sse"
        );
        if ev.data.contains('\u{FFFD}') && !t.warned_replacement {
            t.warned_replacement = true;
            tracing::debug!(
                target: TARGET,
                n = t.lines,
                "gemini: U+FFFD in an SSE line; sse.rs decodes each network chunk with from_utf8_lossy, so a character split across two reads is mangled"
            );
        }
        let mut rec = json!({"kind": "sse", "n": t.lines, "line": shown});
        if truncated {
            rec["truncated"] = json!(true);
            rec["chars"] = json!(chars);
        }
        if !ev.event.is_empty() {
            rec["event"] = json!(ev.event);
        }
        t.write(&rec);
    }

    /// A parsed chunk, before its parts are read.
    pub fn chunk(&mut self, v: &Value) {
        let Some(t) = self.inner.as_mut() else {
            return;
        };
        if let Some(u) = v.get("usageMetadata") {
            t.usage_metadata = Some(u.clone());
        }
        if let Some(pf) = v.get("promptFeedback") {
            if t.prompt_feedback.is_none() {
                tracing::debug!(target: TARGET, feedback = %pf, "gemini: promptFeedback in the response");
            }
            t.prompt_feedback = Some(pf.clone());
        }
        if let Some(m) = v["modelVersion"].as_str() {
            t.model_version = Some(m.to_string());
        }
        if let Some(r) = v["responseId"].as_str() {
            t.response_id = Some(r.to_string());
        }
        let n = v["candidates"].as_array().map_or(0, Vec::len) as u64;
        t.candidates_seen = t.candidates_seen.max(n);
        if n > 1 && !t.warned_multi {
            t.warned_multi = true;
            tracing::debug!(
                target: TARGET,
                candidates = n,
                "gemini: more than one candidate; the adapter emits every candidate's parts into one stream"
            );
        }
    }

    /// One response part and what the adapter emitted from it: the
    /// characters of the `Token` it pushed (`None` for none) and the id of
    /// the tool call it pushed.
    pub fn part(
        &mut self,
        candidate: u64,
        part: &Value,
        token_chars: Option<usize>,
        tool_call: Option<&str>,
    ) {
        let Some(t) = self.inner.as_mut() else {
            return;
        };
        let text = part["text"].as_str();
        let fc = part.get("functionCall");
        let thought = part["thought"].as_bool() == Some(true);
        let sig = part["thoughtSignature"].as_str().map(str::len);
        if let Some(n) = token_chars {
            t.tokens += 1;
            t.chars += n as u64;
        }
        if tool_call.is_some() {
            t.calls += 1;
        }
        if let (Some(txt), None) = (text, fc) {
            if thought && token_chars.is_some() && !t.warned_thought {
                t.warned_thought = true;
                tracing::debug!(
                    target: TARGET,
                    candidate,
                    chars = txt.chars().count(),
                    preview = %cut_chars(txt, PREVIEW_CHARS).0,
                    "gemini: a text part with thought:true was emitted as answer text (the adapter does not read `thought`)"
                );
            }
            if t.saw_call && !txt.is_empty() && !t.warned_after_call {
                t.warned_after_call = true;
                tracing::debug!(
                    target: TARGET,
                    candidate,
                    chars = txt.chars().count(),
                    preview = %cut_chars(txt, PREVIEW_CHARS).0,
                    "gemini: a text part after a functionCall in the same turn; emitted as answer text, and the agent's history entry puts the turn's text before its calls"
                );
            }
            if sig.is_some() {
                tracing::debug!(
                    target: TARGET,
                    candidate,
                    chars = txt.chars().count(),
                    signature = sig,
                    "gemini: a thoughtSignature on a text part is not kept (only functionCall signatures are echoed back)"
                );
            }
            // Merge into the previous text record: same candidate and
            // `thought`, and that record has not closed with a signature.
            let merge = t.parts.last().is_some_and(|p| {
                p.kind == "text"
                    && p.candidate == candidate
                    && p.thought == thought
                    && p.signature.is_none()
            });
            if !merge {
                t.parts.push(PartRec {
                    candidate,
                    kind: "text".into(),
                    thought,
                    ..PartRec::default()
                });
            }
            let Some(rec) = t.parts.last_mut() else {
                return;
            };
            rec.chunks += 1;
            rec.chars += txt.chars().count();
            if rec.preview.chars().count() < PREVIEW_CHARS {
                let need = PREVIEW_CHARS - rec.preview.chars().count();
                rec.preview.push_str(cut_chars(txt, need).0);
            }
            if txt.is_empty() {
                rec.empty_chunks += 1;
            }
            if sig.is_some() {
                rec.signature = sig;
                rec.note("thoughtSignature on a text part: not kept");
            }
            match token_chars {
                Some(n) => {
                    rec.tokens += 1;
                    rec.emitted_chars += n;
                }
                None if txt.is_empty() => rec.note("empty text chunk: nothing emitted"),
                None => rec.note("text not emitted"),
            }
            if thought && token_chars.is_some() {
                rec.note("thought:true text emitted as answer text");
            }
            if t.saw_call && !txt.is_empty() {
                rec.note("text after a functionCall in the same turn");
            }
            return;
        }
        let mut rec = PartRec {
            candidate,
            thought,
            signature: sig,
            chunks: 1,
            ..PartRec::default()
        };
        if let Some(fc) = fc {
            t.saw_call = true;
            rec.kind = format!("functionCall:{}", fc["name"].as_str().unwrap_or("?"));
            rec.tool_call = tool_call.map(str::to_string);
            if let Some(id) = fc["id"].as_str() {
                rec.api_call_id = Some(id.to_string());
                rec.note("functionCall.id from the API is not kept (the response goes back keyed by name)");
                tracing::debug!(
                    target: TARGET,
                    id,
                    name = fc["name"].as_str().unwrap_or("?"),
                    "gemini: a functionCall carried an id; the adapter replaces it with call_N and the functionResponse goes back without it"
                );
            }
            if let Some(txt) = text {
                rec.chars = txt.chars().count();
                rec.note("text and functionCall on one part: both emitted");
            }
            if let Some(n) = token_chars {
                rec.tokens = 1;
                rec.emitted_chars = n;
            }
        } else {
            let keys: Vec<&str> = part
                .as_object()
                .map(|m| m.keys().map(String::as_str).collect())
                .unwrap_or_default();
            rec.kind = format!("other:{}", keys.join("+"));
            rec.note("neither text nor functionCall: dropped");
            tracing::debug!(
                target: TARGET,
                candidate,
                keys = %keys.join(","),
                "gemini: a part with neither text nor functionCall was dropped"
            );
        }
        t.parts.push(rec);
    }

    /// A candidate object of a chunk, after its parts.
    pub fn candidate(&mut self, candidate: u64, c: &Value) {
        let Some(t) = self.inner.as_mut() else {
            return;
        };
        let no_parts = c["content"]["parts"].as_array().is_none_or(Vec::is_empty);
        if no_parts {
            tracing::debug!(
                target: TARGET,
                candidate,
                finish = c["finishReason"].as_str().unwrap_or("-"),
                "gemini: a candidate with an empty part list"
            );
        }
        if let Some(f) = c["finishReason"].as_str() {
            t.finish.insert(candidate, f.to_string());
        }
        if let Some(m) = c["finishMessage"].as_str() {
            t.finish_message.insert(candidate, m.to_string());
        }
    }

    /// The call failed (HTTP status, a stream error, an undecodable line).
    pub fn error(&mut self, e: &ProviderError) {
        let Some(t) = self.inner.as_mut() else {
            return;
        };
        // `ProviderError`'s text is already redacted (short_body, without_url).
        let message = e.to_string();
        tracing::trace!(target: TARGET, error = %message, "gemini raw error");
        t.write(&json!({"kind": "error", "message": message}));
        t.error = Some(message);
        t.finalize("error", None);
    }

    /// The stream ended: what the adapter reported to the caller.
    pub fn end(&mut self, finish_emitted: &str, usage: &Usage) {
        if let Some(t) = self.inner.as_mut() {
            t.finalize(finish_emitted, Some(usage));
        }
    }
}

impl Drop for RawTrace {
    fn drop(&mut self) {
        if let Some(t) = self.inner.as_mut() {
            if !t.finished {
                t.finalize("abandoned (the stream was dropped before its end)", None);
            }
        }
    }
}

impl Inner {
    fn write(&mut self, v: &Value) {
        let Some(f) = self.file.as_mut() else {
            return;
        };
        if let Err(e) = writeln!(f, "{v}") {
            tracing::debug!(target: TARGET, error = %e, "gemini raw: file write failed; no more lines");
            self.file = None;
        }
    }

    fn finalize(&mut self, finish_emitted: &str, usage: Option<&Usage>) {
        if self.finished {
            return;
        }
        self.finished = true;
        for (cand, reason) in &self.finish {
            if reason != "STOP" {
                tracing::debug!(
                    target: TARGET,
                    candidate = cand,
                    finish = %reason,
                    finish_message = self.finish_message.get(cand).map_or("", String::as_str),
                    reported_as = %finish_emitted,
                    "gemini: finishReason other than STOP (reported as `tool_use` when the turn has a call, else `length` for MAX_TOKENS and the reason lower-cased otherwise)"
                );
            }
        }
        if self.parts.is_empty() && self.error.is_none() {
            tracing::debug!(
                target: TARGET,
                finish = ?self.finish,
                "gemini: the response carried no parts at all"
            );
        }
        let parts: Vec<Value> = self
            .parts
            .iter()
            .enumerate()
            .map(|(i, p)| p.to_json(i))
            .collect();
        for p in &parts {
            tracing::trace!(target: TARGET, part = %p, "gemini raw part");
        }
        let dropped: Vec<String> = self
            .parts
            .iter()
            .enumerate()
            .flat_map(|(i, p)| {
                p.notes
                    .iter()
                    .filter(|n| {
                        n.contains("dropped") || n.contains("not kept") || n.contains("not emitted")
                    })
                    .map(move |n| format!("part {i}: {n}"))
            })
            .collect();
        let summary = json!({
            "model": self.model,
            "modelVersion": self.model_version,
            "responseId": self.response_id,
            "sse_lines": self.lines,
            "candidates": self.candidates_seen,
            "finishReason": self.finish,
            "finishMessage": self.finish_message,
            "usageMetadata": self.usage_metadata,
            "promptFeedback": self.prompt_feedback,
            "emitted": {
                "tokens": self.tokens,
                "chars": self.chars,
                "tool_calls": self.calls,
                "finish_reason": finish_emitted,
                "usage": usage.map(|u| json!({"tokens_in": u.tokens_in, "tokens_out": u.tokens_out})),
            },
            "dropped": dropped,
            "error": self.error,
            "elapsed_ms": self.started.elapsed().as_millis() as u64,
        });
        tracing::trace!(target: TARGET, summary = %summary, "gemini raw final");
        let mut rec = summary;
        rec["kind"] = json!("final");
        rec["parts"] = json!(parts);
        self.write(&rec);
    }
}

/// Create `<dir>/<timestamp>-<n>.jsonl` (never an existing file).
fn open_raw_file(dir: &Path) -> std::io::Result<(std::fs::File, PathBuf)> {
    std::fs::create_dir_all(dir)?;
    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ");
    let mut last = None;
    for _ in 0..1000 {
        let n = SEQ.fetch_add(1, Ordering::Relaxed) + 1;
        let path = dir.join(format!("{ts}-{n}.jsonl"));
        match std::fs::OpenOptions::new()
            .append(true)
            .create_new(true)
            .open(&path)
        {
            Ok(f) => return Ok((f, path)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(last.unwrap_or_else(|| std::io::Error::other("no free file name")))
}
