# Strata Concurrency Crate Analysis

## Overview

The `strata-concurrency` crate is **Layer 6** in the Strata architecture. It provides optimistic concurrency control (OCC) with the following major components:

```
strata-concurrency/
├── Core Transaction System
│   ├── TransactionContext (read/write set tracking)
│   ├── TransactionManager (atomic commit coordination)
│   ├── TransactionStatus (Active, Validating, Committed, Aborted)
│   └── CommitError (validation/WAL errors)
│
├── Validation System
│   ├── validate_transaction (full validation pipeline)
│   ├── validate_read_set (read-write conflict detection)
│   ├── validate_cas_set (CAS version validation)
│   ├── validate_json_set (document-level validation)
│   ├── validate_json_paths (path-level validation)
│   ├── ValidationResult (validation outcome with conflict list)
│   └── ConflictType (ReadWriteConflict, CASConflict, JsonDocConflict, JsonPath*)
│
├── JSON Conflict Detection (M5)
│   ├── check_read_write_conflicts (path overlap detection)
│   ├── check_write_write_conflicts (overlapping writes)
│   ├── check_version_conflicts (document version mismatch)
│   ├── find_first_read_write_conflict (early-exit optimization)
│   ├── find_first_write_write_conflict (early-exit optimization)
│   ├── find_first_version_conflict (early-exit optimization)
│   ├── ConflictResult enum
│   └── JsonConflictError (dedicated error type)
│
├── Snapshot Isolation
│   ├── ClonedSnapshotView (M2 implementation, deep clone)
│   └── SnapshotView trait (from strata_core)
│
├── Recovery System
│   ├── RecoveryCoordinator (WAL-based recovery)
│   ├── RecoveryResult (storage, txn_manager, stats)
│   └── RecoveryStats (replay statistics)
│
└── WAL Integration
    └── TransactionWALWriter (Legacy WAL system)
```

---

## Dependencies

From `Cargo.toml`:
```toml
strata-core = { path = "../core" }
strata-storage = { path = "../storage" }
strata-durability = { path = "../durability" }
parking_lot       # Fast Mutex
rmp-serde         # MessagePack for JSON serialization
tracing           # Logging
```

---

## Transaction System

### 1. TransactionContext (`transaction.rs`)

Core transaction data structure tracking all operations:

```rust
pub struct TransactionContext {
    // Identity
    pub txn_id: u64,
    pub run_id: RunId,

    // Snapshot isolation
    pub start_version: u64,
    snapshot: Option<Box<dyn SnapshotView>>,

    // Operation tracking
    pub read_set: HashMap<Key, u64>,      // Key → version when read
    pub write_set: HashMap<Key, Value>,   // Key → new value
    pub delete_set: HashSet<Key>,         // Keys to delete
    pub cas_set: Vec<CASOperation>,       // CAS operations

    // JSON Operations (M5 - lazy allocation)
    json_reads: Option<Vec<JsonPathRead>>,
    json_writes: Option<Vec<JsonPatchEntry>>,
    json_snapshot_versions: Option<HashMap<Key, u64>>,

    // State
    pub status: TransactionStatus,
    start_time: Instant,
}
```

**Key Operations:**
- `get(key)` - Read with read-your-writes semantics
- `put(key, value)` - Buffer write (blind write)
- `delete(key)` - Buffer delete
- `cas(key, expected_version, new_value)` - Buffer CAS operation
- `scan_prefix(prefix)` - Scan with read tracking
- `commit(store)` - Validate and transition state
- `pending_operations()` - Get summary of pending operations
- `clear_operations()` - Clear all buffered operations
- `has_pending_operations()` - Check if any operations are buffered
- `is_read_only()` - Check if transaction has no writes
- `read_count()`, `write_count()`, `delete_count()`, `cas_count()` - Operation counters
- `is_expired(timeout)` - Check if transaction exceeded timeout
- `elapsed()` - Get elapsed time since transaction start
- `capacity()` - Get current capacity of internal collections

**Read-Your-Writes Semantics:**
1. Check `write_set` (uncommitted writes) → NO read_set entry
2. Check `delete_set` (uncommitted deletes) → NO read_set entry
3. Read from `snapshot` → tracks in read_set

**Core Types Used:** `Key`, `RunId`, `Value`, `Version`, `VersionedValue`, `SnapshotView` trait

### PendingOperations (`transaction.rs`)

Summary of buffered operations in a transaction:

```rust
pub struct PendingOperations {
    pub puts: usize,     // Number of pending put operations
    pub deletes: usize,  // Number of pending delete operations
    pub cas: usize,      // Number of pending CAS operations
}
```

