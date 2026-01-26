# Strata Durability Crate Analysis

## Overview

The `strata-durability` crate is **Layer 8** in the Strata architecture. It provides WAL (Write-Ahead Log) infrastructure, snapshot management, crash recovery, and portable run export/import. This crate contains **two competing WAL systems** - a legacy system and a modern system.

```
strata-durability/
├── WAL Systems
│   ├── Legacy WAL (wal.rs)
│   │   ├── WALEntry enum (14 variants)
│   │   ├── bincode serialization
│   │   └── DurabilityMode enum
│   │
│   └── Modern WAL (wal_types.rs, wal_entry_types.rs)
│       ├── WalEntry struct
│       ├── WalEntryType enum (20+ variants)
│       ├── TxId (UUID-based transaction ID)
│       └── CRC32 validation
│
├── Transaction Log (transaction_log.rs)
│   ├── Transaction struct (cross-primitive)
│   └── TxEntry enum (15 variants)
│
├── WAL I/O
│   ├── WalWriter (wal_writer.rs)
│   ├── WalReader (wal_reader.rs)
│   └── WalManager (wal_manager.rs)
│
├── Snapshots
│   ├── SnapshotWriter/Reader (snapshot.rs)
│   ├── SnapshotEnvelope, PrimitiveSection (snapshot_types.rs)
│   └── Primitive IDs (1-7)
│
├── Recovery
│   ├── RecoveryEngine (recovery_manager.rs)
│   ├── RecoveryOptions, RecoveryResult
│   └── Transaction boundary awareness
│
├── Run Lifecycle (run_lifecycle.rs)
│   ├── RunBegin / RunEnd entries
│   └── Payload parsing
│
└── RunBundle (run_bundle/)
    ├── Portable archive format (.runbundle.tar.zst)
    ├── MANIFEST.json + RUN.json + WAL.runlog
    ├── RunBundleWriter / RunBundleReader
    └── Checksum validation (xxh3)
```

---

## Two Competing WAL Systems

### 1. Legacy WAL System (`wal.rs`)

**Used extensively**: 821 occurrences across 23 files (per STRATA_STORAGE_REVIEW.md)

```rust
pub enum WALEntry {
    // Transaction control (5 variants)
    BeginTxn { txn_id: u64, run_id: RunId, timestamp: Timestamp },
    Write { run_id: RunId, key: Key, value: Value, version: u64 },
    Delete { run_id: RunId, key: Key, version: u64 },
    CommitTxn { txn_id: u64, run_id: RunId },
    AbortTxn { txn_id: u64, run_id: RunId },

    // Checkpoint (with snapshot support)
    Checkpoint { snapshot_id: Uuid, version: u64, active_runs: Vec<RunId> },

    // JSON operations (4 variants)
    JsonCreate { run_id: RunId, doc_id: JsonDocId, value_bytes: Vec<u8>, version: u64, timestamp: Timestamp },
    JsonSet { run_id: RunId, doc_id: JsonDocId, path: JsonPath, value_bytes: Vec<u8>, version: u64 },
    JsonDelete { run_id: RunId, doc_id: JsonDocId, path: JsonPath, version: u64 },
    JsonDestroy { run_id: RunId, doc_id: JsonDocId },

    // Vector operations (4 variants)
    VectorCollectionCreate { run_id: RunId, collection: String, dimension: usize, metric: u8, version: u64 },
    VectorCollectionDelete { run_id: RunId, collection: String, version: u64 },
    VectorUpsert { run_id: RunId, collection: String, key: String, vector_id: u64, embedding: Vec<f32>, metadata: Option<Vec<u8>>, version: u64, source_ref: Option<EntityRef> },
    VectorDelete { run_id: RunId, collection: String, key: String, vector_id: u64, version: u64 },
}
```

**Total: 14 variants** (5 transaction + 1 checkpoint + 4 JSON + 4 Vector)

**Characteristics:**
- Uses `bincode` serialization via `encoding.rs`
- Transaction ID is `u64`
- Run-scoped entries (all entries include `run_id`)
- Used by RunBundle for export/import

### 2. Modern WAL System (`wal_types.rs` + `wal_entry_types.rs`)

**The newer, cleaner system:**

```rust
// Transaction ID (UUID-based)
pub struct TxId(Uuid);

// Self-describing WAL entry with CRC32 validation
pub struct WalEntry {
    pub entry_type: WalEntryType,  // What kind of operation
    pub version: u8,                // Protocol version (always 1)
    pub tx_id: TxId,               // Transaction grouping
    pub payload: Vec<u8>,          // Operation-specific data
}
```

