# Expert Code Review: strata-engine

## Summary

Thorough expert review of the `strata-engine` crate looking for inconsistencies, dead code, bad practices, architectural gaps, and potential bugs.

---

## Critical Architectural Issues

### 1. Uses Legacy WAL System (Continuation from Lower Layers)

**Location**: `database.rs:33`

```rust
use strata_durability::wal::{DurabilityMode, WAL};
```

The engine crate, like the concurrency crate, uses the Legacy WAL system:

| Layer | WAL System | Impact |
|-------|------------|--------|
| Storage | WalRecord (own format) | Unused by engine |
| Durability | Legacy WALEntry (bincode) | Used by engine |
| Durability | Modern WalEntry (CRC32) | Unused |
| Concurrency | Legacy WALEntry | Used by engine |
| Engine | Legacy WALEntry | Locked in |

**Impact:**
- Entire transaction stack is tied to deprecated format
- Cannot migrate to Modern WAL without updating engine + concurrency + durability
- RunBundle portability depends on Legacy format

### 2. JSON and Vector Operations Are Stubs

**Location**: `transaction/context.rs`

The `Transaction` wrapper implementing `TransactionOps` has stub implementations for JSON and Vector operations:

```rust
// Json operations - ALL return unimplemented!
fn json_create(&mut self, _doc_id: &JsonDocId, _value: JsonValue) -> Result<Version, StrataError> {
    Err(StrataError::Internal {
        message: "json_create not implemented in engine Transaction".to_string(),
    })
}

fn json_get(&self, _doc_id: &JsonDocId) -> Result<Option<Versioned<JsonValue>>, StrataError> {
    Err(StrataError::Internal {
        message: "json_get not implemented in engine Transaction".to_string(),
    })
}

// Vector operations - ALL return unimplemented!
fn vector_insert(...) -> Result<Version, StrataError> {
    Err(StrataError::Internal {
        message: "vector_insert not implemented in engine Transaction".to_string(),
    })
}
```

**Impact:**
- `TransactionOps` trait promises JSON/Vector support but engine doesn't deliver
- Users calling these methods get runtime errors, not compile-time errors
- Inconsistent primitive support

### 3. Global Static Recovery Registry

**Location**: `recovery_participant.rs:75-76`

```rust
static RECOVERY_REGISTRY: once_cell::sync::Lazy<RwLock<Vec<RecoveryParticipant>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(Vec::new()));
```

The recovery participant registry is a global static:

**Problems:**
- Cannot have multiple isolated Database instances with different participants
- Tests can interfere with each other (requires `TEST_LOCK` in tests)
- Hard to reason about registration order in multi-threaded startup

**Impact:** Test isolation requires explicit `clear_recovery_registry()` calls.

---

## Moderate Issues

### 4. commit_transaction and commit_with_durability Have Duplicated Code

**Location**: `database.rs:1102-1182` vs `database.rs:1270-1350+`

Both methods contain nearly identical logic:
1. Acquire per-run commit lock
2. Validate transaction
3. Allocate commit version
4. Write to WAL
5. Apply to storage
6. Mark committed

The only difference is how WAL sync is handled.

**Impact:**
- Maintenance burden (changes must be made in two places)
- Risk of divergence
- ~80 lines of duplicate code

### 5. BufferedDurability Drop Doesn't Call flush_sync

**Location**: `durability/buffered.rs:360-391`

```rust
impl Drop for BufferedDurability {
    fn drop(&mut self) {
        // ... signals shutdown, waits for thread ...

        // Note: We don't call flush_sync() in Drop because it could fail
        // and Drop can't return errors. The explicit shutdown() should be
        // called for guaranteed flush.
    }
}
```

**Impact:**
- Data loss possible if `shutdown()` not called explicitly
- Background thread does final flush but only on normal shutdown
- Silent data loss on panic or unexpected termination

### 6. Error Information Loss in RunError

**Location**: `replay.rs:63-67`

```rust
impl From<strata_core::error::Error> for RunError {
    fn from(e: strata_core::error::Error) -> Self {
        RunError::Storage(e.to_string())  // Loses error type!
    }
}
```

**Impact:**
- Original error type lost
- Cannot match on specific error kinds after conversion
- Same pattern seen in other error types throughout codebase

### 7. TransactionCoordinator::new Starts at Version 1, ephemeral() Also Starts at 1

**Location**: `database.rs:627` and `coordinator.rs:49`

```rust
// ephemeral() creates coordinator starting at version 1
let coordinator = TransactionCoordinator::new(1);

// But TransactionCoordinator::new(0) would also work for recovery case
```

This is inconsistent with recovery which starts from `final_version` (could be 0).

**Impact:** Minor - version 0 is reserved for "key doesn't exist" so starting at 1 is correct, but the reasoning isn't documented.

---

## Minor Issues & Potential Bugs

### 8. RetryConfig::calculate_delay Can Overflow

**Location**: `database.rs:110-116`

```rust
fn calculate_delay(&self, attempt: usize) -> Duration {
    let shift = attempt.min(63);  // Caps at 63 to prevent overflow
    let multiplier = 1u64 << shift;  // But 1 << 63 is valid
    let delay_ms = self.base_delay_ms.saturating_mul(multiplier);
    Duration::from_millis(delay_ms.min(self.max_delay_ms))
}
```

If `base_delay_ms` is large (e.g., 1000) and attempt is 63:
- `1 << 63` = 9223372036854775808
- `1000 * 9223372036854775808` would overflow, but saturating_mul handles this

**Impact:** Low - saturating_mul prevents panic, but delay becomes u64::MAX briefly before capping.

