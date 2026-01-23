# Foundational Capabilities Audit

> **Status**: Gap Analysis
> **Date**: 2026-01-22
> **Scope**: All 7 Primitives vs Architectural Requirements

---

## Executive Summary

This audit cross-references the **7 Invariants** from `PRIMITIVE_CONTRACT.md`, the **Substrate API requirements** from `M11_ARCHITECTURE.md`, and the **actual implementations** to identify gaps in foundational capability support.

**Critical Finding**: Multiple primitives have stubbed or incomplete implementations for core capabilities that are architecturally mandated. The most significant gaps are in **history/versioning APIs** and **transaction support**.

---

## Foundational Capabilities Matrix

### The 7 Invariants (PRIMITIVE_CONTRACT.md)

| # | Invariant | Description |
|---|-----------|-------------|
| I1 | **Addressable** | Every entity has a stable identity (run + key/id) |
| I2 | **Versioned** | Every mutation produces a version; reads include version info |
| I3 | **Transactional** | All primitives participate in transactions the same way |
| I4 | **Lifecycle** | Create/Exist/Evolve/Destroy pattern |
| I5 | **Run-scoped** | Everything belongs to exactly one run |
| I6 | **Introspectable** | Can check existence, current state, and version |
| I7 | **Consistent R/W** | Reads don't modify; writes produce versions |

### Required Substrate Capabilities (M11_ARCHITECTURE.md)

| # | Capability | Description |
|---|------------|-------------|
| C1 | **Versioned<T> Returns** | All reads return `Versioned<T>` (value + version + timestamp) |
| C2 | **Version Returns** | All writes return `Version` |
| C3 | **Explicit Run** | All operations require explicit `run_id` |
| C4 | **History Access** | `*_history()` methods for version history |
| C5 | **Point-in-Time Read** | `*_get_at(version)` for historical reads |
| C6 | **Retention Policy** | Per-run retention configuration |
| C7 | **Replay Support** | Deterministic replay from EventLog (P1-P6 invariants) |

---

## Per-Primitive Gap Analysis

### 1. KVStore

**Trait Location**: `crates/api/src/substrate/kv.rs`

| Capability | Required | Implemented | Status | Notes |
|------------|----------|-------------|--------|-------|
| I1 Addressable | run + key | Yes | OK | `EntityRef::Kv` |
| I2 Versioned | All reads | Yes | OK | Returns `Versioned<Value>` |
| I3 Transactional | Cross-primitive | Partial | **GAP** | Single-primitive only |
| I4 Lifecycle | CRUD | Yes | OK | put/get/delete/exists |
| I5 Run-scoped | Explicit run | Yes | OK | `ApiRunId` required |
| I6 Introspectable | exists/get/version | Yes | OK | All present |
| I7 Consistent R/W | Pure reads | Yes | OK | Clear separation |
| C1 Versioned<T> | kv_get | Yes | OK | Returns `Versioned<Value>` |
| C2 Version Returns | kv_put | Yes | OK | Returns `Version` |
| C4 History Access | kv_history | **STUBBED** | **P0** | Returns `vec![]` at line 355 |
| C5 Point-in-Time | kv_get_at | **STUBBED** | **P0** | Only returns current, line 321-335 |
| C6 Retention | run-level | No | **P1** | Not integrated |

**KVStore Gaps Summary**:
- `kv_history()` - STUBBED, returns empty vector
- `kv_get_at()` - STUBBED, only checks current version
- `kv_incr()` - Uses unchecked arithmetic (overflow bug, Issue 699)
- Key validation - Missing (empty, NUL, reserved prefix accepted)

---

### 2. EventLog

**Trait Location**: `crates/api/src/substrate/event.rs`

| Capability | Required | Implemented | Status | Notes |
|------------|----------|-------------|--------|-------|
| I1 Addressable | run + stream + seq | Yes | OK | `EntityRef::Event` |
| I2 Versioned | All reads | Yes | OK | Returns `Versioned<Value>` with `Version::Sequence` |
| I3 Transactional | Cross-primitive | **Unknown** | **AUDIT** | Needs verification |
| I4 Lifecycle | CR (immutable) | Yes | OK | append/read (no update/delete) |
| I5 Run-scoped | Explicit run | Yes | OK | `ApiRunId` required |
| I6 Introspectable | exists/get/len | Yes | OK | event_get, event_len |
| I7 Consistent R/W | Pure reads | Yes | OK | Append-only model |
| C1 Versioned<T> | event_get | Yes | OK | Returns `Versioned<Value>` |
| C2 Version Returns | event_append | Yes | OK | Returns `Version::Sequence` |
| C4 History Access | N/A | N/A | OK | Append-only (range is history) |
| C5 Point-in-Time | event_get(seq) | Yes | OK | Direct sequence access |
| C6 Retention | run-level | No | **P1** | Not integrated |