**Wire Format:**
```
[length: u32][type: u8][version: u8][tx_id: 16 bytes][payload: N bytes][crc32: u32]
```

**Entry Type Registry (`WalEntryType`):**

| Range | Primitive | Entry Types |
|-------|-----------|-------------|
| 0x00-0x0F | Core | `TransactionCommit` (0x00), `TransactionAbort` (0x01), `SnapshotMarker` (0x02) |
| 0x10-0x1F | KV | `KvPut` (0x10), `KvDelete` (0x11) |
| 0x20-0x2F | JSON | `JsonCreate` (0x20), `JsonSet` (0x21), `JsonDelete` (0x22), `JsonDestroy` (0x23), `JsonPatch` (0x24) |
| 0x30-0x3F | Event | `EventAppend` (0x30) |
| 0x40-0x4F | State | `StateInit` (0x40), `StateSet` (0x41), `StateTransition` (0x42) |
| 0x60-0x6F | Run | `RunCreate` (0x60), `RunUpdate` (0x61), `RunEnd` (0x62), `RunBegin` (0x63) |
| 0x70-0x7F | Vector | `VectorCollectionCreate` (0x70), `VectorCollectionDelete` (0x71), `VectorUpsert` (0x72), `VectorDelete` (0x73) |

---

## DurabilityMode

Both systems share the same durability mode enum (3 modes total):

```rust
pub enum DurabilityMode {
    /// No persistence - data lost on crash
    /// Performance: < 3µs per operation
    /// Previously called "InMemory" in older documentation
    None,

    /// fsync after every commit marker
    /// Performance: ~2ms per operation (disk latency)
    Strict,

    /// Periodic fsync based on time/count
    /// Default: 100ms interval OR 1000 writes
    /// Performance: ~10-50µs per operation
    Batched { interval_ms: u64, batch_size: usize },
}
```

> **Note:** Some older documentation may reference "Async" or "InMemory" modes. The actual enum variants are `None`, `Strict`, and `Batched`. The "InMemory" mode was renamed to `None` for clarity.

---

## Transaction Log (`transaction_log.rs`)

The `Transaction` struct enables **cross-primitive atomic operations**:

```rust
pub struct Transaction {
    id: TxId,
    entries: Vec<TxEntry>,
}

pub enum TxEntry {
    // KV operations
    KvPut { key: String, value: String },
    KvDelete { key: String },

    // JSON operations
    JsonCreate { key: String, doc: Vec<u8> },
    JsonSet { key: String, doc: Vec<u8> },
    JsonDelete { key: String },
    JsonDestroy { key: String },
    JsonPatch { key: String, patch: Vec<u8> },

    // Event operations
    EventAppend { payload: Vec<u8> },

    // State operations
    StateInit { key: String, value: String },
    StateSet { key: String, value: String },
    StateTransition { key: String, from: String, to: String },

    // Run operations
    RunCreate { metadata: Vec<u8> },
    RunUpdate { metadata: Vec<u8> },
    RunBegin { metadata: Vec<u8> },
    RunEnd { metadata: Vec<u8> },
}
```

**Usage (Builder Pattern):**
```rust
let mut tx = Transaction::new();
tx.kv_put("user:123", "active")
  .json_set("profile:123", profile_json)
  .event_append(b"user_created".to_vec())
  .state_set("session:123", "authenticated");

// Convert to WAL entries with shared TxId
let (tx_id, wal_entries) = tx.into_wal_entries();
```

---

## WAL Writer (`wal_writer.rs`)

The `WalWriter` implements the transaction commit protocol:

```rust
pub struct WalWriter {
    path: PathBuf,
    writer: Arc<Mutex<BufWriter<File>>>,
    current_offset: Arc<AtomicU64>,
    durability_mode: DurabilityMode,
    // Batching state...
}
```

**Key Methods:**

| Method | Description |
|--------|-------------|
| `begin_transaction()` | Creates new TxId (UUID) |
| `write_tx_entry(tx_id, type, payload)` | Writes entry with TxId |
| `commit_transaction(tx_id)` | Writes commit marker, syncs in Strict mode |
| `abort_transaction(tx_id)` | Writes abort marker |
| `write_transaction(entries)` | Convenience: writes + commits |
| `commit_atomic(Transaction)` | Commits cross-primitive transaction |

**Commit Marker Protocol:**
1. Entries without commit marker are **invisible** after recovery
2. Commit marker is the **durability point**
3. After commit marker is synced, transaction is durable
4. Abort markers are optional (uncommitted entries discarded anyway)

