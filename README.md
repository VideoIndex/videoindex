# VideoIndex

APIs to index and query long videos. A Rust core (decode, index, retrieval, agent loop) with Python and Node.js bindings, a `vi` CLI, and a server with HTTP, SSE and MCP. Applications such as the video QnA chat app are built on the SDK, never inside it.

The design lives in [`docs/`](docs/README.md); read it in order the first time. [`docs/10-roadmap.md`](docs/10-roadmap.md) tracks milestones, [`docs/DECISIONS.md`](docs/DECISIONS.md) records choices made where the design was silent, and [`docs/MACHINE.md`](docs/MACHINE.md) describes the azuremc host.

## Status

M0 (skeleton and decode) is complete: probe and decode through a sandboxed worker process, an embedded SQLite + FTS5 + blob index, an operator DAG scheduler, and `vi init | probe | index | status | doctor`. M1 (coarse index and search) is next.

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
vi index ./talks.vidx talk.mp4 [more.mp4]  # sample at 1 fps, pHash, WebP thumbnails
vi status ./talks.vidx                     # videos, states, sample counts, sizes, jobs
```

Every command takes `--json` for machine-readable output and `--config videoindex.toml`; `VI_*` environment variables override config keys (`VI_MEDIA__SAMPLE_MAX_DIM=320`).

## Evaluation

Benchmarks: LVBench, 1H-VideoQA and Minerva, as named in Google's [agentic video](https://blog.google/innovation-and-ai/models-and-research/gemini-models/introducing-agentic-video-in-gemini/) announcement. Model backends are abstracted so open-source and frontier models can be compared A/B on the same index.

## License

Apache-2.0 for `crates/`, `bindings/`, `eval/` and `docs/` (proposed; see the roadmap).
