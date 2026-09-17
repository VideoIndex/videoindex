#!/usr/bin/env python3
"""Per-video timing table from a `vidx index` log.

    python3 scripts/index_log_timings.py <log>[,<log>...] [index.vidx]

Reads the `indexing <uri>`, `acquired and probed ... video=<id> duration=<hms>`,
`<stage> done: <n> items in <s>s` and `finished: ...` lines and prints a
Markdown table: title (from the index's SQLite when given, else the file
name), duration, wall time, speed as a multiple of real time, and the item
counts of the stages that dominate (asr, ocr, image_embed, shot_boundary).
"""
import re
import sqlite3
import sys
from pathlib import Path

ANSI = re.compile(r"\x1b\[[0-9;]*m")
RE_INDEXING = re.compile(r"^indexing (\S+)")
RE_PROBED = re.compile(r"acquired and probed in [\d.]+s video=(\S+) duration=(\d+):(\d+):([\d.]+)")
RE_DONE = re.compile(r"^\s+(\w+)\s+done: (\d+) items in ([\d.]+)s")
RE_FINISHED = re.compile(r"^finished: (\d+) stages?, (\d+) samples, ([\d.]+)s")
RE_SKIPPED = re.compile(r"^\s+(\w+)\s+(skipped|replayed)")
# Embedding stages store rows without emitting items, so `done: 0 items`;
# their INFO summary lines carry the counts.
RE_IMG = re.compile(r"image embeddings written frames=(\d+) embedded=(\d+)")
RE_TXT = re.compile(r"text embeddings written embedded=(\d+)")
STAGES = ["asr", "ocr", "image_embed", "shot_boundary", "text_embed"]


def parse(path):
    videos = []
    cur = None
    for raw in Path(path).read_text(errors="replace").splitlines():
        line = ANSI.sub("", raw)
        m = RE_INDEXING.match(line)
        if m:
            cur = {"uri": m.group(1), "stages": {}, "cached": set(), "written": {}}
            videos.append(cur)
            continue
        if cur is None:
            continue
        m = RE_PROBED.search(line)
        if m:
            cur["video_id"] = m.group(1)
            cur["duration"] = int(m.group(2)) * 3600 + int(m.group(3)) * 60 + float(m.group(4))
            continue
        m = RE_DONE.match(line)
        if m:
            cur["stages"][m.group(1)] = (int(m.group(2)), float(m.group(3)))
            continue
        m = RE_SKIPPED.match(line)
        if m:
            cur["cached"].add(m.group(1))
            continue
        m = RE_IMG.search(line)
        if m:
            cur["written"]["image_embed"] = int(m.group(2))
            continue
        m = RE_TXT.search(line)
        if m:
            cur["written"]["text_embed"] = int(m.group(1))
            continue
        m = RE_FINISHED.match(line)
        if m:
            cur["samples"] = int(m.group(2))
            cur["elapsed"] = float(m.group(3))
    return videos


def titles(index_dir):
    if not index_dir:
        return {}
    db = Path(index_dir) / "meta.sqlite"
    if not db.is_file():
        return {}
    c = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    return {vid: (title or "") for vid, title in c.execute("SELECT id, title FROM videos")}


def hms(secs):
    secs = int(round(secs))
    return f"{secs // 3600}:{secs % 3600 // 60:02d}:{secs % 60:02d}"


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    videos = []
    for log in sys.argv[1].split(","):
        videos.extend(parse(log))
    # The same video may appear in several logs (a resumed run replays it
    # from the cache in seconds); keep the run that did the work.
    by_id = {}
    for v in videos:
        if "elapsed" not in v:
            continue
        key = v.get("video_id", v["uri"])
        if key not in by_id or v["elapsed"] > by_id[key]["elapsed"]:
            by_id[key] = v
    videos = list(by_id.values())
    names = titles(sys.argv[2] if len(sys.argv) > 2 else None)
    cols = ["Video", "Duration", "Wall", "× real time"] + STAGES
    print("| " + " | ".join(cols) + " |")
    print("|" + "---|" * len(cols))
    tot_dur = tot_wall = 0.0
    for v in videos:
        if "elapsed" not in v:
            continue
        name = names.get(v.get("video_id"), "") or Path(v["uri"]).stem[:12]
        dur = v.get("duration", 0.0)
        tot_dur += dur
        tot_wall += v["elapsed"]
        speed = f"{dur / v['elapsed']:.0f}×" if v["elapsed"] else ""
        cells = [name[:60].replace("|", "/"), hms(dur), f"{v['elapsed']:.0f} s", speed]
        for st in STAGES:
            if st in v["written"]:
                cells.append(str(v["written"][st]))
            elif st in v["stages"]:
                cells.append(str(v["stages"][st][0]))
            elif st in v["cached"]:
                cells.append("cached")
            else:
                cells.append("")
        print("| " + " | ".join(cells) + " |")
    if tot_wall:
        print(f"| **{sum('elapsed' in v for v in videos)} videos** | {hms(tot_dur)} | {tot_wall:.0f} s | {tot_dur / tot_wall:.0f}× | | | | | |")


if __name__ == "__main__":
    main()
