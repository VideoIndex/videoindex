#!/usr/bin/env bash
# Generate the synthetic 1-hour 720p H.264 fixture used for the M0 timing test
# (see docs/MACHINE.md). Takes 5 to 20 minutes depending on CPU.
#
# Usage: scripts/make_fixture_1h.sh [OUT_FILE]
set -euo pipefail
OUT="${1:-/data/videoindex/videos/fixtures/synthetic-1h-720p.mp4}"
mkdir -p "$(dirname "$OUT")"
FONT=""
for f in /usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf /System/Library/Fonts/Supplemental/Arial.ttf; do
  [ -f "$f" ] && FONT="fontfile=$f:" && break
done
ffmpeg -hide_banner -loglevel error -y \
  -f lavfi -i "testsrc2=size=1280x720:rate=30" \
  -f lavfi -i "sine=frequency=440:sample_rate=48000" \
  -t 3600 \
  -vf "drawtext=${FONT}text='%{pts\:hms}':fontsize=64:fontcolor=white:box=1:boxcolor=black@0.6:x=(w-tw)/2:y=h-th-40" \
  -c:v libx264 -preset veryfast -crf 26 -g 150 -pix_fmt yuv420p \
  -c:a aac -b:a 96k -movflags +faststart \
  "$OUT"
echo "wrote $OUT"
