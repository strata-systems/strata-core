# Expert Code Review: strata-storage

## Summary

Thorough expert review of the `strata-storage` crate looking for inconsistencies, dead code, bad practices, architectural gaps, and potential bugs.

---

## Critical Architectural Issues

### 1. Three Competing WAL Systems

**Location**: `storage/wal/` directory vs `durability/wal.rs` vs `durability/wal_types.rs`

The codebase has **three different WAL implementations**:

| System | Location | Format | Usage |
|--------|----------|--------|-------|
| Storage WAL | `crates/storage/src/wal/` | `WalRecord` with CRC32 | Used by storage layer |
| Legacy WAL | `crates/durability/src/wal.rs` | `WALEntry` enum (bincode) | 821 occurrences across 23 files |
| Modern WAL | `crates/durability/src/wal_types.rs` | `WalEntry` struct with CRC32 | Internal to durability crate |

**Impact**:
- Different serialization formats (bincode vs custom length-prefixed)
- Different type hierarchies
- Confusion about which to use
- Redundant code maintenance

### 2. UnifiedStore vs ShardedStore Inconsistent APIs

**Location**: `unified.rs` vs `sharded.rs`

Both stores implement the `Storage` trait, but have different secondary index support:

| Feature | UnifiedStore | ShardedStore |
|---------|-------------|--------------|
| Run Index | Yes (Built-in) | No secondary index |
| Type Index | Yes (Built-in) | No secondary index |
| TTL Index | Yes (Built-in) | No secondary index |
| `scan_by_type()` | O(type size) | Not implemented |
| `find_expired_keys()` | O(expired) | Not implemented |

**Impact**: TTLCleaner only works with UnifiedStore (line 21 in `cleaner.rs`). ShardedStore is the "performance" backend but lacks important functionality.

### 3. ShardedSnapshot Copy-on-Read Cache Unbounded Growth

**Location**: `sharded.rs:723`

```rust
cache: parking_lot::RwLock<FxHashMap<Key, Option<VersionedValue>>>,
```

The cache grows unbounded during snapshot lifetime. For long-running transactions accessing many keys, this can cause memory exhaustion.

**Impact**: Potential OOM for transactions that scan large portions of the store.

---

## Moderate Issues

### 4. WAL Entry Type Ranges (Verified Correct in Code)

**Location**: `primitive_ext.rs:94-107`

The **actual WAL entry type ranges** in code are:

| Primitive | Range | Status |
|-----------|-------|--------|
| Core | 0x00-0x0F | FROZEN |
| KV | 0x10-0x1F | FROZEN |
| JSON | 0x20-0x2F | FROZEN |
| Event | 0x30-0x3F | FROZEN |
| State | 0x40-0x4F | FROZEN |
| (Reserved) | 0x50-0x5F | Was Trace (deprecated) |
| Run | 0x60-0x6F | FROZEN |
| Vector | 0x70-0x7F | RESERVED |

**Note**: Any documentation (e.g., old ARCHITECTURE.md) showing different ranges is outdated. The code in `primitive_ext.rs` is the source of truth.

### 5. Missing primitive_type_id #5

**Location**: `primitive_ext.rs:174-187`

```rust
pub const KV: u8 = 1;
pub const JSON: u8 = 2;
pub const EVENT: u8 = 3;
pub const STATE: u8 = 4;
// ID 5 is skipped!
pub const RUN: u8 = 6;
pub const VECTOR: u8 = 7;
```

**Impact**: Minor - ID 5 is unused, creating a gap in the sequence.

### 6. DurabilityMode Doc Comment Inconsistency

**Location**: `wal/mod.rs:8` vs `wal/durability.rs:17-43`

The module doc mentions 4 durability modes: "InMemory, Batched, Strict, Async"

But `DurabilityMode` enum only has 3:
- `None` (called InMemory in docs)
- `Strict`
- `Batched`

**Impact**: Documentation mismatch - `Async` doesn't exist, and `None` is called "InMemory" in some places.

### 7. Dead Code: `WalReader::codec` Field

**Location**: `wal/reader.rs:18-19`

```rust
#[allow(dead_code)]
codec: Box<dyn StorageCodec>,
```

The codec is stored but never used - the reader always uses raw bytes.

