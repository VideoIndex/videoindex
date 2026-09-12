# M1 report: coarse index and search on the GCP A100 machine

Written 2026-09-11 at the end of M1 (with the start of M2 folded in). The machine is described in [MACHINE.md](MACHINE.md); every choice made along the way is a dated bullet in [DECISIONS.md](DECISIONS.md).

## What was built

| Area | Delivered | Where |
|---|---|---|
| Provider layer | Capability traits, role registry, per-provider governor (semaphore, request and token buckets), retries with backoff and `Retry-After`, cost accounting into `Provenance`, key redaction, prompt registry | `vi-providers` |
| Adapters | `openai_compat` (ASR, chat with tools/images/JSON schema, embeddings), `anthropic` (Messages, tools, images), `gemini` (function calling, inline images and video), `onnx_local` (SigLIP, bge-small, RapidOCR) | `vi-providers::adapters`, `vi-perceive::onnx_local` |
| Local ASR | faster-whisper `large-v3` on the A100 behind an OpenAI-compatible server, 4 GB VRAM | `scripts/asr-server/`, `scripts/asr_server.sh` |
| Operators | `vad` (Silero, batched), `asr`, `shot_boundary`, `image_embed` (SigLIP), `ocr` (RapidOCR, change-gated), `text_embed` (bge), `scenes`, `chapters`, `vlm_describe`, `entities_events` | `vi-pipeline::ops`, `vi-perceive` |
| Storage | Flat memory-mapped vector store with exact search, schema v2, sessions, entities and events, embedding lookups | `vi-index` |
| Search | Hybrid `vi search`: BM25 + text-vector + image-vector lists, RRF, grouping by scenes, shots (~60 s pieces), chapters or windows | `vi-query` |
| Scheduler | Budgets, operator output cache with skip/replay/run planning, per-range provider failures, `auto` default policy | `vi-pipeline::scheduler` |
| Acquirers | `Http` (limits, private-address refusal, resume) and `ObjectStore` (S3/GCS/Azure/R2, prefix expansion) | `vi-media::remote` |
| Agent | `vi ask`: tool-using loop with `search`, `timeline`, `get_transcript`, `get_ocr`, `get_descriptions`, `view`, `describe`; budgets; inline citations; sessions; `RetrievalOnlyPolicy` baseline; `vi view`, `vi timeline` | `vi-agent`, `vi-cli` |
| Python | `videoindex` package: `Index.create/open/add/search/ask/aask/view/frame/timeline/status`, `Budget`, `Config`, prompt overrides | `bindings/python` |
| Dev sets | 72 retrieval questions with caption/OCR anchors; 52 QA questions with accepted answers | `dataset/devset.jsonl`, `dataset/devset_qa.jsonl`, `scripts/devset_*_eval.py` |

## Timings on this machine (12 vCPUs, one A100 80 GB)

| Measurement | Result |
|---|---|
| M0 policy (sample, pHash, thumbnails), synthetic 1-hour 720p | 48.6 s wall, 6.6 CPUs busy (azuremc: 167.6 s) |
| Coarse pass with ASR (`coarse_asr`), 34-minute talk, machine otherwise idle | 81 to 91 s wall; decode-bound, ASR and embeddings in its shadow |
| Whisper large-v3 through the pipeline | 32 minutes of speech in 65 s = 29× real time at 4 concurrent requests; a lone 60 s clip 41× |
| Silero VAD, 34-minute track | 14 s including the audio decode, batched 32 parts in lockstep |
| Coarse pass without ASR (`visual`), 47-minute slide lecture | 160 s wall; 247 frames read by OCR (841 lines), 168 SigLIP embeddings |
| Full dataset (36.6 h, 30 videos, `coarse_only`), one process | running while builds compete for the CPU: 4 to 17 min per hour of video; see the per-video table below when complete |
| Fine pass (`full`) on the 34-minute talk with Claude Sonnet 5 | see below |
| Hybrid `vi search`, one-shot CLI | 1.5 to 2.4 s, of which about 1.3 s loads bge and the SigLIP text tower; the search itself under 30 ms |
| `vi ask` on the 34-minute talk | 3 tool calls, correct timestamps, $0.065, 14.6 s |

GPU utilisation: the A100 is used only by Whisper (about 4.5 GB, 60 to 100% while ASR requests run, idle otherwise). ONNX Runtime runs on the CPU because no CUDA toolkit is installed (see below).

Hosted-provider cost: the only paid calls were Anthropic (`claude-sonnet-5`) for the agent and the fine pass; figures are in the sections below and in each run's report line.

## Dev set results

FILLED_IN_WHEN_THE_DATASET_RUN_COMPLETES

## What the design got wrong or left unclear

1. **Slide changes are not shots.** The design lists `ShotBoundary` as the visual unit and `Ocr` as running "on distinct frames whose text-likelihood heuristic fires". On slide lectures a title change moves as many pixels as a sponsor-logo swap; no histogram or edge threshold separates them, and pHash of a white slide ignores its text. OCR now runs on a pixel-change gate and collapses repeated lines; slide changes surface as OCR spans, and chapters use OCR title changes. Shots stay true camera cuts.
2. **Lance.** `04-data-model.md` names Lance as the default vector store. Its current release pulls 689 crates (Arrow, DataFusion) into a workspace of about 250, for exact search over a few hundred thousand vectors that a flat memory-mapped file does in milliseconds. The flat store ships; Lance or usearch remain a drop-in behind the trait.
3. **Text embeddings belong to the coarse pass.** The design put `text_embed` in the fine pass, but hybrid search needs span vectors before any VLM runs. It moved.
4. **Resume needed a replay hook.** "Re-running with a different VLM re-runs only the VLM stage" is only true if cached producers can hand their stored outputs to a re-running consumer. Operators got `replay()`; operators whose outputs are pixels (`sample`) cannot replay, so anything needing frames re-decodes.
5. **One 30 s ASR chunk per request wastes the GPU.** The design's "30 s chunks with 1 s overlap" gives 17× real time; 120 s VAD segments that the batched server cuts itself give 29×.
6. **Scenes need to split long shots.** A static camera holds one shot for half an hour; the design's "scenes 20 s to 3 min" only holds if long shots are cut before grouping.
7. **Tool results cannot carry images on OpenAI-shaped APIs**, so `view` returns text and the grid follows as a user message; Anthropic and Gemini accept this too.
8. **Claude 5 rejects `temperature`.** The eval doc's "temperature 0 where the provider allows it" needs the adapter to omit the field for these models.
9. **Sort comparators with tolerances panic.** Rust's sort checks for total orders; the OCR reading-order sort tripped it on the first long video. Every float sort uses `total_cmp` now.
10. **The kickoff assumed a 40 GB A100.** This machine has 80 GB, so a 32B VLM fits in fp16 for the M4 A/B runs without quantisation.

## Open items carried into M2/M4

- ~~Install a CUDA toolkit~~ Done 2026-09-11 after asking: CUDA 13.1 + cuDNN 9, `vi` built with `--features cuda`. The models run 10 to 35× faster per call on the GPU; the coarse pass is then bound by CPU video decode (no NVDEC: no `libnvcuvid`, and the corpus is AV1/VP9). See `MACHINE.md`.
- ~~Python-side operators and policies~~ Done 2026-09-12 (`@vi.operator`, `vi.Policy`, inline policies). Still open: wheels verified in CI; the zero-copy frame view (frames are copied into NumPy today).
- `Diarize`, `listen`, `find_similar_frames`.
- Format the local SSD if build times or temp files ever matter.