- **Methods**: `total()`, `is_empty()`
- **Usage**: Returned by `TransactionContext::pending_operations()`
- **Note**: No `reads` field - read operations are tracked separately in `read_set`

### 2. TransactionManager (`manager.rs`)

Coordinates atomic commit with WAL:

```rust
pub struct TransactionManager {
    version: AtomicU64,       // Global version counter
    next_txn_id: AtomicU64,   // Transaction ID allocator
    commit_lock: Mutex<()>,   // Prevents TOCTOU race
}
```

**Commit Sequence (per spec):**
```text
1. Acquire commit_lock
2. txn.commit(store) → Active → Validating → Committed/Aborted
3. Allocate commit_version (increment global version)
4. Write BeginTxn to WAL
5. Write all operations to WAL (with commit_version)
6. Write CommitTxn to WAL ← DURABILITY POINT
7. Apply writes to storage
8. Release commit_lock
9. Return commit_version
```

**Thread Safety:**
- Commit lock prevents TOCTOU race between validation and apply
- Version allocated atomically with `fetch_add`
- Transaction ID allocated atomically

**Additional Methods:**
- `with_txn_id(start_txn_id)` - Constructor with explicit starting txn_id (for recovery)
- `current_version()` - Get current global version
- `next_txn_id()` - Allocate next transaction ID
- `abort(txn)` - Abort a transaction
- `commit_or_rollback(txn, store, should_commit)` - Conditional commit/rollback

> **Note:** The single commit lock serializes ALL commits, even for different runs. This is a bottleneck under high throughput. The engine crate improves on this with per-run locks.

### 3. TransactionStatus (`transaction.rs`)

State machine for transaction lifecycle:

```rust
pub enum TransactionStatus {
    Active,                          // Can read/write
    Validating,                      // Being validated
    Committed,                       // Successfully committed
    Aborted { reason: String },      // Aborted with reason
}
```

**State Transitions:**
- `Active` → `Validating` (begin commit)
- `Validating` → `Committed` (validation passed)
- `Validating` → `Aborted` (conflict detected)
- `Active` → `Aborted` (user abort)

### 4. CASOperation (`transaction.rs`)

Compare-and-swap operation:

```rust
pub struct CASOperation {
    pub key: Key,
    pub expected_version: u64,  // 0 = key must not exist
    pub new_value: Value,
}
```

**CAS Semantics:**
- `expected_version = 0` → key must not exist
- `expected_version = N` → key must be at version N
- CAS does NOT add to read_set (validated separately)
- Multiple CAS operations on different keys allowed

---

## Validation System

### 5. Conflict Detection (`validation.rs`)

Per spec Section 3: First-committer-wins based on READ-SET, not write-set.

```rust
pub enum ConflictType {
    ReadWriteConflict { key, read_version, current_version },
    CASConflict { key, expected_version, current_version },
    JsonDocConflict { key, snapshot_version, current_version },
    JsonPathReadWriteConflict { key, read_path, write_path },
    JsonPathWriteWriteConflict { key, path1, path2 },
}
```

### ValidationResult (`validation.rs`)

The result of transaction validation:

```rust
pub struct ValidationResult {
    pub conflicts: Vec<ConflictType>,  // PUBLIC field - access directly
}
```

**Methods:**
- `ok()` - Create successful result with no conflicts
- `conflict(conflict_type)` - Create result with single conflict
- `is_valid()` - Returns true if no conflicts
- `merge(other)` - Combine two results (union of conflicts)
- `conflict_count()` - Number of conflicts detected

**Note:** There is NO `conflicts()` method. The `conflicts` field is public - access it directly as `result.conflicts`.

**Validation Pipeline:**
```rust
pub fn validate_transaction<S: Storage>(txn: &TransactionContext, store: &S) -> ValidationResult {
    // Per spec Section 3.2 Scenario 3: Read-only transactions ALWAYS commit
    if txn.is_read_only() && txn.json_writes().is_empty() {
        return ValidationResult::ok();
    }

    let mut result = ValidationResult::ok();
    result.merge(validate_read_set(&txn.read_set, store));      // Condition 1
    result.merge(validate_write_set(...));                       // No-op (blind writes OK)
    result.merge(validate_cas_set(&txn.cas_set, store));        // Condition 3
    result.merge(validate_json_set(...));                        // JSON document versions
    result.merge(validate_json_paths(...));                      // JSON path conflicts
    result
}
```

**Key Rules from Spec:**
- **Condition 1 (Read-Write Conflict):** Read key K at version V, current version V' ≠ V
- **Condition 3 (CAS Conflict):** CAS expected_version ≠ current version
- **Blind writes do NOT conflict** (write without read)
- **Write skew is ALLOWED** (do not try to prevent it)

