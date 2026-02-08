# Strata: Paper Strategy

**Goal**: Publish 3-4 papers targeting distinct research communities, each backed by benchmark results from established public evaluation frameworks.

**Prerequisite**: The [benchmark suite](./benchmarks.md) must be implemented and results collected before any paper is submitted. Benchmarks provide the empirical grounding that separates a publishable contribution from a position paper.

---

## Paper 1: The core systems paper

**Title direction**: "Strata: A Branch-Native Embedded Database for Autonomous Agent Workloads"

**Venue**: CIDR (primary), VLDB Industry Track, or SIGMOD Industry Track

**Contribution**: A unified multi-primitive storage model with branch-native MVCC. Six data models (KV, JSON, event log, state cells, vectors, branch metadata) stored in one transactional substrate with type-tagged keys, immutable version chains, and first-class fork/diff/merge. The thesis: agent workloads need a fundamentally different data model than traditional applications — branches as isolation boundaries, not tables or collections.

### Key claims

1. **Unified type-tagged storage** is simpler and more efficient for multi-model workloads than composing separate systems (SQLite + vector extension + event store + git).
2. **Branch-native MVCC** with fork/diff/merge enables agent isolation patterns (speculative execution, sandboxing, reproducible replay) that are impossible or prohibitively expensive in conventional databases.
3. **Three versioning schemes** (Txn for atomicity, Sequence for append-only logs, Counter for CAS) naturally fit their respective primitives better than forcing a single scheme across all data types.
4. **Immutable version chains** with user-facing history (`getv()`) turn the storage layer into an audit log without external tooling.

### Evaluation

- **YCSB** (Workloads A-F) across all three durability modes, compared against RocksDB, LMDB, SQLite
- **Branch operation microbenchmarks**: fork, diff, merge latency at 10K / 100K / 1M entries
- **Version chain overhead**: read latency as version depth grows (1, 100, 10K versions per key)
- **Multi-primitive transaction throughput**: commits spanning KV + Event + State + JSON vs. equivalent multi-system composition
- **Composed system comparison**: SQLite + sqlite-vss + git performing the same fork/diff/merge/search workflow, measuring integration overhead

### Why this venue

CIDR (Conference on Innovative Data Systems Research) explicitly values new system designs with strong opinions and clear scope decisions. Strata's thesis — "agent workloads need branches, not tables" — fits CIDR's emphasis on novel architectures. The industry tracks at VLDB and SIGMOD are alternatives if the evaluation is comprehensive enough.

---

## Paper 2: The retrieval/search paper

**Title direction**: "Hybrid Search Across Heterogeneous Data Primitives with Reciprocal Rank Fusion"

**Venue**: SIGIR (primary), CIKM, or ECIR

**Contribution**: Most hybrid search research fuses keyword and vector results over a single document collection. Strata fuses across four distinct primitive types (KV, JSON, events, state) using shadow vector collections, BM25, and RRF. The paper evaluates whether cross-primitive fusion improves retrieval quality over single-primitive search.

### Key claims

1. **Cross-primitive hybrid search** (fusing results from KV, JSON, event, and state stores) outperforms single-primitive search on agent workloads where relevant information is distributed across data types.
2. **Shadow vector collections** (automatically maintained embeddings linked to source records via EntityRef) enable transparent semantic search across non-vector primitives without application-layer coordination.
3. **RRF normalizes heterogeneous score distributions** — BM25 scores, cosine similarity, and event relevance produce incomparable magnitudes, but RRF's rank-based fusion handles this gracefully.

### Evaluation

