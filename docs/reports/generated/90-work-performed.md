# Work performed

This chapter is generated from the repository at build time (2026-09-12). The project has 68 commits between 2026-09-10 and 2026-09-12; the Rust workspace holds 31,968 lines of library and binary source plus 3,331 lines of tests, examples and Python, with 159 automated tests.

## Commits by type

| Type | Commits | Meaning |
|---|---|---|
| `feat` | 26 | Features |
| `docs` | 19 | Documentation |
| `fix` | 11 | Fixes |
| `other` | 4 |  |
| `chore` | 4 | Chores |
| `ci` | 2 | CI |
| `perf` | 2 | Performance |

## Code by crate

| Crate | Source lines | Test / example / Python lines | Files | Tests |
|---|---|---|---|---|
| `vi-agent` | 1,768 | 347 | 7 | 5 |
| `vi-cli` | 1,663 | 221 | 14 | 4 |
| `vi-core` | 2,359 | 0 | 8 | 20 |
| `vi-index` | 3,790 | 0 | 8 | 12 |
| `vi-media` | 4,792 | 514 | 17 | 33 |
| `vi-perceive` | 3,311 | 126 | 13 | 20 |
| `vi-pipeline` | 5,958 | 896 | 21 | 26 |
| `vi-providers` | 4,308 | 63 | 16 | 25 |
| `vi-query` | 482 | 225 | 3 | 2 |
| `vi-server` | 2,030 | 488 | 11 | 3 |
| `vi-testkit` | 153 | 0 | 2 | 2 |
| `bindings/python` | 1,354 | 451 | 5 | 7 |
| **total** | **31,968** | **3,331** | | **159** |

## Commit log by day

Conventional-commit subjects, oldest first. Scopes name the crates a change touched.

### 2026-09-10

- **other**: first commit (`797f626`)
- **other**: updated plan for implementing videoindex (`7400565`)

### 2026-09-11

- **docs**: record azuremc machine facts and initial decisions (`158e188`)
- **feat** `vi-core`: timestamps, ids, config, errors, event bus, data model (`7449620`)
- **chore** `vi-core`: silence clippy on figment jail test (`c010657`)
- **feat** `vi-media`: libav probe/decode in a sandboxed worker process (`4e75edf`)
- **feat** `vi-perceive`: perceptual hash and WebP thumbnails (`c37c4ee`)
- **feat** `vi-index`: Storage trait and embedded SQLite/FTS5/blob backend (`fb26080`)
- **feat** `vi-pipeline`: operator DAG, tokio scheduler, checkpoints, Sample/PHash/Thumbnail (`c170e5a`)
- **feat** `vi-cli`: vi init, probe, index, status, doctor (`ef2e6a7`)
- **chore**: cargo-deny passes; optimise deps in dev profile; fix testkit item order (`a3cfb7a`)
- **ci**: GitHub Actions on ubuntu and macos (fmt, clippy -D warnings, tests, doc, cargo-deny) (`340a17f`)
- **docs**: record M0 timing on azuremc and mark M0 done (`95d2720`)
- **feat** `vi-media,vi-pipeline`: sidecar import, media cache, subtitle_import, YtDlp acquirer (`ab6c3e5`)
- **feat** `vi-query,vi-cli`: text search with RRF and chapter grouping; vi search (`c90bc7b`)
- **docs**: machine recommendations for indexing/dev and serving hosts (`4aa2889`)
- **docs**: kickoff prompt for the GCP A100 machine; azuremc machine file renamed (`baf4dc8`)
- **other**: update vibe summaries (`65354f5`)
- **other**: downloads vibe summary (`301140d`)
- **docs** `machine`: GCP A100 host facts, M0 re-timing (48.6 s/hour), local Whisper server (`7f80dda`)
- **feat** `vi-providers`: capability traits, role registry, governor, retries, cost accounting, openai_compat ASR (`3d934c4`)
- **feat** `vi-perceive,vi-pipeline`: Silero VAD and Whisper ASR operators through the provider layer (`c980840`)
- **feat** `vi-perceive,vi-pipeline`: shot_boundary operator with gap-free shot segments (`757f50b`)
- **feat** `vi-index,vi-perceive,vi-pipeline`: SigLIP/bge embeddings and a flat vector store (`731328d`)
- **feat** `vi-query,vi-cli`: hybrid search with text and image vector lists, temporal units from shots (`63ed64e`)
- **feat** `vi-perceive,vi-pipeline`: RapidOCR on-screen text with change-gated reads (`2e988b8`)
- **feat** `vi-pipeline`: budgets, operator output cache with skip/replay/run planning, per-range provider failures (`b886300`)
- **feat** `vi-media,vi-core`: Http and ObjectStore acquirers; auto default policy (`4bee414`)
- **fix** `ocr`: total-order reading sort; total_cmp everywhere; dev set and eval script (`68c8c6a`)
- **feat** `vi-providers`: openai_compat chat and embeddings, anthropic and gemini adapters, SSE parser, price tables (`8bbafdb`)
- **feat** `vi-agent,vi-cli`: agentic ask loop with tools, budgets, citations, sessions; vi ask, view, timeline (`4c6af4d`)
- **feat** `vi-pipeline`: fine pass operators scenes, chapters, vlm_describe, entities_events (`0ae338d`)
- **feat** `bindings/python`: PyO3 binding; fix scenes for long shots and extraction JSON (`0ddb4bf`)
- **docs**: M1 report draft; QA dev set (52 questions) and its evaluation script (`819f16f`)
- **fix** `vi-media`: spawn the decode worker from /proc/self/exe; relax the default wall-clock budget (`f685855`)
- **ci**: build and test the Python wheel on ubuntu and macos (`b5f6f42`)
- **fix** `chapters`: titles need three words, no URL/e-mail, and an upper-frame box (`5516f65`)
- **docs**: citation markers, adapter status, Python usage in the README (`60c2f7e`)
- **fix** `vi-perceive`: share loaded ONNX models across provider handles; document CUDA 13 install (`16972b3`)
- **perf** `vi-perceive`: onnx_bench example; record GPU per-call timings and the no-NVDEC finding (`f530bc8`)

