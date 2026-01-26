# Cross-Crate Analysis: Bottom 4 Layers

## Summary

Analysis of cross-crate consistency between the bottom 4 layers of the Strata architecture:
- **strata-core** (Layer 1) - Foundation types
- **strata-storage** (Layer 7) - Storage backends
- **strata-durability** (Layer 8) - WAL, snapshots, recovery
- **strata-concurrency** (Layer 6) - OCC, transactions

---

## Critical Issues

### 1. Three Competing WAL Systems (No Integration)

**Severity: CRITICAL**

The codebase has three incompatible WAL systems with **zero integration**:

| System | Location | TxId Type | Format | Usage |
|--------|----------|-----------|--------|-------|
| **Legacy** | `durability/wal.rs` | `u64` | bincode | Concurrency, RunBundle |
| **Modern** | `durability/wal_types.rs` | `TxId(Uuid)` | Envelope+CRC32 | Unused in practice |
| **Storage** | `storage/format/wal_record.rs` | `u64` | Custom segments | Recovery only |

**Problems:**
- **No conversion layer** between WALEntry, WalEntry, and WalRecord
- **Transaction IDs incompatible**: u64 counters vs 16-byte UUIDs
- **RunBundle locked to legacy format** - cannot deprecate without breaking portability
- **Concurrency crate only uses legacy** - never sees modern WAL types

**Evidence:**
```rust
// Concurrency uses Legacy WAL only (wal_writer.rs:4-5)
use strata_durability::wal::{WALEntry, WAL};

// Creates legacy entries with u64 txn_id
WALEntry::BeginTxn { txn_id: u64, run_id, timestamp }
```

---

### 2. Version Type Inconsistency (Enum vs u64)

**Severity: HIGH**

Core defines `Version` enum with semantic variants, but lower layers ignore it:

| Layer | Representation | API Boundary |
|-------|----------------|--------------|
| **Core** | `Version` enum (Txn/Sequence/Counter) | Returns Version |
| **Storage** | Wraps to `Version::txn()` internally | Accepts u64 |
| **Durability** | Raw `u64` everywhere | Stores u64 |
| **Concurrency** | Raw `u64` everywhere | Uses u64 |

**The Problem:**
```rust
// Storage wraps u64 into Version (unified.rs:206)
let stored_value = StoredValue::new(value, Version::txn(version), None);

// Immediately unwrapped for comparison (unified.rs:112)
if sv.version().as_u64() <= max_version { ... }

// Concurrency never sees Version enum (transaction.rs:80)
pub commit_version: u64,  // Raw u64
```

**Impact:**
- Type safety of Version enum is completely lost
- All 3 variants (Txn/Sequence/Counter) treated identically as u64
- 27+ locations with `.as_u64()` conversion calls
- Version semantics exist only at API boundary

---

### 3. Timestamp Unit Mismatch (Milliseconds vs Microseconds)

**Severity: HIGH**

The `Timestamp` type uses microseconds, but primitives use milliseconds:

| Component | Unit | Type | Location |
|-----------|------|------|----------|
| `Timestamp` (canonical) | Microseconds | u64 | core/contract/timestamp.rs |
| `Event.timestamp` | Milliseconds | i64 | core/primitives/event.rs:24 |
| `State.updated_at` | Milliseconds | i64 | core/primitives/state.rs:20 |
| EventLog timestamps | Milliseconds | i64 | primitives/event_log.rs:66-68 |
| RunIndex timestamps | Milliseconds | i64 | primitives/run_index.rs:159-161 |
| JsonStore timestamps | Milliseconds | i64 | primitives/json_store.rs:116 |
| Storage formats | Microseconds | u64 | storage/format/primitives.rs |
| Durability snapshots | Microseconds | u64 | durability/snapshot_types.rs |

**Critical Bug Found (durability/recovery.rs:704):**
```rust
// WRONG: Converts microseconds to SECONDS when MILLISECONDS expected!
let timestamp_secs = (timestamp.as_micros() / 1_000_000) as i64;
let doc = RecoveryJsonDoc::new(*doc_id, value, *version, timestamp_secs);
// RecoveryJsonDoc.created_at and updated_at expect MILLISECONDS (per comments at line 84-86)
// But this code produces SECONDS (dividing by 1,000,000 instead of 1,000)
```

