Milestone 1: Retrieval Surfaces ✅

Goal: Add primitive-native search APIs and a composite planner surface

Deliverable: kv.search(), json.search(), etc. plus db.hybrid.search() that fuses results

Status: Planned (this is M6)

Success Criteria:

 SearchRequest/SearchResponse/DocRef types finalized

 Each primitive implements search() (scan fallback acceptable)

 Composite hybrid.search() orchestrates and fuses

 Snapshot-consistent search execution

 Budget enforcement works

 Deterministic ordering and stable pointers

Risk: Over-abstracting and accidentally forcing data movement or expensive normalization.

Milestone 2: Keyword Indexing Foundation

Goal: Make keyword search fast enough to feel native

Deliverable: Lightweight inverted index option for primitives with commit-consistent watermarking

Status: Planned

Success Criteria:

 Tokenizer + postings format implemented

 Incremental updates on commit for opt-in primitives

 Query-time top-k with bounded candidate work

 Index can be disabled with scan fallback still correct

 Benchmarks show big win vs scan for medium corpora

Risk: Write amplification and index consistency bugs under crash/recovery.

Milestone 3: Hello World Hybrid Retrieval

Goal: Validate fusion and multi-primitive retrieval end-to-end

Deliverable: KeywordTopK per primitive + RRF fusion in composite search

Status: Planned

Success Criteria:

 RRF implementation with deterministic tie-breaking

 Primitive weighting knobs (light-touch)

 Simple demo workload shows cross-primitive retrieval works

 Debug traces explain which primitive contributed each hit

Risk: If this is slow or opaque, iteration speed on future algorithms collapses.

Milestone 4: Evaluation Harness and Ground Truth

Goal: Turn retrieval into a measurable research loop

Deliverable: Bench harness + datasets + offline metrics + regression gates

Status: Planned

Success Criteria:

 Synthetic “agent memory” datasets for all primitives

 Query sets with expected hits

 Metrics: Recall@K, MRR, nDCG, latency p50/p95

 Automated regression tests for relevance and latency

Risk: Without a harness, “blazing fast” and “good retrieval” become vibes.

Milestone 5: Vector Retrieval (Later Milestone)

Goal: Add semantic retrieval as an additional retriever, not a replacement

Deliverable: Per-primitive vector search where relevant, plus composite fusion with keyword

Status: Planned

Success Criteria:

 Vector index primitive or module integrated cleanly

 Composite can fuse keyword + vector

 Budgets and determinism maintained

 Evaluation shows clear wins on semantic queries

Risk: Vectors can dominate engineering effort and derail the planner-centric design.

Milestone 6: Reranking and Multi-Step Retrieval

Goal: Human-like retrieval: generate candidates fast, rerank intelligently, iterate

Deliverable: Optional reranker hooks (model-based or heuristic) and multi-hop retrieval loops

Status: Planned

Success Criteria:

 Rerank interface that accepts top-N candidates

 Multi-step query expansion experiments

 Reranker runs under strict time budget and is optional

 Evaluation proves improved relevance

Risk: Latency blowups and non-determinism.

Milestone 7: Production Hardening

Goal: Make retrieval reliable at scale

Deliverable: Robust crash handling, index rebuild, observability, and tuning knobs

Status: Planned

Success Criteria:

 Index rebuild and verification tools

 Observability: per-stage timings, candidate counts, truncation reasons

 Backpressure and memory caps

 Stable behavior across durability modes

Risk: Index corruption and silent relevance regressions.