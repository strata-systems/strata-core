---
summary: Declare the embedding model a vector collection's vectors come from.
mcp_description: Use this when a collection has no recorded embedding model and the user wants to store or search it by text.
---

Records, once, which embedding model produces this collection's vectors. A declaration, not a verification: a stored vector carries no model, so this takes the caller's word for the vectors present; from then on `text` on upsert and query is embedded with this model and no other, and a collection with no recorded model cannot embed `text` at all. A vector supplied directly is not checked against the record — it cannot be — so supplying one remains the caller's statement that the recorded model produced it. Declaring the model a collection already records is a no-op and commits nothing. Declaring a different one is refused with `failed_precondition.engine.embedding_model_mismatch`: the stored vectors came from the recorded model, so create a separate collection for the other one. The current wire response uses the collection-list output with one item.
