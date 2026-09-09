---
summary: Insert or replace one vector.
mcp_description: Use this when the user wants to store a vector embedding and optional metadata.
---

Upserts one vector key with a dense embedding and optional metadata. The vector dimension must match the collection configuration. With `text` instead of a vector, the text is embedded through the collection's recorded model on the same inference path as `inference.embed`, so the `inference.*` failures listed below apply only to that form; a supplied vector never reaches the inference runtime.
