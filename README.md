# in-mem: State Substrate for AI Agents

**A memory, coordination, and replay foundation for building reliable AI agents**

## The Problem

AI agents today are non-deterministic black boxes. When they fail, you can't replay what happened. When they coordinate (like handling tool calls or managing state machines), you build fragile locking on top of Redis. When you need to debug multi-step reasoning, you're stuck with scattered logs.

**in-mem** solves this by giving agents what operating systems give programs: durable memory, safe coordination primitives, and deterministic replay.

## What is in-mem?

**in-mem is not a traditional database.** It's a state substrate for AI agents that need:

- **Durable Memory**: KV storage, JSON documents, vectors, and event logs that survive crashes
- **Safe Coordination**: Lock-free primitives for managing state machines and tool outputs
- **Deterministic Replay**: Reconstruct any agent execution exactly, like Git for runs
- **Fast Search**: Hybrid keyword + semantic search across all primitives

Think of runs as commits. Every agent execution is a `RunId`—a first-class entity you can replay, diff, fork, and debug. Just like you can `git checkout` any commit, you can replay any run and see exactly what the agent did.

### For Whom?

**in-mem is for people building agents**, not using them. If you're:

- Building an agent framework and need reliable state management
- Debugging why your agent made a decision 3 tool calls ago
- Coordinating between multiple agents or LLM calls
- Implementing deterministic testing for agent workflows

...then in-mem provides the substrate you'd otherwise build yourself on Redis + Postgres + custom replay logic.

### What in-mem Is NOT

- **Not a standalone vector database**: Use Qdrant/Pinecone for large-scale embedding search (we provide run-scoped vectors)
- **Not a general-purpose database**: Use Postgres/MySQL for application data
- **Not a cache**: Use Redis for hot ephemeral data
- **Not LangGraph/LangChain**: We're the state layer they can build on

**in-mem sits below agent frameworks**, providing the durable memory and replay guarantees they need.

## Features

### Seven Primitives for Agent State

All transactional. All replay-able. All tagged with the run that created them.

| Primitive | Purpose | Example Use |
|-----------|---------|-------------|
| **KVStore** | Working memory | Tool outputs, scratchpads, config |
| **EventLog** | Immutable history | Tool calls, decisions, audit trail |
| **StateCell** | CAS-based coordination | State machines, counters, locks |
| **TraceStore** | Structured reasoning | Confidence scores, alternatives |
| **RunIndex** | Run metadata | Status, tags, parent-child relationships |
| **JsonStore** | Structured documents | Conversation history, agent config |
| **VectorStore** | Semantic memory | Embeddings, similarity search, RAG context |

### Vector Store with Similarity Search

Run-scoped vector storage with collections and metadata filtering:

```rust
// Create a collection with cosine similarity
let config = VectorConfig::new(384)  // 384-dimensional vectors
    .with_metric(DistanceMetric::Cosine);
vector.create_collection(run_id, "memories", config)?;

// Insert vectors with metadata
vector.insert(run_id, "memories", "doc_1", &embedding, Some(json!({
    "source": "conversation",
    "timestamp": 1234567890
})))?;

// Search with optional metadata filtering
let filter = MetadataFilter::eq("source", "conversation");
let results = vector.search(run_id, "memories", &query_embedding, 10, Some(filter))?;
```

Features:
- **Collection management**: Create, delete, list collections per run
- **Metadata filtering**: Filter search results by JSON metadata
- **Three distance metrics**: Cosine, Euclidean, Dot Product
- **Full durability**: WAL + snapshots for crash recovery

### JSON with Path-Level Mutations

Native JSON primitive with fine-grained conflict detection:

```rust
// Create and mutate JSON documents
json.create(&run_id, "config", json!({"model": "gpt-4", "temp": 0.7}))?;
json.set(&run_id, "config", "$.temp", json!(0.9))?;

// Sibling paths don't conflict - concurrent writers can update different fields
// $.model and $.temp can be modified in parallel transactions
```

### Hybrid Search

Search across all primitives with BM25 keyword scoring and RRF fusion:

```rust
let request = SearchRequest::new(run_id, "error handling")
    .with_k(10)
    .with_budget(SearchBudget::default().with_time(50_000)); // 50ms

let response = db.hybrid().search(&request)?;
```

### Three Durability Modes

Choose your trade-off between speed and safety:

| Mode | Latency | Throughput | Data Loss on Crash |
|------|---------|------------|-------------------|
| **InMemory** | <3µs | 250K+ ops/sec | All |
| **Buffered** | <30µs | 50K+ ops/sec | Last ~100ms |
| **Strict** | ~2ms | ~500 ops/sec | None |

