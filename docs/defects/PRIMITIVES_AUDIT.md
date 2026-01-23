# Primitives Audit Report

> Comprehensive audit of all 7 primitives in the Strata storage system.
> Date: 2026-01-22

## Primitives Overview

| # | Primitive | Location | Status | Defects |
|---|-----------|----------|--------|---------|
| 1 | **KVStore** | `crates/primitives/src/kv.rs` | Mature | 14 issues (see [KV_DEFECTS.md](./KV_DEFECTS.md)) |
| 2 | **EventLog** | `crates/primitives/src/event_log.rs` | Complete | 0 |
| 3 | **StateCell** | `crates/primitives/src/state_cell.rs` | Complete | 0 |
| 4 | **JsonStore** | `crates/primitives/src/json_store.rs` | Complete | 0 |
| 5 | **VectorStore** | `crates/primitives/src/vector/store.rs` | Complete | 2 minor |
| 6 | **TraceStore** | `crates/primitives/src/trace.rs` | Complete | 0 |
| 7 | **RunIndex** | `crates/primitives/src/run_index.rs` | Complete | 0 |

---

## Summary by Category

| Category | KV | Event | State | JSON | Vector | Trace | Run |
|----------|:--:|:-----:|:-----:|:----:|:------:|:-----:|:---:|
| CRUD Operations | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Batch Operations | ✓ | ✓* | - | - | - | - | - |
| Versioned Returns | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Fast Path Reads | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | - |
| Search API (M6) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Run Isolation | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | N/A |
| Extension Trait | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | - |
| Transaction Support | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| WAL Recovery | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Key Validation | **✗** | - | - | ✓ | ✓ | - | - |
| Scan/List Operations | **✗** | ✓ | ✓ | - | ✓ | ✓ | ✓ |
| Version History | **✗** | N/A | N/A | - | - | - | - |

Legend: ✓ = Implemented, ✗ = Missing/Buggy, - = Not Applicable, * = Via range read

---

## 1. KVStore (14 Defects)

**Status:** Mature but with significant gaps

See **[KV_DEFECTS.md](./KV_DEFECTS.md)** for full details.

### Defect Summary

