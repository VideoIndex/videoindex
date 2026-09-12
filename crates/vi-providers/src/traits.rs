//! Capability traits and their request/response types
//! (`docs/07-model-providers.md`). Adapters implement whichever they can;
//! operators ask the registry for a role and get a trait object.

use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use vi_core::config::Pricing;

use crate::cost::CallStats;
use crate::error::Result;

// ---------------------------------------------------------------------------
// shared payload types

/// Pixels for a provider call. Local adapters take raw RGB; remote ones need
/// an encoded image.
#[derive(Debug, Clone)]
pub enum ImageData {
    /// Packed 8-bit RGB, row-major, no padding.
    Rgb8 {
        /// Width.
        width: u32,
        /// Height.
        height: u32,
        /// `width * height * 3` bytes.
        data: Arc<[u8]>,
    },
    /// An encoded image (`image/jpeg`, `image/png`, `image/webp`).
    Encoded {
        /// MIME type.
        mime: &'static str,
        /// Bytes.
        bytes: bytes::Bytes,
    },
}

impl ImageData {
    /// Width and height when known without decoding.
    pub fn dims(&self) -> Option<(u32, u32)> {
        match self {
            Self::Rgb8 { width, height, .. } => Some((*width, *height)),
            Self::Encoded { .. } => None,
        }
    }
}

/// Mono 16-bit PCM for ASR.
#[derive(Debug, Clone)]
pub struct AudioData {
    /// Samples.
    pub samples: Arc<[i16]>,
    /// Sample rate in Hz (adapters expect 16 000).
    pub sample_rate: u32,
}

impl AudioData {
    /// Duration in seconds.
    pub fn duration_secs(&self) -> f64 {
        self.samples.len() as f64 / f64::from(self.sample_rate.max(1))
    }

    /// Encode as a RIFF/WAVE file (PCM 16-bit mono), what transcription
    /// endpoints accept.
    pub fn to_wav(&self) -> Vec<u8> {
        let data_len = (self.samples.len() * 2) as u32;
        let mut out = Vec::with_capacity(44 + data_len as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&1u16.to_le_bytes()); // mono
        out.extend_from_slice(&self.sample_rate.to_le_bytes());
        out.extend_from_slice(&(self.sample_rate * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for s in self.samples.iter() {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out
    }
}

/// A vector.
pub type Vector = Vec<f32>;

// ---------------------------------------------------------------------------
// ASR

/// Options for one transcription call.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AsrOptions {
    /// BCP-47 language hint; `None` lets the model detect it.
    pub language: Option<String>,
    /// Context text (previous transcript, vocabulary).
    pub prompt: Option<String>,
    /// Ask for word-level timings.
    pub word_timestamps: bool,
    /// Sampling temperature.
    pub temperature: f32,
}

/// One word with timing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AsrWord {
    /// Text, usually with a leading space.
    pub word: String,
    /// Start in seconds, relative to the audio sent.
    pub start: f64,
    /// End in seconds.
    pub end: f64,
    /// 0-1 when the model reports it.
    pub probability: Option<f32>,
}

/// One transcribed segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AsrSegment {
    /// Start in seconds, relative to the audio sent.
    pub start: f64,
    /// End in seconds.
    pub end: f64,
    /// Text.
    pub text: String,
    /// Words with timings when requested and available.
    pub words: Vec<AsrWord>,
    /// Confidence 0-1 (Whisper: `exp(avg_logprob)`).
    pub confidence: Option<f32>,
    /// Probability the segment is not speech.
    pub no_speech_prob: Option<f32>,
}

/// Transcription result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AsrResponse {
    /// Segments in time order.
    pub segments: Vec<AsrSegment>,
    /// Detected or given language.
    pub language: Option<String>,
    /// Call accounting.
    pub stats: CallStats,
}

/// What an ASR adapter can do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AsrCapabilities {
    /// Word timings available.
    pub word_timestamps: bool,
    /// Longest clip accepted, seconds.
    pub max_audio_secs: f64,
    /// Price table.
    pub price: Pricing,
}

/// Speech to timed text.
#[async_trait]
pub trait Asr: Send + Sync {
    /// Provider name from config.
    fn provider_name(&self) -> &str;
    /// Model name.
    fn model(&self) -> &str;
    /// Capabilities.
    fn asr_capabilities(&self) -> AsrCapabilities;
    /// Transcribe one clip.
    async fn transcribe(&self, audio: &AudioData, opts: &AsrOptions) -> Result<AsrResponse>;
}

// ---------------------------------------------------------------------------
// OCR

/// Normalised box.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OcrBox {
    /// Left, 0-1.
    pub x: f32,
    /// Top, 0-1.
    pub y: f32,
    /// Width, 0-1.
    pub w: f32,
    /// Height, 0-1.
    pub h: f32,
}

