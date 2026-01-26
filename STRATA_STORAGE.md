# Strata Storage Crate Analysis

## Overview

The `strata-storage` crate is **Layer 7** in the Strata architecture. It provides both in-memory and disk-based storage backends with the following major components:

```
strata-storage/
├── In-Memory Storage
│   ├── UnifiedStore (BTreeMap + RwLock - MVP)
│   ├── ShardedStore (DashMap + FxHashMap - Performance)
│   ├── Secondary Indices (RunIndex, TypeIndex, TTLIndex)
│   ├── Snapshot Views (ClonedSnapshotView, ShardedSnapshot)
│   └── TTL Cleaner (background task)
│
├── Disk Storage
│   ├── WAL (Write-Ahead Log)
│   │   ├── WalWriter / WalReader
│   │   ├── DurabilityMode (None, Batched, Strict)
│   │   └── Segment rotation
│   ├── Snapshots (SnapshotWriter / SnapshotReader)
│   ├── MANIFEST (database metadata)
│   ├── Codec (identity, future: AES-GCM, LZ4)
│   └── Recovery (RecoveryCoordinator, WalReplayer)
│
└── Extension System
    ├── PrimitiveStorageExt trait
    └── PrimitiveRegistry
```

---

## Dependencies

From `Cargo.toml`:
```toml
strata-core = { path = "../core" }
parking_lot     # Fast RwLock
dashmap         # Lock-free concurrent HashMap
rustc-hash      # FxHashMap (fast non-crypto hash)
smallvec        # Stack-allocated vectors
crc32fast       # CRC32 checksums for WAL
byteorder       # Binary serialization
```

---

## In-Memory Storage

### 1. StoredValue (`stored_value.rs`)

Wraps `VersionedValue` (from core) with TTL support:

```rust
pub struct StoredValue {
    inner: VersionedValue,  // From strata_core
    ttl: Option<Duration>,  // Storage concern, not in contract
}
```

**Key Methods:**
- `is_expired()` - Checks if TTL has passed using `Timestamp::now()`
- `expiry_timestamp()` - Calculates when value expires

**Core Types Used:** `Timestamp`, `Value`, `Version`, `VersionedValue`

### 2. UnifiedStore (`unified.rs`)

MVP storage backend using BTreeMap with RwLock.

```rust
pub struct UnifiedStore {
    data: Arc<RwLock<BTreeMap<Key, StoredValue>>>,
    run_index: Arc<RwLock<RunIndex>>,
    type_index: Arc<RwLock<TypeIndex>>,
    ttl_index: Arc<RwLock<TTLIndex>>,
    version: AtomicU64,
}
```

**Implements:** `Storage` trait from `strata_core`

**Design Notes:**
- Global RwLock - contention on writes
- Version allocated BEFORE write lock (prevents contention)
- Secondary indices updated atomically with data
- TTL expiration at read time (not background)
- Has full secondary index support: RunIndex, TypeIndex, TTLIndex

**Core Types Used:** `Key`, `RunId`, `TypeTag`, `Value`, `Version`, `VersionedValue`, `Timestamp`, `Storage` trait, `Result`

### 3. ShardedStore (`sharded.rs`)

High-performance storage with per-run sharding.

```rust
pub struct ShardedStore {
    shards: DashMap<RunId, Shard>,  // Per-run partitioning
    version: AtomicU64,
}

pub struct Shard {
    data: FxHashMap<Key, VersionChain>,  // O(1) lookups
}

pub struct VersionChain {
    versions: VecDeque<StoredValue>,  // Newest first (MVCC)
}
```

**Performance Targets:**
- `get()`: Lock-free via DashMap
- `put()`: Only locks target shard
- Snapshot acquisition: < 500ns
- Different runs: Never contend

**MVCC Support:**
- `VersionChain` stores multiple versions per key
- Versions stored newest-first for efficient snapshot reads
- `gc(min_version)` removes old versions

> **Important:** ShardedStore does NOT have secondary indices (RunIndex, TypeIndex, TTLIndex). These are only available in UnifiedStore. This means `scan_by_type()` is not efficiently implemented in ShardedStore.

