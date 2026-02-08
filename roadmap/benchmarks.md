# Strata Benchmarks

**Goal**: Build external credibility by running Strata against established public benchmarks across retrieval, search, vector performance, and storage throughput.

**Narrative**: Strata is not the fastest at any single dimension, but it's the only system where you get ACID transactions, versioned writes, branch isolation, hybrid search, and auto-embedding in one embedded package — and the performance is competitive on each axis individually.

---

## Durability modes as a benchmarking axis

Strata has three durability modes, each a legitimate operating mode for different workloads. Benchmarks should report results across all three where applicable, since the performance profile changes significantly:

| Mode | WAL | fsync | Write latency target | Data loss on crash | Use case |
|------|-----|-------|----------------------|--------------------|----------|
| **Cache** | None | Never | <3 µs | All data lost | Ephemeral caches, test harnesses, hot-path intermediaries |
| **Standard** | Yes | Periodic (~100ms / 1000 writes) | <30 µs | Last flush interval | Production default |
| **Always** | Yes | Every commit | ~6-10 ms | Zero | Financial, audit-critical |

Cache mode bypasses the WAL entirely and acts as a pure in-memory store. This is not a test mode — it's a valid deployment option for workloads that treat Strata as a fast structured cache with branch isolation, semantic search, and ACID transactions, where persistence isn't required.

For storage benchmarks (YCSB), reporting all three modes shows the full performance envelope. For search benchmarks (BEIR, ANN-Benchmarks), Cache mode isolates search performance from I/O overhead. For RAG benchmarks (RAGAS), Standard mode is the realistic configuration.

---

## Tier 1 — High impact, high feasibility

### 1. BEIR (hybrid search comparison)

**What it measures**: Zero-shot information retrieval across heterogeneous datasets. The defining benchmark for search quality.

**Why it matters for Strata**: This directly demonstrates the value of Strata's hybrid search architecture (BM25 + vector + RRF). Weaviate, Azure AI Search, Vespa, and Elasticsearch all publish BEIR numbers — Strata can produce an apples-to-apples comparison.

**Datasets** (start with these 5):

| Dataset | Documents | Queries | Domain |
|---------|-----------|---------|--------|
| SciFact | 5K | 300 | Scientific claim verification |
| FiQA | 57K | 648 | Financial Q&A |
| NFCorpus | 3.6K | 323 | Medical/nutrition |
| TREC-COVID | 171K | 50 | COVID-19 research |
| Natural Questions | 2.7M | 3.5K | Open-domain Q&A |

**Metric**: NDCG@10 (Normalized Discounted Cumulative Gain at rank 10)

**What to report** (per dataset):

| Search mode | NDCG@10 |
|---|---|
| BM25 only | X |
| Vector only (MiniLM-L6-v2) | Y |
| Hybrid (BM25 + vector + RRF) | Z |

The story: hybrid search with RRF should beat both BM25-only and vector-only. Published results from Weaviate show hybrid boosting NDCG@10 by up to 42% over pure vector on certain datasets. Azure AI Search and Vespa report similar gains.

**How to run**: The `beir` Python library provides all datasets with a simple API. Write an adapter that indexes documents into Strata (auto-embed for vector, BM25 for keyword), runs the query set through each search mode, and computes NDCG@10 via `pytrec_eval`. The smaller datasets (SciFact, NFCorpus) run in minutes on a laptop. Natural Questions requires more memory but is tractable on a single machine.

