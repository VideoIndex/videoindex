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
| Coarse pass without ASR (`visual`), 47-minute slide lecture, idle machine | CPU build 112 s; CUDA build **50 s** after the cuDNN fix (177 s before it); decode alone 39.5 s. 247 frames read by OCR (824 to 843 lines), 168 SigLIP embeddings. Details in `MACHINE.md`. |
| Full dataset (36.6 h, 30 videos, `coarse_only`), one process, CPU ONNX build, builds and tests competing for the CPU | 17,856 s = 4 h 58 min, 7× real time overall (5× on OCR-heavy slide decks, 23× on camera-heavy talks); 0 failures; index 1.06 GiB (187 MiB SQLite, 567 MiB thumbnails in 130,907 blobs). Per-video table below. |
| Fine pass (`full`) on the 34-minute talk with Claude Sonnet 5 | 17 scenes, 12 descriptions, 74 entities / 82 events, $0.63 |
| Fine pass over the whole dataset (36.6 h, 30 videos), CUDA build, machine shared with other jobs | 18,491 s = 5 h 8 min; **$45.96** ($24.02 VLM descriptions of 1,014 scenes, $21.94 entity/event extraction over 460 five-minute windows, 9 windows returned non-JSON and were skipped); 236 chapters, 5,074 entities with 9,116 mentions, 4,934 events; index 1.06 → 1.98 GiB. $1.26 per hour of video. |
| Hybrid `vi search`, one-shot CLI | 1.5 to 2.4 s, of which about 1.3 s loads bge and the SigLIP text tower; the search itself under 30 ms |
| `vi ask` on the 34-minute talk | 3 tool calls, correct timestamps, $0.065, 14.6 s |

GPU utilisation during the dataset run: the A100 was used only by Whisper (about 4.5 GB, 60 to 100% while ASR requests run). ONNX Runtime ran on the CPU because the run started before the CUDA toolkit was installed; the CUDA build (`--features cuda`) landed the same night and, after fixing cuDNN's exhaustive per-shape algorithm search, cuts the visual pass from 112 s to 50 s on the test lecture. Re-indexing the dataset on the GPU build is the obvious next timing run (expected: bounded by decode and ASR, roughly 2 h for the 36.6 h).

<details>
<summary>Per-video timings, dataset run (CPU ONNX build, loaded machine)</summary>