### Periodic Snapshots

Bounded recovery time with automatic WAL management:

```rust
db.configure_snapshots(SnapshotConfig {
    wal_size_threshold: 100 * 1024 * 1024,  // 100 MB
    time_interval_minutes: 30,
    retention_count: 2,
    snapshot_on_shutdown: true,
});
```

### Crash Recovery

Deterministic, idempotent, prefix-consistent recovery:

- **Deterministic**: Same WAL + Snapshot = Same state
- **Idempotent**: Replaying recovery produces identical state
- **Prefix-consistent**: No partial transactions visible
- **All primitives recovered**: KV, JSON, Event, State, Trace, Run, and Vector

```rust
// Check recovery result after restart
if let Some(result) = db.last_recovery_result() {
    println!("Recovered {} transactions", result.transactions_recovered);
}
```

### Deterministic Replay

Reproduce any agent execution exactly:

```rust
// Replay a completed run (read-only, side-effect free)
let view = db.replay_run(run_id)?;
println!("Run had {} events", view.events().len());

// Diff two runs to see what changed
let diff = db.diff_runs(run_a, run_b)?;
for entry in &diff.modified {
    println!("Changed: {:?}", entry.key);
}
```

### Run Lifecycle Management

Explicit lifecycle with orphan detection:

```rust
let run_id = RunId::new();
db.begin_run(run_id)?;

// Do work
db.kv.put(&run_id, "step", Value::String("started".into()))?;

// End run normally
db.end_run(run_id)?;

// After restart: detect runs that crashed mid-execution
for orphan in db.orphaned_runs()? {
    println!("Orphaned run: {:?}", orphan);
}
```

## Quick Start

```rust
use in_mem::{Database, DurabilityMode, Value};
use std::sync::Arc;

// Open with buffered durability (fast + durable)
let db = Arc::new(Database::builder()
    .path("./agent-state")
    .buffered()
    .open()?);

// Every agent execution is a run
let run_id = db.begin_run()?;

// Use primitives to manage state
db.kv.put(&run_id, "thinking", Value::String("analyzing query".into()))?;
db.event.append(&run_id, "tool_call", json!({"tool": "search"}))?;
db.state.set(&run_id, "status", Value::String("working".into()))?;

// Store and search vectors
let vector = VectorStore::new(db.clone());
vector.create_collection(run_id, "context", VectorConfig::new(384))?;
vector.insert(run_id, "context", "chunk_1", &embedding, None)?;

// End the run (makes it replay-able)
db.end_run(run_id)?;

// Later: replay this exact execution
let view = db.replay_run(run_id)?;
```

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
in-mem = "0.8"
```

Or clone and build:

```bash
git clone https://github.com/anibjoshi/in-mem.git
cd in-mem
cargo build --release
cargo test --all
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│              API Layer (embedded/rpc/mcp)               │
└───────────────────────────┬─────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────┐
│  Primitives (KV, EventLog, StateCell, Trace, RunIndex,  │
│              JsonStore, VectorStore)                    │
└───────────────────────────┬─────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────┐
│  Search Layer (HybridSearch, BM25, InvertedIndex, RRF)  │
└───────────────────────────┬─────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────┐
│       Engine (Database, Run Lifecycle, Coordinator)     │
└───────┬───────────────────┬───────────────────┬─────────┘
        │                   │                   │
