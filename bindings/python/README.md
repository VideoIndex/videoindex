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

Build locally with `maturin develop --release` inside a virtualenv (needs the ffmpeg
development libraries, `pkg-config` and `clang`, like the Rust crates).