---

## WAL Reader (`wal_reader.rs`)

The `WalReader` provides corruption detection and resync:

```rust
pub struct WalReader {
    reader: BufReader<File>,
    position: u64,
    file_size: u64,
    corruption_count: u64,
    resync_count: u64,
}
```

**Features:**
- CRC32 validation on every entry
- Automatic resync after corruption (scans forward looking for valid entry)
- Transaction-aware iteration (`read_committed()` returns only committed entries)
- Offset tracking for recovery

---

## WAL Manager (`wal_manager.rs`)

Handles WAL file operations including truncation after snapshots:

```rust
pub struct WalManager {
    path: PathBuf,
    base_offset: AtomicU64,
}
```

**Truncation Strategy:**
1. Snapshot taken at WAL offset X
2. After snapshot is safely persisted, call `truncate_to(X)`
3. Keeps 1KB safety buffer before X
4. Uses atomic temp + rename pattern
5. Updates base offset tracking

---

## Snapshots (`snapshot.rs` + `snapshot_types.rs`)

### Snapshot File Layout

```
+------------------+
| Magic (10 bytes) |  "INMEM_SNAP"
+------------------+
| Version (4)      |  Format version (1)
+------------------+
| Timestamp (8)    |  Microseconds since epoch
+------------------+
| WAL Offset (8)   |  WAL position covered
+------------------+
| Tx Count (8)     |  Transactions included
+------------------+
| Primitive Count  |  Number of sections (1 byte)
+------------------+
| Primitive 1      |  Type (1) + Length (8) + Data
+------------------+
| ...              |
+------------------+
| CRC32 (4)        |  Checksum of everything above
+------------------+
```

### Primitive IDs

| ID | Primitive | Status |
|----|-----------|--------|
| 1 | KV | Active |
| 2 | JSON | Active |
| 3 | Event | Active |
| 4 | State | Active |
| 5 | (Trace) | Skipped - was deprecated primitive, ID reserved |
| 6 | Run | Active |
| 7 | Vector | Active |

### Key Principle

> Snapshots are **physical** (materialized state), not **semantic** (history).
> They compress WAL effects but are not the history itself.

### SnapshotWriter

```rust
pub struct SnapshotWriter {
    hasher: crc32fast::Hasher,
}

// Atomic write pattern
let info = writer.write_atomic(&header, &sections, path)?;
```

Uses temp file + rename pattern for atomic writes.

---

## Recovery (`recovery_manager.rs`)

The `RecoveryEngine` combines snapshot loading with WAL replay:

### Recovery Sequence

1. Find latest valid snapshot (falls back to older if corrupt)
2. Load snapshot into memory
3. Replay WAL from snapshot's WAL offset
4. Respect transaction boundaries

### Recovery Options

```rust
pub struct RecoveryOptions {
    max_corrupt_entries: usize,    // Default: 10
    verify_all_checksums: bool,    // Default: true
    rebuild_indexes: bool,         // Default: true
    verbose: bool,                 // Default: false
}
```

Presets:
- `RecoveryOptions::default()` - Balanced
- `RecoveryOptions::strict()` - Fail on any corruption
- `RecoveryOptions::permissive()` - Tolerate up to 100 corrupt entries
- `RecoveryOptions::fast()` - Skip some safety checks

### Key Principle

> After crash recovery, the database must correspond to a **prefix of the
> committed transaction history**. No partial transactions may be visible.

### Transaction Boundary Awareness

```rust
// Only returns entries from committed transactions
pub fn replay_wal_committed(
    wal_path: &Path,
    from_offset: u64,
    options: &RecoveryOptions,
) -> Result<(CommittedTransactions, WalReplayResultPublic), RecoveryError>
```

- Only returns entries from transactions with commit markers
- Discards entries from aborted transactions
- Discards entries from incomplete (orphaned) transactions

---

## Run Lifecycle (`run_lifecycle.rs`)

WAL entries for tracking run start/end:

### RunBegin Payload
```
[run_id: 16 bytes][timestamp: 8 bytes]
```

### RunEnd Payload
```
[run_id: 16 bytes][timestamp: 8 bytes][event_count: 8 bytes]
```

Both use `TxId::nil()` (non-transactional markers).

---

## RunBundle (Portable Artifacts)

The `run_bundle/` module implements export/import of completed runs as portable archives.

### Archive Format

```
<run_id>.runbundle.tar.zst
└── runbundle/
    ├── MANIFEST.json    # Format version, checksums
    ├── RUN.json         # Run metadata (id, state, tags, error)
    └── WAL.runlog       # Run-scoped WAL entries (Legacy format!)
```