**Impact:**
- **1000x timestamp errors in recovery path** - timestamps will be 1000x smaller than correct values
- For example: a document created at 2025-01-26 would appear to be created at ~1972
- Mixing i64 (signed) and u64 (unsigned) timestamps
- No compile-time protection against unit confusion
- **This is a data corruption bug that should be fixed immediately**

---

### 4. Error Information Loss During Conversion

**Severity: MEDIUM-HIGH**

Errors are converted to strings at multiple boundaries, losing structured information:

| Pattern | Locations | Impact |
|---------|-----------|--------|
| `e.to_string()` | 15+ locations | Loses error kind, source |
| `format!("{}", e)` | 8+ locations | Loses structured details |
| `Apply(String)` variants | recovery/mod.rs, transaction.rs | Forces string conversion |

**Examples:**
```rust
// storage/wal/reader.rs:38 - Loses IO error kind
.map_err(|e: std::io::Error| WalReaderError::IoError(e.to_string()))?;

// concurrency/transaction.rs:41 - WAL errors become strings
WALError(String),

// durability/recovery.rs:105 - Loses serde error details
.map_err(|e| strata_core::error::Error::SerializationError(e.to_string()))
```

**Impact:**
- Cannot match on specific error types after conversion
- Stack traces lost
- OS error codes lost
- Debugging production issues becomes harder

---

## Moderate Issues

### 5. Core Exports Unused by Lower Layers

**Severity: MEDIUM**

Several core exports are not consumed by storage/durability/concurrency:

| Export | Defined In | Used By | Status |
|--------|-----------|---------|--------|
| `SearchRequest`, `SearchResponse` | core/search_types.rs | primitives, search | Not in bottom 4 |
| `SearchBudget`, `SearchMode` | core/search_types.rs | search crate only | Not in bottom 4 |
| `RunEventOffsets` | core/run_types.rs | primitives, engine | Not in bottom 4 |
| `ChainVerification` | core/primitives/event.rs | primitives only | Not in bottom 4 |
| `placeholder()` | core/lib.rs | Nobody | Dead code |

**The `placeholder()` function is completely dead code:**
```rust
// core/lib.rs - does nothing, should be removed
pub fn placeholder() {
    // This crate will contain core types once implemented
}
```

---

### 6. Snapshot Isolation Implementation Mismatch

**Severity: MEDIUM**

Two different snapshot implementations with different characteristics:

| Implementation | Location | Creation Time | Memory |
|---------------|----------|---------------|--------|
| `ClonedSnapshotView` | concurrency/snapshot.rs | O(n) deep clone | Full copy |
| `ShardedSnapshot` | storage/sharded.rs | O(1) reference | Copy-on-read |

**Problem:**
- Concurrency crate uses `ClonedSnapshotView` (deep clone)
- Storage crate has `ShardedSnapshot` (efficient)
- No way for concurrency to use the efficient version
- Recovery outputs ShardedStore but tests use UnifiedStore

---

### 7. Single Commit Lock Bottleneck

**Severity: MEDIUM**

All transactions serialize through one lock, even for different runs:

```rust
// manager.rs:76
commit_lock: Mutex<()>,

// manager.rs:176 - ALL commits serialize here
let _commit_guard = self.commit_lock.lock();
```

**Impact:**
- ShardedStore's per-run sharding benefits lost at commit
- Only one transaction commits at a time across entire system
- Bottleneck under high throughput

---

### 8. Storage Trait Not Fully Implemented

**Severity: MEDIUM**

The `Storage` trait from core has methods that aren't consistently implemented:

| Method | UnifiedStore | ShardedStore |
|--------|-------------|--------------|
| `get()` | ✓ | ✓ |
| `put()` | ✓ | ✓ |
| `scan_by_run()` | ✓ (indexed) | ✓ (O(n) scan) |
| `scan_by_type()` | ✓ (indexed) | ✗ (not impl) |
| Secondary indices | ✓ RunIndex, TypeIndex, TTLIndex | ✗ None |

**Impact:** TTLCleaner only works with UnifiedStore (hardcoded dependency).

---

## Minor Issues

### 9. Primitive ID Gap (ID 5 Skipped)

Both storage and durability skip primitive ID 5:

```rust
// From primitive_ext.rs
pub const KV: u8 = 1;
pub const JSON: u8 = 2;
pub const EVENT: u8 = 3;
pub const STATE: u8 = 4;
// ID 5 SKIPPED (was Trace, deprecated)
pub const RUN: u8 = 6;
pub const VECTOR: u8 = 7;
```

