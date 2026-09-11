#!/usr/bin/env python3
"""Score `vi search` on the retrieval dev set (dataset/devset.jsonl).

    scripts/devset_eval.py <index.vidx> [--config FILE] [--k 5] [--tol 30]
                           [--devset dataset/devset.jsonl] [--vi target/release/vi]
                           [--text-only] [--json OUT]

Each question names a video and either an `anchor` phrase (resolved to a
time from the video's captions, or from the index's OCR spans for `ocr`
questions) or an explicit `t0`/`t1`. A hit at rank r counts when its video
matches and [hit.t0 - tol, hit.t1 + tol] overlaps the ground-truth range.
Reports video@1, hit@1, hit@k and MRR overall and per type, then the misses.
"""
import argparse, glob, json, os, sqlite3, subprocess, sys, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from devset_digest import find_srt, parse_srt  # noqa: E402


def secs(ts):
    return ts["num"] / ts["den"]


def video_map(vi, config, index):
    cmd = [vi] + (["--config", config] if config else []) + ["status", index, "--json"]
    out = subprocess.run(cmd, capture_output=True, text=True, check=True).stdout
    data = json.loads(out)
    m = {}
    for v in data["videos"]:
        uri = v["video"]["source_uri"]
        yt = None
        if "v=" in uri:
            yt = uri.split("v=")[1].split("&")[0]
        m[v["video"]["id"]] = yt
    return m


def resolve_anchor_caption(vid, anchor):
    path = find_srt(vid)
    if not path:
        return None
    needle = anchor.lower()
    for t, txt in parse_srt(path):
        if needle in txt.lower():
            return (max(0.0, t - 5.0), t + 30.0)
    return None


def resolve_anchor_ocr(index, ulid, anchor):
    c = sqlite3.connect(os.path.join(index, "meta.sqlite"))
    row = c.execute(
        "select min(o.t_secs) from ocr_spans o join frame_samples f on f.id = o.frame_sample_id "
        "join tracks tr on tr.id = f.track_id where tr.video_id = ? and lower(o.text) like ?",
        (ulid, f"%{anchor.lower()}%"),
    ).fetchone()
    if row and row[0] is not None:
        return (max(0.0, row[0] - 5.0), row[0] + 60.0)
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("index")
    ap.add_argument("--config")
    ap.add_argument("--k", type=int, default=5)
    ap.add_argument("--tol", type=float, default=30.0)
    ap.add_argument("--devset", default="dataset/devset.jsonl")
    ap.add_argument("--vi", default="target/release/vi")
    ap.add_argument("--text-only", action="store_true")
    ap.add_argument("--json")
    a = ap.parse_args()

    ulid_to_yt = video_map(a.vi, a.config, a.index)
    yt_to_ulid = {v: k for k, v in ulid_to_yt.items() if v}
    rows = [json.loads(l) for l in open(a.devset) if l.strip()]
    results = []
    for q in rows:
        yt = q["video"]
        ulid = yt_to_ulid.get(yt)
        if ulid is None:
            results.append({**q, "status": "video-not-indexed"})
            continue
        if q.get("t0") is not None:
            gt = (float(q["t0"]), float(q["t1"]))
        elif q["type"] == "ocr":
            gt = resolve_anchor_ocr(a.index, ulid, q["anchor"])
        else:
            gt = resolve_anchor_caption(yt, q["anchor"])
        if gt is None:
            results.append({**q, "status": "anchor-unresolved"})
            continue
        cmd = [a.vi] + (["--config", a.config] if a.config else []) + [
            "search", a.index, q["question"], "--json", "-k", str(a.k)]
        if a.text_only:
            cmd.append("--text-only")
        t = time.time()
        proc = subprocess.run(cmd, capture_output=True, text=True)
        ms = int((time.time() - t) * 1000)
        if proc.returncode != 0:
            results.append({**q, "status": "search-failed", "error": proc.stderr[-300:]})
            continue
        resp = json.loads(proc.stdout)
        rank = None
        video_rank = None
        for i, h in enumerate(resp["hits"]):
            if h["video_id"] != ulid:
                continue
            if video_rank is None:
                video_rank = i + 1
            h0, h1 = secs(h["t0"]) - a.tol, secs(h["t1"]) + a.tol
            if h0 <= gt[1] and h1 >= gt[0]:
                rank = i + 1
                break
        results.append({**q, "status": "ok", "gt": gt, "rank": rank, "video_rank": video_rank,
                        "top": [(ulid_to_yt.get(h["video_id"]), round(secs(h["t0"])), round(secs(h["t1"]))) for h in resp["hits"][:3]],
                        "lists": resp.get("lists"), "ms": ms})

    scored = [r for r in results if r["status"] == "ok"]
    def summarize(rs):
        n = len(rs)
        if n == 0:
            return {}
        return {
            "n": n,
            "video@1": round(sum(1 for r in rs if r["video_rank"] == 1) / n, 3),
            "hit@1": round(sum(1 for r in rs if r["rank"] == 1) / n, 3),
            f"hit@{a.k}": round(sum(1 for r in rs if r["rank"] is not None) / n, 3),
            "mrr": round(sum(1.0 / r["rank"] for r in rs if r["rank"]) / n, 3),
            "p50_ms": sorted(r["ms"] for r in rs)[n // 2],
        }
    report = {"overall": summarize(scored)}
    for t in sorted({r["type"] for r in scored}):
        report[t] = summarize([r for r in scored if r["type"] == t])
    skipped = [r for r in results if r["status"] != "ok"]
    print(json.dumps(report, indent=1))
    if skipped:
        print(f"\n{len(skipped)} question(s) not scored:")
        for r in skipped:
            print(f"  {r['id']} {r['video']} {r['status']} {r.get('error','')}")
    misses = [r for r in scored if r["rank"] is None]
    print(f"\n{len(misses)} miss(es) at k={a.k}:")
    for r in misses:
        print(f"  {r['id']} [{r['type']}] {r['video']} gt={tuple(round(x) for x in r['gt'])} video@{r['video_rank']} top={r['top']} lists={r['lists']}\n      {r['question']}")
    if a.json:
        with open(a.json, "w") as f:
            json.dump({"report": report, "results": results}, f, indent=1)


if __name__ == "__main__":
    main()