### Design Principles

| Principle | Description |
|-----------|-------------|
| **Explicit** | All operations explicit, no background behavior |
| **Immutable** | Only terminal runs can be exported (Completed, Failed, Cancelled, Archived) |
| **Portable** | Archives can be moved between machines, stored in VCS |
| **Inspectable** | Standard tools (tar, jq) can inspect contents |
| **Deterministic** | Same run exported twice produces identical bundles |

### MANIFEST.json

```json
{
  "format_version": 1,
  "strata_version": "0.12.0",
  "created_at": "2025-01-24T12:00:00Z",
  "checksum_algorithm": "xxh3",
  "checksums": {
    "RUN.json": "abc123...",
    "WAL.runlog": "def456...",
    "MANIFEST.json": "ghi789..."
  },
  "contents": {
    "wal_entry_count": 100,
    "wal_size_bytes": 5000
  }
}
```

### RUN.json

```json
{
  "run_id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "my-test-run",
  "state": "completed",
  "created_at": "2025-01-24T10:00:00Z",
  "closed_at": "2025-01-24T11:30:00Z",
  "parent_run_id": null,
  "tags": ["test", "v2"],
  "metadata": {"user_id": "user_123"},
  "error": null
}
```

### WAL.runlog Format

Uses **Legacy WAL format** (bincode):
```
[magic: "STRATA_WAL"][version: u16][entry_count: u64]
[entry 1][entry 2]...[entry N]
[checksum: xxh3]
```

### Usage

```rust
// Export
let info = RunBundleWriter::with_defaults()
    .write(&run_info, &wal_entries, Path::new("./run.runbundle.tar.zst"))?;

// Verify
let verify_info = RunBundleReader::validate(&path)?;

// Import
let contents = RunBundleReader::read_all(&path)?;
```

---

## Key Architectural Insights

### Two WAL Systems Coexist

| Aspect | Legacy WAL | Modern WAL |
|--------|------------|------------|
| **Location** | `wal.rs` | `wal_types.rs` + `wal_entry_types.rs` |
| **Format** | bincode | Custom length-prefixed + CRC32 |
| **TxId** | `u64` | `Uuid` (16 bytes) |
| **Usage** | RunBundle, older code | `WalWriter`, `WalReader`, recovery |
| **Entries** | 14 variants | 20+ variants |

### RunBundle Uses Legacy Format

The `run_bundle/wal_log.rs` uses `WALEntry` (legacy) for serialization:
```rust
use crate::wal::WALEntry;
```

This creates a dependency on the legacy system for portable artifacts.

### Transaction Atomicity

Both systems support atomic cross-primitive commits:
1. All entries in a transaction share the same TxId
2. Commit marker makes the entire transaction visible
3. Recovery respects transaction boundaries

### Durability Guarantees

| Mode | Latency | Guarantee |
|------|---------|-----------|
| `None` | < 3µs | No durability (data lost on crash) |
| `Batched` | 10-50µs | May lose up to `interval_ms` of commits |
| `Strict` | ~2ms | Every commit survives crash |

---

## Dependencies

From the crate:

```toml
strata-core = { path = "../core" }
strata-storage = { path = "../storage" }  # For PrimitiveStorageExt
bincode        # Legacy WAL serialization
crc32fast      # Checksum validation
uuid           # TxId in modern system
parking_lot    # Mutex for writer
zstd           # RunBundle compression
tar            # RunBundle archive format
xxhash-rust    # RunBundle checksums
serde_json     # MANIFEST.json, RUN.json
tracing        # Logging
```

---

## Summary

The durability crate provides:

1. **Two WAL Systems:**
   - Legacy (bincode, `u64` txn_id, 14 entry types) - used by RunBundle
   - Modern (CRC32, UUID txn_id, 20+ entry types) - used by writer/reader/recovery

2. **Cross-Primitive Transactions:**
   - `Transaction` struct with builder pattern
   - All entries share TxId for atomic commit/abort

3. **Configurable Durability:**
   - None (testing), Batched (production), Strict (audit)

4. **Snapshot System:**
   - Binary format with per-primitive sections
   - CRC32 checksums
   - Atomic write pattern

5. **Recovery Engine:**
   - Snapshot + WAL replay
   - Transaction boundary awareness
   - Corruption detection with resync

6. **Portable Archives:**
   - `.runbundle.tar.zst` format
   - Export completed runs
   - Verify checksums
   - Import into other instances
