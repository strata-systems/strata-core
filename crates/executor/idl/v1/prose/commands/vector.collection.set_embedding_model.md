---
summary: Declare the embedding model a vector collection's vectors come from.
mcp_description: Use this when a collection has no recorded embedding model and the user wants to store or search it by text, or wants queries checked against the model that produced the stored vectors.
---

Records, once, which embedding model produces this collection's vectors. A collection created without a model accepts vectors from anywhere, so nothing can check that a query is comparable with what is stored, and `text` cannot be embedded for it; declaring the model fixes both. Declaring the model a collection already records is a no-op and commits nothing. Declaring a different one is refused with `failed_precondition.engine.embedding_model_mismatch`: the stored vectors came from the recorded model, so create a separate collection for the other one. The current wire response uses the collection-list output with one item.