**EventLog Gaps Summary**:
- Transaction integration needs audit
- Retention policy not integrated
- No explicit stream listing API (`event_streams()`)

---

### 3. StateCell

**Trait Location**: `crates/api/src/substrate/state.rs`

| Capability | Required | Implemented | Status | Notes |
|------------|----------|-------------|--------|-------|
| I1 Addressable | run + cell name | Yes | OK | `EntityRef::State` |
| I2 Versioned | All reads | Yes | OK | Returns `Versioned<Value>` with `Version::Counter` |
| I3 Transactional | Cross-primitive | **Unknown** | **AUDIT** | Needs verification |
| I4 Lifecycle | CRUD | Yes | OK | set/get/delete/exists |
| I5 Run-scoped | Explicit run | Yes | OK | `ApiRunId` required |
| I6 Introspectable | exists/get/version | Yes | OK | All present |
| I7 Consistent R/W | Pure reads | Yes | OK | Clear separation |
| C1 Versioned<T> | state_get | Yes | OK | Returns `Versioned<Value>` |
| C2 Version Returns | state_set | Yes | OK | Returns `Version::Counter` |
| C4 History Access | state_history | **STUBBED** | **P0** | Returns `vec![]` at line 218-221 |
| C5 Point-in-Time | N/A | N/A | OK | Counter semantics, not temporal |
| C6 Retention | run-level | No | **P1** | Not integrated |

**StateCell Gaps Summary**:
- `state_history()` - STUBBED, returns empty vector
- Transaction integration needs audit
- No cell listing API (`state_cells()`)

---

### 4. JsonStore

**Trait Location**: `crates/api/src/substrate/json.rs`

| Capability | Required | Implemented | Status | Notes |
|------------|----------|-------------|--------|-------|
| I1 Addressable | run + doc_id | Yes | OK | `EntityRef::Json` |
| I2 Versioned | All reads | Yes | OK | Returns `Versioned<Value>` |
| I3 Transactional | Cross-primitive | **Unknown** | **AUDIT** | Needs verification |
| I4 Lifecycle | CRUD | Yes | OK | set/get/delete |
| I5 Run-scoped | Explicit run | Yes | OK | `ApiRunId` required |
| I6 Introspectable | exists/get/version | Partial | **GAP** | No explicit `json_exists()` |
| I7 Consistent R/W | Pure reads | Yes | OK | Clear separation |
| C1 Versioned<T> | json_get | Yes | OK | Returns `Versioned<Value>` |
| C2 Version Returns | json_set | Yes | OK | Returns `Version` |
| C4 History Access | json_history | **STUBBED** | **P0** | Returns `vec![]` at line 308-310 |
| C5 Point-in-Time | json_get_at | **MISSING** | **P0** | Not in trait |
| C6 Retention | run-level | No | **P1** | Not integrated |

**JsonStore Gaps Summary**:
- `json_history()` - STUBBED, returns empty vector
- `json_get_at()` - MISSING from trait entirely
- `json_exists()` - MISSING from trait
- No document listing API (`json_keys()`)
- Transaction integration needs audit

---

### 5. VectorStore

**Trait Location**: `crates/api/src/substrate/vector.rs`

| Capability | Required | Implemented | Status | Notes |
|------------|----------|-------------|--------|-------|
| I1 Addressable | run + collection + key | Yes | OK | `EntityRef::Vector` |
| I2 Versioned | All reads | Partial | **GAP** | Returns `Version::Txn(0)` hardcoded, lines 294, 315, 355, 386 |
| I3 Transactional | Cross-primitive | **Unknown** | **AUDIT** | Needs verification |
| I4 Lifecycle | CRUD | Yes | OK | upsert/get/delete |
| I5 Run-scoped | Explicit run | Yes | OK | `ApiRunId` required |
| I6 Introspectable | exists/get/version | Partial | **GAP** | No `vector_exists()`, version always 0 |
| I7 Consistent R/W | Pure reads | Yes | OK | Clear separation |
| C1 Versioned<T> | vector_get | Partial | **GAP** | Version is hardcoded 0 |
| C2 Version Returns | vector_upsert | Partial | **GAP** | Returns `Version::Txn(0)` always |
| C4 History Access | vector_history | **MISSING** | **P0** | Not in trait |
| C5 Point-in-Time | vector_get_at | **MISSING** | **P0** | Not in trait |
| C6 Retention | run-level | No | **P1** | Not integrated |

