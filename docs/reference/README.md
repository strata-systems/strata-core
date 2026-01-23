# Strata Reference Documentation

**Strata** - a fast, durable, embedded database for AI agent workloads.

**Current Version**: 0.11.0 (M11 API Surface)

## Quick Links

- [Getting Started](getting-started.md) - Installation and quick start
- [Architecture](architecture.md) - How Strata works internally
- [Milestones](../milestones/MILESTONES.md) - Project roadmap

## External APIs (User-Facing)

These APIs are for application developers using Strata:

- **[Facade API](facade-api.md)** - Redis-like convenience API
  - Simplified return types (no version metadata by default)
  - Implicit default run targeting
  - Auto-commit semantics
  - Familiar operations: `get`, `set`, `incr`, `mget`, `mset`, etc.

- **[Substrate API](substrate-api.md)** - Full-power API
  - Explicit run targeting on every operation
  - Full versioning with `Versioned<T>` returns
  - History access and point-in-time reads
  - CAS operations for optimistic concurrency
  - Transaction control

## Internal APIs (Engine)

- [Engine API Reference](api-reference.md) - Internal primitives and storage layer
  - KVStore, EventLog, StateCell, TraceStore, RunIndex, JsonStore primitives
  - WAL types and snapshot/recovery internals
  - Used by Substrate implementation, not directly by applications

## Features

- **Six Primitives**: KVStore, EventLog, StateCell, TraceStore, RunIndex, JsonStore
- **Hybrid Search**: BM25 + semantic search with RRF fusion
- **Three Durability Modes**: InMemory (<3µs), Buffered (<30µs), Strict (~2ms)
- **OCC Transactions**: Optimistic concurrency with snapshot isolation
- **Run-Scoped Operations**: Every operation tagged with RunId for replay
- **Periodic Snapshots**: Bounded recovery time with automatic WAL truncation
- **Crash Recovery**: Deterministic, idempotent, prefix-consistent recovery
- **Deterministic Replay**: Side-effect free reconstruction of agent run state

## Current Status

| Milestone | Status |
|-----------|--------|
| M1 Foundation | ✅ |
| M2 Transactions | ✅ |
| M3 Primitives | ✅ |
| M4 Performance | ✅ |
| M5 JSON | ✅ |
| M6 Retrieval | ✅ |
| M7 Durability | ✅ |
| M8 Vector | ✅ |
| M9 Runs | ✅ |
| M10 Schemas | ✅ |
| M11 API Surface | ✅ |

## Quick Start

### Facade API (Recommended for Most Users)

```rust
use strata_api::facade::{FacadeImpl, KVFacade, KVFacadeBatch};
use strata_api::substrate::SubstrateImpl;
use strata_core::Value;
use strata_engine::Database;
use std::sync::Arc;

// Create facade
let db = Arc::new(Database::open("./my-agent-db")?);
let substrate = Arc::new(SubstrateImpl::new(db));
let facade = FacadeImpl::new(substrate);

// Simple get/set (targets default run, auto-commits)
facade.set("user:1", Value::String("Alice".into()))?;
let name = facade.get("user:1")?;

// Atomic increment
let count = facade.incr("page_views")?;

// Batch operations
facade.mset(&[("a", Value::Int(1)), ("b", Value::Int(2))])?;
```

### Substrate API (Full Power)

```rust
use strata_api::substrate::{SubstrateImpl, ApiRunId, KVStore};
use strata_core::Value;

// Create a specific run
let run = ApiRunId::new();

// Explicit run targeting, versioned results
let version = substrate.kv_put(&run, "key", Value::String("value".into()))?;
let versioned = substrate.kv_get(&run, "key")?;

// Access history
let history = substrate.kv_history(&run, "key", Some(10), None)?;
```

## Performance

| Mode | Latency | Throughput |
|------|---------|------------|
| InMemory | <3µs | 250K+ ops/sec |
| Buffered | <30µs | 50K+ ops/sec |
| Strict | ~2ms | ~500 ops/sec |

---

**Version**: 0.11.0
**Last Updated**: 2026-01-23
