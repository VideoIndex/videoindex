# 07. Model providers

VideoIndex never depends on a specific model. Every model capability is a Rust trait; concrete adapters implement them; configuration selects which adapter serves which role. The same Index can be built with one set of providers and queried with another, and the eval harness sweeps configurations to compare them.

## Capability traits

| Trait | Purpose | Key method |
|---|---|---|
| `Vlm` | Describe or answer over images, frame grids, or native video clips | `generate(VlmRequest) -> Stream<VlmEvent>` |
| `Llm` | Text-only reasoning, tool use for the agent loop, entity extraction | `generate(LlmRequest) -> Stream<LlmEvent>` |
| `Asr` | Speech to timed text | `transcribe(AudioChunk, AsrOptions) -> Vec<TranscriptSpan>` |
| `Ocr` | Text and boxes from a frame | `read(Image) -> Vec<OcrSpan>` |
| `TextEmbedder` | Text to vector | `embed(&[String]) -> Vec<Vector>` |
| `ImageEmbedder` | Image and text to a shared space | `embed_images(&[Image])`, `embed_text(&[String])` |
| `Reranker` | Score (query, passage) pairs | `rerank(query, &[String]) -> Vec<f32>` |

One adapter may implement several traits. A Gemini adapter implements `Vlm`, `Llm`, `Asr` (via audio input), `Ocr` (via a prompt), and `TextEmbedder`.

## Capabilities negotiation

Adapters report what they can do so operators and policies adapt without special-casing vendors:

```rust
pub struct VlmCapabilities {
    pub native_video: bool,         // accepts a video clip, not just images
    pub native_audio: bool,
    pub max_images_per_request: u32,
    pub max_image_pixels: u32,
    pub supports_tools: bool,
    pub supports_streaming: bool,
    pub supports_json_schema: bool,
    pub context_tokens: u32,
    pub price: Pricing,             // per input/output token, per image, per second of video
}
```

Examples of how this is used:
- `vlm_describe` sends a native clip to a provider with `native_video`, and a 3×3 frame grid otherwise.
- The agent's `view` tool sizes the grid to `max_image_pixels` and `max_images_per_request`.
- The policy sees `price` so it can weigh a `view` against remaining budget.

## Adapters

| Adapter | Traits | Notes |
|---|---|---|
| `openai_compat` | Vlm, Llm, Asr, TextEmbedder | Chat completions, embeddings, audio transcription endpoints. Covers OpenAI, vLLM, Ollama, LM Studio, llama.cpp server, Together, Groq, and Whisper servers. The single most important adapter: any open model served by vLLM plugs in here. |
| `gemini` | Vlm (native video, audio), Llm, Asr, TextEmbedder | Gemini API with file upload for clips longer than inline limits. |
| `anthropic` | Vlm (images), Llm | Claude models. Frame grids for video. |
| `onnx_local` | ImageEmbedder, TextEmbedder, Ocr, plus VAD internally | In-process via ONNX Runtime: SigLIP or CLIP, a small text embedder, RapidOCR, Silero VAD. CPU by default, CUDA or CoreML execution providers when available. |
| `candle_local` | ImageEmbedder, TextEmbedder | Alternative pure-Rust backend for platforms where ONNX Runtime is awkward. |
| `whisper_local` | Asr | whisper.cpp via its server or bindings, for fully offline indexing. Not built: the faster-whisper server in `scripts/asr-server/` is reached through `openai_compat`, which covers this case. |
| `paddleocr_sidecar` | Ocr | Optional HTTP sidecar for higher-quality OCR. |

Status (2026-09-11): `openai_compat` (ASR, chat, embeddings), `anthropic`, `gemini` and `onnx_local` (SigLIP, bge-small, RapidOCR; Silero VAD is used by the `vad` operator directly) are implemented in `vi-providers` and `vi-perceive`. Gemini's file upload for clips over the inline limit, `candle_local` and `paddleocr_sidecar` are not. Adapters that live outside `vi-providers` (`onnx_local`) register through `ProviderRegistry::register_factory`.

Adding an adapter is one crate module implementing the traits and a capabilities struct. No other crate changes.

## Provider configuration

Roles bind a capability to an adapter and model. The eval harness varies these.

```toml
[providers.gemini]
adapter = "gemini"
api_key_env = "GEMINI_API_KEY"

[providers.vllm_qwen]
adapter = "openai_compat"
base_url = "http://localhost:8000/v1"
model = "Qwen/Qwen2.5-VL-32B-Instruct"

[providers.local]
adapter = "onnx_local"
image_model = "siglip-so400m-patch14-384"
text_model = "bge-small-en-v1.5"

[roles]
vlm_describe   = { provider = "gemini",    model = "gemini-2.5-flash" }
agent_llm      = { provider = "anthropic", model = "claude-sonnet-5" }
asr            = { provider = "openai_compat", model = "whisper-large-v3", base_url = "http://localhost:9000/v1" }
ocr            = { provider = "local" }
text_embed     = { provider = "local" }
image_embed    = { provider = "local" }
```

Roles used by the pipeline: `asr`, `ocr`, `image_embed`, `text_embed`, `vlm_describe`, `extract_llm`. Roles used by the agent: `agent_llm`, `agent_vlm` (defaults to `agent_llm` if multimodal), `reranker`.

## Cross-cutting behavior in `vi-providers`

- **Rate limiting**: token bucket per provider for requests and for tokens per minute, configured or learned from 429 headers.
- **Concurrency**: semaphore per provider.
- **Retries**: exponential backoff with jitter on 429, 5xx, and transport errors; idempotent by construction since all calls are pure.
- **Batching**: embedders batch to the provider's maximum; ASR batches chunks when the endpoint allows.
- **Streaming**: SSE and chunked parsers normalized to `LlmEvent { Token, ToolCall, Usage, Done }`.
- **Cost accounting**: every call records tokens, images, seconds of media, latency, and computed cost into a `Provenance` row. Costs come from the adapter's pricing table, overridable in config.
- **Prompt registry**: prompts are versioned assets in `vi-providers/prompts/*.md` with a hash; the hash is part of every cache key and Provenance row. Python and JS can register overrides at runtime.
- **Structured output**: operators that need JSON request it via `supports_json_schema` when available and fall back to instruction-plus-repair otherwise.
- **Redaction**: keys never appear in logs, errors, or Provenance.

## A/B evaluation support

Because Descriptions and Embeddings carry Provenance, several providers' outputs coexist in one Index. A query can be pinned to a Provenance filter ("only Descriptions from run X") so two configurations are compared over identical retrieval inputs. The harness in [08-evaluation](08-evaluation.md) uses this to separate "the VLM described better" from "the agent looked in the right place".

## Local versus remote decision guide

| Capability | Default | Why |
|---|---|---|
| VAD | local | Tiny model, runs everywhere |
| Image embeddings | local | SigLIP on CPU does 20 to 50 images/s; avoids per-image API cost for tens of thousands of frames |
| Text embeddings | local | Small models are good enough for retrieval; remote is optional |
| OCR | local by default, VLM for hard cases | RapidOCR is fast; VLM OCR is better on stylized text |
| ASR | remote or local server | Whisper-large quality matters; run it under vLLM or whisper.cpp on the GPU box if one exists |
| VLM describe | remote | Frontier quality per dollar; open models via vLLM for A/B |
| Agent LLM | remote | Tool use quality dominates answer accuracy |
