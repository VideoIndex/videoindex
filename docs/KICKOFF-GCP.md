# Kickoff prompt for continuing development on the GCP A100 machine

Copy the text below into a fresh Claude Code session in this repository on the new GCP machine (an `a2-highgpu` instance with an NVIDIA A100). The first kickoff, which produced M0 on azuremc, is in [KICKOFF.md](KICKOFF.md).

---

You are continuing development of VideoIndex on a new machine: a GCP `a2-highgpu` instance with an NVIDIA A100. The previous machine (azuremc, 4 vCPUs, no GPU) completed milestone M0 and the start of M1; that work is in this repository's `main` branch. Nothing has been built on this machine yet.

## Step 1. Read the state, not just the design

Read in this order:

1. `docs/README.md`, then `docs/01-overview.md` through `docs/11-deployment.md`. These remain the specification.
2. `docs/DECISIONS.md`: every choice made where the design was silent. Do not re-decide these without a reason; append new decisions as dated bullets.
3. `docs/MACHINE-azuremc.md`: the previous host and the M0 measurements (1-hour 720p indexed in 168 s on 4 vCPUs).
4. `docs/MACHINE-RECOMMENDATIONS.md`: the sizing rationale that led to this machine.
5. `docs/10-roadmap.md`: M0 is marked done; the M1 progress note lists what remains.
6. `README.md` for build and usage; `git log --oneline` for the commit history.

## Step 2. Inspect this machine and record it

Write `docs/MACHINE.md` for this host (the azuremc file keeps its name). Record:

- GCP machine type (`curl -s -H "Metadata-Flavor: Google" http://metadata.google.internal/computeMetadata/v1/instance/machine-type`), zone, OS and version, CPU model and vCPU count, RAM, swap.
- GPU: `nvidia-smi` model, VRAM, driver and CUDA versions. Note that `a2-highgpu` A100s have **40 GB** each; if there is more than one GPU, record the count. A 32B VLM in fp16 does not fit one 40 GB GPU; plan on 7B to 8B models or AWQ/GPTQ 4-bit quantised 32B models (about 20 GB) for the vLLM A/B runs, or tensor parallel across two GPUs if present.
- Disks: OS disk, any persistent disk for `/data`, and the local SSD (`/dev/nvme*`). Put `/data/videoindex` on the persistent disk, and use the local SSD for `/dev/shm`-style frame slots (`vi-media` uses `/dev/shm` by default; check its size with `df -h /dev/shm` and raise it if under 4 GB), the Rust `target/` directory, and ffmpeg temp files.
- Toolchain versions or "missing": `rustc`, `cargo`, `python3`, `node`, `npm`, `ffmpeg`, `ffprobe`, `yt-dlp`, `docker`, `nvidia-container-toolkit`, `caddy`/`nginx`, `pkg-config`, `clang`.
- Network: whether `yt-dlp --simulate` reaches YouTube for one URL from `dataset/videolist.md`. GCP addresses are usually blocked too; the working path stays rsync from the development Mac into `/data/videoindex/videos/incoming/`. Check whether that directory has content.
- Which of `/etc/videoindex`, systemd units, listening ports, and DNS for videoindex.app exist here. This machine is the indexing and development box; serving stays on a separate small machine per `MACHINE-RECOMMENDATIONS.md`, so do not install a reverse proxy here.

Install what is missing for the build (Rust stable via rustup; `ffmpeg pkg-config clang libclang-dev libavcodec-dev libavformat-dev libavutil-dev libswscale-dev libswresample-dev libsqlite3-dev fonts-dejavu-core`; yt-dlp as the upstream binary; `cargo-deny`). Ask before installing NVIDIA drivers or CUDA if they are absent, before changing system services, and before opening ports.

## Step 3. Rebuild and re-verify M0 here

1. `cargo build --release && cargo test --workspace` must pass. Fix anything the new environment breaks (newer ffmpeg, different libclang path) and record it.
2. Regenerate the 1-hour fixture with `scripts/make_fixture_1h.sh` and re-run the timing: `vi index /data/videoindex/indexes/m0-timing.vidx /data/videoindex/videos/fixtures/synthetic-1h-720p.mp4 --policy m0 --compact`. Record the result in `docs/MACHINE.md` next to the azuremc number.
3. `vi doctor` must report the GPU. Extend it if it does not show CUDA and driver versions.

