# GCP machine facts

Recorded 2026-09-11 at the M1 kickoff on the GCP A100 machine. `vi doctor` reports the live values; this file is the snapshot the M1 decisions were made against. The previous host is described in [MACHINE-azuremc.md](MACHINE-azuremc.md).

## Hardware

| Item | Value |
|---|---|
| Hostname | `instance-20260911-162245`, GCP `a2-ultragpu-1g`, zone `us-central1-c` |
| OS | Ubuntu 26.04.1 LTS, kernel 7.0.0-1011-gcp, x86_64 |
| CPU | Intel Xeon @ 2.20 GHz (Cascade Lake class), **12 vCPUs** (6 cores × 2 threads), AVX-512 |
| RAM | 167 GiB total, 164 GiB available, **no swap** |
| GPU | **1 × NVIDIA A100-SXM4-80GB** (80 GB, not the 40 GB the kickoff assumed for `a2-highgpu`). Driver 595.91.07, CUDA 13.2 driver API. CUDA toolkit 13.1 and cuDNN 9 installed on 2026-09-11 from NVIDIA's `ubuntu2404` apt repository; see "GPU stack" below. |

The design docs assume an 8-core server; this machine has 12 vCPUs, so timings here are above the design baseline but well below the 24 vCPUs recommended in `MACHINE-RECOMMENDATIONS.md`.

## Disks

| Device | Size | Free | Mount | Notes |
|---|---|---|---|---|
| `/dev/sda1` | 750 G | 715 G | `/` | OS persistent disk (ext4). The repository and the Rust `target/` live here. |
| `/dev/sdb` | 2 T | 1.9 T | `/data` | Persistent disk (ext4, `discard`). **`/data/videoindex` lives here**: `videos/` (media cache, `incoming/`, `fixtures/`), `indexes/`, `models/`, `asr/` (Python venv), `logs/`. |
| `/dev/nvme0n1` | 375 G | – | **not mounted** | Local SSD, blank (no filesystem). The kickoff planned to put `target/`, `/dev/shm`-style frame slots and ffmpeg temp files here; formatting it was not done in the kickoff session (see DECISIONS.md). `/dev/shm` is a 84 G tmpfs, far above the 4 G the frame slots need, so nothing is lost by leaving it unused. |

`/data/videoindex/videos/incoming/` holds the two dataset playlists transferred from the Mac: **30 MP4 files, 5.1 GB, 36.6 hours**, every one with its `.info.json` and 2 to 4 `.srt` sidecars (`PLcfpQ4tk2k0XfT3YLdjLXKHAaBJ9MX6re` AI Engineer workshops, 19 files; `PLS01nW3RtgoqGkm4UeqNeZLccW-OGc1fJ` Berkeley Agentic AI MOOC, 11 files) plus yt-dlp's `archive.txt`.

## Toolchain

| Tool | At kickoff | Now |
|---|---|---|
| `rustc` / `cargo` / `rustup` | missing | **installed** via rustup, stable 1.98.1 (`~/.cargo/bin`) |
| `cargo-deny` | missing | installed via `cargo install` |
| `gcc`, `cmake`, `pkg-config`, `clang` 21, `libclang-dev` | missing | **installed** (`build-essential`, `cmake` 4.2) |
| `ffmpeg` / `ffprobe` | missing | **installed** 8.0.1 with dev headers: libavformat 62.3, libavcodec 62, libswscale, libswresample, libavfilter, libavdevice. `ffmpeg-next` 9.0 built against it without changes. |
| `libsqlite3-dev`, `fonts-dejavu-core` | missing | installed |
| `python3` | 3.14.4 (system) | unchanged. Too new for the ML wheels, so the ASR server runs in a **uv-managed Python 3.12 venv** at `/data/videoindex/asr/.venv` |
| `uv` | missing | installed in `~/.local/bin` |
| `node` / `npm` | missing | not installed (M3) |
| `yt-dlp` | missing | **installed** standalone binary 2026.08.19 in `/usr/local/bin` |
| `docker`, `nvidia-container-toolkit` | missing | **not installed**. Adding Docker is a system-service change; the ASR server runs without it (see below). |
| `caddy`, `nginx` | missing | not installed on purpose: this is the indexing box, serving is a separate machine |
| `git` | 2.53.0 | unchanged; `gh` missing |

