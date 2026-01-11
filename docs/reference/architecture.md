# Architecture Overview

Learn how **in-mem** works internally and why it's designed the way it is.

## Design Philosophy

**in-mem** is built around three core principles:

1. **Run-First Design**: Every operation is scoped to a run, enabling deterministic replay and debugging
2. **Accept MVP Limitations, Design for Evolution**: Simple implementations now, trait abstractions for future optimization
3. **Speed Over Perfect Durability**: Batched fsync by default (agents prefer fast writes over perfect durability)

## System Architecture

### Layered Design

```
┌─────────────────────────────────────────────┐
│         API Layer (embedded/rpc/mcp)        │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────▼───────────────────────────┐
│  Primitives (KV, Events, StateMachine,      │
│              Trace, RunIndex, Vector)       │  ← Stateless facades
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────▼───────────────────────────┐
│  Engine (Database, Run Lifecycle, Coord)    │  ← Orchestration
└──┬──────────────────────────────────────┬───┘
   │                                      │
┌──▼──────────────┐            ┌──────────▼────┐
│  Concurrency    │            │  Durability   │
│  (OCC/Txn)      │            │  (WAL/Snap)   │
└─────────┬───────┘            └─────────┬─────┘
          │                              │
┌─────────▼──────────────────────────────▼─────┐
│       Storage (UnifiedStore + Indices)       │
└──────────────────┬───────────────────────────┘
                   │
┌──────────────────▼───────────────────────────┐
│  Core Types (RunId, Key, Value, TypeTag)     │
└──────────────────────────────────────────────┘
```

### Layer Responsibilities

**Core Types**: Foundation data structures (RunId, Key, Value, Namespace)

**Storage**: Unified BTreeMap with secondary indices (run_id, type_tag, TTL)

**Durability**: Write-ahead log (WAL) with configurable fsync modes

**Concurrency**: Optimistic Concurrency Control (OCC) - coming in M2

**Engine**: Orchestrates run lifecycle, transactions, recovery

**Primitives**: High-level APIs (KV, Event Log, State Machine, etc.)

**API**: Embedded library interface (network layer in M7)

## Data Model

### Keys

Every key in **in-mem** has three components:

```rust
pub struct Key {
    namespace: Namespace,  // tenant/app/agent/run hierarchy
    type_tag: TypeTag,     // KV, Event, StateMachine, etc.
    user_key: Vec<u8>,     // your application key
}
```

**Key Ordering**: Keys are ordered by namespace → type_tag → user_key

This enables:
- Efficient prefix scans (list all keys for a run)
- Cross-primitive queries (get all events and KV for a run)
- Namespace isolation (tenant separation)

### Values

Values are versioned with metadata:

```rust
pub struct VersionedValue {
    value: Value,              // The actual data
    version: u64,              // Monotonically increasing
    timestamp: Timestamp,      // When written
    ttl: Option<Duration>,     // Expiration (optional)
}
```

**Version Numbers**: Global, monotonically increasing counter. Enables:
- Snapshot isolation (read as-of version V)
- Conflict detection (version changed during transaction)
- Replay (apply operations in version order)

## Storage Layer

### Unified Store

**M1 Implementation**: Single `BTreeMap<Key, VersionedValue>` with RwLock

```rust
pub struct UnifiedStore {
    data: Arc<RwLock<BTreeMap<Key, VersionedValue>>>,
    global_version: AtomicU64,

    // Secondary indices
    run_index: HashMap<RunId, HashSet<Key>>,
    type_index: HashMap<TypeTag, HashSet<Key>>,
    ttl_index: BTreeMap<Instant, HashSet<Key>>,
}
```

**Why Unified?**
- Single BTreeMap for all primitives (not separate stores)
- Enables atomic multi-primitive transactions
- Simplifies recovery (one data structure to replay into)

**Trade-offs**:
- ✅ Simple, correct, easy to reason about
- ⚠️ Writers block readers (RwLock contention)
- ⚠️ Global version counter will contend under load

**Future**: Storage is behind a trait, can swap to sharded/lock-free implementation without API changes.

### Secondary Indices

