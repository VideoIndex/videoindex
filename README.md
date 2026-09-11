# VideoIndex

APIs to index and query long videos. A Rust core (decode, index, retrieval, agent loop) with Python and Node.js bindings, a `vi` CLI, and a server with HTTP, SSE and MCP. Applications such as the video QnA chat app are built on the SDK, never inside it.

The design lives in [`docs/`](docs/README.md); read it in order the first time. [`docs/10-roadmap.md`](docs/10-roadmap.md) tracks milestones, [`docs/DECISIONS.md`](docs/DECISIONS.md) records choices made where the design was silent, and [`docs/MACHINE-azuremc.md`](docs/MACHINE-azuremc.md) describes the azuremc host.

## Status

M0 (skeleton and decode) is complete: probe and decode through a sandboxed worker process, an embedded SQLite + FTS5 + blob index, an operator DAG scheduler, and `vi init | probe | index | status | doctor`. M1 (coarse index and search) is in progress: yt-dlp sidecar import (`.info.json`, subtitles, chapters), the content-addressed media cache, and the `YtDlp` acquirer are done; VAD, ASR, shot detection, embeddings, OCR and `vi search` are next.

## Build

Requires Rust stable, the ffmpeg development libraries, `pkg-config` and `clang` (for the libav bindings), and the `ffmpeg` CLI for the test fixture.

```sh
# Ubuntu
sudo apt install ffmpeg pkg-config clang libclang-dev libavcodec-dev libavformat-dev \
  libavutil-dev libswscale-dev libswresample-dev
# macOS
brew install ffmpeg pkg-config

cargo build --release
cargo test
```

## Use

```sh
vi doctor                                  # machine facts, toolchain, decode worker
vi probe talk.mp4                          # container, streams, chapters, keyframe interval
vi init ./talks.vidx
vi index ./talks.vidx talk.mp4 [more.mp4]  # subtitles/chapters from sidecars, 1 fps samples, pHash, thumbnails
vi index ./talks.vidx /data/videoindex/videos/incoming/   # a directory: every video in it
vi index ./talks.vidx "https://www.youtube.com/playlist?list=..."   # via yt-dlp, where YouTube is reachable
vi search ./talks.vidx "hybrid retrieval"  # BM25 over captions/OCR/descriptions, grouped by chapter
vi status ./talks.vidx                     # videos, states, sample counts, sizes, jobs
```

Every command takes `--json` for machine-readable output and `--config videoindex.toml`; `VI_*` environment variables override config keys (`VI_MEDIA__SAMPLE_MAX_DIM=320`).

## Evaluation

Benchmarks: LVBench, 1H-VideoQA and Minerva, as named in Google's [agentic video](https://blog.google/innovation-and-ai/models-and-research/gemini-models/introducing-agentic-video-in-gemini/) announcement. Model backends are abstracted so open-source and frontier models can be compared A/B on the same index.

## License

Apache-2.0 for `crates/`, `bindings/`, `eval/` and `docs/` (proposed; see the roadmap).