**VectorStore Gaps Summary**:
- **Version always returns `Version::Txn(0)`** - violates I2
- `vector_history()` - MISSING from trait
- `vector_get_at()` - MISSING from trait
- `vector_exists()` - MISSING from trait
- Search doesn't return vector data (line 343: `vector: vec![]`)
- Transaction integration needs audit

---

### 6. TraceStore

**Trait Location**: `crates/api/src/substrate/trace.rs`

| Capability | Required | Implemented | Status | Notes |
|------------|----------|-------------|--------|-------|
| I1 Addressable | run + trace_id | Yes | OK | `EntityRef::Trace` |
| I2 Versioned | All reads | Partial | **GAP** | Returns `Version::Txn(0)` hardcoded, lines 320, 339, 359 |
| I3 Transactional | Cross-primitive | **Unknown** | **AUDIT** | Needs verification |
| I4 Lifecycle | CR (immutable) | Yes | OK | create/get (no update/delete) |
| I5 Run-scoped | Explicit run | Yes | OK | `ApiRunId` required |
| I6 Introspectable | exists/get/version | Partial | **GAP** | No `trace_exists()`, version always 0 |
| I7 Consistent R/W | Pure reads | Yes | OK | Append-only model |
| C1 Versioned<T> | trace_get | Partial | **GAP** | Version is hardcoded 0 |
| C2 Version Returns | trace_create | Partial | **GAP** | Returns version but query loses it |
| C4 History Access | N/A | N/A | OK | Append-only (list is history) |
| C5 Point-in-Time | N/A | N/A | OK | Immutable traces |
| C6 Retention | run-level | No | **P1** | Not integrated |

**TraceStore Gaps Summary**:
- **Version info lost in queries** - `trace_list`, `trace_children`, `trace_tree` return `Version::Txn(0)`
- `trace_create_with_id()` - STUBBED, returns error (line 258-264)
- `trace_update_tags()` - STUBBED, returns error (line 374-380)
- `trace_exists()` - MISSING from trait
- Transaction integration needs audit

---

### 7. RunIndex

**Trait Location**: `crates/api/src/substrate/run.rs`

| Capability | Required | Implemented | Status | Notes |
|------------|----------|-------------|--------|-------|
| I1 Addressable | run_id | Yes | OK | `EntityRef::Run` |
| I2 Versioned | All reads | Partial | **GAP** | Returns `Version::Txn(0)` hardcoded, lines 199, 266, 282 |
| I3 Transactional | N/A | N/A | OK | Run management is meta |
| I4 Lifecycle | CRUD | Yes | OK | create/get/close |
| I5 Run-scoped | N/A (meta) | N/A | OK | RunIndex is the exception |
| I6 Introspectable | exists/get/state | Yes | OK | run_exists, run_get |
| I7 Consistent R/W | Pure reads | Yes | OK | Clear separation |
| C1 Versioned<T> | run_get | Partial | **GAP** | Version is hardcoded 0 |
| C2 Version Returns | run_create | Partial | **GAP** | Version always 0 |
| C4 History Access | run_history | **MISSING** | **P1** | Not in trait (runs are mutable) |
| C6 Retention | run_set/get_retention | **STUBBED** | **P0** | No-op implementations, lines 296-303 |

**RunIndex Gaps Summary**:
- **Version always returns `Version::Txn(0)`** - violates I2
- `run_set_retention()` - STUBBED, no-op (returns 0)
- `run_get_retention()` - STUBBED, always returns KeepAll
- Run history API missing (could track state transitions)
- Run deletion API missing (deferred to GC milestone per M11)

---

## Cross-Cutting Gaps

### 1. Transaction Control (CRITICAL)

**Location**: `crates/api/src/substrate/transaction.rs`

**Status**: Trait defined but **NOT IMPLEMENTED** on SubstrateImpl