Commit after each of these works.

## Step 4. Continue M1 (`docs/10-roadmap.md`)

Already done: `LocalFile` sidecar import, the content-addressed media cache, `subtitle_import`, chapter import, directory and playlist expansion, `YtDlp`, and text-only `vi search` (BM25 per kind, RRF, chapter grouping). Implement the rest in this order, committing after each works and keeping `docs/` current:

1. **`vi-providers` skeleton**: the capability traits from `docs/07-model-providers.md` (`Vlm`, `Llm`, `Asr`, `Ocr`, `TextEmbedder`, `ImageEmbedder`, `Reranker`), `Capabilities`, role binding from config, provenance and cost accounting, retries with backoff, per-provider concurrency and rate limits. No network adapters yet beyond what ASR needs.
2. **Local ASR on the GPU**: run a Whisper-compatible server on this machine (faster-whisper server or whisper.cpp with CUDA, in a container on `127.0.0.1:9000`) and implement the `openai_compat` adapter's `Asr` method against it. Then the `Vad` operator (Silero via ONNX Runtime, CPU is fine) and the `Asr` operator that sends only speech ranges, aligned to VAD boundaries, and writes `TranscriptSpan`s with word timings. Decide and record the ONNX Runtime crate (`ort`) and where model files are cached (`/data/videoindex/models/`).
3. **`ShotBoundary`**: HSV histogram distance plus edge change ratio with an adaptive threshold, producing `shot` Segments that cover the whole video with no gaps. Validate on the 2-minute fixture, which has a hard cut every 10 s; add that assertion to the pipeline tests.
4. **`ImageEmbed`**: SigLIP through ONNX Runtime with the CUDA execution provider (CPU fallback), embedding one frame per pHash-distinct group. **Lance** replaces the vector store stub behind the `Storage` trait; `vector_search` becomes real; `put_embeddings` stores vectors.
5. **`Ocr`**: RapidOCR through ONNX Runtime on pHash-distinct frames whose text-likelihood heuristic fires; `OcrSpan`s into FTS.
6. **Hybrid search**: add text-vector and image-vector (SigLIP text tower) lists to the RRF in `vi-query`, and switch temporal grouping from chapters to scene segments once `scenes` exists (otherwise shots grouped to about 60 s).
7. **Scheduler**: budgets (`max_cost_usd_per_hour`, `max_wallclock_per_hour`) enforced, operator output cache keyed per `docs/05-indexing-pipeline.md`, per-stage resume instead of the current all-or-nothing rule, and provider-error handling that marks failed ranges and continues.
8. **`Http` and `ObjectStore` acquirers** with the private-address allowlist and size limits.
9. Update the default policy to the design's `lecture_default` once its operators exist, keeping `coarse_local` for machines without providers.

Definition of done for M1 is unchanged: the two playlists in `dataset/videolist.md` index to `coarse` unattended once transferred into `incoming/`, and `vi search` answers transcript, OCR, and visual queries with correct timestamps on the 50-question dev set's retrieval subset (write that dev set as `dataset/devset.jsonl` when the videos arrive; until then use the synthetic fixture and the sidecar tests).

## Conventions (unchanged)

- Crate names `vi-*`; module and type names per the glossary in `docs/README.md`.
- Errors: `thiserror` in libraries, `anyhow` only in `vi-cli`. No `unwrap` outside tests. Every `unsafe` block carries a `// SAFETY:` comment (clippy enforces both).
- Async with tokio; CPU-bound work on rayon via `vi_core::cpu::run`; GPU work through the provider layer or ONNX Runtime sessions owned by the operator; never block the runtime.
- Conventional commit messages; one logical change per commit. `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo deny check` must pass before each commit; CI runs the same on Ubuntu and macOS.
- No Python or Node bindings until M2. Provider adapters only as far as M1 needs them (`openai_compat` for ASR).
- If implementation forces a design change, edit the relevant doc in the same commit and add a dated line to `docs/DECISIONS.md`.

## After M1

Report what was built, the timings measured on this machine (coarse pass per hour of video with and without ASR, GPU utilisation, cost per hour if any hosted provider was used), and what in the design turned out wrong or unclear. Then start M2 with `vi-providers` adapters for Gemini, Anthropic and `openai_compat` chat, and the `vi-agent` loop.
