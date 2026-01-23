# KVStore Defects and Gaps

> Consolidated from test suite analysis and architecture review.
> Source: `tests/substrate_api_comprehensive/` and `docs/architecture/translations/KVSTORE_TRANSLATION.md`

## Summary

| Category | Count | Priority |
|----------|-------|----------|
| Implementation Bugs | 3 | P0 |
| Concurrency Issues | 4 | P1 |
| Missing Core Features | 5 | P0-P1 |
| Missing Table Stakes | 2 | P0 |
| **Total** | **14** | |

---

## P0: Implementation Bugs (3)

### 1. Empty Key Not Rejected

**Test:** `kv_edge_cases::test_key_empty_rejected`

**Expected:** `kv_put(&run, "", value)` should return `InvalidKey` error

**Actual:** Operation succeeds

**Contract Reference:** Section 11.3 - "Keys must be non-empty"

**Fix Location:** Key validation in `SubstrateImpl::kv_put()` or primitive layer

---

### 2. NUL Byte in Key Not Rejected

**Test:** `kv_edge_cases::test_key_nul_byte_rejected`

**Expected:** `kv_put(&run, "key\0with\0nuls", value)` should return `InvalidKey` error

**Actual:** Operation succeeds

**Contract Reference:** Section 11.3 - "Keys must have no NUL bytes"

**Fix Location:** Key validation in `SubstrateImpl::kv_put()` or primitive layer

---

### 3. Reserved Prefix Not Rejected

**Test:** `kv_edge_cases::test_key_reserved_prefix_rejected`

**Expected:** `kv_put(&run, "_strata/internal", value)` should return `InvalidKey` error

**Actual:** Operation succeeds

**Contract Reference:** Section 11.3 - "Keys must not start with `_strata/` (reserved prefix)"

**Fix Location:** Key validation in `SubstrateImpl::kv_put()` or primitive layer

---

## P1: Concurrency/Transaction Issues (4)

### 4. WriteConflict on Concurrent Increment

**Tests:**
- `kv_concurrency::test_incr_atomic_under_concurrency`
- `kv_transactions::test_incr_atomic_isolation`

**Expected:** `kv_incr` should be truly atomic - concurrent increments should all succeed

**Actual:** Threads receive `WriteConflict` errors

**Analysis:** The `kv_incr` implementation likely does read-modify-write in a transaction that conflicts under contention. True atomic increment should use a different strategy (e.g., retry loop or lock-free increment at storage layer).

**Fix Options:**
1. Implement retry loop in substrate for `kv_incr`
2. Add lock-free increment at storage layer
3. Document that `kv_incr` may fail under contention and callers should retry

---

### 5. WriteConflict on CAS Retry Pattern

**Tests:**
- `kv_concurrency::test_cas_value_under_concurrency`
- `kv_transactions::test_cas_retry_pattern`

**Expected:** CAS operations should return `false` on conflict, not error

**Actual:** Threads receive `WriteConflict` errors, causing panics

**Analysis:** The `kv_cas_value` implementation conflicts at transaction level instead of returning `false` gracefully.

**Contract Reference:** Section 11.3 - "CAS version mismatch returns `false`, not error"

**Fix Location:** `SubstrateImpl::kv_cas_value()` transaction handling

---

## P0: Missing Core Features (5)

### 6. `kv_get_at` Not Implemented

**Tests:**
- `kv_recovery_invariants::test_version_history_get_at`
- `kv_recovery_invariants::test_version_history_survives_crash`

**Expected:** `kv_get_at(&run, key, version)` returns value at specific version

**Actual:** Returns current value or fails (implementation is stubbed)

**Contract Reference:** Section 11.3 - "kv_get_at(run, key, version) -> Versioned<Value> | HistoryTrimmed"

**Architecture Note:** Storage layer HAS this capability:
```rust
// In VersionChain (storage/src/sharded.rs)
pub fn get_at_version(&self, max_version: u64) -> Option<&StoredValue>

// In Storage trait (core/src/traits.rs)
fn get_versioned(&self, key: &Key, max_version: u64) -> Result<Option<VersionedValue>>
```

**Fix:** Primitive must expose `get_at_version` to substrate

---

### 7. `kv_history` Not Implemented

**Test:** `kv_recovery_invariants::test_version_history_enumeration`

**Expected:** `kv_history(&run, key, limit, before)` returns list of historical versions

**Actual:** Returns empty vector (implementation is stubbed)

**Contract Reference:** Section 11.3 - "kv_history(run, key, limit?, before?) -> Vec<Versioned<Value>>"

**Architecture Note:** Storage layer has `VersionChain` which stores all versions, but doesn't expose iteration.