- **BEIR** (SciFact, FiQA, NFCorpus, TREC-COVID, Natural Questions): NDCG@10 for BM25-only, vector-only, and hybrid+RRF
- **Ablation study**: contribution of each primitive type to final result quality. Does searching events alongside documents improve retrieval? Under what query types?
- **RRF k-parameter sensitivity**: how does the smoothing constant affect fusion quality across heterogeneous result sets?
- **Comparison against published numbers** from Weaviate, Elasticsearch, Azure AI Search, and Vespa on the same BEIR datasets
- **Agent-specific evaluation**: construct a benchmark from realistic agent workloads (tool call logs + config objects + conversation fragments + state mutations) and measure retrieval quality when information spans multiple primitive types

### Why this venue

SIGIR is the top venue for information retrieval. The novelty is the heterogeneous primitive fusion — no published work evaluates hybrid search across KV, event, and document stores in one ranked list. The BEIR evaluation provides the standard methodology reviewers expect.

---

## Paper 3: The embedded ML paper

**Title direction**: "Zero-Dependency Transformer Inference for Database-Layer Semantic Indexing"

**Venue**: MLSys (primary), EuroMLSys workshop, or VLDB (ML+DB track)

**Contribution**: Pure-Rust MiniLM-L6-v2 inference (~500 lines of tensor operations) embedded directly in the storage engine, with automatic embedding on writes and best-effort semantics (ML failures never block transactions). The argument: embedding should be a database concern — like full-text indexing in PostgreSQL — not an application concern requiring external services.

### Key claims

1. **Database-layer embedding** (transparent auto-embed on writes) eliminates the consistency gap between source data and embeddings that plagues application-layer embedding pipelines.
2. **Zero-dependency inference** (pure Rust, no Python, no ONNX, no GPU) makes semantic search viable in embedded, edge, and resource-constrained environments where external ML runtimes are impractical.
3. **Best-effort semantics** (embedding failures logged, never propagated) is the correct failure model for a secondary index — analogous to how full-text index failures in PostgreSQL don't block INSERT.
4. **The embedding overhead is acceptable**: <2ms per write for MiniLM-L6-v2, with negligible impact on transaction throughput.

### Evaluation

- **Correctness**: MTEB scores for the pure-Rust implementation vs. the reference PyTorch implementation (must be identical or near-identical)
- **Inference performance**: tokenization, forward pass, and vector insert latency breakdown per write
- **Auto-embed throughput**: writes/sec with embedding on vs. off, across all durability modes, at various document sizes
- **RAGAS evaluation**: full RAG pipeline (Strata auto-embed + hybrid search as retriever + LLM generator) evaluated for context precision, recall, and faithfulness
- **Comparison against application-layer pattern**: separate embedding service (sentence-transformers) + vector DB (Qdrant/Weaviate) vs. Strata's integrated approach — measuring end-to-end latency, consistency guarantees, and operational complexity

### Why this venue

MLSys values practical systems contributions with rigorous evaluation. "We embedded a transformer in a database and measured what happened" with thorough benchmarks is a good MLSys paper. The reviewers will care about inference performance, engineering tradeoffs (why MiniLM, why pure Rust, why best-effort), and the end-to-end evaluation.

---

## Paper 4: The sync protocol paper

**Title direction**: "Git-Style Sync for Embedded Databases: A Dumb-Hub Protocol for Agent Collaboration"

**Venue**: SOCC (primary), EuroSys workshop, or VLDB (distributed data track)

**Dependency**: Requires cloud sync implementation to be shipped with real-world usage data.

**Contribution**: The origin mirror protocol for embedded database sync — delta-based incremental replication with version cursors, a dumb hub (object storage + stateless auth), and an edge compute layer for operational concerns. The thesis: embedded databases don't need server mode. They need a sync protocol where the remote is dumb storage and all intelligence stays on the client.

### Key claims