**Run Index**: `RunId → Set<Key>`
- Find all keys written by a run
- Enables O(run size) replay (not O(WAL size))

**Type Index**: `TypeTag → Set<Key>`
- Find all Event Log entries, all State Machine records, etc.
- Efficient cross-primitive queries

**TTL Index**: `Expiration Time → Set<Key>`
- Efficient expiration (no need to scan all keys)
- Background cleanup thread

### TTL Cleanup

Background thread checks for expired keys every second:

```rust
impl TTLCleaner {
    fn cleanup_expired(&self) {
        let expired = self.find_expired_keys(Instant::now());
        for key in expired {
            self.store.delete(&key); // Transactional delete
        }
    }
}
```

**Key Design**: Cleanup uses normal delete operations (transactional, logged to WAL). No special "expire" operation.

## Durability

### Write-Ahead Log (WAL)

Every write is logged before applying to storage:

```rust
pub enum WALEntry {
    BeginTxn { txn_id: u64, run_id: RunId, timestamp: Timestamp },
    Write { run_id: RunId, key: Key, value: Value, version: u64 },
    Delete { run_id: RunId, key: Key, version: u64 },
    CommitTxn { txn_id: u64, run_id: RunId },
    AbortTxn { txn_id: u64, run_id: RunId },
}
```

**Entry Format**: `[Length][Type][Payload][CRC32]`

**CRC Protection**: Every entry has CRC32 checksum. Corrupted entries stop recovery (fail-safe).

### Durability Modes

**Strict**: fsync after every commit
```
write → log to WAL → fsync → apply to storage → return
```
- Safest (no data loss except on disk failure)
- Slowest (~10ms per write on typical SSD)

**Batched** (default): fsync every 100ms or 1000 commits
```
write → log to WAL → apply to storage → return
                 ↓
          background thread fsyncs every 100ms
```
- Balanced (may lose <100ms of commits on crash)
- Fast (<1ms per write)

**Async**: background thread fsyncs every 1 second
```
write → log to WAL → apply to storage → return
                 ↓
          background thread fsyncs every 1s
```
- Fastest (<0.1ms per write)
- May lose up to 1 second of commits on crash

### Recovery

On database open:

1. Scan WAL from beginning
2. Validate each entry (CRC check)
3. Replay committed transactions
4. Discard incomplete transactions (no CommitTxn = rollback)
5. Rebuild secondary indices
6. Resume normal operation