### 9. Database::flush() Doesn't Check Durability Mode

**Location**: `database.rs:682-690`

```rust
pub fn flush(&self) -> Result<()> {
    if let Some(ref wal) = self.wal {
        let wal = wal.lock();
        wal.fsync()  // Always calls fsync even in None durability mode
    } else {
        Ok(())
    }
}
```

**Impact:** Low - calling flush() on None durability mode is wasteful but not incorrect.

### 10. TransactionPool MAX_POOL_SIZE is Small

**Location**: `transaction/pool.rs`

```rust
pub const MAX_POOL_SIZE: usize = 8;
```

For highly concurrent workloads with many threads, 8 pooled transactions per thread may be insufficient.

**Impact:** Low - pool just creates new transactions when exhausted.

### 11. Extension Downcast Panic

**Location**: `database.rs:732-738`

```rust
entry.value().clone().downcast::<T>()
    .expect("extension type mismatch - this is a bug")
```

Uses `expect()` which will panic on type mismatch.

**Impact:** Low - this should never happen if the code is correct, but a panic in production is severe.

### 12. accepting_transactions Flag Race Condition

**Location**: `database.rs:773-777`

```rust
if !self.accepting_transactions.load(Ordering::SeqCst) {
    return Err(Error::InvalidOperation("Database is shutting down".to_string()));
}
// ... transaction could start here before close() fully completes
let mut txn = self.begin_transaction(run_id);
```

There's a TOCTOU race between checking the flag and starting the transaction.

**Impact:** Low - transactions started during shutdown will complete normally; just means cleanup isn't perfectly clean.

---

## Dead Code & Unused Items

### 13. DurabilityMode docs mention "Async" mode

**Location**: `database.rs:512` (doc comment)

```rust
/// Allows selecting between Strict, Batched, or Async durability modes.
```

But `DurabilityMode` enum only has: `None`, `Strict`, `Batched`.

**Impact:** Documentation inconsistency. The "Async" mode never existed - it was likely a planned feature that was never implemented. The actual modes are:
- `None` - No persistence (was previously called "InMemory")
- `Strict` - fsync every commit
- `Batched` - Periodic fsync

**Fix Required**: Update doc comment to reference actual modes.

### 14. PersistenceMode::default() May Not Match Usage

**Location**: `database.rs:163-167`

```rust
impl Default for PersistenceMode {
    fn default() -> Self {
        PersistenceMode::Disk
    }
}
```

But `DatabaseBuilder::new()` uses `PersistenceMode::Disk` explicitly, making the Default impl unused internally.

**Impact:** Low - Default is useful for external code.

---

## Recommendations

### Immediate Fixes (Low Risk)

1. **Document version 1 reasoning**: Add comment explaining why ephemeral starts at version 1
2. **Fix documentation**: Remove "Async" mode mention
3. **Add BufferedDurability drop warning**: Log warning if pending writes > 0 on drop without shutdown (already done)
4. **Extract commit logic**: Create shared `commit_internal()` called by both commit methods

### Short-Term Improvements

5. **Implement JSON operations**: Wire up JsonStore through TransactionOps
6. **Implement Vector operations**: Wire up VectorStore through TransactionOps
7. **Make recovery registry per-Database**: Use Database field instead of global static
8. **Add flush_sync to Drop**: Best-effort flush even if errors can't be returned

### Long-Term Architectural Changes

9. **Migrate to Modern WAL**: After consolidating WAL systems in lower layers
10. **Use compile-time feature flags for primitives**: Instead of runtime unimplemented!() errors
11. **Add structured error chain**: Preserve error types through conversions
12. **Consider larger default pool size**: Or make it configurable

---

## Test Coverage Assessment

The crate has **excellent test coverage**:

- `database.rs`: Comprehensive tests for all transaction APIs
- `coordinator.rs`: Adversarial tests for concurrency edge cases
- `replay.rs`: Full coverage of P1-P6 invariants
- `recovery_participant.rs`: Concurrent registration tests
- `durability/*.rs`: Tests for all three modes

**Particularly Strong:**
- Coordinator active_count saturation (prevents underflow)
- Retry exponential backoff
- Transaction timeout handling
- Pool acquire/release semantics
- Run diff scenarios

---

## Cross-Crate Consistency Issues

### 1. Uses Legacy WAL (consistent with concurrency crate)

Engine and concurrency both use Legacy WAL, creating a dependency chain that blocks migration.

### 2. Uses ShardedStore (not UnifiedStore)

Engine uses ShardedStore directly, which lacks some indices that UnifiedStore has:
- No TypeIndex (can't query by primitive type)
- No TTLIndex (TTLCleaner won't work)

### 3. Per-Run Commit Locks (improvement over concurrency)

Engine improves on concurrency's single commit lock with per-run locks.
This is good, but means the two crates have different concurrency models.

---

## Overall Assessment

**Rating**: Good with moderate architectural debt

The `strata-engine` crate is well-implemented with:
- Clean builder pattern for configuration
- Multiple transaction APIs for different use cases
- Proper transaction pooling
- Clear separation of concerns

The main issues are:

1. **Legacy WAL dependency** - Inherited from lower layers
2. **Incomplete TransactionOps** - JSON/Vector operations are stubs
3. **Global recovery registry** - Complicates testing and isolation
4. **Code duplication** - Two commit methods with similar code

The code follows excellent Rust practices:
- Clear documentation with examples
- Comprehensive error handling
- Thread-safe design with explicit reasoning
- Feature-gated instrumentation

Test coverage is exceptional, giving high confidence in correctness.
