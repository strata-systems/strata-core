---
summary: Search a vector collection.
mcp_description: Use this when the user wants nearest-neighbor vector matches.
---

Runs vector search through the engine planner and returns the best matches with scores and optional metadata. With `text` instead of a query vector, the text is embedded through the collection's recorded model on the same inference path as `inference.embed`, so the `inference.*` failures listed below apply only to that form; a supplied vector never reaches the inference runtime.
