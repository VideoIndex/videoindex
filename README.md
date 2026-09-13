# VideoIndex

APIs to index and query long videos. A Rust core (decode, index, retrieval, agent loop) with Python and Node.js bindings, a `vi` CLI, and a server with HTTP, SSE and MCP. Applications such as the video QnA chat app are built on the SDK, never inside it.

The design lives in [`docs/`](docs/README.md); read it in order the first time. [`docs/10-roadmap.md`](docs/10-roadmap.md) tracks milestones and [`docs/results/`](docs/results/) holds the benchmark results pages. Deployment notes, machine facts, kickoff prompts and the dated decisions log are internal and live in the `vi_internal` repository.

## Status

M0 (skeleton and decode) is complete: probe and decode through a sandboxed worker process, an embedded SQLite + FTS5 + blob index, an operator DAG scheduler, and `vi init | probe | index | status | doctor`. M1 (coarse index and search) is in progress: yt-dlp sidecar import (`.info.json`, subtitles, chapters), the content-addressed media cache, the `YtDlp` acquirer, text `vi search`, the provider layer (`vi-providers`), Silero VAD and Whisper ASR through an OpenAI-compatible server, shot detection, SigLIP and bge embeddings with a vector store, RapidOCR, hybrid `vi search`, budgets and the operator cache, and the `Http`/`ObjectStore` acquirers are done; the two-playlist run and the dev set close M1. M2 has started: chat adapters for OpenAI-compatible servers, Anthropic and Gemini, and the `vi ask` agent loop with tools, budgets and citations.

## Build

Requires Rust stable, the ffmpeg development libraries, `pkg-config` and `clang` (for the libav bindings), and the `ffmpeg` CLI for the test fixture.

```sh
# Ubuntu
sudo apt install ffmpeg pkg-config clang libclang-dev libavcodec-dev libavformat-dev \
  libavutil-dev libswscale-dev libswresample-dev
# macOS
brew install ffmpeg pkg-config

cargo build --release
# with an NVIDIA GPU and CUDA 13 + cuDNN 9 installed (ONNX models on the GPU):
cargo build --release -p vi-cli --features cuda
cargo test
```

Bindings: `bindings/python` (PyO3, `maturin develop`) and `bindings/node` (napi-rs, `npm run build`). The HTTP/SSE/MCP server is `vi serve`; the demo app and SDK docs live in the `videoindex_app` repository.

## Use

```sh
vi doctor                                  # machine facts, toolchain, decode worker
vi probe talk.mp4                          # container, streams, chapters, keyframe interval
vi init ./talks.vidx
vi index ./talks.vidx talk.mp4 [more.mp4]  # subtitles/chapters from sidecars, 1 fps samples, pHash, thumbnails
vi index ./talks.vidx /data/videoindex/videos/incoming/   # a directory: every video in it
vi index ./talks.vidx "https://www.youtube.com/playlist?list=..."   # via yt-dlp, where YouTube is reachable
vi index ./talks.vidx https://cdn.example.com/talks/day1.mp4 s3://bucket/talks/   # direct downloads, object stores
vi --config config/gcp-a100.toml index ./talks.vidx talk.mp4 --policy coarse_asr   # + VAD and Whisper ASR (scripts/asr_server.sh start)
vi search ./talks.vidx "hybrid retrieval"  # BM25 + text and image vectors, grouped by shot/scene
vi search ./talks.vidx "slide with a diagram" --kind frame   # SigLIP text-to-frame search
vi ask ./talks.vidx "When do they discuss evaluation?"   # agentic answer with [HH:MM:SS] citations (needs an agent_llm role)
vi view ./talks.vidx <video-id> --t0 1830 --t1 1860 --fps 1 -o grid.png   # labelled frame grid
vi timeline ./talks.vidx <video-id> --level shot
vi status ./talks.vidx                     # videos, states, sample counts, sizes, jobs
```

Every command takes `--json` for machine-readable output and `--config videoindex.toml`; `VI_*` environment variables override config keys (`VI_MEDIA__SAMPLE_MAX_DIM=320`).

## Python

```sh
cd bindings/python && maturin develop --release   # into an active virtualenv; needs the same build deps as the crates
```

```python
import videoindex as vi
idx = vi.Index.open("./talks.vidx", config=vi.Config.from_file("config/gcp-a100.toml"))
hits = idx.search("hybrid retrieval", k=5)
for ev in idx.ask("When do they discuss evaluation?"):
    if ev["type"] == "token": print(ev["text"], end="")
```

`bindings/python/README.md` has the full surface. The decode worker is a separate binary, so set `media.worker.path` to a built `vi` or `vi-media-worker` when the package is used outside this repository (wheels will bundle it later).

## Evaluation

Benchmarks: LVBench, 1H-VideoQA and Minerva, as named in Google's [agentic video](https://blog.google/innovation-and-ai/models-and-research/gemini-models/introducing-agentic-video-in-gemini/) announcement. Model backends are abstracted so open-source and frontier models can be compared A/B on the same index.

## License

Apache-2.0 for `crates/`, `bindings/`, `eval/` and `docs/` (proposed; see the roadmap).
