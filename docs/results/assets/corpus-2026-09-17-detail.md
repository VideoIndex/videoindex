# Questions, ground truth and answers

For each question: the expected videos (and acceptable extras), then every system's score, the videos it named wrongly or missed, the judge's two-sentence assessment and the opening of its answer.

## corpus-01: Find all videos that mention Anthropic.

*Kind: mention.* Spoken mentions define the expected set; four more talks only show the word on a slide or model id and are acceptable.

Expected: Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Agentic AI Safety & Security by Dawn Song; [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Multi-Agent AI by Noam Brown; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; Evals 101 — Doug Guthrie, Braintrust; Full Workshop: Realtime Voice AI — Mark Backman, Daily; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia
  
Acceptable, not required: How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust); [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS; From Mixture of Experts to Mixture of Agents with Super Fast Inference - Daniel Kim & Daria Soboleva

**VI agent with library tools (Sonnet 5)** — quality 0.95, P 0.90, R 1.00, timestamps near a mention 1.00 (n=16), $0.082, 25 s
  
Wrong: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois
  
Judge: The answer lists 14 catalog videos, all mapping to real library entries with specific timestamps and brief evidence, so no videos appear invented and none lack timestamps. Some attributions look questionable (e.g., 'Mike at Anthropic' as CPO, Clay Bavor's 'alma mater' remark, and the Daniel Han on-screen text), and the eval/agent talks that most plausibly discuss Anthropic tooling may be over- or under-covered without verification.
  
> Based on an exhaustive scan of the library, "Anthropic" is mentioned in 14 of the 30 videos: - VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS: [00:51:57] Code/OCR shows the model ID us.anthropic.claude-3-7-sonnet used via AWS Bedrock . - Agentic AI MOOC Agentic AI Safety & Security by Dawn Song: [01:24:46] Discusses Anthropic's Constitutional Classifiers as an example of guardrails, and mentions a bug Anthropic fixed in early June . - Full Workshop: Realtime Voice AI — Mark Backman, Daily: [00:22:52] Says you can switch your LLM calls between OpenAI, Anthropic, Gemini, and Bedrock; also notes Anthropic and Google require a named system pro…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 0.95 (n=19), $0.160, 36 s
  
Judge: The answer names 14 catalog videos (it miscounts as 13), all of which map to real library entries with no invented titles, and every entry carries at least one specific timestamp. Attributions are plausible and specific though unverifiable here; the only clear flaw is the inconsistent final count.
  
> The following videos mention Anthropic: VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS [00:51:57] – The notebook code displayed on screen sets modelid using the Anthropic Claude identifier string "us.anthropic.claude-3-7-sonnet-20250219-v1:0" . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song [00:39:46] – A slide displays a vulnerability report for Claude Code that Anthropic patched . [01:24:46] – Dawn Song presents an example of Anthropic constitutional classifiers . [01:44:37] – Dawn Song notes that their work has been incorporated into system cards for Anthropic's latest models . Agentic AI MOOC …

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.00, P 0.00, R 0.00, timestamps near a mention 1.00 (n=2), $0.062, 15 s
  
Missed: Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Agentic AI Safety & Security by Dawn Song; [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Multi-Agent AI by Noam Brown; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; Evals 101 — Doug Guthrie, Braintrust; Full Workshop: Realtime Voice AI — Mark Backman, Daily; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia
  
Judge: The answer names only one library video (VoiceVision RAG) with a precise timestamp range and correctly maps it to a real catalog entry, explicitly dismissing other search hits as false positives. No invented videos appear, though the answer is likely under-inclusive and hedges about exhaustiveness rather than committing to a complete list.
  
> Based on the search results gathered, only one video shows clear textual evidence of "Anthropic": - VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS — [00:51:36–00:53:02] On-screen text/OCR shows a model identifier string "us.anthropic.claude37-sonnet..." displayed during a segment on building a multimodal agentic RAG system, indicating Anthropic's Claude model is referenced/used in the demo. The other hits returned by the search (e.g., in the ComfyUI workshop, ElevenLabs conversational AI workshop, and several UC Berkeley Agentic AI MOOC lectures) did not contain actual mentions of "Anthropic" in their transcript, OCR, or description text —…

**VI agent (Sonnet 5)** — quality 0.80, P 1.00, R 0.67, timestamps near a mention 0.94 (n=16), $0.577, 55 s
  
Missed: How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; Evals 101 — Doug Guthrie, Braintrust; Full Workshop: Realtime Voice AI — Mark Backman, Daily
  
Judge: The answer lists ten real catalog videos, each with specific timestamps and quoted evidence, and no invented titles appear. One entry is labeled with the wrong talk title (Oriol Vinyals' multi-agent lecture) though the answer self-corrects that the Cognition/Anthropic blog-post content belongs to Noam Brown, leaving a minor attribution ambiguity.
  
> Based on my search across the library, here are all videos that explicitly mention "Anthropic": - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [01:24:46] and [01:44:37] The lecturer cites "Anthropic Constitutional Classifiers" as an example runtime input-guardrail defense, and later notes her team's CyberGym benchmark work "has been included in the system cards for Anthropic's latest model release, Sonnet 4.5" . - VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS — [00:54:53]–[01:03:06] On-screen code repeatedly shows AWS Bedrock model IDs like "us.anthropic.claude-3-7-sonnet" and the word "ANTHR…

**VI agent (Gemini 3.8 Flash)** — quality 0.88, P 1.00, R 0.78, timestamps near a mention 1.00 (n=14), $0.191, 30 s
  
Missed: How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; Evals 101 — Doug Guthrie, Braintrust
  
Judge: The answer names ten real catalog videos, all mapped correctly and each with at least one specific timestamp, so no videos are invented or left unlocated. It hedges that visual/OCR coverage may be incomplete, and the attributions appear plausible though some (e.g., Tau-bench, Notion slide) cannot be verified here.
  
> Here are the videos in the library confirmed to mention or display Anthropic: Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song [01:24:46] / [01:44:37]: The speaker highlights Anthropic's constitutional classifiers and notes that their research was included in the system card for Anthropic's Claude 3.5 Sonnet release [[cite:01M296373YNY9A2442ZWNZK0WT:5086-5095], [cite:01M296373YNY9A2442ZWNZK0WT:6275-6285]]. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia [00:14:13] / [00:31:26]: The speaker references Anthropic when discussing the popularity of closed-source model providers and the debate over whether compani…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=16), $2.825, 111 s
  
Judge: The answer names 11 catalog videos, all mapping cleanly to real library entries with specific bracketed timestamps and plausible Anthropic-related evidence, and invents no videos (the parenthetical values after titles appear to be durations, not claims). Minor issues: some entries are labeled with odd "Video 13/14/16" prefixes, the final count of 11 excludes nothing important, and one Pipecat mention is attributed to "Aleix" rather than the listed speaker, which may be a misattribution.
  
> Evals 101 — Doug Guthrie, Braintrust (00:48:30) [00:22:28] Doug Guthrie mentions that an automated prompt evaluation run was hindered by an issue with his Anthropic API keys while an Anthropic rate limit error is displayed on screen. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song (01:48:49) [01:24:58] Dawn Song discusses Anthropic's constitutional classifiers as an example of runtime input guardrails. [01:44:41] Dawn Song notes that their CyberGym evaluation work was included in the system card for Anthropic's model release. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown (00:54:49) [00:53:24] Noam Brown highlights two i…

## corpus-02: Which videos mention DeepSeek (any DeepSeek model, such as R1 or V3)?

*Kind: mention.* Daniel Han's ASR renders it 'DC R1' once but also says DeepSeek elsewhere.

Expected: Introduction to LLM serving with SGLang - Philip Kiely and Yineng Zhang, Baseten; [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments; [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois; A2A & MCP Workshop: Automating Business Processes with LLMs — Damien Murphy, Bench; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia
  
Acceptable, not required: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Post-Training Verifiable Agents by Jiantao Jiao; Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Agentic AI Safety & Security by Dawn Song; From Mixture of Experts to Mixture of Agents with Super Fast Inference - Daniel Kim & Daria Soboleva; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | AI Agents to Automate Science by James Zou

**VI agent with library tools (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 0.90 (n=20), $0.115, 40 s
  
Judge: The answer names 12 catalog videos, all mapping to real library entries with specific timestamps and quoted or described evidence, and it explicitly excludes several false-positive matches. Without ground truth it's unclear which attributions are accurate; some claims (e.g., a DeepSeek chatbot demo in the MongoDB multimodal agents talk and the Vanilla JS talk quote) look plausible but could be misattributed or fabricated evidence.
  
> Based on an exhaustive scan of transcripts, on-screen text, and descriptions across the library, DeepSeek (including R1, V3, etc.) is mentioned in 12 videos: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois — [00:00:46], [00:01:01], [00:01:19]: The speaker cites approximate scaling numbers from open-source projects "especially LLaMA and DeepSeek" alongside Kimi . - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han — [00:02:00], [00:02:37]: On-screen slide reads "DeepSeek R1-0528" as Daniel Han discusses uploading AI models to Hugging Face using it as an example . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=14), $0.137, 27 s
  
Judge: The answer names 12 catalog videos, all mapping to real library entries, each with at least one specific timestamp and a short evidence description. No invented videos appear, though some attributions (e.g., DeepSeek-R1-Zero benchmarks in the MongoDB multimodal agents workshop and DeepSeek training dynamics in the Vanilla JS talk) are plausible-sounding but unverified and could be misattributed.
  
> The following 12 videos mention DeepSeek: Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois Timestamp: [00:00:46] Yann Dubois notes that his lecture covers open-source model details drawn from projects such as Kimi, LLaMA, and DeepSeek . [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han Timestamps: [00:02:35], [00:07:26] Daniel Han discusses releasing 1.58-bit quants of DeepSeek-R1-0528 and explains how the January 2025 release of DeepSeek-R1 shifted perceptions by proving open-source models could match reasoning models like o1 , . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangq…

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.60, P 1.00, R 0.43, timestamps near a mention 1.00 (n=5), $0.071, 16 s
  
Missed: Introduction to LLM serving with SGLang - Philip Kiely and Yineng Zhang, Baseten; [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; A2A & MCP Workshop: Automating Business Processes with LLMs — Damien Murphy, Bench
  
Judge: The answer names three real catalog videos with specific timestamps and plausible, well-described DeepSeek content, and it correctly excludes low-relevance hits (ComfyUI, ElevenLabs, Peter Stone) rather than hallucinating them as matches. No invented videos appear, though the answer admits its list may be non-exhaustive, so coverage of other brief mentions in the library could be incomplete.
  
> Based on the search results, the following videos contain clear, substantive mentions of DeepSeek models: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois — [01:12:24], [01:02:44], [00:05:44]. The lecturer discusses DeepSeek-R1 as an example of a reasoning model, recommends reading the DeepSeek-R1 paper for how reasoning prompts (math/coding) are used, and explains DeepSeek R1's rejection-sampling approach for scalable SFT data generation . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia — [00:12:22]. The speaker notes that DeepSeek was one of the first papers to detail test-time scaling capability, educating t…

**VI agent (Sonnet 5)** — quality 0.73, P 1.00, R 0.57, timestamps near a mention 1.00 (n=10), $0.136, 24 s
  
Missed: [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; A2A & MCP Workshop: Automating Business Processes with LLMs — Damien Murphy, Bench
  
Judge: All five named videos map to real catalog entries, each with specific timestamps and concrete DeepSeek-related evidence, with no invented titles. The claims are plausible and well-attributed, though the accuracy of individual timestamps and the completeness of the list (e.g., other workshops possibly mentioning DeepSeek) cannot be verified here.
  
> Based on multiple searches across the library, here are the videos that mention DeepSeek models: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois — [01:02:06], [01:10:19], [01:12:29]: The lecturer discusses DeepSeek-R1 as an example of scalable SFT data generation via rejection sampling, walks through its RL training pipeline, and explains the GRPO algorithm it introduced, even recommending students read the DeepSeek R1 paper . - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han — [00:06:30], [02:22:16]–[02:32:20]: Daniel Han describes how DeepSeek R1's January 2025 release proved open-source models could match …

**VI agent (Gemini 3.8 Flash)** — quality 0.73, P 1.00, R 0.57, timestamps near a mention 1.00 (n=6), $0.152, 35 s
  
Missed: [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; A2A & MCP Workshop: Automating Business Processes with LLMs — Damien Murphy, Bench
  
Judge: The answer names six catalog videos, all real entries, each with a specific timestamp and a plausible DeepSeek-related description. No invented videos or missing timestamps, though correctness of individual attributions and completeness of the list cannot be verified from the answer alone.
  
> Here are the videos across the library that mention DeepSeek: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [00:07:26] : Daniel Han explains how the release of DeepSeek-R1 in January 2025 showed that open-source models could match proprietary reasoning models such as OpenAI's o1. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia [00:12:22] : Yangqing Jia highlights that DeepSeek was among the first papers demonstrating test-time scaling and reasoning capabilities, enabling open models to close the gap with proprietary systems. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Post-Training Verifiable Age…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 0.94 (n=17), $2.515, 104 s
  
Judge: The answer names ten catalog videos, all mapping to real library entries, each with at least one specific timestamp and a short supporting quote or paraphrase. No invented videos appear, though some attributions (e.g., DeepSeek listed among multimodal architectures, and the SGLang speaker attribution) are plausible but unverified, and the many timestamps given for the Yann Dubois lecture are broad and could include imprecise hits.
  
> Below are the videos from the library that mention DeepSeek: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han (02:42:27): [00:07:27] – Daniel Han describes how the open-source community was lagging behind OpenAI's o1 until "in 2025 January, DeepSeek R1 came along and they released R1," proving that open-source models can achieve parity with frontier reasoning models. Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB (00:36:57): [00:17:48] – Apoorva Joshi notes that modern multimodal LLM architectures include models from DeepSeek, Anthropic (Claude), and OpenAI. A2A & MCP Workshop: Automating Business Processes with LLMs — Damien…

## corpus-03: Which talks mention Cursor, the AI code editor?

*Kind: mention.* Sida Wang says 'see the cursor?' about the mouse pointer (excluded: claiming it is wrong). The Vibe Coding workshop's 'but, like the cursor' is ambiguous.

Expected: Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Post-Training Verifiable Agents by Jiantao Jiao; How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust); RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI; Evals 101 — Doug Guthrie, Braintrust; A2A & MCP Workshop: Automating Business Processes with LLMs — Damien Murphy, Bench; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia
  
Acceptable, not required: [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments; VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | AI Agents to Automate Science by James Zou

**VI agent with library tools (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 0.93 (n=14), $0.247, 50 s
  
Judge: The answer names ten catalog videos, all mapping to real library entries with specific timestamps and quoted evidence, plus an explicit exclusion list for false 'cursor' (mouse pointer) hits. No invented videos or missing timestamps; attributions appear plausible and internally consistent, though the accuracy of individual quotes and times cannot be verified here.
  
> Based on the transcripts and on-screen evidence, these talks explicitly mention Cursor (the AI code editor): - RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI [00:24:55], [00:29:34], [00:30:07] — During a live demo, the speaker says "here I have cursor" while loading a banking dataset for a fine-tuning example, and Cursor UI elements ("Cursor Tab") appear on screen . - Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize [00:16:07] — The speaker describes pasting a notebook into Cursor and asking it to build a LangGraph-based UI workflow example . - Agentic AI MOOC Evolution of System Designs by Yangqing Jia [00:26:28] — The speaker cites Cursor as an exampl…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=12), $0.128, 38 s
  
Judge: The answer names ten catalog videos, all mapping to real library entries with specific timestamps and short evidence quotes, so there are no invented videos or untimestamped mentions. Some attributions (e.g., the Evals 101 'Cursor-like interface' and the Notion/Braintrust reference) are plausible but unverifiable here and could be misattributed or padded.
  
> The following talks mention Cursor, the AI code editor: RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI [00:24:55], [00:26:25] Ilan Bigio opens the Cursor code editor to demonstrate fine-tuning on a banking dataset and remarks during the demo that "Cursor abandoned me today" , . Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize [00:16:13], [00:20:30] Aman Khan explains putting a notebook into Cursor to generate a UI workflow with LangGraph, later pointing out the agent application that Cursor helped write , . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia [00:24:16] Yangqing Jia discusses the rise of coding ass…

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.00, P 0.00, R 0.00, $0.032, 9 s
  
Missed: Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Post-Training Verifiable Agents by Jiantao Jiao; How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust); RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI; Evals 101 — Doug Guthrie, Braintrust; A2A & MCP Workshop: Automating Business Processes with LLMs — Damien Murphy, Bench; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia
  
Judge: The answer cites no videos at all and asserts that no talk in the library mentions Cursor, while admitting the search was incomplete. There are no hallucinated videos or misattributions, but it provides no timestamps or positive evidence, making it an unhelpful non-answer.
  
> Based on the search performed, I did not find clear evidence of any talk explicitly mentioning "Cursor" the AI code editor by name. The search results returned matches on generic terms like "video" (OCR noise) and unrelated topics (fine-tuning, RL, multimodal RAG), but none of the retrieved evidence actually references the Cursor code editor. I do not have sufficient evidence in the index to confirm any video discussing Cursor specifically. A more targeted exhaustive scan (e.g., via findmentions for "Cursor") would be needed to verify this definitively, but based on the search results available in this turn, no video can be confidently cited as mentioning Cursor. Videos: 0

**VI agent (Sonnet 5)** — quality 0.55, P 1.00, R 0.38, timestamps near a mention 1.00 (n=8), $0.239, 36 s
  
Missed: Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize; How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust); Evals 101 — Doug Guthrie, Braintrust; A2A & MCP Workshop: Automating Business Processes with LLMs — Damien Murphy, Bench; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen
  
Judge: The answer names five real catalog videos, each mapped correctly and each with specific timestamps, with no invented titles. Attributions are plausible and specific (slide text, UI elements, spoken quote), though correctness of individual timestamp evidence cannot be verified here.
  
> Based on my research, here are the talks that mention Cursor: - [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments — [00:57:52]–[00:58:16] The presenter demonstrates installing the Playwright MCP server and shows on-screen documentation/README text listing supported editors: "VS Code, Cursor, Wiz..." with steps for "Installin[g] Cursor" alongside VS Code . - RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI — [00:29:30], [00:29:34] While live-coding in a notebook, the on-screen editor UI displays a "Cursor Tab" autocomplete suggestion element, indicating the presenter is using the Cursor code editor for the demo . - Agentic AI MOOC UC Berkel…

**VI agent (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=15), $0.130, 32 s
  
Judge: The answer names nine catalog videos, all real entries with specific timestamps and brief evidence quotes, though its self-reported count of 9 conflicts with the ten bullet items listed (an internal inconsistency). Some cited ranges are implausibly wide (e.g., 00:20:38–01:15:28 for Aman Khan and 00:24:55–01:23:10 for Ilan Bigio), so attribution precision is questionable even though no videos appear invented.
  
> The following talks mention Cursor, the AI code editor: Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia [00:24:20] – [00:26:32]: Yangqing Jia notes the rapid adoption of AI coding tools like Cursor among prosumers who embed them into their everyday coding workflows [[cite:01M297H2QCHQMH7C6GPX6NE0V5:1455-1465], [cite:01M297H2QCHQMH7C6GPX6NE0V5:1588-1600]]. Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize [00:20:38] – [01:15:28]: Aman Khan explains how Cursor helped him write the code for an agent system using product documentation and encourages opening up codebase access via Cursor across engineering and product teams [[ci…

**Gemini agentic video (3×10 + merge)** — quality 0.93, P 1.00, R 0.88, timestamps near a mention 0.94 (n=18), $0.081, 38 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen
  
Judge: All eight cited videos exist in the catalog and each claim carries specific timestamps, with no invented titles. However, the attribution for the Notion/Braintrust talk (6YdPI9YbjbI) credits Doug Guthrie with a Braintrust prompt-optimization demo that duplicates the Evals 101 claim, which looks like a misattribution/copy across entries.
  
> The following talks mention Cursor, the AI code editor: Evals 101 — Doug Guthrie, Braintrust (00:48:30) [00:21:44] While demonstrating Braintrust's new prompt optimization loop feature, Doug Guthrie compares its workflow to a Cursor-like interface. A2A & MCP Workshop: Automating Business Processes with LLMs — Damien Murphy, Bench (01:23:13) [00:40:37], [00:44:18] Damien Murphy notes that whenever he catches Cursor making an error it reflexively replies "you're absolutely right," and later explains that command-line interfaces are easier for coding tools like Cursor or Augment to write and test. RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI (01:46:14) [00:24:55], [00:26:15], [01…

## corpus-04: Which videos talk about poker?

*Kind: mention.* 

Expected: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Multi-Agent AI by Noam Brown; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone; Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals

**VI agent with library tools (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=6), $0.049, 6 s
  
Judge: The answer names three real catalog videos (Noam Brown, Peter Stone, Oriol Vinyals), each with specific timestamps, and invents no videos. Noam Brown's poker-centric talk is plausibly the strongest match, though the Vinyals poker-analogy claim and some of Stone's timestamps are unverified and may be loose attributions.
  
> - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown — [00:00:27], [00:02:15] The talk opens by citing AlphaGo, DOTA/StarCraft and poker as milestones in game-playing AI, then poses the question of who counts as "the better poker player," using a slide comparing skill vs. money won . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Autonomous Agents by Peter Stone — [00:09:21], [00:13:48], [00:45:23] Peter Stone references poker alongside chess and Go as classic multi-agent/turn-taking benchmark tasks and success cases for game-playing agents . - Agentic AI MOOC UC Berkeley CS294-196 F25 Multi-Agent Systems in Era of LLMs by Oriol Vinyals — [00:36:09] The speaker n…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=8), $0.062, 28 s
  
Judge: All three named videos exist in the catalog and are plausibly poker-related (Noam Brown's multi-agent talk especially), with specific timestamps given for each. No invented videos, though the precise timestamp accuracy and the Libratus/CFR attribution details cannot be verified from the answer alone.
  
> Here are the videos across the library that talk about poker: Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown Timestamps: [00:00:28], [00:02:29], [00:08:40], [00:17:02], [00:22:04] Noam Brown extensively discusses poker as a benchmark imperfect-information game, examining equilibrium concepts and bluffing frequencies, and detailing how counterfactual regret minimization enabled the Libratus AI to defeat top human poker professionals [[cite:01M296FBHQD7SWTC6TZ59NSMQ9:27-41], [01M296FBHQD7SWTC6TZ59NSMQ9:149-210], [01M296FBHQD7SWTC6TZ59NSMQ9:519-555], [01M296FBHQD7SWTC6TZ59NSMQ9:1017-1035], [01M296FBHQD7SWTC6TZ59NSMQ9:1316-1345]]. Agentic AI MOOC UC Berkeley CS294-1…

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.00, P 0.00, R 0.00, $0.036, 12 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Multi-Agent AI by Noam Brown; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone; Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals
  
Judge: The answer concludes no library video could be confirmed to discuss poker, explicitly declining to cite Noam Brown's talk despite flagging it as a plausible candidate, so no claims are asserted. No invented videos or misattributions appear, but the answer provides no timestamps and likely misses the actual poker discussion in the Noam Brown multi-agent talk.
  
> Based on my search, none of the returned hits contain clear, direct evidence of poker being discussed — the results (ComfyUI workshop OCR reading "Video", ElevenLabs speech-to-text OCR, Peter Stone's "Autonomous Agents" talk on human-in-the-loop learning and Gran Turismo drivers, and Weizhu Chen's training-data-size discussion) do not mention poker specifically. I was not able to confirm any video in the library that actually discusses poker from the searches performed. Notably, Noam Brown's talk ("Multi-Agent AI by Noam Brown") is a plausible candidate given his real-world fame for poker-playing AI (Pluribus/Libratus), but my search did not surface a direct poker mention/timestamp in that v…

**VI agent (Sonnet 5)** — quality 0.50, P 1.00, R 0.33, timestamps near a mention 0.86 (n=7), $0.225, 26 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone; Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals
  
Judge: The answer names a single real catalog video (Noam Brown's Multi-Agent AI lecture) with specific timestamp ranges, which is plausible given Brown's poker AI background, and invents no videos. It may under-report other videos that mention poker in passing, but the attribution and details given are coherent and appropriately timestamped.
  
> Based on my search of the transcripts, on-screen text, and descriptions across the library, only one video substantively discusses poker. - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown — [00:02:15]–[00:04:32], [00:08:00]–[00:08:58], [00:16:50]–[00:18:56], [00:22:09] Noam Brown (who led Libratus/Pluribus poker AI research) uses poker throughout the lecture as the central example of imperfect-information game theory: he opens by asking "who is the better poker player?" to distinguish minimax-equilibrium play from population best-response play ; he explains why poker strategy is more complex than rock-paper-scissors and quotes a "poker strategy guide" saying "Pok…

**VI agent (Gemini 3.8 Flash)** — quality 0.80, P 1.00, R 0.67, timestamps near a mention 1.00 (n=6), $0.078, 17 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone
  
Judge: The answer names two real catalog videos (Noam Brown's Multi-Agent AI and Oriol Vinyals' Multi-Agent Systems), both with specific timestamps and no invented entries. Attributions are plausible—poker is central to Brown's work and imperfect-information games fit Vinyals' StarCraft discussion—though the timestamp precision cannot be verified here.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown — [00:00:27], [00:02:23], [00:07:56], [00:17:02], [00:22:08] Noam Brown frequently references poker throughout his lecture to illustrate game-theoretic concepts, discussing self-play AI milestones , evaluating what defines a better poker player , explaining minimax equilibria and bluffing frequencies [[cite:01M296FBHQD7SWTC6TZ59NSMQ9:476-555], [cite:01M296FBHQD7SWTC6TZ59NSMQ9:1017-1036]], and detailing how his research defeated world-class poker professionals in heads-up no-limit Texas Hold'em . Agentic AI MOOC UC Berkeley CS294-196 F25 Multi-Agent Systems in Era of LLMs by Oriol Vinyals — [00:36:12] Oriol Vinyals c…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=9), $0.046, 26 s
  
Judge: The answer names three real catalog videos (Noam Brown, Peter Stone, Oriol Vinyals), each with specific timestamps and plausible poker-related descriptions, with no invented videos. Attributions appear consistent with the speakers' known topics, though the timestamp accuracy and the Vinyals/Stone mentions cannot be confirmed here.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown (00:54:49): [00:00:29], [00:02:22], [00:08:00], [00:17:08], [00:21:55] — Noam Brown uses poker as a prime example of an imperfect-information game to explain concepts such as minimax equilibria versus population best response, the value of bluffing probabilities, and how regret-matching algorithms from his PhD work defeated top human Texas Hold'em poker professionals. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Autonomous Agents by Peter Stone: [00:09:21]: Peter Stone cites poker alongside chess and Go as examples of tasks where AI systems have surpassed the best human players. [00:13:58]: He contrasts real-time…

## corpus-05: Which videos discuss prompt injection attacks?

*Kind: mention.* 

Expected: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Agentic AI Safety & Security by Dawn Song; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor

**VI agent with library tools (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 0.89 (n=9), $0.092, 15 s
  
Judge: The answer names only two real catalog videos (Dawn Song's safety/security lecture and Clay Bavor's deployment lessons), both with specific timestamps and plausible, on-topic evidence. No invented videos appear, though the list may be incomplete since other talks could touch on prompt injection; timestamp accuracy cannot be verified from the answer alone.
  
> Based on exhaustive scans of the transcripts and on-screen text, prompt injection attacks are substantively discussed in two videos: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song [00:14:37], [00:32:26], [00:33:00], [00:42:58], [00:56:24], [01:26:00] — Extended coverage of prompt injection as an attack vector, including heuristic-based and optimization-based attack methods, indirect prompt injection examples, defenses like "PromptArmor," and a case study of a real-world prompt injection attack against a coding agent . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Practical Lessons from Deploying...by Clay Bavor [00:08:51], [00:42:44], [00:45:3…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=4), $0.084, 27 s
  
Judge: Both named videos exist in the catalog and are plausibly on-topic, each with specific timestamp ranges rather than vague references. No invented videos appear, though the answer may be incomplete since other talks (e.g., safety/agent workshops) could also touch on prompt injection.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song [00:32:21]–[00:34:45] Dawn Song details heuristic and optimization-based prompt injection methods, explaining how direct attacks and indirect prompt injection through external data can manipulate agents into following unauthorized instructions . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Practical Lessons from Deploying...by Clay Bavor [00:43:05]–[00:44:15] Clay Bavor discusses the necessity of heavily hardening customer-facing agents against prompt injection attacks—citing real-world examples such as queries phrased in reversed Icelandic to leak system instructions—and employing layered determinis…

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.67, P 1.00, R 0.50, timestamps near a mention 1.00 (n=2), $0.060, 9 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor
  
Judge: The answer names one real catalog video (Dawn Song's Agentic AI Safety & Security) with a specific, plausible timestamp range and correctly attributes prompt injection content to it. Other videos are mentioned only to be dismissed as weak matches, so no invented videos or misattributions appear, though the exclusive focus on a single lecture may under-cover other discussions.
  
> Based on the search results, the library discussion of prompt injection attacks is concentrated in one lecture: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [00:32:23]–[00:34:39] The lecturer explains categories of prompt injection attack methods (heuristic-based and optimization-based) and introduces indirect prompt injection attacks via a diagram showing an attacker, external resources, an LLM-integrated application, and a user; she also notes that prior discussion covered "direct prompt injection attacks where the attacker would need to have direct access." No other video in the library returned clear evidence of discussing prompt injection…

**VI agent (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 0.82 (n=17), $0.292, 25 s
  
Judge: The answer names only two catalog videos (Dawn Song's safety lecture and Clay Bavor's talk), both real and plausibly attributed, each with specific timestamps. It includes leftover reasoning chatter and a duplicated timestamp, and may be under-inclusive of other security/agent talks, but no invented videos or clearly misattributed content appear.
  
> Found another relevant video: "Practical Lessons from Deploying...by Clay Bavor" discusses prompt injection attacks and guardrails. Let me check the Azure AI Agent Service video too, and a couple other likely candidates (agent security-focused talks).I have sufficient evidence for two distinct videos. Let me do one more check for other candidate talks (e.g., MCP workshop, Multi-Agent Safety talks) to be thorough.Based on the searches, prompt injection attacks are discussed substantively in two videos in the library. - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [00:28:34], [00:31:56], [00:34:38], [00:40:15], [00:44:38], [00:48:29], [00:51:52], …

**VI agent (Gemini 3.8 Flash)** — quality 0.67, P 1.00, R 0.50, timestamps near a mention 1.00 (n=4), $0.152, 36 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor
  
Judge: The answer names a single real catalog video (Dawn Song's Agentic AI Safety & Security talk) with specific timestamp ranges, which is a plausible and on-topic attribution for prompt injection discussion. No invented videos appear, though the answer may be incomplete since other talks (e.g., agent/MCP workshops) could also touch on prompt injection.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [00:28:33]–[00:45:20], [01:25:26]–[01:31:48] Dawn Song discusses both direct and indirect prompt injection attacks, demonstrating how untrusted inputs (such as malicious prompts, calendar invites, or resumes) can override system instructions to leak sensitive data or execute unauthorized agent actions, as well as defenses like PromptArmor and dynamic security policies to mitigate them [[cite:01M296373YNY9A2442ZWNZK0WT:1713-1725], [cite:01M296373YNY9A2442ZWNZK0WT:2078-2095], [cite:01M296373YNY9A2442ZWNZK0WT:2680-2696], [cite:01M296373YNY9A2442ZWNZK0WT:5508-5520]]. Videos: 1

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=4), $0.068, 40 s
  
Judge: The answer names two real catalog videos with specific timestamps, both plausibly on-topic for prompt injection (Dawn Song's safety/security lecture and Clay Bavor's deployment lessons). No invented videos appear, though the list may be incomplete given other agent-security-adjacent talks, and the timestamp precision cannot be verified here.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song - [00:28:35] - [00:44:45]: Dawn Song explains direct and indirect prompt injection attacks, demonstrating how malicious user inputs or untrusted third-party data (such as resumes, local files, or calendar invites) can hijack model execution to leak sensitive prompts, exfiltrate credentials, or trigger unintended actions. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Practical Lessons from Deploying...by Clay Bavor - [00:09:36], [00:43:08]: Clay Bavor discusses the critical challenge of protecting customer-facing agents from prompt injection attacks, sharing examples from red teaming and outlining Sier…

## corpus-06: Which talks mention LoRA (low-rank adaptation) fine-tuning?

*Kind: mention.* Whisper writes the acronym as 'Laura' in several talks; the patterns cover both spellings.

Expected: RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI; [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; ComfyUI Full Workshop — first workshop from ComfyAnonymous himself!; From Mixture of Experts to Mixture of Agents with Super Fast Inference - Daniel Kim & Daria Soboleva; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen
  
Acceptable, not required: The AI Engineer’s Guide to Raising VC — Dani Grant (Jam), Chelcie Taylor (Notable)

**VI agent with library tools (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=6), $0.160, 33 s
  
Judge: The answer names five genuine library videos with specific timestamps and quotes, and explicitly flags the VC-raising talk as a false positive (a person named 'Laura'), so that one is excluded from claims. No invented videos appear and all timestamps are concrete, though the stated count of 'six talks' conflicts with the final tally of 5 and attribution accuracy of the quotes cannot be verified here.
  
> Based on an exhaustive scan of the library, LoRA (low-rank adaptation) fine-tuning is mentioned in six talks: - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [02:03:57] Daniel Han shows Unsloth notebook code setting lorarank/maxlorarank and loadin4bit=False for LoRA 16bit, and explicitly advises "your Laura [LoRA] rank... please set the alpha to be two times the LoRA rank... it speeds up training dramatically" . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Training Agentic Models by Weizhu Chen [00:00:34], [00:31:15] An opening slide lists "LoRA: GPT-3 customization in 2020," and later the speaker discusses "how can we use LoRA in the RL"…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 0.88 (n=8), $0.115, 30 s
  
Judge: The answer names five catalog videos, all real, each with specific timestamps and plausible LoRA-related evidence, and no invented titles appear. One citation block is left as raw internal markers, but otherwise attributions are coherent and speaker-appropriate; correctness of each timestamp cannot be verified here.
  
> The following talks across the library mention LoRA (low-rank adaptation) fine-tuning: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han - [00:44:33] Daniel Han mentions using parameter-efficient fine-tuning like LoRA rather than fine-tuning every single parameter . - [02:03:07] In a live coding demonstration with Unsloth, he explains that LoRA is a parameter-efficient fine-tuning method adding small weights instead of updating every model weight, and advises setting LoRA alpha to two times the LoRA rank . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Training Agentic Models by Weizhu Chen - [00:00:34] A slide lists Weizhu Chen's contributions i…

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.57, P 1.00, R 0.40, timestamps near a mention 0.75 (n=4), $0.064, 11 s
  
Missed: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; ComfyUI Full Workshop — first workshop from ComfyAnonymous himself!; From Mixture of Experts to Mixture of Agents with Super Fast Inference - Daniel Kim & Daria Soboleva
  
Judge: The answer names two real catalog videos (Weizhu Chen's Training Agentic Models and Ilan Bigio's OpenAI fine-tuning talk) with specific timestamp ranges and plausible LoRA-related content, inventing no videos. It explicitly excludes ComfyUI, Yann Dubois, and Peter Stone as unconfirmed, which is conservative and may omit genuine mentions (e.g., LoRA in ComfyUI or Daniel Han's fine-tuning workshop), but nothing appears misattributed.
  
> Based on the search results, I found clear evidence of LoRA (low-rank adaptation) fine-tuning being discussed in two talks: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Training Agentic Models by Weizhu Chen — [00:44:46]–[00:48:31]: The speaker presents a slide titled "LoRA on RL" and explains why LoRA works effectively for reinforcement learning training, describing it as a form of regularization that also brings generalization and GPU-efficiency benefits . - RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI — [00:11:36]–[00:13:43]: The speaker explains that fine-tuning methods like LoRA ("low rank adaptation") only train a small subset of model weights and are not suited for…

**VI agent (Sonnet 5)** — quality 0.75, P 1.00, R 0.60, timestamps near a mention 0.67 (n=9), $0.110, 15 s
  
Missed: ComfyUI Full Workshop — first workshop from ComfyAnonymous himself!; From Mixture of Experts to Mixture of Agents with Super Fast Inference - Daniel Kim & Daria Soboleva
  
Judge: All three named videos map to real catalog entries (Daniel Han's workshop, Weizhu Chen's lecture, Ilan Bigio's fine-tuning talk) with specific, plausible timestamps and coherent LoRA-related evidence. No invented videos or obvious misattributions, though the answer may be incomplete if other talks (e.g., other fine-tuning sessions) also mention LoRA.
  
> I have enough evidence now. The three relevant videos are clear from the searches. - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han — [01:57:26], [02:26:58], [02:27:38], [02:28:46] Daniel Han demos Unsloth notebooks setting lorarank, loraalpha, and maxlorarank parameters (e.g. "lorarank = 32, Larger rank = smarter, but slower") while loading models for GRPO fine-tuning . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Training Agentic Models by Weizhu Chen — [00:44:46]–[00:48:31] Weizhu Chen has a dedicated segment "Why LoRA in RL Training?" explaining that LoRA acts as regularization, works well with reinforcement learning, and offers GPU/co…

**VI agent (Gemini 3.8 Flash)** — quality 0.89, P 1.00, R 0.80, timestamps near a mention 1.00 (n=12), $0.126, 27 s
  
Missed: From Mixture of Experts to Mixture of Agents with Super Fast Inference - Daniel Kim & Daria Soboleva
  
Judge: All four named videos map to real catalog entries, each with specific timestamp ranges and plausible LoRA-related content for their topics (Unsloth workshop, OpenAI fine-tuning, agentic model training, ComfyUI image generation). No invented videos or untimestamped mentions, though the exact timestamp accuracy and the Weizhu Chen LoRA-in-RL attribution cannot be verified from the given information.
  
> The following talks mention LoRA (low-rank adaptation) fine-tuning: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [02:03:07]–[02:04:10] : Daniel Han describes utilizing LoRA as a parameter-efficient fine-tuning method to avoid the cost of tuning every weight, advising practitioners to set loraalpha to twice the lorarank for faster training. RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI [00:11:46]–[00:12:15] , [01:03:09]–[01:03:36] : Ilan Bigio introduces low-rank adaptation as training a subset of decomposed matrix weights and clarifies during Q&A that OpenAI fine-tuning generates a LoRA component that gets loaded alongside base …

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 0.82 (n=11), $0.072, 44 s
  
Judge: All five named videos map to real catalog entries, each with specific timestamps and plausible LoRA-related content; no invented videos appear. One oddity is the stray duration-like '(01:04:36)' appended to the Weizhu Chen title, which may be a spurious timestamp, but otherwise attributions look coherent and specific.
  
> [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [00:44:34] Daniel Han notes that parameter-efficient fine-tuning methods like LoRA allow training a smaller subset of weights rather than updating all model parameters. [02:03:10] In the hands-on demonstration, he explains how the notebook uses LoRA to fine-tune added low-rank weights efficiently to save compute and memory, recommending setting alpha to twice the LoRA rank. [02:27:00] He points out that when training with LoRA, you need more than three data rows to prevent the gradients from becoming zero. RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI [00:12:08] Ilan Bigio introduces …

## corpus-07: Which video discusses Kubernetes, and in what context does the speaker bring it up?

*Kind: topic.* 

Expected: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia

Key facts:

- Only Yangqing Jia's 'Evolution of System Designs' lecture discusses Kubernetes.
- He brings it up as one of the choices for managing a GPU/compute cluster, as an alternative to a hand-managed cluster run by system administrators (with an aside that Oriol Vinyals was once one of those sysadmins).

**VI agent with library tools (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=3), $0.074, 7 s
  
Judge: The answer identifies the correct single video (Yangqing Jia's 'Evolution of System Designs') with specific, plausible timestamps and accurately describes Kubernetes as one option for managing a GPU cluster versus a manually administered/Slurm-style cluster. It omits the aside about Oriol Vinyals having been such a sysadmin and adds some extra detail (pods, 'holy war'), but no videos are invented or misattributed.
  
> - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia [00:42:52] [00:43:13]–[00:44:23] The speaker contrasts two ways teams manage GPU clusters—manual/Slurm-style admin vs. Kubernetes—explaining that while Kubernetes is a strong abstraction for SREs/ops people, AI researchers dislike having to think in terms of pods, deployments and parallelism, describing an ongoing "holy war" between the Slurm researcher mindset and the Kubernetes operations mindset, and noting GPUs fail often enough that some abstraction is still needed to hide operational burdens from developers . Videos: 1

**VI agent with library tools (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=1), $0.024, 11 s
  
Judge: The answer identifies the single correct video (Yangqing Jia's 'Evolution of System Designs') with a specific timestamp and no invented videos, and correctly frames Kubernetes as an option for GPU cluster management versus simpler researcher-preferred setups. It omits the aside about Oriol Vinyals having been a cluster sysadmin and adds some unverified color ('holy war', Slurm/MPI), so the contextual detail is close but not exact.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia [00:43:13] – Yangqing Jia discusses GPU cluster management and the "holy war" between researchers and operations, explaining that while Kubernetes (K8s) is a standard abstraction for site reliability engineers, AI researchers find concepts like pods and deployments overly complex and prefer simple execution environments (like Slurm or direct MPI setups) where they don't have to deal with Kubernetes . Videos: 1

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.75, P 1.00, R 1.00, facts 0.50, timestamps near a mention 1.00 (n=2), $0.059, 11 s
  
Judge: The answer correctly and exclusively identifies Yangqing Jia's 'Evolution of System Designs' lecture with a specific timestamp range and a quoted excerpt, with no invented videos. However, it frames the context as a Slurm-vs-Kubernetes cultural 'holy war' rather than as a choice for managing a GPU/compute cluster versus a hand-managed sysadmin cluster, and omits the Oriol Vinyals aside.
  
> Based on the search results, Kubernetes is discussed in one video in the library: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia — [00:42:34]-[00:44:14] (chapter "Compute, storage, network, big data,"): The speaker frames Kubernetes as one side of a cultural/technical divide in infrastructure, saying "I don't really want to deal with Kubernetes. This is the fight today, the holy war, I would say, between the Slurm researcher kind of mindset and the Kubernetes operations kind of mindset, which kind of comes very often in the community" . On-screen text also displays the word "kubernetes" during this segment . No other video in the searched set sh…

**VI agent (Sonnet 5)** — quality 0.75, P 1.00, R 1.00, facts 0.50, timestamps near a mention 0.62 (n=8), $0.057, 15 s
  
Judge: The answer correctly identifies Yangqing Jia's 'Evolution of System Designs' as the only video discussing Kubernetes and provides multiple specific timestamp ranges, with no invented videos. However, it frames the context as Kubernetes vs Slurm/SRE mindsets and etcd anecdotes rather than the ground-truth framing of Kubernetes as a choice versus a hand-managed sysadmin cluster (including the Oriol Vinyals aside), so the second key fact is not conveyed.
  
> Based on the search results, Kubernetes is substantively discussed in one video in the library. - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia - [00:43:26–00:45:59] The speaker contrasts Kubernetes with Slurm as competing approaches to managing GPU clusters for AI research, describing it as "the holy war... between the Slurm researcher kind of mindset and the Kubernetes operations kind of mindset," noting AI researchers often resist it while site-reliability engineers favor it . - [00:46:12–00:47:54] He notes that companies eventually seek "AI native platforms" that abstract away "the Kubernetes jargon" so researchers can focus on their models r…

**VI agent (Gemini 3.8 Flash)** — quality 0.75, P 1.00, R 1.00, facts 0.50, timestamps near a mention 1.00 (n=5), $0.067, 15 s
  
Judge: The answer correctly identifies the single relevant video (Yangqing Jia's 'Evolution of System Designs') and supplies specific timestamps, with no invented videos. However, it frames the Kubernetes mention around Slurm/SRE mindset, etcd, and locality rather than the ground-truth context of choosing Kubernetes versus a hand-managed sysadmin cluster (including the Oriol Vinyals aside), so the second key fact is not conveyed and some details may be misattributed.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia Timestamps: [00:43:13] – [00:44:23], [00:46:13], [01:09:41], [01:18:01] Yangqing Jia brings up Kubernetes to contrast the operational/SRE mindset with the Slurm-oriented workflow of AI researchers who do not want to manage pods and deployments , notes how AI-native platforms try to abstract away Kubernetes jargon , explains that traditional VM and Kubernetes abstractions break down in AI training where rack and switch physical locality is essential , and shares that one of his startup co-founders created etcd, the underlying database of Kubernetes . Videos: 1

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.50 (n=2), $0.158, 104 s
  
Judge: The answer correctly identifies the single relevant video (Yangqing Jia's 'Evolution of System Designs') with a specific timestamp and frames Kubernetes as one option for managing a compute cluster versus simpler alternatives. However, it omits the hand-managed/sysadmin-run cluster alternative and the Oriol Vinyals aside, and adds a quoted slide title and Slurm/MPI specifics that may be embellished; the leading 01:19:31 appears to be the video duration rather than a content timestamp.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia (01:19:31) [00:43:20] Under a slide titled "You shouldn't run on baremetal. Also, K8s is wrong.", Yangqing Jia discusses the divide between operations teams and researchers, explaining that while Kubernetes is an excellent abstraction for site reliability engineers (SREs), AI researchers dislike dealing with Kubernetes jargon (such as pods and deployments) and prefer simpler cluster setups like Slurm or MPI to run distributed PyTorch jobs. Videos: 1

## corpus-08: Which videos mention Notion, the company or its product?

*Kind: mention.* Three talks use 'notion' as an ordinary noun ('the notion that...'); claiming them is wrong.

Expected: How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust); Evals 101 — Doug Guthrie, Braintrust

**VI agent with library tools (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 0.83 (n=6), $0.053, 10 s
  
Judge: The answer names only two real catalog videos (6YdPI9YbjbI and bk0TmxoZlUY) with specific timestamps and no invented titles. It also asserts three other videos use "notion" only as a common noun without identifying them, which is unverifiable here, but the affirmative claims are plausible and well-anchored.
  
> Based on an exhaustive scan, the word "Notion" appears in five videos, but in three of them it's just the common noun ("the notion of..."), not the company. Only two videos actually reference Notion the company/product: - How to build world-class AI products — Sarah Sachs (AI lead @ Notion) & Carlos Esteban (Braintrust): [00:00:00], [00:02:53]. The talk is delivered by Notion's AI lead, who introduces Notion as a connected workspace product with slides describing its features and scale . - Evals 101 — Doug Guthrie, Braintrust: [00:02:28], [00:05:31], [00:37:11], [00:38:53]. Slides list Notion among customer logos, and the speaker references a Notion customer example for human-in-the-loop pra…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=5), $0.066, 23 s
  
Judge: The answer names two real catalog videos with specific timestamps and no invented entries. The Notion-hosted talk attribution is clearly correct; the Evals 101 mentions are plausible but the claim that Doug Guthrie discusses "Sarah from Notion" is unverified and could be a slight misattribution.
  
> How to build world-class AI products — Sarah Sachs (AI lead @ Notion) & Carlos Esteban (Braintrust): [00:00:17], [00:03:02] — Sarah Sachs, the AI lead at Notion, is introduced and describes Notion as a connected workspace while explaining how Notion builds, evaluates, and ships Notion AI features , . Evals 101 — Doug Guthrie, Braintrust: [00:02:28], [00:38:58], [00:45:24] — Notion is shown as a customer on-screen and cited by speaker Doug Guthrie when discussing how Sarah from Notion handles human-in-the-loop workflows and LLM-as-a-judge evaluations , , . Videos: 2

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.67, P 1.00, R 0.50, $0.053, 7 s
  
Missed: Evals 101 — Doug Guthrie, Braintrust
  
Judge: The answer identifies only one video (the Sarah Sachs/Notion talk), correctly mapped but justified solely by the title rather than any in-video evidence, and gives no timestamps. It admits it did not perform an exhaustive search, so coverage is likely incomplete, though no videos were invented or misattributed.
  
> I should use findmentions for this exhaustive query rather than the ranked search results shown, since search alone isn't reliable for "which videos mention X" questions. However, based on what's available, one video obviously mentions Notion prominently: "How to build world-class AI products — Sarah Sachs (AI lead @ Notion) & Carlos Esteban (Braintrust)" — the video's title itself names Notion as the speaker's company.

**VI agent (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=6), $0.135, 19 s
  
Judge: The answer names only real catalog videos (6YdPI9YbjbI and bk0TmxoZlUY) with specific, plausible timestamps for each claim. Attributions are coherent — the Notion AI lead talk and the Braintrust Evals 101 customer-logo slides — though it may be incomplete if other talks mention Notion in passing.
  
> Based on the searches, I have found two videos that explicitly mention Notion (the company/product). - How to build world-class AI products — Sarah Sachs (AI lead @ Notion) & Carlos Esteban (Braintrust) [00:00:16], [00:02:56], [00:06:34], [00:07:51]: This entire talk is by Notion's AI lead; she introduces Notion as "a connected workspace" with over 100 million users, and walks through the evolution of Notion AI (AI Writer, autofill, Q&A/RAG, AI connectors, "Notion AI for Work"), noting "Notion AI actually came out before ChatGPT" . - Evals 101 — Doug Guthrie, Braintrust [00:02:28], [00:05:31]: Notion is listed on a slide of companies Braintrust supports ("We support leading AI teams" — logos…

**VI agent (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=5), $0.124, 19 s
  
Judge: The answer names two real catalog videos with specific, plausible timestamps and no invented entries. Attributions fit each video's known content (Notion AI talk and Braintrust Evals 101 customer slides), though the exact slide timestamps in Evals 101 cannot be verified here.
  
> Here are the videos in the library that mention Notion, the company or its product: - How to build world-class AI products — Sarah Sachs (AI lead @ Notion) & Carlos Esteban (Braintrust) - [00:01:13] Carlos Esteban introduces Sarah Sachs as the lead of Notion AI and notes that Notion has been a Braintrust customer since the beginning . - [00:03:02] Sarah Sachs presents Notion as a connected workspace connecting project management, documents, and tools like Slack and Google Drive for over 100 million users . - [00:06:38] Sarah outlines the product evolution and features of Notion AI, such as the AI Writer, autofill, enterprise search, and Notion AI for Work . - Evals 101 — Doug Guthrie, Braint…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 0.50 (n=4), $0.172, 267 s
  
Judge: The answer names two real catalog videos with specific timestamps and no invented entries, though each entry oddly pairs a header duration-like timestamp with the cited moment. Attributions appear plausible—Notion is central to the Sarah Sachs talk and a plausible reference in the Braintrust Evals 101 session—but other potential mentions across the library may be missed.
  
> Evals 101 — Doug Guthrie, Braintrust (00:48:30) [00:39:03]: Doug Guthrie references Sarah from Notion, noting that she is a Braintrust customer and previously discussed how Notion approaches human-in-the-loop evaluations. How to build world-class AI products — Sarah Sachs (AI lead @ Notion) & Carlos Esteban (Braintrust) (01:43:45) [00:01:21]: Carlos introduces Sarah Sachs as the AI lead at Notion, who then gives a presentation detailing the development, launch, and evaluation of Notion AI products. Videos: 2

## corpus-09: Which talks mention StarCraft?

*Kind: mention.* 

Expected: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Multi-Agent AI by Noam Brown; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone; Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals

**VI agent with library tools (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=5), $0.049, 6 s
  
Judge: The answer names three real catalog videos (Vinyals, Noam Brown, Peter Stone) with specific timestamps and short supporting quotes, with no invented videos. All claims are plausibly attributed to StarCraft-relevant talks, though the exact accuracy of individual timestamps and the Peter Stone reference cannot be verified from the given information.
  
> - Agentic AI MOOC UC Berkeley CS294-196 F25 Multi-Agent Systems in Era of LLMs by Oriol Vinyals — [00:02:14], [00:02:22], [00:05:26]: The speaker discusses DeepMind's earlier RL agent-environment work "that did not have StarCraft but had agent environment," and later shows a benchmarks-of-complexity slide comparing Atari, Go, and StarCraft . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown — [00:00:28]: The speaker lists landmark multi-agent successes "AlphaGo, things like DOTA and StarCraft and also poker" as examples not yet matched by LLMs . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Autonomous Agents by Peter Stone — [00:09:22]: The speaker references…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=4), $0.028, 9 s
  
Judge: All three named videos map to real catalog entries (Vinyals, Brown, Stone) and each comes with a specific timestamp or range, so no invented videos or missing citations. Attributions are plausible for these speakers' topics, though the specific timestamp accuracy and the Stone claim cannot be verified from the answer alone.
  
> Agentic AI MOOC UC Berkeley CS294-196 F25 Multi-Agent Systems in Era of LLMs by Oriol Vinyals [00:01:41] – [00:02:23] Oriol Vinyals explains that his lecture connects lessons from AlphaStar to modern LLM agents, presenting an early DeepMind pitch slide on agent-environment interaction before StarCraft was incorporated . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown [00:00:28] Noam Brown points to StarCraft, alongside AlphaGo, Dota, and poker, as a prime example of AI breakthroughs driven by self-play that have not yet been fully realized in LLMs . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Autonomous Agents by Peter Stone [00:09:23] Peter Stone notes that …

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.50, P 1.00, R 0.33, timestamps near a mention 0.67 (n=6), $0.066, 15 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Multi-Agent AI by Noam Brown; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone
  
Judge: The answer names a single real catalog video (Oriol Vinyals' multi-agent talk) with specific timestamp ranges, which is plausible given AlphaStar/StarCraft II content. It explicitly rules out other search hits and admits non-exhaustive coverage, so no invented videos or obvious misattributions are present.
  
> Based on the available evidence, the clearest mentions of StarCraft in the library come from the Oriol Vinyals talk on multi-agent systems, which discusses DeepMind's AlphaStar/StarCraft II research: - Agentic AI MOOC UC Berkeley CS294-196 F25 Multi-Agent Systems in Era of LLMs by Oriol Vinyals [00:19:49–00:22:46] and [00:23:55–00:26:44]: The speaker explains the exploration challenge in StarCraft II reinforcement learning, arguing that pure RL alone fails and produces exploitative "all-in" worker-rush strategies, motivating the need for imitation learning from human replay data before RL fine-tuning . - Same talk, [00:48:51–00:51:38]: A chapter titled "Automated Battle.net Training" shows f…

**VI agent (Sonnet 5)** — quality 0.80, P 1.00, R 0.67, timestamps near a mention 0.50 (n=14), $0.116, 16 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone
  
Judge: The answer names two real catalog videos (Vinyals' multi-agent LLM talk and Noam Brown's Multi-Agent AI talk), both mapped correctly with abundant specific timestamps and plausible, well-detailed evidence. No invented videos or untimestamped claims appear, though the sheer number of timestamps for the Vinyals talk cannot be verified individually.
  
> Based on my searches, StarCraft is mentioned in two talks in the library. - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent Systems in Era of LLMs by Oriol Vinyals [00:00:00, 00:05:26, 00:14:07, 00:15:47, 00:19:55, 00:26:40, 00:30:04, 00:33:20, 00:46:17, 00:48:51, 01:00:31, 01:02:57, 01:08:33]. Vinyals, who led DeepMind's AlphaStar project, discusses StarCraft extensively throughout: the title slide is "AlphaStar revisited" ; he recounts why StarCraft was hard for AI and his path to leading the project ; he describes AlphaStar's 2019 BlizzCon matches against pro player Serral ; he explains the supervised+RL+league training pipeline and StarCraft's API/action space compared to LLM…

**VI agent (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=3), $0.156, 25 s
  
Judge: The answer names three real catalog videos, each with a specific timestamp, and all three speakers plausibly discuss StarCraft given their backgrounds (Noam Brown on self-play, Peter Stone on game-playing agents, Oriol Vinyals as the AlphaStar lead). No invented videos or missing timestamps, though attributions and exact times cannot be verified here.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown: [00:00:28] — Noam Brown highlights that self-play has powered major AI breakthroughs in games such as AlphaGo, Dota, StarCraft, and poker . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Autonomous Agents by Peter Stone: [00:09:23] — Peter Stone notes that AI systems have surpassed the best human players in games ranging from chess, Go, and poker to StarCraft . Agentic AI MOOC UC Berkeley CS294-196 F25 Multi-Agent Systems in Era of LLMs by Oriol Vinyals: [00:05:59] — Oriol Vinyals discusses DeepMind's roadmap of mastering games and explains how tackling StarCraft served as a milestone due to its sheer complexity .…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 0.94 (n=16), $0.054, 28 s
  
Judge: All three named videos map to real catalog entries (Noam Brown, Peter Stone, Oriol Vinyals) with specific timestamps provided, and the StarCraft/AlphaStar attribution to Vinyals is highly plausible. The Noam Brown entry gives two conflicting times (a header 00:54:49 and a cited 00:00:28), which is internally inconsistent, and the long timestamp list for Vinyals is lumped under one unverified summary claim.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown (00:54:49) [00:00:28] Noam Brown notes that self-play has been behind many major breakthroughs in AI for games, citing examples such as AlphaGo, Dota, StarCraft, and poker. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Autonomous Agents by Peter Stone [00:09:22] Peter Stone notes that the audience had already heard about StarCraft earlier in the course as an example of AI systems outperforming human experts at complex games. Agentic AI MOOC UC Berkeley CS294-196 F25 Multi-Agent Systems in Era of LLMs by Oriol Vinyals [00:02:18], [00:06:04], [00:06:40], [00:07:23], [00:10:10], [00:13:06], [00:15:47], [00:20:07], [0…

## corpus-10: Which videos mention Hugging Face?

*Kind: mention.* 

Expected: Strategies for LLM Evals (GuideLLM, lm-eval-harness, OpenAI Evals Workshop) — Taylor Jordan Smith; [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois
  
Acceptable, not required: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia

**VI agent with library tools (Sonnet 5)** — quality 0.91, P 0.83, R 1.00, timestamps near a mention 0.83 (n=12), $0.060, 12 s
  
Wrong: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | AI Agents to Automate Science by James Zou
  
Judge: All six named videos map to real catalog entries, each with specific timestamps and brief supporting evidence, so there are no invented videos or untimestamped mentions. Attributions look plausible for the speakers involved, though the Yann Dubois entry's second timestamp (1:45:11) is unusually late for a lecture and the James Zou diagram claim is the least verifiable.
  
> - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [00:00:59], [00:01:24] — Discusses working with the Hugging Face, Google, Meta, and Mistral teams and surpassing 10 million monthly downloads on Hugging Face . - VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS [00:35:20], [00:36:07], [00:37:56] — Walks through setting a Hugging Face token as an environment variable and hosting a model on the Hugging Face Hub . - Strategies for LLM Evals (GuideLLM, lm-eval-harness, OpenAI Evals Workshop) — Taylor Jordan Smith [00:18:19], [00:18:32] — Presenter tells attendees they'll need to grab a Hugging Face t…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 0.91, P 0.83, R 1.00, timestamps near a mention 1.00 (n=10), $0.123, 27 s
  
Wrong: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | AI Agents to Automate Science by James Zou
  
Judge: The answer names six catalog videos, all mapped to real entries, each with a specific timestamp range and a concrete description of the Hugging Face mention. No invented videos or missing timestamps are apparent, though the plausibility of some attributions (e.g., the Paper2Agent slide detail) cannot be verified here.
  
> The following 6 videos mention or display Hugging Face: - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [00:00:59] – [00:01:42] Daniel Han mentions collaborating with the Hugging Face team to fix bugs in open-source models, notes that Unsloth surpassed 10 million monthly downloads on Hugging Face, and later demonstrates datasets and model exports using Hugging Face . - VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS [00:36:56] – [00:37:37] Suman Debnath demonstrates loading a model and explains that you need a Hugging Face token to log in and download the ColPali model from Hugging Face . - A…

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.57, P 1.00, R 0.40, timestamps near a mention 1.00 (n=4), $0.064, 14 s
  
Missed: Strategies for LLM Evals (GuideLLM, lm-eval-harness, OpenAI Evals Workshop) — Taylor Jordan Smith; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois
  
Judge: The answer names two real catalog videos with specific timestamps and explicitly excludes several others as false positives, so no invented videos appear. It is appropriately cautious but self-admittedly non-exhaustive, so coverage may be incomplete; attributions given appear plausible and precise.
  
> Based on the search results, I found clear evidence of "Hugging Face" being mentioned in the following videos: - VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS — [00:37:09], [00:37:37], [00:52:59]: The presenter walks through loading and logging into a Hugging Face account, checking the Hugging Face token, and warns against hardcoding tokens (should use an .env file); later "Hugging Face" appears as on-screen text while building a multimodal agentic RAG system . - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han — [00:02:37]: Daniel Han explains Unsloth's practice of uploading dynamic quantized…

**VI agent (Sonnet 5)** — quality 0.73, P 0.67, R 0.80, timestamps near a mention 0.57 (n=14), $0.601, 70 s
  
Wrong: Full Workshop: Realtime Voice AI — Mark Backman, Daily; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | AI Agents to Automate Science by James Zou
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois
  
Judge: The answer names seven real catalog videos, all with specific timestamps, and no invented videos appear. Some attributions are thin or possibly speculative (the James Zou Paper2Agent Hugging Face reference and the Daily voice AI 'smol' logo mention read as weakly supported), and one timestamp for the Ishan Anand talk was garbled in transcription.
  
> Based on my search across the video library, the following videos mention Hugging Face: - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [00:02:37], [00:07:52], [02:00:39], [02:22:38] — Daniel Han discusses Unsloth's practice of uploading dynamic quantized and bug-fixed models to Hugging Face, and shows how to export fine-tuned models to Hugging Face and use Hugging Face's Open R1 Math dataset for training . - VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS [00:35:20], [00:39:12], [01:02:59] — The presenter logs into a Hugging Face account with a token, loads a ColPali model and pre-processor …

**VI agent (Gemini 3.8 Flash)** — quality 0.91, P 0.83, R 1.00, timestamps near a mention 0.90 (n=10), $0.149, 33 s
  
Wrong: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | AI Agents to Automate Science by James Zou
  
Judge: All seven cited videos map to real catalog entries, each with specific timestamps and plausible Hugging Face-related evidence, and no videos were invented. Some timestamps appear derived from citation offsets rather than the stated bracket times (e.g., the AWS and vanilla-JS talks), so exact placement may be slightly off, but attributions to speakers and topics look internally consistent.
  
> [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [00:00:59], [00:01:41] Daniel Han mentions collaborating with the Hugging Face team to fix model bugs and highlights that Unsloth surpassed 10 million monthly downloads on Hugging Face . VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS [00:36:56], [00:37:37], [00:39:36] Suman Debnath explains how to log into Hugging Face using a token and load vision-language models and processors hosted on the Hugging Face Hub [[cite:01M29BFEMHGPPDX3PJRNTKG0Q1:2216-2263], [cite:01M29BFEMHGPPDX3PJRNTKG0Q1:2376-2385]]. Strategies for LLM Evals (GuideLLM, lm-eval-har…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=9), $0.823, 107 s
  
Judge: The answer names five catalog videos, all correctly mapped, each with specific timestamps and plausible Hugging Face-related evidence, with no invented videos. Attributions appear coherent with the speakers and topics, though the summary count of 5 undercounts nothing and coverage may be incomplete relative to other library talks that likely reference Hugging Face.
  
> [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han (02:42:27) [00:01:00]: Daniel Han mentions working directly with Hugging Face, Google, Meta, and Mistral teams to fix bugs in open-source models. [00:01:50]: He notes that Unsloth surpassed 10 million monthly downloads on Hugging Face. [00:02:37]: He explains that they upload fixed models and small quantizations to their Hugging Face page. [00:12:40]: He refers to model naming conventions (like instruct, chat, PT, and base) that users will encounter when looking at models on Hugging Face. [02:01:20]: While discussing top open-source model serving libraries like vLLM and SGLang, he notes that Huggin…

## corpus-11: Which speakers mention Kaggle?

*Kind: mention.* 

Expected: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals

**VI agent with library tools (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=4), $0.047, 6 s
  
Judge: The answer names two real catalog videos with specific timestamps and plausible attributions (Daniel Han on Kaggle's free GPU hours, Vinyals on Kaggle Game Arena), with no invented videos. All claims carry timestamps, though the list may be incomplete relative to other speakers who could mention Kaggle.
  
> - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han — [00:02:09], [02:01:44], [02:28:08]: Daniel Han repeatedly mentions that Kaggle offers 30 free hours of GPU access and free notebooks (alongside Google Colab) for training/fine-tuning . - Agentic AI MOOC UC Berkeley CS294-196 F25 Multi-Agent Systems in Era of LLMs by Oriol Vinyals — [00:56:30]: Oriol Vinyals mentions a "Game Arena" hosted on Kaggle as a popular LLM evaluation domain . Videos: 2

**VI agent with library tools (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=4), $0.048, 17 s
  
Judge: The answer names two real catalog videos with specific timestamps and plausible, well-attributed Kaggle references (Daniel Han's free GPU hours, Vinyals' Kaggle Game Arena). No invented videos appear, though the claims are narrow and may omit other speakers who mention Kaggle.
  
> The following speakers mention Kaggle: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [00:02:09], [02:01:45], [02:28:10]: Daniel Han repeatedly encourages attendees to take advantage of Kaggle offering 30 hours of free GPU access per week and highlights that Unsloth provides free notebooks for Kaggle , , . Agentic AI MOOC UC Berkeley CS294-196 F25 Multi-Agent Systems in Era of LLMs by Oriol Vinyals [00:56:30]: Oriol Vinyals explains how games are increasingly repurposed as LLM evaluation environments and mentions that there is a very popular "Game Arena" on Kaggle . Videos: 2

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.00, P 0.00, R 0.00, $0.056, 9 s
  
Missed: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals
  
Judge: The answer names no library videos and explicitly concludes that no video contains a confirmed mention of Kaggle, so there are no hallucinated videos or misattributions. However, it is a non-answer that hedges about incomplete search coverage and provides no timestamps or speaker attributions.
  
> Based on the searches performed, I could not find clear evidence of any speaker explicitly mentioning "Kaggle" in the transcripts, on-screen text, or descriptions across the indexed videos. The search results returned only loosely related hits (e.g., general references to "video," data counting, or benchmarks) with no direct match to the term "Kaggle" itself. To confirm definitively, an exhaustive findmentions scan for "Kaggle" (and common misspellings) would be needed, which was not available in this turn. Based on the evidence gathered so far, no video in the library shows a confirmed mention of Kaggle. Videos: 0

**VI agent (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 0.80 (n=5), $0.050, 10 s
  
Judge: The answer names two real catalog videos (Daniel Han's workshop and Oriol Vinyals' MOOC lecture) with plausible speaker attributions and timestamps. However, the first entry's timestamp is messy and self-contradictory ("00:08:08 - 00:00:41... actually 02:28:08"), showing uncertainty in localization, and no invented videos appear.
  
> Based on the search results, two videos actually mention Kaggle explicitly: - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han — [00:08:08 - 00:00:41... actually 02:28:08] — [02:28:08] Daniel Han tells attendees to use Kaggle's free GPUs for running large models, repeating "Kaggle" multiple times while pointing it out on screen . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent Systems in Era of LLMs by Oriol Vinyals — [00:56:37] Oriol Vinyals notes there is a "Game Arena" on Kaggle that is popular for evaluating LLMs playing games, mentioning some models even try to complete Pokémon . Videos: 2

**VI agent (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=4), $0.058, 16 s
  
Judge: The answer names two real catalog videos with specific timestamps and plausible Kaggle-related evidence, with no invented videos. Attributions appear consistent with each speaker's topic, though the Vinyals claim rests on a single timestamp without a supporting citation, so its accuracy is less verifiable.
  
> [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han Timestamps: [00:02:09], [02:01:45], [02:28:14] Speaker Daniel Han highlights that Kaggle offers 30 hours of free GPU compute per week without restrictions and mentions Unsloth notebooks available on Kaggle [[cite:01M2920GDE5E53XCVRSC3R8SPA:129-148], [cite:01M2920GDE5E53XCVRSC3R8SPA:7303-7323], [cite:01M2920GDE5E53XCVRSC3R8SPA:8888-8925]]. Agentic AI MOOC UC Berkeley CS294-196 F25 Multi-Agent Systems in Era of LLMs by Oriol Vinyals Timestamp: [00:56:30] Speaker Oriol Vinyals discusses using games as evaluation benchmarks for LLMs, citing a popular game arena hosted on Kaggle . Videos: 2

**Gemini agentic video (3×10 + merge)** — quality 0.67, P 1.00, R 0.50, timestamps near a mention 0.80 (n=5), $0.065, 35 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals
  
Judge: The answer cites a single real catalog video (Daniel Han's workshop) with specific timestamps and a plausible Kaggle free-GPU claim, so no invented videos appear. However, it lists only one speaker, which may under-cover other talks mentioning Kaggle, and the leading (02:42:27) appears to be a duration rather than a citation point, making the timestamp set slightly ambiguous.
  
> [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han (02:42:27): [00:02:12] (also mentioned at [00:02:29], [02:01:51], and [02:28:40]) — Daniel Han mentions that Kaggle provides 30 hours of free GPU resources per week and encourages the audience to take advantage of their free notebooks on Kaggle. Videos: 1

## corpus-12: In which videos is GRPO (the RL algorithm) mentioned?

*Kind: mention.* 

Expected: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Post-Training Verifiable Agents by Jiantao Jiao; [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois

**VI agent with library tools (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=5), $0.269, 36 s
  
Judge: The answer names four catalog videos, all real, each with specific timestamps and quoted evidence, plus an explicitly excluded mention of a 'spec.grpo.md' filename in the Vibe Coding workshop (correctly not counted as a claim). Attributions appear plausible and consistent with each speaker's topic, though correctness of the exact timestamps and of the borderline Ishan Anand mention cannot be verified here.
  
> Based on an exhaustive search of the library, GRPO (Group Relative Policy Optimization) is discussed as the RL algorithm in 4 videos: - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [00:23:39] Daniel Han explains that GRPO is "the algorithm behind DeepSeek R1" and that it smartly deletes the value model used in PPO to save compute, making training more efficient . The talk also has an extended slide/diagram walkthrough of GRPO starting around [00:22:56] . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Post-Training Verifiable Agents by Jiantao Jiao [00:03:39] Jiao mentions that after generating multiple responses and grading them with a rew…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=4), $0.130, 22 s
  
Judge: The answer names four real catalog videos, each with a specific timestamp and a plausible GRPO-related description, with no invented titles. Attributions appear consistent with the speakers and topics, though the accuracy of each timestamp and whether other library videos also mention GRPO cannot be verified here.
  
> Here are the videos in the library where the RL algorithm GRPO (Group Relative Policy Optimization) is mentioned: - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han - [00:23:39] Daniel Han explains that GRPO, the algorithm behind DeepSeek-R1, modifies PPO by removing the value model to save compute and parameters . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Post-Training Verifiable Agents by Jiantao Jiao - [00:06:20] Jiantao Jiao explains that post-training algorithms such as supervised fine-tuning, PPO, and GRPO are used to train agentic models to maximize correctness . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview…

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.00, P 0.00, R 0.00, $0.059, 12 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Post-Training Verifiable Agents by Jiantao Jiao; [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois
  
Judge: The answer explicitly declines to identify any video, stating it found no confirmed GRPO mentions, though it speculates (without citation or timestamps) that Daniel Han's RL workshop, the OpenAI RFT/DPO/SFT talk, and Weizhu Chen's lecture might plausibly discuss it. No invented videos or misattributions are asserted, but the response provides no usable evidence or timestamps, so it is effectively a non-answer.
  
> Based on the searches run, I was not able to locate clear, direct evidence of "GRPO" (Group Relative Policy Optimization) being mentioned by name in any indexed video. The search results returned only tangential/irrelevant hits (e.g., ComfyUI OCR text, ElevenLabs voice cloning, VoiceVision RAG frames, Dawn Song's safety lecture, Peter Stone's autonomous agents lecture) that do not actually reference GRPO or the RL algorithm. Given the nature of the library (it contains several RL/fine-tuning-heavy talks, e.g., Daniel Han's workshop on Reinforcement Learning, Kernels, Reasoning, Quantization & Agents, and OpenAI's RFT/DPO/SFT fine-tuning talk, and Weizhu Chen's Training Agentic Models lecture…

**VI agent (Sonnet 5)** — quality 0.86, P 1.00, R 0.75, timestamps near a mention 0.69 (n=16), $0.451, 39 s
  
Missed: How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand
  
Judge: The answer names three real catalog videos with specific, plausible timestamps and coherent descriptions of GRPO content, with no invented videos. Timestamps for the Daniel Han workshop are numerous and partially overlapping, which is slightly noisy but not clearly erroneous; all attributions appear consistent with the speakers' topics.
  
> Based on my searches, GRPO (Group Relative Policy Optimization) is discussed substantively in three videos in the library: - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han — [01:16:16], [01:16:03], [01:15:29–01:19:32], [01:24:14–01:27:32], [01:26:47–01:28:36], [02:12:22–02:20:39], [02:29:04–02:33:00] — Daniel Han gives an extended deep dive on GRPO: he explains the transition from RLHF→PPO→GRPO, showing how GRPO removes the value model and reward model in favor of a simple reward function and rollout/z-score-based advantage estimation, why it's called "group relative" (multiple questions/rollouts per group), and walks through a live Unsloth Col…

**VI agent (Gemini 3.8 Flash)** — quality 0.86, P 1.00, R 0.75, timestamps near a mention 1.00 (n=14), $0.143, 22 s
  
Missed: How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand
  
Judge: The answer names three catalog videos (Daniel Han's RL workshop, Jiantao Jiao's post-training lecture, and Yann Dubois's LLM agents overview), all real entries, each with specific timestamp ranges and no invented titles. Attributions are plausible for these RL-focused talks, though the specific timestamps and some described details (e.g., clipping mechanics at 1:02:43) cannot be verified here and the list may omit other mentions such as in fine-tuning talks.
  
> Here are the videos in the library that mention GRPO (Group Relative Policy Optimization): - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han - [00:23:39 - 00:23:55] : Daniel Han introduces GRPO as the algorithm behind DeepSeek-R1 that smartly eliminates the value model to save compute and parameters. - [01:16:17 - 01:18:25] : He details the mathematical differences between PPO and GRPO, explaining how GRPO computes relative advantage scores across sampled candidate outputs per prompt without a separate critic model. - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Post-Training Verifiable Agents by Jiantao Jiao - [00:03:39 - 00:03:45] : Jiantao…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=9), $0.083, 28 s
  
Judge: The answer names four catalog videos, all real, each with specific timestamps and plausible speaker-appropriate descriptions of GRPO discussion. No invented videos or untimestamped claims appear, though the exactness of some timestamps (e.g., the "GRPO Elite" paper reference) and whether other library talks also mention GRPO cannot be verified here.
  
> [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [00:23:45] Daniel Han introduces GRPO as the algorithm behind DeepSeek-R1, explaining that it improves efficiency by removing the value model used in PPO. [01:16:25] Daniel Han explains the mechanics of GRPO (Group Relative Policy Optimization), detailing how it calculates group statistics from multiple rollout inferences without needing a separate value model. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Post-Training Verifiable Agents by Jiantao Jiao [00:03:45] Jiantao Jiao notes that one can use reinforcement learning algorithms such as PPO and GRPO to train models that maximize preference. […

## corpus-13: Show me the moments where speakers talk about improving RAG (retrieval-augmented generation): better or multimodal retrieval, or evaluating retrieval quality.

*Kind: topic.* Other talks mention RAG in passing (RAG poisoning in Dawn Song's lecture, 'RAG is falling out of favor' in Yangqing Jia's) and are acceptable but not required.

Expected: Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB; How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust); Strategies for LLM Evals (GuideLLM, lm-eval-harness, OpenAI Evals Workshop) — Taylor Jordan Smith; VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS
  
Acceptable, not required: Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Agentic AI Safety & Security by Dawn Song; RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI; Building Code First AI Agents with Azure AI Agent Service — Cedric Vidal, Microsoft; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; Evals 101 — Doug Guthrie, Braintrust; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor; From Mixture of Experts to Mixture of Agents with Super Fast Inference - Daniel Kim & Daria Soboleva; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia

Key facts:

- Suman Debnath (AWS, VoiceVision RAG) describes multimodal RAG over documents using image/page embeddings with late-interaction (ColPali-style) token embeddings stored in a vector database.
- Apoorva Joshi (MongoDB) covers multimodal RAG: embedding page screenshots with a multimodal embedding model into a vector store, and the difficulty of chunking mixed-modality documents.
- Sarah Sachs (Notion) explains evaluating retrieval separately from generation, with its own evaluation framework.
- Taylor Jordan Smith's evals workshop discusses setting up evaluation for a RAG system.

**VI agent with library tools (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.72 (n=18), $0.352, 50 s
  
Judge: All eight cited videos map to real catalog entries with specific timestamp ranges, and the four key facts are at least substantively touched, though the Suman Debnath entry omits the ColPali-style late-interaction/vector-DB mechanism and the Sarah Sachs entry frames her point mostly around index freshness rather than a distinct retrieval evaluation framework. The extra videos (Evals 101, Aman Khan, Cedric Vidal, Ilan Bigio) are plausible but unverified additions that could dilute precision; no invented videos and no untimestamped citations.
  
> Based on the library, several talks discuss improving RAG, multimodal retrieval, or evaluating retrieval quality: - Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB: [00:19:23–00:22:35] and [00:22:35–00:24:00] The speaker walks through the challenges of preparing mixed-modality (text/image/table) documents for retrieval, explains how CLIP-style dual-encoder embeddings cause a "modality gap," and shows how newer vision-language-model (VLM) embeddings unify text and image representations to get "better retrieval quality" by avoiding separate encoders . - VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS: [00:49:16 & 01:03:14]…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 0.80, P 1.00, R 0.75, facts 0.75, timestamps near a mention 0.80 (n=10), $0.151, 35 s
  
Missed: Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB
  
Judge: The answer correctly identifies three library videos with plausible timestamp ranges and no invented entries, covering Debnath's ColPali multimodal retrieval, Smith's RAG eval, and Sachs's separated retrieval evaluation. It omits Apoorva Joshi's MongoDB multimodal RAG segment, and does not mention the late-interaction token embedding detail beyond naming ColPali.
  
> VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS [00:07:22]–[00:08:29], [00:18:31]–[00:20:53], [00:35:20]–[00:37:45] Suman Debnath demonstrates improving traditional document RAG by adopting multimodal, vision-based retrieval using the ColPali model with a vector database, preserving visual structure, charts, and tables without relying on error-prone text extraction or OCR pipelines [[cite:01M29BFEMHGPPDX3PJRNTKG0Q1:442-509], [cite:01M29BFEMHGPPDX3PJRNTKG0Q1:1111-1253], [cite:01M29BFEMHGPPDX3PJRNTKG0Q1:2120-2265]]. Strategies for LLM Evals (GuideLLM, lm-eval-harness, OpenAI Evals Workshop) — Taylor Jordan Smith [00:11:33]–[00:13:00] Taylor J…

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.80, P 1.00, R 0.75, facts 0.75, timestamps near a mention 0.71 (n=7), $0.044, 14 s
  
Missed: Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB
  
Judge: All four cited videos exist in the catalog with specific timestamps, and three of the four key facts (Debnath's multimodal document RAG, Sachs's separate retrieval evaluation, Smith's RAG eval setup) are conveyed, though the ColPali-style late-interaction detail is omitted. The answer misses Apoorva Joshi's multimodal RAG segment entirely and adds an extra Yangqing Jia claim about coarse/fine ranking that is outside the ground-truth set.
  
> Based on the search results, here are the moments across the library where speakers discuss improving RAG, multimodal retrieval, or evaluating retrieval quality: - VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS [00:01:01] The speaker introduces the talk's focus on vision-based document retrieval research, wrapped with an agent framework . [00:07:22]–[00:10:13] He explains multimodal RAG architecture in depth: documents are decomposed into images, text, and tables, embedded, stored in a vector database, and retrieved via the same embedding model when a query comes in . - Strategies for LLM Evals (GuideLLM, lm-eval-harness, OpenAI Evals Work…

**VI agent (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.78 (n=9), $0.255, 33 s
  
Judge: The answer identifies all four expected videos with correct speaker attributions and specific timestamps, with no invented videos or untimestamped mentions. Content descriptions (ColPali/Qdrant late-interaction, multimodal chunking difficulty, separate retrieval evals, RAG eval setup) align with the key facts, though the Apoorva Joshi entry emphasizes chunking/retrieval mechanisms more than page-screenshot embedding.
  
> Based on my research, here are the moments across the video library where speakers discuss improving RAG—better/multimodal retrieval, or evaluating retrieval quality: - VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS [00:07:22] – Introduces "Multimodal RAG" architecture, explaining how documents are decomposed into images, text, and tables, embedded, and stored in a vector database for retrieval . [00:35:20] – Walks through loading the ColPali multimodal document retrieval model and setting up Qdrant with a multi-vector "MaxSim" comparator for late-interaction retrieval of page images . [00:47:47] – Demonstrates the "Retrieval (Query Time)"…

**VI agent (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.75 (n=12), $0.092, 21 s
  
Judge: All five named videos exist in the catalog, each with specific timestamp ranges, and the four key facts are covered — though the Apoorva Joshi entry emphasizes chunking and parent-document retrieval without explicitly mentioning embedding page screenshots with a multimodal embedding model. The extra Evals 101 (Doug Guthrie) citation is plausible and on-topic rather than an invented video, so no hallucinated sources appear.
  
> VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS [00:16:25] - [00:18:31] / [00:36:16] - [00:39:12]: The speaker describes how decomposing documents into raw text and separate images breaks context, advocating instead for multimodal vision-based retrieval using models like ColPali to embed and retrieve full document page images directly [[cite:01M29BFEMHGPPDX3PJRNTKG0Q1:1085-1111], [cite:01M29BFEMHGPPDX3PJRNTKG0Q1:2176-2242]]. Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB [00:19:35] - [00:22:00]: The speaker explains that chunking mixed-modality documents causes critical context loss across boundaries, and discusses impr…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.57 (n=14), $0.098, 34 s
  
Judge: All seven named videos map to real catalog entries with specific timestamps, and all four key facts are covered (Debnath's ColPali late-interaction multimodal RAG, Joshi's page-screenshot embeddings and chunking difficulty, Sachs's separate retrieval evaluation, and Smith's RAG eval setup). Two extra attributions (Ilan Bigio on HyDE fine-tuning and Yangqing Jia on multi-stage ranking) are plausible but unverified and may be misattributed embellishments; no invented videos appear.
  
> Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB [00:22:50] Apoorva Joshi explains how switching from traditional chunking and CLIP-based embeddings to unified vision-language model (VLM) embeddings on page screenshots resolves modality gaps and preserves layout context, providing significantly better retrieval quality for multimodal RAG. Evals 101 — Doug Guthrie, Braintrust [00:34:08] Doug Guthrie explains how to evaluate retrieval quality in a RAG workflow by applying targeted online scorers to individual tool-call spans to assess whether retrieved context is relevant to the user query. RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI [01:14:48] Ilan Bigio dis…

## corpus-14: Which talks explain reward models or reward functions in RL post-training, and what do they say about them?

*Kind: topic.* 

Expected: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Post-Training Verifiable Agents by Jiantao Jiao; [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen
  
Acceptable, not required: RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone

Key facts:

- Jiantao Jiao distinguishes generic preference reward models from task-completion (verifiable) rewards used to post-train agents.
- Daniel Han explains that GRPO removes the value model and the reward model, replacing them with reward functions.
- Yann Dubois contrasts rule-based (verifiable) rewards with reward models trained to predict human preferences, including LLM-as-a-judge rewards, in RLHF.
- Weizhu Chen contrasts RL with a verifiable signal against RLHF, where training a reward model is hard.

**VI agent with library tools (Sonnet 5)** — quality 0.71, P 0.50, R 1.00, facts 0.75, timestamps near a mention 0.64 (n=14), $0.277, 58 s
  
Wrong: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Agentic AI Safety & Security by Dawn Song; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Multi-Agent AI by Noam Brown; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals
  
Judge: All ten cited videos map to real catalog entries with specific timestamps, and the Jiantao Jiao, Daniel Han and Weizhu Chen points match the expected substance closely. The Yann Dubois entry only gives the human-preference reward-model definition plus reward hacking and omits the contrast with rule-based/verifiable and LLM-as-judge rewards; the remaining six entries (Bigio, Brown, Song, Anand, Stone, Vinyals) are plausible but peripheral additions whose details cannot be verified here.
  
> Based on transcripts, OCR and descriptions across the library, here are the talks that explain reward models/reward functions in RL post-training: - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [00:17:11], [00:21:04], [00:48:10], [01:15:54] — Explains the RL loop (state/action/reward) and RLHF, then walks through how GRPO/RLVR strips away the neural value model and reward model entirely, replacing them with a simple "reward function" (ground truth check, LLM-as-judge, regex/format check, code execution) that assigns +1/0 scores . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Post-Training Verifiable Agents by Jiantao Jiao [00:01:22]–[00:0…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 0.75, P 0.75, R 0.75, facts 0.75, timestamps near a mention 1.00 (n=12), $0.187, 32 s
  
Wrong: How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen
  
Judge: All five cited videos exist in the catalog with specific timestamp ranges, and the Han, Jiao, and Dubois explanations match the expected substance (though Dubois's LLM-as-a-judge reward point is omitted). Weizhu Chen's contrast of verifiable-signal RL with RLHF's hard-to-train reward model is entirely missing, and two extra talks (Ishan Anand, Ilan Bigio) are added while Peter Stone and Dawn Song are flagged as unverified due to tool budget.
  
> Here are the talks that explain reward models or reward functions in RL post-training and what they say about them: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [01:16:30]–[01:16:47] / [02:08:55]–[02:11:00]: Daniel Han explains that in modern reinforcement learning with verifiable rewards (RLVR) and GRPO, the traditional neural reward model and value model are eliminated and replaced with explicit, programmatic reward functions (e.g., regex checking formatting tags and exact mathematical correctness scoring) [[cite:01M2920GDE5E53XCVRSC3R8SPA:4590-4663], [cite:01M2920GDE5E53XCVRSC3R8SPA:6633-6648], [cite:01M2920GDE5E53XCVRSC3R8SPA:7734-7835]].…

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.50, P 1.00, R 1.00, facts 0.00, timestamps near a mention 0.67 (n=6), $0.073, 18 s
  
Judge: The answer names the four correct catalog videos with specific timestamps and no invented sources, so attribution is sound. However, it misses the substance of every key fact: it omits Han's point that GRPO drops the value and reward models in favor of reward functions, Dubois's contrast with preference-trained reward models/LLM-as-judge, Chen's RLHF-reward-model-difficulty contrast, and Jiao's preference-vs-task-completion reward distinction.
  
> Based on my searches, four talks in the library directly discuss reward models/functions used in RL post-training: - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han — [00:28:39] and [00:38:46]: Daniel Han explains that RL training (via GRPO) is reward maximization rather than next-token prediction, and that "the trick of RL is you just need a reward function... you don't need the chain of thought, just lots of questions" with an answer to check against . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Training Agentic Models by Weizhu Chen — [00:12:24]–[00:14:57]: Weizhu Chen discusses RLVR (reinforcement learning with verified rewards), showi…

**VI agent (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.67 (n=15), $0.268, 42 s
  
Judge: All five cited videos map to real catalog entries with specific timestamps, and each of the four key facts is conveyed, though the Daniel Han entry omits that GRPO also removes the value model and the Dubois entry omits LLM-as-judge rewards explicitly. The extra Ilan Bigio (RFT graders) attribution is plausible and on-topic rather than hallucinated, so the answer appears well-grounded overall.
  
> Based on my search across the library, here are the talks that substantively explain reward models or reward functions in RL post‑training: - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [00:21:04] Han introduces the RLHF diagram, explaining that a reward function/model converts an answer into a numeric score that the model is trained to maximize . [00:48:10] He explains how RLVR (GRPO) deletes the neural reward model entirely and replaces it with a simple ground-truth reward function (e.g., +1 correct/0 incorrect) . [01:13:37] He warns that maximizing reward directly can cause "reward hacking" and explains why PPO's KL-divergence term is add…

**VI agent (Gemini 3.8 Flash)** — quality 0.88, P 1.00, R 1.00, facts 0.75, timestamps near a mention 0.88 (n=8), $0.167, 41 s
  
Judge: All four named videos exist in the catalog and are correctly mapped with specific timestamp ranges, and the Jiao, Han, and Dubois characterizations match the key facts (though Han's GRPO point omits removal of the value model and Dubois's LLM-as-a-judge reward is attributed instead to Han). The Weizhu Chen entry drifts from the expected fact—it describes graders and reward hacking rather than the contrast between verifiable RL signals and the difficulty of training a reward model in RLHF.
  
> [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [00:48:08] - [00:49:25] Daniel Han explains that while traditional RLHF relies on a neural reward model to score outputs, reinforcement learning with verifiable rewards (RLVR) and GRPO replace the reward model entirely with an explicit reward function (such as ground-truth verification, regex formatting checks, code execution, or LLM-as-a-judge) that outputs a scalar value . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Post-Training Verifiable Agents by Jiantao Jiao [00:01:45] - [00:04:15] Jiantao Jiao explains how traditional RL post-training trains a neural reward model from human preference p…

**Gemini agentic video (3×10 + merge)** — quality 0.82, P 0.80, R 1.00, facts 0.75, timestamps near a mention 0.55 (n=20), $0.128, 109 s
  
Wrong: How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand
  
Judge: All six cited videos exist in the catalog and each carries specific timestamps; the Jiao, Han, and Dubois attributions match the expected content well, though Han's point about GRPO also dropping the value model is omitted. The Weizhu Chen entry describes graders and reward hacking but misses his verifiable-signal-versus-hard-to-train-reward-model contrast, and the Ilan Bigio and Ishan Anand entries are plausible but extra additions beyond the expected set.
  
> [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han (02:42:27) [00:22:50], [00:48:10], [01:30:40] Daniel Han explains that traditional RLHF feeds model outputs into a learned neural reward model, whereas reinforcement learning with verifiable rewards (RLVR) and GRPO replace the reward model with deterministic reward functions—such as regex formatting rules, ground-truth math checks, distance-based scoring, code execution tests, or LLM judges—to reward desired answers and penalize incorrect ones. RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI (01:46:14) [00:07:10], [00:46:10], [00:48:00], [01:11:20] Ilan Bigio explains that reinforcement…

## corpus-15: Where in the library is Mixture of Experts (MoE) explained, and how does each speaker describe it?

*Kind: topic.* 

Expected: How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois; From Mixture of Experts to Mixture of Agents with Super Fast Inference - Daniel Kim & Daria Soboleva; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia
  
Acceptable, not required: Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB; Introduction to LLM serving with SGLang - Philip Kiely and Yineng Zhang, Baseten; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Agentic AI Safety & Security by Dawn Song; [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments; RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI; [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han

Key facts:

- Daniel Kim and Daria Soboleva (Cerebras) explain MoE as the architecture their 'mixture of agents' pattern imitates, with agents replacing experts.
- Ishan Anand describes MoE as one of the biggest architectural changes since GPT-2, in answer to an audience question.
- Yann Dubois names transformers with mixture of experts as the dominant open-source architecture.
- Yangqing Jia presents MoE as sparsely activated models that improve efficiency when dense models plateaued.

**VI agent with library tools (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=11), $0.337, 42 s
  
Judge: All five named videos map to real catalog entries with specific timestamps, and all four key facts are covered with correct speaker attributions. The extra Daniel Han quantization mention is plausible and appropriately framed as context rather than a core explanation; no invented videos or untimestamped claims.
  
> Now I have enough detail across the relevant videos. Let me compile the answer. Mixture of Experts (MoE) across the library - From Mixture of Experts to Mixture of Agents with Super Fast Inference — Daniel Kim & Daria Soboleva [00:00:45], [00:07:04]-[00:10:18]: This is the deepest and most dedicated explanation. The Cerebras researcher explains that instead of one monolithic feed-forward network, MoE splits it into multiple specialized "expert" feed-forward networks, each good at a different task (e.g., one for math, one for biology), and adds a "router" network that decides which expert(s) to activate per token, letting the model scale parameters/capacity without increasing inference comput…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 0.62, P 1.00, R 1.00, facts 0.25, timestamps near a mention 1.00 (n=8), $0.138, 26 s
  
Judge: The answer identifies all four correct library videos with precise, plausible timestamp ranges and no invented sources. However, it substitutes generic technical descriptions of MoE for the speaker-specific framings in the key facts — missing the mixture-of-agents analogy (Kim/Soboleva), the 'biggest change since GPT-2' audience-question framing (Anand), and the 'dominant open-source architecture' point (Dubois) — with only the Yangqing Jia sparsity/efficiency characterization matching.
  
> From Mixture of Experts to Mixture of Agents with Super Fast Inference - Daniel Kim & Daria Soboleva [00:08:40]–[00:09:52] Daria Soboleva explains that Mixture of Experts replaces a single monolithic feed-forward network with multiple specialized "expert" feed-forward networks paired with a router network that assigns each token to specific experts, allowing models to scale in capacity and parameters without increasing inference compute time . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois [01:55:56]–[01:57:06] Yann Dubois describes Mixture of Experts as a system optimization utilizing architectural sparsity, where each data point only activates a subset o…

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.33, P 1.00, R 0.50, facts 0.00, timestamps near a mention 1.00 (n=2), $0.062, 13 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia
  
Judge: The answer names two real library videos (Cerebras MoE-to-MoA talk and Ishan Anand's GPT-in-JS talk) with specific timestamps and no invented videos, and correctly notes the Ishan Anand explanation came from an audience question. However, it misses the 'agents replacing experts' framing, the 'biggest architectural change since GPT-2' claim, and omits the Yann Dubois and Yangqing Jia Berkeley lectures entirely, instead asserting no Berkeley lecture evidence was found.
  
> Based on the available evidence, Mixture of Experts (MoE) is explicitly explained in two videos in the library: - From Mixture of Experts to Mixture of Agents with Super Fast Inference — Daniel Kim & Daria Soboleva [00:08:39] The speaker explains that MoE architecture replaces a single feed-forward network with multiple specialized "expert" networks plus a router, allowing model scaling without a proportional increase in inference compute, and notes it is now the industry standard used by OpenAI, Anthropic, and others . - How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS — Ishan Anand [01:39:20] During the Q&A, the speaker explains MoE models in response to an audience question, des…

**VI agent (Sonnet 5)** — quality 0.55, P 1.00, R 0.75, facts 0.25, timestamps near a mention 1.00 (n=6), $0.229, 26 s
  
Missed: How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand
  
Judge: All four cited videos are real catalog entries with specific timestamps, and no videos were invented; the Yangqing Jia sparsity/efficiency point is accurately captured. However, the answer misses Ishan Anand's MoE discussion entirely, omits the Cerebras talk's core framing that agents replace experts in the mixture-of-agents pattern, and reframes Yann Dubois's point as a general sparsity explanation rather than naming MoE transformers as the dominant open-source architecture, while adding an extra (plausible but unverified) Daniel Han quantization mention.
  
> Based on my research, here's where Mixture of Experts (MoE) is explained across the library: - From Mixture of Experts to Mixture of Agents with Super Fast Inference — Daniel Kim & Daria Soboleva [00:07:17]–[00:09:52]: This is the most detailed treatment. Daniel Kim explains that the feed-forward layer in a transformer is a bottleneck because one network has to "disentangle" all information (different languages, domains, tasks); MoE replaces the single feed-forward network with multiple specialized "expert" networks (e.g., one for math, one for biology) plus a new "router" network that decides which expert handles each token, letting you scale parameters/model capacity without increasing inf…

**VI agent (Gemini 3.8 Flash)** — quality 0.62, P 1.00, R 1.00, facts 0.25, timestamps near a mention 1.00 (n=8), $0.118, 19 s
  
Judge: The answer identifies exactly the four correct library videos with specific timestamp ranges and no invented sources, and the Yangqing Jia description matches the expected sparsely-activated-efficiency framing. However, it misses the distinctive framing in three cases — it omits the Cerebras mixture-of-agents analogy (agents replacing experts), Ishan Anand's 'biggest architectural change since GPT-2' audience-question context, and Yann Dubois's point that transformers with MoE are the dominant open-source architecture — instead giving generic mechanism descriptions.
  
> Mixture of Experts (MoE) is explained in the following videos across the library: From Mixture of Experts to Mixture of Agents with Super Fast Inference - Daniel Kim & Daria Soboleva [00:07:17 - 00:10:18]: Daria Soboleva explains that Mixture of Experts addresses the bottleneck of monolithic feed-forward networks by replacing them with multiple specialized feed-forward networks ("experts") and a router network that selects which expert to activate per token, allowing the model to increase parameter capacity and quality without increasing inference time compute [[cite:01M29G8NETDNGGY5QT9XHCJRXN:437-550], [cite:01M29G8NETDNGGY5QT9XHCJRXN:595-618]]. How LLMs work for Web Devs: GPT in 600 lines …

**Gemini agentic video (3×10 + merge)** — quality 0.62, P 1.00, R 1.00, facts 0.25, timestamps near a mention 0.75 (n=4), $0.082, 37 s
  
Judge: The answer identifies all four correct library videos with specific timestamps and no invented entries, which is a strong retrieval result. However, it misses the distinguishing framings in three facts — it never mentions the mixture-of-agents analogy for Cerebras, Ishan Anand's 'biggest architectural change since GPT-2' point, or Yann Dubois's claim that MoE transformers dominate open-source models — describing generic mechanism instead.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia [00:16:03] Yangqing Jia describes Mixture of Experts as using a massively sparse, sparsely activated model architecture to improve computational efficiency and reduce effective model size while maintaining high quality, drawing parallels to early Bayesian ensemble classifiers, multi-path Inception units, and tensor decomposition. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois [01:16:04] Yann Dubois explains Mixture of Experts as a sparse architecture where the linear layer is split into separate sub-layers so that each data point routes through only a subset of para…

## corpus-16: Which speakers refer to Rich Sutton's 'bitter lesson', and how do they use it?

*Kind: topic.* 

Expected: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone; Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois
  
Acceptable, not required: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia

Key facts:

- Yann Dubois recommends reading Sutton's blog post and summarises it as: general methods that scale with compute win, which has driven the field.
- Peter Stone cites it as the warning that injecting human input can lower ultimate performance, then argues humans and RL can be combined (TAMER + RL).
- Oriol Vinyals relates it to whether to bootstrap StarCraft agents with imitation learning from human games before reinforcement learning.

**VI agent with library tools (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=5), $0.152, 21 s
  
Judge: All four named videos exist in the catalog with specific timestamps, and the three key facts (Dubois on compute scaling, Stone on human input lowering performance plus TAMER+RL, Vinyals on imitation-learning bootstrapping for StarCraft) are all conveyed accurately. The only concern is the extra Yangqing Jia attribution, which is plausible but unverified by the ground-truth facts and may be an over-inclusion; Dubois is also credited with recommending the blog post only implicitly.
  
> Based on the library's transcripts, four talks explicitly invoke Rich Sutton's "bitter lesson": - Agentic AI MOOC LLM Agents Overview by Yann Dubois — [00:48:29] Dubois introduces the "bitter lesson" right after discussing the Chinchilla scaling paper, summarizing it as Sutton's claim that "the only thing that matters in the long run is leveraging compute," since empirically more compute into models yields more improvement, and (per Moore's-law-like trends) compute will keep growing . - Agentic AI MOOC Evolution of System Designs by Yangqing Jia — [00:33:46]–[00:34:20] Jia cites "Professor Richard Sutton's blog post" from 2019 to justify why AI infrastructure/compute has become the "third pi…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=8), $0.089, 20 s
  
Judge: All four named videos exist in the catalog with specific timestamp ranges, and the three key facts (Dubois, Stone, Vinyals) are each covered with correct mechanisms and attributions. The extra Yangqing Jia claim is a plausible but unverified addition beyond the ground-truth set; otherwise no hallucinated videos or missing timestamps.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois Timestamp: [00:48:29–00:50:23] Yann Dubois summarizes Sutton's blog post to emphasize that because compute continually expands, long-term progress comes from simple methods that scale with computation rather than overcomplicating or over-optimizing systems at current compute budgets . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia Timestamp: [00:33:38–00:34:07] Yangqing Jia cites Sutton's essay on a slide to explain that AI compute infrastructure has become a foundational pillar of IT strategy because general methods leveraging large-scale numerical computation acro…

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.73, P 1.00, R 0.67, facts 0.67, timestamps near a mention 1.00 (n=4), $0.067, 15 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone
  
Judge: The answer correctly identifies Dubois and Vinyals with plausible catalog mappings and specific timestamp ranges, but misses Peter Stone's use of the bitter lesson as a warning about human input lowering ultimate performance and his TAMER+RL combination argument. No invented videos are cited and the attributions given appear accurate, though coverage is incomplete (2 of 3 speakers).
  
> Based on my searches, two speakers in the library explicitly invoke Rich Sutton's "bitter lesson": - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois — [00:48:33–00:50:31] A slide titled "Bitter lesson" is shown while Dubois says "the bitter lesson... I would really recommend reading this blog post," using it to argue that leveraging compute matters more in the long run than manually engineering methods, since compute keeps growing (Moore's Law) and correlates with better model performance . - Agentic AI MOOC UC Berkeley CS294-196 F25 Multi-Agent Systems in Era of LLMs by Oriol Vinyals — [00:20:07–00:20:33] An on-screen "Bitter Lesson" label accompanies the …

**VI agent (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=8), $0.105, 21 s
  
Judge: All four named videos exist in the catalog with precise timestamp ranges, and the three key facts (Dubois, Stone, Vinyals) are accurately covered with correct mechanisms and conclusions. The extra Yangqing Jia attribution is an additional claim not in the ground-truth fact list and may be a false positive, though it is at least mapped to a real catalog video.
  
> Based on the search across the library, four talks explicitly invoke Rich Sutton's "bitter lesson": - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois [00:48:29–00:49:16]: Dubois cites Sutton's blog post directly, summarizing that "the only thing that matters in the long run is leveraging compute" since more compute empirically yields better performance, tying it to Moore's Law and the Chinchilla scaling discussion . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia [00:33:38–00:34:07]: Jia uses Sutton's 2019 blog post to justify why AI infrastructure ("the third pillar") is essential, paraphrasing the lesson as "t…

**VI agent (Gemini 3.8 Flash)** — quality 0.73, P 1.00, R 0.67, facts 0.67, timestamps near a mention 1.00 (n=3), $0.125, 29 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals
  
Judge: The answer correctly identifies Peter Stone (TAMER+RL) and Yann Dubois (general methods that scale with compute win) with plausible timestamps, but omits Oriol Vinyals' StarCraft imitation-learning bootstrapping discussion. It adds a third claim about Yangqing Jia quoting the bitter lesson for compute infrastructure, which is not in the ground-truth fact list and may be a misattribution/extra claim; no invented videos were named and all cited videos have timestamps.
  
> The following speakers refer directly to Rich Sutton's "bitter lesson": Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Autonomous Agents by Peter Stone [00:21:42] Peter Stone references Rich Sutton's "bitter lesson" to note that injecting human input can degrade ultimate performance compared to learning from scratch with reinforcement learning, before presenting work on combining human feedback with autonomous RL (TAMER+RL) . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia [00:33:38] Yangqing Jia displays and quotes Richard Sutton's 2019 blog post "The Bitter Lesson" to justify the necessity of AI computation infrastructure as a foundational IT pi…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=4), $0.040, 17 s
  
Judge: All four cited videos exist in the catalog, each with a specific timestamp, and the three ground-truth attributions (Dubois, Stone, Vinyals) are correctly matched with accurate mechanisms, though Vinyals' StarCraft context is left implicit. The extra Yangqing Jia claim is an unverified addition beyond the key facts, and the parenthetical figures after two titles appear to be video durations rather than cited timestamps.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia (01:19:31) [00:33:41] Yangqing Jia cites Rich Sutton's 2019 essay to justify the massive demand for AI computation and infrastructure, summarizing the core takeaway that the most effective method in 70 years of AI research is deploying general methods powered by large amounts of numerical computation on large datasets. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Autonomous Agents by Peter Stone (01:01:41) [00:21:42] When discussing the TAMER system, Peter Stone notes that while learning from human feedback is faster, autonomous learning from scratch reaches higher performance, citing Rich Sutton's…

## corpus-17: Which talks discuss vector databases or vector stores?

*Kind: mention.* 

Expected: Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB; How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust); Building Code First AI Agents with Azure AI Agent Service — Cedric Vidal, Microsoft; VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS

**VI agent with library tools (Sonnet 5)** — quality 0.89, P 0.80, R 1.00, timestamps near a mention 1.00 (n=6), $0.102, 21 s
  
Wrong: Strategies for LLM Evals (GuideLLM, lm-eval-harness, OpenAI Evals Workshop) — Taylor Jordan Smith
  
Judge: The answer names five catalog videos, all real entries, each with specific timestamps and quoted or paraphrased evidence that plausibly matches the vector-database topic. No invented videos or untimestamped mentions appear, though the accuracy of individual timestamps and the completeness of the list (e.g., other RAG-related talks) cannot be verified here.
  
> - Building Code First AI Agents with Azure AI Agent Service — Cedric Vidal, Microsoft — [00:15:20], [00:58:31] The speaker discusses whether the agent uses a vector database behind the scenes, and later on-screen code shows a "Contoso Product Information Vector Store" created and used via the filesearchtool with vector store IDs . - VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS — [00:09:20] Explains that after generating multimodal embeddings for images, tables and text, "you go to the database, any vector database, and store all these embeddings" for later retrieval . - Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB …

**VI agent with library tools (Gemini 3.8 Flash)** — quality 0.89, P 0.80, R 1.00, timestamps near a mention 1.00 (n=8), $0.131, 49 s
  
Wrong: Strategies for LLM Evals (GuideLLM, lm-eval-harness, OpenAI Evals Workshop) — Taylor Jordan Smith
  
Judge: All five named videos map to real catalog entries, each with specific timestamps and plausible, well-differentiated evidence (MongoDB, Azure AI Search, Qdrant, Elasticsearch, RAG eval diagram). No invented videos or missing timestamps, though the Notion and evals citations are thinly supported and could be incidental mentions rather than substantive discussion.
  
> The following talks discuss or show vector databases or vector stores: Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB [00:11:32], [00:25:03] The speaker introduces vector stores as tools for agents to interact with external data and demonstrates indexing multimodal embeddings into MongoDB acting as a vector database [[cite:01M292E276W4FYTE118Y3MCMH5:692-706], [cite:01M292E276W4FYTE118Y3MCMH5:1503-1516]]. Building Code First AI Agents with Azure AI Agent Service — Cedric Vidal, Microsoft [00:15:20], [01:19:14], [01:22:16] The speaker discusses how Azure AI Search functions as the underlying vector database and demonstrates defining and populating a vector store with chunk…

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.00, P 0.00, R 0.00, $0.067, 8 s
  
Missed: Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB; How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust); Building Code First AI Agents with Azure AI Agent Service — Cedric Vidal, Microsoft; VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS
  
Judge: The answer contains no actual response — it only states an intention to run another search and shows a tool call, naming no videos and providing no timestamps. It is entirely non-responsive to the question, with no attributions correct or incorrect.
  
> I need to run more targeted searches to confirm which videos actually discuss vector databases/stores, since the initial query was too generic. : search({"query": "vector database", "k": 10})

**VI agent (Sonnet 5)** — quality 0.75, P 0.75, R 0.75, timestamps near a mention 0.90 (n=10), $0.251, 27 s
  
Wrong: Strategies for LLM Evals (GuideLLM, lm-eval-harness, OpenAI Evals Workshop) — Taylor Jordan Smith
  
Missed: How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust)
  
Judge: The answer names four real catalog videos with specific timestamps and plausible, well-described vector-database content, with no invented titles or untimestamped entries. Attributions look coherent (MongoDB vector search, Qdrant RAG, evals RAG slide, Azure file search vector store), though the exact timestamp ranges cannot be verified here.
  
> Based on searches across the library, here are the talks that discuss vector databases/stores: - Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB — [00:25:03] and [00:31:51]–[00:34:13]: The speaker explains storing multimodal embeddings in a vector database (using MongoDB as the vector store) and shows an architecture diagram where the agent calls a "vector search" tool to retrieve relevant chunks . - VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS — [00:07:22], [00:35:52]–[00:41:29], [00:47:47]–[00:53:28]: The talk repeatedly covers setting up a local Qdrant vector database via Docker, storing page-embedding vectors, and…

**VI agent (Gemini 3.8 Flash)** — quality 0.75, P 0.75, R 0.75, timestamps near a mention 1.00 (n=3), $0.134, 24 s
  
Wrong: Strategies for LLM Evals (GuideLLM, lm-eval-harness, OpenAI Evals Workshop) — Taylor Jordan Smith
  
Missed: How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust)
  
Judge: All four named videos map to real catalog entries, each with a specific timestamp and a concrete description of the vector-database content, and no videos appear invented. The answer leaves raw citation markers in the text and its coverage may be incomplete (e.g., other RAG-related talks), but the attributions themselves are plausible and channel-consistent.
  
> Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB [00:25:03] The speaker explains that page screenshot embeddings are generated using a multimodal embedding model and stored in MongoDB acting as a vector database alongside metadata references . VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS [00:34:12] The presenter discusses storing late interaction token embeddings in a vector database and demonstrates deploying and querying the Qdrant vector database [[cite:01M29BFEMHGPPDX3PJRNTKG0Q1:2052-2070], [cite:01M29BFEMHGPPDX3PJRNTKG0Q1:2389-2489]]. Building Code First AI Agents with Azure AI Agent Service — Cedric Vidal, Micros…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=12), $0.061, 25 s
  
Judge: All four named videos map to real catalog entries (MongoDB multimodal agents, Notion/Braintrust AI products, AWS VoiceVision RAG, Azure AI Agent Service), each with specific timestamps and plausible vector-database-related evidence. No invented videos or untimestamped claims appear, though the answer may be incomplete since other talks (e.g., A2A/MCP or conversational agent workshops) could also touch on vector stores.
  
> Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB [00:11:38] Apoorva Joshi notes that agent tools can range from simple APIs to vector stores and specialized machine learning models. [00:25:05] Apoorva Joshi explains how document page screenshots, metadata, and embeddings are stored into a vector database, using MongoDB as the vector database for the workshop. How to build world-class AI products — Sarah Sachs (AI lead @ Notion) & Carlos Esteban (Braintrust) [00:25:08] Sarah Sachs explains how Notion isolates retrieval from generation when evaluating RAG systems, noting that retrieval evaluation has "much more to do with like our vector databases and elastic search setups."…

## corpus-18: Which talks cover guardrails for AI agents, and what kinds of guardrails do they describe?

*Kind: topic.* Four other talks say the word once in passing (Damien Murphy, Taylor Jordan Smith, Thor Schaeff, Mark Backman) and are acceptable.

Expected: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Agentic AI Safety & Security by Dawn Song; [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor
  
Acceptable, not required: Strategies for LLM Evals (GuideLLM, lm-eval-harness, OpenAI Evals Workshop) — Taylor Jordan Smith; [Full Workshop] Building Conversational AI Agents - Thor Schaeff, ElevenLabs; Full Workshop: Realtime Voice AI — Mark Backman, Daily; A2A & MCP Workshop: Automating Business Processes with LLMs — Damien Murphy, Bench

Key facts:

- Dawn Song lists runtime protections: input guardrails, output guardrails, information-flow control and taint tracking, monitoring, and human-in-the-loop; she shows Anthropic's constitutional classifiers as an input-guardrail example.
- Clay Bavor says deployed agents must be checked for adherence to policies and guardrails, combining AI checks with deterministic guardrails.
- The Vibe Coding at Scale workshop frames custom instructions and custom tools as guardrails that keep AI coding assistants consistent with enterprise conventions.

**VI agent with library tools (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.75 (n=8), $0.180, 31 s
  
Judge: All seven cited videos exist in the catalog and each carries a specific timestamp, with no invented entries; the three key facts are all conveyed, though the Dawn Song entry omits the constitutional-classifiers example and the Clay Bavor entry frames the guardrail check via simulation testing rather than the AI-plus-deterministic-checks combination. The four extra talks (evals, ElevenLabs, Daily, A2A) are plausible additions but unverified here and may stretch the notion of agent guardrails.
  
> Based on the evidence gathered, here are the talks that cover guardrails for AI agents: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [01:24:02] The talk presents a full taxonomy of agent defense mechanisms: input guardrails (detect/sanitize malicious inputs), output guardrails (prevent unsafe tool use/responses), information flow control and taint tracking, runtime monitoring, human-in-the-loop validation, privilege separation, formal verification, identity/credential/access management, and model/tool hardening, framed under "defense in depth" and least-privilege principles . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Practical Lessons …

**VI agent with library tools (Gemini 3.8 Flash)** — quality 0.83, P 1.00, R 1.00, facts 0.67, timestamps near a mention 1.00 (n=14), $0.124, 23 s
  
Judge: All four cited videos exist in the catalog and each is given specific timestamp ranges, with the Clay Bavor and Vibe Coding attributions matching the expected content well. The Dawn Song entry only partially covers her runtime-protection list (input/output guardrails and constitutional classifiers, but no information-flow control/taint tracking, monitoring, or human-in-the-loop), and the extra A2A & MCP claim is a marginal, likely over-reaching addition.
  
> Based on the video library transcripts and presentations, the following talks discuss guardrails for AI agents and describe the specific kinds of guardrails implemented or needed: Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song [01:24:20]–[01:25:56]: Dawn Song discusses runtime protection mechanisms, detailing input guardrails (e.g., Anthropic's constitutional classifiers, DataSentinel, and PromptArmor) that block, filter, normalize, and sanitize untrusted or malicious inputs and prompt injection attacks before they reach the agent . [01:26:59]–[01:27:28]: She details output guardrails (e.g., Progent) that validate, filter, and apply programmable pri…

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.42, P 1.00, R 0.33, facts 0.33, timestamps near a mention 0.50 (n=2), $0.038, 12 s
  
Missed: [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor
  
Judge: The answer correctly identifies and timestamps the Dawn Song talk, accurately listing the runtime guardrail categories and the constitutional classifiers example (omitting only human-in-the-loop). However, it misses the Clay Bavor policy/deterministic-guardrail point and the Vibe Coding at Scale custom instructions/tools framing, and wrongly asserts that no other library talk covers guardrails.
  
> Based on my search, only one talk in the library explicitly and substantively covers "guardrails" for AI agents as a named security concept; the other hits from my search were not actually about guardrails (they were tangential matches on generic terms like "video" or unrelated ML topics). - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [01:24:04–01:37:04] The lecturer presents runtime protection mechanisms for agentic AI security, explicitly naming "input guardrails, output guardrails, information flow control and taint tracking, monitoring" as the main categories of runtime protection, and gives concrete examples such as Anthropic's Constitutio…

**VI agent (Sonnet 5)** — quality 0.73, P 1.00, R 0.67, facts 0.67, timestamps near a mention 0.75 (n=8), $0.264, 36 s
  
Missed: [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments
  
Judge: All three named videos exist in the catalog and are given specific timestamps; the Dawn Song and Clay Bavor entries capture the expected guardrail substance (input/output guardrails, constitutional classifiers, deterministic plus AI checks), though information-flow control/taint tracking and human-in-the-loop are not explicitly mentioned. The answer misses the Vibe Coding at Scale workshop's custom-instructions-as-guardrails point and instead substitutes the ElevenLabs conversational agents talk, whose moderation/watermarking content is plausible but outside the expected set.
  
> Based on my search across the library, here are the talks that substantively discuss guardrails for AI agents: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [01:24:07], [01:27:04], [01:39:14]. Dawn Song details a layered "defense-in-depth" architecture: input guardrails that block/filter/sanitize untrusted inputs (citing Anthropic's Constitutional Classifiers, and her own DataSentinel and PromptArmor systems for detecting/removing prompt-injection attacks), output guardrails that validate tool calls and generated content against security policies (e.g., the Progent privilege-control system), and access-control/identity-management guardrails (ma…

**VI agent (Gemini 3.8 Flash)** — quality 0.73, P 1.00, R 0.67, facts 0.67, timestamps near a mention 1.00 (n=2), $0.124, 21 s
  
Missed: [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments
  
Judge: The answer correctly identifies Dawn Song's and Clay Bavor's talks with plausible timestamps and captures the input/output guardrail and deterministic-plus-AI-check substance, though it omits Song's information-flow control, taint tracking, monitoring, and human-in-the-loop items. It misses the Vibe Coding at Scale workshop's framing of custom instructions and tools as enterprise guardrails; no videos were invented.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song [01:24:22] Dawn Song details runtime protections for AI agents, describing input guardrails that sanitize, filter, and block malicious prompts (such as prompt-injection detection models and constitutional classifiers) and output guardrails that inspect generated text and validate tool calls against programmable privilege and security policies . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Practical Lessons from Deploying...by Clay Bavor [00:44:06] Clay Bavor presents a layered guardrail framework designed to thwart prompt injection and abuse, combining deterministic input validation and secondary LLM…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.40 (n=10), $1.079, 66 s
  
Judge: All five named videos map to real catalog entries with specific timestamps, and the three ground-truth talks (Dawn Song, Clay Bavor, Vibe Coding at Scale) are correctly identified with their guardrail mechanisms, though the Dawn Song entry omits information-flow control/taint tracking, monitoring, and human-in-the-loop. Two extra videos (ElevenLabs conversational agents and Daily realtime voice) are added with plausible but unverified guardrail details, and some cited timestamps appear to be video durations rather than moments of evidence.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song [01:24:31] – [01:29:20] Dawn Song details runtime defenses consisting of input guardrails (which block, filter, sanitize, or normalize untrusted inputs to prevent prompt injection, such as constitutional classifiers, DataSentinel, and PromptArmor) and output guardrails (which validate and filter generated content and tool calls to enforce security policies and dynamic least-privilege control, exemplified by Progent). Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Practical Lessons from Deploying...by Clay Bavor [00:40:26] Clay Bavor explains using "supervisor" micro-agents sitting around the core agent…

## corpus-19: Which talks discuss scaling laws?

*Kind: topic.* 

Expected: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Predictable Noise in LLM Benchmarks by Sida Wang; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois

Key facts:

- Yann Dubois explains scaling laws as tuning at small scale and extrapolating to a large final run, and mentions the Chinchilla paper.
- Sida Wang uses scaling laws (consistent gains when model size doubles) as a source of signal for judging benchmark noise.

**VI agent with library tools (Sonnet 5)** — quality 0.75, P 1.00, R 1.00, facts 0.50, timestamps near a mention 1.00 (n=4), $0.080, 8 s
  
Judge: Both named videos map correctly to catalog entries with specific timestamp ranges and accurate attributions, with no invented videos. The Yann Dubois entry captures the small-scale-tuning-then-extrapolation mechanism but omits the Chinchilla paper mention, so that fact is only partially covered.
  
> - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois — [00:41:54]–[00:44:39] The lecturer explains that pretraining compute predictably determines test loss, showing a plot where blue "runs" trace a line that can be fit to predict performance at 10x or 100x more compute, calling this the "scaling law," and describing how it lets researchers tune recipes at small scale before committing large GPU budgets . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Predictable Noise in LLM Benchmarks by Sida Wang — [00:42:34]–[00:43:07] Discusses how, when a model family releases a series of sizes, doubling model size gives a "scaling law" of consistent improvement, used…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=6), $0.122, 32 s
  
Judge: The answer identifies both correct library videos with plausible timestamp ranges and no invented entries. It conveys both key facts, including the Chinchilla reference for Dubois and the doubling-model-size signal argument for Sida Wang, though the small-scale-to-large-final-run extrapolation is phrased somewhat loosely.
  
> Based on an exhaustive scan of the library transcripts and on-screen text, the talks that discuss scaling laws are: Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois — [00:41:54–00:47:00] and [00:56:50–00:57:15]: Yann Dubois presents pre-training scaling laws (referencing papers like Kaplan et al. and Chinchilla) to show how test loss can be predicted as compute increases and how researchers can tune architectures and hyperparameters at small scale , later contrasting train-time compute scaling laws with test-time compute scaling for reasoning models . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Predictable Noise in LLM Benchmarks by Sida Wang — [00:42:27…

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.17, P 0.25, R 0.50, facts 0.00, timestamps near a mention 1.00 (n=2), $0.099, 28 s
  
Wrong: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Agentic AI Safety & Security by Dawn Song; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Predictable Noise in LLM Benchmarks by Sida Wang
  
Judge: The answer names four real catalog videos with timestamps but misses the key Sida Wang benchmark-noise discussion entirely and only vaguely gestures at Yann Dubois's scaling-law content without the small-scale-tuning/extrapolation mechanism or the Chinchilla reference. Two of its four hits (Dawn Song, Peter Stone) are hedged, speculative attributions ('appear to show', 'touches on'), suggesting likely misattribution, and the Weizhu Chen claim is similarly unsupported by concrete evidence.
  
> Based on the search evidence gathered, the following talks contain discussions of scaling laws: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois — [00:44:31–00:47:01] The transcript at this point discusses how LLM capabilities scale with compute, data, and parameters, consistent with a scaling-laws discussion in an LLM fundamentals overview . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Training Agentic Models by Weizhu Chen — [00:11:46–00:12:24] The talk's description/transcript around this timestamp references scaling behavior in the context of training agentic models . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security b…

**VI agent (Sonnet 5)** — quality 0.75, P 0.33, R 1.00, facts 1.00, timestamps near a mention 0.75 (n=4), $0.177, 31 s
  
Wrong: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Multi-Agent AI by Noam Brown; From Mixture of Experts to Mixture of Agents with Super Fast Inference - Daniel Kim & Daria Soboleva; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen
  
Judge: All six named videos map to real catalog entries with specific timestamp ranges, and both key facts (Dubois on extrapolation/Chinchilla and Sida Wang on scaling laws as benchmark signal) are covered accurately. The additional talks (Daniel Han, Kim/Soboleva, Weizhu Chen, Noam Brown) are plausible but more loosely related test-time/training scaling content, which risks over-inclusion though no videos appear invented or misattributed.
  
> Based on the search results, here are the talks that explicitly discuss scaling laws: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois — [00:41:55]–[00:53:28]: The lecturer gives a dedicated segment on scaling laws, explaining that pretraining test loss decreases predictably with more compute (model size × data), allows extrapolating performance from small-scale experiments, and introduces the Chinchilla paper on optimal compute allocation between model size and data, noting it ignores inference costs . He also ties this to Sutton's "Bitter Lesson" that scale and compute matter more than manual optimization , and later discusses test-time compute scaling f…

**VI agent (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.75 (n=4), $0.136, 28 s
  
Judge: Both named videos exist in the catalog and are correctly attributed with specific timestamp ranges, and the Chinchilla reference plus the benchmark-noise signal point are both captured. Fact 1's framing as "tune at small scale and extrapolate to a large final run" is only loosely conveyed via the compute-allocation/architecture-comparison description, but the substance of scaling-law extrapolation is present and no videos are invented.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois — [00:41:54] - [00:48:26]: Yann Dubois examines empirical scaling laws showing how log test loss decreases predictably with log compute, demonstrates how scaling curves evaluate architectural trade-offs between Transformers and LSTMs, and reviews the Chinchilla scaling law for optimal compute allocation between model parameters and training tokens . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Predictable Noise in LLM Benchmarks by Sida Wang — [00:42:27] - [00:42:47]: Sida Wang explains that scaling laws provide consistent performance improvements when doubling model size across model families, which benchm…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=5), $0.137, 175 s
  
Judge: The answer names only two catalog videos, both correctly mapped (Sida Wang's benchmark noise talk and Yann Dubois's LLM agents overview), with specific timestamps for each claim. Both key facts are covered, including the Chinchilla reference and the small-scale-to-large extrapolation mechanism; no invented videos or apparent misattributions.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Predictable Noise in LLM Benchmarks by Sida Wang [00:42:44] Sida Wang discusses scaling laws when evaluating benchmark signal-to-noise ratios, noting that while scaling laws yield consistent improvements when doubling model size, many benchmarks (especially code benchmarks) cannot reliably measure this gain over the noise. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois [00:43:40] Yann Dubois introduces empirical scaling laws, demonstrating how test loss follows a predictable linear trajectory on a log-log plot against compute, which allows researchers to experiment at small scales and extrapolate performance …

## corpus-20: Which speakers mention self-driving cars or autonomous vehicles, and in what connection?

*Kind: topic.* 

Expected: Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone
  
Acceptable, not required: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Agentic AI Safety & Security by Dawn Song; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand

Key facts:

- Aman Khan (Arize) started his career on self-driving cars at Cruise, became a PM for evaluation systems there, and applies those lessons to AI evals (he also polls the room about Waymo).
- Peter Stone calls an autonomous car an agent that must sense, decide and act, and mentions his DARPA Urban Challenge car.

**VI agent with library tools (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=9), $0.092, 15 s
  
Judge: All three named videos exist in the catalog and both key facts are covered with specific timestamps, though the answer omits Aman Khan's Waymo audience poll and adds a third (Dawn Song) claim not in the ground-truth fact list that may or may not be accurate. No invented videos and no missing timestamps; attributions to Khan and Stone appear correct.
  
> - Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize — [00:01:26], [00:01:38], [00:01:52] — Aman Khan describes starting his career working on self-driving cars at Cruise, later becoming a PM for self-driving evaluation systems, and jokes he now works on "self-writing code agents" instead of self-driving cars . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Autonomous Agents by Peter Stone — [00:11:44], [00:11:52], [00:12:06], [00:22:26]–[00:22:40] — Peter Stone frames autonomous driving as a canonical agent example ("an autonomous car is an agent... it has to sense, decide, and act"), mentions his own DARPA Urban Challenge car and companies like Waymo and Zoox, an…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=4), $0.119, 35 s
  
Judge: Both named videos exist in the catalog and are correctly attributed with specific timestamps, covering Aman Khan's Cruise/AV-eval background (including Waymo mention) and Peter Stone's DARPA Urban Challenge car. Stone's framing of a car as an agent that senses, decides and acts is not stated explicitly, but the substance of the autonomous-agent connection is conveyed; timestamp accuracy cannot be verified here.
  
> The following speakers mention self-driving cars or autonomous vehicles across the library: Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize [00:01:25] Aman Khan notes that he began his career working on self-driving cars at Cruise and became a PM for autonomous vehicle evaluation systems before moving to evaluation platforms for AI agents . [01:06:35] Khan uses the history of self-driving cars at Cruise and Waymo as an analogy for iteratively building evaluation datasets, showing how cars progressed from driving down straight roads to requiring dedicated test sets for left turns and pedestrian encounters . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Autonomous …

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.75, P 1.00, R 1.00, facts 0.50, timestamps near a mention 0.50 (n=4), $0.065, 14 s
  
Judge: The answer correctly identifies both relevant videos with specific timestamps and accurately captures Aman Khan's Cruise/self-driving evaluation background (though it omits the Waymo poll). For Peter Stone it substitutes GT Sophy / Gran Turismo racing content instead of the expected framing of an autonomous car as a sense-decide-act agent and the DARPA Urban Challenge car, which is likely a misattribution of the relevant segment.
  
> Based on the search results, mentions of self-driving cars/autonomous vehicles appear in two videos: - Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize [00:01:26] — Aman Khan describes starting his engineering career working on self-driving cars at Cruise, and becoming a PM for self-driving evaluation systems there before moving to Spotify and then Arize . [01:06:11] — He later uses a self-driving car analogy (handling left turns, pedestrians on sidewalks) to illustrate the iterative process of building AI evaluation datasets . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Autonomous Agents by Peter Stone [00:45:51–00:48:06] — Peter Stone discusses "GT Sophy," a…

**VI agent (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.60 (n=10), $0.200, 24 s
  
Judge: Both claimed videos exist in the catalog and are correctly mapped, with specific timestamp ranges given for each, and both key facts are conveyed (Aman Khan's Cruise background applied to evals, Peter Stone's sense-decide-act autonomous car plus DARPA Urban Challenge). The Waymo reference is attributed to Stone rather than Khan's audience poll, a minor misattribution, and the GT Sophy details are plausible but unverified elaboration.
  
> Based on searches across the full library, self-driving cars / autonomous vehicles come up mainly in two talks: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Autonomous Agents by Peter Stone - [00:10:48–00:13:23] Peter Stone frames "an autonomous car" as a prototypical agent that must sense, decide, and act, recalling his own DARPA Urban Challenge car and noting that companies like Waymo and Zoox are investing heavily in making fully autonomous roads a reality, including speculation about whether traffic lights would still be needed once all cars are autonomous . - [00:13:23–00:15:57] He describes Sony AI's Gran Turismo Sophy project, showing a split-screen of a human driving in the raci…

**VI agent (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=7), $0.134, 18 s
  
Judge: The answer correctly identifies both key videos (Aman Khan on Cruise evals and Peter Stone on autonomous cars/DARPA Urban Challenge) with specific timestamps, though it omits the Waymo audience poll detail. It adds two extra attributions (Ishan Anand's Waymo tokenization and Dawn Song's collision-avoidance example) that are plausible but unverified and may be fabricated; no invented videos are named and all claims carry timestamps.
  
> Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize - [00:01:25], [00:06:42], and [01:06:37] - Aman Khan discusses his background building evaluation systems for self-driving cars at Cruise and draws parallels between curating evaluation datasets for edge cases in autonomous driving (such as unprotected left turns and pedestrians) and evaluating generative AI agents [[cite:01M29ABBRDKG9AF3DVQRQBPSN2:85-115], [cite:01M29ABBRDKG9AF3DVQRQBPSN2:442-452], [cite:01M29ABBRDKG9AF3DVQRQBPSN2:3997-4045]]. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Autonomous Agents by Peter Stone - [00:11:43] and [00:22:25] - Peter Stone highlights autonomous cars as fundamental examples of…

**Gemini agentic video (3×10 + merge)** — quality 0.75, P 1.00, R 1.00, facts 0.50, timestamps near a mention 1.00 (n=6), $1.193, 188 s
  
Judge: All three named videos exist in the catalog with specific timestamps, and the Aman Khan fact is well covered (Cruise career, evaluation PM role, Waymo, lessons applied to AI evals). The Peter Stone entry covers the DARPA Urban Challenge but omits his framing of an autonomous car as an agent that senses, decides and acts; the Ishan Anand Waymo-tokenization claim is plausible but unverified and possibly embellished.
  
> Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize (01:26:15) [00:01:25]: Aman Khan notes that he began his career working on self-driving cars and evaluation systems at Cruise before transitioning to evaluating AI agents. [00:07:25]: He cites Waymo and self-driving cars as physical-world AI systems whose development and reliability lessons directly apply to building and evaluating software AI agents. [01:06:38]: He uses the incremental development of self-driving cars at Cruise and Waymo—progressing from driving straight to mastering left turns with pedestrians—as an analogy for iteratively curating evaluation datasets for AI agents. Agentic AI MOOC UC Berkeley CS294…

## corpus-21: Compare how Noam Brown and Oriol Vinyals talk about self-play: what did it achieve, and where does it break down?

*Kind: synthesis.* 

Expected: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Multi-Agent AI by Noam Brown; Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals
  
Acceptable, not required: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen

Key facts:

- Noam Brown: self-play is behind the game breakthroughs (AlphaGo, Dota, StarCraft, poker) and has not yet appeared in LLMs.
- Noam Brown: intuitions about self-play are overfit to two-player zero-sum perfect-information games like Go and chess; the nice properties break outside them.
- Oriol Vinyals: naive self-play oscillates (the rock-paper-scissors example), so AlphaStar used a league with exploiter agents.
- Oriol Vinyals: AlphaStar was also evaluated by playing humans, not only by climbing self-play Elo.

**VI agent with library tools (Sonnet 5)** — quality 0.65, P 1.00, R 1.00, facts 0.50, timestamps near a mention 0.92 (n=12), $0.141, 28 s
  
Judge: Both named videos are real catalog entries correctly attributed to Brown and Vinyals, with specific timestamp ranges throughout and no invented sources. It captures the two-player zero-sum limitation and the rock-paper-scissors/league-with-exploiters points, but omits Brown's claim that self-play has not yet worked for LLMs and Vinyals' point about evaluating AlphaStar against humans rather than self-play Elo; some details (worker-rush framing, population 'collapse' rather than oscillation) are plausible but unverified embellishments.
  
> - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown [00:09:38–00:11:37] Brown explains that in any two-player zero-sum game, "sound" self-play needs no human data and provably converges to a minimax equilibrium (guaranteeing you won't lose in expectation), which is what powered victories in Go, chess, poker and RTS games — though in practice convergence is imperfect and agents can remain vulnerable to adversarial exploits . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown [00:36:03–00:38:02] He describes DORA, trained purely via self-play like AlphaZero for no-press Diplomacy, achieving a "clearly superhuman" win rate in 2-player games…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 0.65, P 1.00, R 1.00, facts 0.50, timestamps near a mention 0.75 (n=8), $0.140, 33 s
  
Judge: Both cited videos are real catalog entries correctly matched to Noam Brown and Oriol Vinyals, with specific timestamps given for each, and the core breakdown mechanisms (zero-sum-only guarantees, non-transitive oscillation, exploiter-based league training) are conveyed. It omits that self-play has not yet worked for LLMs and that AlphaStar was evaluated against human players rather than just self-play Elo, and some added details (PPO non-convergence, worker-rush exploits) are plausible but unverified elaborations.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown [00:10:07], [00:11:37], [00:13:07], [00:37:43] Noam Brown explains that while sound self-play successfully achieves a minimax equilibrium without requiring human data in balanced two-player zero-sum games like Go, chess, and poker, it breaks down in practice because neural network approximations remain vulnerable to adversarial exploits, standard reinforcement learning algorithms like PPO fail to converge in imperfect-information games, and multi-player games like Diplomacy suffer from coordination failures when self-play agents converge on an arbitrary equilibrium that performs poorly against humans or differently …

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.65, P 1.00, R 1.00, facts 0.50, timestamps near a mention 1.00 (n=4), $0.067, 15 s
  
Judge: Both cited videos exist in the catalog and are correctly attributed with specific timestamp ranges, covering Brown's 'self-play worked in games but not yet LLMs' point and Vinyals's rock-paper-scissors non-convergence example. It misses Brown's point about intuitions being overfit to two-player zero-sum perfect-information games, does not explicitly mention AlphaStar's league/exploiter agents or human evaluation, and slightly softens Vinyals's population-based training into a generic alternative.
  
> Based on the available evidence, two videos in the library directly address self-play in multi-agent AI, from two different UC Berkeley CS294-196 guest lectures: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown — [00:00:00–00:02:21]: Brown opens by tracing self-play's role in past AI breakthroughs (e.g., the AlphaGo training pipeline), drawing an analogy to LLM development to set up why recursive self-improvement via self-play "hasn't yet worked" for LLMs the way it did for game-playing agents . - Agentic AI MOOC UC Berkeley CS294-196 F25 Multi-Agent Systems in Era of LLMs by Oriol Vinyals — [00:39:57–00:42:11]: Vinyals uses a rock-paper-scissors example to show…

**VI agent (Sonnet 5)** — quality 0.82, P 1.00, R 1.00, facts 0.75, timestamps near a mention 0.86 (n=14), $0.214, 39 s
  
Judge: Both named videos map correctly to catalog entries (Noam Brown and Oriol Vinyals lectures) with specific, plausible timestamp ranges and no invented sources. It misses Brown's point that self-play has not yet shown up in LLMs and reframes his 'overfit intuitions' argument as a formal guarantee, and it renders Vinyals' oscillation/rock-paper-scissors dynamic as a one-strategy collapse, but the league/exploiter and human-evaluation points are attributed accurately.
  
> Based on the two Berkeley CS294-196 lectures that explicitly discuss self-play, here is the comparison: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown — [00:09:38–00:11:03] Brown explains the theoretical power of self-play: in any two-player zero-sum game, a "sound" self-play algorithm provably converges to a minimax equilibrium without any human data, which underlies victories in Go, chess, poker, and RTS games . - [00:30:46–00:32:51] He then shows where it breaks down even in a simple two-player general-sum game (the Ultimatum Game): pure self-play converges to a rational-but-inhuman equilibrium (offering the smallest possible amount), which fails to model r…

**VI agent (Gemini 3.8 Flash)** — quality 0.47, P 1.00, R 1.00, facts 0.25, timestamps near a mention 0.86 (n=7), $0.076, 20 s
  
Judge: Both cited videos are real catalog entries correctly attributed to Noam Brown and Oriol Vinyals, each with specific timestamps, and no videos were invented. However, coverage is partial: it omits that self-play has not yet worked for LLMs, omits AlphaStar's league/exploiter-agent remedy and the rock-paper-scissors framing of oscillation, and omits evaluation against human players, while adding details (adversarial exploits, Void Rays, delusion) whose accuracy cannot be verified from the listed facts.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown [00:10:05], [00:11:37], [00:31:16], [00:38:19] Noam Brown highlights that sound self-play provably converges to a minimax equilibrium without human data in two-player zero-sum games (driving milestones in Go, chess, and poker), but explains that it breaks down due to function approximation leaving neural networks vulnerable to adversarial exploits, and fundamentally fails in non-zero-sum or multi-player settings (like the Ultimatum Game or Diplomacy) where agents converge to idiosyncratic equilibria that perform terribly against humans or differently trained agents [[cite:01M296FBHQD7SWTC6TZ59NSMQ9:605-673], [cite:0…

**Gemini agentic video (3×10 + merge)** — quality 0.65, P 1.00, R 1.00, facts 0.50, timestamps near a mention 0.83 (n=6), $0.099, 32 s
  
Judge: Both named videos are real catalog entries correctly matched to Noam Brown and Oriol Vinyals, each with several concrete timestamps and no invented sources. It captures the zero-sum-vs-non-zero-sum breakdown and the rock-paper-scissors/league-with-exploiters points, but omits that self-play has not yet worked for LLMs and does not state that AlphaStar was evaluated by playing humans rather than only self-play Elo; some specifics (ultimatum game, Void Rays, minimax convergence) are plausible but unverified elaborations.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown [00:09:47] / [00:26:11] / [00:37:30]: Noam Brown explains that self-play achieved superhuman milestones in two-player zero-sum games (like Go, chess, and poker) by provably converging to a minimax equilibrium without needing human data, but it breaks down in non-zero-sum and cooperative environments (such as Diplomacy and the ultimatum game) where pure self-play finds arbitrary equilibria that fail to coordinate with humans, making self-play without human data a dead end for human cooperation. Agentic AI MOOC UC Berkeley CS294-196 F25 Multi-Agent Systems in Era of LLMs by Oriol Vinyals [00:09:31] / [00:40:00] - [00:…

## corpus-22: How many talks in the library come from the UC Berkeley Agentic AI MOOC (CS294-196), and who are the speakers?

*Kind: synthesis.* A metadata question: both systems see the video titles (VideoIndex in the agent's system prompt, Gemini in the catalog attached to the request).

Expected: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Post-Training Verifiable Agents by Jiantao Jiao; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Agentic AI Safety & Security by Dawn Song; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Predictable Noise in LLM Benchmarks by Sida Wang; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Multi-Agent AI by Noam Brown; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone; Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | AI Agents to Automate Science by James Zou

Key facts:

- There are eleven MOOC lectures in the library.
- The speakers are Dawn Song, Noam Brown, Yangqing Jia, Jiantao Jiao, Clay Bavor, Sida Wang, Peter Stone, Weizhu Chen, Yann Dubois, Oriol Vinyals and James Zou.

**VI agent with library tools (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, $0.152, 39 s
  
Judge: The answer correctly identifies all 11 MOOC lectures and names every speaker, mapping each to a real catalog entry with an opening timestamp and no invented videos. Attributions look sound, with the only soft spots being inference-based identifications for Yangqing Jia, Sida Wang and James Zou rather than direct self-introductions.
  
> Based on the video titles and confirmed by each talk's opening self-introduction, there are 11 talks in the library from the UC Berkeley Agentic AI MOOC (CS294-196), each given by a different guest speaker: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [00:00:00] Dawn Song opens: "So today, we will talk about towards building safe and secure agentic AI." - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown — [00:00:01] "I'm Noam Brown... I'll be talking about multi-agent AI." - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia — [00:00:02] Speaker recounts his Berkeley PhD, Goo…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, facts 1.00, $0.173, 42 s
  
Judge: The answer correctly identifies all eleven MOOC lectures and their speakers, with each mapped to a real catalog entry and given a timestamp. No invented videos or misattributions appear; only the James Zou timestamp (00:34:00) is an odd mid-talk citation rather than an intro, but the content claim is plausible.
  
> There are 11 talks in the library from the UC Berkeley Agentic AI MOOC (CS294-196): Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song [00:00:00] Dawn Song introduces the session on building safe and secure agentic AI amid the rapid development of frontier AI systems . Speaker: Dawn Song Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown [00:00:01] Noam Brown introduces himself and begins his broad presentation on multi-agent AI and self-play algorithms . Speaker: Noam Brown Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia [00:00:02] Yangqing Jia reflects on earning his PhD at UC Berke…

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, $0.069, 32 s
  
Judge: The answer correctly identifies all eleven MOOC lectures and their speakers with no invented videos, matching the ground truth exactly. Timestamps are provided for only four of the eleven talks (the rest are justified by title cards alone), but the cited evidence appears plausible and no misattributions are evident.
  
> Based on the video titles in the library (each explicitly labeled "Agentic AI MOOC UC Berkeley CS294-196 Fall 2025") and supporting on-screen/transcript evidence found via search, the following 11 talks belong to this Berkeley course series: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [01:48:17] The lecturer closes the session by inviting participation in the AgentBeats competition and thanking attendees for joining the Agentic AI MOOC - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown — title card identifies this as a lecture in the CS294-196 series - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of Sy…

**VI agent (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, $0.139, 42 s
  
Judge: The answer correctly identifies all eleven MOOC lectures with the right speakers and provides a timestamped opening quote for each, with no invented videos or misattributions. Timestamps are all near 0:00 (essentially just opening lines), which is appropriate for this identification-style question.
  
> Based on the video titles and opening transcripts, the following 11 videos in the library are from the UC Berkeley Agentic AI MOOC (CS294-196): - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [00:00:00] Dawn Song opens: "So today, we will talk about towards building safe and secure agentic AI." - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown — [00:00:01] Noam Brown introduces himself: "I'm Noam Brown... I'll be talking about multi-agent AI." - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia — [00:00:02] The speaker recounts his Berkeley PhD and career at Google/Caffe/Tenso…

**VI agent (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, facts 1.00, $0.052, 16 s
  
Judge: The answer correctly counts 11 MOOC lectures and names all eleven speakers, mapping each to a real catalog video with specific opening timestamps. No invented videos or misattributions are present, and every named talk carries a timestamp range.
  
> There are 11 talks in the library from the UC Berkeley Agentic AI MOOC (CS294-196): Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song [00:00:00 - 00:00:10]: Speaker Dawn Song introduces the session discussing safe and secure agentic AI . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown [00:00:01 - 00:00:13]: Speaker Noam Brown introduces himself and announces he will be speaking on multi-agent AI . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia [00:00:02 - 00:00:15]: Speaker Yangqing Jia reflects on completing his PhD at UC Berkeley between 2009 and 2013 and introduces his talk on…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, facts 1.00, $0.111, 45 s
  
Judge: The answer correctly identifies all eleven MOOC lectures and names every speaker, mapping cleanly to catalog entries with no invented videos. Timestamps are present (mostly trivial 00:00 openings, plus some durations reported as timestamps), and attributions match the catalog titles and speakers.
  
> There are 11 talks in the library that come from the UC Berkeley Agentic AI MOOC (CS294-196). The speakers are Dawn Song, Noam Brown, Yangqing Jia, Jiantao Jiao, Clay Bavor, Sida Wang, Peter Stone, Weizhu Chen, Yann Dubois, Oriol Vinyals, and James Zou. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song [00:00:00] Speaker Dawn Song displays the title slide "Agentic AI: Towards Building Safe & Secure Agentic AI" and begins the lecture by stating, "So today we will talk about towards building safe and secure agentic AI." Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown [00:00:00] Speaker Noam Brown introduces himself and his le…

## corpus-23: Which talks are given by Microsoft speakers, and which Microsoft products do they demonstrate?

*Kind: synthesis.* 

Expected: Collaborating with Agents in your Software Dev Workflow - Jon Peck & Christopher Harrison, Microsoft; Building Code First AI Agents with Azure AI Agent Service — Cedric Vidal, Microsoft
  
Acceptable, not required: Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Agentic AI Safety & Security by Dawn Song; [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia

Key facts:

- Jon Peck and Christopher Harrison (Microsoft) demonstrate GitHub Copilot (agent mode / coding agent) in the software development workflow.
- Cedric Vidal (Microsoft) demonstrates building code-first agents with Azure AI Agent Service (Azure AI Foundry).

**VI agent with library tools (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=8), $0.286, 44 s
  
Judge: Both key facts are covered with correctly mapped catalog videos and specific timestamps, and the extra Weizhu Chen entry is a plausible Microsoft-affiliated addition explicitly flagged as not a product demo. No invented videos appear, though the Chen timestamps and the precise Copilot demo timings are unverified and could be slightly off.
  
> Microsoft speaker talks and Microsoft products demonstrated - Building Code First AI Agents with Azure AI Agent Service — Cedric Vidal, Microsoft — [00:06:15], [00:08:45], [00:41:15]–[00:49:00]: Cedric Vidal, a Principal AI Advocate at Microsoft, walks through building an agent with the Azure AI Agent Service and Azure AI Foundry, live-signing into Azure AI Foundry, copying a project connection string, and pasting it into VS Code to wire up the SDK . He also explains the pros/cons of letting the managed Azure AI Agent Service handle orchestration versus building agents code-first . - Collaborating with Agents in your Software Dev Workflow - Jon Peck & Christopher Harrison, Microsoft — [00:52…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=5), $0.169, 40 s
  
Judge: Both key facts are covered accurately with timestamps, and the additional Vibe Coding workshop and Weizhu Chen mentions are plausible Microsoft-affiliated extras rather than invented videos. Minor concerns: the Weizhu Chen entry is given without timestamps, and the Copilot Workspace/speaker-name details in the extras are unverified and could be embellished.
  
> Collaborating with Agents in your Software Dev Workflow - Jon Peck & Christopher Harrison, Microsoft [00:31:14] – [00:51:13] The presenters demonstrate using GitHub Copilot (including Copilot Workspace, Copilot coding agents, and chat) within Visual Studio Code (VS Code) and GitHub to implement application features and resolve repository issues [[cite:01M297QEC5K6FC7WXAZA3D2RZ7:1874-1900], [cite:01M297QEC5K6FC7WXAZA3D2RZ7:3073-3100]]. Building Code First AI Agents with Azure AI Agent Service — Cedric Vidal, Microsoft [00:00:21] – [00:05:00] Microsoft Principal AI Advocate Cedric Vidal demonstrates building and orchestrating code-first AI agents using Azure AI Agent Service and Azure AI Found…

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=2), $0.037, 12 s
  
Judge: The answer correctly identifies both Microsoft talks and their products, with a plausible timestamped range for the GitHub Copilot demo and no invented videos. However, the Azure AI Agent Service talk is named without any timestamp and is hedged as a title-level match only, weakening the evidentiary support.
  
> Based on the search results and video titles, the library contains two talks explicitly by Microsoft speakers: - Collaborating with Agents in your Software Dev Workflow - Jon Peck & Christopher Harrison, Microsoft — [00:57:45–00:58:45] The presenters work inside a file named "1-copilot-coding-agent.md" and interact with an "Add Context..." panel, demonstrating GitHub Copilot's coding-agent workflow inside the editor . - Building Code First AI Agents with Azure AI Agent Service — Cedric Vidal, Microsoft — this talk's title itself indicates the whole session is a demonstration of the Azure AI Agent Service product, building code-first AI agents on Azure; I was not able to pull a specific times…

**VI agent (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.73 (n=15), $0.391, 48 s
  
Judge: The answer correctly identifies both ground-truth Microsoft talks (Jon Peck/Christopher Harrison with GitHub Copilot and Cedric Vidal with Azure AI Agent Service/Foundry) with specific timestamps, and no videos are invented. It adds two extra library items — the Vibe Coding workshop, attributed to Microsoft only via inferred slide branding without a named speaker, and Weizhu Chen's lecture (mentioned without timestamps and explicitly excluded as a product demo) — which are plausible but unverified attributions.
  
> Based on the searches, here are the talks I found with confirmed Microsoft speakers demonstrating Microsoft products: - Collaborating with Agents in your Software Dev Workflow - Jon Peck & Christopher Harrison, Microsoft [00:08:48], [00:44:02], [00:55:45]–[00:58:45] The two Microsoft presenters demo GitHub Copilot end-to-end: starting with code completions ("code completion is sort of the original version...of GitHub Copilot") , then walking through the Copilot coding agent workflow where an issue is assigned to Copilot, it opens a sandboxed environment, and produces a pull request for review , and finally showing Copilot generating git commands and reviewing PRs together with an MCP-server …

**VI agent (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.67 (n=6), $0.135, 26 s
  
Judge: The answer correctly identifies both Microsoft product talks with plausible timestamp ranges and the right products (GitHub Copilot; Azure AI Agent Service / AI Foundry), with no invented videos. It additionally flags Weizhu Chen's lecture as a Microsoft-affiliated speaker without a product demo, which is an extra but reasonably qualified inclusion.
  
> Collaborating with Agents in your Software Dev Workflow - Jon Peck & Christopher Harrison, Microsoft [00:33:01] - [00:53:45] Jon Peck and Christopher Harrison demonstrate using GitHub Copilot (including Copilot agent mode and Copilot workspace/instructions) integrated inside Visual Studio Code and GitHub to build, test, and edit application code [[cite:01M297QEC5K6FC7WXAZA3D2RZ7:1980-2040], [cite:01M297QEC5K6FC7WXAZA3D2RZ7:3185-3225]]. Building Code First AI Agents with Azure AI Agent Service — Cedric Vidal, Microsoft [00:44:18] - [01:45:00] Cedric Vidal demonstrates Azure AI Agent Service within Azure AI Foundry, showing hands-on live code examples integrating agent capabilities with Azure …

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=7), $0.354, 304 s
  
Judge: All three cited videos exist in the catalog and each is given specific timestamps, with both key facts (GitHub Copilot agent mode by Peck/Harrison and Azure AI Agent Service by Vidal) correctly attributed. The extra Vibe Coding workshop claim adds a Microsoft speaker attribution (Harald Kirschner) not in the key facts, which may or may not be accurate, but it does not misattribute content across the main talks.
  
> Collaborating with Agents in your Software Dev Workflow - Jon Peck & Christopher Harrison, Microsoft (01:04:05) [00:30:40]: Christopher Harrison demonstrates GitHub Copilot's coding agent automatically working on an assigned GitHub issue, executing inside GitHub Actions, and opening a draft pull request. [00:36:50]: He shows how to iterate with GitHub Copilot coding agent by submitting follow-up comments on the pull request to restart agent execution. [00:42:30]: Inside GitHub Codespaces (web-based Visual Studio Code), he demonstrates how GitHub Copilot utilizes repository-level copilot-instructions.md and custom .instructions files to guide coding agent and chat behavior. [Full Workshop] Vi…

## corpus-24: Which videos mention Sam Altman?

*Kind: negative.* Nobody says the name; it appears once on a slide in Dawn Song's lecture (acceptable). A good answer says no talk mentions him, or at most points at that slide.

Expected: no video
  
Acceptable, not required: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Agentic AI Safety & Security by Dawn Song

**VI agent with library tools (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=1), $0.071, 8 s
  
Judge: The answer names a single catalog video (Dawn Song's Agentic AI Safety & Security) with a specific timestamp at 00:00:28 and a plausible quoted Sam Altman slide, with no invented videos. Coverage may be incomplete if other talks also mention Sam Altman, but the single attribution given is specific and verifiable.
  
> This appears as a quoted headline/screenshot on a slide (a Sam Altman quote about AGI) rather than spoken discussion. - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [00:00:28] A slide displays a Sam Altman quote/headline stating "We are now confident we know how to build AGI as we have traditionally understood it... 2025 will be the year of AI agents," shown alongside other 2025 AI predictions . Videos: 1

**VI agent with library tools (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=1), $0.037, 15 s
  
Judge: The answer names a single real catalog video (Dawn Song's Agentic AI Safety & Security talk) with a precise timestamp and a plausible slide-based description of a Sam Altman quote. No invented videos appear, though the answer may be incomplete if other talks also mention Sam Altman.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song [00:00:28]: A slide titled "2025 is the Year of Agents" features a post from Sam Altman asserting that they are confident they know how to build AGI as traditionally understood and expect the first AI agents to enter the workforce and alter company output in 2025 . Videos: 1

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, $0.058, 9 s
  
Judge: The answer claims no video in the library mentions Sam Altman and names no specific videos or timestamps, so there are no hallucinated or misattributed video claims. It is honest about incomplete searching, but provides no positive evidence and would be incorrect if any talk does mention Sam Altman.
  
> Based on the searches performed, none of the results actually contain evidence of "Sam Altman" being mentioned by name — the returned hits were about unrelated topics (ComfyUI video nodes, speech-to-text OCR fragments, training data slides, GPT history, etc.) and none of the transcript or OCR excerpts reference "Sam Altman" specifically. I do not have confirmed evidence in the index that any video mentions Sam Altman. To answer this reliably, an exhaustive findmentions scan for "Sam Altman" (and common misspellings) across the library would be needed, which was not able to be completed in this turn. Videos: 0

**VI agent (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=1), $0.242, 24 s
  
Judge: The answer names a single catalog video (Dawn Song's Agentic AI Safety & Security) with a precise timestamp and a plausible quoted slide, and explicitly dismisses other candidate hits as OCR noise. No invented videos appear, though the exclusive claim of only one mention may undercount, and the dismissal of the Notion/Braintrust talk is asserted without evidence.
  
> Based on my search, only one video in the library contains an explicit reference to Sam Altman. - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [00:00:28] A slide titled "2025 is the Year of Agents" displays a quoted tweet attributed to Sam Altman stating "We are now confident we know how to build AGI as we have traditionally understood it... in 2025, we may see the first AI agents 'join the workforce.'" Other apparent "Sam" hits (e.g., in the Notion/Braintrust product talk) turned out to be OCR noise from unrelated UI text (like "Doug Sandhu" or generic labels), not references to Sam Altman . Videos: 1

**VI agent (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=1), $0.113, 25 s
  
Judge: The answer names one real catalog video (Dawn Song's Agentic AI Safety & Security lecture) with a precise timestamp and a plausible quoted Sam Altman slide, so no invented videos appear. However, it is likely incomplete, citing only a single mention across a library where other talks may also reference Sam Altman.
  
> - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song [00:00:28] A slide titled "2025 is the Year of Agents" displays a quote from Sam Altman stating, "We are now confident we know how to build AGI as we have traditionally understood it. We believe that in 2025, we may see the first AI agents join the workforce and materially change the output of companies" . Videos: 1

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, $0.107, 44 s
  
Judge: The answer asserts that no video in the library mentions Sam Altman, naming no videos and giving no timestamps. There are no invented videos or misattributions, but the blanket negative claim provides no supporting evidence.
  
> None of the videos in the library mention Sam Altman. Videos: 0

## corpus-25: Which videos talk about using an LLM as a judge for evaluation?

*Kind: mention.* 

Expected: Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize; How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust); [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; Evals 101 — Doug Guthrie, Braintrust; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor
  
Acceptable, not required: RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI

**VI agent with library tools (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 0.83 (n=12), $0.212, 36 s
  
Judge: The answer names seven real catalog videos, each mapped correctly and each with a specific timestamp or range, with no invented titles. Attributions are plausible and topically consistent (evals workshops, fine-tuning/RL reward discussion, agent deployment lessons), though the exact timestamp accuracy and quote fidelity cannot be verified here and some entries (e.g., Yann Dubois slide) are given only approximate timing.
  
> Based on the transcripts and on-screen text across the library, these videos discuss using an LLM as a judge for evaluation: - How to build world-class AI products — Sarah Sachs (AI lead @ Notion) & Carlos Esteban (Braintrust): [00:10:48-00:11:16] The speaker explains scoring functions for evals and says "I'll talk a bit about the LLM as a judge process that we use. That's probably what we rely on the most," contrasting it with heuristic/deterministic scoring functions . - Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize: [00:03:41-00:04:07] The session is framed as going from "writing an eval" to "writing an eval for an LLM as a judge system" . - Evals 101 — Doug G…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 0.91, P 1.00, R 0.83, timestamps near a mention 0.82 (n=11), $0.142, 43 s
  
Missed: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han
  
Judge: The answer names six real catalog videos, all plausibly related to LLM-as-a-judge, each with at least one specific timestamp and no invented titles. Some attributions (e.g., Clay Bavor and Yann Dubois) are only loosely described and rely on single unverified timestamps, so their precision is uncertain.
  
> The following videos discuss using an LLM as a judge for evaluation: How to build world-class AI products — Sarah Sachs (AI lead @ Notion) & Carlos Esteban (Braintrust) [00:11:00], [00:13:08], [00:33:24], [00:40:21] — The speakers explain how Notion and Braintrust use LLM-as-a-judge scoring functions and prompt-based evaluators to grade model outputs against golden datasets and validate their agreement with human annotations [[cite:01M296M6JG0G3VBMDVH6X65ANV:648-676], [cite:01M296M6JG0G3VBMDVH6X65ANV:784-805], [cite:01M296M6JG0G3VBMDVH6X65ANV:2004-2016], [cite:01M296M6JG0G3VBMDVH6X65ANV:2421-2445]]. Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize [00:03:55], [00:43…

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.25, P 0.50, R 0.17, timestamps near a mention 1.00 (n=2), $0.066, 14 s
  
Wrong: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | AI Agents to Automate Science by James Zou
  
Missed: How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust); [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; Evals 101 — Doug Guthrie, Braintrust; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor
  
Judge: The answer identifies only one clear match (Aman Khan's evals talk) with a specific timestamp range and quoted on-screen/spoken evidence, plus a hedged secondary mention of the Berkeley 'AI Agents to Automate Science' talk without any timestamp. No invented videos appear, but coverage is likely incomplete since the answer itself admits the search was not exhaustive, omitting other evals-focused talks in the library.
  
> Based on the search results gathered, only one video in the library explicitly discusses "LLM as a judge" as an evaluation technique: - Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize [00:50:05]–[00:52:39] — On-screen slide reads "Evaluate LLM data with an LLM-as-a-Judge," and the presenter says "I'm using an LLM as a judge. You could use code as well. You don't have to use an LM as a judge here," while demonstrating a code-based evaluator that checks whether an LLM-as-a-judge's labels agree with human annotations on a travel-itineraries dataset . A related but not identical case appeared in the Berkeley "AI Agents to Automate Science" talk, where the speaker discu…

**VI agent (Sonnet 5)** — quality 0.73, P 0.80, R 0.67, timestamps near a mention 0.93 (n=14), $0.436, 43 s
  
Wrong: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen
  
Missed: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor
  
Judge: All six named videos map to real catalog entries, each with specific timestamp ranges and plausible speaker-attributed evidence about LLM-as-judge usage. No invented videos or untimestamped mentions appear, though the attributions cannot be verified here and some (e.g., the Weizhu Chen grader snippet) are brief and could be loosely characterized.
  
> Based on my research, here are the videos in the library that discuss using an LLM as a judge for evaluation: - RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI — [00:11:06]–[00:11:31] Ilan notes that reinforcement fine-tuning is "a really interesting candidate for training an LLM judge model" since grading a problem is itself a verifiable task, and later revisits "LLM as a judge model" concepts around [00:55:03]–[00:56:21] . - Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize — [00:42:38]–[00:47:16] The speaker builds and demos an "LLM-as-a-Judge" evaluator in Arize's platform to classify chatbot responses as "friendly" or "robotic," then checks it agains…

**VI agent (Gemini 3.8 Flash)** — quality 0.80, P 1.00, R 0.67, timestamps near a mention 1.00 (n=8), $0.153, 32 s
  
Missed: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor
  
Judge: All five cited videos exist in the catalog and each claim carries specific timestamps, with evals-focused talks (Braintrust, Arize, Notion) being plausible core matches. The Yann Dubois AlpacaEval and OpenAI fine-tuning DPO-judge attributions are more tangential and could be misattributed, but no invented videos appear.
  
> The following videos discuss using an LLM as a judge for evaluation: How to build world-class AI products — Sarah Sachs (AI lead @ Notion) & Carlos Esteban (Braintrust) [00:13:08] Sarah Sachs details Notion's LLM-as-a-judge evaluation system, contrasting universal judging prompts (such as faithfulness and conciseness) with item-specific prompts to catch model regressions . [00:39:45] Carlos Esteban outlines best practices for LLM-as-a-judge scorers, including using larger models to judge smaller ones and calibrating judge outputs against human-annotated feedback . Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize [00:47:36] Aman Khan demonstrates using an LLM as a ju…

**Gemini agentic video (3×10 + merge)** — quality 0.91, P 1.00, R 0.83, timestamps near a mention 0.81 (n=16), $0.106, 45 s
  
Missed: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han
  
Judge: The answer names five catalog videos, all plausibly relevant to LLM-as-a-judge, each with specific timestamps and quoted evidence, and invents no videos. One stray artifact is the '(01:58:20)' appended to the Yann Dubois title (apparently a duration, not a cited moment), but otherwise attributions of speakers to titles are consistent with the catalog.
  
> Evals 101 — Doug Guthrie, Braintrust [00:08:00], [00:15:05], [00:44:30]: Doug Guthrie explains how Braintrust implements "LLM as a judge" scorers where a language model evaluates output against defined criteria to assign scores, offers guidance on breaking down scorers into focused tasks, and discusses evaluating LLM judges for online production monitoring. How to build world-class AI products — Sarah Sachs (AI lead @ Notion) & Carlos Esteban (Braintrust) [00:13:08], [00:16:45], [00:19:55], [00:39:10]: Sarah Sachs details Notion's methodology of using targeted "LLM as a judge" prompts tailored to specific evaluation rules and regression checks, while Carlos Esteban outlines best practices fo…

## corpus-26: Summarize the top themes across both event series in the library (the AI Engineer workshops and the UC Berkeley Agentic AI MOOC lectures). What do the two series have in common, and how do they differ?

*Kind: library.* Whole-library synthesis; scored on the facts only. Both systems see the titles and channels of all 30 videos.

Expected: scored on facts only

Key facts:

- Both series centre on LLM agents: building them, tool use, multi-agent systems and protocols such as MCP/A2A.
- Both series treat evaluation of LLM systems and agents as a major theme (evals tooling and frameworks at AI Engineer; benchmarks and verifiable rewards at Berkeley).
- Both series cover RL, post-training and fine-tuning (Daniel Han's and Ilan Bigio's workshops; the Dubois, Jiao and Chen lectures).
- AI Engineer is hands-on and applied: workshops on voice AI, RAG and multimodal retrieval, coding assistants, LLM serving/inference, image generation with ComfyUI, and even raising venture capital.
- The Berkeley MOOC is lecture-style and more research-oriented: multi-agent and game AI (self-play), agent safety and security, benchmark noise, agents for science, system design.

**VI agent with library tools (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, $0.241, 62 s
  
Judge: The answer invents no videos and gives plausible, specific timestamps for eight AI Engineer talks, and its high-level comparison of the two series (agents, evals, multi-agent, RL/fine-tuning, applied vs research) broadly matches the ground truth. However, it cites zero Berkeley MOOC lectures by name or timestamp despite characterizing them (AlphaStar/safety/science claims are unsourced), it omits several applied AI Engineer topics (ComfyUI, VC raising, coding assistants), and the response is cut off mid-sentence.
  
> I'll research the top recurring themes (agents, evaluation, multi-agent systems, fine-tuning/RL, tool use, safety, deployment) across both series with targeted searches. Top themes across the library Both the AI Engineer workshops (practitioner-facing, tool/product focused) and the UC Berkeley Agentic AI MOOC (academic, research-oriented) converge heavily on a shared vocabulary of "agentic AI," but they treat it from different angles. What they have in common: - Both series spend significant time defining what an "AI agent" is (LLM + tools + memory/loop), treating this as a foundational, still-unsettled concept. - Both discuss evaluation of agents as categorically harder than evaluating trad…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, facts 1.00, $0.083, 25 s
  
Judge: The answer cites eight real catalog videos with precise timestamp ranges and plausible, correctly attributed evidence (Brown on self-play, Song on safety vs security, Jiao on verifiable rewards, Dubois on SFT, Guthrie on eval flywheel, Murphy on MCP/A2A, Bigio on RFT, SGLang serving), with no invented videos; two further videos (ElevenLabs voice agents, Azure AI Agent Service) are named in the differences section without any timestamps. Coverage of the AI Engineer side is thin — RAG/multimodal retrieval, coding assistants, ComfyUI image generation and the VC talk are omitted, as are Berkeley's agents-for-science and system-design lectures and Daniel Han's RL workshop.
  
> Summary of Top Themes, Commonalities, and Differences Across both event series—the UC Berkeley Agentic AI MOOC lectures (CS294-196) and the AI Engineer workshops: - Top Themes: Core themes across both programs include autonomous agent architectures, multi-agent coordination, post-training methods (SFT, RL, DPO, RFT), rigorous evaluation and benchmarking, runtime tool integration, and systems/inference optimization. - What They Have in Common: Both series explore how to transition large language models from conversational text predictors into reliable, goal-directed agents capable of invoking external tools, orchestrating multi-step workflows, and achieving high task accuracy through post-tra…

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.80, P 1.00, R 1.00, facts 0.80, $0.063, 29 s
  
Judge: All seven cited videos are real catalog entries with concrete timestamps, and the applied-vs-academic contrast plus the shared agent/training themes are stated correctly. However, coverage is thin: evaluation is only name-dropped without citing any of the several evals-focused workshops or Berkeley benchmark lectures, and major library topics (MCP/A2A protocols, coding assistants, LLM serving, agents for science, benchmark noise, VC fundraising) are omitted entirely.
  
> Based on the evidence gathered, here is a summary of themes across the AI Engineer workshops and the UC Berkeley Agentic AI MOOC lectures, illustrated with representative video moments: - ComfyUI Full Workshop — first workshop from ComfyAnonymous himself! [00:00:00] — ComfyAnonymous opens an AI Engineer World's Fair workshop introducing ComfyUI's features and popularity, exemplifying the series' hands-on, tool-building focus . - [Full Workshop] Building Conversational AI Agents - Thor Schaeff, ElevenLabs [00:18:15] — Thor demonstrates ElevenLabs' voice library and marketplace for cloned voices, showing the AI Engineer series' emphasis on shipping practical, commercial AI agent products . - V…

**VI agent (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, $0.244, 66 s
  
Judge: The answer cites 17 real catalog videos with specific, plausible timestamps and no invented titles, and it organizes them into coherent cross-series themes with an explicit commonality/difference discussion. Coverage gaps remain: Daniel Han's RL/quantization workshop, the voice AI, RAG/multimodal retrieval, ComfyUI and VC-raising workshops, and the Jiao/Chen/Jia MOOC lectures (verifiable rewards, agentic model training, system design) are never mentioned, so the breadth of the AI Engineer applied track and parts of the Berkeley research track are underrepresented; the stated video count (19) also exceeds the videos actually cited.
  
> Top Themes Across the Library Theme 1 – What is an "AI Agent" & its components - Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB [00:03:02] "I like to define an AI agent as a system that uses a large language model..." - Building Code First AI Agents with Azure AI Agent Service — Cedric Vidal, Microsoft [00:03:08] Notes that even specialists disagree on the definition of "agent," which has evolved over time. - Agentic AI MOOC Autonomous Agents by Peter Stone [00:11:23]-[00:13:23] Slides define "intelligent agents" and "autonomous agents" as core academic framing for the rest of the lecture. Theme 2 – Evals as a discipline (both practical tooling and academic benchmarking)…

**VI agent (Gemini 3.8 Flash)** — quality 0.80, P 1.00, R 1.00, facts 0.80, $0.153, 45 s
  
Judge: All ten cited videos are real catalog entries with specific timestamp ranges, and attributions match the speakers and expected content (Song on safety, Brown on self-play, Wang on benchmark noise, Bigio on SFT/DPO/RFT). However, the answer omits much of the applied AI Engineer breadth — no mention of voice AI workshops, RAG/multimodal retrieval, ComfyUI image generation, LLM serving (SGLang), or the VC fundraising talk — and Daniel Han's RL/quantization workshop is not cited, so the hands-on applied theme is only generically asserted rather than evidenced.
  
> Overview of Themes, Commonalities, and Differences Top Themes: - Agent Architecture & Autonomy: Moving past single-turn prompt engineering toward multi-step reasoning, tool usage, environment interaction, and agent swarms/multi-agent collaboration. - Post-Training & Optimization: Advancing model capabilities via reinforcement learning (RL), supervised fine-tuning (SFT), direct preference optimization (DPO), reinforcement fine-tuning (RFT), and verifiable feedback loops. - Evaluation & Benchmarking: Developing robust evaluation harnesses, automated grading, and detecting predictable noise or leakage in agent benchmarks. - Safety, Security, & Systems Engineering: Hardening agentic systems agai…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, facts 1.00, $0.257, 140 s
  
Judge: The answer covers all five key themes and cites all 30 catalog videos with specific, plausible timestamps and no invented titles. Attributions appear accurate and appropriately matched to speakers and topics (e.g., MCP/A2A to Murphy, benchmark noise to Sida Wang), making this a comprehensive and well-grounded response.
  
> Summary of Top Themes, Commonalities, and Differences Commonalities Both the AI Engineer workshops and the UC Berkeley Agentic AI MOOC lectures focus on the fundamental shift from static, single-turn text prompts and conversational chatbots to autonomous Agentic AI systems. Key overlapping themes include: - Agent Architectures & Multi-Agent Collaboration: Designing systems capable of autonomous planning, external tool execution, API calls, and coordinating multiple specialized agents to execute complex workflows. - Training, Post-Training, & Inference-Time Optimization: Exploring alignment and optimization techniques—such as reinforcement learning (RL/RLHF), supervised fine-tuning (SFT), dir…

## corpus-27: Which of these AI topics is talked about the most across the whole library: agents, evaluation, RAG, fine-tuning/RL, or MCP? Rank them and say roughly how concentrated each one is.

*Kind: library.* Ground truth from spoken-mention counts over the ASR transcripts (agents 2,479 in 28 videos; evaluation 721 in 22; fine-tuning/RL 669 in 22; MCP 210 in 11; RAG 66 in 11).

Expected: scored on facts only

Key facts:

- Agents is by far the most-discussed topic, spoken about in nearly every talk (about 2,500 spoken mentions in 28 of 30 videos).
- Evaluation and fine-tuning/RL come next, each in the low hundreds of mentions and each spread over about 22 talks.
- MCP is discussed in about a third of the talks, mostly the AI Engineer workshops, and RAG is the least-discussed of the five (around a dozen talks, mostly in passing).
- Agent talk is concentrated in the Berkeley MOOC lectures; MCP is mostly an AI Engineer topic.

**VI agent with library tools (Sonnet 5)** — quality 0.50, P 1.00, R 1.00, facts 0.50, timestamps near a mention 0.50 (n=8), $0.330, 116 s
  
Judge: All nine cited videos map to real catalog entries with mostly specific timestamps, though VoiceVision RAG and the Collaborating with Agents workshop are named without any timestamp and the answer is truncated mid-sentence. Ranking deviates from ground truth (MCP placed above evaluation, fine-tuning counted in only 16 videos), and it inverts the concentration finding by calling agents the least concentrated rather than noting its concentration in the Berkeley MOOC lectures, while inflating RAG's spread.
  
> Ranking: Which AI topic dominates the library Based on transcript/OCR/description counts across all 30 videos (~7,465 total hits): 1. Agents — by far the most discussed topic (~4,496 hits, appearing in 28 of 30 videos). It's the least "concentrated" topic — it shows up almost everywhere, from dedicated Berkeley agentic-AI lectures to workshops on RAG, MCP, and fine-tuning. 2. Fine-tuning / RL — ~819 combined hits (fine-tuning + RLHF + reinforcement learning) across 16 videos, moderately concentrated in a handful of deep technical sessions. 3. MCP — ~810 hits across 14 videos, but highly concentrated: three workshops (Vibe Coding at Scale, A2A & MCP Workshop, Collaborating with Agents) accoun…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 0.00, P 1.00, R 1.00, facts 0.00, $0.204, 66 s
  
Judge: The answer is not an answer at all—it is a raw, malformed tool call referencing an opaque video ID that matches no catalog entry, so no substantive claims, rankings, or timestamps are provided. None of the key facts about topic frequency or concentration are conveyed.
  
> call:defaultapi:gettranscript{t0:0,t1:60,videoid:01M29BFEMHGPPDX3PJRNTKG0Q1}

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.25, P 1.00, R 1.00, facts 0.25, timestamps near a mention 0.80 (n=5), $0.063, 28 s
  
Judge: The answer correctly puts agents first and cites only real catalog videos, with timestamps for the five sampled clips, though most of the videos it names in the ranking discussion (the agent and eval talks) get no timestamps and rest on title inference rather than mention counts. It gets the scale and concentration facts wrong — it claims agents appears in roughly a dozen talks rather than nearly all, lumps RAG and MCP together as equally rare and single-workshop topics, understates fine-tuning/RL's spread, and misses that agent talk concentrates in the Berkeley MOOC while MCP is an AI Engineer phenomenon.
  
> Based on the single exploratory search I was able to run this turn (I was not able to follow up with the more rigorous countmentions/findmentions scans that this kind of library‑wide ranking question really needs), here is what the evidence shows, with the caveat that this is a partial picture rather than an exhaustive count. Qualitatively, judging both by video titles across the 31‑video library and by the hits that did surface, "agents" is clearly the most talked‑about topic, appearing as the explicit subject of roughly a dozen talks (e.g. Multi‑Agent AI, Autonomous Agents, Multi‑Agent Systems, Training Agentic Models, LLM Agents Overview, AI Agents to Automate Science, Building Code‑First…

**VI agent (Sonnet 5)** — quality 0.25, P 1.00, R 1.00, facts 0.25, $0.200, 60 s
  
Judge: The answer correctly puts agents first and evaluation second and names only real catalog videos (no invented titles), but it provides zero timestamps and no mention counts or per-video/channel distribution. It is also truncated mid-sentence, so MCP and RAG are never ranked and the Berkeley-MOOC vs AI-Engineer concentration insight is missing; its claim that fine-tuning/RL is confined to a small set also conflicts with the broad spread in ground truth.
  
> Ranking (most to least discussed across the library) 1. Agents — by far the most pervasive topic. It is the explicit subject of entire dedicated talks (Peter Stone's "Autonomous Agents," Azure AI Agent Service, A2A & MCP Workshop, Multimodal AI Agents From Scratch, Noam Brown's and Oriol Vinyals' multi-agent talks, etc.) and comes up as a supporting theme in nearly every other talk in the index (evals talks frame themselves around "evaluating agents," fine-tuning talks discuss "agentic models," RAG talks discuss "agentic RAG"). Concentration: diffuse but massive — low per-video density but present almost everywhere. 2. Evaluation — the second-most common thread, with several full talks dedic…

**VI agent (Gemini 3.8 Flash)** — quality 0.50, P 1.00, R 1.00, facts 0.50, timestamps near a mention 0.73 (n=22), $0.200, 44 s
  
Judge: The ranking order (agents > evaluation > fine-tuning/RL > MCP > RAG) is close to ground truth and all cited videos are real catalog entries with plausible, specific timestamps, plus a correct note that agents dominate the Berkeley MOOC lectures. However, it quantifies concentration only by counting 'dedicated' sessions, badly understating how widely evaluation, fine-tuning/RL and MCP are mentioned (no mention counts, no ~22-talk or ~third-of-talks spread), and it names 3l0Zxus34es and HV8pugcFVO0 without any timestamps.
  
> Across the entire video library, agents is talked about the most by a wide margin . Ranking and Concentration 1. Agents (Highest concentration): Heavily dominates the library, accounting for over half of all videos (including nearly the entire UC Berkeley CS294-196 "Agentic AI MOOC" series and multiple practical workshops on building agent systems). 2. Evaluation (Moderate-to-high concentration): Featured as the central subject of roughly five dedicated presentations and workshops (covering production evaluation frameworks, benchmark noise, and eval tooling like Braintrust and GuideLLM) as well as dedicated evaluation modules in agent courses. 3. Fine-tuning / Reinforcement Learning (Moderat…

**Gemini agentic video (3×10 + merge)** — quality 0.75, P 1.00, R 1.00, facts 0.75, timestamps near a mention 0.82 (n=28), $0.371, 114 s
  
Judge: The answer correctly ranks agents first and cites 26 real catalog videos with specific timestamps, inventing no titles, though its odd 'Video 11/12/...' numbering and lack of any mention counts make the concentration claims qualitative rather than quantified. Its main error is inverting the bottom of the ranking — it calls MCP the least-covered and RAG fourth, whereas RAG is actually the least-discussed and MCP appears in roughly a third of talks; it also omits the near-equal second-place standing and ~22-talk spread of evaluation and fine-tuning/RL.
  
> Across the entire library, agents is talked about the most by a wide margin. Topic Ranking and Concentration 1. Agents (Most talked about / Heavily concentrated): The dominant theme across the collection, appearing in over 20 talks across the library. It is the central topic of numerous full developer workshops (agent workflows, multimodal agents, conversational voice agents, coding assistants) and the primary focus of the UC Berkeley Agentic AI MOOC lectures spanning multi-agent systems, agent architectures, safety, and scientific automation. 2. Fine-tuning / RL (Moderately high concentration): Extensively covered as the core training and optimization foundation for reasoning models and aut…

## corpus-28: Which model family is named most often across all the talks: GPT/ChatGPT, Gemini, Llama, Claude or DeepSeek? Give the ranking.

*Kind: library.* Counts from the ASR transcripts; DeepSeek and Claude are within noise of each other so either order is accepted.

Expected: scored on facts only

Key facts:

- GPT/ChatGPT is named most often and in the most talks (roughly 180 spoken mentions across 18 videos).
- Gemini is second (about 90 mentions in 13 talks).
- Llama is third (about 55 mentions in 11 talks); Claude (about 35 in 9) and DeepSeek (about 40 in 7) trail at similar levels, in either order.

**VI agent with library tools (Sonnet 5)** — quality 0.67, P 1.00, R 1.00, facts 0.67, timestamps near a mention 1.00 (n=10), $0.120, 41 s
  
Judge: The answer correctly puts GPT/ChatGPT first and Gemini second, and every cited video maps to a real catalog entry with a specific timestamp, so no hallucinated talks appear. However, its tail ranking deviates from ground truth by placing Claude third above Llama, and its raw counts (inflated by OCR/description hits) are several times higher than the spoken-mention figures, though the citations themselves look plausibly attributed.
  
> Based on transcript, on-screen text (OCR) and description counts across the library (~1,171 total hits), the ranking of model families named most often is: 1. GPT / ChatGPT — ~467 mentions (385 "GPT" + 82 "ChatGPT") across 21+ videos — clearly the most-named family. 2. Gemini — ~233 mentions across 16 videos. 3. Claude — ~173 mentions across 16 videos. 4. Llama — ~170 mentions across 11 videos. 5. DeepSeek — ~128 mentions (124 "DeepSeek" + 4 "deep seek") across 10 videos. Representative evidence (mostly for the 1 family, GPT/ChatGPT, with some cross-family context): - How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand — [00:00:17] "Uh, GPT in 600 [lines]" — the whole ta…

**VI agent with library tools (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=17), $0.228, 49 s
  
Judge: The answer correctly identifies GPT/ChatGPT as most named and Gemini second, and although its headline ranking places Claude above Llama (counting slide text), it explicitly notes Llama is third by spoken mentions, roughly matching the expected ordering; counts are inflated relative to ground truth but the ranking substance holds. All cited videos are real catalog entries with specific timestamps, no inventions detected, though the supporting citations only illustrate GPT mentions and the final entry is truncated mid-citation.
  
> The GPT/ChatGPT model family is named most often across all the talks. Across speech transcripts and on-screen slide text in the library: 1. GPT / ChatGPT: ~467 total mentions (~129 spoken segments across 21 videos) 2. Gemini: ~233 total mentions (~74 spoken segments across 16 videos) 3. Claude: ~173 total mentions (~27 spoken segments across 16 videos; total occurrences slightly edge out Llama when including slide text) 4. Llama: ~170 total mentions (~47 spoken segments across 11 videos; ranks 3 if measuring spoken mentions alone) 5. DeepSeek: ~128 total mentions including ASR variants like "deep seek" (~38 spoken segments across 12 videos) (Note: The inspection budget was exhausted before …

**VI retrieval-only with per-video cap (Sonnet 5)** — quality 0.33, P 1.00, R 1.00, facts 0.33, timestamps near a mention 1.00 (n=6), $0.052, 19 s
  
Judge: The answer correctly concludes GPT/ChatGPT is the most-named family and cites three real catalog videos with specific timestamp ranges, but it explicitly admits no exhaustive count was run and gives no mention totals or per-talk coverage. Its ranking is off: it ties Llama with Gemini for second rather than placing Gemini clearly second and Llama third, and the evidence base is just three passages rather than library-wide counts.
  
> Based on the evidence gathered, here is what I found across the library. Note: a fully precise ranking would require an exhaustive countmentions scan, which I was not able to run in this turn — the following is based on the strongest hits found via search, so treat the ranking as approximate rather than a definitive count. - How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS — Ishan Anand — [00:17:27]–[00:18:26]: The speaker shows a "family tree" of LLMs, naming ChatGPT, Llama, Bard/Gemini, GPT-4, and Claude as all descending from GPT-2, with GPT and its variants (GPT-2, GPT-4, ChatGPT) mentioned multiple times in the same passage . - Agentic AI MOOC Evolution of System Designs by Ya…

**VI agent (Sonnet 5)** — quality 0.00, P 1.00, R 1.00, facts 0.00, timestamps near a mention 1.00 (n=7), $0.201, 50 s
  
Judge: The answer correctly puts GPT/ChatGPT first but gives no mention counts or talk counts, and it misranks the rest by elevating Claude to a tie with Gemini and placing Llama last instead of third. All three cited videos exist in the catalog with timestamps, though the Clay Bavor citation is truncated with no evidence, and the answer's overall ranking claims about Gemini/Claude/DeepSeek prevalence are unsupported by any specific citations.
  
> Based on hybrid search across the whole library, GPT/ChatGPT is clearly the most-mentioned model family, appearing constantly across nearly every talk (including an entire session built around GPT/ChatGPT). The rest rank roughly: Gemini and Claude next (both mentioned heavily in several dedicated demo/workshop segments), then DeepSeek, then Llama least often (though still present in several deep-technical talks). Ranking: 1. GPT/ChatGPT → 2. Gemini ≈ Claude → 3. DeepSeek → 4. Llama GPT / ChatGPT - How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS — Ishan Anand — [00:17:27], [01:23:18], [01:29:36], [01:32:03] — The whole talk is a deep dive contrasting GPT-2 with ChatGPT: "ChatGPT, L…

**VI agent (Gemini 3.8 Flash)** — quality 0.00, P 1.00, R 1.00, facts 0.00, timestamps near a mention 1.00 (n=3), $0.170, 44 s
  
Judge: The answer declares that no library content provides the ranking and instead cites three real catalog videos (Ishan Anand, James Zou, Yangqing Jia) with specific timestamps, none of which establishes the cross-corpus mention counts requested. No invented videos are claimed, but none of the three key facts (GPT first, Gemini second, Llama third with Claude/DeepSeek trailing) is conveyed, and the Zou attribution frames Claude as co-most-common, contradicting the ground truth ordering.
  
> Nothing in the video library contains an analysis or ranking of which model family is named most often across all the talks. However, individual talks discuss the relative usage or family relationships among these models: How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand: [00:17:27], the speaker points out a family tree diagram showing that popular models such as ChatGPT, Llama, Bard/Gemini, GPT-4, and Claude all descend from the foundational architecture of GPT-2 . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 AI Agents to Automate Science by James Zou: [00:47:22], a slide summarizes model usage across workshop paper submissions, showing GPT-4/5 and Claude as the mo…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.91 (n=82), $0.181, 62 s
  
Judge: The answer gives the correct ordering (GPT/ChatGPT first, Gemini second, Llama third, with Claude and DeepSeek trailing at similar levels) and supports it with 22 real catalog videos, each with specific timestamps, so no videos are invented and none are left untimestamped. Counts deviate somewhat from ground truth (Gemini ~55 vs ~90, talk counts inflated), and some citations look shaky (e.g., a malformed [00:66:10] timestamp and possibly loose attributions like Llama in the Azure talk), but no clear misattribution to the wrong speaker is evident.
  
> Across all the talks in the library, GPT/ChatGPT is the model family named most often by a substantial margin, leading in both total mention count (~180+ mentions) and talk presence (named in 20 talks). Overall Ranking (across all talks): 1. GPT / ChatGPT (~180+ mentions across 20 talks) 2. Gemini (~55 mentions across 14 talks) 3. Llama (~52 mentions across 10 talks) 4. Claude (~39 mentions across 12 talks) 5. DeepSeek (~34 mentions across 9 talks) --- Evidence from Relevant Talks: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han (02:42:27) [00:07:45]: Daniel Han discusses how the release of ChatGPT in December 2022 demonstrated that instruction …