### 2026-09-12

- **feat** `python`: operators and agent policies in Python; inline policies; frame descriptions searchable (`79e1417`)
- **chore** `scripts`: per-video timing table from vi index logs (`3d6be69`)
- **fix** `vi-pipeline`: list registered custom operators in the unknown-operator error (`b0f8dec`)
- **docs**: M1 report open items updated for CUDA and Python operators (`c2a7c39`)
- **fix** `vi-index`: OR full-text terms with stopword removal; eval anchors match across caption cues (`b0bb4d3`)
- **perf** `vi-perceive`: heuristic cuDNN conv search and bucketed OCR widths; agent retries an empty final answer (`bf66eff`)
- **docs** `M1 report`: dataset timings, per-video table, retrieval and first QA results (`c904209`)
- **docs**: link the M1 report; roadmap M2 progress with the QA result (`88f4104`)
- **docs** `M1 report`: QA rerun 96.2% and retrieval-only baseline 73.1%; vi ask accepts retrieval_only (`3644ef2`)
- **docs** `M1 report`: describe the two remaining QA misses accurately (`3ba39c4`)
- **feat** `scripts/report`: Markdown-to-PDF technical report generator with Graphviz diagrams and charts; benchmark and team report builders; Gemini agentic-video eval harness (`28fbffb`)
- **docs**: team catch-up report PDF; decisions on report generation and the Gemini comparison (`c0e845b`)
- **docs**: benchmark report PDF with retrieval, QA, indexing and Gemini agentic-video comparison (20-question like-for-like run) (`603999c`)
- **fix** `vi-pipeline`: a replaying consumer does not force its producer to run (`2090603`)
- **chore**: never track the maturin develop artefact; strip the CI wheel (`dbeeb82`)
- **feat** `vi-server`: HTTP API, SSE jobs and answers, blobs, bearer keys with daily spend cap, MCP, metrics, OpenAPI; vi serve (`d478888`)
- **feat** `bindings/node`: napi-rs binding with open/create/videos/status/timeline/search/add and an AsyncIterable ask; CI job; docs for M3 progress and the GPU end-to-end timing (`fb570e2`)
- **feat** `eval`: benchmark harness: LVBench/MINERVA/1H-VideoQA loaders, acquisition and indexing, ask runner with stratified sampling, uniform-sampling baseline, metrics with Wilson CIs, pareto report (`37e35b0`)
- **fix** `eval`: larger per-question budget for multiple-choice runs; re-ask once on an empty answer (`c7602e6`)
- **fix** `eval`: baseline uses the same indexed-video question pool as the index runners (`d39ba6d`)
- **fix** `vi-agent,vi-providers`: final turns keep tool definitions with tool_choice=none; eval prompts for tool-less runs (`2af9223`)
- **docs** `results`: LVBench pilot over 17 videos: agent 68%, retrieval-only 50%, uniform-32 52% (`30d5328`)
- **docs** `report`: team report covers M3/M4 progress, the eval harness and the results pages (`7b0d30e`)
- **fix** `entities_events`: repair trailing commas in extraction JSON; record the dataset fine pass (5 h 8 min, $45.96) (`d06d052`)
- **docs**: fine index vs coarse index on the dev set; reports regenerated with M3/M4 progress and LVBench pilot (`f2891ca`)
- **docs** `roadmap`: fine pass measured (`a7ba8da`)
- **feat** `eval`: fraction-based stratified sampling drawn from the whole benchmark, matrix runner (eval.run), sample-scoped acquisition, MINERVA config (`6e4888a`)
- **docs** `results`: LVBench 25% sample, interim pass (140/387 questions): agent 65.0%, retrieval-only 52.1%, uniform-32 57.1% (`ece9e0d`)