/// One text line read from an image.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrLine {
    /// Text.
    pub text: String,
    /// Where.
    pub bbox: Option<OcrBox>,
    /// 0-1.
    pub confidence: Option<f32>,
}

/// OCR result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrResponse {
    /// Lines in reading order (top to bottom, left to right).
    pub lines: Vec<OcrLine>,
    /// Accounting.
    pub stats: CallStats,
}

/// Text and boxes from a frame.
#[async_trait]
pub trait Ocr: Send + Sync {
    /// Provider name.
    fn provider_name(&self) -> &str;
    /// Model name.
    fn model(&self) -> &str;
    /// Read one image.
    async fn read(&self, image: &ImageData) -> Result<OcrResponse>;
}

// ---------------------------------------------------------------------------
// embeddings

/// Embedding result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbedResponse {
    /// One vector per input, L2-normalised.
    pub vectors: Vec<Vector>,
    /// Accounting.
    pub stats: CallStats,
}

/// Text to vector.
#[async_trait]
pub trait TextEmbedder: Send + Sync {
    /// Provider name.
    fn provider_name(&self) -> &str;
    /// Model name, also the vector-store table name.
    fn model(&self) -> &str;
    /// Vector length.
    fn dim(&self) -> u32;
    /// Largest batch the adapter accepts.
    fn max_batch(&self) -> usize;
    /// Embed passages.
    async fn embed(&self, texts: &[String]) -> Result<EmbedResponse>;
    /// Embed a search query. Models with an asymmetric query prefix (bge)
    /// override this; the default embeds the text as a passage.
    async fn embed_query(&self, query: &str) -> Result<EmbedResponse> {
        self.embed(&[query.to_string()]).await
    }
}

/// Image and text into a shared space.
#[async_trait]
pub trait ImageEmbedder: Send + Sync {
    /// Provider name.
    fn provider_name(&self) -> &str;
    /// Model name, also the vector-store table name.
    fn model(&self) -> &str;
    /// Vector length.
    fn dim(&self) -> u32;
    /// Largest batch the adapter accepts.
    fn max_batch(&self) -> usize;
    /// Embed images.
    async fn embed_images(&self, images: &[ImageData]) -> Result<EmbedResponse>;
    /// Embed queries with the text tower.
    async fn embed_text(&self, texts: &[String]) -> Result<EmbedResponse>;
}

/// Score (query, passage) pairs.
#[async_trait]
pub trait Reranker: Send + Sync {
    /// Provider name.
    fn provider_name(&self) -> &str;
    /// Model name.
    fn model(&self) -> &str;
    /// One score per passage, higher is more relevant.
    async fn rerank(&self, query: &str, passages: &[String]) -> Result<(Vec<f32>, CallStats)>;
}

// ---------------------------------------------------------------------------
// LLM and VLM (types now, adapters in M2)

/// Chat role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// System instructions.
    System,
    /// The user.
    User,
    /// The model.
    Assistant,
    /// A tool result.
    Tool,
}

/// One part of a message.
#[derive(Debug, Clone)]
pub enum ContentPart {
    /// Text.
    Text(String),
    /// An image.
    Image(ImageData),
    /// A video clip (native-video providers only).
    Video {
        /// MIME type.
        mime: &'static str,
        /// Bytes.
        bytes: bytes::Bytes,
        /// Duration in seconds, for cost estimates.
        duration_secs: f64,
    },
    /// A tool call the model made earlier (assistant turns in history).
    ToolCall {
        /// Call id.
        id: String,
        /// Tool name.
        name: String,
        /// JSON arguments.
        arguments: String,
    },
    /// A tool call result being returned to the model.
    ToolResult {
        /// Call id.
        call_id: String,
        /// Tool name (Gemini keys responses by name).
        name: String,
        /// JSON result.
        content: String,
    },
}

impl Message {
    /// A text-only message.
    pub fn text(role: Role, text: impl Into<String>) -> Self {
        Self {
            role,
            parts: vec![ContentPart::Text(text.into())],
        }
    }
}

impl GenerateRequest {
    /// A request with defaults: 1024 output tokens, temperature 0.
    pub fn new(messages: Vec<Message>) -> Self {
        Self {
            messages,
            tools: Vec::new(),
            tool_choice: ToolChoice::Auto,
            max_tokens: 1024,
            temperature: 0.0,
            json_schema: None,
            model: None,
        }
    }

