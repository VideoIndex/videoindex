"""OpenAI-compatible Whisper server over faster-whisper (CTranslate2, CUDA).

Exposes `POST /v1/audio/transcriptions` the way the OpenAI audio API and
vLLM/whisper servers do, so the `openai_compat` adapter in `vi-providers`
speaks to it unchanged. Run with `scripts/asr_server.sh`; it binds
127.0.0.1:9000 by default and never opens a public port.

Request fields honoured: `file` (any container PyAV reads; 16 kHz mono
16-bit WAV is parsed directly without PyAV), `model` (ignored; the loaded
model is what you get), `language`, `prompt`, `temperature`,
`response_format` (`json`, `verbose_json`, `text`),
`timestamp_granularities[]` (`segment`, `word`; verbose_json always
includes segments, and words when asked), `vad_filter` (default true; only
used to pick cut points in clips of 30 s or more).

Environment: WHISPER_MODEL (default large-v3), WHISPER_DEVICE (cuda),
WHISPER_COMPUTE (float16), WHISPER_DOWNLOAD_ROOT
(/data/videoindex/models/whisper), WHISPER_BATCH (16), WHISPER_WORKERS (2).
"""

from __future__ import annotations

import asyncio
import io
import logging
import os
import struct
import time
from concurrent.futures import ThreadPoolExecutor
from typing import Any

import numpy as np
from fastapi import FastAPI, File, Form, HTTPException, UploadFile
from fastapi.responses import JSONResponse, PlainTextResponse
from faster_whisper import BatchedInferencePipeline, WhisperModel, decode_audio

log = logging.getLogger("asr")
logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")

MODEL_NAME = os.environ.get("WHISPER_MODEL", "large-v3")
DEVICE = os.environ.get("WHISPER_DEVICE", "cuda")
COMPUTE = os.environ.get("WHISPER_COMPUTE", "float16")
DOWNLOAD_ROOT = os.environ.get("WHISPER_DOWNLOAD_ROOT", "/data/videoindex/models/whisper")
BATCH = int(os.environ.get("WHISPER_BATCH", "16"))
WORKERS = int(os.environ.get("WHISPER_WORKERS", "2"))

app = FastAPI(title="videoindex-asr")
_pool = ThreadPoolExecutor(max_workers=WORKERS)
_model: WhisperModel | None = None
_batched: BatchedInferencePipeline | None = None
_loaded_at = 0.0


def _load() -> None:
    global _model, _batched, _loaded_at
    t = time.time()
    os.makedirs(DOWNLOAD_ROOT, exist_ok=True)
    _model = WhisperModel(MODEL_NAME, device=DEVICE, compute_type=COMPUTE, download_root=DOWNLOAD_ROOT)
    _batched = BatchedInferencePipeline(model=_model)
    _loaded_at = time.time() - t
    log.info("loaded %s on %s/%s in %.1fs", MODEL_NAME, DEVICE, COMPUTE, _loaded_at)


@app.on_event("startup")
async def _startup() -> None:
    await asyncio.get_running_loop().run_in_executor(_pool, _load)


def _pcm_from_wav(data: bytes) -> np.ndarray | None:
    """Parse a 16 kHz mono 16-bit PCM WAV without PyAV. None if not that."""
    if len(data) < 44 or data[:4] != b"RIFF" or data[8:12] != b"WAVE":
        return None
    pos = 12
    fmt: tuple[int, int, int, int] | None = None
    while pos + 8 <= len(data):
        cid = data[pos : pos + 4]
        size = struct.unpack("<I", data[pos + 4 : pos + 8])[0]
        body = data[pos + 8 : pos + 8 + size]
        if cid == b"fmt " and size >= 16:
            audio_format, channels, rate, _, _, bits = struct.unpack("<HHIIHH", body[:16])
            fmt = (audio_format, channels, rate, bits)
        elif cid == b"data":
            if fmt is None or fmt != (1, 1, 16000, 16):
                return None
            n = len(body) // 2
            return np.frombuffer(body[: n * 2], dtype="<i2").astype(np.float32) / 32768.0
        pos += 8 + size + (size & 1)
    return None


