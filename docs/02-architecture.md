# 02. Architecture

## Layers

```mermaid
flowchart TB
    subgraph Apps["Applications (outside the SDK)"]
        Chat["Video QnA chat app"]
        Agents["External agents via MCP"]
        Notebooks["Eval harness, notebooks"]
    end

    subgraph Surface["SDK surface"]
        Py["Python (PyO3)"]
        Node["Node.js (napi-rs)"]
        CLI["vi CLI"]
        Server["vi-server: HTTP, SSE, WebSocket, MCP"]
    end

    subgraph Core["Rust core"]
        Agent["vi-agent: agentic loop, budgets, policies"]
        Query["vi-query: hybrid retrieval, temporal fusion, tools"]
        Pipeline["vi-pipeline: operator DAG, scheduler, checkpoints"]
        Perceive["vi-perceive: shot detection, VAD, embeddings (in-process)"]
        Providers["vi-providers: VLM, ASR, OCR, embed, LLM adapters"]
        Media["vi-media: acquire, demux, decode, seek, sample"]
        Index["vi-index: storage trait, embedded backend, blob cache"]
        CoreTypes["vi-core: types, time, config, events, errors"]
    end

    subgraph External["External"]
        FF["ffmpeg libav (sandboxed)"]
        ONNX["ONNX Runtime / candle"]
        APIs["Gemini, Claude, OpenAI, vLLM, Ollama"]
        Store["SQLite + Lance | Postgres + pgvector | Qdrant | S3"]
    end

    Apps --> Surface --> Agent & Query & Pipeline
    Agent --> Query --> Index
    Agent --> Providers
    Pipeline --> Media & Perceive & Providers & Index
    Media --> FF
    Perceive --> ONNX
    Providers --> APIs
    Index --> Store
    Core --> CoreTypes
```

Dependencies point downward only. `vi-core` has no dependency on any other crate. Nothing in `Core` knows about Python, Node, or HTTP.

## Cargo workspace

```
videoindex/
  Cargo.toml                 workspace
  crates/
    vi-core/                 shared types, time model, config, events, errors
    vi-media/                acquirers, ffmpeg demux/decode, hardware decode, seeking, sampling, tiling
    vi-perceive/             in-process operators: shot boundary, pHash, VAD, image/text embeddings
    vi-providers/            provider traits + adapters, rate limiting, retries, cost accounting
    vi-index/                storage trait, embedded backend (SQLite+FTS5, Lance/usearch), blob cache
    vi-pipeline/             operator DAG, scheduler, checkpoints, progress, caching
    vi-query/                retrieval, temporal fusion, tool primitives
    vi-agent/                default agentic loop, budgets, policy trait
    vi-server/               axum HTTP/SSE/WebSocket API, MCP server
    vi-cli/                  `vi` binary
  bindings/
    python/                  PyO3 + maturin -> `videoindex` wheel
    node/                    napi-rs -> `@videoindex/core` package
  eval/                      Python: benchmark loaders, runners, reports
  apps/
    chat/                    demo QnA app (separate deployable, consumes SDK)
    site/                    videoindex.app SDK site (later)
  docs/
  dataset/
  scripts/
```

Crate responsibilities are described in the documents that follow. The one-line rule for placing code: if it needs frames, threads, storage, or sockets it goes in a `vi-*` crate. If it is a prompt, a policy, or an experiment it can start in Python and be ported to Rust once it stabilizes.

## Indexing data flow

```mermaid
sequenceDiagram
    participant U as User / SDK
    participant P as vi-pipeline
    participant M as vi-media
    participant PE as vi-perceive
    participant PR as vi-providers
    participant I as vi-index

    U->>P: index(source, policy, budget)
    P->>M: acquire(source)  // download or open
    M-->>P: local media handle + probe (duration, streams, codec)
    P->>I: create Video, Tracks
    P->>M: decode audio
    M-->>P: 16 kHz mono chunks
    P->>PE: VAD
    P->>PR: ASR(speech chunks)
    PR-->>I: TranscriptSpans
    P->>M: decode video at 1 fps (keyframe-aligned)
    M-->>P: FrameSamples
    P->>PE: shot boundaries, pHash dedup, image embeddings
    PE-->>I: Segments(shot), Embeddings
    P->>PR: OCR(distinct frames)
    PR-->>I: OcrSpans
    P->>I: mark coarse pass complete  // queries now work
    P->>PR: VLM describe(scene segments, frame grids)
    PR-->>I: Descriptions, Entities, Events
    P->>PR: text embeddings(spans, descriptions)
    PR-->>I: Embeddings
    P->>I: mark fine pass complete
    P-->>U: progress events throughout, final summary
```

The pipeline is a DAG of operators, not a fixed sequence. The sequence above shows the default policy. Each arrow into `vi-index` is checkpointed, so a killed job resumes.

## Query data flow

```mermaid
sequenceDiagram
    participant U as User / SDK
    participant A as vi-agent
    participant Q as vi-query
    participant I as vi-index
    participant M as vi-media
    participant PR as vi-providers

    U->>A: ask(question, budget)
    A->>Q: search(question)  // hybrid: vector + BM25 + temporal
    Q->>I: query spans, descriptions, segments
    I-->>Q: ranked hits with timestamps
    Q-->>A: candidate segments
    A->>PR: LLM decides: answer now or look closer?
    alt needs pixels
        A->>M: view(t0, t1, fps, res)  // decode on demand
        M-->>A: frame grid
        A->>PR: VLM(frame grid + question)
        PR-->>A: observation
    end
    A->>PR: LLM compose answer with citations
    A-->>U: streamed tokens + citations [t=HH:MM:SS]
```

Retrieval is always tried first because it is cheap and cached. Decoding and VLM calls happen only when the agent decides the index does not contain the answer, and only within budget.

## Process model

VideoIndex runs in three shapes from the same crates:

| Shape | Entry | Use |
|---|---|---|
| **Library** | Python or Node binding | Embedded in a user's process. Pipeline and query run in the binding's tokio runtime, off the GIL / event loop. |
| **CLI** | `vi` | Batch indexing, scripting, eval. |
| **Server** | `vi-server` | Hosted or shared deployments. HTTP + SSE for answers, WebSocket for progress, MCP for agents. Multi-index, auth, quotas. |

Decoding of untrusted media always runs in a child process (`vi-media` worker) with resource limits, regardless of shape. See [03-rust-boundary](03-rust-boundary.md#security).

## Concurrency model

- One tokio runtime per process. CPU-bound work (decode, embeddings, hashing) runs on a dedicated rayon pool or in the decode worker process; the async runtime never blocks.
- Provider calls are async with per-provider concurrency limits and token-bucket rate limits.
- Frames move as `Arc<FrameBuffer>` with a pixel format tag. No copies between decode, hashing, embedding, and tiling. Frames cross the Python boundary as NumPy arrays via the buffer protocol without a copy.
- Backpressure: the scheduler bounds in-flight frames per job so an hour of 4K video never sits in memory.

## Deployment shapes

- **Local**: `pip install videoindex` or `npm install @videoindex/core`, index lives in a directory.
- **Self-hosted**: `vi serve` behind a reverse proxy, indexes on local disk or object storage.
- **Hosted (videoindex.app)**: `vi-server` with the pluggable storage backends, multi-tenant, quotas, keys. See [11-deployment](11-deployment.md).