1. **Origin mirror branches** (analogous to git's remote tracking branches) provide a simple, correct mechanism for tracking sync state without requiring the hub to understand database semantics.
2. **The dumb-hub architecture** (object storage + stateless edge functions) scales to zero, requires no database runtime on the server, and delegates all merge intelligence to the client.
3. **Version cursors with monotonic ordering** eliminate the need for DAG resolution (unlike git), making the sync protocol simpler and more predictable for automated agents.
4. **Edge inference for NL search** demonstrates that query understanding can be offloaded to stateless compute at the boundary while data intelligence remains in the client.

### Evaluation

- **Sync protocol correctness**: formal argument for no data loss, no duplicate uploads, correct conflict detection
- **Push/pull performance**: latency vs. delta size, throughput under concurrent agents
- **Compaction effectiveness**: manifest-based compaction with GC delay, measured clone time improvement
- **Multi-agent collaboration scenarios**: measured overhead for shared-workspace, branch-per-agent, and speculative execution patterns
- **Comparison against CouchDB/PouchDB replication**: the closest existing system for embedded database sync

### Why this venue

SOCC (Symposium on Cloud Computing) covers the intersection of distributed systems and cloud infrastructure. The edge architecture (stateless Workers + object storage) is a cloud-native design. The sync protocol for embedded databases is an underserved topic in the literature.

---

## Reviewer objections to anticipate

### "Why not just use SQLite + extensions + git?"

The composed system comparison (Paper 1 evaluation) must quantify the integration overhead. Key differentiators to demonstrate:
- A single transaction spanning KV + event + vector + state is impossible in a composed system without a coordination layer
- Branch fork/diff/merge operates on the entire database state atomically — git only tracks files, not structured data
- Auto-embed on writes maintains consistency between data and embeddings — a composed system requires application-layer coordination

### "How does this scale?"

Be explicit: Strata is embedded, designed for single-node agent workloads, not distributed scale. Define the target clearly (databases up to tens of GB, thousands of branches, millions of keys) and show it performs well within that envelope. CIDR and MLSys respect clear scope decisions. VLDB may push harder on scalability — the industry track is more forgiving here.

### "Is the benchmark fair?"

Use established frameworks unchanged (BEIR, YCSB, ANN-Benchmarks, MTEB, RAGAS). Report all durability modes. Show where Strata loses, not just where it wins. Unfair benchmarks damage credibility more than honest losses.

### "What about larger embedding models?"

Acknowledge that MiniLM-L6-v2 is a quality-vs-deployability tradeoff. Show the MTEB comparison table. Discuss model swapping as future work. The contribution is the architecture (database-layer embedding with best-effort semantics), not the specific model.

---

## Writing order

| Priority | Paper | Reason |
|----------|-------|--------|
| 1 | Paper 2 (hybrid search / SIGIR) | Tightest scope, most straightforward evaluation, builds the BEIR benchmarking infrastructure that other papers reuse |
| 2 | Paper 3 (embedded ML / MLSys) | Shares evaluation infrastructure with Paper 2 (RAGAS uses the same search pipeline), distinct enough to develop in parallel |
| 3 | Paper 1 (core systems / CIDR) | Broadest scope, requires the most comprehensive evaluation (YCSB + branch microbenchmarks + composed system comparison) |
| 4 | Paper 4 (sync protocol / SOCC) | Blocked on cloud sync shipping with real usage data |

Papers 2 and 3 can be developed concurrently. Paper 1 builds on evaluation infrastructure from both. Paper 4 is independent but blocked on implementation.

---

## Publication timeline

| Milestone | Papers enabled |
|-----------|----------------|
| Benchmarks complete (BEIR + YCSB + ANN + RAGAS) | Papers 1, 2, 3 ready to write |
| Cloud sync shipped with usage data | Paper 4 ready to write |

### Conference deadlines (approximate annual cycles)

| Venue | Typical deadline | Notification |
|-------|-----------------|--------------|
| CIDR | June | October |
| VLDB (industry) | March | June |
| SIGMOD (industry) | October | February |
| SIGIR | January | April |
| CIKM | May | August |
| MLSys | October | February |
| SOCC | May | August |

Target the nearest deadline after benchmarks are complete. An arXiv preprint can go up at any time for visibility while the peer review cycle runs.
