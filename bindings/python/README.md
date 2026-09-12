# videoindex (Python)

```python
import videoindex as vi

idx = vi.Index.create("./talks.vidx", config=vi.Config.from_file("videoindex.toml"))
job = idx.add("talk.mp4", policy="coarse_local")
for ev in job.progress():
    print(ev["type"], ev.get("stage"), ev.get("fraction"))
report = job.wait()

for h in idx.search("hybrid retrieval", k=5):
    print(h["video_id"], h["t0"], h["t1"], h["evidence"][0]["text"])

for ev in idx.ask("When do they discuss evaluation?"):
    if ev["type"] == "token":
        print(ev["text"], end="")
    elif ev["type"] == "citation":
        print(f" [{ev['t0']:.0f}s]", end="")

grid = idx.view(video_id, t0=1830, t1=1860, fps=1)   # {"png": bytes, "width", "height", "timestamps"}
frame = idx.frame(video_id, t=1834.5)                # numpy HxWx3 uint8
```

## Operators and policies in Python

An indexing stage can be a Python function. It receives items from the
operators before it (frames as NumPy arrays) and returns rows the core
stores and hands on, with ids, provenance and caching handled like any
built-in stage:

```python
@vi.operator(id="brightness", inputs=["hashed"], outputs=["description"])
def brightness(ctx, item):
    return {"kind": "description", "target_kind": "frame",
            "target_id": item["sample_id"],
            "text": f"mean brightness {item['frame'].mean():.0f} at {item['t']:.0f}s"}

idx.register_operator(brightness)
idx.add("talk.mp4", policy={"coarse": ["sample", "phash", "thumbnail", "brightness"]})
```

Input and output kinds are the core's item kinds (`media`, `frame`, `hashed`,
`speech_range`, `transcript_span`, `shot`, `ocr_span`, `scene`, `chapter`,
`description`, ...). Rows may be `transcript_span`, `ocr_span`, `shot`, `scene`,
`chapter` or `description`, with times in seconds. Subclass `vi.Operator` for
stateful operators with a `finish(ctx)` hook. A policy given as a dict runs
just those operators; a string names one from the config.

An agent strategy is a class with `next_step(state)`:

```python
class TwoSearches(vi.Policy):
    def next_step(self, state):
        if len(state["steps"]) < 2:
            return {"tool": "search", "args": {"query": state["question"], "k": 5}}
        return None            # let the LLM write the answer from the observations

answer = idx.ask("...", policy=TwoSearches()).collect()
```

Build locally with `maturin develop --release` inside a virtualenv (needs the ffmpeg
development libraries, `pkg-config` and `clang`, like the Rust crates).
