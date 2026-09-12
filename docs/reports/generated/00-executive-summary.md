# Executive summary

VideoIndex turns long videos into a queryable knowledge base: an SDK and infrastructure
framework whose core is Rust, with a Python binding, a CLI and (planned) a server with HTTP,
SSE and MCP. Applications such as a video question-answering chat sit on top of the SDK.

## Where the project stands

- **M0 (skeleton) and M1 (coarse index) are complete.** Every video that enters the system is
  decoded in a sandboxed worker, sampled at 1 fps, hashed, thumbnailed, cut into shots, transcribed
  (Silero VAD + Whisper large-v3 on the GPU), read for on-screen text (RapidOCR), and embedded
  (SigLIP for frames, bge-small for text). Hybrid search fuses BM25, text vectors and image vectors.
- **M2 is largely delivered.** Provider adapters for OpenAI-compatible servers, Anthropic and Gemini;
  an agent loop with tools, budgets, sessions and timestamp citations (`vi ask`); the fine-pass
  operators (scenes, chapters, VLM descriptions, entities and events); a Python binding with
  operators and agent policies written in Python.
- **Measured on a 30-video, 36.6-hour dataset** (AI Engineer conference workshops and the Berkeley
  Agentic AI MOOC): retrieval hit@5 0.94 and MRR 0.72 on 72 questions; question answering 96.2%
  with citations on 52 questions at $0.05 per question, against a 73.1% retrieval-only baseline.
  Coarse indexing runs at 56× real time on the GPU build.

## What this document contains

Part I reproduces the design documents that define the system (overview, architecture, the Rust
boundary, data model, indexing pipeline, query and agents, model providers, evaluation, SDK
surfaces, deployment). Part II covers the milestones and the M1/M2 report with measurements.
Part III is generated from the repository: the work log by day, code and test counts, the dated
decisions made where the design was silent, and the facts about the development machine.

## Next steps

1. Re-index the dataset with the CUDA build to measure the GPU end to end (about two hours).
2. A measured fine pass over the dataset (roughly $60 at $1.60 per hour of video with Claude Sonnet 5).
3. Programmatic comparison against Gemini's agentic video understanding on the same questions.
4. M3: Node binding, `vi-server` (HTTP, SSE, MCP) and the chat application at videoindex.app.