**Impact:** Minor - should be documented why ID 5 is reserved/skipped.

---

### 10. WAL Entry Type Range Documentation Mismatch

**Location:** storage/ARCHITECTURE.md vs storage/primitive_ext.rs

| Primitive | Code | Documentation |
|-----------|------|---------------|
| State | 0x40-0x4F | 0x30-0x3F |
| Event | 0x30-0x3F | 0x40-0x4F |

Documentation and implementation have swapped ranges.

---

### 11. DurabilityMode Documentation Inconsistency

Documentation mentions 4 modes, enum has 3:

| Documented | Actual Enum |
|------------|-------------|
| InMemory | `None` |
| Strict | `Strict` |
| Batched | `Batched` |
| Async | (doesn't exist) |

---

## Cross-Crate Dependency Map

```
strata-core (Layer 1)
    ↓ exports: Key, RunId, Value, Version, Timestamp, Storage trait, SnapshotView trait

strata-storage (Layer 7)
    ↓ imports from core: Key, RunId, Value, Version (wraps to u64), Timestamp, traits
    ↓ exports: UnifiedStore, ShardedStore, ClonedSnapshotView, ShardedSnapshot

strata-durability (Layer 8)
    ↓ imports from core: Key, RunId, Value, Timestamp (but uses u64 internally)
    ↓ exports: WAL (legacy), WalWriter (modern), RunBundle, RecoveryCoordinator

strata-concurrency (Layer 6)
    ↓ imports from core: Key, RunId, Value, traits
    ↓ imports from storage: UnifiedStore (tests), ShardedStore (recovery output)
    ↓ imports from durability: WAL, WALEntry (LEGACY ONLY)
    ↓ exports: TransactionContext, TransactionManager, ClonedSnapshotView
```

---

## Recommendations

### Immediate Fixes (Low Risk)

1. **Remove `placeholder()` function** from core/lib.rs
2. **Fix recovery.rs timestamp bug** - convert to milliseconds, not seconds
3. **Document primitive ID 5 gap** - add comment explaining why skipped
4. **Fix ARCHITECTURE.md** - correct State/Event WAL entry type ranges
5. **Fix DurabilityMode docs** - remove Async mention, clarify None vs InMemory

### Short-Term Improvements

6. **Add `#[from]` attributes** to error enums to preserve error chain
7. **Create timestamp unit newtype wrappers** - `Millis(i64)`, `Micros(u64)`
8. **Add per-run commit locks** - allow parallel commits for different runs
9. **Make TTLCleaner generic** over Storage trait instead of UnifiedStore

### Long-Term Architectural Changes

10. **Consolidate WAL systems** - choose one format and migrate:
    - Option A: Migrate everything to Modern WAL (breaking RunBundle)
    - Option B: Create adapter layer for format conversion
    - Option C: Version RunBundle format to support both

11. **Decide on Version representation**:
    - Option A: Use Version enum consistently through all layers
    - Option B: Use u64 everywhere, keep Version only at API boundary

12. **Standardize timestamp units**:
    - Pick one unit (microseconds) for all layers
    - Use Timestamp type everywhere instead of raw i64/u64

13. **Transaction ID unification**:
    - Migrate from u64 counter to UUID-based TxId
    - Or decide u64 is sufficient and remove TxId type

---

## Summary Table

| Issue | Severity | Layers Affected | Fix Complexity |
|-------|----------|-----------------|----------------|
| 3 WAL systems | CRITICAL | durability, concurrency, storage | HIGH |
| Version enum unused | HIGH | all 4 | MEDIUM |
| Timestamp unit mismatch | HIGH | all 4 | MEDIUM |
| Error info loss | MEDIUM-HIGH | storage, durability, concurrency | MEDIUM |
| Unused core exports | MEDIUM | core | LOW |
| Snapshot impl mismatch | MEDIUM | concurrency, storage | MEDIUM |
| Single commit lock | MEDIUM | concurrency | MEDIUM |
| Storage trait gaps | MEDIUM | storage | MEDIUM |
| Primitive ID gap | LOW | storage, durability | LOW (docs) |
| WAL entry range docs | LOW | storage | LOW (docs) |
| DurabilityMode docs | LOW | durability | LOW (docs) |
