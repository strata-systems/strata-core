# Strata Engine Crate Analysis

## Overview

The `strata-engine` crate is the **top orchestration layer** in the Strata architecture. It coordinates all lower layers and provides the main `Database` API for external consumers.

```
strata-engine/
├── Database (main entry point)
│   ├── DatabaseBuilder (fluent configuration)
│   ├── PersistenceMode (Ephemeral vs Disk)
│   └── RetryConfig (automatic retry)
│
├── Transaction System
│   ├── TransactionCoordinator (lifecycle, metrics)
│   ├── TransactionPool (thread-local pooling)
│   └── Transaction (wrapper implementing TransactionOps)
│
├── Durability System
│   ├── Durability trait (abstraction)
│   ├── InMemoryDurability (no-op, fastest)
│   ├── BufferedDurability (background flush thread)
│   └── StrictDurability (fsync every commit)
│
├── Run Lifecycle
│   ├── RunIndex (metadata + event offsets)
│   ├── ReadOnlyView (P1-P6 invariants)
│   ├── RunDiff (comparison)
│   └── diff_views()
│
├── Recovery
│   ├── RecoveryParticipant (registry entry)
│   ├── RecoveryFn (function signature)
│   └── recover_all_participants()
│
└── Instrumentation
    └── PerfTrace (feature-gated timing)
```

---

## Dependencies

From `Cargo.toml`:
```toml
strata-core = { path = "../core" }
strata-storage = { path = "../storage" }
strata-durability = { path = "../durability" }
strata-concurrency = { path = "../concurrency" }
dashmap         # Per-run commit locks
parking_lot     # Fast Mutex
tracing         # Logging
uuid            # Temp path generation
once_cell       # Lazy static registry
thiserror       # Error types
sha2            # Event hash computation (cryptographic linking)
```

---

## Database Struct

### Main Entry Point (`database.rs`)

```rust
pub struct Database {
    /// Data directory path (empty for ephemeral databases)
    data_dir: PathBuf,

    /// Sharded storage with O(1) lazy snapshots
    storage: Arc<ShardedStore>,

    /// Write-ahead log (None for ephemeral)
    wal: Option<Arc<ParkingMutex<WAL>>>,

    /// Persistence mode (Ephemeral vs Disk)
    persistence_mode: PersistenceMode,

    /// Transaction coordinator (version allocation, metrics)
    coordinator: TransactionCoordinator,

    /// Per-run commit locks (allows parallel commits for different runs)
    commit_locks: DashMap<RunId, ParkingMutex<()>>,

    /// Current durability mode
    durability_mode: DurabilityMode,

    /// Flag for shutdown rejection
    accepting_transactions: AtomicBool,

    /// Type-erased extension storage for primitive state
    extensions: DashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}
```

### Complete Database API

| Category | Method | Description |
|----------|--------|-------------|
| **Construction** | `builder()` | Get DatabaseBuilder |
| | `open(path)` | Open/create at path (Batched durability) |
| | `open_with_mode(path, mode)` | Open with specific DurabilityMode |
| | `ephemeral()` | Create in-memory-only database |
| **Accessors** | `storage()` | Get Arc<ShardedStore> |
| | `data_dir()` | Get data directory path |
| | `wal()` | Get WAL (Option<Arc<Mutex<WAL>>>) |
| | `is_ephemeral()` | Check if database is memory-only |
| | `persistence_mode()` | Get PersistenceMode |
| | `durability_mode()` | Get DurabilityMode |
| | `coordinator()` | Get TransactionCoordinator reference |
| | `metrics()` | Get TransactionMetrics snapshot |
| **Transactions** | `transaction(run_id, f)` | Execute closure in transaction |
| | `transaction_with_version(run_id, f)` | Execute and return commit version |
| | `transaction_with_retry(run_id, config, f)` | Auto-retry on conflict |
| | `transaction_with_timeout(run_id, timeout, f)` | Execute with timeout |
| | `transaction_with_durability(run_id, mode, f)` | Override durability mode |
| | `begin_transaction(run_id)` | Start manual transaction |
| | `commit_transaction(txn)` | Commit transaction |
| | `end_transaction(txn)` | Return transaction to pool |
| **Direct Ops** | `put(run_id, key, value)` | Direct single-key put |
| | `get(key)` | Direct single-key get |
| | `delete(run_id, key)` | Direct single-key delete |
| | `cas(run_id, key, expected, value)` | Direct compare-and-swap |
| **Replay** | `replay_run(run_id)` | Get ReadOnlyView of run |
| | `diff_runs(run_a, run_b)` | Compare two runs |
| **Lifecycle** | `flush()` | Force WAL sync |
| | `shutdown()` | Stop accepting transactions |
| | `close()` | Close database |
| | `is_open()` | Check if accepting transactions |
| **Extensions** | `extension<T>()` | Get/create typed extension |