**Core Types Used:** `Key`, `RunId`, `TypeTag`, `Value`, `Version`, `VersionedValue`, `Timestamp`

### 4. Secondary Indices (`index.rs`, `ttl.rs`)

```rust
// RunIndex: Maps RunId → Set<Key>
pub struct RunIndex {
    index: HashMap<RunId, HashSet<Key>>,
}

// TypeIndex: Maps TypeTag → Set<Key>
pub struct TypeIndex {
    index: HashMap<TypeTag, HashSet<Key>>,
}

// TTLIndex: Maps Timestamp → Set<Key> (BTreeMap for range queries)
pub struct TTLIndex {
    index: BTreeMap<Timestamp, HashSet<Key>>,
}
```

**Purpose:**
- `scan_by_run()`: O(run size) instead of O(total data)
- `scan_by_type()`: O(type size) instead of O(total data)
- `find_expired()`: O(expired count) instead of O(total data)

**Core Types Used:** `Key`, `RunId`, `TypeTag`, `Timestamp`

### 5. Snapshot Views (`snapshot.rs`, `sharded.rs`)

**ClonedSnapshotView** (MVP - for UnifiedStore):
```rust
pub struct ClonedSnapshotView {
    version: u64,
    data: Arc<BTreeMap<Key, StoredValue>>,  // Deep clone
}
```
- O(n) creation (clones entire BTreeMap)
- Immutable after creation

**ShardedSnapshot** (Performance - for ShardedStore):
```rust
pub struct ShardedSnapshot {
    version: u64,
    store: Arc<ShardedStore>,               // Reference only
    cache: RwLock<FxHashMap<Key, Option<VersionedValue>>>,  // Copy-on-read
}
```
- O(1) creation (just Arc::clone + atomic load)
- < 500ns acquisition time
- Copy-on-read caching for isolation

> **Warning:** The `ShardedSnapshot` cache grows unboundedly during snapshot lifetime. For long-running transactions accessing many keys, this could cause memory exhaustion. The cache is never evicted.

Both implement `SnapshotView` trait from `strata_core`.

### 6. TTL Cleaner (`cleaner.rs`)

Background task for cleaning expired keys:

```rust
pub struct TTLCleaner {
    store: Arc<UnifiedStore>,  // NOTE: Hardcoded to UnifiedStore!
    check_interval: Duration,
    shutdown: Arc<AtomicBool>,
}
```

- Runs in background thread
- Uses `store.find_expired_keys()` + `store.delete()`
- Graceful shutdown via atomic flag

> **Limitation:** TTLCleaner only works with `UnifiedStore`. It cannot be used with `ShardedStore` because ShardedStore lacks the TTLIndex required for `find_expired_keys()`.

---

## Disk Storage

### 7. WAL (Write-Ahead Log) (`wal/`)