┌───────▼───────┐   ┌───────▼───────┐   ┌───────▼───────┐
│  Concurrency  │   │   Durability  │   │    Replay     │
│(OCC/Txn/CAS)  │   │(WAL/Snapshot) │   │(RunView/Diff) │
└───────────────┘   └───────────────┘   └───────────────┘
```

**Key Design Choices**:

1. **Unified Storage**: All primitives share one sorted map. Enables atomic multi-primitive transactions.

2. **Run-Tagged Keys**: Every key includes its `RunId`. Replay is O(run size), not O(history size).

3. **Optimistic Concurrency**: Lock-free transactions with compare-and-swap. Agents rarely conflict.

4. **Batched Durability**: fsync batched by default. Agents prefer speed; losing 100ms of work is acceptable.

5. **Stateless Facades**: All primitives are stateless—they hold only a Database reference. Multiple instances are safe, no warm-up needed, idempotent retry works correctly.

See [Architecture Overview](docs/reference/architecture.md) for technical details.

## Performance

| Metric | Target |
|--------|--------|
| InMemory put | <3µs |
| InMemory throughput (1 thread) | 250K ops/sec |
| InMemory throughput (4 threads) | 800K+ ops/sec |
| Buffered put | <30µs |
| Buffered throughput | 50K ops/sec |
| Fast path read | <10µs |
| Snapshot write (100MB) | < 5 seconds |
| Full recovery (100MB + 10K WAL) | < 5 seconds |
| Replay run (1K events) | < 100 ms |
| Vector search (10K vectors) | < 50ms |
| Vector insert | < 100µs |

## Documentation

- **[Getting Started](docs/reference/getting-started.md)** - Installation, patterns, best practices
- **[API Reference](docs/reference/api-reference.md)** - Complete API documentation
- **[Architecture](docs/reference/architecture.md)** - How in-mem works internally
- **[WAL Entry Types](docs/architecture/WAL_ENTRY_TYPES.md)** - WAL format and entry allocation

## Development

### Workspace Structure

```
in-mem/
├── crates/
│   ├── core/           # Core types (RunId, Key, Value)
│   ├── storage/        # UnifiedStore + primitive extension
│   ├── concurrency/    # OCC transactions
│   ├── durability/     # WAL + snapshots + recovery
│   ├── primitives/     # 7 primitives (KV, Event, State, Trace, Run, JSON, Vector)
│   ├── search/         # Hybrid search + BM25 + inverted index
│   └── engine/         # Database orchestration + replay
├── tests/              # Integration tests (M1-M8 comprehensive)
├── benches/            # Performance benchmarks
└── docs/               # Documentation
```

### Running Tests

```bash
# All tests
cargo test --all

# Specific crate
cargo test -p in-mem-durability

# Integration tests
cargo test --test '*'

# Stress tests (run sequentially)
cargo test --test m6_comprehensive -- --ignored --test-threads=1

# Benchmarks
cargo bench
```

## Why Not Just Use Redis + Postgres?

You *can* build this yourself. Most agent frameworks do. But you'll end up with:

- **Fragile replay**: Scanning logs and hoping you capture everything
- **Locking hell**: Redis locks for coordination, race conditions everywhere
- **No causality**: Events in Postgres have timestamps, not causal relationships
- **Manual versioning**: Tracking what changed when, rolling back partial runs
- **Separate vector DB**: Another service to manage for embeddings

**in-mem gives you all of this out of the box**, designed for agents from the ground up.

## Roadmap

**Complete (M1-M8)**:
- Foundation (storage, WAL, recovery)
- Transactions (OCC, snapshot isolation)
- Primitives (KV, EventLog, StateCell, TraceStore, RunIndex, JsonStore, VectorStore)
- Performance (250K+ ops/sec, three durability modes)
- JSON Primitive (path-level mutations, region-based conflict detection)
- Retrieval (hybrid search, BM25, inverted index)
- Durability (snapshots, crash recovery, replay, run lifecycle)
- Vector Primitive (collections, similarity search, metadata filtering, WAL + snapshots)

**Next**:
- Python Client
- Security (authentication, authorization, multi-tenancy)
- Production Readiness (observability, deployment)
- Distributed Mode (replication, sharding)

See [MILESTONES.md](docs/milestones/MILESTONES.md) for detailed roadmap.

## FAQ

**Q: Is this a replacement for Redis/Postgres?**
A: No. in-mem complements traditional databases. Use Postgres for application data, Redis for caching, Qdrant for large-scale vector search. Use in-mem for agent state that needs replay and coordination.

**Q: Why not just use SQLite?**
A: SQLite is great for relational data but doesn't have run-scoped operations, deterministic replay, or causality tracking built in. You'd build in-mem's features yourself on top of SQLite.

**Q: Is this production-ready?**
A: Yes for embedded use. Comprehensive test coverage (M1-M8), crash recovery verified, performance benchmarked. Network layer and distributed mode are planned.

**Q: What about horizontal scaling?**
A: Currently embedded (in-process). Distributed mode is planned. For now, use multiple in-mem instances with agent-level sharding.

**Q: Can I use this with LangChain/LangGraph?**
A: Yes! in-mem sits below agent frameworks. They can use in-mem for state management instead of building custom persistence.

**Q: How does VectorStore compare to dedicated vector DBs?**
A: VectorStore is designed for run-scoped agent memory, not large-scale similarity search. It provides durability (WAL + snapshots), collection management, and metadata filtering—all scoped to runs. For million-scale embeddings, use a dedicated vector DB; for agent context and working memory, use in-mem.

## License

[MIT License](LICENSE)

## Contact

- **GitHub**: [anibjoshi/in-mem](https://github.com/anibjoshi/in-mem)
- **Issues**: [GitHub Issues](https://github.com/anibjoshi/in-mem/issues)