## GPU stack

- `nvidia-smi` works; the driver supports CUDA 13.2. The machine came without a toolkit. `cuda-toolkit-13-1`, `libcudnn9-cuda-13` and `libcudnn9-dev-cuda-13` were installed from NVIDIA's `ubuntu2404` repository (`cuda-keyring`; Ubuntu 26.04's own package is CUDA 12.4, which the ONNX Runtime binaries below cannot use). `ldconfig` now lists `libcudart.so.13`, `libcublas.so.13`, `libcudnn.so.9`, `libcufft`, `libcurand` and `libnvrtc`; `nvcc` is at `/usr/local/cuda-13.1/bin/nvcc`.
- **ASR**: faster-whisper 1.2.1 on CTranslate2 4.8.2 in the Python 3.12 venv, with `nvidia-cublas-cu12` and `nvidia-cudnn-cu12` pip wheels supplying the CUDA 12 runtime libraries in user space (`scripts/asr_server.sh` puts their `lib/` directories on `LD_LIBRARY_PATH`; CTranslate2 does not find them by itself). `ctranslate2.get_cuda_device_count()` reports 1. First measurement: a 60 s lecture clip transcribes in 1.46 s with word timestamps (41× real time, batch 16, beam 5, fp16), 4 GB of VRAM in use. In the pipeline (`coarse_asr` policy, 4 concurrent requests of up to 120 s, server batch 16): a 34-minute talk with 31.7 minutes of speech transcribed in 65 s, 29× real time; the whole coarse pass with subtitles, VAD, ASR, sampling, pHash and thumbnails took 81 s and was bounded by the video decode (sample stage 81 s). The server (`scripts/asr-server/server.py`, started by `scripts/asr_server.sh`) exposes the OpenAI `/v1/audio/transcriptions` API on `127.0.0.1:9000` with `large-v3` in fp16. Model weights cache in `/data/videoindex/models/whisper/`.
- **ONNX Runtime** (VAD, SigLIP, RapidOCR, bge-small): `ort` 2.0.0-rc.13 downloads ONNX Runtime 1.28 built for CUDA 13 when `vi` is compiled with `cargo build --release -p vi-cli --features cuda`; the build drops `libonnxruntime_providers_cuda.so` and `libonnxruntime_providers_shared.so` next to the binary and `models.device = "auto"` then resolves to `cuda` (`vi doctor` prints `device cuda (config: auto, cuda feature on)`). A CPU-only build (`cargo build --release`) still works and is what CI produces. The first GPU run of the `visual` policy on the 47-minute slide lecture took 473 s against 160 s on the CPU and exposed a bug rather than a slow GPU: the adapter handed every operator call a fresh handle with an empty model cell, so the 518 OCR/embedding calls each re-created a CUDA session (about 1.2 s each; the CPU build paid the same reload at a few tens of milliseconds). With the models shared behind `Arc<OnceLock>` (one `loaded ONNX model` line per model) the same run took 220 s while the 30-video dataset run was saturating the CPUs (load average 9 on 12 vCPUs), so a clean GPU-versus-CPU number waits for an idle machine; see the timing table below once it exists.

  Per-call latency of the models themselves (`cargo build --release -p vi-perceive --features cuda --example onnx_bench`, then `onnx_bench /data/videoindex/models <cpu|cuda> frame.png`; 640×360 slide frame, 20 iterations, measured while the dataset run had the CPUs at load 9):

  | Call | CPU (6 threads) | CUDA | Speed-up |
  |---|---|---|---|
  | SigLIP image embed, batch 1 | 270 ms | 11.7 ms | 23× |
  | SigLIP image embed, batch 8 | 1623 ms | 46 ms | 35× |
  | SigLIP text embed (one query) | 83 ms | 6.9 ms | 12× |
  | bge-small, 32 spans | 165 ms | 8.3 ms | 20× |
  | RapidOCR detect + recognise (4 lines) | 262 ms | 26 ms | 10× |
  | Model load (each) | 0.07 to 0.47 s | 0.10 to 0.52 s | |

  That made the models look free and the pass decode-bound, which the idle-machine measurements below contradicted: decode alone (`m0`: sample, pHash, thumbnails) takes 39.5 s for this 47-minute video (71× real time), yet the `visual` pass took 112 s on the CPU build and 177 s on the first CUDA build. OCR alone was 96 s (CPU) against 176 s (CUDA); image embedding alone 53 s against 38 s. The CUDA OCR loss was cuDNN's default *exhaustive* convolution-algorithm search, which re-benchmarks every convolution for each new input shape, and the recogniser's batch tensor had a new width on almost every call. With the heuristic search (`cudnn_conv_algo_search`) and recogniser widths rounded up to multiples of 32, the same video runs:

  | Policy (idle machine, this 47-min 720p VP9 lecture) | CPU build | CUDA build (first) | CUDA build (fixed) |
  |---|---|---|---|
  | `m0`: decode + pHash + thumbnails | 39.7 s | 39.5 s | |
  | image embedding only (sample, phash, image_embed) | 52.9 s | 38.4 s | |
  | OCR only (sample, phash, ocr) | 95.9 s | 175.9 s | 48.2 s |
  | `visual` (all of the above + shots + text_embed) | 112.3 s | 177.4 s | **50.2 s** |

  So with the GPU the visual pass runs at 56× real time and sits about 10 s above the decode floor; on the CPU it is OCR-bound at 25×. The 30-video dataset run (`coarse_only`, CPU build, loaded machine) averaged 7× real time with OCR-heavy slide decks at 5×. Re-run on the fixed CUDA build (2026-09-12, `dataset-gpu.vidx`, with a fine pass and builds sharing the machine for part of it): 6,109 s for 36.6 h, **22× real time**, zero failures; camera-heavy lectures 45×, the slowest slide decks about 10×. What remains is decode plus ASR plus OCR on the densest decks.