| Priority | Count | Category |
|----------|-------|----------|
| P0 | 3 | Key validation bugs (empty, NUL, reserved prefix) |
| P0 | 1 | Overflow panic (Issue #699) |
| P0 | 2 | Stubbed history APIs (`kv_get_at`, `kv_history`) |
| P0 | 2 | Missing scan operations (`kv_keys`, `kv_scan`) |
| P1 | 4 | WriteConflict on concurrent atomic ops |
| P1 | 2 | Potential CAS issues |

### Test Coverage

```
Total: 163 tests
Passed: 138
Failed: 10
Ignored: 15 (14 scan tests + 1 overflow test)
```

---

## 2. EventLog (0 Defects)

**Status:** Complete and well-tested

### Features
- Append-only immutable event stream
- Hash chain verification (tamper evidence)
- Single-writer-ordered semantics
- Query by type, read range, head access
- Fast path reads via SnapshotView

### Architecture Highlights
- `EventLogMeta` tracks `next_sequence` and `head_hash`
- Hash chain uses `DefaultHasher` padded to 32 bytes (future SHA-256 path)
- High retry count (200) for contention scenarios

### Test Coverage
- Core structure tests
- Append/read operations
- Chain verification
- Query by type
- Extension trait (cross-primitive transactions)
- Fast path tests

---

## 3. StateCell (0 Defects)

**Status:** Complete and well-tested

### Features
- CAS-based versioned cells for coordination
- Pure transition closure pattern
- Version monotonicity guarantees
- List/exists operations

### Architecture Highlights
- `State` struct with value, version, `updated_at`
- `Version::Counter` type for versioning
- `transition()` with automatic retry on conflict
- Purity requirement enforced via documentation

### Test Coverage
- Init/read/delete operations
- CAS success and conflict
- Transition patterns
- Run isolation
- Fast path tests
- Versioned returns (M9)

---

## 4. JsonStore (0 Defects)

**Status:** Complete and well-tested

### Features
- JSON document storage with path-based access
- MessagePack serialization for efficiency
- Document versioning
- Limit validation (Issue #440)

### Architecture Highlights
- `JsonDoc` struct with id, value, version, timestamps
- Path semantics via `JsonPath`
- Stateless facade pattern
- Automatic intermediate object creation on set

### Test Coverage
- Document CRUD
- Nested path operations
- Array element access
- Run isolation
- Limit validation

---

## 5. VectorStore (2 Minor Issues)

**Status:** Complete with minor gaps

### Features
- Vector storage and similarity search
- Multiple distance metrics (Cosine, Euclidean, DotProduct)
- Collection management
- Metadata filtering (partial)
- WAL recovery

### Architecture Highlights
- Stateless facade over `VectorBackendState`
- BruteForce backend (M8, configurable in M9)
- BTreeMap for deterministic iteration (Invariant R3)
- Extension mechanism for shared state

### Minor Issues

#### 5.1 Search Filter Not Fully Wired

**Location:** `crates/api/src/substrate/vector.rs:334`

```rust
fn vector_search(..., _filter: Option<SearchFilter>, ...) {
    // Note: The primitive's search() takes an optional MetadataFilter as 5th arg
    let results = self.vector().search(run_id, collection, query, k as usize, None)
```

**Issue:** `SearchFilter` parameter is accepted but not passed through to primitive.

**Impact:** Low - metadata filtering works at primitive level but not at Substrate API level.

**Fix:** Wire `SearchFilter` conversion to `MetadataFilter`.

---

#### 5.2 Distance Metric Not Configurable Per-Search

**Location:** `crates/api/src/substrate/vector.rs:334`

```rust
fn vector_search(..., _metric: Option<DistanceMetric>, ...) {
```

**Issue:** `metric` parameter is accepted but ignored; collection default is always used.

**Impact:** Low - most use cases use collection default.

**Fix:** Pass metric to primitive search method.

---

## 6. TraceStore (0 Defects)

**Status:** Complete and well-tested

### Features
- Structured reasoning traces
- Parent-child relationships (tree structure)
- Secondary indices (by-type, by-tag, by-parent, by-time)
- Tree reconstruction
- Performance warning documented

### Architecture Highlights
- `Trace` struct with rich `TraceType` enum
- 3-4 index entries per trace (write amplification)
- Hour-bucket time index for range queries
- Designed for debuggability, not high-volume telemetry

### Test Coverage
- Record and get operations
- Child trace handling
- Index queries (type, tag, time)
- Tree reconstruction
- Extension trait
- Fast path tests

---

## 7. RunIndex (0 Defects)

**Status:** Complete and well-tested

### Features
- First-class run lifecycle management
- Status transitions with validation
- Cascading delete
- Secondary indices (by-status, by-tag, by-parent)
- Soft archive vs hard delete

### Architecture Highlights
- Global namespace (not run-scoped)
- `RunMetadata` with name, status, timestamps, tags
- Valid transitions enforced (no resurrection)
- Archived is terminal state

### Test Coverage
- Create/get operations
- Status transitions (complete, fail, pause, resume, cancel)
- Query operations (by-status, by-tag, children)
- Delete and archive
- Integration tests with all primitives

---

## Recommendations

### Immediate (P0)

1. **Fix KVStore key validation** - Add validation for empty keys, NUL bytes, and reserved prefix
2. **Fix `kv_incr` overflow** - Use `checked_add()` instead of `+` operator
3. **Implement `kv_get_at`** - Storage layer already supports this
4. **Implement `kv_history`** - Needs `VersionChain` iteration exposure
5. **Add `kv_keys`/`kv_scan`** - Table stakes for any KV store

### Short-term (P1)

6. **Wire VectorStore filter** - Pass `SearchFilter` through to primitive
7. **Wire VectorStore metric** - Pass metric option to primitive
8. **Investigate WriteConflict issues** - Analyze transaction layer for atomic ops

### Long-term

9. **Add comprehensive test suites** for other primitives (following KV pattern)
10. **Document cross-primitive transaction patterns**
11. **Add stress tests for all primitives**

---

## Cross-Primitive Patterns

All primitives share common patterns that worked well:

1. **Stateless Facade** - Only `Arc<Database>` held, all state in storage
2. **Fast Path Reads** - Direct snapshot read bypasses transaction overhead
3. **Extension Traits** - Enable cross-primitive atomic operations
4. **Versioned Returns** - M9 spec compliance
5. **Search API** - M6 integration for cross-primitive search
6. **Run Isolation** - Namespace-based separation

---

## Test Suite Status by Primitive

| Primitive | Unit Tests | Integration | Stress | Total |
|-----------|:----------:|:-----------:|:------:|:-----:|
| KVStore | 163 | ✓ | ✓ | 163 |
| EventLog | ~40 | ✓ | - | ~40 |
| StateCell | ~40 | ✓ | - | ~40 |
| JsonStore | ~50 | ✓ | - | ~50 |
| VectorStore | ~30 | ✓ | - | ~30 |
| TraceStore | ~40 | ✓ | - | ~40 |
| RunIndex | ~50 | ✓ | - | ~50 |

Note: KVStore test suite is the most comprehensive. Consider creating similar comprehensive suites for other primitives.
