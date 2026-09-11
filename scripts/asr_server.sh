#!/usr/bin/env bash
# Start the local Whisper server (faster-whisper on the GPU) on 127.0.0.1:9000.
#
# One-time setup (see docs/MACHINE.md):
#   uv venv --python 3.12 /data/videoindex/asr/.venv
#   uv pip install --python /data/videoindex/asr/.venv/bin/python \
#     faster-whisper fastapi "uvicorn[standard]" python-multipart \
#     nvidia-cublas-cu12 nvidia-cudnn-cu12
#
# Usage: scripts/asr_server.sh [start|stop|status|run]
set -euo pipefail
VENV="${ASR_VENV:-/data/videoindex/asr/.venv}"
HOST="${ASR_HOST:-127.0.0.1}"
PORT="${ASR_PORT:-9000}"
LOG="${ASR_LOG:-/data/videoindex/logs/asr-server.log}"
PID="${ASR_PID:-/data/videoindex/asr/server.pid}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export WHISPER_MODEL="${WHISPER_MODEL:-large-v3}"
export WHISPER_DOWNLOAD_ROOT="${WHISPER_DOWNLOAD_ROOT:-/data/videoindex/models/whisper}"

run() {
  # CTranslate2 dlopens cuBLAS and cuDNN; the pip wheels put them under
  # site-packages/nvidia/*/lib, which is not on the loader path.
  NV=$(ls -d "$VENV"/lib/python3.*/site-packages/nvidia/*/lib 2>/dev/null | tr '\n' ':')
  export LD_LIBRARY_PATH="${NV}${LD_LIBRARY_PATH:-}"
  cd "$HERE/asr-server"
  exec "$VENV/bin/python" -m uvicorn server:app --host "$HOST" --port "$PORT" --log-level info
}

case "${1:-start}" in
  run) run ;;
  start)
    if [ -f "$PID" ] && kill -0 "$(cat "$PID")" 2>/dev/null; then
      echo "already running (pid $(cat "$PID"))"; exit 0
    fi
    mkdir -p "$(dirname "$LOG")" "$(dirname "$PID")"
    nohup "$0" run >>"$LOG" 2>&1 &
    echo $! >"$PID"
    echo "started pid $! on $HOST:$PORT, log $LOG"
    ;;
  stop)
    if [ -f "$PID" ]; then kill "$(cat "$PID")" 2>/dev/null && echo stopped; rm -f "$PID"; fi
    ;;
  status)
    curl -s "http://$HOST:$PORT/health" || { echo "not responding"; exit 1; }
    echo
    ;;
  *) echo "usage: $0 [start|stop|status|run]"; exit 2 ;;
esac
