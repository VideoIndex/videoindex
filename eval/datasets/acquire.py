#!/usr/bin/env python3
"""Get a benchmark's videos and index them.

    python3 -m eval.datasets.acquire lvbench --root /data/videoindex/eval/lvbench [--download]
    python3 -m eval.datasets.acquire lvbench --root ... --index /data/videoindex/indexes/eval-lvbench.vidx --config config/gcp-a100.toml [--policy coarse_only]

`--download` runs yt-dlp (720p, English subtitles, .info.json, archive file) for
every video key the loader knows; without it the step only reports what is
present. `--index` runs `vi index` over the present videos (the LocalFile
acquirer imports the sidecars) and writes `<root>/video_map.json`: YouTube key
-> VideoIndex video id, which the runners need.
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

from . import load

YTDLP = [
    "yt-dlp", "-f", "bv*[height<=720]+ba/b[height<=720]", "--merge-output-format", "mp4",
    "--write-info-json", "--write-subs", "--write-auto-subs", "--sub-langs", "en,en-US,en-GB,en-orig",
    "--sub-format", "srt", "--convert-subs", "srt", "--ignore-errors", "--no-overwrites", "--sleep-requests", "1",
]


def present(root: Path) -> dict[str, Path]:
    return {p.stem: p for p in (root / "videos").glob("*.mp4")}


def download(root: Path, keys: list[str]) -> None:
    (root / "videos").mkdir(parents=True, exist_ok=True)
    urls = root / "video_ids.txt"
    urls.write_text("\n".join(f"https://www.youtube.com/watch?v={k}" for k in keys) + "\n")
    env = dict(os.environ, PATH=f"{Path.home() / '.deno' / 'bin'}:{os.environ.get('PATH', '')}")
    subprocess.run(
        YTDLP + ["--download-archive", str(root / "archive.txt"), "-o", str(root / "videos" / "%(id)s.%(ext)s"), "-a", str(urls)],
        env=env, check=False,
    )


def index(root: Path, index_dir: Path, config: str | None, policy: str, vi: str) -> dict[str, str]:
    files = sorted(present(root).values())
    if not files:
        raise SystemExit("no videos present")
    cmd = [vi] + (["--config", config] if config else []) + ["index", str(index_dir)] + [str(f) for f in files] + ["--policy", policy]
    print(" ".join(cmd[:6]), f"... ({len(files)} files)", file=sys.stderr)
    subprocess.run(cmd, check=False)
    out = subprocess.run([vi] + (["--config", config] if config else []) + ["status", str(index_dir), "--json"], capture_output=True, text=True, check=True).stdout
    status = json.loads(out)
    by_hash = {}
    for v in status["videos"]:
        by_hash[v["video"]["content_hash"]] = v["video"]["id"]
    # Map YouTube key -> video id through the file's blake3 (the acquirer's content hash).
    import hashlib  # noqa: PLC0415

    vmap = {}
    for key, path in present(root).items():
        h = blake3_hex(path)
        if h in by_hash:
            vmap[key] = by_hash[h]
    (root / "video_map.json").write_text(json.dumps(vmap, indent=1))
    print(f"{len(vmap)} of {len(files)} videos mapped -> {root / 'video_map.json'}", file=sys.stderr)
    return vmap


def blake3_hex(path: Path) -> str:
    try:
        import blake3  # type: ignore
    except ImportError:
        raise SystemExit("pip install blake3 (the index keys media by blake3 content hash)") from None
    h = blake3.blake3()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 22), b""):
            h.update(chunk)
    return h.hexdigest()


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("benchmark")
    ap.add_argument("--root", required=True)
    ap.add_argument("--download", action="store_true")
    ap.add_argument("--index")
    ap.add_argument("--config")
    ap.add_argument("--policy", default="coarse_only")
    ap.add_argument("--vi", default="target/release/vi")
    a = ap.parse_args()
    root = Path(a.root)
    qs = load(a.benchmark, root)
    keys = sorted({q.video_key for q in qs})
    have = present(root)
    print(f"{a.benchmark}: {len(qs)} questions over {len(keys)} videos; {sum(k in have for k in keys)} present", file=sys.stderr)
    if a.download:
        download(root, [k for k in keys if k not in have])
    if a.index:
        index(root, Path(a.index), a.config, a.policy, a.vi)


if __name__ == "__main__":
    main()
