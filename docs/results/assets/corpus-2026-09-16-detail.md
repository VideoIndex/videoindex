# Questions, ground truth and answers

For each question: the expected videos (and acceptable extras), then every system's score, the videos it named wrongly or missed, the judge's two-sentence assessment and the opening of its answer.

## corpus-01: Find all videos that mention Anthropic.

*Kind: mention.* Spoken mentions define the expected set; four more talks only show the word on a slide or model id and are acceptable.

Expected: Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Agentic AI Safety & Security by Dawn Song; [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Multi-Agent AI by Noam Brown; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; Evals 101 — Doug Guthrie, Braintrust; Full Workshop: Realtime Voice AI — Mark Backman, Daily; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia
  
Acceptable, not required: How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust); [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS; From Mixture of Experts to Mixture of Agents with Super Fast Inference - Daniel Kim & Daria Soboleva

**VI agent (Sonnet 5)** — quality 0.80, P 1.00, R 0.67, timestamps near a mention 0.94 (n=16), $0.577, 55 s
  
Missed: How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; Evals 101 — Doug Guthrie, Braintrust; Full Workshop: Realtime Voice AI — Mark Backman, Daily
  
Judge: The answer lists ten real catalog videos, each with specific timestamps and quoted evidence, and no invented titles appear. One entry is labeled with the wrong talk title (Oriol Vinyals' multi-agent lecture) though the answer self-corrects that the Cognition/Anthropic blog-post content belongs to Noam Brown, leaving a minor attribution ambiguity.
  
> Based on my search across the library, here are all videos that explicitly mention "Anthropic": - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [01:24:46] and [01:44:37] The lecturer cites "Anthropic Constitutional Classifiers" as an example runtime input-guardrail defense, and later notes her team's CyberGym benchmark work "has been included in the system cards for Anthropic's latest model release, Sonnet 4.5" . - VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS — [00:54:53]–[01:03:06] On-screen code repeatedly shows AWS Bedrock model IDs like "us.anthropic.claude-3-7-sonnet" and the word "ANTHR…

**VI agent (Gemini 3.8 Flash)** — quality 0.88, P 1.00, R 0.78, timestamps near a mention 1.00 (n=14), $0.191, 30 s
  
Missed: How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; Evals 101 — Doug Guthrie, Braintrust
  
Judge: The answer names ten real catalog videos, all mapped correctly and each with at least one specific timestamp, so no videos are invented or left unlocated. It hedges that visual/OCR coverage may be incomplete, and the attributions appear plausible though some (e.g., Tau-bench, Notion slide) cannot be verified here.
  
> Here are the videos in the library confirmed to mention or display Anthropic: Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song [01:24:46] / [01:44:37]: The speaker highlights Anthropic's constitutional classifiers and notes that their research was included in the system card for Anthropic's Claude 3.5 Sonnet release [[cite:01M296373YNY9A2442ZWNZK0WT:5086-5095], [cite:01M296373YNY9A2442ZWNZK0WT:6275-6285]]. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia [00:14:13] / [00:31:26]: The speaker references Anthropic when discussing the popularity of closed-source model providers and the debate over whether compani…

**VI retrieval-only (Sonnet 5)** — quality 0.00, P 0.00, R 0.00, timestamps near a mention 1.00 (n=2), $0.054, 18 s
  
Missed: Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Agentic AI Safety & Security by Dawn Song; [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Multi-Agent AI by Noam Brown; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; Evals 101 — Doug Guthrie, Braintrust; Full Workshop: Realtime Voice AI — Mark Backman, Daily; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia
  
Judge: The answer identifies only one library video (Suman Debnath's VoiceVision RAG talk) with a specific timestamp range, and explicitly dismisses the other retrieved candidates as false positives. No invented videos appear, though the answer is hedged and likely incomplete in recall, admitting it could not search exhaustively for other Anthropic/Claude mentions.
  
> Based on the search results, I found clear evidence of an Anthropic mention in one video: - VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS — [00:51:36]–[00:53:02]: On-screen text/code shows the AWS Bedrock model identifier "us.anthropic.claude3-7-sonnet-...-v1:0" being used in the workshop's multimodal agentic RAG system, i.e., the presenter is invoking Anthropic's Claude 3.7 Sonnet model via AWS Bedrock . The other search hits returned (ComfyUI workshop, ElevenLabs conversational AI workshop, Peter Stone's Autonomous Agents lecture, Yann Dubois' LLM Agents Overview, Sida Wang's benchmarks talk, Weizhu Chen's Training Agentic Models) did n…

**VI agent (Sonnet 5, 1.5k cap)** — quality 0.00, P 0.00, R 0.00, timestamps near a mention 1.00 (n=5), $0.639, 74 s
  
Missed: Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Agentic AI Safety & Security by Dawn Song; [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Multi-Agent AI by Noam Brown; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; Evals 101 — Doug Guthrie, Braintrust; Full Workshop: Realtime Voice AI — Mark Backman, Daily; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia
  
Judge: The answer names only one library video (VoiceVision RAG by Suman Debnath), correctly mapped and with specific timestamp ranges, so no videos are invented. However, for a 'find all videos' query it is likely incomplete, and the response is truncated mid-citation, suggesting other Anthropic mentions across the library were missed.
  
> Based on multiple searches across the library, here are all the videos where "Anthropic" is explicitly mentioned (spoken or shown on screen): - VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS — [00:53:26]–[00:56:00], [00:50:08]–[00:53:02], [01:02:05] — On-screen code and slides repeatedly show "ANTHROPIC" branding and model IDs like "us.anthropic.claude-3-7-sonnet" while the presenter builds a Bedrock-based multimodal RAG agent using Claude Sonnet 3.7 via AWS Bedrock [[cite:01M29BFEMHGPPD

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=16), $2.825, 111 s
  
Judge: The answer names 11 catalog videos, all mapping cleanly to real library entries with specific bracketed timestamps and plausible Anthropic-related evidence, and invents no videos (the parenthetical values after titles appear to be durations, not claims). Minor issues: some entries are labeled with odd "Video 13/14/16" prefixes, the final count of 11 excludes nothing important, and one Pipecat mention is attributed to "Aleix" rather than the listed speaker, which may be a misattribution.
  
> Evals 101 — Doug Guthrie, Braintrust (00:48:30) [00:22:28] Doug Guthrie mentions that an automated prompt evaluation run was hindered by an issue with his Anthropic API keys while an Anthropic rate limit error is displayed on screen. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song (01:48:49) [01:24:58] Dawn Song discusses Anthropic's constitutional classifiers as an example of runtime input guardrails. [01:44:41] Dawn Song notes that their CyberGym evaluation work was included in the system card for Anthropic's model release. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown (00:54:49) [00:53:24] Noam Brown highlights two i…

## corpus-02: Which videos mention DeepSeek (any DeepSeek model, such as R1 or V3)?

*Kind: mention.* Daniel Han's ASR renders it 'DC R1' once but also says DeepSeek elsewhere.

Expected: Introduction to LLM serving with SGLang - Philip Kiely and Yineng Zhang, Baseten; [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments; [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois; A2A & MCP Workshop: Automating Business Processes with LLMs — Damien Murphy, Bench; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia
  
Acceptable, not required: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Post-Training Verifiable Agents by Jiantao Jiao; Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Agentic AI Safety & Security by Dawn Song; From Mixture of Experts to Mixture of Agents with Super Fast Inference - Daniel Kim & Daria Soboleva; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | AI Agents to Automate Science by James Zou

**VI agent (Sonnet 5)** — quality 0.73, P 1.00, R 0.57, timestamps near a mention 1.00 (n=10), $0.136, 24 s
  
Missed: [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; A2A & MCP Workshop: Automating Business Processes with LLMs — Damien Murphy, Bench
  
Judge: All five named videos map to real catalog entries, each with specific timestamps and concrete DeepSeek-related evidence, with no invented titles. The claims are plausible and well-attributed, though the accuracy of individual timestamps and the completeness of the list (e.g., other workshops possibly mentioning DeepSeek) cannot be verified here.
  
> Based on multiple searches across the library, here are the videos that mention DeepSeek models: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois — [01:02:06], [01:10:19], [01:12:29]: The lecturer discusses DeepSeek-R1 as an example of scalable SFT data generation via rejection sampling, walks through its RL training pipeline, and explains the GRPO algorithm it introduced, even recommending students read the DeepSeek R1 paper . - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han — [00:06:30], [02:22:16]–[02:32:20]: Daniel Han describes how DeepSeek R1's January 2025 release proved open-source models could match …

**VI agent (Gemini 3.8 Flash)** — quality 0.73, P 1.00, R 0.57, timestamps near a mention 1.00 (n=6), $0.152, 35 s
  
Missed: [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; A2A & MCP Workshop: Automating Business Processes with LLMs — Damien Murphy, Bench
  
Judge: The answer names six catalog videos, all real entries, each with a specific timestamp and a plausible DeepSeek-related description. No invented videos or missing timestamps, though correctness of individual attributions and completeness of the list cannot be verified from the answer alone.
  
> Here are the videos across the library that mention DeepSeek: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [00:07:26] : Daniel Han explains how the release of DeepSeek-R1 in January 2025 showed that open-source models could match proprietary reasoning models such as OpenAI's o1. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia [00:12:22] : Yangqing Jia highlights that DeepSeek was among the first papers demonstrating test-time scaling and reasoning capabilities, enabling open models to close the gap with proprietary systems. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Post-Training Verifiable Age…

**VI retrieval-only (Sonnet 5)** — quality 0.60, P 1.00, R 0.43, timestamps near a mention 1.00 (n=5), $0.037, 12 s
  
Missed: Introduction to LLM serving with SGLang - Philip Kiely and Yineng Zhang, Baseten; [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; A2A & MCP Workshop: Automating Business Processes with LLMs — Damien Murphy, Bench
  
Judge: The answer names three real catalog videos with specific timestamps and plausible DeepSeek-related content, with no invented titles. Attributions appear consistent with each speaker's topic, though the claim of DeepSeek-V2 detailing 'test-time scaling' may be a slight conflation, and the list may be incomplete relative to the full library.
  
> Based on the search results, here are the videos in the library that mention DeepSeek: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois — [00:05:44], [01:02:06], [01:10:19]: The lecturer discusses DeepSeek-R1 as the first open-source reasoning model, describes its "Reasoning RL" training stage, and explains how DeepSeek-R1 used rejection sampling based on verifiers for scalable SFT data generation on reasoning prompts like math and coding questions . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia — [00:10:42]: The speaker credits DeepSeek (and DeepSeek-V2) as among the first papers to detail test-time scaling …

**VI agent (Sonnet 5, 1.5k cap)**: no scored answer

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 0.94 (n=17), $2.515, 104 s
  
Judge: The answer names ten catalog videos, all mapping to real library entries, each with at least one specific timestamp and a short supporting quote or paraphrase. No invented videos appear, though some attributions (e.g., DeepSeek listed among multimodal architectures, and the SGLang speaker attribution) are plausible but unverified, and the many timestamps given for the Yann Dubois lecture are broad and could include imprecise hits.
  
> Below are the videos from the library that mention DeepSeek: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han (02:42:27): [00:07:27] – Daniel Han describes how the open-source community was lagging behind OpenAI's o1 until "in 2025 January, DeepSeek R1 came along and they released R1," proving that open-source models can achieve parity with frontier reasoning models. Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB (00:36:57): [00:17:48] – Apoorva Joshi notes that modern multimodal LLM architectures include models from DeepSeek, Anthropic (Claude), and OpenAI. A2A & MCP Workshop: Automating Business Processes with LLMs — Damien…

## corpus-03: Which talks mention Cursor, the AI code editor?

*Kind: mention.* Sida Wang says 'see the cursor?' about the mouse pointer (excluded: claiming it is wrong). The Vibe Coding workshop's 'but, like the cursor' is ambiguous.

Expected: Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Post-Training Verifiable Agents by Jiantao Jiao; How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust); RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI; Evals 101 — Doug Guthrie, Braintrust; A2A & MCP Workshop: Automating Business Processes with LLMs — Damien Murphy, Bench; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia
  
Acceptable, not required: [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments; VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | AI Agents to Automate Science by James Zou

**VI agent (Sonnet 5)** — quality 0.55, P 1.00, R 0.38, timestamps near a mention 1.00 (n=8), $0.239, 36 s
  
Missed: Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize; How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust); Evals 101 — Doug Guthrie, Braintrust; A2A & MCP Workshop: Automating Business Processes with LLMs — Damien Murphy, Bench; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen
  
Judge: The answer names five real catalog videos, each mapped correctly and each with specific timestamps, with no invented titles. Attributions are plausible and specific (slide text, UI elements, spoken quote), though correctness of individual timestamp evidence cannot be verified here.
  
> Based on my research, here are the talks that mention Cursor: - [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments — [00:57:52]–[00:58:16] The presenter demonstrates installing the Playwright MCP server and shows on-screen documentation/README text listing supported editors: "VS Code, Cursor, Wiz..." with steps for "Installin[g] Cursor" alongside VS Code . - RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI — [00:29:30], [00:29:34] While live-coding in a notebook, the on-screen editor UI displays a "Cursor Tab" autocomplete suggestion element, indicating the presenter is using the Cursor code editor for the demo . - Agentic AI MOOC UC Berkel…

**VI agent (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=15), $0.130, 32 s
  
Judge: The answer names nine catalog videos, all real entries with specific timestamps and brief evidence quotes, though its self-reported count of 9 conflicts with the ten bullet items listed (an internal inconsistency). Some cited ranges are implausibly wide (e.g., 00:20:38–01:15:28 for Aman Khan and 00:24:55–01:23:10 for Ilan Bigio), so attribution precision is questionable even though no videos appear invented.
  
> The following talks mention Cursor, the AI code editor: Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia [00:24:20] – [00:26:32]: Yangqing Jia notes the rapid adoption of AI coding tools like Cursor among prosumers who embed them into their everyday coding workflows [[cite:01M297H2QCHQMH7C6GPX6NE0V5:1455-1465], [cite:01M297H2QCHQMH7C6GPX6NE0V5:1588-1600]]. Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize [00:20:38] – [01:15:28]: Aman Khan explains how Cursor helped him write the code for an agent system using product documentation and encourages opening up codebase access via Cursor across engineering and product teams [[ci…

**VI retrieval-only (Sonnet 5)** — quality 0.00, P 0.00, R 0.00, $0.030, 11 s
  
Missed: Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Post-Training Verifiable Agents by Jiantao Jiao; How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust); RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI; Evals 101 — Doug Guthrie, Braintrust; A2A & MCP Workshop: Automating Business Processes with LLMs — Damien Murphy, Bench; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia
  
Judge: The answer concludes no library video mentions Cursor, and it names no video as a positive match (it only speculatively mentions 'Collaborating with Agents in your Software Dev Workflow' and 'Vibe Coding at Scale' as unverified possibilities). No invented videos or timestamps are given, but the response provides no evidence or citations at all, effectively a non-answer.
  
> Based on my search results, I did not find clear evidence that any indexed video explicitly mentions "Cursor" as the AI code editor. The search results returned included topics like video generation tools, ComfyUI, fine-tuning workflows, and Copilot instructions in a software development workflow talk , but none of the retrieved transcript, OCR, or description snippets specifically named "Cursor" the code editor. I was not able to run additional targeted searches in this turn to confirm further, so I cannot rule out that Cursor is mentioned elsewhere in the library (e.g., in developer-tooling talks such as "Collaborating with Agents in your Software Dev Workflow" or "Vibe Coding at Scale") —…

**VI agent (Sonnet 5, 1.5k cap)** — quality 0.67, P 1.00, R 0.50, timestamps near a mention 1.00 (n=11), $0.342, 64 s
  
Missed: How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust); Evals 101 — Doug Guthrie, Braintrust; A2A & MCP Workshop: Automating Business Processes with LLMs — Damien Murphy, Bench; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen
  
Judge: The answer names six catalog videos with mostly specific timestamps and plausible evidence, though the final entry ('Shipping AI That Works') is truncated with no timestamp or supporting evidence. One attribution is questionable: the Vibe Coding entry conflates a textual mention of "the cursor" (text caret) with the Cursor editor, and the OCR-only "Cursor Tab" claim in the OpenAI fine-tuning talk is weak evidence of a mention.
  
> Based on my search across the library, Cursor is mentioned or shown in the following talks: - RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI — [00:29:34], [00:29:30-00:30:07], [01:36:57-01:37:19] — On-screen OCR repeatedly shows "Cursor Tab" (Cursor's inline-completion feature) visible in the presenter's code editor while he works through a fine-tuning notebook . - [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments — [00:14:36], [00:58:00-00:58:16] — The presenter refers to "the cursor" while troubleshooting terminal execution and later shows a slide/demo captioned "VS Code, Cursor, Wiz" and "Installing Cursor" as one of the AI coding too…

**Gemini agentic video (3×10 + merge)** — quality 0.93, P 1.00, R 0.88, timestamps near a mention 0.94 (n=18), $0.081, 38 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen
  
Judge: All eight cited videos exist in the catalog and each claim carries specific timestamps, with no invented titles. However, the attribution for the Notion/Braintrust talk (6YdPI9YbjbI) credits Doug Guthrie with a Braintrust prompt-optimization demo that duplicates the Evals 101 claim, which looks like a misattribution/copy across entries.
  
> The following talks mention Cursor, the AI code editor: Evals 101 — Doug Guthrie, Braintrust (00:48:30) [00:21:44] While demonstrating Braintrust's new prompt optimization loop feature, Doug Guthrie compares its workflow to a Cursor-like interface. A2A & MCP Workshop: Automating Business Processes with LLMs — Damien Murphy, Bench (01:23:13) [00:40:37], [00:44:18] Damien Murphy notes that whenever he catches Cursor making an error it reflexively replies "you're absolutely right," and later explains that command-line interfaces are easier for coding tools like Cursor or Augment to write and test. RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI (01:46:14) [00:24:55], [00:26:15], [01…

## corpus-04: Which videos talk about poker?

*Kind: mention.* 

Expected: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Multi-Agent AI by Noam Brown; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone; Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals

**VI agent (Sonnet 5)** — quality 0.50, P 1.00, R 0.33, timestamps near a mention 0.86 (n=7), $0.225, 26 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone; Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals
  
Judge: The answer names a single real catalog video (Noam Brown's Multi-Agent AI lecture) with specific timestamp ranges, which is plausible given Brown's poker AI background, and invents no videos. It may under-report other videos that mention poker in passing, but the attribution and details given are coherent and appropriately timestamped.
  
> Based on my search of the transcripts, on-screen text, and descriptions across the library, only one video substantively discusses poker. - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown — [00:02:15]–[00:04:32], [00:08:00]–[00:08:58], [00:16:50]–[00:18:56], [00:22:09] Noam Brown (who led Libratus/Pluribus poker AI research) uses poker throughout the lecture as the central example of imperfect-information game theory: he opens by asking "who is the better poker player?" to distinguish minimax-equilibrium play from population best-response play ; he explains why poker strategy is more complex than rock-paper-scissors and quotes a "poker strategy guide" saying "Pok…

**VI agent (Gemini 3.8 Flash)** — quality 0.80, P 1.00, R 0.67, timestamps near a mention 1.00 (n=6), $0.078, 17 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone
  
Judge: The answer names two real catalog videos (Noam Brown's Multi-Agent AI and Oriol Vinyals' Multi-Agent Systems), both with specific timestamps and no invented entries. Attributions are plausible—poker is central to Brown's work and imperfect-information games fit Vinyals' StarCraft discussion—though the timestamp precision cannot be verified here.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown — [00:00:27], [00:02:23], [00:07:56], [00:17:02], [00:22:08] Noam Brown frequently references poker throughout his lecture to illustrate game-theoretic concepts, discussing self-play AI milestones , evaluating what defines a better poker player , explaining minimax equilibria and bluffing frequencies [[cite:01M296FBHQD7SWTC6TZ59NSMQ9:476-555], [cite:01M296FBHQD7SWTC6TZ59NSMQ9:1017-1036]], and detailing how his research defeated world-class poker professionals in heads-up no-limit Texas Hold'em . Agentic AI MOOC UC Berkeley CS294-196 F25 Multi-Agent Systems in Era of LLMs by Oriol Vinyals — [00:36:12] Oriol Vinyals c…

**VI retrieval-only (Sonnet 5)** — quality 0.00, P 0.00, R 0.00, $0.029, 11 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Multi-Agent AI by Noam Brown; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone; Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals
  
Judge: The answer concludes no video in the library discusses poker and explicitly declines to cite Noam Brown's talk, offering it only as unconfirmed speculation rather than a claim, so no video claims are extracted. No invented videos appear and no timestamps are given, but the answer likely misses genuine poker content in Noam Brown's multi-agent talk.
  
> Based on the searches performed, I did not find clear evidence of any video in this library actually discussing poker. The search results returned only incidental matches on the word "video" (via OCR) rather than any content about poker, card games, or poker-playing AI systems . Note: Given that Noam Brown (creator of Libratus/Pluribus poker AI) has a talk in this library ("Multi-Agent AI by Noam Brown"), it's plausible poker is discussed there, but my searches did not surface a direct transcript or OCR match confirming this, so I cannot cite it with confidence. Videos: 0

**VI agent (Sonnet 5, 1.5k cap)** — quality 0.50, P 1.00, R 0.33, timestamps near a mention 0.88 (n=8), $0.382, 39 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone; Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals
  
Judge: The answer names one real catalog video (Noam Brown's Multi-Agent AI lecture) with multiple specific timestamps, which is plausible given his poker-AI background, and invents no videos. It also explicitly rules out other talks as irrelevant, though the abundance of precise timestamps and quoted lines cannot be verified here and some may be approximate or misplaced.
  
> Based on my search, the poker discussion in this video library is concentrated almost entirely in one lecture. - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown — [00:02:15], [00:08:00], [00:17:17], [00:22:09], [00:14:00–00:16:50], [00:22:09–00:22:14] Noam Brown (who did his PhD on poker AI) uses poker extensively as a running example: he opens by asking "who is the better poker player?" to introduce the difference between a minimax-equilibrium strategy and a population best-response strategy ; he explains why poker requires more complex randomized strategies than rock-paper-scissors, quoting a poker strategy guide ("Poker is simple. As your opponents make mistak…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=9), $0.046, 26 s
  
Judge: The answer names three real catalog videos (Noam Brown, Peter Stone, Oriol Vinyals), each with specific timestamps and plausible poker-related descriptions, with no invented videos. Attributions appear consistent with the speakers' known topics, though the timestamp accuracy and the Vinyals/Stone mentions cannot be confirmed here.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown (00:54:49): [00:00:29], [00:02:22], [00:08:00], [00:17:08], [00:21:55] — Noam Brown uses poker as a prime example of an imperfect-information game to explain concepts such as minimax equilibria versus population best response, the value of bluffing probabilities, and how regret-matching algorithms from his PhD work defeated top human Texas Hold'em poker professionals. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Autonomous Agents by Peter Stone: [00:09:21]: Peter Stone cites poker alongside chess and Go as examples of tasks where AI systems have surpassed the best human players. [00:13:58]: He contrasts real-time…

## corpus-05: Which videos discuss prompt injection attacks?

*Kind: mention.* 

Expected: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Agentic AI Safety & Security by Dawn Song; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor

**VI agent (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 0.82 (n=17), $0.292, 25 s
  
Judge: The answer names only two catalog videos (Dawn Song's safety lecture and Clay Bavor's talk), both real and plausibly attributed, each with specific timestamps. It includes leftover reasoning chatter and a duplicated timestamp, and may be under-inclusive of other security/agent talks, but no invented videos or clearly misattributed content appear.
  
> Found another relevant video: "Practical Lessons from Deploying...by Clay Bavor" discusses prompt injection attacks and guardrails. Let me check the Azure AI Agent Service video too, and a couple other likely candidates (agent security-focused talks).I have sufficient evidence for two distinct videos. Let me do one more check for other candidate talks (e.g., MCP workshop, Multi-Agent Safety talks) to be thorough.Based on the searches, prompt injection attacks are discussed substantively in two videos in the library. - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [00:28:34], [00:31:56], [00:34:38], [00:40:15], [00:44:38], [00:48:29], [00:51:52], …

**VI agent (Gemini 3.8 Flash)** — quality 0.67, P 1.00, R 0.50, timestamps near a mention 1.00 (n=4), $0.152, 36 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor
  
Judge: The answer names a single real catalog video (Dawn Song's Agentic AI Safety & Security talk) with specific timestamp ranges, which is a plausible and on-topic attribution for prompt injection discussion. No invented videos appear, though the answer may be incomplete since other talks (e.g., agent/MCP workshops) could also touch on prompt injection.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [00:28:33]–[00:45:20], [01:25:26]–[01:31:48] Dawn Song discusses both direct and indirect prompt injection attacks, demonstrating how untrusted inputs (such as malicious prompts, calendar invites, or resumes) can override system instructions to leak sensitive data or execute unauthorized agent actions, as well as defenses like PromptArmor and dynamic security policies to mitigate them [[cite:01M296373YNY9A2442ZWNZK0WT:1713-1725], [cite:01M296373YNY9A2442ZWNZK0WT:2078-2095], [cite:01M296373YNY9A2442ZWNZK0WT:2680-2696], [cite:01M296373YNY9A2442ZWNZK0WT:5508-5520]]. Videos: 1

**VI retrieval-only (Sonnet 5)** — quality 0.67, P 1.00, R 0.50, timestamps near a mention 1.00 (n=2), $0.030, 9 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor
  
Judge: The answer names a single catalog video (Dawn Song's Agentic AI Safety & Security) with a precise timestamp range and a plausible, specific description of prompt injection content, and it explicitly dismisses other search hits as irrelevant. No invented videos appear, though the answer may be under-inclusive if other talks also touch on prompt injection.
  
> Based on my searches, only one video in the library contains clear, substantive discussion of prompt injection attacks: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [00:32:23]–[00:34:39] The lecturer explains categories of prompt injection attack methods (heuristic-based and optimization-based) and introduces indirect prompt injection attacks with a diagram showing an attacker, external resources, an LLM-integrated application, and a user, contrasting them with direct prompt injection attacks where the attacker needs direct access to the prompt . Other hits returned by search (e.g., Peter Stone's "Autonomous Agents" talk, ComfyUI workshop, Ele…

**VI agent (Sonnet 5, 1.5k cap)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 0.71 (n=14), $0.436, 35 s
  
Judge: Both named videos exist in the catalog and are plausibly on-topic (Dawn Song's safety/security lecture and Clay Bavor's deployment lessons), each with specific timestamps rather than vague references. No invented videos appear, though the answer may be under-inclusive by omitting other talks that briefly touch prompt injection, and the numerous Dawn Song timestamps are asserted without individually distinguishable evidence.
  
> Based on my searches, prompt injection attacks are discussed in depth in two videos of the library: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [00:22:41] [00:27:59] [00:34:38] [00:44:41] [00:49:23] [01:31:03] [01:32:07] [01:38:00] [01:39:57] [01:44:24] [01:45:29] [01:51:38] The lecture extensively covers direct prompt injection (e.g., a user tricking a system into leaking its system prompt via "ignore previous instructions") , indirect prompt injection via a poisoned resume tricking an LLM hiring agent , real-world attack case studies including a malicious file exploiting Claude Code (CVE-2025-55284) to exfiltrate API keys , a malicious cale…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=4), $0.068, 40 s
  
Judge: The answer names two real catalog videos with specific timestamps, both plausibly on-topic for prompt injection (Dawn Song's safety/security lecture and Clay Bavor's deployment lessons). No invented videos appear, though the list may be incomplete given other agent-security-adjacent talks, and the timestamp precision cannot be verified here.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song - [00:28:35] - [00:44:45]: Dawn Song explains direct and indirect prompt injection attacks, demonstrating how malicious user inputs or untrusted third-party data (such as resumes, local files, or calendar invites) can hijack model execution to leak sensitive prompts, exfiltrate credentials, or trigger unintended actions. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Practical Lessons from Deploying...by Clay Bavor - [00:09:36], [00:43:08]: Clay Bavor discusses the critical challenge of protecting customer-facing agents from prompt injection attacks, sharing examples from red teaming and outlining Sier…

## corpus-06: Which talks mention LoRA (low-rank adaptation) fine-tuning?

*Kind: mention.* Whisper writes the acronym as 'Laura' in several talks; the patterns cover both spellings.

Expected: RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI; [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; ComfyUI Full Workshop — first workshop from ComfyAnonymous himself!; From Mixture of Experts to Mixture of Agents with Super Fast Inference - Daniel Kim & Daria Soboleva; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen
  
Acceptable, not required: The AI Engineer’s Guide to Raising VC — Dani Grant (Jam), Chelcie Taylor (Notable)

**VI agent (Sonnet 5)** — quality 0.75, P 1.00, R 0.60, timestamps near a mention 0.67 (n=9), $0.110, 15 s
  
Missed: ComfyUI Full Workshop — first workshop from ComfyAnonymous himself!; From Mixture of Experts to Mixture of Agents with Super Fast Inference - Daniel Kim & Daria Soboleva
  
Judge: All three named videos map to real catalog entries (Daniel Han's workshop, Weizhu Chen's lecture, Ilan Bigio's fine-tuning talk) with specific, plausible timestamps and coherent LoRA-related evidence. No invented videos or obvious misattributions, though the answer may be incomplete if other talks (e.g., other fine-tuning sessions) also mention LoRA.
  
> I have enough evidence now. The three relevant videos are clear from the searches. - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han — [01:57:26], [02:26:58], [02:27:38], [02:28:46] Daniel Han demos Unsloth notebooks setting lorarank, loraalpha, and maxlorarank parameters (e.g. "lorarank = 32, Larger rank = smarter, but slower") while loading models for GRPO fine-tuning . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Training Agentic Models by Weizhu Chen — [00:44:46]–[00:48:31] Weizhu Chen has a dedicated segment "Why LoRA in RL Training?" explaining that LoRA acts as regularization, works well with reinforcement learning, and offers GPU/co…

**VI agent (Gemini 3.8 Flash)** — quality 0.89, P 1.00, R 0.80, timestamps near a mention 1.00 (n=12), $0.126, 27 s
  
Missed: From Mixture of Experts to Mixture of Agents with Super Fast Inference - Daniel Kim & Daria Soboleva
  
Judge: All four named videos map to real catalog entries, each with specific timestamp ranges and plausible LoRA-related content for their topics (Unsloth workshop, OpenAI fine-tuning, agentic model training, ComfyUI image generation). No invented videos or untimestamped mentions, though the exact timestamp accuracy and the Weizhu Chen LoRA-in-RL attribution cannot be verified from the given information.
  
> The following talks mention LoRA (low-rank adaptation) fine-tuning: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [02:03:07]–[02:04:10] : Daniel Han describes utilizing LoRA as a parameter-efficient fine-tuning method to avoid the cost of tuning every weight, advising practitioners to set loraalpha to twice the lorarank for faster training. RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI [00:11:46]–[00:12:15] , [01:03:09]–[01:03:36] : Ilan Bigio introduces low-rank adaptation as training a subset of decomposed matrix weights and clarifies during Q&A that OpenAI fine-tuning generates a LoRA component that gets loaded alongside base …

**VI retrieval-only (Sonnet 5)** — quality 0.57, P 1.00, R 0.40, timestamps near a mention 0.75 (n=4), $0.034, 11 s
  
Missed: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; ComfyUI Full Workshop — first workshop from ComfyAnonymous himself!; From Mixture of Experts to Mixture of Agents with Super Fast Inference - Daniel Kim & Daria Soboleva
  
Judge: The answer names two real catalog videos with specific timestamp ranges and no invented titles, and it explicitly dismisses ComfyUI and Peter Stone's talk as false matches. Attributions appear plausible and well-scoped, though it may under-report other talks (e.g., Daniel Han's fine-tuning workshop) that likely discuss LoRA.
  
> Based on the search results, only two talks contain clear, specific mentions of LoRA (low-rank adaptation) fine-tuning: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Training Agentic Models by Weizhu Chen — [00:44:46]–[00:48:31]: The speaker presents a slide titled "LoRA on RL" and explains why LoRA works effectively in reinforcement learning training, describing it as a form of regularization that offers generalization and GPU-efficiency benefits . - RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI — [00:11:36]–[00:13:43]: Ilan Bigio explains that fine-tuning methods like LoRA ("low rank adaptation") only train a small subset of model weights and are not well suited for teach…

**VI agent (Sonnet 5, 1.5k cap)** — quality 0.75, P 1.00, R 0.60, timestamps near a mention 0.89 (n=9), $0.189, 20 s
  
Missed: ComfyUI Full Workshop — first workshop from ComfyAnonymous himself!; From Mixture of Experts to Mixture of Agents with Super Fast Inference - Daniel Kim & Daria Soboleva
  
Judge: The answer names three real catalog videos (Daniel Han's workshop, Weizhu Chen's lecture, Ilan Bigio's fine-tuning talk) with specific timestamps and plausible LoRA-related content, no invented videos. Attributions look consistent with each speaker's topic, though the claimed coverage may be incomplete since other fine-tuning-oriented talks could also mention LoRA.
  
> Based on the searches, three videos contain substantive mentions of LoRA fine-tuning: - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [01:59:06], [02:27:34], [02:28:44], [02:29:32], [02:36:52] — Daniel Han demonstrates Unsloth notebooks setting up LoRA parameters (lorarank, loraalpha, maxlorarank), explaining that "larger rank = smarter, but slower," and shows code saving/loading LoRA adapters (model.savelora, model.loadlora) during a GRPO reinforcement-learning fine-tuning demo . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Training Agentic Models by Weizhu Chen [00:44:46]–[00:48:31] — Weizhu Chen explains why LoRA (Low-Rank Adaptation) …

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 0.82 (n=11), $0.072, 44 s
  
Judge: All five named videos map to real catalog entries, each with specific timestamps and plausible LoRA-related content; no invented videos appear. One oddity is the stray duration-like '(01:04:36)' appended to the Weizhu Chen title, which may be a spurious timestamp, but otherwise attributions look coherent and specific.
  
> [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [00:44:34] Daniel Han notes that parameter-efficient fine-tuning methods like LoRA allow training a smaller subset of weights rather than updating all model parameters. [02:03:10] In the hands-on demonstration, he explains how the notebook uses LoRA to fine-tune added low-rank weights efficiently to save compute and memory, recommending setting alpha to twice the LoRA rank. [02:27:00] He points out that when training with LoRA, you need more than three data rows to prevent the gradients from becoming zero. RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI [00:12:08] Ilan Bigio introduces …

## corpus-07: Which video discusses Kubernetes, and in what context does the speaker bring it up?

*Kind: topic.* 

Expected: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia

Key facts:

- Only Yangqing Jia's 'Evolution of System Designs' lecture discusses Kubernetes.
- He brings it up as one of the choices for managing a GPU/compute cluster, as an alternative to a hand-managed cluster run by system administrators (with an aside that Oriol Vinyals was once one of those sysadmins).

**VI agent (Sonnet 5)** — quality 0.75, P 1.00, R 1.00, facts 0.50, timestamps near a mention 0.62 (n=8), $0.057, 15 s
  
Judge: The answer correctly identifies Yangqing Jia's 'Evolution of System Designs' as the only video discussing Kubernetes and provides multiple specific timestamp ranges, with no invented videos. However, it frames the context as Kubernetes vs Slurm/SRE mindsets and etcd anecdotes rather than the ground-truth framing of Kubernetes as a choice versus a hand-managed sysadmin cluster (including the Oriol Vinyals aside), so the second key fact is not conveyed.
  
> Based on the search results, Kubernetes is substantively discussed in one video in the library. - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia - [00:43:26–00:45:59] The speaker contrasts Kubernetes with Slurm as competing approaches to managing GPU clusters for AI research, describing it as "the holy war... between the Slurm researcher kind of mindset and the Kubernetes operations kind of mindset," noting AI researchers often resist it while site-reliability engineers favor it . - [00:46:12–00:47:54] He notes that companies eventually seek "AI native platforms" that abstract away "the Kubernetes jargon" so researchers can focus on their models r…

**VI agent (Gemini 3.8 Flash)** — quality 0.75, P 1.00, R 1.00, facts 0.50, timestamps near a mention 1.00 (n=5), $0.067, 15 s
  
Judge: The answer correctly identifies the single relevant video (Yangqing Jia's 'Evolution of System Designs') and supplies specific timestamps, with no invented videos. However, it frames the Kubernetes mention around Slurm/SRE mindset, etcd, and locality rather than the ground-truth context of choosing Kubernetes versus a hand-managed sysadmin cluster (including the Oriol Vinyals aside), so the second key fact is not conveyed and some details may be misattributed.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia Timestamps: [00:43:13] – [00:44:23], [00:46:13], [01:09:41], [01:18:01] Yangqing Jia brings up Kubernetes to contrast the operational/SRE mindset with the Slurm-oriented workflow of AI researchers who do not want to manage pods and deployments , notes how AI-native platforms try to abstract away Kubernetes jargon , explains that traditional VM and Kubernetes abstractions break down in AI training where rack and switch physical locality is essential , and shares that one of his startup co-founders created etcd, the underlying database of Kubernetes . Videos: 1

**VI retrieval-only (Sonnet 5)** — quality 0.75, P 1.00, R 1.00, facts 0.50, timestamps near a mention 1.00 (n=2), $0.050, 9 s
  
Judge: The answer correctly identifies Yangqing Jia's 'Evolution of System Designs' as the only video discussing Kubernetes and gives a concrete timestamp range, with no invented videos. However, the context it describes (Slurm vs. Kubernetes holy war) does not convey the ground-truth framing of Kubernetes as a GPU cluster management choice versus a hand-managed sysadmin cluster, nor the Oriol Vinyals aside, so the second fact is unmet.
  
> - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia — [00:42:34]–[00:45:32]: The speaker discusses infrastructure choices for compute/storage/networking and contrasts Kubernetes with Slurm, saying "I don't really want to deal with Kubernetes. This is the fight today, the holy war, I would say, between the Slurm researcher kind of mindset and the Kubernetes operations kind of mindset" as a debate within the ML systems community . I did not find other videos in the library where Kubernetes is discussed as a topic; the other search hits returned unrelated content (ComfyUI, ElevenLabs, agentic training lectures) with no mention of Kubernetes. Videos: 1

**VI agent (Sonnet 5, 1.5k cap)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=4), $0.089, 14 s
  
Judge: The answer correctly identifies the single relevant video (Yangqing Jia's 'Evolution of System Designs') with specific timestamps and no invented videos. It captures Kubernetes as a compute-cluster management choice contrasted with another approach, though it frames the alternative as Slurm/researcher mindset rather than the hand-managed sysadmin cluster and omits the Oriol Vinyals aside, and some added details (etcd co-founder, rack-locality) are unverified extras.
  
> Based on my search across the library, only one video substantively discusses Kubernetes. - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia [00:42:58–00:44:34] and [01:09:40] and [01:18:00] Yangqing Jia contrasts two philosophies for managing AI compute clusters: Kubernetes, which he calls "a very nice abstraction" favored by SREs/ops people, versus the Slurm-style mindset preferred by AI researchers who don't want to deal with Kubernetes concepts like pods, deployments, and parallelism, calling this tension "the holy war" between researcher and ops mindsets . He later notes that conventional cloud abstraction practices using VMs and Kubernetes don…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.50 (n=2), $0.158, 104 s
  
Judge: The answer correctly identifies the single relevant video (Yangqing Jia's 'Evolution of System Designs') with a specific timestamp and frames Kubernetes as one option for managing a compute cluster versus simpler alternatives. However, it omits the hand-managed/sysadmin-run cluster alternative and the Oriol Vinyals aside, and adds a quoted slide title and Slurm/MPI specifics that may be embellished; the leading 01:19:31 appears to be the video duration rather than a content timestamp.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia (01:19:31) [00:43:20] Under a slide titled "You shouldn't run on baremetal. Also, K8s is wrong.", Yangqing Jia discusses the divide between operations teams and researchers, explaining that while Kubernetes is an excellent abstraction for site reliability engineers (SREs), AI researchers dislike dealing with Kubernetes jargon (such as pods and deployments) and prefer simpler cluster setups like Slurm or MPI to run distributed PyTorch jobs. Videos: 1

## corpus-08: Which videos mention Notion, the company or its product?

*Kind: mention.* Three talks use 'notion' as an ordinary noun ('the notion that...'); claiming them is wrong.

Expected: How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust); Evals 101 — Doug Guthrie, Braintrust

**VI agent (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=6), $0.135, 19 s
  
Judge: The answer names only real catalog videos (6YdPI9YbjbI and bk0TmxoZlUY) with specific, plausible timestamps for each claim. Attributions are coherent — the Notion AI lead talk and the Braintrust Evals 101 customer-logo slides — though it may be incomplete if other talks mention Notion in passing.
  
> Based on the searches, I have found two videos that explicitly mention Notion (the company/product). - How to build world-class AI products — Sarah Sachs (AI lead @ Notion) & Carlos Esteban (Braintrust) [00:00:16], [00:02:56], [00:06:34], [00:07:51]: This entire talk is by Notion's AI lead; she introduces Notion as "a connected workspace" with over 100 million users, and walks through the evolution of Notion AI (AI Writer, autofill, Q&A/RAG, AI connectors, "Notion AI for Work"), noting "Notion AI actually came out before ChatGPT" . - Evals 101 — Doug Guthrie, Braintrust [00:02:28], [00:05:31]: Notion is listed on a slide of companies Braintrust supports ("We support leading AI teams" — logos…

**VI agent (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=5), $0.124, 19 s
  
Judge: The answer names two real catalog videos with specific, plausible timestamps and no invented entries. Attributions fit each video's known content (Notion AI talk and Braintrust Evals 101 customer slides), though the exact slide timestamps in Evals 101 cannot be verified here.
  
> Here are the videos in the library that mention Notion, the company or its product: - How to build world-class AI products — Sarah Sachs (AI lead @ Notion) & Carlos Esteban (Braintrust) - [00:01:13] Carlos Esteban introduces Sarah Sachs as the lead of Notion AI and notes that Notion has been a Braintrust customer since the beginning . - [00:03:02] Sarah Sachs presents Notion as a connected workspace connecting project management, documents, and tools like Slack and Google Drive for over 100 million users . - [00:06:38] Sarah outlines the product evolution and features of Notion AI, such as the AI Writer, autofill, enterprise search, and Notion AI for Work . - Evals 101 — Doug Guthrie, Braint…

**VI retrieval-only (Sonnet 5)** — quality 0.67, P 1.00, R 0.50, $0.035, 13 s
  
Missed: Evals 101 — Doug Guthrie, Braintrust
  
Judge: The answer effectively concludes it could not confirm any Notion mentions, naming only the Sarah Sachs/Notion talk as a title-based candidate without any timestamp or transcript evidence. No videos were invented, but the response is hedged and provides no usable timestamped citations, and it misses other likely mentions.
  
> Based on the searches I was able to run, none of the returned evidence (transcript, OCR, or frame hits) actually contained the word "Notion" or a clear reference to the company/product — the hits returned were unrelated matches (e.g., ComfyUI video generation, ElevenLabs speech-to-text, Berkeley MOOC lectures on training/safety/autonomous agents), not genuine mentions of Notion. The one strong candidate in the library is "How to build world-class AI products — Sarah Sachs (AI lead @ Notion) & Carlos Esteban (Braintrust)," since its title identifies Sarah Sachs as the AI lead at Notion, implying the company is discussed. However, my searches did not surface a specific timestamped transcript/O…

**VI agent (Sonnet 5, 1.5k cap)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=6), $0.321, 33 s
  
Judge: The answer names two real catalog videos with specific timestamps and no invented entries; the Notion talk attribution is clearly correct, while the Braintrust Evals 101 logo-slide claim is plausible but unverified here. Some stray meta-commentary about searching leaks into the response, but attributions and timestamps are concrete rather than vague.
  
> Both these are logo mentions in customer lists on slides (not spoken about specifically), but they are visible on-screen text mentioning Notion as a company/customer. Let me also check the other candidates more precisely, and search for any other mentions across the library.Based on these searches, I have identified two videos that clearly mention Notion. Let me confirm there's nothing else in other videos (e.g., Braintrust customer showcase in other talks).Based on the search results across the video library, two videos contain confirmed mentions of Notion (the company/product): - How to build world-class AI products — Sarah Sachs (AI lead @ Notion) & Carlos Esteban (Braintrust) [00:00:17],…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 0.50 (n=4), $0.172, 267 s
  
Judge: The answer names two real catalog videos with specific timestamps and no invented entries, though each entry oddly pairs a header duration-like timestamp with the cited moment. Attributions appear plausible—Notion is central to the Sarah Sachs talk and a plausible reference in the Braintrust Evals 101 session—but other potential mentions across the library may be missed.
  
> Evals 101 — Doug Guthrie, Braintrust (00:48:30) [00:39:03]: Doug Guthrie references Sarah from Notion, noting that she is a Braintrust customer and previously discussed how Notion approaches human-in-the-loop evaluations. How to build world-class AI products — Sarah Sachs (AI lead @ Notion) & Carlos Esteban (Braintrust) (01:43:45) [00:01:21]: Carlos introduces Sarah Sachs as the AI lead at Notion, who then gives a presentation detailing the development, launch, and evaluation of Notion AI products. Videos: 2

## corpus-09: Which talks mention StarCraft?

*Kind: mention.* 

Expected: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Multi-Agent AI by Noam Brown; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone; Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals

**VI agent (Sonnet 5)** — quality 0.80, P 1.00, R 0.67, timestamps near a mention 0.50 (n=14), $0.116, 16 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone
  
Judge: The answer names two real catalog videos (Vinyals' multi-agent LLM talk and Noam Brown's Multi-Agent AI talk), both mapped correctly with abundant specific timestamps and plausible, well-detailed evidence. No invented videos or untimestamped claims appear, though the sheer number of timestamps for the Vinyals talk cannot be verified individually.
  
> Based on my searches, StarCraft is mentioned in two talks in the library. - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent Systems in Era of LLMs by Oriol Vinyals [00:00:00, 00:05:26, 00:14:07, 00:15:47, 00:19:55, 00:26:40, 00:30:04, 00:33:20, 00:46:17, 00:48:51, 01:00:31, 01:02:57, 01:08:33]. Vinyals, who led DeepMind's AlphaStar project, discusses StarCraft extensively throughout: the title slide is "AlphaStar revisited" ; he recounts why StarCraft was hard for AI and his path to leading the project ; he describes AlphaStar's 2019 BlizzCon matches against pro player Serral ; he explains the supervised+RL+league training pipeline and StarCraft's API/action space compared to LLM…

**VI agent (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=3), $0.156, 25 s
  
Judge: The answer names three real catalog videos, each with a specific timestamp, and all three speakers plausibly discuss StarCraft given their backgrounds (Noam Brown on self-play, Peter Stone on game-playing agents, Oriol Vinyals as the AlphaStar lead). No invented videos or missing timestamps, though attributions and exact times cannot be verified here.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown: [00:00:28] — Noam Brown highlights that self-play has powered major AI breakthroughs in games such as AlphaGo, Dota, StarCraft, and poker . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Autonomous Agents by Peter Stone: [00:09:23] — Peter Stone notes that AI systems have surpassed the best human players in games ranging from chess, Go, and poker to StarCraft . Agentic AI MOOC UC Berkeley CS294-196 F25 Multi-Agent Systems in Era of LLMs by Oriol Vinyals: [00:05:59] — Oriol Vinyals discusses DeepMind's roadmap of mastering games and explains how tackling StarCraft served as a milestone due to its sheer complexity .…

**VI retrieval-only (Sonnet 5)** — quality 0.50, P 1.00, R 0.33, timestamps near a mention 0.75 (n=8), $0.033, 11 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Multi-Agent AI by Noam Brown; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone
  
Judge: The answer identifies a single relevant video that maps correctly to Oriol Vinyals' multi-agent talk, with multiple specific timestamp ranges, though it cites an opaque internal video_id rather than the catalog key. Content attributed (AlphaStar, StarCraft II RL exploration, imitation learning from replays) is plausible for that speaker; no invented videos are named, but its claim that no other talk mentions StarCraft may be overly restrictive.
  
> Based on the search results, only one video in the library contains clear, substantive references to StarCraft: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent Systems in Era of LLMs by Oriol Vinyals (videoid: 01M29FVH0TZTRJZ2BEDJWR4TWV) - [00:07:34]–[00:10:07] Vinyals describes how StarCraft II pioneered new video-game camera perspectives and recalls playing online against top-100 regional players in the DeepMind office as part of the AlphaStar project . - [00:19:49]–[00:22:46] He explains the exploration challenge in StarCraft II reinforcement learning, noting that pure RL alone tends to produce exploitative "all-in" worker-rush strategies, which motivated using imitation lea…

**VI agent (Sonnet 5, 1.5k cap)** — quality 0.80, P 1.00, R 0.67, timestamps near a mention 0.88 (n=8), $0.115, 16 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone
  
Judge: The answer names two real catalog videos (Vinyals' multi-agent LLM lecture and Noam Brown's multi-agent AI lecture), each with specific timestamps and plausible StarCraft-related content, with no invented videos. Attribution appears consistent with the speakers' known topics (Vinyals led AlphaStar; Brown works on self-play/poker), though the large number of timestamps for the Vinyals talk cannot be verified here.

  
> Two talks in the library discuss StarCraft substantially, both from UC Berkeley's Agentic AI MOOC series. - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent Systems in Era of LLMs by Oriol Vinyals — [00:05:26], [00:14:07], [00:26:40], [00:30:07], [00:48:51], [00:55:31], [00:54:47]: Vinyals (who led DeepMind's AlphaStar project) discusses StarCraft extensively as a benchmark of AI complexity, explains why StarCraft is hard for AI, describes the imitation learning + reinforcement learning + league self-play pipeline used to train AlphaStar, recounts AlphaStar's 2019 exhibition matches against pro players like Serral, and contrasts StarCraft's clean win/lose reward with the messier r…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 0.94 (n=16), $0.054, 28 s
  
Judge: All three named videos map to real catalog entries (Noam Brown, Peter Stone, Oriol Vinyals) with specific timestamps provided, and the StarCraft/AlphaStar attribution to Vinyals is highly plausible. The Noam Brown entry gives two conflicting times (a header 00:54:49 and a cited 00:00:28), which is internally inconsistent, and the long timestamp list for Vinyals is lumped under one unverified summary claim.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown (00:54:49) [00:00:28] Noam Brown notes that self-play has been behind many major breakthroughs in AI for games, citing examples such as AlphaGo, Dota, StarCraft, and poker. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Autonomous Agents by Peter Stone [00:09:22] Peter Stone notes that the audience had already heard about StarCraft earlier in the course as an example of AI systems outperforming human experts at complex games. Agentic AI MOOC UC Berkeley CS294-196 F25 Multi-Agent Systems in Era of LLMs by Oriol Vinyals [00:02:18], [00:06:04], [00:06:40], [00:07:23], [00:10:10], [00:13:06], [00:15:47], [00:20:07], [0…

## corpus-10: Which videos mention Hugging Face?

*Kind: mention.* 

Expected: Strategies for LLM Evals (GuideLLM, lm-eval-harness, OpenAI Evals Workshop) — Taylor Jordan Smith; [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois
  
Acceptable, not required: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia

**VI agent (Sonnet 5)** — quality 0.73, P 0.67, R 0.80, timestamps near a mention 0.57 (n=14), $0.601, 70 s
  
Wrong: Full Workshop: Realtime Voice AI — Mark Backman, Daily; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | AI Agents to Automate Science by James Zou
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois
  
Judge: The answer names seven real catalog videos, all with specific timestamps, and no invented videos appear. Some attributions are thin or possibly speculative (the James Zou Paper2Agent Hugging Face reference and the Daily voice AI 'smol' logo mention read as weakly supported), and one timestamp for the Ishan Anand talk was garbled in transcription.
  
> Based on my search across the video library, the following videos mention Hugging Face: - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [00:02:37], [00:07:52], [02:00:39], [02:22:38] — Daniel Han discusses Unsloth's practice of uploading dynamic quantized and bug-fixed models to Hugging Face, and shows how to export fine-tuned models to Hugging Face and use Hugging Face's Open R1 Math dataset for training . - VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS [00:35:20], [00:39:12], [01:02:59] — The presenter logs into a Hugging Face account with a token, loads a ColPali model and pre-processor …

**VI agent (Gemini 3.8 Flash)** — quality 0.91, P 0.83, R 1.00, timestamps near a mention 0.90 (n=10), $0.149, 33 s
  
Wrong: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | AI Agents to Automate Science by James Zou
  
Judge: All seven cited videos map to real catalog entries, each with specific timestamps and plausible Hugging Face-related evidence, and no videos were invented. Some timestamps appear derived from citation offsets rather than the stated bracket times (e.g., the AWS and vanilla-JS talks), so exact placement may be slightly off, but attributions to speakers and topics look internally consistent.
  
> [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [00:00:59], [00:01:41] Daniel Han mentions collaborating with the Hugging Face team to fix model bugs and highlights that Unsloth surpassed 10 million monthly downloads on Hugging Face . VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS [00:36:56], [00:37:37], [00:39:36] Suman Debnath explains how to log into Hugging Face using a token and load vision-language models and processors hosted on the Hugging Face Hub [[cite:01M29BFEMHGPPDX3PJRNTKG0Q1:2216-2263], [cite:01M29BFEMHGPPDX3PJRNTKG0Q1:2376-2385]]. Strategies for LLM Evals (GuideLLM, lm-eval-har…

**VI retrieval-only (Sonnet 5)** — quality 0.57, P 1.00, R 0.40, timestamps near a mention 1.00 (n=3), $0.056, 13 s
  
Missed: Strategies for LLM Evals (GuideLLM, lm-eval-harness, OpenAI Evals Workshop) — Taylor Jordan Smith; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois
  
Judge: The answer names two real catalog videos (Daniel Han's workshop and Suman Debnath's VoiceVision RAG) with specific timestamps and no invented titles. It explicitly excludes other search hits as false positives, which is plausible but may under-report other videos that mention Hugging Face (e.g., ComfyUI or SGLang talks).
  
> Based on the search results, clear mentions of "Hugging Face" appear in two videos: - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han — [00:02:37] On-screen text reads "Hugging Face," and Daniel Han explains Unsloth's practice of uploading dynamic quantized and bug-fixed AI models to Hugging Face, using DeepSeek R1 0528 as an example . - VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS — [00:37:09] and [00:52:59] The presenter walks through logging into a Hugging Face account and using a Hugging Face token to load the ColPali multimodal document retrieval model, and later on-screen text again sh…

**VI agent (Sonnet 5, 1.5k cap)** — quality 0.33, P 1.00, R 0.20, timestamps near a mention 0.67 (n=3), $0.437, 75 s
  
Missed: Strategies for LLM Evals (GuideLLM, lm-eval-harness, OpenAI Evals Workshop) — Taylor Jordan Smith; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois
  
Judge: The answer names only one catalog video (Daniel Han's workshop) with three specific timestamps, all plausibly relevant to Hugging Face mentions, and no invented videos are cited. However, the response is truncated mid-citation and likely incomplete, since many other library talks (e.g., fine-tuning, serving, evals workshops) plausibly mention Hugging Face but are not addressed.
  
> Based on searches across the library, here are the videos that mention or show "Hugging Face": - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [00:02:37], [02:00:09], [02:17:34] — Daniel Han explains that Unsloth uploads dynamic quantized/bug-fixed models to Hugging Face, later shows Unsloth using "Hugging Face's Open R1 Math dataset" for training data and demonstrates exporting a fine-tuned model "to GGUF, Ollama, vLLM or Hugging Face" [[cite:01M2920GDE5E53XCV

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=9), $0.823, 107 s
  
Judge: The answer names five catalog videos, all correctly mapped, each with specific timestamps and plausible Hugging Face-related evidence, with no invented videos. Attributions appear coherent with the speakers and topics, though the summary count of 5 undercounts nothing and coverage may be incomplete relative to other library talks that likely reference Hugging Face.
  
> [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han (02:42:27) [00:01:00]: Daniel Han mentions working directly with Hugging Face, Google, Meta, and Mistral teams to fix bugs in open-source models. [00:01:50]: He notes that Unsloth surpassed 10 million monthly downloads on Hugging Face. [00:02:37]: He explains that they upload fixed models and small quantizations to their Hugging Face page. [00:12:40]: He refers to model naming conventions (like instruct, chat, PT, and base) that users will encounter when looking at models on Hugging Face. [02:01:20]: While discussing top open-source model serving libraries like vLLM and SGLang, he notes that Huggin…

## corpus-11: Which speakers mention Kaggle?

*Kind: mention.* 

Expected: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals

**VI agent (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 0.80 (n=5), $0.050, 10 s
  
Judge: The answer names two real catalog videos (Daniel Han's workshop and Oriol Vinyals' MOOC lecture) with plausible speaker attributions and timestamps. However, the first entry's timestamp is messy and self-contradictory ("00:08:08 - 00:00:41... actually 02:28:08"), showing uncertainty in localization, and no invented videos appear.
  
> Based on the search results, two videos actually mention Kaggle explicitly: - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han — [00:08:08 - 00:00:41... actually 02:28:08] — [02:28:08] Daniel Han tells attendees to use Kaggle's free GPUs for running large models, repeating "Kaggle" multiple times while pointing it out on screen . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent Systems in Era of LLMs by Oriol Vinyals — [00:56:37] Oriol Vinyals notes there is a "Game Arena" on Kaggle that is popular for evaluating LLMs playing games, mentioning some models even try to complete Pokémon . Videos: 2

**VI agent (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=4), $0.058, 16 s
  
Judge: The answer names two real catalog videos with specific timestamps and plausible Kaggle-related evidence, with no invented videos. Attributions appear consistent with each speaker's topic, though the Vinyals claim rests on a single timestamp without a supporting citation, so its accuracy is less verifiable.
  
> [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han Timestamps: [00:02:09], [02:01:45], [02:28:14] Speaker Daniel Han highlights that Kaggle offers 30 hours of free GPU compute per week without restrictions and mentions Unsloth notebooks available on Kaggle [[cite:01M2920GDE5E53XCVRSC3R8SPA:129-148], [cite:01M2920GDE5E53XCVRSC3R8SPA:7303-7323], [cite:01M2920GDE5E53XCVRSC3R8SPA:8888-8925]]. Agentic AI MOOC UC Berkeley CS294-196 F25 Multi-Agent Systems in Era of LLMs by Oriol Vinyals Timestamp: [00:56:30] Speaker Oriol Vinyals discusses using games as evaluation benchmarks for LLMs, citing a popular game arena hosted on Kaggle . Videos: 2

**VI retrieval-only (Sonnet 5)** — quality 0.00, P 0.00, R 0.00, $0.025, 7 s
  
Missed: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals
  
Judge: The answer claims no video in the library mentions Kaggle and names no specific videos or timestamps, so there are no invented videos or misattributions. It is a clean refusal, though it likely misses actual mentions since it relies only on failed searches.
  
> Based on the searches performed, none of the returned evidence (transcripts, OCR, or descriptions) actually contain the word "Kaggle" or a clear reference to the Kaggle platform/competitions. The hits returned were topically unrelated (video APIs, keyword counting code, human-in-the-loop learning, etc.) and did not surface any speaker discussing Kaggle. I could not find any video in the library where a speaker mentions Kaggle. Videos: 0

**VI agent (Sonnet 5, 1.5k cap)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=2), $0.049, 9 s
  
Judge: The answer names two real catalog videos with specific timestamps and plausible speaker attributions, with no invented entries. Both claims are precise, though the Vinyals Kaggle Game Arena attribution and exact timestamps would need verification, and the answer may be incomplete if other speakers also mention Kaggle.
  
> Based on my search, two videos have clear mentions of Kaggle by name: - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han — [02:28:08] Daniel Han recommends using Kaggle's free GPUs for training/fine-tuning models, showing the Kaggle notebooks page on screen . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent Systems in Era of LLMs by Oriol Vinyals — [00:56:37] Oriol Vinyals mentions a "Game Arena" on Kaggle that is popular for evaluating LLM-based multi-agent systems . Videos: 2

**Gemini agentic video (3×10 + merge)** — quality 0.67, P 1.00, R 0.50, timestamps near a mention 0.80 (n=5), $0.065, 35 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals
  
Judge: The answer cites a single real catalog video (Daniel Han's workshop) with specific timestamps and a plausible Kaggle free-GPU claim, so no invented videos appear. However, it lists only one speaker, which may under-cover other talks mentioning Kaggle, and the leading (02:42:27) appears to be a duration rather than a citation point, making the timestamp set slightly ambiguous.
  
> [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han (02:42:27): [00:02:12] (also mentioned at [00:02:29], [02:01:51], and [02:28:40]) — Daniel Han mentions that Kaggle provides 30 hours of free GPU resources per week and encourages the audience to take advantage of their free notebooks on Kaggle. Videos: 1

## corpus-12: In which videos is GRPO (the RL algorithm) mentioned?

*Kind: mention.* 

Expected: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Post-Training Verifiable Agents by Jiantao Jiao; [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois

**VI agent (Sonnet 5)** — quality 0.86, P 1.00, R 0.75, timestamps near a mention 0.69 (n=16), $0.451, 39 s
  
Missed: How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand
  
Judge: The answer names three real catalog videos with specific, plausible timestamps and coherent descriptions of GRPO content, with no invented videos. Timestamps for the Daniel Han workshop are numerous and partially overlapping, which is slightly noisy but not clearly erroneous; all attributions appear consistent with the speakers' topics.
  
> Based on my searches, GRPO (Group Relative Policy Optimization) is discussed substantively in three videos in the library: - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han — [01:16:16], [01:16:03], [01:15:29–01:19:32], [01:24:14–01:27:32], [01:26:47–01:28:36], [02:12:22–02:20:39], [02:29:04–02:33:00] — Daniel Han gives an extended deep dive on GRPO: he explains the transition from RLHF→PPO→GRPO, showing how GRPO removes the value model and reward model in favor of a simple reward function and rollout/z-score-based advantage estimation, why it's called "group relative" (multiple questions/rollouts per group), and walks through a live Unsloth Col…

**VI agent (Gemini 3.8 Flash)** — quality 0.86, P 1.00, R 0.75, timestamps near a mention 1.00 (n=14), $0.143, 22 s
  
Missed: How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand
  
Judge: The answer names three catalog videos (Daniel Han's RL workshop, Jiantao Jiao's post-training lecture, and Yann Dubois's LLM agents overview), all real entries, each with specific timestamp ranges and no invented titles. Attributions are plausible for these RL-focused talks, though the specific timestamps and some described details (e.g., clipping mechanics at 1:02:43) cannot be verified here and the list may omit other mentions such as in fine-tuning talks.
  
> Here are the videos in the library that mention GRPO (Group Relative Policy Optimization): - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han - [00:23:39 - 00:23:55] : Daniel Han introduces GRPO as the algorithm behind DeepSeek-R1 that smartly eliminates the value model to save compute and parameters. - [01:16:17 - 01:18:25] : He details the mathematical differences between PPO and GRPO, explaining how GRPO computes relative advantage scores across sampled candidate outputs per prompt without a separate critic model. - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Post-Training Verifiable Agents by Jiantao Jiao - [00:03:39 - 00:03:45] : Jiantao…

**VI retrieval-only (Sonnet 5)** — quality 0.00, P 0.00, R 0.00, $0.030, 10 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Post-Training Verifiable Agents by Jiantao Jiao; [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois
  
Judge: The answer concludes that no library video mentions GRPO, explicitly reporting "Videos: 0" despite speculating that the Daniel Han RL workshop, the OpenAI RFT/DPO/SFT workshop, and Weizhu Chen's lecture are plausible candidates. No videos are actually cited and no timestamps are given, so it hallucinates nothing but provides no usable attribution.
  
> Based on the searches performed, I did not find clear evidence of GRPO (Group Relative Policy Optimization) being explicitly discussed or shown in any of the indexed videos. The search results returned only tangential or unrelated hits (e.g., generic UI text reading "Video," discussions of fine-tuning data points, human-in-the-loop learning, and RAG demos) with no direct mention of "GRPO" in transcripts, OCR, or descriptions. Given several plausible candidates in the library that discuss RL fine-tuning of LLMs (e.g., the OpenAI RFT/DPO/SFT workshop, the Daniel Han full workshop covering RL and kernels, and the Berkeley "Training Agentic Models" lecture by Weizhu Chen), it's likely GRPO is me…

**VI agent (Sonnet 5, 1.5k cap)** — quality 0.86, P 1.00, R 0.75, timestamps near a mention 0.83 (n=6), $0.427, 33 s
  
Missed: How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand
  
Judge: The answer names three real catalog videos with specific timestamp ranges and no invented entries, giving plausible speaker-specific detail for each. Attribution risk is mainly in the brief mentions (Jiao, Dubois) whose exact timestamps and claim specifics can't be verified here, and the Daniel Han range is very broad (over an hour) rather than pinpointed.
  
> Based on my search of the video library, GRPO (Group Relative Policy Optimization) is mentioned substantively in three videos: - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [01:22:56–02:29:33]: This talk gives the most extensive coverage of GRPO. Daniel Han explains the transition from RLHF→PPO→GRPO, showing how GRPO removes the value model and reward model, replacing them with a reward function and group-relative Z-score advantage estimates . He walks through why it's called "group relative" using a multi-question example and the full GRPO formula , discusses GRPO/RLVR replacing neural reward models with simple reward functions , and then d…

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

**VI agent (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.78 (n=9), $0.255, 33 s
  
Judge: The answer identifies all four expected videos with correct speaker attributions and specific timestamps, with no invented videos or untimestamped mentions. Content descriptions (ColPali/Qdrant late-interaction, multimodal chunking difficulty, separate retrieval evals, RAG eval setup) align with the key facts, though the Apoorva Joshi entry emphasizes chunking/retrieval mechanisms more than page-screenshot embedding.
  
> Based on my research, here are the moments across the video library where speakers discuss improving RAG—better/multimodal retrieval, or evaluating retrieval quality: - VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS [00:07:22] – Introduces "Multimodal RAG" architecture, explaining how documents are decomposed into images, text, and tables, embedded, and stored in a vector database for retrieval . [00:35:20] – Walks through loading the ColPali multimodal document retrieval model and setting up Qdrant with a multi-vector "MaxSim" comparator for late-interaction retrieval of page images . [00:47:47] – Demonstrates the "Retrieval (Query Time)"…

**VI agent (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.75 (n=12), $0.092, 21 s
  
Judge: All five named videos exist in the catalog, each with specific timestamp ranges, and the four key facts are covered — though the Apoorva Joshi entry emphasizes chunking and parent-document retrieval without explicitly mentioning embedding page screenshots with a multimodal embedding model. The extra Evals 101 (Doug Guthrie) citation is plausible and on-topic rather than an invented video, so no hallucinated sources appear.
  
> VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS [00:16:25] - [00:18:31] / [00:36:16] - [00:39:12]: The speaker describes how decomposing documents into raw text and separate images breaks context, advocating instead for multimodal vision-based retrieval using models like ColPali to embed and retrieve full document page images directly [[cite:01M29BFEMHGPPDX3PJRNTKG0Q1:1085-1111], [cite:01M29BFEMHGPPDX3PJRNTKG0Q1:2176-2242]]. Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB [00:19:35] - [00:22:00]: The speaker explains that chunking mixed-modality documents causes critical context loss across boundaries, and discusses impr…

**VI retrieval-only (Sonnet 5)** — quality 0.80, P 1.00, R 0.75, facts 0.75, timestamps near a mention 0.80 (n=10), $0.042, 19 s
  
Missed: Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB
  
Judge: The answer correctly identifies Suman Debnath's multimodal RAG talk, Taylor Jordan Smith's RAG eval discussion, and Sarah Sachs's separation of retrieval evaluation, all with specific timestamp ranges and no invented videos. It misses Apoorva Joshi's multimodal RAG segment entirely and adds an extra Yangqing Jia claim about multi-stage ranking that is outside the key facts; the Debnath description also omits the ColPali-style late-interaction detail.
  
> Based on my search of the library, here are the moments where speakers specifically discuss improving RAG — better/multimodal retrieval or evaluating retrieval quality: - VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS [00:00:00–00:01:55] and [00:07:22–00:10:13]: The speaker introduces the talk's focus on vision-based (multimodal) document retrieval, then explains the multimodal RAG architecture — decomposing documents into images, text, and tables, embedding them, storing in a vector database, and retrieving relevant chunks (which may be image, text, or table) to answer a query . - Strategies for LLM Evals (GuideLLM, lm-eval-harness, OpenA…

**VI agent (Sonnet 5, 1.5k cap)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.64 (n=11), $0.371, 37 s
  
Judge: The answer names exactly the four relevant catalog videos with specific timestamps and no invented entries, covering all four key facts (multimodal/ColPali retrieval, chunking mixed-modality docs, separate retrieval evaluation at Notion, RAG evals in the evals workshop). Attributions match the correct speakers, though some timestamps are dense and unverifiable and the Notion retrieval-freezing detail is an elaboration beyond the stated fact.

  
> Based on my research across the video library, here are the moments discussing improving RAG or evaluating retrieval quality: - VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS [00:07:22], [00:16:08], [00:36:32], [00:39:12], [00:50:08], [01:03:45]: The entire talk centers on building a "Multimodal RAG" pipeline — explaining how documents are decomposed into images, text, and tables, embedded and stored in a vector database (Qdrant), then retrieved via a ColPali multimodal document-retrieval model and "vision-based retrieval" (late-interaction/MaxSim scoring over page-image embeddings) . He also discusses how vector-database indexing (e.g., H…

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

**VI agent (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.67 (n=15), $0.268, 42 s
  
Judge: All five cited videos map to real catalog entries with specific timestamps, and each of the four key facts is conveyed, though the Daniel Han entry omits that GRPO also removes the value model and the Dubois entry omits LLM-as-judge rewards explicitly. The extra Ilan Bigio (RFT graders) attribution is plausible and on-topic rather than hallucinated, so the answer appears well-grounded overall.
  
> Based on my search across the library, here are the talks that substantively explain reward models or reward functions in RL post‑training: - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [00:21:04] Han introduces the RLHF diagram, explaining that a reward function/model converts an answer into a numeric score that the model is trained to maximize . [00:48:10] He explains how RLVR (GRPO) deletes the neural reward model entirely and replaces it with a simple ground-truth reward function (e.g., +1 correct/0 incorrect) . [01:13:37] He warns that maximizing reward directly can cause "reward hacking" and explains why PPO's KL-divergence term is add…

**VI agent (Gemini 3.8 Flash)** — quality 0.88, P 1.00, R 1.00, facts 0.75, timestamps near a mention 0.88 (n=8), $0.167, 41 s
  
Judge: All four named videos exist in the catalog and are correctly mapped with specific timestamp ranges, and the Jiao, Han, and Dubois characterizations match the key facts (though Han's GRPO point omits removal of the value model and Dubois's LLM-as-a-judge reward is attributed instead to Han). The Weizhu Chen entry drifts from the expected fact—it describes graders and reward hacking rather than the contrast between verifiable RL signals and the difficulty of training a reward model in RLHF.
  
> [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [00:48:08] - [00:49:25] Daniel Han explains that while traditional RLHF relies on a neural reward model to score outputs, reinforcement learning with verifiable rewards (RLVR) and GRPO replace the reward model entirely with an explicit reward function (such as ground-truth verification, regex formatting checks, code execution, or LLM-as-a-judge) that outputs a scalar value . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Post-Training Verifiable Agents by Jiantao Jiao [00:01:45] - [00:04:15] Jiantao Jiao explains how traditional RL post-training trains a neural reward model from human preference p…

**VI retrieval-only (Sonnet 5)** — quality 0.50, P 1.00, R 1.00, facts 0.00, timestamps near a mention 0.83 (n=6), $0.065, 18 s
  
Judge: All four named videos exist in the catalog and are the right ones, with specific timestamps, so there are no hallucinated sources. However, none of the key substantive points are conveyed: Jiao's preference vs. task-completion reward distinction is replaced by a generic 'three pillars' claim, Han's point that GRPO removes the value and reward models is missing, Dubois's contrast with preference/LLM-judge reward models is omitted, and Chen's RLHF reward-model-is-hard contrast is replaced by a format-reward/single-example anecdote.
  
> Based on the search results, here are the talks in the library that explain reward models or reward functions in RL post-training: - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [00:28:40] and [00:38:47] — The speaker explains that GRPo-style RL is reward maximization rather than next-token prediction, and states "the trick of RL is you just need a reward function… you don't need the chain of thought, just lots of questions" like math problems to compute correctness-based rewards . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Training Agentic Models by Weizhu Chen [00:12:26] and [00:13:24] — Discusses RLVR (RL with verified rewards) usin…

**VI agent (Sonnet 5, 1.5k cap)** — quality 0.88, P 1.00, R 1.00, facts 0.75, timestamps near a mention 0.80 (n=15), $0.417, 50 s
  
Judge: All four cited videos exist in the catalog and are plausibly relevant, with specific timestamps given for each; the Han, Dubois, and Jiao attributions match the expected substance including GRPO removing the value/reward model and the verifiable-vs-preference contrast. The Weizhu Chen entry instead describes reward hacking/grader cheating rather than the verifiable-signal vs hard-to-train-reward-model contrast, so that fact is not conveyed, and the answer is truncated mid-sentence.
  
> Based on the search results, several talks discuss reward models/functions in RL post-training: - [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han [00:21:04], [00:48:10], [00:56:33], [01:12:04–01:18:33], [01:19:46], [01:52:13] — Daniel Han gives the most extensive treatment: he first explains classic RLHF where an agent's outputs are fed into a trained reward model to compute a reward that updates the policy ; he then shows how "reward functions" are hand-designed (e.g. +1 for a correct answer, −10 for a bad one) ; he explains that RLVR (RL with Verifiable Rewards) deletes the learned neural reward model entirely and replaces it with a simple gro…

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

**VI agent (Sonnet 5)** — quality 0.55, P 1.00, R 0.75, facts 0.25, timestamps near a mention 1.00 (n=6), $0.229, 26 s
  
Missed: How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand
  
Judge: All four cited videos are real catalog entries with specific timestamps, and no videos were invented; the Yangqing Jia sparsity/efficiency point is accurately captured. However, the answer misses Ishan Anand's MoE discussion entirely, omits the Cerebras talk's core framing that agents replace experts in the mixture-of-agents pattern, and reframes Yann Dubois's point as a general sparsity explanation rather than naming MoE transformers as the dominant open-source architecture, while adding an extra (plausible but unverified) Daniel Han quantization mention.
  
> Based on my research, here's where Mixture of Experts (MoE) is explained across the library: - From Mixture of Experts to Mixture of Agents with Super Fast Inference — Daniel Kim & Daria Soboleva [00:07:17]–[00:09:52]: This is the most detailed treatment. Daniel Kim explains that the feed-forward layer in a transformer is a bottleneck because one network has to "disentangle" all information (different languages, domains, tasks); MoE replaces the single feed-forward network with multiple specialized "expert" networks (e.g., one for math, one for biology) plus a new "router" network that decides which expert handles each token, letting you scale parameters/model capacity without increasing inf…

**VI agent (Gemini 3.8 Flash)** — quality 0.62, P 1.00, R 1.00, facts 0.25, timestamps near a mention 1.00 (n=8), $0.118, 19 s
  
Judge: The answer identifies exactly the four correct library videos with specific timestamp ranges and no invented sources, and the Yangqing Jia description matches the expected sparsely-activated-efficiency framing. However, it misses the distinctive framing in three cases — it omits the Cerebras mixture-of-agents analogy (agents replacing experts), Ishan Anand's 'biggest architectural change since GPT-2' audience-question context, and Yann Dubois's point that transformers with MoE are the dominant open-source architecture — instead giving generic mechanism descriptions.
  
> Mixture of Experts (MoE) is explained in the following videos across the library: From Mixture of Experts to Mixture of Agents with Super Fast Inference - Daniel Kim & Daria Soboleva [00:07:17 - 00:10:18]: Daria Soboleva explains that Mixture of Experts addresses the bottleneck of monolithic feed-forward networks by replacing them with multiple specialized feed-forward networks ("experts") and a router network that selects which expert to activate per token, allowing the model to increase parameter capacity and quality without increasing inference time compute [[cite:01M29G8NETDNGGY5QT9XHCJRXN:437-550], [cite:01M29G8NETDNGGY5QT9XHCJRXN:595-618]]. How LLMs work for Web Devs: GPT in 600 lines …

**VI retrieval-only (Sonnet 5)** — quality 0.33, P 1.00, R 0.50, facts 0.00, timestamps near a mention 1.00 (n=2), $0.028, 10 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia
  
Judge: The answer identifies two correct library videos with specific timestamps, but it does not convey the key substance for either: it omits the mixture-of-agents-imitates-MoE framing for Cerebras and the 'biggest architectural change since GPT-2' characterization for Ishan Anand. It also entirely misses the Yann Dubois and Yangqing Jia mentions, and adds an unsupported claim about OpenAI/Anthropic being the industry standard.
  
> - From Mixture of Experts to Mixture of Agents with Super Fast Inference - Daniel Kim & Daria Soboleva — [00:08:39]: The speaker explains that MoE architecture replaces a single feed-forward network with multiple specialized "expert" networks plus a router, letting models scale in parameter count without a proportional increase in inference compute, and notes it's now the industry standard used by OpenAI, Anthropic, and others . - How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand — [01:39:20]: During Q&A, the presenter describes MoE models as a way to add more parameters "without a proportional increase in compute," while also noting the training challenges this archit…

**VI agent (Sonnet 5, 1.5k cap)** — quality 0.62, P 1.00, R 1.00, facts 0.25, timestamps near a mention 1.00 (n=10), $0.319, 39 s
  
Judge: All five cited videos are real catalog entries with specific timestamp ranges, and the attributions (Cerebras MoE explanation, Dubois sparsity framing, Anand Q&A, Jia sparse-activation history, Han quantization aside) appear plausible and correctly assigned. However, it misses several key substantive points: it never notes that the Cerebras talk uses MoE as the template its 'mixture of agents' imitates, that Anand calls MoE one of the biggest architectural changes since GPT-2, or that Dubois names transformers-with-MoE the dominant open-source architecture.
  
> Based on my search, here is where Mixture of Experts (MoE) is explained across the library: - From Mixture of Experts to Mixture of Agents with Super Fast Inference — Daniel Kim & Daria Soboleva [00:07:17]–[00:10:18]: This is the most detailed explanation in the library. The speaker (a Cerebras MoE researcher) describes a standard transformer's feed-forward layer as a bottleneck that must "disentangle" all information for every token; MoE fixes this by replacing one monolithic feed-forward network with multiple specialized "expert" feed-forward networks (e.g., one for math, one for biology) plus a router network that picks which expert(s) handle each token, letting parameter count/capacity g…

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

**VI agent (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=8), $0.105, 21 s
  
Judge: All four named videos exist in the catalog with precise timestamp ranges, and the three key facts (Dubois, Stone, Vinyals) are accurately covered with correct mechanisms and conclusions. The extra Yangqing Jia attribution is an additional claim not in the ground-truth fact list and may be a false positive, though it is at least mapped to a real catalog video.
  
> Based on the search across the library, four talks explicitly invoke Rich Sutton's "bitter lesson": - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois [00:48:29–00:49:16]: Dubois cites Sutton's blog post directly, summarizing that "the only thing that matters in the long run is leveraging compute" since more compute empirically yields better performance, tying it to Moore's Law and the Chinchilla scaling discussion . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia [00:33:38–00:34:07]: Jia uses Sutton's 2019 blog post to justify why AI infrastructure ("the third pillar") is essential, paraphrasing the lesson as "t…

**VI agent (Gemini 3.8 Flash)** — quality 0.73, P 1.00, R 0.67, facts 0.67, timestamps near a mention 1.00 (n=3), $0.125, 29 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals
  
Judge: The answer correctly identifies Peter Stone (TAMER+RL) and Yann Dubois (general methods that scale with compute win) with plausible timestamps, but omits Oriol Vinyals' StarCraft imitation-learning bootstrapping discussion. It adds a third claim about Yangqing Jia quoting the bitter lesson for compute infrastructure, which is not in the ground-truth fact list and may be a misattribution/extra claim; no invented videos were named and all cited videos have timestamps.
  
> The following speakers refer directly to Rich Sutton's "bitter lesson": Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Autonomous Agents by Peter Stone [00:21:42] Peter Stone references Rich Sutton's "bitter lesson" to note that injecting human input can degrade ultimate performance compared to learning from scratch with reinforcement learning, before presenting work on combining human feedback with autonomous RL (TAMER+RL) . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia [00:33:38] Yangqing Jia displays and quotes Richard Sutton's 2019 blog post "The Bitter Lesson" to justify the necessity of AI computation infrastructure as a foundational IT pi…

**VI retrieval-only (Sonnet 5)** — quality 0.73, P 1.00, R 0.67, facts 0.67, timestamps near a mention 1.00 (n=4), $0.029, 11 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone
  
Judge: The answer correctly identifies Dubois and Vinyals with specific timestamp ranges and accurate characterizations, but omits Peter Stone's use of the bitter lesson as a warning about human input lowering performance and his TAMER+RL combination argument. No invented videos or misattributions are present; the coverage is simply incomplete.
  
> - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois — [00:48:33]–[00:50:31]: Dubois explicitly names Sutton, puts up a slide titled "Bitter lesson," and argues that leveraging ever-growing compute matters more in the long run than hand-engineering methods, recommending the audience read the original blog post . - Agentic AI MOOC UC Berkeley CS294-196 F25 Multi-Agent Systems in Era of LLMs by Oriol Vinyals — [00:20:07]–[00:20:32]: While discussing the exploration problem in StarCraft II RL (where a random policy is useless), a "Bitter Lesson" slide appears on screen and Vinyals invokes the lesson to frame why pure RL alone struggles and imitation learning from…

**VI agent (Sonnet 5, 1.5k cap)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=8), $0.107, 22 s
  
Judge: All four named videos exist in the catalog and each comes with specific timestamp ranges, and the three key facts (Dubois, Stone, Vinyals) are conveyed accurately with correct attributions. The only extra is the Yangqing Jia mention, which is plausible but beyond the ground-truth fact list; no videos are invented or left without timestamps.
  
> Here are the videos in the library that explicitly reference Rich Sutton's "bitter lesson": - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois — [00:48:29]–[00:49:57]: Yann Dubois cites Sutton's 2019 blog post to argue that "the only thing that matters in the long run is leveraging compute," concluding that methods should be designed to scale with compute rather than hand-optimized at current compute levels . - Agentic AI MOOC UC Berkeley CS294-196 F25 Multi-Agent Systems in Era of LLMs by Oriol Vinyals — [00:20:33]–[00:21:19]: Oriol Vinyals frames the "bitter lesson discussion" around whether to bootstrap StarCraft agents with imitation learning versus rely…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=4), $0.040, 17 s
  
Judge: All four cited videos exist in the catalog, each with a specific timestamp, and the three ground-truth attributions (Dubois, Stone, Vinyals) are correctly matched with accurate mechanisms, though Vinyals' StarCraft context is left implicit. The extra Yangqing Jia claim is an unverified addition beyond the key facts, and the parenthetical figures after two titles appear to be video durations rather than cited timestamps.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia (01:19:31) [00:33:41] Yangqing Jia cites Rich Sutton's 2019 essay to justify the massive demand for AI computation and infrastructure, summarizing the core takeaway that the most effective method in 70 years of AI research is deploying general methods powered by large amounts of numerical computation on large datasets. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Autonomous Agents by Peter Stone (01:01:41) [00:21:42] When discussing the TAMER system, Peter Stone notes that while learning from human feedback is faster, autonomous learning from scratch reaches higher performance, citing Rich Sutton's…

## corpus-17: Which talks discuss vector databases or vector stores?

*Kind: mention.* 

Expected: Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB; How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust); Building Code First AI Agents with Azure AI Agent Service — Cedric Vidal, Microsoft; VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS

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

**VI retrieval-only (Sonnet 5)** — quality 0.40, P 1.00, R 0.25, timestamps near a mention 1.00 (n=2), $0.056, 15 s
  
Missed: Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB; How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust); Building Code First AI Agents with Azure AI Agent Service — Cedric Vidal, Microsoft
  
Judge: The answer identifies only one relevant video (the VoiceVision RAG talk) with a specific, plausible timestamp range and no invented videos. It explicitly dismisses other candidates including the MongoDB multimodal agents talk, which likely does cover vector stores, so the response is under-inclusive though not misattributed.
  
> Based on the search results, only one video in the library has clear, direct evidence of discussing vector databases/vector stores: - VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS [00:33:04–00:35:20] — The speaker explains late-interaction embeddings for multi-vector RAG, noting that token-level embeddings are "already stored... in your vector database," and then introduces a live demo using the Qdrant vector database, remarking that "this functionality is not supported in all the vector databases... we are going to use one of the vector database called quadrant [Qdrant]." The other results returned by search (ComfyUI workshop, ElevenLabs…

**VI agent (Sonnet 5, 1.5k cap)** — quality 0.75, P 0.75, R 0.75, timestamps near a mention 0.62 (n=8), $0.175, 25 s
  
Wrong: Strategies for LLM Evals (GuideLLM, lm-eval-harness, OpenAI Evals Workshop) — Taylor Jordan Smith
  
Missed: How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust)
  
Judge: The answer names four real catalog videos, each with specific timestamps or ranges, and no invented titles appear. Attributions are plausible and topic-appropriate (Qdrant/ColPali, MongoDB vector search, RAG eval slide, Azure vector_store), though the exact timestamp precision cannot be verified here.
  
> Based on my search across the library, here are the talks that discuss vector databases/vector stores: - VoiceVision RAG - Integrating Visual Document Intelligence with Voice Response — Suman Debnath, AWS [00:39:12–01:05:39] The speaker builds a full multimodal RAG pipeline live, setting up a Qdrant vector database, storing ColPali embeddings, and querying it, including a detailed explainer on how vector databases use indexing techniques (e.g., HNSW) to search millions of vectors efficiently . - Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB [00:16:43–00:34:13] The presenter describes storing multimodal embeddings in MongoDB used as a vector database and performing vecto…

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

**VI agent (Sonnet 5)** — quality 0.73, P 1.00, R 0.67, facts 0.67, timestamps near a mention 0.75 (n=8), $0.264, 36 s
  
Missed: [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments
  
Judge: All three named videos exist in the catalog and are given specific timestamps; the Dawn Song and Clay Bavor entries capture the expected guardrail substance (input/output guardrails, constitutional classifiers, deterministic plus AI checks), though information-flow control/taint tracking and human-in-the-loop are not explicitly mentioned. The answer misses the Vibe Coding at Scale workshop's custom-instructions-as-guardrails point and instead substitutes the ElevenLabs conversational agents talk, whose moderation/watermarking content is plausible but outside the expected set.
  
> Based on my search across the library, here are the talks that substantively discuss guardrails for AI agents: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [01:24:07], [01:27:04], [01:39:14]. Dawn Song details a layered "defense-in-depth" architecture: input guardrails that block/filter/sanitize untrusted inputs (citing Anthropic's Constitutional Classifiers, and her own DataSentinel and PromptArmor systems for detecting/removing prompt-injection attacks), output guardrails that validate tool calls and generated content against security policies (e.g., the Progent privilege-control system), and access-control/identity-management guardrails (ma…

**VI agent (Gemini 3.8 Flash)** — quality 0.73, P 1.00, R 0.67, facts 0.67, timestamps near a mention 1.00 (n=2), $0.124, 21 s
  
Missed: [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments
  
Judge: The answer correctly identifies Dawn Song's and Clay Bavor's talks with plausible timestamps and captures the input/output guardrail and deterministic-plus-AI-check substance, though it omits Song's information-flow control, taint tracking, monitoring, and human-in-the-loop items. It misses the Vibe Coding at Scale workshop's framing of custom instructions and tools as enterprise guardrails; no videos were invented.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song [01:24:22] Dawn Song details runtime protections for AI agents, describing input guardrails that sanitize, filter, and block malicious prompts (such as prompt-injection detection models and constitutional classifiers) and output guardrails that inspect generated text and validate tool calls against programmable privilege and security policies . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Practical Lessons from Deploying...by Clay Bavor [00:44:06] Clay Bavor presents a layered guardrail framework designed to thwart prompt injection and abuse, combining deterministic input validation and secondary LLM…

**VI retrieval-only (Sonnet 5)** — quality 0.42, P 1.00, R 0.33, facts 0.33, timestamps near a mention 0.50 (n=2), $0.055, 13 s
  
Missed: [Full Workshop] Vibe Coding at Scale: Customizing AI Assistants for Enterprise Environments; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor
  
Judge: The answer correctly identifies Dawn Song's talk with a precise timestamp range and accurately describes the runtime guardrail categories and constitutional classifiers example, though it omits human-in-the-loop. It misses the Clay Bavor and Vibe Coding at Scale talks entirely, wrongly concluding only one video is relevant; no videos were invented.
  
> Based on the search results, only one talk in the library clearly and explicitly covers "guardrails" as a named AI-agent safety mechanism: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [01:24:04–01:30:24] Dawn Song presents runtime protection mechanisms for agentic AI security, explicitly naming "input guardrails, output guardrails, information flow control and taint tracking, monitoring" as the categories of runtime security checks performed during agent execution, and gives examples such as Anthropic's Constitutional Classifiers, DataSentinel, and PromptArmor for detecting and sanitizing prompt-injection attacks . None of the other videos ret…

**VI agent (Sonnet 5, 1.5k cap)** — quality 0.93, P 0.75, R 1.00, facts 1.00, timestamps near a mention 0.67 (n=12), $0.325, 41 s
  
Wrong: Building Code First AI Agents with Azure AI Agent Service — Cedric Vidal, Microsoft
  
Judge: All four named videos map to real catalog entries with specific timestamp ranges, and the three key facts (Dawn Song's runtime protection taxonomy with constitutional classifiers, Bavor's deterministic-plus-AI guardrail layering, and the Vibe Coding workshop's custom instructions/tools framing) are conveyed. Minor gaps: the Dawn Song summary omits information-flow control/taint tracking and monitoring while adding sandboxing, and the extra Azure Agent Service sandbox claim is tangential and unverified, though plausible.
  
> Based on my search of the library, here are the talks that explicitly discuss guardrails for AI agents: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [01:24:04–01:27:04], [01:27:04–01:29:02], [01:39:01–01:41:33], [01:33:01–01:34:23]. This is the most comprehensive treatment: Dawn Song lays out a full taxonomy of runtime protections including input guardrails (block/filter/sanitize malicious inputs, e.g. Constitutional Classifiers, DataSentinel, PromptArmor for prompt-injection detection) , output guardrails (validating/filtering tool calls and generated content against safety policies, illustrated with the "Progent" domain-specific policy-enfor…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.40 (n=10), $1.079, 66 s
  
Judge: All five named videos map to real catalog entries with specific timestamps, and the three ground-truth talks (Dawn Song, Clay Bavor, Vibe Coding at Scale) are correctly identified with their guardrail mechanisms, though the Dawn Song entry omits information-flow control/taint tracking, monitoring, and human-in-the-loop. Two extra videos (ElevenLabs conversational agents and Daily realtime voice) are added with plausible but unverified guardrail details, and some cited timestamps appear to be video durations rather than moments of evidence.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song [01:24:31] – [01:29:20] Dawn Song details runtime defenses consisting of input guardrails (which block, filter, sanitize, or normalize untrusted inputs to prevent prompt injection, such as constitutional classifiers, DataSentinel, and PromptArmor) and output guardrails (which validate and filter generated content and tool calls to enforce security policies and dynamic least-privilege control, exemplified by Progent). Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Practical Lessons from Deploying...by Clay Bavor [00:40:26] Clay Bavor explains using "supervisor" micro-agents sitting around the core agent…

## corpus-19: Which talks discuss scaling laws?

*Kind: topic.* 

Expected: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Predictable Noise in LLM Benchmarks by Sida Wang; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois

Key facts:

- Yann Dubois explains scaling laws as tuning at small scale and extrapolating to a large final run, and mentions the Chinchilla paper.
- Sida Wang uses scaling laws (consistent gains when model size doubles) as a source of signal for judging benchmark noise.

**VI agent (Sonnet 5)** — quality 0.75, P 0.33, R 1.00, facts 1.00, timestamps near a mention 0.75 (n=4), $0.177, 31 s
  
Wrong: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Multi-Agent AI by Noam Brown; From Mixture of Experts to Mixture of Agents with Super Fast Inference - Daniel Kim & Daria Soboleva; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen
  
Judge: All six named videos map to real catalog entries with specific timestamp ranges, and both key facts (Dubois on extrapolation/Chinchilla and Sida Wang on scaling laws as benchmark signal) are covered accurately. The additional talks (Daniel Han, Kim/Soboleva, Weizhu Chen, Noam Brown) are plausible but more loosely related test-time/training scaling content, which risks over-inclusion though no videos appear invented or misattributed.
  
> Based on the search results, here are the talks that explicitly discuss scaling laws: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois — [00:41:55]–[00:53:28]: The lecturer gives a dedicated segment on scaling laws, explaining that pretraining test loss decreases predictably with more compute (model size × data), allows extrapolating performance from small-scale experiments, and introduces the Chinchilla paper on optimal compute allocation between model size and data, noting it ignores inference costs . He also ties this to Sutton's "Bitter Lesson" that scale and compute matter more than manual optimization , and later discusses test-time compute scaling f…

**VI agent (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.75 (n=4), $0.136, 28 s
  
Judge: Both named videos exist in the catalog and are correctly attributed with specific timestamp ranges, and the Chinchilla reference plus the benchmark-noise signal point are both captured. Fact 1's framing as "tune at small scale and extrapolate to a large final run" is only loosely conveyed via the compute-allocation/architecture-comparison description, but the substance of scaling-law extrapolation is present and no videos are invented.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois — [00:41:54] - [00:48:26]: Yann Dubois examines empirical scaling laws showing how log test loss decreases predictably with log compute, demonstrates how scaling curves evaluate architectural trade-offs between Transformers and LSTMs, and reviews the Chinchilla scaling law for optimal compute allocation between model parameters and training tokens . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Predictable Noise in LLM Benchmarks by Sida Wang — [00:42:27] - [00:42:47]: Sida Wang explains that scaling laws provide consistent performance improvements when doubling model size across model families, which benchm…

**VI retrieval-only (Sonnet 5)** — quality 0.58, P 1.00, R 0.50, facts 0.50, timestamps near a mention 1.00 (n=3), $0.053, 12 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Predictable Noise in LLM Benchmarks by Sida Wang
  
Judge: The answer correctly identifies and timestamps the Yann Dubois lecture, capturing the small-scale-to-large-scale extrapolation idea and the Chinchilla reference. However, it misses Sida Wang's benchmark-noise talk and wrongly asserts no other video discusses scaling laws.
  
> Based on the search results, scaling laws are explicitly discussed in one video in this library: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois — [00:41:55] and [00:44:31]–[00:47:00]: The lecturer explains how scaling laws changed the ML training pipeline, showing that pretraining test loss predictably decreases with more compute (enabling small-scale experiments to predict large-scale performance), and references the "very famous paper about scaling laws," including a discussion of transformer vs. LSTM scaling curves and the Chinchilla paper on optimal compute allocation . No other videos in the searched set contained direct evidence of a "scaling laws"…

**VI agent (Sonnet 5, 1.5k cap)** — quality 0.72, P 0.29, R 1.00, facts 1.00, timestamps near a mention 0.83 (n=6), $0.096, 21 s
  
Wrong: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Multi-Agent AI by Noam Brown; From Mixture of Experts to Mixture of Agents with Super Fast Inference - Daniel Kim & Daria Soboleva; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia
  
Judge: Both key facts are covered accurately and attributed to the correct speakers (Yann Dubois with Chinchilla, Sida Wang with doubling-model-size signal), with specific timestamp ranges throughout and no invented videos. The answer adds several extra talks about test-time/inference scaling that may be broader than the intended scope, and the final entry is truncated with a stray citation artifact, but all named videos map to real catalog entries with plausible timestamps.
  
> Based on searches across the library, here are the talks that explicitly discuss scaling laws (compute/data/model-size scaling relationships), including both classic pretraining scaling laws and test-time/inference-time scaling: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 LLM Agents Overview by Yann Dubois [00:41:45]–[00:48:41], [00:56:56]–[00:57:51] Explains how scaling laws let researchers extrapolate performance from small-scale runs to large ones, cites the Chinchilla scaling law for optimal compute allocation between model size and data, and discusses the "Bitter Lesson" that scale + Moore's Law drive improvement . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Predictable Nois…

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

**VI agent (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.60 (n=10), $0.200, 24 s
  
Judge: Both claimed videos exist in the catalog and are correctly mapped, with specific timestamp ranges given for each, and both key facts are conveyed (Aman Khan's Cruise background applied to evals, Peter Stone's sense-decide-act autonomous car plus DARPA Urban Challenge). The Waymo reference is attributed to Stone rather than Khan's audience poll, a minor misattribution, and the GT Sophy details are plausible but unverified elaboration.
  
> Based on searches across the full library, self-driving cars / autonomous vehicles come up mainly in two talks: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Autonomous Agents by Peter Stone - [00:10:48–00:13:23] Peter Stone frames "an autonomous car" as a prototypical agent that must sense, decide, and act, recalling his own DARPA Urban Challenge car and noting that companies like Waymo and Zoox are investing heavily in making fully autonomous roads a reality, including speculation about whether traffic lights would still be needed once all cars are autonomous . - [00:13:23–00:15:57] He describes Sony AI's Gran Turismo Sophy project, showing a split-screen of a human driving in the raci…

**VI agent (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=7), $0.134, 18 s
  
Judge: The answer correctly identifies both key videos (Aman Khan on Cruise evals and Peter Stone on autonomous cars/DARPA Urban Challenge) with specific timestamps, though it omits the Waymo audience poll detail. It adds two extra attributions (Ishan Anand's Waymo tokenization and Dawn Song's collision-avoidance example) that are plausible but unverified and may be fabricated; no invented videos are named and all claims carry timestamps.
  
> Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize - [00:01:25], [00:06:42], and [01:06:37] - Aman Khan discusses his background building evaluation systems for self-driving cars at Cruise and draws parallels between curating evaluation datasets for edge cases in autonomous driving (such as unprotected left turns and pedestrians) and evaluating generative AI agents [[cite:01M29ABBRDKG9AF3DVQRQBPSN2:85-115], [cite:01M29ABBRDKG9AF3DVQRQBPSN2:442-452], [cite:01M29ABBRDKG9AF3DVQRQBPSN2:3997-4045]]. Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Autonomous Agents by Peter Stone - [00:11:43] and [00:22:25] - Peter Stone highlights autonomous cars as fundamental examples of…

**VI retrieval-only (Sonnet 5)** — quality 0.75, P 1.00, R 1.00, facts 0.50, timestamps near a mention 0.50 (n=4), $0.060, 16 s
  
Judge: The answer correctly identifies both relevant videos with plausible timestamps and fully captures Aman Khan's Cruise/self-driving evaluation background, though it omits the Waymo poll. For Peter Stone it substitutes a GT Sophy racing-agent description instead of the sense-decide-act autonomous car framing and DARPA Urban Challenge car, so that fact is not conveyed and the attributed content may be misplaced.
  
> Based on the searches, self‑driving/autonomous‑vehicle references appear in two talks: - Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize — [00:01:08] and [01:06:11]: Aman Khan describes starting his career "working on self-driving cars at Cruise," where he became a PM for self-driving evaluation systems, and later uses a self-driving-car analogy (handling left turns, pedestrians on sidewalks) to illustrate how to iteratively build AI evaluation datasets . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Autonomous Agents by Peter Stone — [00:45:23]–[00:48:06]: Peter Stone discusses Sony's "GT Sophy," an autonomous-driving AI agent that races against top human Gran…

**VI agent (Sonnet 5, 1.5k cap)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.82 (n=11), $0.141, 21 s
  
Judge: The answer names only two real catalog videos, correctly mapped, with specific timestamps for each claim and no invented entries. Both key facts are covered (Aman Khan's Cruise background and eval lessons, Peter Stone's sense-decide-act agent framing plus his DARPA Urban Challenge car), though the Waymo mention is attributed to Stone rather than Khan's audience poll and the Gran Turismo Sophy segment timestamps are unverified.
  
> Based on the search results, I found two videos with substantial, direct discussion of self-driving cars/autonomous vehicles. - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Autonomous Agents by Peter Stone [00:11:51], [00:12:52], [00:12:54] — Peter Stone describes being inspired by autonomous driving, noting "an autonomous car is an agent, it has to sense, decide, and act," recalling his own DARPA Urban Challenge car and noting companies like Waymo and Zoox investing heavily in making full autonomy a reality . [00:12:52]-[00:13:23] — He discusses research on what intersections might look like once all cars are autonomous, versus mixed human/autonomous traffic . [00:52:10]-[01:00:24] — He …

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

**VI agent (Sonnet 5)** — quality 0.82, P 1.00, R 1.00, facts 0.75, timestamps near a mention 0.86 (n=14), $0.214, 39 s
  
Judge: Both named videos map correctly to catalog entries (Noam Brown and Oriol Vinyals lectures) with specific, plausible timestamp ranges and no invented sources. It misses Brown's point that self-play has not yet shown up in LLMs and reframes his 'overfit intuitions' argument as a formal guarantee, and it renders Vinyals' oscillation/rock-paper-scissors dynamic as a one-strategy collapse, but the league/exploiter and human-evaluation points are attributed accurately.
  
> Based on the two Berkeley CS294-196 lectures that explicitly discuss self-play, here is the comparison: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown — [00:09:38–00:11:03] Brown explains the theoretical power of self-play: in any two-player zero-sum game, a "sound" self-play algorithm provably converges to a minimax equilibrium without any human data, which underlies victories in Go, chess, poker, and RTS games . - [00:30:46–00:32:51] He then shows where it breaks down even in a simple two-player general-sum game (the Ultimatum Game): pure self-play converges to a rational-but-inhuman equilibrium (offering the smallest possible amount), which fails to model r…

**VI agent (Gemini 3.8 Flash)** — quality 0.47, P 1.00, R 1.00, facts 0.25, timestamps near a mention 0.86 (n=7), $0.076, 20 s
  
Judge: Both cited videos are real catalog entries correctly attributed to Noam Brown and Oriol Vinyals, each with specific timestamps, and no videos were invented. However, coverage is partial: it omits that self-play has not yet worked for LLMs, omits AlphaStar's league/exploiter-agent remedy and the rock-paper-scissors framing of oscillation, and omits evaluation against human players, while adding details (adversarial exploits, Void Rays, delusion) whose accuracy cannot be verified from the listed facts.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown [00:10:05], [00:11:37], [00:31:16], [00:38:19] Noam Brown highlights that sound self-play provably converges to a minimax equilibrium without human data in two-player zero-sum games (driving milestones in Go, chess, and poker), but explains that it breaks down due to function approximation leaving neural networks vulnerable to adversarial exploits, and fundamentally fails in non-zero-sum or multi-player settings (like the Ultimatum Game or Diplomacy) where agents converge to idiosyncratic equilibria that perform terribly against humans or differently trained agents [[cite:01M296FBHQD7SWTC6TZ59NSMQ9:605-673], [cite:0…

**VI retrieval-only (Sonnet 5)** — quality 0.65, P 1.00, R 1.00, facts 0.50, timestamps near a mention 1.00 (n=4), $0.053, 13 s
  
Judge: Both named videos map correctly to catalog entries with timestamp ranges, and the Vinyals rock-paper-scissors/population-training point is accurately attributed, though it omits the league/exploiter framing's human-evaluation aspect. The Brown segment covers self-play's game breakthroughs and absence in LLMs but misses the two-player zero-sum perfect-information overfitting argument, and the cited 0:00–2:21 opening window is a plausible but generic attribution.
  
> - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown — [00:00:00–00:02:21] Brown opens by framing self-play's role in past AI breakthroughs (citing the AlphaGo training pipeline as an analogy) and sets up why recursive self-improvement via self-play hasn't yet worked for LLMs the way it did for games like Go . - Agentic AI MOOC UC Berkeley CS294-196 F25 Multi-Agent Systems in Era of LLMs by Oriol Vinyals — [00:39:57–00:42:11] Vinyals explains where self-play breaks down, using a rock-paper-scissors example to show how plain self-play can fail to converge to a robust policy, contrasting it with "original co-play" and population-based training against weakest/stronges…

**VI agent (Sonnet 5, 1.5k cap)** — quality 0.82, P 1.00, R 1.00, facts 0.75, timestamps near a mention 0.94 (n=16), $0.122, 32 s
  
Judge: The answer cites only the two correct catalog videos (Brown's Multi-Agent AI and Vinyals' Multi-Agent Systems talks) with specific timestamp ranges and no invented sources, correctly capturing the two-player zero-sum limitation, the naive self-play collapse/exploiter-league fix, and the human-evaluation point. It omits Brown's claim that self-play has not yet materialized in LLMs, and some details (Dota/StarCraft attribution, the specific rock-paper-scissors framing) are loosely reconstructed, so timestamp precision for those segments is uncertain.
  
> Self-Play: Noam Brown vs. Oriol Vinyals Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown - [00:10:33]–[00:11:37] Brown explains the theoretical promise: in any balanced two-player zero-sum game, self-play needs no human data and, given enough memory/compute, provably converges to a minimax equilibrium that "guarantees you will not lose in expectation to any human or any other player" — this underlies victories in Go, chess, poker, and real-time strategy games . - [00:11:37]–[00:12:00] He immediately caveats that convergence is only theoretical; in practice neural nets trained by self-play don't fully reach equilibrium and remain "vulnerable to adversarial attacks"…

**Gemini agentic video (3×10 + merge)** — quality 0.65, P 1.00, R 1.00, facts 0.50, timestamps near a mention 0.83 (n=6), $0.099, 32 s
  
Judge: Both named videos are real catalog entries correctly matched to Noam Brown and Oriol Vinyals, each with several concrete timestamps and no invented sources. It captures the zero-sum-vs-non-zero-sum breakdown and the rock-paper-scissors/league-with-exploiters points, but omits that self-play has not yet worked for LLMs and does not state that AlphaStar was evaluated by playing humans rather than only self-play Elo; some specifics (ultimatum game, Void Rays, minimax convergence) are plausible but unverified elaborations.
  
> Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown [00:09:47] / [00:26:11] / [00:37:30]: Noam Brown explains that self-play achieved superhuman milestones in two-player zero-sum games (like Go, chess, and poker) by provably converging to a minimax equilibrium without needing human data, but it breaks down in non-zero-sum and cooperative environments (such as Diplomacy and the ultimatum game) where pure self-play finds arbitrary equilibria that fail to coordinate with humans, making self-play without human data a dead end for human cooperation. Agentic AI MOOC UC Berkeley CS294-196 F25 Multi-Agent Systems in Era of LLMs by Oriol Vinyals [00:09:31] / [00:40:00] - [00:…

## corpus-22: How many talks in the library come from the UC Berkeley Agentic AI MOOC (CS294-196), and who are the speakers?

*Kind: synthesis.* A metadata question: both systems see the video titles (VideoIndex in the agent's system prompt, Gemini in the catalog attached to the request).

Expected: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Post-Training Verifiable Agents by Jiantao Jiao; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Agentic AI Safety & Security by Dawn Song; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Predictable Noise in LLM Benchmarks by Sida Wang; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Multi-Agent AI by Noam Brown; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone; Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Evolution of System Designs by Yangqing Jia; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | AI Agents to Automate Science by James Zou

Key facts:

- There are eleven MOOC lectures in the library.
- The speakers are Dawn Song, Noam Brown, Yangqing Jia, Jiantao Jiao, Clay Bavor, Sida Wang, Peter Stone, Weizhu Chen, Yann Dubois, Oriol Vinyals and James Zou.

**VI agent (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, $0.139, 42 s
  
Judge: The answer correctly identifies all eleven MOOC lectures with the right speakers and provides a timestamped opening quote for each, with no invented videos or misattributions. Timestamps are all near 0:00 (essentially just opening lines), which is appropriate for this identification-style question.
  
> Based on the video titles and opening transcripts, the following 11 videos in the library are from the UC Berkeley Agentic AI MOOC (CS294-196): - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [00:00:00] Dawn Song opens: "So today, we will talk about towards building safe and secure agentic AI." - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown — [00:00:01] Noam Brown introduces himself: "I'm Noam Brown... I'll be talking about multi-agent AI." - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia — [00:00:02] The speaker recounts his Berkeley PhD and career at Google/Caffe/Tenso…

**VI agent (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, facts 1.00, $0.052, 16 s
  
Judge: The answer correctly counts 11 MOOC lectures and names all eleven speakers, mapping each to a real catalog video with specific opening timestamps. No invented videos or misattributions are present, and every named talk carries a timestamp range.
  
> There are 11 talks in the library from the UC Berkeley Agentic AI MOOC (CS294-196): Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song [00:00:00 - 00:00:10]: Speaker Dawn Song introduces the session discussing safe and secure agentic AI . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown [00:00:01 - 00:00:13]: Speaker Noam Brown introduces himself and announces he will be speaking on multi-agent AI . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia [00:00:02 - 00:00:15]: Speaker Yangqing Jia reflects on completing his PhD at UC Berkeley between 2009 and 2013 and introduces his talk on…

**VI retrieval-only (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, $0.062, 29 s
  
Judge: The answer correctly identifies all eleven MOOC lectures and names every speaker, with no invented videos or misattributed content. Timestamps are mostly placeholder 00:00:00 title references rather than real in-video evidence (which the answer transparently acknowledges), while the few substantive timestamps given appear plausibly tied to their videos.
  
> Based on the video titles in the index (which explicitly name the course and lecturer) and the evidence retrieved, the following 11 videos in the library are lectures from the UC Berkeley Agentic AI MOOC (CS294-196, Fall 2025): - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [01:48:17] The lecturer closes the session by thanking attendees for joining the "Agentic AI MOOC" and inviting them to the AgentBeats competition and Agentic AI Summit . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown — [00:00:00] Title identifies this as a CS294-196 MOOC lecture by Noam Brown on Multi-Agent AI. - Agentic AI MOOC UC Berkeley CS…

**VI agent (Sonnet 5, 1.5k cap)** — quality 0.54, P 1.00, R 0.45, facts 0.50, $0.216, 51 s
  
Missed: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Predictable Noise in LLM Benchmarks by Sida Wang; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Autonomous Agents by Peter Stone; Agentic AI MOOC | UC Berkeley CS294-196 F25 | Multi-Agent Systems in Era of LLMs by Oriol Vinyals; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | AI Agents to Automate Science by James Zou
  
Judge: The answer correctly states there are 11 MOOC talks and maps its five listed entries to real catalog videos with accurate speaker attributions and 00:00:00 timestamps. However, it is truncated mid-sentence and only names five of the eleven speakers, omitting Sida Wang, Peter Stone, Weizhu Chen, Yann Dubois, Oriol Vinyals and James Zou, so the speaker list fact is incomplete.
  
> I found 11 talks in the library from the UC Berkeley Agentic AI MOOC (CS294-196), Fall 2025: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [00:00:00] Opens with an "Agentic AI"/"Towards Building Safe & Secure Agentic AI" title slide and the speaker outlining the lecture on attacks and defenses in agentic AI . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Multi-Agent AI by Noam Brown — [00:00:00] Noam Brown introduces himself and the topic: "Hi, everybody. So yeah. I'm Noam Brown... I'll be talking about multi-agent AI" . - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Evolution of System Designs by Yangqing Jia — [00:00:00] Yangqing Jia o…

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

**VI agent (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.73 (n=15), $0.391, 48 s
  
Judge: The answer correctly identifies both ground-truth Microsoft talks (Jon Peck/Christopher Harrison with GitHub Copilot and Cedric Vidal with Azure AI Agent Service/Foundry) with specific timestamps, and no videos are invented. It adds two extra library items — the Vibe Coding workshop, attributed to Microsoft only via inferred slide branding without a named speaker, and Weizhu Chen's lecture (mentioned without timestamps and explicitly excluded as a product demo) — which are plausible but unverified attributions.
  
> Based on the searches, here are the talks I found with confirmed Microsoft speakers demonstrating Microsoft products: - Collaborating with Agents in your Software Dev Workflow - Jon Peck & Christopher Harrison, Microsoft [00:08:48], [00:44:02], [00:55:45]–[00:58:45] The two Microsoft presenters demo GitHub Copilot end-to-end: starting with code completions ("code completion is sort of the original version...of GitHub Copilot") , then walking through the Copilot coding agent workflow where an issue is assigned to Copilot, it opens a sandboxed environment, and produces a pull request for review , and finally showing Copilot generating git commands and reviewing PRs together with an MCP-server …

**VI agent (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.67 (n=6), $0.135, 26 s
  
Judge: The answer correctly identifies both Microsoft product talks with plausible timestamp ranges and the right products (GitHub Copilot; Azure AI Agent Service / AI Foundry), with no invented videos. It additionally flags Weizhu Chen's lecture as a Microsoft-affiliated speaker without a product demo, which is an extra but reasonably qualified inclusion.
  
> Collaborating with Agents in your Software Dev Workflow - Jon Peck & Christopher Harrison, Microsoft [00:33:01] - [00:53:45] Jon Peck and Christopher Harrison demonstrate using GitHub Copilot (including Copilot agent mode and Copilot workspace/instructions) integrated inside Visual Studio Code and GitHub to build, test, and edit application code [[cite:01M297QEC5K6FC7WXAZA3D2RZ7:1980-2040], [cite:01M297QEC5K6FC7WXAZA3D2RZ7:3185-3225]]. Building Code First AI Agents with Azure AI Agent Service — Cedric Vidal, Microsoft [00:44:18] - [01:45:00] Cedric Vidal demonstrates Azure AI Agent Service within Azure AI Foundry, showing hands-on live code examples integrating agent capabilities with Azure …

**VI retrieval-only (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=2), $0.062, 18 s
  
Judge: The answer correctly identifies both Microsoft talks with accurate product attributions—GitHub Copilot coding agent for Peck/Harrison with a supporting timestamp range, and Azure AI Agent Service for Cedric Vidal, though the latter is hedged as unverified and given no timestamp. No invented videos are present; the main weakness is the incomplete evidence and missing timestamp for the Azure talk, and the slightly equivocal framing of the second fact.
  
> Based on the searches run, here is what I can confirm from the evidence retrieved: - Collaborating with Agents in your Software Dev Workflow - Jon Peck & Christopher Harrison, Microsoft — [00:56:10]–[00:58:45]: The screen shows a markdown file titled "1-copilot-coding-agent.md" open in an editor with a chat/"Add Context..." panel, consistent with a demo of GitHub Copilot's coding agent workflow . I also note that the library contains another talk explicitly credited to a Microsoft speaker in its title — "Building Code First AI Agents with Azure AI Agent Service — Cedric Vidal, Microsoft" — which by its title would demonstrate Azure AI Agent Service, but my searches in this session did not re…

**VI agent (Sonnet 5, 1.5k cap)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=10), $0.129, 23 s
  
Judge: The answer names exactly the two correct Microsoft talks with valid catalog mappings and provides specific timestamp ranges for each, covering both key facts (GitHub Copilot agent mode/coding agent and Azure AI Agent Service / Azure AI Foundry). No invented videos or misattributions are apparent, though the timestamp accuracy for the specific claimed moments cannot be verified here.
  
> Based on the video titles and content, two talks in the library are explicitly given by Microsoft speakers: - Collaborating with Agents in your Software Dev Workflow - Jon Peck & Christopher Harrison, Microsoft [00:08:48–00:11:00] — The speakers introduce and demo GitHub Copilot, starting with code completions ("this is where I'm going... let's talk about code completions... the original version of GitHub Copilot") . [00:56:25–00:58:45] — They run a hands-on lab showing the GitHub Copilot coding agent generating git commands and handling a pull-request review workflow, with an MCP architecture diagram explaining how the agent accesses remote resources and requires user approval before acting…

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 1.00 (n=7), $0.354, 304 s
  
Judge: All three cited videos exist in the catalog and each is given specific timestamps, with both key facts (GitHub Copilot agent mode by Peck/Harrison and Azure AI Agent Service by Vidal) correctly attributed. The extra Vibe Coding workshop claim adds a Microsoft speaker attribution (Harald Kirschner) not in the key facts, which may or may not be accurate, but it does not misattribute content across the main talks.
  
> Collaborating with Agents in your Software Dev Workflow - Jon Peck & Christopher Harrison, Microsoft (01:04:05) [00:30:40]: Christopher Harrison demonstrates GitHub Copilot's coding agent automatically working on an assigned GitHub issue, executing inside GitHub Actions, and opening a draft pull request. [00:36:50]: He shows how to iterate with GitHub Copilot coding agent by submitting follow-up comments on the pull request to restart agent execution. [00:42:30]: Inside GitHub Codespaces (web-based Visual Studio Code), he demonstrates how GitHub Copilot utilizes repository-level copilot-instructions.md and custom .instructions files to guide coding agent and chat behavior. [Full Workshop] Vi…

## corpus-24: Which videos mention Sam Altman?

*Kind: negative.* Nobody says the name; it appears once on a slide in Dawn Song's lecture (acceptable). A good answer says no talk mentions him, or at most points at that slide.

Expected: no video
  
Acceptable, not required: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Agentic AI Safety & Security by Dawn Song

**VI agent (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=1), $0.242, 24 s
  
Judge: The answer names a single catalog video (Dawn Song's Agentic AI Safety & Security) with a precise timestamp and a plausible quoted slide, and explicitly dismisses other candidate hits as OCR noise. No invented videos appear, though the exclusive claim of only one mention may undercount, and the dismissal of the Notion/Braintrust talk is asserted without evidence.
  
> Based on my search, only one video in the library contains an explicit reference to Sam Altman. - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [00:00:28] A slide titled "2025 is the Year of Agents" displays a quoted tweet attributed to Sam Altman stating "We are now confident we know how to build AGI as we have traditionally understood it... in 2025, we may see the first AI agents 'join the workforce.'" Other apparent "Sam" hits (e.g., in the Notion/Braintrust product talk) turned out to be OCR noise from unrelated UI text (like "Doug Sandhu" or generic labels), not references to Sam Altman . Videos: 1

**VI agent (Gemini 3.8 Flash)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=1), $0.113, 25 s
  
Judge: The answer names one real catalog video (Dawn Song's Agentic AI Safety & Security lecture) with a precise timestamp and a plausible quoted Sam Altman slide, so no invented videos appear. However, it is likely incomplete, citing only a single mention across a library where other talks may also reference Sam Altman.
  
> - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song [00:00:28] A slide titled "2025 is the Year of Agents" displays a quote from Sam Altman stating, "We are now confident we know how to build AGI as we have traditionally understood it. We believe that in 2025, we may see the first AI agents join the workforce and materially change the output of companies" . Videos: 1

**VI retrieval-only (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, $0.027, 8 s
  
Judge: The answer names no videos at all and explicitly states that no library video was confirmed to mention Sam Altman, so there are no hallucinated videos or misattributions. It provides no timestamps and hedges by suggesting further searches, making it a null response rather than an incorrect attribution.
  
> Based on the search results, I did not find clear, specific evidence of "Sam Altman" being mentioned by name in any of the videos in this library. The search returned only tangential or unrelated matches (e.g., generic references to "video," data point counts, or unrelated OCR text) rather than actual discussion of Sam Altman. I was not able to confirm any video that explicitly mentions Sam Altman with the searches performed. A more targeted search (e.g., specifically for "Altman" or "OpenAI CEO") would be needed to verify this further, but based on current evidence, no qualifying mentions were found. Videos: 0

**VI agent (Sonnet 5, 1.5k cap)** — quality 1.00, P 1.00, R 1.00, timestamps near a mention 1.00 (n=1), $0.231, 23 s
  
Judge: The answer names a single catalog video (Dawn Song's Agentic AI Safety & Security) with a precise timestamp and a specific quoted slide, and explicitly dismisses other OCR-noise hits as irrelevant. No invented videos are cited, though the single-video claim may be under-inclusive and the attribution rests on an unverified slide screenshot.
  
> Based on searches across the entire library, the only video where Sam Altman is genuinely mentioned (by name, in a slide screenshot) is: - Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 Agentic AI Safety & Security by Dawn Song — [00:00:28] The opening slide displays a tweet attributed to Sam Altman stating "We are now confident we know how to build AGI as we have traditionally understood it," shown as part of Dawn Song's framing of rapid frontier-AI advancement . Other apparent hits (e.g., stray "SAM," "Alt," "Altcn" OCR fragments in videos about Braintrust, SGLang, Copilot, Autonomous Agents, Mixture of Agents) were checked and are unrelated OCR noise from code/UI text, not references to …

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, $0.107, 44 s
  
Judge: The answer asserts that no video in the library mentions Sam Altman, naming no videos and giving no timestamps. There are no invented videos or misattributions, but the blanket negative claim provides no supporting evidence.
  
> None of the videos in the library mention Sam Altman. Videos: 0

## corpus-25: Which videos talk about using an LLM as a judge for evaluation?

*Kind: mention.* 

Expected: Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize; How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust); [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; Evals 101 — Doug Guthrie, Braintrust; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor
  
Acceptable, not required: RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI

**VI agent (Sonnet 5)** — quality 0.73, P 0.80, R 0.67, timestamps near a mention 0.93 (n=14), $0.436, 43 s
  
Wrong: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen
  
Missed: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor
  
Judge: All six named videos map to real catalog entries, each with specific timestamp ranges and plausible speaker-attributed evidence about LLM-as-judge usage. No invented videos or untimestamped mentions appear, though the attributions cannot be verified here and some (e.g., the Weizhu Chen grader snippet) are brief and could be loosely characterized.
  
> Based on my research, here are the videos in the library that discuss using an LLM as a judge for evaluation: - RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI — [00:11:06]–[00:11:31] Ilan notes that reinforcement fine-tuning is "a really interesting candidate for training an LLM judge model" since grading a problem is itself a verifiable task, and later revisits "LLM as a judge model" concepts around [00:55:03]–[00:56:21] . - Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize — [00:42:38]–[00:47:16] The speaker builds and demos an "LLM-as-a-Judge" evaluator in Arize's platform to classify chatbot responses as "friendly" or "robotic," then checks it agains…

**VI agent (Gemini 3.8 Flash)** — quality 0.80, P 1.00, R 0.67, timestamps near a mention 1.00 (n=8), $0.153, 32 s
  
Missed: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor
  
Judge: All five cited videos exist in the catalog and each claim carries specific timestamps, with evals-focused talks (Braintrust, Arize, Notion) being plausible core matches. The Yann Dubois AlpacaEval and OpenAI fine-tuning DPO-judge attributions are more tangential and could be misattributed, but no invented videos appear.
  
> The following videos discuss using an LLM as a judge for evaluation: How to build world-class AI products — Sarah Sachs (AI lead @ Notion) & Carlos Esteban (Braintrust) [00:13:08] Sarah Sachs details Notion's LLM-as-a-judge evaluation system, contrasting universal judging prompts (such as faithfulness and conciseness) with item-specific prompts to catch model regressions . [00:39:45] Carlos Esteban outlines best practices for LLM-as-a-judge scorers, including using larger models to judge smaller ones and calibrating judge outputs against human-annotated feedback . Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize [00:47:36] Aman Khan demonstrates using an LLM as a ju…

**VI retrieval-only (Sonnet 5)** — quality 0.29, P 1.00, R 0.17, timestamps near a mention 1.00 (n=2), $0.034, 15 s
  
Missed: How to build world-class AI products — Sarah Sachs (AI lead @ Notion) &  Carlos Esteban (Braintrust); [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; Evals 101 — Doug Guthrie, Braintrust; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | LLM Agents Overview by Yann Dubois; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor
  
Judge: The answer confidently cites only one video (Aman Khan's Arize talk) with a specific timestamp range, which is a plausible and well-supported attribution, and it invents no videos. However, it is incomplete and hedging: it names Evals 101, Strategies for LLM Evals, and RFT/DPO/SFT only as unverified 'candidates' without timestamps or evidence, so those likely-relevant videos are effectively not claimed.
  
> Based on the searches performed, the clearest and most explicit discussion of "LLM-as-a-judge" evaluation found in the library is: - Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize — [00:50:05]–[00:52:39]: The presenter builds and runs an LLM-as-a-Judge evaluator in Arize's platform, checking whether the judge's labels agree with human annotations on a travel-itineraries dataset, with an on-screen slide titled "Evaluate LLM data with an LLM-as-a-Judge" and the presenter saying "I'm using an LLM as a judge. You could use code as well. You don't have to use an LLM as a judge here…" I was not able to confirm explicit "LLM-as-a-judge" discussion in the other search res…

**VI agent (Sonnet 5, 1.5k cap)** — quality 0.73, P 0.80, R 0.67, timestamps near a mention 0.92 (n=12), $0.462, 46 s
  
Wrong: Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Training Agentic Models by Weizhu Chen
  
Missed: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han; Agentic AI MOOC | UC Berkeley CS294-196 Fall 2025 | Practical Lessons from Deploying...by Clay Bavor
  
Judge: All six named videos map to real catalog entries, each with specific timestamps and plausible LLM-as-judge content attributions (Braintrust evals, Arize PM evals, Notion, Yann Dubois, OpenAI fine-tuning, Weizhu Chen). No invented videos or untimestamped mentions appear, though the answer omits other likely candidates such as the LLM evals strategies workshop.
  
> Based on the search results, several videos discuss using an LLM as a judge for evaluation: - Evals 101 — Doug Guthrie, Braintrust [00:07:56], [00:14:57], [00:44:29], [00:45:20] — Explains that Braintrust scorers include "LLM as a judge," where an LLM is given output and criteria to assess quality (e.g., assigning a score), contrasted with code-based scorers and discussed in Q&A regarding how to validate whether the judge itself did a good job . - Shipping AI That Works: An Evaluation Framework for PMs – Aman Khan, Arize [00:50:05], [00:39:56] — Demonstrates running an "LLM as a Judge" to evaluate hallucination and friendliness, comparing its agreement with human labels and recommending usin…

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

**VI agent (Sonnet 5)** — quality 1.00, P 1.00, R 1.00, facts 1.00, $0.244, 66 s
  
Judge: The answer cites 17 real catalog videos with specific, plausible timestamps and no invented titles, and it organizes them into coherent cross-series themes with an explicit commonality/difference discussion. Coverage gaps remain: Daniel Han's RL/quantization workshop, the voice AI, RAG/multimodal retrieval, ComfyUI and VC-raising workshops, and the Jiao/Chen/Jia MOOC lectures (verifiable rewards, agentic model training, system design) are never mentioned, so the breadth of the AI Engineer applied track and parts of the Berkeley research track are underrepresented; the stated video count (19) also exceeds the videos actually cited.
  
> Top Themes Across the Library Theme 1 – What is an "AI Agent" & its components - Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB [00:03:02] "I like to define an AI agent as a system that uses a large language model..." - Building Code First AI Agents with Azure AI Agent Service — Cedric Vidal, Microsoft [00:03:08] Notes that even specialists disagree on the definition of "agent," which has evolved over time. - Agentic AI MOOC Autonomous Agents by Peter Stone [00:11:23]-[00:13:23] Slides define "intelligent agents" and "autonomous agents" as core academic framing for the rest of the lecture. Theme 2 – Evals as a discipline (both practical tooling and academic benchmarking)…

**VI agent (Gemini 3.8 Flash)** — quality 0.80, P 1.00, R 1.00, facts 0.80, $0.153, 45 s
  
Judge: All ten cited videos are real catalog entries with specific timestamp ranges, and attributions match the speakers and expected content (Song on safety, Brown on self-play, Wang on benchmark noise, Bigio on SFT/DPO/RFT). However, the answer omits much of the applied AI Engineer breadth — no mention of voice AI workshops, RAG/multimodal retrieval, ComfyUI image generation, LLM serving (SGLang), or the VC fundraising talk — and Daniel Han's RL/quantization workshop is not cited, so the hands-on applied theme is only generically asserted rather than evidenced.
  
> Overview of Themes, Commonalities, and Differences Top Themes: - Agent Architecture & Autonomy: Moving past single-turn prompt engineering toward multi-step reasoning, tool usage, environment interaction, and agent swarms/multi-agent collaboration. - Post-Training & Optimization: Advancing model capabilities via reinforcement learning (RL), supervised fine-tuning (SFT), direct preference optimization (DPO), reinforcement fine-tuning (RFT), and verifiable feedback loops. - Evaluation & Benchmarking: Developing robust evaluation harnesses, automated grading, and detecting predictable noise or leakage in agent benchmarks. - Safety, Security, & Systems Engineering: Hardening agentic systems agai…

**VI retrieval-only (Sonnet 5)** — quality 0.80, P 1.00, R 1.00, facts 0.80, $0.063, 31 s
  
Judge: All seven cited videos are real catalog entries with specific timestamp ranges, and the hands-on-vs-research contrast is drawn correctly, though the sample is thin (only 7 of 30 videos) and misses whole clusters such as the evals workshops (Braintrust, Arize, lm-eval-harness), MCP/A2A protocol sessions, fine-tuning with OpenAI, LLM serving, benchmark noise, agents for science and system design. The evaluation-as-shared-theme claim is asserted on weak evidence (Daniel Han's training pipeline is not an eval topic), making that attribution somewhat stretched, but no videos are invented and no content is grossly misattributed.
  
> Based on the search results gathered, here is a thematic summary across the library's two series. Common ground — both series center on "agents" as the core theme (building, training, and evaluating LLM-driven agents), just from different angles: - [Full Workshop] Building Conversational AI Agents - Thor Schaeff, ElevenLabs — [00:13:26]–[00:16:20], [00:18:00]–[00:19:58]: Hands-on build of a voice agent, including a live speech-to-text demo and a walkthrough of ElevenLabs' cloned-voice marketplace ("this is a marketplace... we actually just surpassed the $5 million US" in royalties) . - ComfyUI Full Workshop — first workshop from ComfyAnonymous himself! — [00:00:00]–[00:02:43]: ComfyAnonymous…

**VI agent (Sonnet 5, 1.5k cap)** — quality 0.40, P 1.00, R 1.00, facts 0.40, $0.235, 38 s
  
Judge: The answer cites only real catalog videos with specific timestamp ranges and no invented titles, and its evaluation and RL/post-training themes are well supported. However, it is truncated mid-sentence at "Common theme 4 — Multi-agent", so it never covers multi-agent systems/MCP-A2A as a shared agent-building theme, nor the contrast between AI Engineer's hands-on applied workshops (voice, RAG, serving, ComfyUI, VC) and Berkeley's research-lecture focus (safety, benchmark noise, science agents, system design).
  
> Cross-Series Theme Summary Common theme 1 — "What is an agent" and core agent architecture - Building Multimodal AI Agents From Scratch — Apoorva Joshi, MongoDB — [00:01:45]–[00:07:10] — Defines an AI agent and its four main components (LLM, tools, memory, planning) as the basis for multimodal agent systems . - Agentic AI MOOC Autonomous Agents by Peter Stone — [00:02:33]–[00:08:08] — Peter Stone frames an "agent" as any system perceiving and acting in an environment, spanning robotics, multi-agent systems, and RL . Common theme 2 — Reinforcement learning / RLHF as the mechanism behind modern agents - RFT, DPO, SFT: Fine-tuning with OpenAI — Ilan Bigio, OpenAI — [00:52:16]–[00:59:12] — Walks…

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

**VI agent (Sonnet 5)** — quality 0.25, P 1.00, R 1.00, facts 0.25, $0.200, 60 s
  
Judge: The answer correctly puts agents first and evaluation second and names only real catalog videos (no invented titles), but it provides zero timestamps and no mention counts or per-video/channel distribution. It is also truncated mid-sentence, so MCP and RAG are never ranked and the Berkeley-MOOC vs AI-Engineer concentration insight is missing; its claim that fine-tuning/RL is confined to a small set also conflicts with the broad spread in ground truth.
  
> Ranking (most to least discussed across the library) 1. Agents — by far the most pervasive topic. It is the explicit subject of entire dedicated talks (Peter Stone's "Autonomous Agents," Azure AI Agent Service, A2A & MCP Workshop, Multimodal AI Agents From Scratch, Noam Brown's and Oriol Vinyals' multi-agent talks, etc.) and comes up as a supporting theme in nearly every other talk in the index (evals talks frame themselves around "evaluating agents," fine-tuning talks discuss "agentic models," RAG talks discuss "agentic RAG"). Concentration: diffuse but massive — low per-video density but present almost everywhere. 2. Evaluation — the second-most common thread, with several full talks dedic…

**VI agent (Gemini 3.8 Flash)** — quality 0.50, P 1.00, R 1.00, facts 0.50, timestamps near a mention 0.73 (n=22), $0.200, 44 s
  
Judge: The ranking order (agents > evaluation > fine-tuning/RL > MCP > RAG) is close to ground truth and all cited videos are real catalog entries with plausible, specific timestamps, plus a correct note that agents dominate the Berkeley MOOC lectures. However, it quantifies concentration only by counting 'dedicated' sessions, badly understating how widely evaluation, fine-tuning/RL and MCP are mentioned (no mention counts, no ~22-talk or ~third-of-talks spread), and it names 3l0Zxus34es and HV8pugcFVO0 without any timestamps.
  
> Across the entire video library, agents is talked about the most by a wide margin . Ranking and Concentration 1. Agents (Highest concentration): Heavily dominates the library, accounting for over half of all videos (including nearly the entire UC Berkeley CS294-196 "Agentic AI MOOC" series and multiple practical workshops on building agent systems). 2. Evaluation (Moderate-to-high concentration): Featured as the central subject of roughly five dedicated presentations and workshops (covering production evaluation frameworks, benchmark noise, and eval tooling like Braintrust and GuideLLM) as well as dedicated evaluation modules in agent courses. 3. Fine-tuning / Reinforcement Learning (Moderat…

**VI retrieval-only (Sonnet 5)** — quality 0.25, P 1.00, R 1.00, facts 0.25, timestamps near a mention 0.83 (n=6), $0.070, 35 s
  
Judge: All six cited videos exist in the catalog with plausible, specific timestamps and no invented titles, and the top-line conclusion that agents dominate (concentrated in the Berkeley MOOC series) is conveyed. However, the ranking is largely inferred from titles rather than mention counts: it gives no per-topic spread or frequency figures, understates MCP (placing it 4th and calling it confined to one or two sessions rather than about a third of talks) and misorders it against RAG's relative footprint, and it omits that evaluation and fine-tuning/RL are comparable in breadth.
  
> Based on searches across the library (transcripts, OCR, and descriptions), here is a ranked view of how much each topic is discussed, followed by the concrete evidence found. Overall ranking (most → least discussed): 1. Agents – by far the most concentrated topic; roughly a third of the library's titles are explicitly agent-focused (the entire Berkeley "Agentic AI MOOC" series, plus dedicated workshops on building/orchestrating agents), and it recurs as a throughline even in talks nominally about other things (voice, evals, MCP). 2. Evaluation – a strong secondary theme, with several dedicated sessions (Evals 101, LLM Evals strategies, benchmark-noise talks) and it appears as a supporting to…

**VI agent (Sonnet 5, 1.5k cap)** — quality 0.25, P 1.00, R 1.00, facts 0.25, timestamps near a mention 1.00 (n=3), $0.155, 26 s
  
Judge: The answer is truncated: it only covers the 'agents' category and never ranks or quantifies evaluation, fine-tuning/RL, MCP, or RAG, so three of four key facts are missing. Videos cited are all real catalog entries with plausible timestamps, though the A2A & MCP Workshop is named with no timestamp or quote, and the breadth claim ('~15+ of 31 videos') understates the actual coverage and omits the concentration pattern in the Berkeley MOOC lectures.
  
> Based on searches across the whole library, here is the ranking with evidence. 1. Agents — most talked about, very broadly spread (low concentration, appears as a dominant theme in ~15+ of 31 videos, including an entire MOOC track). - Agentic AI MOOC Autonomous Agents by Peter Stone — [00:00:14] "my view of agents, autonomous agents have been a focus of study in the field of artificial intelligence for a long time" - Building Multimodal AI Agents From Scratch — [00:06:13] "we are building an AI agent today... use agents for..." - Building Code First AI Agents with Azure AI Agent Service — [00:01:50] presenter introduces a working definition of an "agent" for an e-commerce use case - A2A & MC…

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

**VI agent (Sonnet 5)** — quality 0.00, P 1.00, R 1.00, facts 0.00, timestamps near a mention 1.00 (n=7), $0.201, 50 s
  
Judge: The answer correctly puts GPT/ChatGPT first but gives no mention counts or talk counts, and it misranks the rest by elevating Claude to a tie with Gemini and placing Llama last instead of third. All three cited videos exist in the catalog with timestamps, though the Clay Bavor citation is truncated with no evidence, and the answer's overall ranking claims about Gemini/Claude/DeepSeek prevalence are unsupported by any specific citations.
  
> Based on hybrid search across the whole library, GPT/ChatGPT is clearly the most-mentioned model family, appearing constantly across nearly every talk (including an entire session built around GPT/ChatGPT). The rest rank roughly: Gemini and Claude next (both mentioned heavily in several dedicated demo/workshop segments), then DeepSeek, then Llama least often (though still present in several deep-technical talks). Ranking: 1. GPT/ChatGPT → 2. Gemini ≈ Claude → 3. DeepSeek → 4. Llama GPT / ChatGPT - How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS — Ishan Anand — [00:17:27], [01:23:18], [01:29:36], [01:32:03] — The whole talk is a deep dive contrasting GPT-2 with ChatGPT: "ChatGPT, L…

**VI agent (Gemini 3.8 Flash)** — quality 0.00, P 1.00, R 1.00, facts 0.00, timestamps near a mention 1.00 (n=3), $0.170, 44 s
  
Judge: The answer declares that no library content provides the ranking and instead cites three real catalog videos (Ishan Anand, James Zou, Yangqing Jia) with specific timestamps, none of which establishes the cross-corpus mention counts requested. No invented videos are claimed, but none of the three key facts (GPT first, Gemini second, Llama third with Claude/DeepSeek trailing) is conveyed, and the Zou attribution frames Claude as co-most-common, contradicting the ground truth ordering.
  
> Nothing in the video library contains an analysis or ranking of which model family is named most often across all the talks. However, individual talks discuss the relative usage or family relationships among these models: How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand: [00:17:27], the speaker points out a family tree diagram showing that popular models such as ChatGPT, Llama, Bard/Gemini, GPT-4, and Claude all descend from the foundational architecture of GPT-2 . Agentic AI MOOC UC Berkeley CS294-196 Fall 2025 AI Agents to Automate Science by James Zou: [00:47:22], a slide summarizes model usage across workshop paper submissions, showing GPT-4/5 and Claude as the mo…

**VI retrieval-only (Sonnet 5)** — quality 0.33, P 1.00, R 1.00, facts 0.33, timestamps near a mention 1.00 (n=7), $0.047, 20 s
  
Judge: The answer correctly ranks GPT/ChatGPT first and cites three real catalog videos with plausible timestamps, but it bases the ranking on only three videos rather than the full library, so it misses the per-talk counts and puts Llama second ahead of Gemini, contradicting facts 2 and 3. No videos were invented and all cited videos carry timestamps, though the evidence is thin and the ranking is explicitly hedged as indicative.
  
> Based on the search results available, here is what I found regarding mentions of each model family across the talks. Note: this is based on a targeted search sample, not an exhaustive transcript count of all 31 videos, so treat the ranking as indicative rather than a precise tally. Evidence found, video by video: - How LLMs work for Web Devs: GPT in 600 lines of Vanilla JS - Ishan Anand — [00:16:19]–[00:18:14] The speaker builds a "family tree" of LLMs, naming ChatGPT, Llama, Bard/Gemini, GPT-4 and Claude as all descending from GPT-2, and repeatedly references GPT-2/GPT-4/ChatGPT as the core subject of the talk . - Agentic AI MOOC Evolution of System Designs by Yangqing Jia — [00:10:30]–[00…

**VI agent (Sonnet 5, 1.5k cap)**: no scored answer

**Gemini agentic video (3×10 + merge)** — quality 1.00, P 1.00, R 1.00, facts 1.00, timestamps near a mention 0.91 (n=82), $0.181, 62 s
  
Judge: The answer gives the correct ordering (GPT/ChatGPT first, Gemini second, Llama third, with Claude and DeepSeek trailing at similar levels) and supports it with 22 real catalog videos, each with specific timestamps, so no videos are invented and none are left untimestamped. Counts deviate somewhat from ground truth (Gemini ~55 vs ~90, talk counts inflated), and some citations look shaky (e.g., a malformed [00:66:10] timestamp and possibly loose attributions like Llama in the Azure talk), but no clear misattribution to the wrong speaker is evident.
  
> Across all the talks in the library, GPT/ChatGPT is the model family named most often by a substantial margin, leading in both total mention count (~180+ mentions) and talk presence (named in 20 talks). Overall Ranking (across all talks): 1. GPT / ChatGPT (~180+ mentions across 20 talks) 2. Gemini (~55 mentions across 14 talks) 3. Llama (~52 mentions across 10 talks) 4. Claude (~39 mentions across 12 talks) 5. DeepSeek (~34 mentions across 9 talks) --- Evidence from Relevant Talks: [Full Workshop] Reinforcement Learning, Kernels, Reasoning, Quantization & Agents — Daniel Han (02:42:27) [00:07:45]: Daniel Han discusses how the release of ChatGPT in December 2022 demonstrated that instruction …
