# VideoIndex design documents

VideoIndex is an SDK and infrastructure framework that turns long videos into a queryable knowledge base. The core is Rust. Python and Node.js bindings, a CLI, and a server mode (HTTP, SSE, MCP) sit on top. Applications such as the interactive video QnA chat app are built against the SDK, never inside it.

Read the documents in order the first time. After that, each stands alone.

| # | Document | What it answers |
|---|---|---|
| 01 | [Overview](01-overview.md) | Why VideoIndex exists, goals, non-goals, principles, positioning |
| 02 | [Architecture](02-architecture.md) | Layers, Cargo workspace, indexing and query data flows, process model |
| 03 | [Rust boundary](03-rust-boundary.md) | What is Rust, what is Python/JS, why, and how the bindings work |
| 04 | [Data model](04-data-model.md) | Index schema, time model, on-disk layout, storage trait |
| 05 | [Indexing pipeline](05-indexing-pipeline.md) | Acquire, decode, sample, perceive, schedule, cache, progressive indexing |
| 06 | [Query and agents](06-query-and-agents.md) | Retrieval, the agentic loop, tools, citations, streaming, MCP |
| 07 | [Model providers](07-model-providers.md) | Provider traits, adapters, capability negotiation, A/B configuration |
| 08 | [Evaluation](08-evaluation.md) | LVBench, Minerva, 1H-VideoQA harness, metrics, dataset acquisition |
| 09 | [SDK and APIs](09-sdk-and-apis.md) | Python, Node, CLI, and server API surfaces, stability policy |
| 10 | [Roadmap](10-roadmap.md) | Milestones M0 to M5, repo layout, license, open questions |
| 11 | [Deployment](11-deployment.md) | videoindex.app on the azuremc machine |
| — | [Kickoff prompt](KICKOFF.md) | The prompt that started development on azuremc (M0) |
| — | [GCP kickoff prompt](KICKOFF-GCP.md) | The prompt to continue development on the GCP A100 machine (M1 onward) |
| — | [azuremc facts](MACHINE-azuremc.md) | The first host and what M0 measured on it; `MACHINE.md` describes the current host |
| — | [Machine recommendations](MACHINE-RECOMMENDATIONS.md) | Specs for the indexing/dev machine and the serving machine |
| — | [Decisions](DECISIONS.md) | Dated choices made where the design was silent |

## Glossary

These terms are used consistently across all documents.

- **Source**: a video the user asks to index. A local file, an object-storage URL, an HTTP URL, or a video-site URL such as YouTube.
- **Video**: the indexed record of one Source, with its Tracks and derived facts.
- **Track**: one elementary stream of a Video: video, audio, or subtitle.
- **Segment**: a time range of a Video at one level of a hierarchy: shot, scene, chapter. Segments are the unit of retrieval.
- **FrameSample**: a decoded frame at a known timestamp, kept as a thumbnail or a content hash. Never the whole video.
- **Span**: a timed piece of text derived from a Track: a TranscriptSpan from speech, an OcrSpan from on-screen text.
- **Description**: text produced by a vision-language model about a Segment or a FrameSample.
- **Operator**: one processing step in the indexing pipeline, such as shot detection or ASR. Operators are typed, versioned, and cacheable.
- **Provider**: an implementation of a model capability: VLM, ASR, OCR, text embedding, image embedding, LLM. Providers are swappable so runs can be A/B compared.
- **Index**: the complete stored output for one or more Videos. In the embedded backend an Index is a portable directory.
- **Tool**: a function exposed to the query-time agent, such as searching spans or viewing a time range at a chosen frame rate.
- **Budget**: the token, cost, and wall-clock limits a query or indexing job runs under.
- **Provenance**: which Operator, Provider, model version, and prompt produced a stored fact.
