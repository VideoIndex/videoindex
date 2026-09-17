#!/usr/bin/env bash
# Download the VideoIndex dataset videos on the development Mac, then transfer to azuremc.
#
# YouTube blocks most downloads from datacenter IPs, so this runs on a residential
# machine and the files are rsync'd to the server afterwards.
#
# Usage:
#   scripts/download_videos.sh                 # playlists from dataset/videolist.md
#   scripts/download_videos.sh urls.txt        # one URL per line (playlists or videos), # comments allowed
#
# Environment:
#   OUT_DIR       where to put files            (default: dataset/videos)
#   MAX_HEIGHT    max video height              (default: 720)
#   AZUREMC_HOST  ssh host for the rsync hint   (default: azuremc)
#   REMOTE_DIR    remote media directory        (default: /data/videoindex/videos/incoming)
#
# Re-running is safe: a download archive skips anything already fetched.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="${OUT_DIR:-$REPO_ROOT/dataset/videos}"
MAX_HEIGHT="${MAX_HEIGHT:-720}"
AZUREMC_HOST="${AZUREMC_HOST:-azuremc}"
REMOTE_DIR="${REMOTE_DIR:-/data/videoindex/videos/incoming}"
URL_FILE="${1:-}"

log() { printf '[download_videos] %s\n' "$*" >&2; }

# --- tools -------------------------------------------------------------------
need_brew() {
  if ! command -v brew >/dev/null 2>&1; then
    log "Homebrew not found; install it from https://brew.sh or install yt-dlp and ffmpeg manually."
    exit 1
  fi
}
if ! command -v yt-dlp >/dev/null 2>&1; then
  need_brew; log "installing yt-dlp"; brew install yt-dlp
fi
if ! command -v ffmpeg >/dev/null 2>&1; then
  need_brew; log "installing ffmpeg"; brew install ffmpeg
fi
log "yt-dlp $(yt-dlp --version), ffmpeg $(ffmpeg -version | head -1 | awk '{print $3}')"

# --- collect URLs ------------------------------------------------------------
URLS_TMP="$(mktemp)"
trap 'rm -f "$URLS_TMP"' EXIT

if [ -n "$URL_FILE" ]; then
  [ -f "$URL_FILE" ] || { log "no such file: $URL_FILE"; exit 1; }
  grep -Eo 'https?://[^[:space:])]+' "$URL_FILE" > "$URLS_TMP" || true
else
  # From dataset/videolist.md take playlist and single-video URLs only.
  # Channel URLs (youtube.com/@name) are skipped: they would fetch an entire channel.
  grep -Eo 'https?://[^[:space:])]+' "$REPO_ROOT/dataset/videolist.md" \
    | grep -E 'list=|watch\?v=|youtu\.be/' > "$URLS_TMP" || true
fi

URL_COUNT="$(grep -c . "$URLS_TMP" || true)"
if [ "${URL_COUNT:-0}" -eq 0 ]; then
  log "no URLs found"; exit 1
fi
log "$URL_COUNT URL(s) to process into $OUT_DIR"
mkdir -p "$OUT_DIR"

# --- download ----------------------------------------------------------------
# Layout: OUT_DIR/<playlist_id or 'single'>/<index>-<video_id>.mp4 plus sidecars:
#   .info.json (title, chapters, description), .en.srt subtitles when available.
while IFS= read -r url; do
  [ -z "$url" ] && continue
  log "fetching $url"
  yt-dlp \
    --yes-playlist \
    --format "bv*[height<=${MAX_HEIGHT}][ext=mp4]+ba[ext=m4a]/bv*[height<=${MAX_HEIGHT}]+ba/b[height<=${MAX_HEIGHT}]/b" \
    --merge-output-format mp4 \
    --write-info-json \
    --write-subs --write-auto-subs --sub-langs "en.*,en" --convert-subs srt \
    --embed-chapters \
    --download-archive "$OUT_DIR/archive.txt" \
    --no-overwrites --continue \
    --retries 10 --fragment-retries 10 --concurrent-fragments 4 \
    --sleep-requests 1 --sleep-interval 2 --max-sleep-interval 6 \
    --ignore-errors \
    --output "$OUT_DIR/%(playlist_id|single)s/%(playlist_index|0)03d-%(id)s.%(ext)s" \
    "$url" || log "yt-dlp reported errors for $url (continuing)"
done < "$URLS_TMP"

# --- summary -----------------------------------------------------------------
MP4_COUNT="$(find "$OUT_DIR" -name '*.mp4' | wc -l | tr -d ' ')"
TOTAL_SIZE="$(du -sh "$OUT_DIR" | awk '{print $1}')"
log "done: $MP4_COUNT mp4 file(s), $TOTAL_SIZE in $OUT_DIR"

cat >&2 <<MSG

Transfer to the server (resumable, run again if interrupted):

  rsync -avP --partial "$OUT_DIR/" "$AZUREMC_HOST:$REMOTE_DIR/"

Then on $AZUREMC_HOST:

  vidx index ./dataset.vidx $REMOTE_DIR/**/*.mp4
MSG
