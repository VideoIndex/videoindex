# 01. Overview

## The problem

Long videos are the least searchable common media type. An hour-long conference workshop, a lecture series, a recorded meeting, or a security camera day holds information that today is reached only by scrubbing. Frontier video-language models can now answer questions about video, but sending an hour of frames to a model per question is slow, expensive, and non-repeatable. The recent shift, exemplified by Gemini's agentic video understanding, is to let a model decide *what to watch, at what rate, through which modality* instead of feeding every frame uniformly. Google reports up to 88% fewer tokens and up to 66% lower cost with higher accuracy from that approach.

That approach needs infrastructure the model does not provide: fast decoding and seeking, a persistent multimodal index, retrieval, budgets, caching, and a stable tool surface. VideoIndex is that infrastructure.

## What VideoIndex is

VideoIndex is:

1. **An indexing engine.** It ingests long videos once and produces a persistent, portable knowledge base: segments, transcripts, on-screen text, visual descriptions, entities, events, and embeddings, each with timestamps and provenance.
2. **A query engine.** It answers questions over that knowledge base using hybrid retrieval plus an agentic loop that can go back to the pixels when needed, with timestamped citations.
3. **A framework.** Models, storage, and operators are swappable behind traits so that open-source and frontier models can be compared A/B on the same index.
4. **An SDK.** A Rust core with first-class Python and Node.js bindings, a CLI, and a server mode with HTTP, streaming, and MCP so any agent framework can use an index as a tool.

The analogy is LlamaIndex for documents: an open-source SDK that people run locally, with a hosted version available for those who do not want to run infrastructure.

## Goals

- Index hour-scale videos at a cost and time that make batch indexing of hundreds of hours practical.
- Answer questions interactively, with first tokens within a couple of seconds and citations to exact timestamps.
- Work with any model backend: Gemini, Claude, OpenAI, vLLM or Ollama-hosted open models, and local ONNX models for lightweight perception.
- Make a video index a portable artifact: copy a directory and query it elsewhere.
- Be measurable: run LVBench, Minerva, and 1H-VideoQA end to end and report accuracy, tokens, cost, and latency per configuration.
- Keep the SDK independent of any application. The chat app is a consumer of the SDK.

## Non-goals for v1

- Live stream ingestion (RTSP, HLS, WebRTC). The architecture leaves room for it, but v1 is files and URLs.
- Video editing, transcoding as a product, or generation.
- Training or fine-tuning models.
- Bindings beyond Python and Node.js. Other languages use server mode.
- A general-purpose vector database. Storage is embedded first, pluggable second.

## Who uses it

- **SDK developers** building agentic applications who want video as a first-class retrievable source alongside documents.
- **Application developers** building the video QnA chat app or similar products on top.
- **Researchers** comparing sampling strategies and model backends on long-video benchmarks.
- **Hosted customers** who want the API without running decoders and indexes themselves.

## Guiding principles

1. **Coarse to fine.** Index cheaply and broadly first, then spend model budget only where a query or a policy asks for it. Every expensive step must be justified by retrieval or an explicit request.
2. **Budgets are first class.** Every indexing job and query has token, cost, and time budgets. Running out of budget produces a partial, honest answer, never a hang.
3. **Provider agnostic.** No provider-specific type escapes the provider layer. The same index must be queryable with a different VLM than the one that built it.
4. **Provenance everywhere.** Every stored fact records the operator, provider, model version, and prompt that produced it. This is what makes A/B comparison and cache invalidation possible.
5. **Portable index.** The embedded backend produces a self-contained directory with a versioned manifest. Hosted backends implement the same storage trait.
6. **Untrusted input.** Video files, URLs, transcripts, and on-screen text are attacker-controlled data. Decoders are sandboxed and text derived from video is never treated as instructions.
7. **Rust for the machine, Python for the ideas.** Everything that touches bytes, threads, storage, or the network is Rust. Prompts, agent policies, and evaluation logic iterate in Python with Rust defaults. See [03-rust-boundary](03-rust-boundary.md).

## Positioning

| | VideoIndex | Gemini agentic video | LlamaIndex | Twelve Labs-style APIs |
|---|---|---|---|---|
| Open source | Yes, SDK | No | Yes | No |
| Runs locally | Yes | No | Yes | No |
| Model choice | Any | Gemini only | Any LLM | Vendor models |
| Persistent index | Yes, portable | Per-request | Yes | Hosted only |
| Video native | Yes | Yes | Minimal | Yes |
| Agent tool surface | Native + MCP | Internal | Native | REST |

VideoIndex adopts the agentic-video idea and makes it model-independent, persistent, and open.