- **Hardware decode is not available here.** ffmpeg 8 has the `cuda` hwaccel and the `*_cuvid` decoders compiled in, but the GCP driver install ships no `libnvcuvid.so`, so `-hwaccel cuda` fails at setup (`Failed setup for format cuda`) and `vp9_cuvid` exits immediately. It would not help much anyway: the 30 dataset videos are 22 AV1 and 8 VP9 streams at 720p, and the A100's NVDEC (GA100) decodes H.264, HEVC and VP9 but not AV1. Software decode of the 720p VP9 lecture runs at 31× real time in plain ffmpeg (9.5 s for 5 minutes, 2.7 cores); `vi-media`'s worker with 1 fps sampling, 640 px scaling and shared-memory delivery reached about 18× on an idle machine (160 s for 47 minutes). The decode itself is not the coarse pass's bottleneck (see the table above); OCR was.
- vLLM for the open-VLM A/B runs (M2/M4) is not installed. With 80 GB, a 32B VLM in fp16 fits on the single GPU, so no tensor parallelism or 4-bit quantisation is needed.

## Model files

Under `/data/videoindex/models/` (934 MB before Whisper):

| Directory | Files | Source |
|---|---|---|
| `silero-vad/` | `silero_vad.onnx` (2.2 MB), MIT licence | `onnx-community/silero-vad` |
| `siglip-base-patch16-224/` | `vision_model.onnx` (372 MB), `text_model.onnx` (441 MB), `tokenizer.json`, configs | `Xenova/siglip-base-patch16-224` (768-dim, 224 px; the so400m-384 export does not exist on the Hub) |
| `bge-small-en-v1.5/` | `model.onnx` (133 MB), `tokenizer.json` | `Xenova/bge-small-en-v1.5` (384-dim) |
| `rapidocr/` | `ch_PP-OCRv4_det_infer.onnx`, `ch_PP-OCRv4_rec_infer.onnx`, `en_PP-OCRv3_det_infer.onnx`, `en_PP-OCRv3_rec_infer.onnx`, `ppocr_keys_v1.txt`, `en_dict.txt` | `SWHL/RapidOCR`, PaddleOCR dictionaries |
| `whisper/` | faster-whisper `large-v3` (CTranslate2, ~3 GB) | downloaded by the ASR server on first start |

