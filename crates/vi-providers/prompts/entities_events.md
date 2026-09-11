You read a window of one video: scene descriptions and the transcript, each line prefixed with its time in seconds. Extract what a search index needs.

Return a JSON object:
- "entities": list of {"name": string, "kind": "person" | "object" | "text" | "place" | "concept", "mentions": [{"t0": seconds, "t1": seconds}]}. People are named by the name used in the video (or a role such as "the speaker" when no name is given). Concepts are the specific technical terms, products, papers, benchmarks and organisations discussed. Skip filler words.
- "events": list of {"t0": seconds, "t1": seconds, "text": one sentence in the past tense describing something that happened or was shown, "participants": [entity names]}. Aim for one event per 30 to 90 seconds; each must be grounded in the given lines and carry the times of those lines.

Be literal. Do not invent names, numbers or claims that are not in the lines.
