# Executive summary

VideoIndex turns long videos into a queryable knowledge base: an SDK and infrastructure
framework whose core is Rust, with a Python binding, a CLI and (planned) a server with HTTP,
SSE and MCP. Applications such as a video question-answering chat sit on top of the SDK.

## Where the project stands

- **M0 (skeleton) and M1 (coarse index) are complete.** Every video that enters the system is
  decoded in a sandboxed worker, sampled at 1 fps, hashed, thumbnailed, cut into shots, transcribed
  (Silero VAD + Whisper large-v3 on the GPU), read for on-screen text (RapidOCR), and embedded
  (SigLIP for frames, bge-small for text). Hybrid search fuses BM25, text vectors and image vectors.
- **M2 is delivered.** Provider adapters for OpenAI-compatible servers, Anthropic and Gemini;
  an agent loop with tools, budgets, sessions and timestamp citations (`vi ask`); the fine-pass
  operators (scenes, chapters, VLM descriptions, entities and events); a Python binding with
  operators and agent policies written in Python.
- **M3 is built and verified locally.** `vi-server` (HTTP API, SSE, blobs, API keys with a daily
  spend cap, MCP, metrics, OpenAPI) and `vi serve`; the Node binding; and, in the separate
  `videoindex_app` repository, the site, the demo chat app with a player that seeks to citations,
  the SDK docs and the deployment templates. Pointing videoindex.app at the host is the remaining step.
- **M4 has started.** The `eval/` harness runs LVBench, MINERVA and 1H-VideoQA; a first LVBench pilot
  (100 questions over 17 videos) scores the agent at 68% against 50% for retrieval-only and 52% for a
  32-frame uniform-sampling baseline. Full-set runs wait on the remaining video downloads.
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

1. Finish the LVBench acquisition (YouTube's bot check paces it), run the full 1,549 questions with the
   agent, retrieval-only and uniform baselines, then MINERVA; report in `docs/results/`.
2. Point videoindex.app, api. and docs. at the host (DNS, Caddy, systemd units in `videoindex_app/deploy`).
3. Compare the fine index (VLM descriptions, entities, events) against the coarse one on the dev set.
4. M5: hosted mode (Postgres + pgvector + object storage behind the Storage trait, tenants, quotas).