| Video | Duration | Wall | × real time | asr | ocr | image_embed | shot_boundary | text_embed |
|---|---|---|---|---|---|---|---|---|
| Building Multimodal AI Agents From Scratch — Apoorva Joshi,  | 0:36:58 | 229 s | 10× | 207 | 859 | 133 | 45 | 1227 |
| Evals 101 — Doug Guthrie, Braintrust | 0:48:31 | 301 s | 10× | 258 | 1453 | 183 | 51 | 1920 |
| A2A & MCP Workshop: Automating Business Processes with LLMs  | 1:23:13 | 678 s | 7× | 417 | 1451 | 801 | 61 | 2236 |
| RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI | 1:46:14 | 2130 s | 3× | 652 | 35098 | 368 | 77 | 36160 |
| Agentic AI MOOC / UC Berkeley CS294-196 Fall 2025 / Agentic  | 1:48:50 | 398 s | 16× | 583 | 3610 | 77 | 39 | 5225 |
| Agentic AI MOOC / UC Berkeley CS294-196 Fall 2025 / Multi-Ag | 0:54:49 | 159 s | 21× | 320 | 675 | 42 | 18 | 1484 |
| How to build world-class AI products — Sarah Sachs (AI lead  | 1:43:45 | 946 s | 7× | 556 | 10200 | 553 | 66 | 11214 |
| Agentic AI MOOC / UC Berkeley CS294-196 Fall 2025 / Evolutio | 1:19:31 | 209 s | 23× | 428 | 1021 | 80 | 36 | 2147 |
| Collaborating with Agents in your Software Dev Workflow - Jo | 1:04:06 | 752 s | 5× | 368 | 6221 | 486 | 44 | 6854 |
| Strategies for LLM Evals (GuideLLM, lm-eval-harness, OpenAI  | 0:32:28 | 282 s | 7× | 177 | 1420 | 149 | 37 | 1740 |
| Agentic AI MOOC / UC Berkeley CS294-196 Fall 2025 / Post-Tra | 1:17:28 | 225 s | 21× | 414 | 1117 | 13 | 14 | 2219 |
| [Full Workshop] Reinforcement Learning, Kernels, Reasoning,  | 2:42:27 | 1490 s | 7× | 863 | 22663 | 220 | 65 | 24225 |
| Shipping AI That Works: An Evaluation Framework for PMs – Am | 1:26:16 | 862 s | 6× | 457 | 6275 | 733 | 76 | 7109 |
| Agentic AI MOOC / UC Berkeley CS294-196 Fall 2025 / Practica | 0:46:54 | 172 s | 16× | 252 | 841 | 168 | 41 | 1509 |
| Agentic AI MOOC / UC Berkeley CS294-196 Fall 2025 / Predicta | 0:44:04 | 148 s | 18× | 258 | 1208 | 37 | 17 | 1875 |
| VoiceVision RAG - Integrating Visual Document Intelligence w | 1:23:51 | 740 s | 7× | 503 | 10024 | 250 | 65 | 10898 |
| The AI Engineer’s Guide to Raising VC — Dani Grant (Jam), Ch | 0:34:16 | 191 s | 11× | 197 | 580 | 47 | 6 | 922 |
| Introduction to LLM serving with SGLang - Philip Kiely and Y | 0:43:42 | 524 s | 5× | 254 | 6882 | 114 | 35 | 7330 |
| Agentic AI MOOC / UC Berkeley CS294-196 Fall 2025 / Autonomo | 1:01:42 | 590 s | 6× | 326 | 3443 | 469 | 103 | 4319 |
| Agentic AI MOOC / UC Berkeley CS294-196 Fall 2025 / Training | 1:04:37 | 275 s | 14× | 366 | 742 | 121 | 23 | 1673 |
| Agentic AI MOOC / UC Berkeley CS294-196 Fall 2025 / LLM Agen | 1:58:21 | 400 s | 18× | 623 | 2148 | 154 | 87 | 3839 |
| ComfyUI Full Workshop — first workshop from ComfyAnonymous h | 0:51:25 | 499 s | 6× | 297 | 1281 | 595 | 31 | 1810 |
| [Full Workshop] Building Conversational AI Agents - Thor Sch | 1:01:42 | 391 s | 9× | 347 | 3349 | 245 | 41 | 3971 |
| How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - | 1:41:33 | 978 s | 6× | 567 | 8070 | 415 | 83 | 9065 |
| Agentic AI MOOC / UC Berkeley CS294-196 F25 / Multi-Agent Sy | 0:58:58 | 224 s | 16× | 309 | 883 | 166 | 50 | 1719 |
| Agentic AI MOOC / UC Berkeley CS294-196 Fall 2025 / AI Agent | 1:01:15 | 206 s | 18× | 327 | 1401 | 79 | 26 | 2272 |
| From Mixture of Experts to Mixture of Agents with Super Fast | 0:53:15 | 431 s | 7× | 301 | 4421 | 119 | 38 | 4955 |
| Full Workshop: Realtime Voice AI — Mark Backman, Daily | 1:09:40 | 722 s | 6× | 383 | 5963 | 559 | 47 | 6641 |
| [Full Workshop] Vibe Coding at Scale: Customizing AI Assista | 1:20:38 | 1255 s | 4× | 541 | 23800 | 370 | 66 | 24694 |
| Building Code First AI Agents with Azure AI Agent Service —  | 1:54:05 | 1447 s | 5× | 762 | 11034 | 759 | 106 | 12289 |
| **30 videos** | 36:34:35 | 17856 s | 7× | | | | | |

Columns after the speed: transcript spans written by `asr`, OCR lines, SigLIP frame embeddings (pHash-distinct frames), shots, and bge text embeddings (transcript + OCR spans). Generated by `scripts/index_log_timings.py`.

</details>

Hosted-provider cost: the only paid calls were Anthropic (`claude-sonnet-5`) for the agent and the fine pass; figures are in the sections below and in each run's report line.

## Dev set results

Two dev sets over the 30 dataset videos (`dataset/`): 72 retrieval questions (65 transcript, 3 OCR, 4 visual) scored by `scripts/devset_eval.py` (`vi search`, k=5, a hit is a result within 30 s of the anchored ground truth), and 52 QA questions with accepted answer strings scored by `scripts/devset_qa_eval.py` (`vi ask --json`, budget $0.30 and 6 tool calls per question, `claude-sonnet-5` as `agent_llm`).

### Retrieval (`vi search`, hybrid: BM25 per kind + bge + SigLIP, RRF)

| | n | video@1 | hit@1 | hit@5 | MRR |
|---|---|---|---|---|---|
| First run (FTS ANDs every word) | 64 scored, 8 anchors unresolved | 0.578 | 0.344 | 0.766 | 0.502 |
| **After OR-ing terms with stopword removal** | 72 | **0.819** | **0.583** | **0.944** | **0.724** |
| transcript questions | 65 | 0.831 | 0.569 | 0.954 | 0.721 |
| OCR questions | 3 | 1.0 | 1.0 | 1.0 | 1.0 |
| visual questions (SigLIP text-to-frame) | 4 | 0.5 | 0.5 | 0.75 | 0.562 |

The first run's failure mode was structural, not a ranking problem: the FTS5 query required every word of the question, so BM25 returned nothing for almost every natural-language query and the fusion ran on the two vector lists alone. With OR-ed terms all four lists contribute to every hit (`lists` in the eval output). The four remaining misses are two right-video-wrong-minute cases (the question's wording matches a later restatement), one visual question whose frame ranks fourth, and one paraphrase ("collision rate" is spoken as "collisions") that neither BM25 nor bge bridges. Median `vi search` latency is 2.4 s as a one-shot CLI, of which about 2 s is process start and model load (SigLIP text tower and bge on CUDA); the search itself is tens of milliseconds and the Python binding pays the load once.

### Question answering (`vi ask`)

| Run | accuracy | cited | citation video correct | citation within the anchor | cost / question | tool calls / question | median latency |
|---|---|---|---|---|---|---|---|
| Agent, first retrieval (AND-ed FTS) | 45/52 = **0.865** | 0.942 | 0.942 | 0.712 | $0.060 ($3.10 total) | 2.46 | 7.1 s |
| Agent, OR-ed FTS + empty-answer retry | 50/52 = **0.962** | 1.000 | 1.000 | 0.808 | $0.052 ($2.71 total) | 2.04 | 6.6 s |
| Retrieval-only baseline (one search, then answer) | 38/52 = **0.731** | 0.750 | 0.750 | 0.635 | $0.017 ($0.90 total) | 1.00 | 4.6 s |

The M2 target ("`idx.ask` answers the dev set at or above 80% with citations, retrieval-only baseline reported next to it") is met: 96.2% with the fixed retrieval against a 73.1% retrieval-only baseline (one search, then answer, no agent loop), at a third of the cost per question. The two remaining agent misses are real: it names Notion's "AI Writer" where the speaker's answer is meeting notes, and "covariance" where the lecture's metric is the collision rate (the same speech-to-text paraphrase gap that costs retrieval question q32). The baseline's 14 misses are mostly questions whose answer sits in a span the first search does not surface, which is what the agent's extra 1.04 tool calls per question buy. Three of the seven first-run misses were empty answers: the agent spent its six tool calls searching (the AND-ed FTS starved it of candidates), and the final no-tools turn came back with no text. The loop now asks once more, explicitly, before returning a partial answer. The other four misses are genuine: an answer that names the wrong product, one that stops short of the number asked for, and two where the accepted strings are stricter than the (arguably correct) paraphrase the model gave.

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
