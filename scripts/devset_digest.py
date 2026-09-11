#!/usr/bin/env python3
"""Print a time-bucketed digest of a video's captions to help write dev-set
questions: `scripts/devset_digest.py <youtube-id> [window_secs] [chars]`.

Finds the SRT next to the media in the incoming directory or in the media
cache (by the `.info.json` id) and prints, per window, the start time and
the first N characters of the collapsed caption text.
"""
import glob, json, os, re, sys

ROOT = "/data/videoindex/videos"

def find_srt(vid):
    for p in glob.glob(f"{ROOT}/incoming/*/*-{vid}.en.srt") + glob.glob(f"{ROOT}/incoming/*/*-{vid}.en-orig.srt"):
        return p
    for info in glob.glob(f"{ROOT}/*.info.json"):
        try:
            if json.load(open(info)).get("id") == vid:
                stem = info[:-len(".info.json")]
                for cand in (stem + ".en.srt", stem + ".en-orig.srt"):
                    if os.path.exists(cand):
                        return cand
        except Exception:
            pass
    return None

def parse_srt(path):
    text = open(path, encoding="utf-8", errors="replace").read().replace("\r\n", "\n")
    cues = []
    for block in text.split("\n\n"):
        lines = [l.strip() for l in block.split("\n") if l.strip()]
        timing = next((l for l in lines if "-->" in l), None)
        if not timing:
            continue
        t0 = timing.split("-->")[0].strip().replace(",", ".")
        h, m, s = t0.split(":")
        secs = int(h) * 3600 + int(m) * 60 + float(s)
        body = [l for l in lines if "-->" not in l and not l.isdigit()]
        if body:
            cues.append((secs, body[-1]))  # last line = new material in rolling captions
    # collapse consecutive duplicates
    out = []
    for t, txt in cues:
        if out and out[-1][1] == txt:
            continue
        out.append((t, txt))
    return out

def main():
    vid = sys.argv[1]
    window = float(sys.argv[2]) if len(sys.argv) > 2 else 120.0
    chars = int(sys.argv[3]) if len(sys.argv) > 3 else 170
    path = find_srt(vid)
    if not path:
        print(f"no srt for {vid}"); return
    cues = parse_srt(path)
    print(f"# {vid}  ({path})  {len(cues)} cues")
    buckets = {}
    for t, txt in cues:
        buckets.setdefault(int(t // window), []).append(txt)
    for b in sorted(buckets):
        t = b * window
        joined = " ".join(buckets[b])
        joined = re.sub(r"\s+", " ", joined)
        print(f"{int(t//3600):01d}:{int(t%3600//60):02d}:{int(t%60):02d} {joined[:chars]}")

if __name__ == "__main__":
    main()
