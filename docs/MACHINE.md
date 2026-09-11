# azuremc machine facts

Recorded 2026-09-11 during the M0 kickoff. `vi doctor` reports the live values; this file is the snapshot the design decisions were made against.

## Hardware

| Item | Value |
|---|---|
| Hostname | `codemind` (ssh alias `azuremc`), Azure VM, public IP 20.64.235.155 |
| OS | Ubuntu 24.04.3 LTS (Noble), kernel 6.14.0-1017-azure, x86_64 |
| CPU | Intel Xeon E5-2673 v4 @ 2.30 GHz, **4 vCPUs** (2 cores × 2 threads, 1 socket), AVX2, SSE4.2, no AVX-512 |
| RAM | 15 GiB total, ~13 GiB available, **no swap** |
| GPU | **None.** `nvidia-smi` not installed, no GPU device on the PCI bus. ffmpeg lists `cuda vaapi qsv vdpau vulkan` hwaccel *methods*, but no device backs them; decode is software only. |

The design docs assume an 8-core server. This machine has half that, so the M0 timing target (1-hour MP4 indexed in under 5 minutes) is measured against 4 vCPUs. See `DECISIONS.md`.

## Disks

| Device | Size | Free | Mount | Notes |
|---|---|---|---|---|
| `/dev/sdb1` | 255 G | 164 G | `/` | OS disk (ext4). **`/data` lives here** as a plain directory. |
| `/dev/sda` | 100 G | 73 G | `/azure_disk` | Existing data disk holding other projects (`arun/talkbox`, `recallhq_events_kb`, nginx and cert configs). Not ours. |
| `/dev/sdc1` | 32 G | 30 G | `/mnt` | Azure ephemeral resource disk. **Data is lost on deallocation**; do not put media or indexes here. |

`/data` did not exist. Created `/data/videoindex/{videos/incoming,indexes,cache,logs}` owned by `azureuser` on the root filesystem (164 G free). That is short of the 500 G the deployment doc plans for LVBench; a dedicated data disk will be needed before M4. `/data/videoindex/videos/incoming/` is **empty**: no videos have been transferred from the Mac yet.

## Toolchain

| Tool | Status at kickoff | Now |
|---|---|---|
| `rustc` / `cargo` / `rustup` | missing | **installed** via rustup, stable 1.98.1 (`~/.cargo/bin`) |
| `cargo-deny` | missing | installed via `cargo install` |
| `python3` | 3.12.3 | unchanged |
| `node` / `npm` | v18.19.1 / 9.2.0 | unchanged. Design requires Node 20+ for the bindings (M3); upgrade then. |
| `ffmpeg` / `ffprobe` | 6.1.1-3ubuntu5 (runtime only) | **dev headers installed**: libavcodec 60.31, libavformat 60.16, libavutil 58.29, libswscale 7.5, libswresample 4.12, libavfilter, libavdevice |
| `pkg-config`, `clang`, `libclang-dev`, `libsqlite3-dev` | missing | **installed** (needed by the ffmpeg bindgen build and rusqlite) |
| `yt-dlp` | missing | **installed** standalone binary 2026.08.19 in `/usr/local/bin` |
| `docker` | 29.1.3 | unchanged, running |
| `caddy` | missing | not installed (see below) |
| `nginx` | 1.24.0 | present and **running**, serving other sites |
| `gcc`, `cmake`, `git` | 13.3.0, 3.28.3, 2.43.0 | unchanged |
| `gh` | missing | not needed for M0 |

Encoders available to ffmpeg for fixtures and thumbnails: `libx264`, `libwebp`, `aac`, `libopus`. Fonts: DejaVu at `/usr/share/fonts/truetype/dejavu/`.

## Network

- **YouTube via yt-dlp: blocked.** `yt-dlp --simulate https://www.youtube.com/watch?v=OkEGJ5G3foU` reaches the site but fails with `Sign in to confirm you're not a bot`, the standard datacenter-IP block. As the design expects, videos arrive by rsync from the development Mac into `/data/videoindex/videos/incoming/` with `.info.json` and `.srt` sidecars.
- crates.io, GitHub, and the Ubuntu archive are reachable. `curl https://crates.io/` returns 403 (Cloudflare bot check on the HTML site) but `cargo fetch` works normally.
- yt-dlp warns that no JavaScript runtime (deno) is present; irrelevant while YouTube is blocked here.

## Existing deployment state

- `/etc/videoindex`: **does not exist.**
- systemd: **no `videoindex-*` units.** Present and active: `nginx.service`, `docker.service`, two docker containers (`codewalk-portal-nodejs` on 9090, `codewalk-db-nodejs` Postgres on 5432).
- No `videoindex` system user.
- **Port conflicts with `docs/11-deployment.md`:** an unrelated Node app (`/azure_disk/arun/talkbox/server_nodejs/app.js`) already listens on `*:8080`, which the design assigns to `vi serve`. Ports 80 and 443 are held by nginx, which proxies `codewalk.app` and `talkboxdhs.com`. Redis is on 127.0.0.1:6379, Postgres on 0.0.0.0:5432. `vi serve` will need a different port (candidate: 127.0.0.1:8090) and must be added as an nginx site rather than replacing the proxy with Caddy. This is an M3 concern; nothing was changed.
- DNS: `videoindex.app` and `www.videoindex.app` resolve (Cloudflare, AAAA `2606:4700:3033::ac43:8bdf` and `2606:4700:3036::6815:411b`). `app.videoindex.app` and `api.videoindex.app` have **no records** yet.

## Other

- User `azureuser` has passwordless sudo and is in the `docker` group.
- No 1-hour MP4 existed on the machine. A synthetic 1-hour 720p H.264 fixture (`testsrc2` + burned-in timestamp + 440 Hz tone, keyframe every 5 s) is generated at `/data/videoindex/videos/fixtures/synthetic-1h-720p.mp4` for the M0 timing test. Real dataset videos replace it once transferred.