The `TransactionControl` trait is defined but there's no implementation for `SubstrateImpl`. This means:
- No explicit `txn_begin()` / `txn_commit()` / `txn_rollback()`
- Cross-primitive transactions may not work
- Invariant I3 (Everything is Transactional) is potentially violated

**Priority**: P0 - This is the foundation for atomic operations

### 2. Retention System (CRITICAL)

**Location**: `crates/api/src/substrate/retention.rs`

**Status**: Completely STUBBED

```rust
fn retention_get(&self, _run: &ApiRunId) -> StrataResult<Option<RetentionVersion>> {
    Ok(None)
}
fn retention_set(&self, _run: &ApiRunId, _policy: RetentionPolicy) -> StrataResult<u64> {
    Ok(0)
}
```

Without retention:
- History cannot be garbage collected
- `HistoryTrimmed` error is never returned correctly
- Memory/disk usage grows unbounded

### 3. Key/Input Validation (HIGH)

From KV test suite (documented in `docs/defects/KV_DEFECTS.md`):
- Empty keys accepted (should reject)
- NUL bytes in keys accepted (should reject)
- `_strata/` reserved prefix accepted (should reject)

This likely affects ALL primitives, not just KV.

### 4. Version Tracking

Multiple primitives return hardcoded `Version::Txn(0)`:
- VectorStore: upsert, get, search, create_collection
- TraceStore: create (lost in list/children/tree queries)
- RunIndex: create, get, close

This violates Invariant I2 (Everything is Versioned) and makes history tracking impossible.

---

## Summary by Severity

### P0 - Critical (Violates Core Invariants)

| # | Issue | Primitives | Invariant Violated |
|---|-------|------------|-------------------|
| 1 | `*_history()` STUBBED | KV, StateCell, JsonStore | I2 (Versioned) |
| 2 | `*_get_at()` STUBBED/MISSING | KV, JsonStore, VectorStore | I2 (Versioned) |
| 3 | Version always 0 | VectorStore, TraceStore, RunIndex | I2 (Versioned) |
| 4 | TransactionControl not implemented | All | I3 (Transactional) |
| 5 | RetentionSubstrate STUBBED | All | C6 (Retention) |

### P1 - High (Missing Table-Stakes Features)

| # | Issue | Primitives |
|---|-------|------------|
| 1 | Key/input validation | KV (tested), likely all |
| 2 | Missing `*_exists()` | JsonStore, VectorStore, TraceStore |
| 3 | Missing listing APIs | StateCell, JsonStore, EventLog, VectorStore |
| 4 | `trace_create_with_id()` STUBBED | TraceStore |
| 5 | `trace_update_tags()` STUBBED | TraceStore |
| 6 | `kv_scan` / `kv_keys` MISSING | KVStore |

### P2 - Medium (Implementation Quality)

| # | Issue | Primitives |
|---|-------|------------|
| 1 | Search doesn't return vector data | VectorStore |
| 2 | WriteConflict handling | KV (concurrent operations) |
| 3 | Overflow in `kv_incr` | KVStore |

---

## Recommended Fix Order

### Phase 1: Foundation (Unblock Everything)
1. Implement proper version tracking across all primitives
2. Implement TransactionControl on SubstrateImpl
3. Implement RetentionSubstrate basics

### Phase 2: History APIs (Core Value Proposition)
1. Implement `kv_history()` properly
2. Implement `kv_get_at()` properly
3. Add `json_history()`, `json_get_at()`
4. Add `vector_history()`, `vector_get_at()`
5. Add `state_history()` properly

### Phase 3: Validation & Table Stakes
1. Key validation across all primitives
2. Add missing `*_exists()` methods
3. Add listing APIs (`kv_keys`, `state_cells`, etc.)
4. Fix VectorStore search to return vector data

### Phase 4: Advanced Features
1. Complete TraceStore (`trace_create_with_id`, `trace_update_tags`)
2. Implement retention garbage collection
3. Cross-primitive transaction testing

---

## Appendix: Architecture Document References

- `PRIMITIVE_CONTRACT.md` - 7 Invariants definition
- `CORE_API_SHAPE.md` - API shape patterns
- `M11_ARCHITECTURE.md` - Substrate API contract, Version model, Error model
- `DURABILITY_REPLAY_CONTRACT.md` - R1-R6 (recovery), P1-P6 (replay)
- `translations/KVSTORE_TRANSLATION.md` - KVStore gap analysis pattern

---

## Document History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-01-22 | Initial audit of all 7 primitives |