## Network

- **YouTube via yt-dlp: blocked**, as on azuremc (datacenter IP). Videos arrive by rsync into `incoming/`; they are already there.
- crates.io, GitHub, the Ubuntu archive, Hugging Face and PyPI are reachable.

## Existing deployment state

- `/etc/videoindex`: does not exist. No `videoindex-*` systemd units, no `videoindex` user.
- Listening ports at kickoff: only `sshd` (22) and `systemd-resolved` (53 on loopback). No nginx, no Docker. The ASR server adds `127.0.0.1:9000` (loopback only).
- DNS: `videoindex.app` and `www.videoindex.app` resolve through Cloudflare (`104.21.65.27`, `172.67.139.223`, and the AAAA records recorded for azuremc); `app.` and `api.videoindex.app` have no records. This machine is not a serving host and no reverse proxy is installed here.
- User `nash` has passwordless sudo (`google-sudoers`).

## M0 re-verification on this machine (2026-09-11, release build)

`cargo build --release && cargo test --workspace` pass unchanged against ffmpeg 8.0.1 and rustc 1.98.1 (azuremc had ffmpeg 6.1). `cargo clippy -D warnings` and `cargo deny check` pass.

`vi index /data/videoindex/indexes/m0-timing.vidx /data/videoindex/videos/fixtures/synthetic-1h-720p.mp4 --policy m0 --compact` on the regenerated 1-hour 720p H.264 30 fps fixture (policy `m0`: sample at 1 fps, pHash, WebP thumbnails at 320 px):

| Metric | azuremc (4 vCPUs) | this machine (12 vCPUs) |
|---|---|---|
| Wall clock, whole job | 167.6 s | **48.6 s** (3.4× faster) |
| Acquire (blake3) + probe | 0.9 s | 0.6 s |
| CPU time | 418 s user + 11 s sys (255%) | 313 s user + 7 s sys (657% of one CPU) |
| Frames delivered | 3,600 | 3,600 |
| Peak RSS, parent | 90 MB | 117 MB |
| Index size | 14.6 MB | 14.5 MB (1.0 MB SQLite, 13.5 MB in 3,600 WebP blobs) |

The fixture regenerated here is 917 MB (azuremc: 920 MB; same script, ffmpeg 8 instead of 6). Total CPU time fell from 429 s to 320 s (newer libavcodec, AVX-512), and the job used 6.6 of the 12 vCPUs, so with two or three jobs in parallel the machine is saturated. At this rate the 36.6 hours in `incoming/` take about 30 minutes for the M0 stages alone, before ASR, embeddings and OCR.

Coarse pass with every M1 operator on the CPU (`coarse_asr` / `visual` policies: sampling, pHash, thumbnails, shots, SigLIP embeddings of pHash-distinct frames, RapidOCR on changed frames, bge embeddings of spans, plus VAD and Whisper on the GPU): 34-minute talk in 91 s, 47-minute slide lecture (no ASR) in 160 s, both bounded by the H.264 decode with the other stages in its shadow. About 3.3 minutes per hour of video; the 36.6 hours in `incoming/` are roughly two hours of indexing on this machine, one at a time.

`vi doctor` on this machine reports the GPU line as `NVIDIA A100-SXM4-80GB, 81920 MiB, 595.91.07` followed by `count 1  driver 595.91.07  cuda (driver) 13.2  cuda toolkit 13.1  cuda libs: libcudart.so libcublas.so libcudnn.so` (before the toolkit install: `cuda toolkit missing  cuda libs: none`). It was extended in this kickoff to print the GPU count, the CUDA version the driver supports, the toolkit version when `nvcc` exists, and which CUDA runtime libraries the dynamic linker can find; it also counts incoming videos recursively (playlists are directories) and finds `rustc`/`cargo` in `~/.cargo/bin` when they are not on `PATH`.