**Impact**: Codec-aware decoding is not actually implemented.

---

## Minor Issues & Potential Bugs

### 8. ShardedSnapshot Clone Clones Cache Contents

**Location**: `sharded.rs:726-735`

```rust
impl Clone for ShardedSnapshot {
    fn clone(&self) -> Self {
        Self {
            // ...
            cache: parking_lot::RwLock::new(self.cache.read().clone()),
        }
    }
}
```

Cloning a snapshot also clones its entire cache. This could be surprising - the clone shares no isolation from the original's cached reads.

**Impact**: Unexpected memory usage when cloning snapshots.

### 9. Version Overflow Potential

**Location**: `sharded.rs:269-277`

```rust
pub fn next_version(&self) -> u64 {
    self.version
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| {
            Some(v.wrapping_add(1))
        })
        .unwrap()
        .wrapping_add(1)
}
```

Uses `wrapping_add` which will overflow to 0 at `u64::MAX`. While practically unlikely (~584 years at 1B/sec), this could cause version comparison issues.

**Impact**: Low - version 0 would be less than all existing versions, causing incorrect MVCC behavior.

### 10. Segment Number Parsing Assumes 6-Digit Format

**Location**: `wal/writer.rs:273-275`, `wal/reader.rs:176-178`

```rust
// Extract segment number from "wal-NNNNNN.seg"
let num_str = &name[4..10];
num_str.parse::<u64>().ok()
```

This hardcodes 6-digit segment numbers. Segments > 999999 will fail to parse.

**Impact**: WAL segment limit of 999,999 before failures.

### 11. ShardedStore::list_* Methods Don't Respect Version

**Location**: `sharded.rs:478-567`

The `list_run`, `list_by_prefix`, and `list_by_type` methods always return the latest version, ignoring MVCC:

```rust
chain.latest().map(|sv| (k.clone(), sv.versioned().clone()))
```

But `Storage` trait `scan_*` methods properly filter by version.

**Impact**: Inconsistent snapshot isolation when using list methods directly on `ShardedStore` vs through `Storage` trait.

### 12. TTLCleaner Only Works with UnifiedStore

**Location**: `cleaner.rs:21`

```rust
use crate::UnifiedStore;
```

TTLCleaner is hardcoded to UnifiedStore, not the `Storage` trait.

**Impact**: Cannot use TTL cleanup with ShardedStore.

---

## Recommendations

### Immediate Fixes (Low Risk)

1. Fix documentation: Remove "Async" from `wal/mod.rs` comment, or rename `None` consistently
2. Remove `#[allow(dead_code)]` and implement codec-aware decoding or remove the field
3. Document the primitive_type_id gap (ID 5) or fill it
4. Unify WAL entry type ranges documentation

### Short-Term Improvements

5. Add secondary indices to ShardedStore (RunIndex, TypeIndex, TTLIndex)
6. Add cache size limit to ShardedSnapshot (evict LRU entries)
7. Make TTLCleaner generic over `Storage` trait
8. Add max segment number check (or switch to variable-width formatting)

### Long-Term Architectural Changes

9. **Consolidate WAL systems**: Choose one WAL format and migrate. The storage crate's `WalRecord` has CRC32 validation and is cleaner than the durability crate's bincode-based `WALEntry`.
10. Add proper version overflow handling with monotonic guarantees
11. Consider deprecating `list_*` methods in favor of `scan_*` for consistency

---

## Test Coverage Assessment

The crate has **extensive test coverage**:
- Each module has comprehensive unit tests
- Edge cases (empty stores, concurrent access, TTL expiration) are tested
- Recovery scenarios are well-tested
- Crash/corruption testing exists in `testing/` module

This is a **positive sign** for code quality.

---

## Overall Assessment

**Rating**: Good with notable architectural debt

The `strata-storage` crate is well-implemented with clear separation between in-memory and disk storage. The main issues are:

1. **Three WAL systems** - significant technical debt that should be consolidated
2. **Feature parity gap** - ShardedStore lacks indices that UnifiedStore has
3. **Unbounded snapshot cache** - potential memory issue for long transactions

The code follows good Rust practices with comprehensive error handling, proper use of `parking_lot` for performance, and thorough documentation. The test coverage is excellent.