**DurabilityMode:**
```rust
pub enum DurabilityMode {
    /// No persistence - data lost on crash
    /// Performance: < 3µs per operation
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

> **Note:** Documentation may reference "InMemory" or "Async" modes - these do not exist. The actual modes are `None`, `Strict`, and `Batched`.

**WalWriter:**
```rust
pub struct WalWriter {
    segment: Option<WalSegment>,
    durability: DurabilityMode,
    wal_dir: PathBuf,
    database_uuid: [u8; 16],
    config: WalConfig,
    codec: Box<dyn StorageCodec>,
    // Batching state...
}
```

**Key Operations:**
- `append(&record)` - Writes WAL record
- `rotate_segment()` - Creates new segment when size limit reached
- `flush()` - Force sync to disk

**WAL Directory Structure:**
```
wal/
├── wal-000001.seg   (closed, immutable)
├── wal-000002.seg   (closed, immutable)
└── wal-000003.seg   (active, writable)
```

### 8. Format Module (`format/`)

Defines on-disk byte formats:

**WAL Record:**
- Magic: `STRA`
- Format version: 1
- Segment header: 64 bytes
- Records: length-prefixed with CRC32

**Snapshot:**
- Magic: `SNAP`
- Format version: 1
- Header: 64 bytes
- Sections per primitive type

**MANIFEST:**
- Magic: `MANF`
- Format version: 1
- Contains: database_uuid, codec_id, snapshot_id, watermark

### 9. Codec (`codec/`)

```rust
pub trait StorageCodec: Send + Sync {
    fn codec_id(&self) -> &'static str;
    fn encode(&self, data: &[u8]) -> Vec<u8>;
    fn decode(&self, data: &[u8]) -> Result<Vec<u8>, CodecError>;
}
```

**Current:** `IdentityCodec` (pass-through)
**Future:** `AesGcmCodec`, `Lz4Codec`, `ChainedCodec`

### 10. Recovery (`recovery/`)

```rust
pub struct RecoveryCoordinator {
    db_dir: PathBuf,
    codec: Box<dyn StorageCodec>,
}
```

**Recovery Algorithm:**
1. Load MANIFEST
2. If snapshot exists: load snapshot → replay WAL > watermark
3. If no snapshot: replay all WAL
4. Truncate partial records at WAL tail

**Properties:** Deterministic, Idempotent, Atomic

---

## Database Lifecycle (`database/`)

### 11. DatabaseHandle (`database/handle.rs`)

The main entry point for disk-backed database operations:

```rust
pub struct DatabaseHandle {
    paths: DatabasePaths,
    manifest: Arc<Mutex<ManifestManager>>,
    wal_writer: Arc<Mutex<WalWriter>>,
    checkpoint_coordinator: Arc<Mutex<CheckpointCoordinator>>,
    codec: Box<dyn StorageCodec>,
    config: DatabaseConfig,
    database_uuid: [u8; 16],
}
```

**Key Methods:**
- `create(path, config)` - Create new database at path
- `open(path, config)` - Open existing database
- `open_or_create(path, config)` - Open if exists, create otherwise
- `recover(on_snapshot, on_record)` - Recover with callbacks
- `append_wal(&record)` - Append to WAL
- `flush_wal()` - Force WAL sync
- `checkpoint(watermark, data)` - Create checkpoint
- `close(self)` - Close and flush
- `uuid()` - Database UUID
- `path()`, `paths()` - Path access
- `config()` - Config access
- `manifest()` - Current manifest
- `watermark()` - Current WAL watermark

### 12. DatabaseConfig (`database/config.rs`)

Configuration for database creation:

```rust
pub struct DatabaseConfig {
    pub durability: DurabilityMode,
    pub wal_config: WalConfig,
    pub codec_id: String,
}
```

**Builder Methods:**
- `default()` - Default configuration
- `strict()` - fsync every commit
- `batched()` - Periodic fsync
- `for_testing()` - Fast test configuration
- `with_durability(mode)` - Set durability mode
- `with_wal_config(config)` - Set WAL config
- `with_wal_segment_size(size)` - Set segment size
- `with_codec(codec_id)` - Set codec
- `validate()` - Validate configuration

### 13. DatabasePaths (`database/paths.rs`)

Path layout for database files:

```rust
pub struct DatabasePaths {
    root: PathBuf,
}
```

**Directory Structure:**
```
<root>/
├── MANIFEST
├── wal/
│   └── wal-XXXXXX.seg
├── snapshots/
│   └── snapshot-XXXXXX.snap
└── data/
```

**Methods:**
- `from_root(path)` - Create from root path
- `root()`, `manifest()`, `wal_dir()`, `snapshots_dir()`, `data_dir()`
- `exists()` - Check if database exists
- `create_directories()` - Create directory structure
- `validate()` - Validate directory structure

---

## Compaction System (`compaction/`)

### 14. CompactMode (`compaction/mod.rs`)

Compaction operation modes:

```rust
pub enum CompactMode {
    /// Remove WAL segments covered by snapshot
    WALOnly,
    /// Full compaction: WAL + retention policy enforcement
    Full,
}
```

**Methods:**
- `name()` - Human-readable name
- `applies_retention()` - Whether mode enforces retention

### 15. WalOnlyCompactor (`compaction/wal_only.rs`)

Removes WAL segments covered by snapshots:

```rust
pub struct WalOnlyCompactor {
    wal_dir: PathBuf,
    manifest: Arc<Mutex<ManifestManager>>,
}
```

**Key Method:**
- `compact()` - Returns `CompactInfo` with reclaimed bytes and segments removed

### 16. CompactInfo (`compaction/mod.rs`)

Result of compaction operation:

```rust
pub struct CompactInfo {
    pub mode: CompactMode,
    pub reclaimed_bytes: u64,
    pub wal_segments_removed: usize,
    pub versions_removed: usize,
    pub snapshot_watermark: Option<u64>,
    pub duration_ms: u64,
    pub timestamp: u64,
}
```

---

## Retention Policy (`retention/`)

### 17. RetentionPolicy (`retention/policy.rs`)

Controls how long historical versions are kept:

```rust
pub enum RetentionPolicy {
    /// Keep all versions forever (default)
    KeepAll,
    /// Keep last N versions per key
    KeepLast(usize),
    /// Keep versions younger than duration
    KeepFor(Duration),
    /// Per-primitive policies
    Composite {
        default: Box<RetentionPolicy>,
        overrides: HashMap<PrimitiveType, Box<RetentionPolicy>>,
    },
}
```

**Factory Methods:**
- `keep_all()` - Keep everything
- `keep_last(n)` - Keep last N versions
- `keep_for(duration)` - Keep recent versions
- `composite(default)` - Start building composite policy

**Evaluation:**
- `should_retain(version, timestamp, version_count, current_time, primitive_type)`

**Serialization:**
- `to_bytes()`, `from_bytes()` - Binary serialization

### 18. CompositeBuilder (`retention/policy.rs`)

Builder for composite retention policies:

```rust
pub struct CompositeBuilder {
    default: Box<RetentionPolicy>,
    overrides: HashMap<PrimitiveType, Box<RetentionPolicy>>,
}
```

**Usage:**
```rust
let policy = RetentionPolicy::composite(RetentionPolicy::keep_last(10))
    .with_override(PrimitiveType::Event, RetentionPolicy::keep_all())
    .with_override(PrimitiveType::Kv, RetentionPolicy::keep_last(5))
    .build();
