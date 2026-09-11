# Kickoff prompt for development on azuremc

Copy the text below into a fresh Claude Code session in this repository on the azuremc machine.

---

You are starting development of VideoIndex on this machine (azuremc), which hosts videoindex.app. Nothing has been built yet; the repository contains only the design in `docs/`.

## Step 1. Read the design

Read `docs/README.md`, then `docs/01-overview.md` through `docs/11-deployment.md` in order. Treat these as the specification. Where the docs are silent, choose the simplest thing consistent with them and note the choice in `docs/DECISIONS.md` (create it, one dated bullet per decision).

## Step 2. Inspect the machine and record what you find

Determine and write to `docs/MACHINE.md`:
- OS and version, CPU model and core count, RAM, disk layout and free space, and whether `/data` exists.
- GPU: run `nvidia-smi` if present; record model and VRAM. If absent, say so.
- Toolchain: `rustc`, `cargo`, `python3`, `node`, `npm`, `ffmpeg`, `ffprobe`, `yt-dlp`, `docker`, `caddy` or `nginx` versions, or "missing".
- Network: whether `yt-dlp --simulate` can reach YouTube for one URL from `dataset/videolist.md`. Expect this to be blocked from a datacenter IP. Videos arrive instead by rsync from the development Mac into `/data/videoindex/videos/incoming/` with `.info.json` and subtitle sidecars, per `docs/11-deployment.md`. Check whether that directory already has content.
- Which of `/etc/videoindex`, systemd, and DNS for videoindex.app already exist.

Install missing toolchain pieces you need for M0 (Rust stable via rustup, ffmpeg dev libraries, yt-dlp). Ask before installing anything that changes system services or opens ports.

## Step 3. Build milestone M0 from `docs/10-roadmap.md`

Create the Cargo workspace exactly as laid out in `docs/02-architecture.md`. Implement in this order, committing after each works:

1. `vi-core`: Timestamp (rational), ULID ids, Config from TOML with `VI_` env overrides, error type (thiserror), event bus.
2. `vi-media`: probe via libav (use the `rsmpeg` or `ffmpeg-next` crate; pick one, record why); audio decode to 16 kHz mono PCM chunks; video decode at N fps with keyframe seeking; `Arc<FrameBuffer>`; the decode worker as a separate process with a length-prefixed protocol over a pipe and frames over shared memory. Sandboxing may be a stub on the first pass but the process boundary must exist.
3. `vi-index`: the `Storage` trait from `docs/04-data-model.md`; embedded backend with `manifest.json`, SQLite schema v1 (all tables, FTS5 virtual tables), blob store; Lance vectors can be a stub that stores nothing yet.
4. `vi-pipeline`: `Operator` trait, DAG from inputs/outputs, tokio scheduler with bounded channels, checkpoints, progress events. Operators: Sample, PHash, Thumbnail.
5. `vi-cli`: `vi init`, `vi probe`, `vi index` (Sample + PHash + Thumbnail), `vi status`, `vi doctor`.
6. Tests: generate a 2-minute synthetic fixture with ffmpeg (hard cuts every 10 s, burned-in timestamp text, a tone track) in a build step; unit tests for shot-relevant hashing, seeking accuracy, and storage round-trips.
7. CI: GitHub Actions on ubuntu-latest and macos-latest running fmt, clippy with `-D warnings`, tests, `cargo-deny`.

## Definition of done for M0

- `vi index ./out.vidx <a 1-hour mp4>` finishes in under 5 minutes on this machine's CPU and produces thumbnails and pHashes.
- `vi status ./out.vidx` lists the video, duration, sample count, and directory size.
- `vi doctor` reports the machine facts from Step 2.
- `cargo test` passes without network access.
- Every `unsafe` block has a comment explaining why it is sound.

## Conventions

- Crate names `vi-*`; module and type names per the glossary in `docs/README.md`.
- Errors: `thiserror` in libraries, `anyhow` only in `vi-cli`.
- No `unwrap` outside tests.
- Async with tokio; CPU-bound work on rayon; never block the runtime.
- Conventional commit messages; one logical change per commit.
- Do not add Python or Node bindings in M0. Do not add provider adapters in M0.
- Keep `docs/` current: if implementation forces a change to the design, edit the relevant doc in the same commit and add a line to `docs/DECISIONS.md`.

## After M0

Report what was built, the timings measured, and what in the design turned out wrong or unclear. Then continue to M1 in `docs/10-roadmap.md`, starting with the `LocalFile` acquirer's sidecar import (yt-dlp `.info.json`, `.srt`/`.vtt`) so the transferred dataset playlists can be indexed on this machine with their metadata and captions. The `YtDlp` acquirer is still built, for machines where YouTube is reachable.
