# 09. SDK and APIs

Four surfaces, one core. The Python and Node APIs are the same shape. The CLI and server expose the same operations for scripts and for other languages.

## Python

```python
import videoindex as vi

idx = vi.Index.create("./talks.vidx")            # or vi.Index.open(path)
idx.configure(vi.Config.from_file("videoindex.toml"))

job = idx.add("https://www.youtube.com/watch?v=OkEGJ5G3foU&list=PLcfp...",
              policy="lecture_default",
              budget=vi.Budget(max_cost_usd=5.0))
for ev in job.progress():                        # blocking iterator; job.aprogress() is async
    print(ev.stage, f"{ev.fraction:.0%}", ev.cost_usd)
job.wait()

hits = idx.search("hybrid retrieval evaluation", k=10)
for h in hits:
    print(h.video_id, h.t0, h.t1, h.score, h.evidence[0].text)

for ev in idx.ask("When does the speaker first mention evaluating retrievers?",
                  budget=vi.Budget(max_tokens=50_000)):
    if ev.type == "token":     print(ev.text, end="")
    elif ev.type == "citation": print(f" [{ev.t0:.0f}s]", end="")

answer = idx.ask("...").collect()                # non-streaming convenience
answer.text, answer.citations, answer.usage, answer.partial

# async
async for ev in idx.aask("..."):
    ...

# frames without copying: numpy array over the core buffer
frame = idx.frame(video_id, t=1834.5)             # HxWx3 uint8
grid  = idx.view(video_id, t0=1830, t1=1860, fps=1).image   # PIL-compatible

# extension points
@vi.operator(id="my_captioner", version=1, inputs=["scene"], outputs=["description"])
def my_captioner(ctx, scene):
    ...                                        # returns row dicts; the core stores them
idx.register_operator(my_captioner)
idx.add(url, policy={"coarse": [...], "fine": ["scenes", "my_captioner"]})   # inline policy

class MyPolicy(vi.Policy):
    def next_step(self, state): ...            # {"tool": name, "args": {...}} or None
idx.ask("...", policy=MyPolicy())

vi.prompts.override("vlm_describe", open("my_prompt.md").read())
```

Packaging: `pip install videoindex`. abi3 wheels for manylinux x86_64 and aarch64, macOS arm64 and x86_64, Windows x86_64. ffmpeg libraries are statically linked into the wheel. ONNX Runtime is bundled for CPU; a `videoindex[cuda]` extra pulls the CUDA execution provider. yt-dlp is an optional runtime dependency detected on PATH.

## Node.js

```ts
import { Index, Budget } from "@videoindex/core";

const idx = await Index.create("./talks.vidx");
const job = await idx.add(url, { policy: "lecture_default", budget: { maxCostUsd: 5 } });
for await (const ev of job.progress()) console.log(ev.stage, ev.fraction);

const hits = await idx.search("hybrid retrieval evaluation", { k: 10 });

for await (const ev of idx.ask("When does the speaker first mention evaluating retrievers?")) {
  if (ev.type === "token") process.stdout.write(ev.text);
}

const grid = await idx.view(videoId, { t0: 1830, t1: 1860, fps: 1 }); // { image: Uint8Array, width, height, timestamps }
```

Packaging: `npm install @videoindex/core` with optional platform packages holding the prebuilt `.node` binary, the napi-rs standard layout. TypeScript types shipped.

## CLI

```
vi init  <index-dir>                            create an index
vi acquire <source>... [--out DIR]              download/copy sources into the media cache (yt-dlp for video sites)
vi index <index-dir> <source>... [--policy P] [--budget ...]   acquire + index, streams progress
vi search <index-dir> "<query>" [--k N] [--json]
vi ask   <index-dir> "<question>" [--budget ...] [--json]      streams answer with [HH:MM:SS] citations
vi view  <index-dir> <video-id> --t0 .. --t1 .. --fps .. -o grid.png
vi timeline <index-dir> <video-id>
vi status <index-dir>                            videos, states, sizes, costs
vi serve [--index-root DIR] [--bind 0.0.0.0:8080] [--mcp]
vi eval  <config.toml>                          runs the eval harness (shells to Python in eval/)
vi doctor                                       checks ffmpeg, hardware decode, yt-dlp, ONNX providers, GPU
```

All commands take `--config videoindex.toml` and `--json` for machine-readable output. Exit codes are stable.

## Server (HTTP)

Base path `/v1`. JSON in, JSON or SSE out. OpenAPI document served at `/v1/openapi.json`.

| Method | Path | Purpose |
|---|---|---|
| POST | `/indexes` | create an index (hosted: per tenant) |
| GET | `/indexes/{id}` | status, videos, sizes |
| POST | `/indexes/{id}/videos` | add sources; returns job id |
| GET | `/jobs/{id}` | job status; `Accept: text/event-stream` for progress |
| DELETE | `/jobs/{id}` | cancel |
| POST | `/indexes/{id}/search` | hybrid search |
| POST | `/indexes/{id}/ask` | agentic answer; SSE stream of the events in 06 |
| POST | `/indexes/{id}/view` | frame grid as PNG or WebP |
| GET | `/indexes/{id}/videos/{vid}/timeline` | segments |
| GET | `/indexes/{id}/videos/{vid}/transcript?t0&t1` | spans |
| GET | `/blobs/{key}` | thumbnails, grids |
| GET | `/mcp` | MCP endpoint (streamable HTTP) |
| GET | `/healthz`, `/metrics` | health, Prometheus |

Auth: bearer API keys. Hosted mode adds tenants, quotas per key, and usage records. Self-hosted mode can run with auth disabled on localhost.

## Configuration file

One `videoindex.toml` shared by all surfaces: providers and roles as in [07-model-providers](07-model-providers.md), policies as in [05-indexing-pipeline](05-indexing-pipeline.md), storage backend, media cache directory, decode worker limits, server bind and auth. Environment variables override with a `VI_` prefix.

## Stability policy

- Semantic versioning. Pre-1.0: minor versions may break the API with a changelog entry and a migration note.
- The index schema version is independent of the package version; readers refuse newer schemas and migrate older ones.
- Event types in `ask` streams are additive; consumers must ignore unknown types.
- The MCP tool set and the HTTP API are versioned under `/v1`.
