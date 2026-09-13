# Machine recommendations: indexing and serving

Written 2026-09-11 after M0, from measurements on the current azuremc host (see [MACHINE-azuremc.md](MACHINE-azuremc.md)) and the data volumes in [08-evaluation](08-evaluation.md) and [11-deployment](11-deployment.md). Two machines are recommended: a powerful indexing and development box that is deallocated between runs, and a small always-on serving box for videoindex.app.

## Sizing inputs

| Fact | Value | Source |
|---|---|---|
| Coarse pass (1 fps sample, pHash, thumbnails) on 1 h of 720p H.264 | 168 s wall on 2 physical cores; about 2.4 cores busy per job; 90 MB peak RSS | `MACHINE-azuremc.md`, M0 measurements |
| Video to index | LVBench ~117 h (100 to 150 GB at 720p); dataset playlists ~40 to 60 h (30 to 60 GB); Minerva if obtainable ~100 GB | `08-evaluation.md` |
| Index size | 30 to 80 MB per hour of video; 5 to 10 GB total for the benchmarks | `04-data-model.md`, `11-deployment.md` |
| Model weights | Whisper-large ~3 GB, SigLIP so400m ~1.7 GB, a 7B VLM ~15 GB, a 32B VLM ~65 GB | vendor model cards |
| Rust workspace `target/` | ~15 GB (debug + release) | observed |
| Whisper-large throughput | CPU: ~1× real time; A10 (faster-whisper, fp16): ~10 to 20× real time; A100 with batching: 50× or more | public faster-whisper benchmarks |
| SigLIP embeddings | CPU: 20 to 50 images/s; GPU: thousands/s | `07-model-providers.md`, public benchmarks |
| Serving targets | retrieval < 50 ms; 10 s window decode at 2 fps < 300 ms; first token < 2 s excluding provider latency | `03-rust-boundary.md`, `06-query-and-agents.md` |

## 1. Indexing and development machine

Batch indexing (decode, ASR, embeddings, OCR, VLM A/B runs), Rust builds, evaluation sweeps. Idle between runs: deallocate it when not indexing.

| Component | Recommended | Minimum that still works |
|---|---|---|
| Azure SKU | `Standard_NC24ads_A100_v4` (24 vCPU AMD EPYC Milan, 220 GB RAM, 1× A100 80 GB, ~960 GB local NVMe) | `Standard_NV36ads_A10_v5` (36 vCPU, 440 GB RAM, 1× A10 24 GB) |
| vCPUs | 24 | 16 |
| RAM | 220 GB | 64 GB |
| GPU VRAM | 80 GB | 24 GB |
| OS disk | 128 GB Premium SSD | 128 GB |
| Data disk at `/data` | 1 TB Premium SSD v2 (2 TB if Minerva is added) | 1 TB |
| Local NVMe | `/dev/shm` frame slots, Rust `target/`, ffmpeg temp | same |
| OS and stack | Ubuntu 24.04, NVIDIA driver 550+, CUDA 12.x, Docker with nvidia-container-toolkit | same |

Rationale:

- **CPU.** Decode dominates the coarse pass. Roughly 180 hours of video (LVBench plus the dataset playlists) takes about 9 hours on the current 4 vCPUs. With 24 vCPUs and 8 parallel jobs it is about 1.5 hours. Release builds drop from several minutes to under one.
- **GPU.** Whisper-large on CPU would need about a week for 180 hours of audio; on an A10 about a day; on an A100 a few hours. SigLIP for ~650k sampled frames is minutes on either GPU and 4 to 9 hours on CPU. The 80 GB A100 is what allows a 32B open VLM (Qwen2.5-VL-32B in fp16 needs ~64 GB) under vLLM for the A/B comparisons in [07-model-providers](07-model-providers.md). The A10 caps local VLMs at 7B to 8B, still useful for a cheaper baseline.
- **RAM.** Indexing itself is light. The headroom is for vLLM, ONNX Runtime, and pandas in the eval harness. 64 GB is enough without a local VLM.
- **Disk.** Media 250 to 350 GB, indexes 5 to 10 GB, model weights 20 to 70 GB, Rust target 15 GB: 400 to 500 GB now. 1 TB leaves room for a second set of index configurations.
- **Quota.** GPU vCPU quota for the NC or NV family must be requested per region; allow a day.

## 2. Serving machine (videoindex.app demo)

Runs nginx, `vi serve` (HTTP, SSE, MCP), the Node chat app, and on-demand decode of short windows for the agent's `view` tool. All VLM and LLM calls go to hosted providers, so no GPU.

| Component | Recommended | Minimum |
|---|---|---|
| Azure SKU | `Standard_D8s_v5` (8 vCPU, 32 GB) | `Standard_D4s_v5` (4 vCPU, 16 GB), or the current azuremc once its other apps are moved off |
| vCPUs | 8 | 4 |
| RAM | 32 GB | 16 GB |
| OS disk | 64 GB | 64 GB |
| Data disk at `/data` | 512 GB Premium SSD | 256 GB |
| Network | public IP, ports 80/443 only; `vi serve` on 127.0.0.1:8080 (8090 on a host where 8080 is taken) and the chat app on 127.0.0.1:3000 | same |
| OS and stack | Ubuntu 24.04, Docker, nginx or Caddy, Node 20+ | same |

Rationale:

- **CPU.** A `view` call decodes a few seconds from a keyframe, under a core-second at 720p. 8 vCPUs handles a handful of concurrent demo users with headroom for SQLite, the Node app, and query-time text embeddings on CPU (bge-small is a few ms per query).
- **Disk.** The serving box needs the dataset playlists' media (for `view`) plus their index, about 100 GB. It does not need the benchmark media. 512 GB covers growth and a second index.
- **RAM.** SQLite page cache for a 10 GB index plus Node plus decode workers fits in 16 GB; 32 GB avoids swapping when several `view` decodes overlap.

## Moving between machines

- Both machines get the setup in the repository `README.md` (apt packages, rustup, `cargo build --release`), plus the GPU stack on the indexing box. `vi doctor` verifies the result and reports GPU, hwaccel methods, and the incoming directory.
- Copy `/data/videoindex/` with rsync. Indexes are portable directories; an index built on the GPU box copies to the serving box unchanged, and the content-addressed media cache means the same files are found by hash on either side.
- The current azuremc has nginx and another app on port 8080. If it stays as the serving box, those must move or `vi serve` stays on 8090 behind an nginx site, which is what [DECISIONS.md](DECISIONS.md) now assumes.
- Keep both machines in the same region so index and media copies are fast and free of egress charges.

## Open items

- Which SKUs are chosen. Once decided, this file gets exact provisioning steps (driver, CUDA, container toolkit, data disk mount, quota request) and [11-deployment](11-deployment.md) is updated for the two-machine layout.
- Whether ASR runs locally on the GPU box (whisper.cpp or faster-whisper server) or through a hosted Whisper-compatible endpoint; the GPU recommendation assumes local.