### Opening Databases

| Method | Files | WAL | Recovery | Use Case |
|--------|-------|-----|----------|----------|
| `ephemeral()` | None | None | No | Unit tests, caching |
| `open(path)` | Yes | Yes (Batched) | Yes | Default production |
| `builder().no_durability().open()` | Yes | Yes (no sync) | Yes | Integration tests |
| `builder().strict().open()` | Yes | Yes (fsync) | Yes | Audit logs |

**Recovery Flow:**
1. Create/open data directory
2. Open WAL file at `<path>/wal/current.wal`
3. Use RecoveryCoordinator to replay WAL
4. Re-open WAL for appending
5. Run primitive recovery participants

---

## Transaction API

### 1. Closure API (Recommended)

```rust
// Auto commit on success, abort on error
let result = db.transaction(run_id, |txn| {
    let val = txn.get(&key)?;
    txn.put(key, new_value)?;
    Ok(val)
})?;

// With commit version returned
let (result, commit_version) = db.transaction_with_version(run_id, |txn| {
    txn.put(key, value)?;
    Ok("success")
})?;
```

### 2. Manual API

```rust
let mut txn = db.begin_transaction(run_id);
txn.put(key, value)?;
db.commit_transaction(&mut txn)?;
db.end_transaction(txn); // Return to pool
```

### 3. Retry Support

```rust
let config = RetryConfig {
    max_retries: 5,
    base_delay_ms: 10,  // Exponential backoff
    max_delay_ms: 200,
};

db.transaction_with_retry(run_id, config, |txn| {
    // Automatically retried on conflict
    let val = txn.get(&key)?;
    txn.put(key, Value::Int(val.value + 1))?;
    Ok(())
})?;
```

### 4. Timeout Support

```rust
db.transaction_with_timeout(run_id, Duration::from_secs(5), |txn| {
    txn.put(key, value)?;
    Ok(())
})?;
```

### 5. Durability Override

```rust
// Force strict durability for critical writes
db.transaction_with_durability(run_id, DurabilityMode::Strict, |txn| {
    txn.put(metadata_key, value)?;
    Ok(())
})?;
```

---

## Direct Operations (Outside Transactions)

The Database also provides direct operations that bypass the transaction system:

```rust
// Direct KV operations (immediate commit)
db.put(&run_id, "key", Value::String("value".to_string()))?;
let value = db.get(&run_id, "key")?;
db.delete(&run_id, "key")?;

// Compare-and-swap
let result = db.cas(&run_id, "key", expected_version, new_value)?;
```

> **Note:** These operations are single-key atomic but not part of multi-key transactions. Use for simple operations where transaction overhead is unnecessary.

---

## Namespace Isolation

Each run's operations are isolated using namespaces:

```rust
// Internally, all operations use Namespace::for_run(run_id)
// This creates: "default/default/default/{run_id}"
let namespace = Namespace::for_run(run_id);
let key = Key::for_kv(&namespace, "user_key");
```

This ensures:
- Different runs cannot see each other's data
- Keys are globally unique across all runs
- Run deletion can cleanly remove all run-scoped data

---

## Commit Protocol

Per spec Section 3.3 and 4:

```rust
pub fn commit_transaction(&self, txn: &mut TransactionContext) -> Result<u64> {
    // 1. Acquire per-run commit lock
    let run_lock = self.commit_locks.entry(txn.run_id)
        .or_insert_with(|| ParkingMutex::new(()));
    let _commit_guard = run_lock.lock();

    // 2. Validate (under lock)
    txn.mark_validating()?;
    let validation = validate_transaction(txn, self.storage.as_ref());
    if !validation.is_valid() {
        txn.mark_aborted(...);
        return Err(TransactionConflict);
    }

    // 3. Allocate commit version
    let commit_version = self.coordinator.allocate_commit_version();

    // 4. Write to WAL (if required)
    if self.durability_mode.requires_wal() && self.wal.is_some() {
        // BeginTxn, Writes, Deletes, CAS, CommitTxn
    }

    // 5. Apply to storage atomically
    self.storage.apply_batch(&writes, &deletes, commit_version)?;

    // 6. Mark committed
    txn.mark_committed()?;
    self.coordinator.record_commit();

    Ok(commit_version)
}
```

