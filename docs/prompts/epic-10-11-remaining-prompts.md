# Epic 10-11 Remaining Work - Two Stories

**Remaining Items**:
- Issue #99 - Cross-Primitive Transactions
- Issue #102 - Transaction Timeout Support

**Status**: Ready to implement
**Dependencies**: Epic 10 core complete (Stories #98, #100, #101 implemented)

---

## Summary

Epics 10 and 11 are substantially complete. The remaining unimplemented features are:

### Already Implemented (No Action Needed)
- Issue #98: Database Transaction API
- Issue #100: Transaction Context Lifecycle
- Issue #101: Retry Backoff Strategy
- Issue #103: Implicit Transaction Wrapper
- Issue #104: M1 Test Suite Verification
- Issue #105: Migration Guide (Closed - M1 to M2 is fully backward compatible)

### To Implement
- **Issue #99: Cross-Primitive Transactions** - Test atomic writes to KV and Event keys
- **Issue #102: Transaction Timeout Support** - Abort long-running transactions

---

## Tool Paths

Use fully qualified paths:
- Cargo: `~/.cargo/bin/cargo`
- GitHub CLI: `/opt/homebrew/bin/gh`

---

## Git Workflow

**IMPORTANT**: Create PRs for each story. Do NOT commit directly.

---

## Story #99: Cross-Primitive Transactions

**GitHub Issue**: #99
**Estimated Time**: 3 hours
**Dependencies**: Story #98 complete (already done)

### What This Story Is About

This story validates that different Key types (KV and Event) can be atomically written in a single transaction. The system already supports:

- `Key::new_kv(namespace, key)` - for KV storage
- `Key::new_event(namespace, seq)` - for Event log storage

Both use the same `UnifiedStore`. This story tests that:
1. A transaction can write to both KV and Event keys
2. Both writes commit atomically (all-or-nothing)
3. Conflicts on one primitive type cause rollback of both
4. Cross-primitive reads see consistent state

### What to Implement

Create integration tests in `crates/engine/tests/cross_primitive_tests.rs`:

```rust
//! Cross-Primitive Transaction Tests
//!
//! Per M2_REVISED_PLAN.md Story #54 and GitHub Issue #99:
//! Validates that transactions atomically operate across different
//! Key types (KV and Event) in a single transaction.

use in_mem_core::error::Error;
use in_mem_core::types::{Key, Namespace, RunId};
use in_mem_core::value::Value;
use in_mem_engine::Database;
use tempfile::TempDir;

fn create_ns(run_id: RunId) -> Namespace {
    Namespace::new(
        "tenant".to_string(),
        "app".to_string(),
        "agent".to_string(),
        run_id,
    )
}

// ============================================================================
// Cross-Primitive Atomic Write Tests
// ============================================================================

#[test]
fn test_atomic_kv_and_event_write() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);

    // Create keys for both KV and Event types
    let kv_key = Key::new_kv(ns.clone(), "user_state");
    let event_key = Key::new_event(ns.clone(), 1); // Event with sequence 1

    // Transaction writes to BOTH KV and Event in single transaction
    db.transaction(run_id, |txn| {
        txn.put(kv_key.clone(), Value::String("active".to_string()))?;
        txn.put(event_key.clone(), Value::String("user_logged_in".to_string()))?;
        Ok(())
    })
    .unwrap();

    // Verify BOTH were committed atomically
    let kv_result = db.get(&kv_key).unwrap().unwrap();
    assert_eq!(kv_result.value, Value::String("active".to_string()));

    let event_result = db.get(&event_key).unwrap().unwrap();
    assert_eq!(event_result.value, Value::String("user_logged_in".to_string()));

    // Both should have the SAME version (atomically committed)
    assert_eq!(kv_result.version, event_result.version);
}

#[test]
fn test_atomic_kv_and_event_with_multiple_events() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);

    let kv_key = Key::new_kv(ns.clone(), "counter");
    let event1 = Key::new_event(ns.clone(), 1);
    let event2 = Key::new_event(ns.clone(), 2);
    let event3 = Key::new_event(ns.clone(), 3);

    // Write KV state + 3 events atomically
    db.transaction(run_id, |txn| {
        txn.put(kv_key.clone(), Value::I64(100))?;
        txn.put(event1.clone(), Value::String("event_1".to_string()))?;
        txn.put(event2.clone(), Value::String("event_2".to_string()))?;
        txn.put(event3.clone(), Value::String("event_3".to_string()))?;
        Ok(())
    })
    .unwrap();

    // All should be committed with same version
    let kv = db.get(&kv_key).unwrap().unwrap();
    let e1 = db.get(&event1).unwrap().unwrap();
    let e2 = db.get(&event2).unwrap().unwrap();
    let e3 = db.get(&event3).unwrap().unwrap();

    assert_eq!(kv.version, e1.version);
    assert_eq!(e1.version, e2.version);
    assert_eq!(e2.version, e3.version);
}

// ============================================================================
// Cross-Primitive Rollback Tests
// ============================================================================

#[test]
fn test_cross_primitive_rollback_on_error() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);

    let kv_key = Key::new_kv(ns.clone(), "state");
    let event_key = Key::new_event(ns.clone(), 1);

    // Pre-populate KV key
    db.put(run_id, kv_key.clone(), Value::I64(0)).unwrap();

    // Transaction that writes to both but then fails
    let result: Result<(), Error> = db.transaction(run_id, |txn| {
        txn.put(kv_key.clone(), Value::I64(999))?;
        txn.put(event_key.clone(), Value::String("should_rollback".to_string()))?;

        // Force abort
        Err(Error::InvalidState("intentional failure".to_string()))
    });

    assert!(result.is_err());

    // BOTH writes should be rolled back
    // KV should still have original value
    let kv = db.get(&kv_key).unwrap().unwrap();
    assert_eq!(kv.value, Value::I64(0)); // Original value preserved

    // Event should NOT exist
    assert!(db.get(&event_key).unwrap().is_none());
}

#[test]
fn test_cross_primitive_conflict_rollback() {
    use std::sync::Arc;
    use std::thread;

    let temp_dir = TempDir::new().unwrap();
    let db = Arc::new(Database::open(temp_dir.path().join("db")).unwrap());

    let run_id = RunId::new();
    let ns = create_ns(run_id);

    let kv_key = Key::new_kv(ns.clone(), "contested_kv");
    let event_key = Key::new_event(ns.clone(), 1);

    // Pre-populate with initial value
    db.put(run_id, kv_key.clone(), Value::I64(0)).unwrap();

    let db1 = Arc::clone(&db);
    let db2 = Arc::clone(&db);
    let kv_key1 = kv_key.clone();
    let event_key1 = event_key.clone();

    // Use barriers to control execution order
    use std::sync::Barrier;
    let barrier = Arc::new(Barrier::new(2));
    let barrier1 = Arc::clone(&barrier);
    let barrier2 = Arc::clone(&barrier);

    // T1: Read KV, write Event + KV
    let h1 = thread::spawn(move || {
        db1.transaction(run_id, |txn| {
            // Read KV (adds to read_set)
            let _val = txn.get(&kv_key1)?;

            // Wait for T2 to also start
            barrier1.wait();

            // Write to both primitives
            txn.put(kv_key1.clone(), Value::I64(1))?;
            txn.put(event_key1.clone(), Value::String("from_t1".to_string()))?;

            // Small delay to let T2 commit first (usually)
            thread::sleep(std::time::Duration::from_millis(5));

            Ok(())
        })
    });

    // T2: Just update the KV key (no event)
    let h2 = thread::spawn(move || {
        db2.transaction(run_id, |txn| {
            // Wait for T1 to start
            barrier2.wait();

            // Blind write to KV (should commit quickly)
            txn.put(kv_key.clone(), Value::I64(2))?;
            Ok(())
        })
    });

    let r1 = h1.join().unwrap();
    let r2 = h2.join().unwrap();

    // At least one should succeed
    assert!(r1.is_ok() || r2.is_ok());

    // If T1 failed due to conflict, the event should NOT be written
    // (both KV and Event must roll back together)
    if r1.is_err() {
        // T1 was aborted - event should not exist
        assert!(db.get(&event_key).unwrap().is_none());
    }
}

// ============================================================================
// Cross-Primitive Read Consistency Tests
// ============================================================================

#[test]
fn test_cross_primitive_read_consistency() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);

    let kv_key = Key::new_kv(ns.clone(), "state");
    let event1 = Key::new_event(ns.clone(), 1);
    let event2 = Key::new_event(ns.clone(), 2);

    // Commit 1: Write initial state
    db.transaction(run_id, |txn| {
        txn.put(kv_key.clone(), Value::I64(1))?;
        txn.put(event1.clone(), Value::String("initial".to_string()))?;
        Ok(())
    })
    .unwrap();

    // Commit 2: Update state and add event
    db.transaction(run_id, |txn| {
        txn.put(kv_key.clone(), Value::I64(2))?;
        txn.put(event2.clone(), Value::String("updated".to_string()))?;
        Ok(())
    })
    .unwrap();

    // A transaction reading both should see consistent state
    db.transaction(run_id, |txn| {
        let kv = txn.get(&kv_key)?.unwrap();
        let e1 = txn.get(&event1)?;
        let e2 = txn.get(&event2)?;

        // Both events should be visible
        assert!(e1.is_some());
        assert!(e2.is_some());

        // KV should be the latest value
        assert_eq!(kv, Value::I64(2));

        Ok(())
    })
    .unwrap();
}

#[test]
fn test_cross_primitive_delete_atomicity() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);

    let kv_key = Key::new_kv(ns.clone(), "to_delete_kv");
    let event_key = Key::new_event(ns.clone(), 1);

    // Create both
    db.transaction(run_id, |txn| {
        txn.put(kv_key.clone(), Value::I64(100))?;
        txn.put(event_key.clone(), Value::String("event".to_string()))?;
        Ok(())
    })
    .unwrap();

    // Delete both atomically
    db.transaction(run_id, |txn| {
        txn.delete(kv_key.clone())?;
        txn.delete(event_key.clone())?;
        Ok(())
    })
    .unwrap();

    // Both should be deleted
    assert!(db.get(&kv_key).unwrap().is_none());
    assert!(db.get(&event_key).unwrap().is_none());
}

// ============================================================================
// Recovery Tests for Cross-Primitive Transactions
// ============================================================================

#[test]
fn test_cross_primitive_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("db");
    let run_id = RunId::new();
    let ns = create_ns(run_id);

    let kv_key = Key::new_kv(ns.clone(), "persistent_kv");
    let event_key = Key::new_event(ns.clone(), 42);

    // Write cross-primitive data and close
    {
        let db = Database::open(&db_path).unwrap();
        db.transaction(run_id, |txn| {
            txn.put(kv_key.clone(), Value::String("kv_data".to_string()))?;
            txn.put(event_key.clone(), Value::String("event_data".to_string()))?;
            Ok(())
        })
        .unwrap();
    }

    // Reopen and verify both recovered
    {
        let db = Database::open(&db_path).unwrap();

        let kv = db.get(&kv_key).unwrap().unwrap();
        assert_eq!(kv.value, Value::String("kv_data".to_string()));

        let event = db.get(&event_key).unwrap().unwrap();
        assert_eq!(event.value, Value::String("event_data".to_string()));

        // Should have same version (atomically committed)
        assert_eq!(kv.version, event.version);
    }
}
```

### Implementation Steps

#### Step 1: Create story branch

```bash
git checkout develop
git pull origin develop
git checkout -b epic-10-story-99-cross-primitive-txn
```

#### Step 2: Create the test file

Create `crates/engine/tests/cross_primitive_tests.rs` with the test code above.

#### Step 3: Run tests

```bash
~/.cargo/bin/cargo test -p in-mem-engine --test cross_primitive_tests
```

#### Step 4: Run full validation

```bash
~/.cargo/bin/cargo test --all
~/.cargo/bin/cargo clippy --all -- -D warnings
~/.cargo/bin/cargo fmt --check
```

#### Step 5: Complete the story

```bash
git add -A
git commit -m "$(cat <<'EOF'
Story #99: Cross-Primitive Transaction Tests

Add integration tests validating atomic transactions across
different Key types (KV and Event):

- test_atomic_kv_and_event_write: Single txn writes to both
- test_atomic_kv_and_event_with_multiple_events: Multiple events + KV
- test_cross_primitive_rollback_on_error: Abort rolls back both
- test_cross_primitive_conflict_rollback: Conflict affects both
- test_cross_primitive_read_consistency: Consistent reads
- test_cross_primitive_delete_atomicity: Delete both atomically
- test_cross_primitive_recovery: WAL recovery for both

Per M2_REVISED_PLAN.md Story #54 and GitHub Issue #99.

Acceptance Criteria:
- Transaction writes to KV and Events atomically
- Both primitives committed with same version
- Conflict causes rollback of both primitives
- Cross-primitive reads see consistent state
- Recovery restores both primitives

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"

git push -u origin epic-10-story-99-cross-primitive-txn

/opt/homebrew/bin/gh pr create --base develop --head epic-10-story-99-cross-primitive-txn \
  --title "Story #99: Cross-Primitive Transaction Tests" \
  --body "$(cat <<'EOF'
## Summary
- Add integration tests for cross-primitive (KV + Event) transactions
- Validates atomic commit across different Key types
- Tests rollback behavior when one primitive conflicts
- Tests recovery of cross-primitive transactions

## Test plan
- [ ] test_atomic_kv_and_event_write
- [ ] test_atomic_kv_and_event_with_multiple_events
- [ ] test_cross_primitive_rollback_on_error
- [ ] test_cross_primitive_conflict_rollback
- [ ] test_cross_primitive_read_consistency
- [ ] test_cross_primitive_delete_atomicity
- [ ] test_cross_primitive_recovery

Closes #99

Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

### Acceptance Criteria

- [ ] Transaction writes to KV and Events atomically
- [ ] Both primitives committed with same version
- [ ] Conflict causes rollback of both primitives
- [ ] Cross-primitive reads see consistent state
- [ ] Recovery restores both primitives correctly
- [ ] All tests pass

---

## Story #102: Transaction Timeout Support

**GitHub Issue**: #102
**Estimated Time**: 3 hours
**Dependencies**: Story #98 complete (already done)

### What to Implement

Add timeout support to prevent runaway transactions from holding resources indefinitely.

#### New API

```rust
// In crates/concurrency/src/transaction.rs
impl TransactionContext {
    /// Check if this transaction has exceeded the given timeout
    pub fn is_expired(&self, timeout: Duration) -> bool;

    /// Get elapsed time since transaction started
    pub fn elapsed(&self) -> Duration;
}

// In crates/engine/src/database.rs
impl Database {
    /// Execute a transaction with timeout
    pub fn transaction_with_timeout<F, T>(
        &self,
        run_id: RunId,
        timeout: Duration,
        f: F,
    ) -> Result<T>
    where
        F: FnOnce(&mut TransactionContext) -> Result<T>;
}
```

### Implementation Steps

#### Step 1: Create story branch

```bash
git checkout develop
git pull origin develop
git checkout -b epic-10-story-102-transaction-timeout
```

#### Step 2: Add start_time to TransactionContext

Update `crates/concurrency/src/transaction.rs`:

```rust
use std::time::{Duration, Instant};

pub struct TransactionContext {
    // ... existing fields ...

    /// When this transaction was created
    start_time: Instant,
}

impl TransactionContext {
    pub fn new(txn_id: u64, run_id: RunId) -> Self {
        Self {
            txn_id,
            run_id,
            status: TransactionStatus::Active,
            read_set: Vec::new(),
            write_set: Vec::new(),
            delete_set: Vec::new(),
            cas_set: Vec::new(),
            snapshot: None,
            start_time: Instant::now(),
        }
    }

    pub fn with_snapshot(
        txn_id: u64,
        run_id: RunId,
        snapshot: Box<dyn SnapshotView + Send + Sync>,
    ) -> Self {
        Self {
            txn_id,
            run_id,
            status: TransactionStatus::Active,
            read_set: Vec::new(),
            write_set: Vec::new(),
            delete_set: Vec::new(),
            cas_set: Vec::new(),
            snapshot: Some(snapshot),
            start_time: Instant::now(),
        }
    }

    /// Check if this transaction has exceeded the given timeout
    pub fn is_expired(&self, timeout: Duration) -> bool {
        self.start_time.elapsed() > timeout
    }

    /// Get the elapsed time since transaction started
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}
```

#### Step 3: Add TransactionTimeout error variant

Update `crates/core/src/error.rs`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    // ... existing variants ...

    /// Transaction exceeded timeout
    TransactionTimeout(String),
}

impl Error {
    /// Check if this error is a transaction timeout
    pub fn is_timeout(&self) -> bool {
        matches!(self, Error::TransactionTimeout(_))
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // ... existing matches ...
            Error::TransactionTimeout(msg) => write!(f, "Transaction timeout: {}", msg),
        }
    }
}
```

#### Step 4: Add transaction_with_timeout to Database

Update `crates/engine/src/database.rs`:

```rust
use std::time::Duration;

/// Default transaction timeout (5 seconds)
pub const DEFAULT_TRANSACTION_TIMEOUT: Duration = Duration::from_secs(5);

impl Database {
    /// Execute a transaction with timeout
    ///
    /// If the transaction exceeds the timeout, it will be aborted
    /// before commit is attempted.
    pub fn transaction_with_timeout<F, T>(
        &self,
        run_id: RunId,
        timeout: Duration,
        f: F,
    ) -> Result<T>
    where
        F: FnOnce(&mut TransactionContext) -> Result<T>,
    {
        let mut txn = self.begin_transaction(run_id);

        // Execute closure
        let result = f(&mut txn);

        match result {
            Ok(value) => {
                // Check timeout before commit
                if txn.is_expired(timeout) {
                    let elapsed = txn.elapsed();
                    let _ = txn.mark_aborted(format!(
                        "Transaction timeout: elapsed {:?}, limit {:?}",
                        elapsed, timeout
                    ));
                    self.coordinator.record_abort();
                    return Err(Error::TransactionTimeout(format!(
                        "Transaction exceeded timeout of {:?} (elapsed: {:?})",
                        timeout, elapsed
                    )));
                }

                // Commit on success
                self.commit_transaction(&mut txn)?;
                Ok(value)
            }
            Err(e) => {
                // Abort on error
                let _ = txn.mark_aborted(format!("Closure error: {}", e));
                self.coordinator.record_abort();
                Err(e)
            }
        }
    }
}
```

#### Step 5: Write tests

Add to `crates/engine/src/database.rs` tests section:

```rust
// ========================================================================
// Timeout Tests (Story #102)
// ========================================================================

#[test]
fn test_transaction_is_expired() {
    use std::time::Duration;
    use std::thread;

    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let txn = db.begin_transaction(run_id);

    // Should not be expired immediately
    assert!(!txn.is_expired(Duration::from_secs(1)));

    // Sleep briefly
    thread::sleep(Duration::from_millis(50));

    // Should be expired with very short timeout
    assert!(txn.is_expired(Duration::from_millis(10)));

    // Should not be expired with longer timeout
    assert!(!txn.is_expired(Duration::from_secs(10)));
}

#[test]
fn test_transaction_with_timeout_success() {
    use std::time::Duration;

    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_test_namespace(run_id);
    let key = Key::new_kv(ns, "timeout_success");

    // Transaction completes within timeout
    let result = db.transaction_with_timeout(run_id, Duration::from_secs(5), |txn| {
        txn.put(key.clone(), Value::I64(42))?;
        Ok(42)
    });

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);

    // Verify stored
    let stored = db.get(&key).unwrap().unwrap();
    assert_eq!(stored.value, Value::I64(42));
}

#[test]
fn test_transaction_with_timeout_expired() {
    use std::time::Duration;
    use std::thread;

    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_test_namespace(run_id);
    let key = Key::new_kv(ns, "timeout_expired");

    // Transaction exceeds timeout
    let result: Result<()> = db.transaction_with_timeout(
        run_id,
        Duration::from_millis(10), // Very short timeout
        |txn| {
            txn.put(key.clone(), Value::I64(999))?;
            // Sleep to exceed timeout
            thread::sleep(Duration::from_millis(50));
            Ok(())
        },
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.is_timeout());

    // Data should NOT be committed
    assert!(db.get(&key).unwrap().is_none());
}

#[test]
fn test_transaction_with_timeout_normal_not_affected() {
    use std::time::Duration;

    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_test_namespace(run_id);

    // Run many quick transactions with timeout
    for i in 0..100 {
        let key = Key::new_kv(ns.clone(), &format!("key_{}", i));
        let result = db.transaction_with_timeout(
            run_id,
            Duration::from_secs(5),
            |txn| {
                txn.put(key.clone(), Value::I64(i as i64))?;
                Ok(())
            },
        );
        assert!(result.is_ok());
    }

    // All should be stored
    for i in 0..100 {
        let key = Key::new_kv(ns.clone(), &format!("key_{}", i));
        let val = db.get(&key).unwrap().unwrap();
        assert_eq!(val.value, Value::I64(i as i64));
    }
}

#[test]
fn test_transaction_elapsed() {
    use std::time::Duration;
    use std::thread;

    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let txn = db.begin_transaction(run_id);

    // Elapsed should be very small initially
    let initial = txn.elapsed();
    assert!(initial < Duration::from_millis(100));

    // After sleep, elapsed should increase
    thread::sleep(Duration::from_millis(50));
    let after = txn.elapsed();
    assert!(after >= Duration::from_millis(50));
    assert!(after > initial);
}
```

#### Step 6: Run validation

```bash
~/.cargo/bin/cargo test -p in-mem-concurrency
~/.cargo/bin/cargo test -p in-mem-core
~/.cargo/bin/cargo test -p in-mem-engine
~/.cargo/bin/cargo test --all
~/.cargo/bin/cargo clippy --all -- -D warnings
~/.cargo/bin/cargo fmt --check
```

#### Step 7: Complete the story

```bash
git add -A
git commit -m "$(cat <<'EOF'
Story #102: Transaction Timeout Support

- Add start_time tracking to TransactionContext
- Add is_expired() and elapsed() methods
- Add TransactionTimeout error variant
- Add Database::transaction_with_timeout()
- Add comprehensive unit tests

Acceptance Criteria:
- TransactionContext.is_expired(timeout) method
- Database::transaction_with_timeout(run_id, timeout, f)
- Timeout check before commit attempt
- Aborted transactions on timeout
- Unit tests for timeout functionality

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"

git push -u origin epic-10-story-102-transaction-timeout

/opt/homebrew/bin/gh pr create --base develop --head epic-10-story-102-transaction-timeout \
  --title "Story #102: Transaction Timeout Support" \
  --body "$(cat <<'EOF'
## Summary
- Add timeout support to prevent runaway transactions
- TransactionContext tracks start_time and provides is_expired()/elapsed() methods
- Database provides transaction_with_timeout()
- Timeout is checked before commit - if exceeded, transaction is aborted

## Test plan
- [ ] Unit tests for is_expired() and elapsed()
- [ ] Unit tests for transaction_with_timeout() success case
- [ ] Unit tests for transaction_with_timeout() timeout case
- [ ] Verify aborted transactions don't apply writes
- [ ] Verify normal transactions unaffected

Closes #102

Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

### Acceptance Criteria

- [ ] TransactionContext.is_expired(timeout) method implemented
- [ ] TransactionContext.elapsed() method implemented
- [ ] Database::transaction_with_timeout() implemented
- [ ] TransactionTimeout error variant added
- [ ] Timeout checked before commit attempt
- [ ] Aborted transactions don't apply writes
- [ ] All tests pass
- [ ] No clippy warnings

---

## After Completing Both Stories

After both PRs are merged:

```bash
# Close issues
/opt/homebrew/bin/gh issue close 99 --reason completed
/opt/homebrew/bin/gh issue close 102 --reason completed

# Epics 10 and 11 are now COMPLETE
```

---

## Summary

After implementing Stories #99 and #102, M2 Transaction milestone will have all epics complete:

| Epic | Status |
|------|--------|
| Epic 6: Transaction Foundations | Complete |
| Epic 7: Transaction Semantics | Complete |
| Epic 8: Durability & Commit | Complete |
| Epic 9: Recovery Support | Complete |
| Epic 10: Database API | **Complete after #99, #102** |
| Epic 11: M1 Compatibility | Complete |
| Epic 12: Testing & Validation | Pending |

---

*Generated for remaining Epic 10-11 work*
*Issues #99 (Cross-Primitive Transactions) and #102 (Transaction Timeout Support) remain*
