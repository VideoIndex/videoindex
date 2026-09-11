# VideoIndex project.

 APIs to index and query videos.

 ## Goal

 VideoIndex is an SDK, and a infra-framework in rust with python bindings to process long videos using various VLM, OCR and audio models to create a knowledge base that 
 can be queried in an interactive fashion and other agentic applications can be built on top of it.

 The architecture is to split the SDK/framework from the app so that we can launch the SDK/framework as an open source project and potentially provide a hosted version for the SDK, similar to LlamaIndex.

 We shall build an interactive chat applications for video QnA.

 We shall evaluate against benchmarks mentioned here https://blog.google/innovation-and-ai/models-and-research/gemini-models/introducing-agentic-video-in-gemini/
 namely, LVBench, 1H-VideoQA and Minerva.

 For any backend AI models, we should abstract it out so that we can plug in both opensource and paid models from fronteir labs to evaluate VideoIndex A/B against different models.
