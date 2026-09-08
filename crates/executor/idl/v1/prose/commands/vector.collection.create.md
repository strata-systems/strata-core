---
summary: Create a vector collection with a dimension, metric, and optionally the model that produces its vectors.
mcp_description: Use this when the user wants to add a new vector collection.
---

Creates a collection for dense vectors. The dimension and metric become part of the collection contract for future upserts and queries. Passing `embedding_model` records which model produces the collection's vectors: it lets `text` be embedded for upserts and queries, and lets a query embedded with a different model be refused rather than silently compared. A collection created without one can declare it later with `vector.collection.set_embedding_model`.