def _transcribe(audio: np.ndarray, language: str | None, prompt: str | None, temperature: float, words: bool, vad: bool) -> dict[str, Any]:
    assert _batched is not None
    started = time.time()
    # The batched pipeline needs 30 s pieces. Clips under 30 s run as one
    # piece; longer ones are split at silences by faster-whisper's own
    # Silero VAD (the caller's VAD already trimmed non-speech, this only
    # chooses cut points), unless the caller disables it.
    duration = float(len(audio)) / 16000.0
    use_vad = vad and duration >= 30.0
    segments, info = _batched.transcribe(
        audio,
        batch_size=BATCH,
        language=language or None,
        initial_prompt=prompt or None,
        temperature=temperature,
        word_timestamps=words,
        vad_filter=use_vad,
        vad_parameters={"min_silence_duration_ms": 160, "speech_pad_ms": 200} if use_vad else None,
        beam_size=5,
    )
    out_segments = []
    texts = []
    for i, s in enumerate(segments):
        seg: dict[str, Any] = {
            "id": i,
            "seek": s.seek,
            "start": round(s.start, 3),
            "end": round(s.end, 3),
            "text": s.text,
            "tokens": list(s.tokens) if s.tokens is not None else [],
            "temperature": s.temperature,
            "avg_logprob": s.avg_logprob,
            "compression_ratio": s.compression_ratio,
            "no_speech_prob": s.no_speech_prob,
        }
        if words and s.words:
            seg["words"] = [
                {"word": w.word, "start": round(w.start, 3), "end": round(w.end, 3), "probability": round(w.probability, 4)}
                for w in s.words
            ]
        out_segments.append(seg)
        texts.append(s.text.strip())
    all_words = [w for s in out_segments for w in s.get("words", [])]
    return {
        "task": "transcribe",
        "language": info.language,
        "language_probability": info.language_probability,
        "duration": round(duration, 3),
        "text": " ".join(t for t in texts if t),
        "segments": out_segments,
        "words": all_words,
        "model": MODEL_NAME,
        "latency_ms": int((time.time() - started) * 1000),
    }


@app.get("/health")
async def health() -> dict[str, Any]:
    return {"ok": _batched is not None, "model": MODEL_NAME, "device": DEVICE, "compute": COMPUTE, "load_secs": _loaded_at}


@app.get("/v1/models")
async def models() -> dict[str, Any]:
    return {"object": "list", "data": [{"id": MODEL_NAME, "object": "model", "owned_by": "faster-whisper"}]}


@app.post("/v1/audio/transcriptions")
async def transcriptions(
    file: UploadFile = File(...),
    model: str = Form(MODEL_NAME),
    language: str | None = Form(None),
    prompt: str | None = Form(None),
    temperature: float = Form(0.0),
    response_format: str = Form("json"),
    timestamp_granularities: list[str] | None = Form(None, alias="timestamp_granularities[]"),
    vad_filter: bool = Form(True),
):
    if _batched is None:
        raise HTTPException(status_code=503, detail="model still loading")
    data = await file.read()
    if not data:
        raise HTTPException(status_code=400, detail="empty file")
    audio = _pcm_from_wav(data)
    if audio is None:
        try:
            audio = decode_audio(io.BytesIO(data), sampling_rate=16000)
        except Exception as e:  # noqa: BLE001
            raise HTTPException(status_code=400, detail=f"cannot decode audio: {e}") from e
    words = bool(timestamp_granularities and "word" in timestamp_granularities)
    loop = asyncio.get_running_loop()
    try:
        result = await loop.run_in_executor(_pool, _transcribe, audio, language, prompt, float(temperature), words, vad_filter)
    except Exception as e:  # noqa: BLE001
        log.exception("transcription failed")
        raise HTTPException(status_code=500, detail=str(e)) from e
    if response_format == "text":
        return PlainTextResponse(result["text"])
    if response_format == "verbose_json":
        return JSONResponse(result)
    return JSONResponse({"text": result["text"]})