### 6. JSON Conflict Detection (`conflict.rs`)

Region-based conflict detection for JSON operations:

```rust
pub enum ConflictResult {
    NoConflict,
    ReadWriteConflict { key, read_path, write_path },
    WriteWriteConflict { key, path1, path2 },
    VersionMismatch { key, expected, found },
}

/// Dedicated error type for JSON conflicts
pub enum JsonConflictError {
    ReadWriteConflict { key: Key, read_path: JsonPath, write_path: JsonPath },
    WriteWriteConflict { key: Key, path1: JsonPath, path2: JsonPath },
    VersionMismatch { key: Key, expected: u64, found: u64 },
}
```

**Path Overlap Detection:**
- Ancestor relationship: `foo` is ancestor of `foo.bar`
- Descendant relationship: `foo.bar.baz` is descendant of `foo.bar`
- Equal paths: `foo.bar` equals `foo.bar`

**Functions:**
- `check_read_write_conflicts()` - Path overlap between reads and writes
- `check_write_write_conflicts()` - Overlapping writes (semantic error)
- `check_version_conflicts()` - Document version changed
- `check_all_conflicts()` - Full validation pipeline

**Fast-Failure Optimization Functions:**
- `find_first_read_write_conflict()` - Early-exit on first read-write conflict
- `find_first_write_write_conflict()` - Early-exit on first write-write conflict
- `find_first_version_conflict()` - Early-exit on first version mismatch

These functions return `Option<ConflictResult>` and stop at the first conflict found, which is more efficient when you only need to know if validation failed.

---

## JSON Operations (M5)

### 7. JsonStoreExt Trait (`transaction.rs`)

Per M5 Rule 3: Add JsonStoreExt trait to TransactionContext (NO separate JsonTransaction type).

```rust
pub trait JsonStoreExt {
    fn json_get(&mut self, key: &Key, path: &JsonPath) -> Result<Option<JsonValue>>;
    fn json_set(&mut self, key: &Key, path: &JsonPath, value: JsonValue) -> Result<()>;
    fn json_delete(&mut self, key: &Key, path: &JsonPath) -> Result<()>;
    fn json_get_document(&mut self, key: &Key) -> Result<Option<JsonValue>>;
    fn json_exists(&mut self, key: &Key) -> Result<bool>;
}
```

**Implementation Notes:**
- Read-your-writes: JSON writes visible to subsequent JSON reads in same transaction
- Lazy allocation: JSON fields only allocated when JSON ops performed
- Conflict detection: Document version tracked for commit validation

> **Known Issue:** `json_exists()` only checks the snapshot, NOT the write buffer. This means newly created documents in the current transaction will not be detected by `json_exists()`. See STRATA_CONCURRENCY_REVIEW.md Issue #6.

### 8. JSON Path/Patch Types (`transaction.rs`)

```rust
pub struct JsonPathRead {
    pub key: Key,
    pub path: JsonPath,
    pub version: u64,
}

pub struct JsonPatchEntry {
    pub key: Key,
    pub patch: JsonPatch,
    pub resulting_version: u64,  // NOTE: Always 0 until commit (placeholder)
}
```

> **Important Limitation:** The `resulting_version` field in `JsonPatchEntry` is always 0 because the actual version is not known until commit time. The field exists for future use but is currently a placeholder.

---

## Snapshot Isolation

### 9. ClonedSnapshotView (`snapshot.rs`)

M2 implementation using deep clone:

```rust
pub struct ClonedSnapshotView {
    version: u64,
    data: Arc<BTreeMap<Key, VersionedValue>>,
}
```

**Characteristics:**
- O(data_size) creation time (deep clones BTreeMap)
- Immutable after creation
- Thread-safe (Send + Sync)
- Acceptable for M2 agent workloads (<100MB)

**Implements:** `SnapshotView` trait from strata_core
- `get(key)` - Point lookup
- `scan_prefix(prefix)` - Prefix scan
- `version()` - Snapshot version

**Additional Methods:**
- `from_arc(version, data)` - Constructor from Arc for snapshot sharing
- `empty()` - Create empty snapshot
- `len()`, `is_empty()` - Snapshot introspection
- `data()` - Test-only access to underlying data

---

## Recovery System

### 10. RecoveryCoordinator (`recovery.rs`)

Transaction-aware database recovery from WAL:

```rust
pub struct RecoveryCoordinator {
    wal_path: PathBuf,
    snapshot_path: Option<PathBuf>,
}

pub struct RecoveryResult {
    pub storage: ShardedStore,
    pub txn_manager: TransactionManager,
    pub stats: RecoveryStats,
}

pub struct RecoveryStats {
    pub txns_replayed: usize,
    pub incomplete_txns: usize,
    pub aborted_txns: usize,
    pub writes_applied: usize,
    pub deletes_applied: usize,
    pub final_version: u64,
    pub max_txn_id: u64,
    pub from_checkpoint: bool,
}
```