**Key Improvement:** Per-run commit locks (vs concurrency crate's single lock).

---

## Transaction Struct (`transaction/context.rs`)

The `Transaction` struct wraps `TransactionContext` with event buffering support:

```rust
pub struct Transaction<'a> {
    ctx: &'a mut TransactionContext,
    namespace: Namespace,
    pending_events: Vec<Event>,
    base_sequence: u64,
    last_hash: [u8; 32],
}
```

**Key Methods:**
- `new(ctx, namespace)` - Create transaction wrapper
- `with_base_sequence(ctx, namespace, base_sequence, last_hash)` - Create with explicit sequence
- `run_id()` - Get run ID
- `pending_events()` - Get buffered events

**Note:** This is different from `TransactionContext` from strata-concurrency. It adds event chaining support with SHA-256 hash linking.

---

## CommitData Struct (`durability/traits.rs`)

Captures transaction statistics for durability layer:

```rust
pub struct CommitData {
    pub txn_id: u64,
    pub run_id: RunId,
    pub commit_version: u64,
    pub put_count: usize,
    pub delete_count: usize,
    pub cas_count: usize,
}
```

**Factory Methods:**
- `from_transaction(txn, commit_version)` - Extract from TransactionContext

**Query Methods:**
- `total_operations()` - Sum of all operations
- `is_read_only()` - Check if no writes

---

## TransactionMetrics Struct (`coordinator.rs`)

Snapshot of transaction system statistics:

```rust
pub struct TransactionMetrics {
    pub active_count: u64,
    pub total_started: u64,
    pub total_committed: u64,
    pub total_aborted: u64,
    pub commit_rate: f64,
}
```

**Query Methods:**
- `total_completed()` - committed + aborted
- `abort_rate()` - aborted / started

---

## TransactionCoordinator

Wraps `TransactionManager` from concurrency crate with:
- Active transaction tracking
- Transaction metrics (started/committed/aborted)
- Commit rate calculation
- `wait_for_idle()` for shutdown

**Observation Methods:**
- `record_start()` - Record transaction start
- `record_commit()` - Record successful commit
- `record_abort()` - Record abort/conflict
- `current_version()` - Get current global version
- `metrics()` - Get transaction metrics snapshot (returns `TransactionMetrics`)

```rust
pub struct TransactionCoordinator {
    manager: TransactionManager,      // ID/version allocation
    active_count: AtomicU64,          // Metrics (Relaxed ordering)
    total_started: AtomicU64,
    total_committed: AtomicU64,
    total_aborted: AtomicU64,
}
```

**Memory Ordering:** Metrics use Relaxed ordering because:
1. Purely observational (monitoring/debugging)
2. Don't synchronize other memory operations
3. Approximate counts are acceptable

---

## Transaction Pooling

Thread-local pool avoids allocation after warmup:

```rust
pub const MAX_POOL_SIZE: usize = 8;

thread_local! {
    static TXN_POOL: RefCell<Vec<TransactionContext>> =
        RefCell::new(Vec::with_capacity(MAX_POOL_SIZE));
}
```

**Pool Operations:**
- `acquire()` - Get from pool or create new
- `release()` - Return to pool (reset + clear but preserve capacity)
- `warmup(count)` - Pre-allocate contexts for reduced latency
- `pool_size()` - Get current pool size
- `clear()` - Clear pool (for testing)
- `total_capacity()` - Get total capacity across all pooled contexts

---

## DatabaseBuilder (`database.rs`)

Fluent API for database configuration:

```rust
pub struct DatabaseBuilder {
    path: Option<PathBuf>,
    durability: DurabilityMode,
    persistence: PersistenceMode,
}
```

**Builder Methods:**
| Method | Description |
|--------|-------------|
| `new()` | Create new builder |
| `path(p)` | Set database path |
| `durability(mode)` | Set durability mode |
| `no_durability()` | Set DurabilityMode::None |
| `buffered()` | Set DurabilityMode::Batched (default) |
| `buffered_with(interval_ms, max_pending)` | Set Batched with custom settings |
| `strict()` | Set DurabilityMode::Strict |
| `in_memory()` | **DEPRECATED** - use ephemeral() instead |
| `get_path()` | Get configured path |
| `get_durability()` | Get configured durability |
| `open()` | Build and open database |
| `open_temp()` | Build and open in temp directory |

---

## Durability System

### Durability Trait (`durability/traits.rs`)

```rust
pub trait Durability: Send + Sync {
    fn persist(&self, txn: &TransactionContext, commit_version: u64) -> Result<()>;
    fn shutdown(&self) -> Result<()>;
    fn is_persistent(&self) -> bool;
    fn mode_name(&self) -> &'static str;
    fn requires_wal(&self) -> bool { self.is_persistent() }
}
```

### Implementations

| Mode | Latency | Behavior |
|------|---------|----------|
| `InMemoryDurability` | <3µs | No-op, data lost on crash |
| `BufferedDurability` | <30µs | WAL append, background fsync |
| `StrictDurability` | ~2ms | WAL append + immediate fsync |

> **Note:** Some documentation may reference "Async" durability mode - this does not exist. The actual modes in `DurabilityMode` are `None`, `Strict`, and `Batched`.

### BufferedDurability

Has background flush thread:
- Wakes on timer (flush_interval) or batch size (max_pending_writes)
- Must call `shutdown()` for guaranteed final flush
- `threaded()` factory auto-starts thread

```rust
// Recommended usage
let durability = BufferedDurability::threaded(wal, 100, 1000);

// Manual (must call start_flush_thread)
let durability = Arc::new(BufferedDurability::new(wal, 100, 1000));
durability.start_flush_thread();
```

---

## PersistenceMode

Orthogonal to DurabilityMode:

| PersistenceMode | DurabilityMode | Behavior |
|-----------------|----------------|----------|
| Ephemeral | (ignored) | No files, data lost on drop |
| Disk | None | Files created, no fsync |
| Disk | Batched | Files created, periodic fsync |
| Disk | Strict | Files created, immediate fsync |

---

## TransactionOps Trait

Unified primitive operations for cross-primitive atomic transactions:

```rust
pub trait TransactionOps {
    // KV Operations (Phase 2 - implemented)
    fn kv_get(&self, key: &str) -> Result<Option<Versioned<Value>>, StrataError>;
    fn kv_put(&mut self, key: &str, value: Value) -> Result<Version, StrataError>;
    fn kv_delete(&mut self, key: &str) -> Result<bool, StrataError>;
    fn kv_exists(&self, key: &str) -> Result<bool, StrataError>;
    fn kv_list(&self, prefix: Option<&str>) -> Result<Vec<String>, StrataError>;

    // Event Operations (Phase 2 - implemented)
    fn event_append(&mut self, event_type: &str, payload: Value) -> Result<Version, StrataError>;
    fn event_read(&self, sequence: u64) -> Result<Option<Versioned<Event>>, StrataError>;
    fn event_range(&self, start: u64, end: u64) -> Result<Vec<Versioned<Event>>, StrataError>;
    fn event_len(&self) -> Result<u64, StrataError>;

    // Note: Events use SHA-256 hash chaining for integrity verification
    // Each event contains prev_hash (hash of previous event) and hash (hash of current event)
    // This creates a tamper-evident chain similar to blockchain structures

    // State Operations (Phase 3 - implemented)
    fn state_read(&self, name: &str) -> Result<Option<Versioned<State>>, StrataError>;
    fn state_init(&mut self, name: &str, value: Value) -> Result<Version, StrataError>;
    fn state_cas(&mut self, name: &str, expected: u64, value: Value) -> Result<Version, StrataError>;
    fn state_delete(&mut self, name: &str) -> Result<bool, StrataError>;

    // Json Operations (Phase 4 - stubs)
    // Vector Operations (Phase 4 - stubs)
    // Run Operations (Phase 5 - limited)
}
```

**Design Principles:**
- Reads are `&self`
- Writes are `&mut self`
- All operations return `Result<T, StrataError>`
- All reads return `Versioned<T>`
- All writes return `Version`

---

## Run Lifecycle

### RunIndex

Tracks runs and their event offsets for O(run size) replay:

```rust
pub struct RunIndex {
    runs: HashMap<RunId, RunMetadata>,
    run_events: HashMap<RunId, RunEventOffsets>,
}
```

**Operations:**
- `insert()`, `exists()`, `get()`, `get_mut()`
- `record_event()` - Track event offset
- `find_active()` - Find potential orphans
- `mark_orphaned()` - Mark runs as orphaned
- `count_by_status()` - Statistics

---

## Database Lifecycle

**Shutdown Methods:**
```rust
// Graceful shutdown - stops accepting new transactions, waits for active ones
db.shutdown()?;

// Check if accepting transactions
if db.is_open() {
    // Can start new transactions
}

// Close the database (called automatically on drop)
db.close()?;
```

**Replay Operations:**
```rust
// Replay a run to a ReadOnlyView (for debugging/inspection)
let view = db.replay_run(&run_id)?;

// Compare two runs
let diff = db.diff_runs(&run_a, &run_b)?;
```

### ReadOnlyView

Derived view from replay (NOT a source of truth):

```rust
pub struct ReadOnlyView {
    pub run_id: RunId,
    kv_state: HashMap<Key, Value>,
    events: Vec<(String, Value)>,
    operation_count: u64,
}
```

**Replay Invariants (P1-P6):**

| # | Invariant | Meaning |
|---|-----------|---------|
| P1 | Pure function | Over (Snapshot, WAL, EventLog) |
| P2 | Side-effect free | Does not mutate canonical store |
| P3 | Derived view | Not a new source of truth |
| P4 | Does not persist | Unless explicitly materialized |
| P5 | Deterministic | Same inputs = Same view |
| P6 | Idempotent | Running twice produces identical view |

### RunDiff

Key-level comparison between views:

```rust
pub struct RunDiff {
    pub run_a: RunId,
    pub run_b: RunId,
    pub added: Vec<DiffEntry>,
    pub removed: Vec<DiffEntry>,
    pub modified: Vec<DiffEntry>,
}

pub fn diff_views(view_a: &ReadOnlyView, view_b: &ReadOnlyView) -> RunDiff
```

---

## Recovery Participant Registry

Global registry for primitive recovery:

```rust
pub type RecoveryFn = fn(&Database) -> Result<()>;

pub struct RecoveryParticipant {
    pub name: &'static str,
    pub recover: RecoveryFn,
}

// Registration (at startup)
register_recovery_participant(RecoveryParticipant::new("vector", recover_vector_state));

// Called by Database::open_with_mode after KV recovery
recover_all_participants(&db)?;
```

**Behavior:**
- Called in registration order
- First error stops execution
- Duplicate registration prevented

---

## Extension System

Type-erased storage for primitive state:

```rust
// Get or create typed extension
let state = db.extension::<VectorBackendState>();

// Extension is shared across all uses of that type for this Database
// Created at most once (atomic via DashMap entry API)
```

---

## Performance Instrumentation

Feature-gated timing (enable with `--features perf-trace`):

```rust
#[cfg(feature = "perf-trace")]
pub struct PerfTrace {
    pub snapshot_acquire_ns: u64,
    pub read_set_validate_ns: u64,
    pub write_set_apply_ns: u64,
    pub wal_append_ns: u64,
    pub fsync_ns: u64,
    pub commit_total_ns: u64,
    pub keys_read: usize,
    pub keys_written: usize,
}

// Usage
let mut trace = PerfTrace::new();
let result = perf_time!(trace, snapshot_acquire_ns, { engine.snapshot() });
println!("{}", trace.summary());
```

---

## How Engine Uses Lower Crates

### From `strata_core`:
- `Key`, `RunId`, `Value`, `Version`, `Versioned`, `VersionedValue`
- `RunMetadata`, `RunStatus`, `RunEventOffsets`
- `Event`, `State`, `JsonValue`, `JsonPath`, `JsonDocId`
- `StrataError`, `Error`, `Result`
- `Storage` trait

### From `strata_storage`:
- `ShardedStore` (primary storage)

### From `strata_durability`:
- `WAL`, `DurabilityMode` (Legacy WAL system)

### From `strata_concurrency`:
- `TransactionContext`, `TransactionManager`
- `TransactionWALWriter`
- `validate_transaction`
- `RecoveryCoordinator`, `RecoveryResult`, `RecoveryStats`

---

## Summary

The engine crate provides:

1. **Database API:**
   - Builder pattern for configuration
   - Ephemeral and disk-backed modes
   - Multiple durability levels

2. **Transaction System:**
   - Closure and manual APIs
   - Automatic retry and timeout
   - Thread-local pooling
   - Per-run commit locks

3. **Durability Abstraction:**
   - Three implementations with different tradeoffs
   - Per-operation override capability

4. **Run Lifecycle:**
   - Metadata and event tracking
   - Deterministic replay with invariants
   - Diff capabilities

5. **Extensibility:**
   - Recovery participant registry
   - Type-erased extension storage
   - Performance instrumentation