**Conservative Recovery**: Stop at first corrupted entry (don't skip). Ensures no silent data loss.

## Concurrency

### M1: RwLock

Simple reader-writer lock:
- Multiple readers OR one writer
- Writers block readers
- Readers block writers

**Performance**: Acceptable for M1 (single-agent workloads). Will contend under high load.

### M2: Optimistic Concurrency Control (OCC)

Coming in M2:

1. **Begin Transaction**: Take snapshot of current version
2. **Execute**: Read from snapshot, buffer writes
3. **Validate**: Check no conflicting writes occurred
4. **Commit**: Apply writes if validation passes, retry if conflicts

**Benefits**:
- Readers never block writers
- Writers never block readers
- Only conflicts retry

## Run Lifecycle

### Run States

```
Created → Running → Completed
           ↓
        Forked (future)
```

**Created**: Run registered but not started

**Running**: Actively executing operations

**Completed**: Finished (can be replayed)

**Forked**: Branched into child runs (M3 feature)

### Run Metadata

```rust
pub struct RunMetadata {
    run_id: RunId,
    parent_run_id: Option<RunId>,
    status: RunStatus,
    created_at: Timestamp,
    completed_at: Option<Timestamp>,
    first_version: u64,      // For replay
    last_version: u64,
    wal_start_offset: u64,   // For replay
    wal_end_offset: u64,
}
```

**Why Track Offsets?**
- Replay only reads `[wal_start_offset..wal_end_offset]` (not entire WAL)
- O(run size) replay instead of O(WAL size)
- Enables efficient diffing of two runs

## Performance Characteristics

### M1 Baseline (Single-Threaded)

| Operation | Latency (p99) | Throughput |
|-----------|---------------|------------|
| put (batched) | <1ms | ~50K ops/sec |
| get | <0.1ms | ~200K ops/sec |
| delete (batched) | <1ms | ~50K ops/sec |
| list (100 keys) | <1ms | ~10K scans/sec |
| Recovery (10K txns) | 486ms | 20,564 txns/sec |

**Bottlenecks** (known):
- RwLock: Writers block readers
- Global version counter: AtomicU64 contention
- Snapshot creation: Clones entire BTreeMap

### M2 Targets (with OCC)

- put: ~10K ops/sec (conflicts may cause retries)
- get: ~500K ops/sec (no blocking)
- Concurrent writes: 4-8 cores utilized

## Known Limitations (M1)

| Limitation | Impact | Mitigation |
|------------|--------|------------|
| In-memory only | Can't exceed RAM | M6 will add disk-based storage |
| RwLock | Writers block readers | M2 OCC for non-blocking reads |
| Global version counter | AtomicU64 contention | Can shard per namespace later |
| Snapshot cloning | Memory overhead | Lazy snapshots in M3 |
| No transactions | No atomic multi-key ops | M2 will add OCC transactions |

**Design for Evolution**: All limitations have clear migration paths enabled by trait abstractions.

## Security & Reliability

### Data Integrity

✅ **CRC32 on every WAL entry**: Detects corruption
✅ **Fail-safe recovery**: Stop at corruption (don't skip)
✅ **Transactional deletes**: TTL cleanup goes through normal paths

### Fault Tolerance

✅ **Crash recovery**: Automatic on database open
✅ **Configurable durability**: Choose your trade-off
✅ **Conservative recovery**: Discard incomplete transactions

### Not Yet Implemented

❌ **Authentication**: Not in embedded mode (M7 network layer)
❌ **Encryption at rest**: Planned for M8
❌ **Replication**: Planned for M9
❌ **Backup/restore**: Planned for M4

## Comparisons

### vs. SQLite

**in-mem**:
- ✅ Run-scoped operations (built-in)
- ✅ Multi-primitive (KV + Events + Traces)
- ✅ Optimized for agent workflows
- ❌ No SQL (simple API only)
- ❌ In-memory only (M1)

**SQLite**:
- ✅ SQL queries
- ✅ Disk-based storage
- ✅ Mature ecosystem
- ❌ No run concept (must implement yourself)
- ❌ Single primitive (relational only)

### vs. Redis

**in-mem**:
- ✅ Embedded (no network overhead)
- ✅ Run-scoped operations
- ✅ Deterministic replay
- ❌ No network mode yet (M7)
- ❌ Limited data structures (M1)

**Redis**:
- ✅ Rich data structures
- ✅ Network protocol
- ✅ Pub/sub, streams
- ❌ No run concept
- ❌ Not embedded

### vs. RocksDB

**in-mem**:
- ✅ Multi-primitive unified storage
- ✅ Run-scoped operations
- ✅ Simple API
- ❌ In-memory only (M1)
- ❌ No distributed mode

**RocksDB**:
- ✅ Disk-based (LSM tree)
- ✅ High write throughput
- ✅ Proven at scale
- ❌ No run concept
- ❌ KV only

## Future Roadmap

### M2: Transactions (Week 3)
- Optimistic Concurrency Control
- Snapshot isolation
- Multi-key transactions

### M3: Primitives (Week 4)
- Event Log with chaining
- State Machine with CAS
- Trace Store for reasoning
- Run Index for first-class runs

### M4: Production Durability (Week 5)
- Periodic snapshots
- WAL truncation
- Incremental snapshots

### M5: Replay & Polish (Week 6)
- Deterministic replay
- Run diffing
- Performance benchmarks

### M6+: Post-MVP
- Disk-based storage (LSM tree)
- Vector store (HNSW index)
- Network layer (RPC + MCP)
- Distributed mode
- Encryption at rest

## See Also

- [Getting Started Guide](getting-started.md)
- [API Reference](api-reference.md)
- [Performance Tuning](performance.md)
- [M1_ARCHITECTURE.md](../architecture/M1_ARCHITECTURE.md) - Detailed technical specification

---

**Current Version**: 0.1.0 (M1 Foundation)
**Architecture Status**: Production-ready for embedded use