**Recovery Algorithm (per spec Section 5):**
1. Load snapshot (if exists)
2. Replay WAL entries > snapshot watermark
3. For each transaction:
   - BeginTxn: Start tracking
   - Write/Delete: Buffer operations
   - CommitTxn: Apply buffered operations
   - AbortTxn: Discard buffered operations
4. Discard incomplete transactions (no CommitTxn)
5. Initialize TransactionManager with max_txn_id + 1

**Properties:**
- Deterministic: Same WAL → same result
- Idempotent: Can replay multiple times
- Version-preserving: Replays use original version numbers

---

## WAL Integration

### 11. TransactionWALWriter (`wal_writer.rs`)

Transaction WAL writer using Legacy WAL system:

```rust
pub struct TransactionWALWriter<'a> {
    wal: &'a mut WAL,
    txn_id: u64,
    run_id: RunId,
}
```

**Methods:**
- `write_begin()` - Write BeginTxn entry
- `write_put(key, value, version)` - Write entry
- `write_delete(key, version)` - Delete entry
- `write_commit()` - CommitTxn entry (DURABILITY POINT)
- `write_abort()` - AbortTxn entry

**Vector Operations (TransactionWALWriter only):**
- `write_vector_collection_create(collection, dimension, metric, version)`
- `write_vector_collection_delete(collection, version)`
- `write_vector_upsert(collection, key, vector_id, embedding, metadata, version, source_ref)`
- `write_vector_delete(collection, key, vector_id, version)`

**Important:** Vector operations exist ONLY in `TransactionWALWriter`, NOT in `TransactionContext`. There are NO `vector_insert`, `vector_delete`, or similar methods on `TransactionContext`.

**Uses:** Legacy `WAL` and `WALEntry` from strata_durability

---

## Transaction Pooling (M4)

### 12. Reset for Reuse (`transaction.rs`)

```rust
impl TransactionContext {
    pub fn reset(&mut self, txn_id: u64, run_id: RunId, snapshot: Option<Box<dyn SnapshotView>>) {
        self.txn_id = txn_id;
        self.run_id = run_id;
        self.start_version = snapshot.as_ref().map(|s| s.version()).unwrap_or(0);
        self.snapshot = snapshot;

        // Clear but preserve capacity!
        self.read_set.clear();
        self.write_set.clear();
        self.delete_set.clear();
        self.cas_set.clear();

        // JSON fields deallocated (rare usage)
        self.json_reads = None;
        self.json_writes = None;
        self.json_snapshot_versions = None;

        self.status = TransactionStatus::Active;
        self.start_time = Instant::now();
    }

    pub fn capacity(&self) -> (usize, usize, usize, usize) {
        (read_set.capacity(), write_set.capacity(), delete_set.capacity(), cas_set.capacity())
    }
}
```

**Optimization:** HashMap/HashSet::clear() preserves capacity, enabling transaction pooling without reallocation.

---

## How Concurrency Uses Other Crates

### Types from `strata_core`:
| Type | Usage |
|------|-------|
| `Key` | Operation keys |
| `RunId` | Transaction run identification |
| `Value` | Written values |
| `Version` | Version tracking |
| `VersionedValue` | Snapshot data |
| `SnapshotView` trait | Snapshot interface |
| `Storage` trait | Storage validation |
| `JsonPath`, `JsonPatch`, `JsonValue` | JSON operations |

### From `strata_storage`:
| Type | Usage |
|------|-------|
| `UnifiedStore` | Transaction validation target |
| `ShardedStore` | Recovery output |

### From `strata_durability`:
| Type | Usage |
|------|-------|
| `WAL` | Write-ahead log |
| `WALEntry` | Legacy WAL entries |
| `DurabilityMode` | WAL configuration |

---

## Summary

The concurrency crate provides:

1. **OCC with First-Committer-Wins:**
   - Read-set based conflict detection
   - Blind writes allowed (no conflict)
   - CAS validated separately

2. **Atomic Commit Protocol:**
   - Validation → WAL → Storage application
   - TOCTOU prevention via commit lock
   - WAL is durability point

3. **JSON Support (M5):**
   - Region-based conflict detection
   - Path overlap analysis
   - Cross-primitive atomic transactions

4. **Recovery System:**
   - Transaction-aware WAL replay
   - Incomplete transaction handling
   - Version-preserving replay

5. **Performance Optimizations:**
   - Transaction pooling with capacity preservation
   - Lazy JSON field allocation
   - Lock-free version/txn_id allocation