```

---

## Testing Framework (`testing/`)

### 19. WalCorruptionTester (`testing/`)

Test harness for crash recovery scenarios:

```rust
pub struct WalCorruptionTester { ... }
```

**Key Types:**
- `CrashConfig` - Configuration for crash simulation
- `CrashPoint` - Where to simulate crash (BeforeWrite, DuringWrite, AfterWrite)
- `CrashType` - Type of corruption (Truncate, Corrupt, Garbage)
- `ReferenceModel` - Expected state for verification
- `VerificationResult` - Comparison of actual vs expected state

**Usage:** Testing that recovery correctly handles various failure scenarios.

---

## Extension System

### 20. PrimitiveStorageExt (`primitive_ext.rs`)

Trait for primitives to integrate with storage:

```rust
pub trait PrimitiveStorageExt: Send + Sync {
    fn primitive_type_id(&self) -> u8;
    fn wal_entry_types(&self) -> &'static [u8];
    fn snapshot_serialize(&self) -> Result<Vec<u8>, PrimitiveExtError>;
    fn snapshot_deserialize(&mut self, data: &[u8]) -> Result<(), PrimitiveExtError>;
    fn apply_wal_entry(&mut self, entry_type: u8, payload: &[u8]) -> Result<(), PrimitiveExtError>;
    fn primitive_name(&self) -> &'static str;
    fn rebuild_indexes(&mut self) -> Result<(), PrimitiveExtError>;
}
```

**WAL Entry Type Ranges:**

| Primitive | Range | Status |
|-----------|-------|--------|
| Core | 0x00-0x0F | FROZEN |
| KV | 0x10-0x1F | FROZEN |
| JSON | 0x20-0x2F | FROZEN |
| Event | 0x30-0x3F | FROZEN |
| State | 0x40-0x4F | FROZEN |
| (ID 5 skipped) | 0x50-0x5F | Reserved (was Trace, deprecated) |
| Run | 0x60-0x6F | FROZEN |
| Vector | 0x70-0x7F | RESERVED |
| Future | 0x80-0xFF | AVAILABLE |

### 21. PrimitiveRegistry (`registry.rs`)

Dynamic primitive registration:

```rust
pub struct PrimitiveRegistry {
    primitives: HashMap<u8, Arc<dyn PrimitiveStorageExt>>,
    wal_type_to_primitive: HashMap<u8, u8>,
}
```

**Usage:**
```rust
registry.register(Arc::new(KvStorageExt::new(kv)));
let prim = registry.get_for_wal_type(0x10);  // Routes to KV
```

---

## How Storage Uses Core

### Types Imported from `strata_core`:

| Type | Usage |
|------|-------|
| `Key` | Universal storage key (namespace + type_tag + user_key) |
| `RunId` | Shard partitioning key |
| `Namespace` | Multi-tenant isolation |
| `TypeTag` | Primitive type discrimination |
| `Value` | Universal value type (8 variants) |
| `Version` | MVCC version (Txn, Sequence, Counter) |
| `VersionedValue` | Value + Version + Timestamp |
| `Timestamp` | TTL tracking, expiration |
| `Storage` trait | Backend interface |
| `SnapshotView` trait | Read-only snapshot interface |
| `Result` | Error handling |

### Key Integration Points:

1. **StoredValue wraps VersionedValue:**
   ```rust
   struct StoredValue {
       inner: VersionedValue,  // From core
       ttl: Option<Duration>,  // Storage-only concern
   }
   ```

2. **Stores implement Storage trait:**
   ```rust
   impl Storage for UnifiedStore {
       fn get(&self, key: &Key) -> Result<Option<VersionedValue>>;
       fn put(&self, key: Key, value: Value, ttl: Option<Duration>) -> Result<u64>;
       // ...
   }
   ```

3. **Snapshots implement SnapshotView trait:**
   ```rust
   impl SnapshotView for ShardedSnapshot {
       fn get(&self, key: &Key) -> Result<Option<VersionedValue>>;
       fn scan_prefix(&self, prefix: &Key) -> Result<Vec<(Key, VersionedValue)>>;
       fn version(&self) -> u64;
   }
   ```

4. **Version stored as Version::Txn:**
   ```rust
   let stored_value = StoredValue::new(value, Version::txn(version), ttl);
   ```
   Note: Storage always uses `Version::Txn` variant

5. **Timestamp for TTL:**
   ```rust
   let now = Timestamp::now();
   if let Some(age) = now.duration_since(self.inner.timestamp) {
       return age >= ttl;
   }
   ```

---

## Summary

The storage crate provides:

1. **Two storage backends:**
   - `UnifiedStore`: Simple, correct, uses global lock, has secondary indices
   - `ShardedStore`: High-performance, per-run sharding, MVCC (no secondary indices)

2. **Durability system:**
   - WAL with configurable fsync modes (None, Strict, Batched)
   - Snapshots for point-in-time recovery
   - MANIFEST for metadata tracking
   - Codec seam for future encryption

3. **Database lifecycle:**
   - `DatabaseHandle` for disk-backed databases
   - `DatabaseConfig` for configuration
   - `DatabasePaths` for file layout

4. **Compaction system:**
   - `CompactMode::WALOnly` - Remove covered WAL segments
   - `CompactMode::Full` - WAL + retention enforcement

5. **Retention policies:**
   - `KeepAll`, `KeepLast(n)`, `KeepFor(duration)`
   - `Composite` for per-primitive policies

6. **Extension system:**
   - `PrimitiveStorageExt` trait for new primitives
   - `PrimitiveRegistry` for dynamic registration
   - WAL entry type ranges pre-allocated

7. **Testing framework:**
   - `WalCorruptionTester` for crash recovery testing
   - `ReferenceModel` for correctness verification

8. **Full dependency on core types:**
   - All addressing uses `Key`, `RunId`, `Namespace`, `TypeTag`
   - All values use `Value`, `Version`, `VersionedValue`
   - All timestamps use `Timestamp`
   - Implements `Storage` and `SnapshotView` traits