**Reference**:
- [github.com/beir-cellar/beir](https://github.com/beir-cellar/beir)
- [arxiv.org/abs/2104.08663](https://arxiv.org/abs/2104.08663)
- [Weaviate BEIR benchmarks](https://github.com/weaviate/weaviate-BEIR-benchmarks)
- [Azure AI Search hybrid results](https://techcommunity.microsoft.com/blog/azure-ai-foundry-blog/azure-ai-search-outperforming-vector-search-with-hybrid-retrieval-and-reranking/3929167)

---

### 2. ANN-Benchmarks (vector search performance)

**What it measures**: Throughput (queries per second) vs. recall for approximate nearest neighbor search. The standard for comparing vector search implementations.

**Why it matters for Strata**: Places Strata's HNSW implementation on the same Pareto frontier as hnswlib, Faiss, Annoy, and ScaNN. Strata has ACID overhead that raw libraries don't — the honest story is "competitive recall with transactional guarantees."

**Datasets**:

| Dataset | Dimensions | Vectors | Distance |
|---------|------------|---------|----------|
| GloVe-100 | 100 | 1.2M | Angular |
| SIFT-128 | 128 | 1M | Euclidean |

**Metric**: Recall@10 vs. queries per second (Pareto plot)

**What to report**: Standard ANN-Benchmarks Pareto plot showing Strata alongside existing algorithms. Run in all three durability modes:
- Cache mode isolates pure search performance (no I/O)
- Standard mode shows realistic production performance
- Always mode shows the cost of per-write durability

**How to run**: Add a folder in `ann_benchmarks/algorithms/strata/` with a `config.yml` and Python wrapper that calls Strata's vector search API. Run `python run.py --dataset glove-100-angular` and `python plot.py`. Can be done in a day.

**Reference**:
- [ann-benchmarks.com](https://ann-benchmarks.com/)
- [github.com/erikbern/ann-benchmarks](https://github.com/erikbern/ann-benchmarks)

---

### 3. YCSB (storage throughput)

**What it measures**: Key-value store performance across standardized workloads. The gold standard for database benchmarking that every database developer recognizes.

**Why it matters for Strata**: Benchmarks the storage layer independent of search features. Directly comparable to RocksDB, LMDB, SQLite, and WiredTiger.

**Workloads**:

| Workload | Description | Read/Write ratio |
|----------|-------------|------------------|
| A | Update heavy | 50% read, 50% update |
| B | Read mostly | 95% read, 5% update |
| C | Read only | 100% read |
| D | Read latest | Read recently inserted |
| E | Short ranges | Short range scans |
| F | Read-modify-write | Read, modify, write back |

**Metric**: ops/sec throughput, p50/p99 latency

**What to report**: Results across all three durability modes and all six workloads, compared against RocksDB, LMDB, and SQLite. Cache mode is particularly interesting here — it shows Strata's in-memory performance ceiling, which should be competitive with or better than disk-backed stores on read-heavy workloads.

| Engine | Workload A (ops/s) | Workload B (ops/s) | Workload C (ops/s) | ... |
|--------|-------|-------|-------|-----|
| Strata (Cache) | | | | |
| Strata (Standard) | | | | |
| Strata (Always) | | | | |
| RocksDB | | | | |
| LMDB | | | | |
| SQLite WAL | | | | |

**How to run**: The C++ YCSB harness (`YCSB-cpp`) already supports the comparison targets. Write a thin adapter that maps YCSB operations to Strata's KV API via FFI. Datasets are generated synthetically — no data download required.

**Reference**:
- [github.com/ls4154/YCSB-cpp](https://github.com/ls4154/YCSB-cpp)
- [github.com/unum-cloud/ucsb](https://github.com/unum-cloud/ucsb) (alternative with cache invalidation between phases)

---

## Tier 2 — High impact, moderate effort

### 4. MTEB (embedding model positioning)

**What it measures**: Embedding model quality across 56+ datasets spanning classification, clustering, retrieval, and semantic similarity.

**Why it matters for Strata**: Transparency about the auto-embed model. MiniLM-L6-v2 scores ~56 on the overall MTEB average — respectable for its size (22M parameters, 384 dimensions) but clearly trading accuracy for speed and deployability. Being honest about this tradeoff builds trust.

**What to report**: Cite MiniLM-L6-v2's published MTEB scores alongside the tradeoff rationale:

| Model | MTEB avg | Dimensions | Inference | Dependencies |
|-------|----------|------------|-----------|--------------|
| MiniLM-L6-v2 (Strata built-in) | ~56 | 384 | ~1-2ms, pure Rust | Zero |
| E5-large-v2 | ~64 | 1024 | ~15ms, Python/ONNX | PyTorch |
| text-embedding-3-large (OpenAI) | ~67 | 3072 | API call | Network + API key |

The story: Strata chose the model that can run in-process with zero external dependencies at sub-2ms latency. For most agent workloads (tool call logs, config objects, conversation fragments), the quality tradeoff is acceptable. For workloads that need higher quality, model swapping is a future feature.

**How to run**: No need to run — cite existing leaderboard data. If Strata adds model swapping, run MTEB on each supported model.

**Reference**:
- [huggingface.co/spaces/mteb/leaderboard](https://huggingface.co/spaces/mteb/leaderboard)
- [arxiv.org/abs/2210.07316](https://arxiv.org/abs/2210.07316)

---

### 5. RAGAS (end-to-end RAG evaluation)

**What it measures**: Quality of retrieval-augmented generation pipelines across context precision, context recall, faithfulness, and answer relevancy.

**Why it matters for Strata**: Benchmarks the full product experience — insert documents, auto-embed, hybrid search, feed to LLM. This is what developers actually care about when evaluating Strata for RAG applications.

**Metric**: Scores on a 0–1 scale (>0.8 is strong)

| Metric | What it measures |
|--------|-----------------|
| Context Precision | Are relevant documents ranked higher than irrelevant ones? |
| Context Recall | Are all relevant pieces of information retrieved? |
| Faithfulness | Is the generated answer factually consistent with retrieved context? |
| Answer Relevancy | Does the answer address the user's query? |

**What to report**: A reference RAG pipeline built on Strata, evaluated on a standard dataset:

```
Documents → Strata (auto-embed, Standard durability)
    → hybrid search (BM25 + vector + RRF)
    → top-k results fed to LLM (Claude / GPT-4o)
    → RAGAS evaluation
```

**How to run**: The `ragas` Python library provides the evaluation framework. Build an adapter that uses Strata as the retriever. Requires an LLM judge for scoring (API cost, but manageable). Run on a standard QA dataset (Natural Questions or SQuAD subset).

**Reference**:
- [docs.ragas.io](https://docs.ragas.io/)
- [arxiv.org/abs/2309.15217](https://arxiv.org/abs/2309.15217)

---

## Tier 3 — Good for credibility, heavier lift

### 6. Big-ANN-Benchmarks streaming track

**What it measures**: Concurrent insert + search performance at scale. The NeurIPS 2023 competition included a streaming track that interleaves insertions and queries.

**Why it matters for Strata**: This is the only public benchmark that tests what Strata actually does in production — ACID writes and vector search happening concurrently. Other ANN benchmarks assume a static index.

**Reference**:
- [big-ann-benchmarks.com](https://big-ann-benchmarks.com/)
- [github.com/harsha-simhadri/big-ann-benchmarks](https://github.com/harsha-simhadri/big-ann-benchmarks)

### 7. MS MARCO passage retrieval

**What it measures**: Large-scale retrieval on 8.8M passages with real Bing queries. The standard training and evaluation set for retrieval systems.

**Why it matters for Strata**: Validates that Strata handles non-toy corpus sizes. MRR@10 is the standard metric. If Strata's hybrid search produces competitive MRR@10 on MS MARCO, that's a strong credibility signal.

**Reference**:
- [microsoft.github.io/msmarco](https://microsoft.github.io/msmarco/)

---

## Strata-specific benchmarks (no public equivalent)

Some of Strata's unique features have no existing public benchmark. These are worth designing and publishing as original benchmarks:

### Branch operation performance

No public benchmark tests database branch operations. Strata should publish its own numbers:

| Operation | Dataset size | Latency |
|-----------|-------------|---------|
| Fork branch | 10K entries | |
| Fork branch | 100K entries | |
| Diff branches | 1K changes across 100K entries | |
| Merge branches (LWW) | 1K changes | |
| Merge branches (Strict, no conflicts) | 1K changes | |
| Bundle export | 100K entries | |
| Bundle import | 100K entries | |

### Version chain performance

No other embedded database has user-facing version chains. Benchmark the cost:

| Operation | Versions per key | Latency |
|-----------|-----------------|---------|
| kv_get (latest) | 1 version | |
| kv_get (latest) | 100 versions | |
| kv_get (latest) | 10K versions | |
| kv_getv (full history) | 100 versions | |
| kv_put (append version) | 1 existing | |
| kv_put (append version) | 10K existing | |

### Auto-embed throughput

End-to-end write throughput with auto-embedding enabled vs disabled:

| Mode | Auto-embed | Writes/sec | Latency (p50) | Latency (p99) |
|------|------------|------------|---------------|---------------|
| Cache | Off | | | |
| Cache | On | | | |
| Standard | Off | | | |
| Standard | On | | | |

### Multi-primitive transaction throughput

Benchmark transactions that write across multiple primitives in a single commit (KV + Event + State + JSON), which is a common agent workload pattern that other databases can't do natively.

---

## Implementation plan

### Phase 1: BEIR + YCSB

Start with the two benchmarks that produce the most legible results. BEIR demonstrates search quality (the unique value prop). YCSB demonstrates storage competitiveness (table stakes credibility).

Deliverables:
- Python benchmark harness for BEIR with Strata adapter
- YCSB-cpp adapter for Strata (FFI bridge)
- Results on 3 BEIR datasets (SciFact, FiQA, NFCorpus) and 3 YCSB workloads (A, B, C)
- Blog post with comparison tables

### Phase 2: ANN-Benchmarks + Strata-specific

Add vector search positioning and publish the Strata-specific benchmarks that no one else can run.

Deliverables:
- ANN-Benchmarks integration (GloVe-100, SIFT-128)
- Branch/version/auto-embed microbenchmarks
- Results published to project documentation

### Phase 3: RAGAS + scale

Full RAG evaluation and larger-scale benchmarks.

Deliverables:
- RAGAS evaluation pipeline
- MS MARCO passage retrieval (if warranted by adoption)
- Big-ANN streaming track (if warranted by use case)
