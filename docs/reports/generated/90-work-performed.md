# Work performed

This chapter is generated from the repository at build time (2026-09-12). The project has 51 commits between 2026-09-10 and 2026-09-12; the Rust workspace holds 29,740 lines of library and binary source plus 2,785 lines of tests, examples and Python, with 154 automated tests.

## Commits by type

| Type | Commits | Meaning |
|---|---|---|
| `feat` | 22 | Features |
| `docs` | 12 | Documentation |
| `fix` | 6 | Fixes |
| `other` | 4 |  |
| `chore` | 3 | Chores |
| `ci` | 2 | CI |
| `perf` | 2 | Performance |

## Code by crate

| Crate | Source lines | Test / example / Python lines | Files | Tests |
|---|---|---|---|---|
| `vi-agent` | 1,756 | 345 | 7 | 5 |
| `vi-cli` | 1,580 | 221 | 13 | 4 |
| `vi-core` | 2,314 | 0 | 8 | 20 |
| `vi-index` | 3,790 | 0 | 8 | 12 |
| `vi-media` | 4,792 | 514 | 17 | 33 |
| `vi-perceive` | 3,311 | 126 | 13 | 20 |
| `vi-pipeline` | 5,916 | 840 | 21 | 24 |
| `vi-providers` | 4,284 | 63 | 16 | 25 |
| `vi-query` | 482 | 225 | 3 | 2 |
| `vi-server` | 8 | 0 | 1 | 0 |
| `vi-testkit` | 153 | 0 | 2 | 2 |
| `bindings/python` | 1,354 | 451 | 5 | 7 |
| **total** | **29,740** | **2,785** | | **154** |

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
- **feat** `bindings/python`: PyO3 binding; fix scenes for long shots and extraction JSON (`f501296`)
- **docs**: M1 report draft; QA dev set (52 questions) and its evaluation script (`8b0bbc0`)
- **fix** `vi-media`: spawn the decode worker from /proc/self/exe; relax the default wall-clock budget (`12c92ad`)
- **ci**: build and test the Python wheel on ubuntu and macos (`e366785`)
- **fix** `chapters`: titles need three words, no URL/e-mail, and an upper-frame box (`8b0edff`)
- **docs**: citation markers, adapter status, Python usage in the README (`e3009a7`)
- **fix** `vi-perceive`: share loaded ONNX models across provider handles; document CUDA 13 install (`d4c0da4`)
- **perf** `vi-perceive`: onnx_bench example; record GPU per-call timings and the no-NVDEC finding (`54c38d0`)

### 2026-09-12

- **feat** `python`: operators and agent policies in Python; inline policies; frame descriptions searchable (`68d4e01`)
- **chore** `scripts`: per-video timing table from vi index logs (`ebc7809`)
- **fix** `vi-pipeline`: list registered custom operators in the unknown-operator error (`37be6e4`)
- **docs**: M1 report open items updated for CUDA and Python operators (`3c1da8f`)
- **fix** `vi-index`: OR full-text terms with stopword removal; eval anchors match across caption cues (`e1fb2bf`)
- **perf** `vi-perceive`: heuristic cuDNN conv search and bucketed OCR widths; agent retries an empty final answer (`99cabb2`)
- **docs** `M1 report`: dataset timings, per-video table, retrieval and first QA results (`0caad38`)
- **docs**: link the M1 report; roadmap M2 progress with the QA result (`e85f263`)
- **docs** `M1 report`: QA rerun 96.2% and retrieval-only baseline 73.1%; vi ask accepts retrieval_only (`c2a3cb6`)
- **docs** `M1 report`: describe the two remaining QA misses accurately (`42d0a01`)
- **feat** `scripts/report`: Markdown-to-PDF technical report generator with Graphviz diagrams and charts; benchmark and team report builders; Gemini agentic-video eval harness (`95ca112`)