**Fix:**
1. Add iteration to `VersionChain`
2. Expose in primitive layer
3. Wire through substrate

---

### 8. `kv_incr` Overflow Panics Instead of Returning Error

**Test:** `kv_atomic_ops::test_incr_overflow_returns_error` (ignored)

**GitHub Issue:** #699

**Expected:** `kv_incr(&run, key, 1)` on `i64::MAX` should return `Overflow` error

**Actual:** Panics due to arithmetic overflow

**Contract Reference:** Section 11.3 - "Increment overflow returns `Overflow` error"

**Fix:** Use `checked_add()` instead of `+` operator

---

### 9. `kv_cas_version` Version Check May Be Stubbed

**Test:** `kv_atomic_ops::test_cas_version_wrong_version_behavior`

**Observation:** Test documents that `kv_cas_version` may succeed even with wrong version (stub behavior)

**Expected:** Should return `false` when version doesn't match

**Status:** Needs verification - may be working correctly in some cases

---

### 10. Batch Operations Error Handling

**Observation:** From KVSTORE_TRANSLATION.md - batch operations (mput, mdelete) need transaction-based implementation

**Status:** Working but may have edge cases with partial failures

---

## P0: Missing Table Stakes Operations (2)

### 11. `kv_keys` - List Keys by Prefix

**Tests:** 6 ignored tests in `kv_scan_ops`
- `test_kv_keys_lists_all_keys`
- `test_kv_keys_with_prefix_filter`
- `test_kv_keys_respects_limit`
- `test_kv_keys_empty_for_no_matches`
- `test_kv_keys_excludes_deleted`
- `test_kv_keys_run_isolation`

**Expected API:**
```rust
fn kv_keys(&self, run: &ApiRunId, prefix: &str, limit: usize)
    -> StrataResult<Vec<String>>;
```

**Why Table Stakes:** Cannot enumerate keys without this. Required for:
- Listing all sessions for a user (`user:123:*`)
- Admin/debugging tools
- Data export/migration

---

### 12. `kv_scan` - Paginated Key Scanning

**Tests:** 8 ignored tests in `kv_scan_ops`
- `test_kv_scan_basic`
- `test_kv_scan_with_prefix`
- `test_kv_scan_pagination`
- `test_kv_scan_consistent_order`
- `test_kv_scan_excludes_deleted`
- `test_kv_scan_run_isolation`
- `test_kv_keys_cross_mode`
- `test_kv_scan_cross_mode`

**Expected API:**
```rust
fn kv_scan(&self, run: &ApiRunId, prefix: &str, limit: usize, cursor: Option<&str>)
    -> StrataResult<ScanResult>;

struct ScanResult {
    entries: Vec<(String, Versioned<Value>)>,
    cursor: Option<String>,  // None if no more results
}
```

**Why Table Stakes:** Every major KV store has scan capability (Redis SCAN, etcd range, RocksDB iterators).

---

## Priority Matrix

| ID | Issue | Priority | Effort | Dependencies |
|----|-------|----------|--------|--------------|
| 1-3 | Key validation bugs | P0 | Low | None |
| 8 | Overflow panic | P0 | Low | None |
| 6 | `kv_get_at` stubbed | P0 | Medium | Storage already supports |
| 7 | `kv_history` stubbed | P0 | Medium | Needs VersionChain iteration |
| 11-12 | Scan operations | P0 | Medium | Storage layer changes |
| 4-5 | WriteConflict issues | P1 | High | Transaction layer analysis |
| 9 | CAS version check | P1 | Low | Verification needed |

---

## Recommended Fix Order

### Phase 1: Quick Wins (P0, Low Effort)
1. Add key validation (empty, NUL, reserved prefix)
2. Fix `kv_incr` overflow with `checked_add()`

### Phase 2: Core Features (P0, Medium Effort)
3. Implement `kv_get_at` (storage already supports)
4. Implement `kv_history` (needs VersionChain exposure)

### Phase 3: Table Stakes (P0, Medium Effort)
5. Add `kv_keys` for key enumeration
6. Add `kv_scan` for paginated scanning

### Phase 4: Concurrency (P1, High Effort)
7. Investigate and fix WriteConflict issues
8. Verify `kv_cas_version` behavior

---

## Test Coverage After Fixes

| Current | After Phase 1 | After Phase 2 | After Phase 3 | After Phase 4 |
|---------|---------------|---------------|---------------|---------------|
| 138 pass | 141 pass | 144 pass | 158 pass | 162 pass |
| 10 fail | 7 fail | 4 fail | 4 fail | 0 fail |
| 15 ignore | 14 ignore | 11 ignore | 0 ignore | 0 ignore |