    /// Approximate input size in tokens (4 characters per token plus a
    /// flat cost per image), for rate limiting and budgets.
    pub fn estimated_tokens(&self) -> u64 {
        let mut chars = 0usize;
        let mut images = 0u64;
        for m in &self.messages {
            for p in &m.parts {
                match p {
                    ContentPart::Text(t) => chars += t.len(),
                    ContentPart::Image(_) => images += 1,
                    ContentPart::Video { duration_secs, .. } => {
                        images += (*duration_secs).ceil() as u64
                    }
                    ContentPart::ToolCall { arguments, .. } => chars += arguments.len(),
                    ContentPart::ToolResult { content, .. } => chars += content.len(),
                }
            }
        }
        for t in &self.tools {
            chars += t.description.len() + t.parameters.to_string().len();
        }
        (chars / 4) as u64 + images * 1000
    }
}

/// A chat message.
#[derive(Debug, Clone)]
pub struct Message {
    /// Who.
    pub role: Role,
    /// Parts.
    pub parts: Vec<ContentPart>,
}

/// A tool the model may call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    /// Name.
    pub name: String,
    /// Description.
    pub description: String,
    /// JSON Schema of the arguments.
    pub parameters: serde_json::Value,
}

/// Whether the model may call tools in this turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoice {
    /// The model decides (default).
    #[default]
    Auto,
    /// Tools stay defined (some APIs require the definitions when the history
    /// holds tool calls) but the model must answer in text.
    None,
}

/// Text-only or multimodal generation request.
#[derive(Debug, Clone)]
pub struct GenerateRequest {
    /// Conversation.
    pub messages: Vec<Message>,
    /// Tools available.
    pub tools: Vec<ToolSpec>,
    /// Whether tools may be called in this turn.
    pub tool_choice: ToolChoice,
    /// Max output tokens.
    pub max_tokens: u32,
    /// Temperature.
    pub temperature: f32,
    /// Request JSON conforming to this schema.
    pub json_schema: Option<serde_json::Value>,
    /// Model override.
    pub model: Option<String>,
}

/// Streaming events, normalised across adapters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GenerateEvent {
    /// Text delta.
    Token {
        /// Text.
        text: String,
    },
    /// A complete tool call.
    ToolCall {
        /// Call id.
        id: String,
        /// Tool name.
        name: String,
        /// JSON arguments.
        arguments: String,
    },
    /// Usage so far or final.
    Usage(crate::cost::Usage),
    /// End of stream.
    Done {
        /// Why it stopped: `stop`, `length`, `tool_use`.
        finish_reason: String,
        /// Final accounting.
        stats: CallStats,
    },
}

/// Boxed event stream.
pub type EventStream = Pin<Box<dyn Stream<Item = Result<GenerateEvent>> + Send>>;

/// What a VLM or LLM adapter can do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VlmCapabilities {
    /// Accepts a video clip, not just images.
    pub native_video: bool,
    /// Accepts audio.
    pub native_audio: bool,
    /// Max images per request.
    pub max_images_per_request: u32,
    /// Max pixels per image.
    pub max_image_pixels: u32,
    /// Tool calling.
    pub supports_tools: bool,
    /// Streaming.
    pub supports_streaming: bool,
    /// JSON schema output.
    pub supports_json_schema: bool,
    /// Context window.
    pub context_tokens: u32,
    /// Price table.
    pub price: Pricing,
}

/// Text-only reasoning and tool use.
#[async_trait]
pub trait Llm: Send + Sync {
    /// Provider name.
    fn provider_name(&self) -> &str;
    /// Model name.
    fn model(&self) -> &str;
    /// Capabilities.
    fn llm_capabilities(&self) -> VlmCapabilities;
    /// Generate, streaming.
    async fn generate(&self, req: GenerateRequest) -> Result<EventStream>;
}

/// Describe or answer over images, frame grids, or native video.
#[async_trait]
pub trait Vlm: Send + Sync {
    /// Provider name.
    fn provider_name(&self) -> &str;
    /// Model name.
    fn model(&self) -> &str;
    /// Capabilities.
    fn vlm_capabilities(&self) -> VlmCapabilities;
    /// Generate, streaming.
    async fn generate(&self, req: GenerateRequest) -> Result<EventStream>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_right() {
        let a = AudioData {
            samples: Arc::from(vec![0i16, 1, -1, 32767]),
            sample_rate: 16_000,
        };
        let w = a.to_wav();
        assert_eq!(w.len(), 44 + 8);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..12], b"WAVE");
        assert_eq!(u32::from_le_bytes([w[24], w[25], w[26], w[27]]), 16_000);
        assert_eq!(&w[36..40], b"data");
        assert_eq!(u32::from_le_bytes([w[40], w[41], w[42], w[43]]), 8);
        assert_eq!(&w[44..48], &[0, 0, 1, 0]);
        assert!((a.duration_secs() - 0.00025).abs() < 1e-9);
    }
}
