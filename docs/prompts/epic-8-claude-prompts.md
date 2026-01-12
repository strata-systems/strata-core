# Epic 8: Durability & Commit - Implementation Prompts

**Epic Goal**: Integrate transaction validation with WAL for durable, atomic commits.

**Status**: Ready to begin
**Dependencies**: Epic 7 (Transaction Semantics) complete

---

## 🔴 AUTHORITATIVE SPECIFICATION - READ THIS FIRST

**`docs/architecture/M2_TRANSACTION_SEMANTICS.md` is the GOSPEL for ALL M2 implementation.**

This is not a guideline. This is not a suggestion. This is the **LAW**.

### Rules for Every Story in Every Epic of M2:

1. **Every story MUST implement behavior EXACTLY as specified in the semantics document**
   - No "improvements" that deviate from the spec
   - No "simplifications" that change behavior
   - No "optimizations" that break guarantees

2. **If your code contradicts the spec, YOUR CODE IS WRONG**
   - The spec defines correct behavior
   - Fix the code, not the spec

3. **If your tests contradict the spec, YOUR TESTS ARE WRONG**
   - Tests must validate spec-compliant behavior
   - Never adjust tests to make broken code pass

4. **If the spec seems wrong or unclear:**
   - STOP implementation immediately
   - Raise the issue for discussion
   - Do NOT proceed with assumptions
   - Do NOT implement your own interpretation

5. **No breaking the spec for ANY reason:**
   - Not for "performance"
   - Not for "simplicity"
   - Not for "it's just an edge case"
   - Not for "we can fix it later"

### What the Spec Defines (Read Before Any M2 Work):

| Section | Content | You MUST Follow |
|---------|---------|-----------------|
| Section 1 | Isolation Level | **Snapshot Isolation, NOT Serializability** |
| Section 2 | Visibility Rules | What txns see/don't see/may see |
| Section 3 | Conflict Detection | When aborts happen, first-committer-wins |
| Section 4 | Implicit Transactions | How M1-style ops work in M2 |
| Section 5 | Replay Semantics | No re-validation, single-threaded |
| Section 6 | Version Semantics | Version 0 = never existed, tombstones |

### Before Starting ANY Story:

```bash
# 1. Read the full spec
cat docs/architecture/M2_TRANSACTION_SEMANTICS.md

# 2. Identify which sections apply to your story
# 3. Understand the EXACT behavior required
# 4. Implement EXACTLY that behavior
# 5. Write tests that validate spec compliance
```

**WARNING**: Code review will verify spec compliance. Non-compliant code will be rejected.

---

## 🔴 BRANCHING STRATEGY - READ THIS

### Branch Hierarchy
```
main                          ← Protected: only accepts merges from develop
  └── develop                 ← Integration branch for completed epics
       └── epic-8-durability-commit  ← Epic branch (base for all story PRs)
            └── epic-8-story-88-*    ← Story branches
```

### Critical Rules

1. **Story PRs go to EPIC branch, NOT main**
   ```bash
   # CORRECT: PR base is epic branch
   /opt/homebrew/bin/gh pr create --base epic-8-durability-commit --head epic-8-story-88-commit-path

   # WRONG: Never PR directly to main
   /opt/homebrew/bin/gh pr create --base main --head epic-8-story-88-commit-path  # ❌ NEVER DO THIS
   ```

2. **Epic branches merge to develop** (after all stories complete)
   ```bash
   git checkout develop
   git merge --no-ff epic-8-durability-commit
   ```

3. **develop merges to main** (at milestone boundaries)
   ```bash
   git checkout main
   git merge --no-ff develop -m "M2: Complete"
   ```

4. **main is protected** - requires PR, no direct pushes

### The `complete-story.sh` Script
The script automatically uses the correct base branch:
```bash
./scripts/complete-story.sh 88  # Creates PR to epic-8-durability-commit
```

**If you manually create a PR, ALWAYS verify the base branch is the epic branch, not main.**

---

## Epic 8 Overview

### Scope
- Transaction commit path (validate → write WAL → apply to storage)
- Write application to storage layer
- WAL integration for durability
- Atomic commit guarantees
- Abort/rollback support

### Success Criteria
- [ ] TransactionContext.commit() validates and applies atomically
- [ ] All writes go to WAL before storage application
- [ ] Commit fails atomically if validation fails
- [ ] Abort cleans up properly
- [ ] All unit tests pass (>95% coverage)

### Component Breakdown
- **Story #88**: Transaction Commit Path 🔴 BLOCKS ALL Epic 8
- **Story #89**: Write Application
- **Story #90**: WAL Integration
- **Story #91**: Atomic Commit
- **Story #92**: Rollback Support

---

## Dependency Graph

```
Phase 1 (Sequential - CRITICAL):
  Story #88 (Transaction Commit Path)
    └─> 🔴 BLOCKS #89, #90

Phase 2 (Parallel - 2 Claudes after #88):
  Story #89 (Write Application)
  Story #90 (WAL Integration)
    └─> Both depend on #88
    └─> Independent of each other

Phase 3 (Sequential):
  Story #91 (Atomic Commit)
    └─> Depends on #89, #90

Phase 4 (Sequential):
  Story #92 (Rollback Support)
    └─> Depends on #91
```

---

## Parallelization Strategy

### Optimal Parallel Execution (2 Claudes)

| Phase | Duration | Claude 1 | Claude 2 |
|-------|----------|----------|----------|
| 1 | 4 hours | #88 Commit Path | - |
| 2 | 4 hours | #89 Write Application | #90 WAL Integration |
| 3 | 4 hours | #91 Atomic Commit | - |
| 4 | 3 hours | #92 Rollback Support | - |

**Total Wall Time**: ~15 hours (vs. ~18 hours sequential)

---

## Story #88: Transaction Commit Path

**GitHub Issue**: #88
**Estimated Time**: 4 hours
**Dependencies**: Epic 7 complete
**Blocks**: Stories #89, #90, #91, #92

### ⚠️ PREREQUISITE: Read the Semantics Spec

Before writing ANY code, read:
- Section 3.1-3.3: Conflict Detection and First-Committer-Wins
- Section 5: Replay Semantics
- Core Invariants: All-or-nothing commit, monotonic versions

### Semantics This Story Must Implement

From the spec:

| Invariant | Description |
|-----------|-------------|
| **All-or-nothing commit** | A transaction's writes either all succeed (commit) or all fail (abort). No partial application. |
| **First-committer-wins** | Based on READ-SET, not write-set |
| **Monotonic versions** | Version numbers never decrease |

### Start Story

```bash
./scripts/start-story.sh 8 88 commit-path
```

### Implementation Steps

#### Step 1: Add commit() method to TransactionContext

Update `crates/concurrency/src/transaction.rs`:

```rust
use crate::validation::{validate_transaction, ValidationResult};
use in_mem_core::traits::Storage;

impl TransactionContext {
    /// Commit the transaction
    ///
    /// Per spec Section 3 and Core Invariants:
    /// 1. Transition to Validating state
    /// 2. Run validation against current storage
    /// 3. If valid: apply writes, transition to Committed
    /// 4. If invalid: transition to Aborted
    ///
    /// # Arguments
    /// * `store` - Storage to validate against and apply writes to
    ///
    /// # Returns
    /// - Ok(commit_version) if transaction committed successfully
    /// - Err with ValidationResult if transaction aborted due to conflicts
    ///
    /// # Errors
    /// - TransactionAborted if validation fails
    /// - InvalidState if not in Active state
    pub fn commit<S: Storage>(&mut self, store: &S) -> Result<u64, CommitError> {
        // Step 1: Transition to Validating
        self.begin_validation()?;

        // Step 2: Validate against current storage state
        let validation_result = validate_transaction(self, store);

        if !validation_result.is_valid() {
            // Step 3a: Validation failed - abort
            self.abort(format!(
                "Commit failed: {} conflict(s) detected",
                validation_result.conflict_count()
            ))?;
            return Err(CommitError::ValidationFailed(validation_result));
        }

        // Step 3b: Validation passed - mark committed
        // Note: Actual write application is in Story #89
        self.mark_committed()?;

        // Return commit version (will be set properly in Story #91)
        Ok(self.start_version + 1)
    }
}

/// Error type for commit failures
#[derive(Debug, Clone)]
pub enum CommitError {
    /// Transaction aborted due to validation conflicts
    ValidationFailed(ValidationResult),
    /// Transaction was not in correct state for commit
    InvalidState(String),
}

impl std::fmt::Display for CommitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommitError::ValidationFailed(result) => {
                write!(f, "Commit failed: {} conflicts", result.conflict_count())
            }
            CommitError::InvalidState(msg) => write!(f, "Invalid state: {}", msg),
        }
    }
}

impl std::error::Error for CommitError {}
```

#### Step 2: Add state transition helpers

```rust
impl TransactionContext {
    /// Begin validation phase
    ///
    /// Transitions from Active to Validating state.
    fn begin_validation(&mut self) -> Result<(), CommitError> {
        if self.status != TransactionStatus::Active {
            return Err(CommitError::InvalidState(format!(
                "Cannot begin validation from {:?} state",
                self.status
            )));
        }
        self.status = TransactionStatus::Validating;
        Ok(())
    }

    /// Mark transaction as committed
    ///
    /// Transitions from Validating to Committed state.
    fn mark_committed(&mut self) -> Result<(), CommitError> {
        if self.status != TransactionStatus::Validating {
            return Err(CommitError::InvalidState(format!(
                "Cannot commit from {:?} state",
                self.status
            )));
        }
        self.status = TransactionStatus::Committed;
        Ok(())
    }
}
```

#### Step 3: Write unit tests

```rust
#[cfg(test)]
mod commit_tests {
    use super::*;
    use crate::{ClonedSnapshotView, ValidationResult};
    use in_mem_core::value::Value;
    use in_mem_storage::UnifiedStore;
    use std::collections::BTreeMap;

    fn create_test_namespace() -> Namespace {
        Namespace::new("t".into(), "a".into(), "g".into(), RunId::new())
    }

    fn create_key(ns: &Namespace, name: &[u8]) -> Key {
        Key::new(ns.clone(), TypeTag::KV, name.to_vec())
    }

    fn create_txn_with_store(store: &UnifiedStore) -> TransactionContext {
        let snapshot = store.create_snapshot();
        let run_id = RunId::new();
        TransactionContext::with_snapshot(store.current_version(), run_id, Box::new(snapshot))
    }

    #[test]
    fn test_commit_empty_transaction() {
        let store = UnifiedStore::new();
        let mut txn = create_txn_with_store(&store);

        let result = txn.commit(&store);

        assert!(result.is_ok());
        assert_eq!(txn.status, TransactionStatus::Committed);
    }

    #[test]
    fn test_commit_read_only_transaction() {
        let store = UnifiedStore::new();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key");

        store.put(key.clone(), Value::I64(100), None).unwrap();

        let mut txn = create_txn_with_store(&store);
        let _ = txn.get(&key).unwrap();

        // Read-only transactions always commit (per spec Section 3.2 Scenario 3)
        // UNLESS the read key was modified
        let result = txn.commit(&store);
        assert!(result.is_ok());
    }

    #[test]
    fn test_commit_with_blind_write() {
        let store = UnifiedStore::new();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key");

        store.put(key.clone(), Value::I64(100), None).unwrap();
        let start_version = store.current_version();

        let mut txn = create_txn_with_store(&store);
        // Blind write - no read first
        txn.put(key.clone(), Value::I64(200)).unwrap();

        // Concurrent modification
        store.put(key.clone(), Value::I64(300), None).unwrap();

        // Per spec Section 3.2 Scenario 1: Blind writes do NOT conflict
        let result = txn.commit(&store);
        assert!(result.is_ok());
    }

    #[test]
    fn test_commit_with_read_write_conflict() {
        let store = UnifiedStore::new();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key");

        store.put(key.clone(), Value::I64(100), None).unwrap();

        let mut txn = create_txn_with_store(&store);
        let _ = txn.get(&key).unwrap(); // Read adds to read_set
        txn.put(key.clone(), Value::I64(200)).unwrap();

        // Concurrent modification
        store.put(key.clone(), Value::I64(300), None).unwrap();

        // Per spec Section 3.1 Condition 1: Read-write conflict
        let result = txn.commit(&store);
        assert!(result.is_err());
        assert_eq!(txn.status, TransactionStatus::Aborted);

        if let Err(CommitError::ValidationFailed(validation)) = result {
            assert!(!validation.is_valid());
            assert_eq!(validation.conflict_count(), 1);
        } else {
            panic!("Expected ValidationFailed error");
        }
    }

    #[test]
    fn test_commit_with_cas_conflict() {
        let store = UnifiedStore::new();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"counter");

        store.put(key.clone(), Value::I64(0), None).unwrap();
        let v1 = store.get(&key).unwrap().unwrap().version;

        let mut txn = create_txn_with_store(&store);
        txn.cas(key.clone(), v1, Value::I64(1)).unwrap();

        // Concurrent modification
        store.put(key.clone(), Value::I64(100), None).unwrap();

        // Per spec Section 3.1 Condition 3: CAS conflict
        let result = txn.commit(&store);
        assert!(result.is_err());
        assert_eq!(txn.status, TransactionStatus::Aborted);
    }

    #[test]
    fn test_commit_first_committer_wins() {
        let store = UnifiedStore::new();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"shared");

        store.put(key.clone(), Value::String("initial".into()), None).unwrap();

        // T1 and T2 both read and write the same key
        let mut txn1 = create_txn_with_store(&store);
        let _ = txn1.get(&key).unwrap();
        txn1.put(key.clone(), Value::String("from_t1".into())).unwrap();

        let mut txn2 = create_txn_with_store(&store);
        let _ = txn2.get(&key).unwrap();
        txn2.put(key.clone(), Value::String("from_t2".into())).unwrap();

        // T1 commits first - should succeed
        let result1 = txn1.commit(&store);
        assert!(result1.is_ok());

        // Simulate T1's write being applied (will be in Story #89)
        store.put(key.clone(), Value::String("from_t1".into()), None).unwrap();

        // T2 tries to commit - should fail (read-set version changed)
        let result2 = txn2.commit(&store);
        assert!(result2.is_err());
        assert_eq!(txn2.status, TransactionStatus::Aborted);
    }

    #[test]
    fn test_cannot_commit_twice() {
        let store = UnifiedStore::new();
        let mut txn = create_txn_with_store(&store);

        let result1 = txn.commit(&store);
        assert!(result1.is_ok());

        let result2 = txn.commit(&store);
        assert!(result2.is_err());
    }

    #[test]
    fn test_cannot_commit_aborted_transaction() {
        let store = UnifiedStore::new();
        let mut txn = create_txn_with_store(&store);

        txn.abort("Manual abort".to_string()).unwrap();

        let result = txn.commit(&store);
        assert!(result.is_err());
    }
}
```

### Testing Requirements

**CRITICAL TESTING RULE**:
- **NEVER adjust tests to make them pass**
- If a test fails, the CODE must be fixed, not the test
- Tests define correct behavior - failed tests reveal bugs in implementation
- Only adjust a test if the test itself is incorrect (wrong assertion logic)

Run tests:
```bash
~/.cargo/bin/cargo test -p in-mem-concurrency
~/.cargo/bin/cargo clippy -p in-mem-concurrency -- -D warnings
~/.cargo/bin/cargo fmt --check
```

### Completion

```bash
./scripts/complete-story.sh 88
```

---

## Story #89: Write Application

**GitHub Issue**: #89
**Estimated Time**: 3 hours
**Dependencies**: Story #88 (Transaction Commit Path)
**Blocks**: Story #91

### ⚠️ PREREQUISITE: Read the Semantics Spec

Before writing ANY code, read:
- Section 5.2: Replay Rules (versions are preserved exactly)
- Section 6: Version Semantics (key versions, global version)
- Core Invariants: Monotonic versions

### Semantics This Story Must Implement

| Requirement | Description |
|-------------|-------------|
| **Monotonic versions** | Version numbers never decrease |
| **All writes at commit version** | All keys in a transaction get the same commit version |
| **Deletes create tombstones** | Per spec Section 6.5: deleted keys get versioned tombstones |

### Start Story

```bash
./scripts/start-story.sh 8 89 write-application
```

### Implementation Steps

#### Step 1: Create ApplyResult type

Add to `crates/concurrency/src/transaction.rs`:

```rust
/// Result of applying transaction writes to storage
#[derive(Debug, Clone)]
pub struct ApplyResult {
    /// Version assigned to all writes in this transaction
    pub commit_version: u64,
    /// Number of puts applied
    pub puts_applied: usize,
    /// Number of deletes applied
    pub deletes_applied: usize,
    /// Number of CAS operations applied
    pub cas_applied: usize,
}
```

#### Step 2: Add apply_writes method

```rust
impl TransactionContext {
    /// Apply all buffered writes to storage
    ///
    /// Per spec Section 6.1:
    /// - Global version incremented ONCE for the whole transaction
    /// - All keys in this transaction get the same commit version
    ///
    /// Per spec Section 6.5:
    /// - Deletes create tombstones with the commit version
    ///
    /// # Arguments
    /// * `store` - Storage to apply writes to
    /// * `commit_version` - Version to assign to all writes
    ///
    /// # Returns
    /// ApplyResult with counts of applied operations
    ///
    /// # Preconditions
    /// - Transaction must be in Committed state (validation passed)
    pub fn apply_writes<S: Storage>(
        &self,
        store: &S,
        commit_version: u64,
    ) -> Result<ApplyResult, Error> {
        if self.status != TransactionStatus::Committed {
            return Err(Error::InvalidState(format!(
                "Cannot apply writes from {:?} state - must be Committed",
                self.status
            )));
        }

        let mut result = ApplyResult {
            commit_version,
            puts_applied: 0,
            deletes_applied: 0,
            cas_applied: 0,
        };

        // Apply puts from write_set
        for (key, value) in &self.write_set {
            store.put_with_version(key.clone(), value.clone(), commit_version, None)?;
            result.puts_applied += 1;
        }

        // Apply deletes from delete_set
        for key in &self.delete_set {
            store.delete_with_version(key, commit_version)?;
            result.deletes_applied += 1;
        }

        // Apply CAS operations from cas_set
        for cas_op in &self.cas_set {
            store.put_with_version(
                cas_op.key.clone(),
                cas_op.new_value.clone(),
                commit_version,
                None,
            )?;
            result.cas_applied += 1;
        }

        Ok(result)
    }
}
```

#### Step 3: Add put_with_version and delete_with_version to Storage trait

Update `crates/core/src/traits.rs`:

```rust
pub trait Storage: Send + Sync {
    // ... existing methods ...

    /// Put a value with a specific version
    ///
    /// Used by transaction commit to apply writes with the commit version.
    fn put_with_version(
        &self,
        key: Key,
        value: Value,
        version: u64,
        ttl: Option<Duration>,
    ) -> Result<()>;

    /// Delete a key with a specific version (creates tombstone)
    ///
    /// Used by transaction commit to apply deletes with the commit version.
    fn delete_with_version(&self, key: &Key, version: u64) -> Result<()>;
}
```

#### Step 4: Implement in UnifiedStore

Update `crates/storage/src/unified.rs`:

```rust
impl Storage for UnifiedStore {
    fn put_with_version(
        &self,
        key: Key,
        value: Value,
        version: u64,
        ttl: Option<Duration>,
    ) -> Result<()> {
        // Implementation: set version directly instead of incrementing
        let mut data = self.data.write();
        let expires_at = ttl.map(|d| Instant::now() + d);
        let vv = VersionedValue::new(value, version, expires_at);

        // Update global version if this version is higher
        let current = self.version.load(Ordering::SeqCst);
        if version > current {
            self.version.store(version, Ordering::SeqCst);
        }

        data.insert(key.clone(), vv);

        // Update indices...
        Ok(())
    }

    fn delete_with_version(&self, key: &Key, version: u64) -> Result<()> {
        // Create tombstone with version
        let mut data = self.data.write();

        // Update global version if this version is higher
        let current = self.version.load(Ordering::SeqCst);
        if version > current {
            self.version.store(version, Ordering::SeqCst);
        }

        data.remove(key);

        // Update indices...
        Ok(())
    }
}
```

#### Step 5: Write unit tests

```rust
#[cfg(test)]
mod apply_writes_tests {
    use super::*;

    #[test]
    fn test_apply_writes_empty_transaction() {
        let store = UnifiedStore::new();
        let mut txn = create_txn_with_store(&store);
        txn.mark_committed().unwrap();

        let result = txn.apply_writes(&store, 100).unwrap();

        assert_eq!(result.commit_version, 100);
        assert_eq!(result.puts_applied, 0);
        assert_eq!(result.deletes_applied, 0);
        assert_eq!(result.cas_applied, 0);
    }

    #[test]
    fn test_apply_writes_single_put() {
        let store = UnifiedStore::new();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key");

        let mut txn = create_txn_with_store(&store);
        txn.put(key.clone(), Value::I64(42)).unwrap();
        txn.status = TransactionStatus::Committed; // Simulate validation passed

        let result = txn.apply_writes(&store, 100).unwrap();

        assert_eq!(result.puts_applied, 1);

        // Verify key was written with correct version
        let stored = store.get(&key).unwrap().unwrap();
        assert_eq!(stored.version, 100);
        assert_eq!(stored.value, Value::I64(42));
    }

    #[test]
    fn test_apply_writes_multiple_puts_same_version() {
        let store = UnifiedStore::new();
        let ns = create_test_namespace();
        let key1 = create_key(&ns, b"key1");
        let key2 = create_key(&ns, b"key2");
        let key3 = create_key(&ns, b"key3");

        let mut txn = create_txn_with_store(&store);
        txn.put(key1.clone(), Value::I64(1)).unwrap();
        txn.put(key2.clone(), Value::I64(2)).unwrap();
        txn.put(key3.clone(), Value::I64(3)).unwrap();
        txn.status = TransactionStatus::Committed;

        let result = txn.apply_writes(&store, 50).unwrap();

        assert_eq!(result.puts_applied, 3);

        // All keys should have same commit version
        assert_eq!(store.get(&key1).unwrap().unwrap().version, 50);
        assert_eq!(store.get(&key2).unwrap().unwrap().version, 50);
        assert_eq!(store.get(&key3).unwrap().unwrap().version, 50);
    }

    #[test]
    fn test_apply_writes_with_delete() {
        let store = UnifiedStore::new();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key");

        // Pre-existing key
        store.put(key.clone(), Value::I64(100), None).unwrap();

        let mut txn = create_txn_with_store(&store);
        txn.delete(key.clone()).unwrap();
        txn.status = TransactionStatus::Committed;

        let result = txn.apply_writes(&store, 50).unwrap();

        assert_eq!(result.deletes_applied, 1);
        assert!(store.get(&key).unwrap().is_none());
    }

    #[test]
    fn test_apply_writes_with_cas() {
        let store = UnifiedStore::new();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"counter");

        store.put(key.clone(), Value::I64(0), None).unwrap();
        let v1 = store.get(&key).unwrap().unwrap().version;

        let mut txn = create_txn_with_store(&store);
        txn.cas(key.clone(), v1, Value::I64(1)).unwrap();
        txn.status = TransactionStatus::Committed;

        let result = txn.apply_writes(&store, 50).unwrap();

        assert_eq!(result.cas_applied, 1);

        let stored = store.get(&key).unwrap().unwrap();
        assert_eq!(stored.version, 50);
        assert_eq!(stored.value, Value::I64(1));
    }

    #[test]
    fn test_apply_writes_fails_if_not_committed() {
        let store = UnifiedStore::new();
        let txn = create_txn_with_store(&store);

        // Transaction is still Active
        let result = txn.apply_writes(&store, 100);

        assert!(result.is_err());
    }

    #[test]
    fn test_apply_writes_updates_global_version() {
        let store = UnifiedStore::new();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key");

        let initial_version = store.current_version();

        let mut txn = create_txn_with_store(&store);
        txn.put(key.clone(), Value::I64(42)).unwrap();
        txn.status = TransactionStatus::Committed;

        txn.apply_writes(&store, 100).unwrap();

        assert!(store.current_version() >= 100);
    }
}
```

### Testing Requirements

**CRITICAL TESTING RULE**:
- **NEVER adjust tests to make them pass**
- If a test fails, the CODE must be fixed, not the test

Run tests:
```bash
~/.cargo/bin/cargo test -p in-mem-concurrency
~/.cargo/bin/cargo test -p in-mem-storage
~/.cargo/bin/cargo clippy --all -- -D warnings
```

### Completion

```bash
./scripts/complete-story.sh 89
```

---

## Story #90: WAL Integration

**GitHub Issue**: #90
**Estimated Time**: 4 hours
**Dependencies**: Story #88 (Transaction Commit Path)
**Blocks**: Story #91

### ⚠️ PREREQUISITE: Read the Semantics Spec

Before writing ANY code, read:
- Section 5: Replay Semantics (WAL format, recovery algorithm)
- Section 5.3: WAL Entry Format
- Section 5.5: Incomplete Transaction Handling

### Semantics This Story Must Implement

From the spec Section 5:

| Rule | Description |
|------|-------------|
| **WAL before storage** | All writes go to WAL before storage application |
| **CommitTxn marker** | Transaction is durable only when CommitTxn is written |
| **Incomplete = discard** | Transactions without CommitTxn are discarded on recovery |

### Start Story

```bash
./scripts/start-story.sh 8 90 wal-integration
```

### Implementation Steps

#### Step 1: Create TransactionWALWriter

Create `crates/concurrency/src/wal_writer.rs`:

```rust
//! WAL writer for transactions
//!
//! Writes transaction entries to WAL for durability.
//! Per spec Section 5:
//! - BeginTxn written at transaction start
//! - Write/Delete entries written during commit
//! - CommitTxn written to finalize (marks transaction durable)

use in_mem_durability::wal::{WAL, WALEntry};
use in_mem_core::error::Result;
use in_mem_core::types::RunId;
use in_mem_core::value::Timestamp;

/// Writes transaction operations to WAL
pub struct TransactionWALWriter<'a> {
    wal: &'a WAL,
    txn_id: u64,
    run_id: RunId,
}

impl<'a> TransactionWALWriter<'a> {
    /// Create a new WAL writer for a transaction
    pub fn new(wal: &'a WAL, txn_id: u64, run_id: RunId) -> Self {
        TransactionWALWriter { wal, txn_id, run_id }
    }

    /// Write BeginTxn entry
    pub fn write_begin(&self) -> Result<()> {
        let entry = WALEntry::BeginTxn {
            txn_id: self.txn_id,
            run_id: self.run_id,
            timestamp: Timestamp::now(),
        };
        self.wal.append(&entry)
    }

    /// Write a put operation
    pub fn write_put(&self, key: Key, value: Value, version: u64) -> Result<()> {
        let entry = WALEntry::Write {
            run_id: self.run_id,
            key,
            value,
            version,
        };
        self.wal.append(&entry)
    }

    /// Write a delete operation
    pub fn write_delete(&self, key: Key, version: u64) -> Result<()> {
        let entry = WALEntry::Delete {
            run_id: self.run_id,
            key,
            version,
        };
        self.wal.append(&entry)
    }

    /// Write CommitTxn entry (marks transaction as durable)
    pub fn write_commit(&self) -> Result<()> {
        let entry = WALEntry::CommitTxn {
            txn_id: self.txn_id,
            run_id: self.run_id,
        };
        self.wal.append(&entry)?;

        // Ensure commit marker is flushed to disk
        self.wal.flush()?;

        Ok(())
    }

    /// Write AbortTxn entry (optional for M2, but supported)
    pub fn write_abort(&self) -> Result<()> {
        let entry = WALEntry::AbortTxn {
            txn_id: self.txn_id,
            run_id: self.run_id,
        };
        self.wal.append(&entry)
    }
}
```

#### Step 2: Add write_to_wal method to TransactionContext

Add to `crates/concurrency/src/transaction.rs`:

```rust
use crate::wal_writer::TransactionWALWriter;

impl TransactionContext {
    /// Write all transaction operations to WAL
    ///
    /// Per spec Section 5:
    /// - Write/Delete entries for all buffered operations
    /// - Version numbers are preserved exactly
    ///
    /// # Arguments
    /// * `wal_writer` - WAL writer configured for this transaction
    /// * `commit_version` - Version to assign to all writes
    ///
    /// # Preconditions
    /// - Transaction must be in Committed state (validation passed)
    pub fn write_to_wal(
        &self,
        wal_writer: &TransactionWALWriter,
        commit_version: u64,
    ) -> Result<()> {
        if self.status != TransactionStatus::Committed {
            return Err(Error::InvalidState(format!(
                "Cannot write to WAL from {:?} state - must be Committed",
                self.status
            )));
        }

        // Write puts
        for (key, value) in &self.write_set {
            wal_writer.write_put(key.clone(), value.clone(), commit_version)?;
        }

        // Write deletes
        for key in &self.delete_set {
            wal_writer.write_delete(key.clone(), commit_version)?;
        }

        // Write CAS operations (as puts with the new value)
        for cas_op in &self.cas_set {
            wal_writer.write_put(
                cas_op.key.clone(),
                cas_op.new_value.clone(),
                commit_version,
            )?;
        }

        Ok(())
    }
}
```

#### Step 3: Write unit tests

```rust
#[cfg(test)]
mod wal_integration_tests {
    use super::*;
    use in_mem_durability::wal::WAL;
    use tempfile::TempDir;

    fn create_test_wal() -> (WAL, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");
        let wal = WAL::open(&wal_path).unwrap();
        (wal, temp_dir)
    }

    #[test]
    fn test_write_to_wal_empty_transaction() {
        let (wal, _temp) = create_test_wal();
        let store = UnifiedStore::new();
        let run_id = RunId::new();

        let mut txn = TransactionContext::with_snapshot(
            store.current_version(),
            run_id,
            Box::new(store.create_snapshot()),
        );
        txn.status = TransactionStatus::Committed;

        let writer = TransactionWALWriter::new(&wal, 1, run_id);
        writer.write_begin().unwrap();
        txn.write_to_wal(&writer, 100).unwrap();
        writer.write_commit().unwrap();

        // Verify WAL entries
        let entries = wal.read_all().unwrap();
        assert_eq!(entries.len(), 2); // BeginTxn + CommitTxn
    }

    #[test]
    fn test_write_to_wal_with_puts() {
        let (wal, _temp) = create_test_wal();
        let store = UnifiedStore::new();
        let ns = create_test_namespace();
        let key1 = create_key(&ns, b"key1");
        let key2 = create_key(&ns, b"key2");
        let run_id = RunId::new();

        let mut txn = TransactionContext::with_snapshot(
            store.current_version(),
            run_id,
            Box::new(store.create_snapshot()),
        );
        txn.put(key1.clone(), Value::I64(1)).unwrap();
        txn.put(key2.clone(), Value::I64(2)).unwrap();
        txn.status = TransactionStatus::Committed;

        let writer = TransactionWALWriter::new(&wal, 1, run_id);
        writer.write_begin().unwrap();
        txn.write_to_wal(&writer, 100).unwrap();
        writer.write_commit().unwrap();

        let entries = wal.read_all().unwrap();
        assert_eq!(entries.len(), 4); // BeginTxn + 2 Write + CommitTxn

        // Verify write entries have correct version
        for entry in &entries {
            if let WALEntry::Write { version, .. } = entry {
                assert_eq!(*version, 100);
            }
        }
    }

    #[test]
    fn test_write_to_wal_with_delete() {
        let (wal, _temp) = create_test_wal();
        let store = UnifiedStore::new();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key");
        let run_id = RunId::new();

        store.put(key.clone(), Value::I64(100), None).unwrap();

        let mut txn = TransactionContext::with_snapshot(
            store.current_version(),
            run_id,
            Box::new(store.create_snapshot()),
        );
        txn.delete(key.clone()).unwrap();
        txn.status = TransactionStatus::Committed;

        let writer = TransactionWALWriter::new(&wal, 1, run_id);
        writer.write_begin().unwrap();
        txn.write_to_wal(&writer, 100).unwrap();
        writer.write_commit().unwrap();

        let entries = wal.read_all().unwrap();
        assert_eq!(entries.len(), 3); // BeginTxn + Delete + CommitTxn

        // Verify delete entry
        let delete_entry = &entries[1];
        if let WALEntry::Delete { version, .. } = delete_entry {
            assert_eq!(*version, 100);
        } else {
            panic!("Expected Delete entry");
        }
    }

    #[test]
    fn test_write_to_wal_fails_if_not_committed() {
        let (wal, _temp) = create_test_wal();
        let store = UnifiedStore::new();
        let run_id = RunId::new();

        let txn = TransactionContext::with_snapshot(
            store.current_version(),
            run_id,
            Box::new(store.create_snapshot()),
        );

        let writer = TransactionWALWriter::new(&wal, 1, run_id);
        let result = txn.write_to_wal(&writer, 100);

        assert!(result.is_err());
    }

    #[test]
    fn test_wal_entries_include_run_id() {
        let (wal, _temp) = create_test_wal();
        let store = UnifiedStore::new();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key");
        let run_id = RunId::new();

        let mut txn = TransactionContext::with_snapshot(
            store.current_version(),
            run_id,
            Box::new(store.create_snapshot()),
        );
        txn.put(key.clone(), Value::I64(42)).unwrap();
        txn.status = TransactionStatus::Committed;

        let writer = TransactionWALWriter::new(&wal, 1, run_id);
        writer.write_begin().unwrap();
        txn.write_to_wal(&writer, 100).unwrap();
        writer.write_commit().unwrap();

        let entries = wal.read_all().unwrap();

        // All entries should have the same run_id
        for entry in &entries {
            assert_eq!(entry.run_id(), Some(run_id));
        }
    }
}
```

### Testing Requirements

**CRITICAL TESTING RULE**:
- **NEVER adjust tests to make them pass**
- If a test fails, the CODE must be fixed, not the test

Run tests:
```bash
~/.cargo/bin/cargo test -p in-mem-concurrency
~/.cargo/bin/cargo clippy -p in-mem-concurrency -- -D warnings
```

### Completion

```bash
./scripts/complete-story.sh 90
```

---

## Story #91: Atomic Commit

**GitHub Issue**: #91
**Estimated Time**: 4 hours
**Dependencies**: Stories #89, #90
**Blocks**: Story #92

### ⚠️ PREREQUISITE: Read the Semantics Spec

Before writing ANY code, read:
- Core Invariants: All-or-nothing commit, no partial commits
- Section 5: Replay Semantics
- Section 6.1: Global Version Counter

### Semantics This Story Must Implement

| Invariant | Description |
|-----------|-------------|
| **All-or-nothing commit** | Transaction writes either ALL succeed or ALL fail |
| **No partial commits** | If crash occurs mid-commit, recovery discards incomplete |
| **WAL before storage** | Durability: WAL written before storage application |

### Commit Sequence (CRITICAL ORDER)

```
1. begin_validation() - Change state to Validating
2. validate_transaction() - Check for conflicts
3. IF conflicts: abort() and return error
4. mark_committed() - Change state to Committed
5. Allocate commit_version (increment global version)
6. write_begin() to WAL - BeginTxn entry
7. write_to_wal() - Write/Delete entries with commit_version
8. write_commit() to WAL - CommitTxn entry (DURABILITY POINT)
9. apply_writes() to storage - Apply to in-memory storage
10. Return Ok(commit_version)
```

**If crash occurs before step 8**: Transaction is not durable, will be discarded on recovery.
**If crash occurs after step 8**: Transaction is durable, will be replayed on recovery.

### Start Story

```bash
./scripts/start-story.sh 8 91 atomic-commit
```

### Implementation Steps

#### Step 1: Create TransactionManager

Create `crates/concurrency/src/manager.rs`:

```rust
//! Transaction manager for coordinating commit operations
//!
//! Provides atomic commit by orchestrating:
//! 1. Validation
//! 2. WAL writing
//! 3. Storage application
//!
//! Per spec Core Invariants:
//! - All-or-nothing commit
//! - WAL before storage for durability

use crate::{TransactionContext, CommitError};
use crate::wal_writer::TransactionWALWriter;
use in_mem_core::error::Result;
use in_mem_core::traits::Storage;
use in_mem_durability::wal::WAL;
use std::sync::atomic::{AtomicU64, Ordering};

/// Manages transaction lifecycle and atomic commits
pub struct TransactionManager {
    /// Global version counter
    version: AtomicU64,
    /// Next transaction ID
    next_txn_id: AtomicU64,
}

impl TransactionManager {
    /// Create a new transaction manager
    pub fn new(initial_version: u64) -> Self {
        TransactionManager {
            version: AtomicU64::new(initial_version),
            next_txn_id: AtomicU64::new(1),
        }
    }

    /// Get current global version
    pub fn current_version(&self) -> u64 {
        self.version.load(Ordering::SeqCst)
    }

    /// Allocate next transaction ID
    pub fn next_txn_id(&self) -> u64 {
        self.next_txn_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Allocate next commit version (increment global version)
    fn allocate_commit_version(&self) -> u64 {
        self.version.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Commit a transaction atomically
    ///
    /// Per spec Core Invariants:
    /// - Validates transaction (first-committer-wins)
    /// - Writes to WAL for durability
    /// - Applies to storage only after WAL is durable
    /// - All-or-nothing: either all writes succeed or transaction aborts
    ///
    /// # Arguments
    /// * `txn` - Transaction to commit (must be in Active state)
    /// * `store` - Storage to validate against and apply writes to
    /// * `wal` - WAL for durability
    ///
    /// # Returns
    /// - Ok(commit_version) on success
    /// - Err(CommitError) if validation fails or WAL write fails
    pub fn commit<S: Storage>(
        &self,
        txn: &mut TransactionContext,
        store: &S,
        wal: &WAL,
    ) -> std::result::Result<u64, CommitError> {
        // Step 1-4: Validate and mark committed (in-memory)
        txn.commit(store)?;

        // Step 5: Allocate commit version
        let commit_version = self.allocate_commit_version();

        // Step 6-8: Write to WAL (durability)
        let txn_id = self.next_txn_id();
        let wal_writer = TransactionWALWriter::new(wal, txn_id, txn.run_id);

        // If WAL write fails, we need to handle this carefully
        // For M2, we'll treat WAL failure as commit failure
        if let Err(e) = wal_writer.write_begin() {
            // Revert to aborted state
            txn.status = TransactionStatus::Aborted;
            txn.abort_reason = Some(format!("WAL write failed: {}", e));
            return Err(CommitError::WALError(e.to_string()));
        }

        if let Err(e) = txn.write_to_wal(&wal_writer, commit_version) {
            txn.status = TransactionStatus::Aborted;
            txn.abort_reason = Some(format!("WAL write failed: {}", e));
            return Err(CommitError::WALError(e.to_string()));
        }

        if let Err(e) = wal_writer.write_commit() {
            txn.status = TransactionStatus::Aborted;
            txn.abort_reason = Some(format!("WAL commit failed: {}", e));
            return Err(CommitError::WALError(e.to_string()));
        }

        // DURABILITY POINT: Transaction is now durable
        // Even if we crash after this, recovery will replay from WAL

        // Step 9: Apply to storage
        if let Err(e) = txn.apply_writes(store, commit_version) {
            // This is a serious error - WAL says committed but storage failed
            // Log error but return success since WAL is authoritative
            eprintln!("WARNING: Storage application failed after WAL commit: {}", e);
        }

        // Step 10: Return commit version
        Ok(commit_version)
    }
}
```

#### Step 2: Add WALError variant to CommitError

Update `crates/concurrency/src/transaction.rs`:

```rust
#[derive(Debug, Clone)]
pub enum CommitError {
    ValidationFailed(ValidationResult),
    InvalidState(String),
    /// WAL write failed
    WALError(String),
}
```

#### Step 3: Write integration tests

```rust
#[cfg(test)]
mod atomic_commit_tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_env() -> (TransactionManager, UnifiedStore, WAL, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");
        let wal = WAL::open(&wal_path).unwrap();
        let store = UnifiedStore::new();
        let manager = TransactionManager::new(store.current_version());
        (manager, store, wal, temp_dir)
    }

    #[test]
    fn test_atomic_commit_success() {
        let (manager, store, wal, _temp) = setup_test_env();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key");
        let run_id = RunId::new();

        let mut txn = TransactionContext::with_snapshot(
            manager.current_version(),
            run_id,
            Box::new(store.create_snapshot()),
        );
        txn.put(key.clone(), Value::I64(42)).unwrap();

        let result = manager.commit(&mut txn, &store, &wal);

        assert!(result.is_ok());
        let commit_version = result.unwrap();

        // Verify storage was updated
        let stored = store.get(&key).unwrap().unwrap();
        assert_eq!(stored.value, Value::I64(42));
        assert_eq!(stored.version, commit_version);

        // Verify WAL was written
        let entries = wal.read_all().unwrap();
        assert!(entries.len() >= 3); // BeginTxn + Write + CommitTxn
    }

    #[test]
    fn test_atomic_commit_validation_failure() {
        let (manager, store, wal, _temp) = setup_test_env();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key");
        let run_id = RunId::new();

        store.put(key.clone(), Value::I64(100), None).unwrap();

        let mut txn = TransactionContext::with_snapshot(
            manager.current_version(),
            run_id,
            Box::new(store.create_snapshot()),
        );
        let _ = txn.get(&key).unwrap(); // Read adds to read_set
        txn.put(key.clone(), Value::I64(200)).unwrap();

        // Concurrent modification
        store.put(key.clone(), Value::I64(300), None).unwrap();

        let result = manager.commit(&mut txn, &store, &wal);

        assert!(result.is_err());
        assert_eq!(txn.status, TransactionStatus::Aborted);

        // WAL should NOT have entries for this failed transaction
        let entries = wal.read_all().unwrap();
        assert!(entries.is_empty() || !entries.iter().any(|e| e.run_id() == Some(run_id)));
    }

    #[test]
    fn test_atomic_commit_version_increment() {
        let (manager, store, wal, _temp) = setup_test_env();
        let ns = create_test_namespace();
        let key1 = create_key(&ns, b"key1");
        let key2 = create_key(&ns, b"key2");

        let initial_version = manager.current_version();

        // First transaction
        let run_id1 = RunId::new();
        let mut txn1 = TransactionContext::with_snapshot(
            manager.current_version(),
            run_id1,
            Box::new(store.create_snapshot()),
        );
        txn1.put(key1.clone(), Value::I64(1)).unwrap();
        let v1 = manager.commit(&mut txn1, &store, &wal).unwrap();

        // Second transaction
        let run_id2 = RunId::new();
        let mut txn2 = TransactionContext::with_snapshot(
            manager.current_version(),
            run_id2,
            Box::new(store.create_snapshot()),
        );
        txn2.put(key2.clone(), Value::I64(2)).unwrap();
        let v2 = manager.commit(&mut txn2, &store, &wal).unwrap();

        // Versions should be monotonically increasing
        assert!(v1 > initial_version);
        assert!(v2 > v1);
    }

    #[test]
    fn test_atomic_commit_all_keys_same_version() {
        let (manager, store, wal, _temp) = setup_test_env();
        let ns = create_test_namespace();
        let key1 = create_key(&ns, b"key1");
        let key2 = create_key(&ns, b"key2");
        let key3 = create_key(&ns, b"key3");
        let run_id = RunId::new();

        let mut txn = TransactionContext::with_snapshot(
            manager.current_version(),
            run_id,
            Box::new(store.create_snapshot()),
        );
        txn.put(key1.clone(), Value::I64(1)).unwrap();
        txn.put(key2.clone(), Value::I64(2)).unwrap();
        txn.put(key3.clone(), Value::I64(3)).unwrap();

        let commit_version = manager.commit(&mut txn, &store, &wal).unwrap();

        // Per spec Section 6.1: All keys in a transaction get the same commit version
        assert_eq!(store.get(&key1).unwrap().unwrap().version, commit_version);
        assert_eq!(store.get(&key2).unwrap().unwrap().version, commit_version);
        assert_eq!(store.get(&key3).unwrap().unwrap().version, commit_version);
    }

    #[test]
    fn test_first_committer_wins_with_manager() {
        let (manager, store, wal, _temp) = setup_test_env();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"shared");

        store.put(key.clone(), Value::String("initial".into()), None).unwrap();

        // Both transactions read and write same key
        let run_id1 = RunId::new();
        let mut txn1 = TransactionContext::with_snapshot(
            manager.current_version(),
            run_id1,
            Box::new(store.create_snapshot()),
        );
        let _ = txn1.get(&key).unwrap();
        txn1.put(key.clone(), Value::String("from_t1".into())).unwrap();

        let run_id2 = RunId::new();
        let mut txn2 = TransactionContext::with_snapshot(
            manager.current_version(),
            run_id2,
            Box::new(store.create_snapshot()),
        );
        let _ = txn2.get(&key).unwrap();
        txn2.put(key.clone(), Value::String("from_t2".into())).unwrap();

        // T1 commits first - succeeds
        let result1 = manager.commit(&mut txn1, &store, &wal);
        assert!(result1.is_ok());

        // T2 commits second - fails due to read-write conflict
        let result2 = manager.commit(&mut txn2, &store, &wal);
        assert!(result2.is_err());

        // Final value should be from T1
        let stored = store.get(&key).unwrap().unwrap();
        assert_eq!(stored.value, Value::String("from_t1".into()));
    }
}
```

### Testing Requirements

**CRITICAL TESTING RULE**:
- **NEVER adjust tests to make them pass**
- If a test fails, the CODE must be fixed, not the test

Run tests:
```bash
~/.cargo/bin/cargo test -p in-mem-concurrency
~/.cargo/bin/cargo clippy --all -- -D warnings
```

### Completion

```bash
./scripts/complete-story.sh 91
```

---

## Story #92: Rollback Support

**GitHub Issue**: #92
**Estimated Time**: 3 hours
**Dependencies**: Story #91 (Atomic Commit)
**Blocks**: Epic 9

### ⚠️ PREREQUISITE: Read the Semantics Spec

Before writing ANY code, read:
- Section 5.3: WAL Entry Format (AbortTxn)
- Section 5.5: Incomplete Transaction Handling
- Appendix A.3: Why No AbortTxn WAL Entry (M2)?

### Semantics This Story Must Implement

From the spec:

| Scenario | Behavior |
|----------|----------|
| **Explicit abort** | Clear write/delete/cas sets, set status to Aborted |
| **Validation failure** | Same as explicit abort |
| **WAL write failure** | Same as explicit abort |
| **No AbortTxn in WAL** | Per spec, aborted transactions write nothing |

### Start Story

```bash
./scripts/start-story.sh 8 92 rollback-support
```

### Implementation Steps

#### Step 1: Enhance abort() method

Update `crates/concurrency/src/transaction.rs`:

```rust
impl TransactionContext {
    /// Abort the transaction and clean up
    ///
    /// Per spec:
    /// - Aborted transactions write nothing to storage
    /// - Aborted transactions write nothing to WAL (M2)
    /// - All buffered operations are discarded
    ///
    /// # Arguments
    /// * `reason` - Human-readable reason for abort
    ///
    /// # State Transitions
    /// - Active → Aborted
    /// - Validating → Aborted
    pub fn abort(&mut self, reason: String) -> Result<()> {
        match self.status {
            TransactionStatus::Active | TransactionStatus::Validating => {
                self.status = TransactionStatus::Aborted;
                self.abort_reason = Some(reason);

                // Clear all buffered operations
                self.write_set.clear();
                self.delete_set.clear();
                self.cas_set.clear();

                // Note: read_set is kept for debugging/diagnostics

                Ok(())
            }
            TransactionStatus::Committed => {
                Err(Error::InvalidState(
                    "Cannot abort already committed transaction".to_string(),
                ))
            }
            TransactionStatus::Aborted => {
                Err(Error::InvalidState(
                    "Transaction already aborted".to_string(),
                ))
            }
        }
    }

    /// Check if transaction can still be rolled back
    pub fn can_rollback(&self) -> bool {
        matches!(
            self.status,
            TransactionStatus::Active | TransactionStatus::Validating
        )
    }

    /// Get operations that would be rolled back
    ///
    /// Useful for debugging/logging before abort.
    pub fn pending_operations(&self) -> PendingOperations {
        PendingOperations {
            puts: self.write_set.len(),
            deletes: self.delete_set.len(),
            cas: self.cas_set.len(),
        }
    }
}

/// Summary of pending operations that would be rolled back
#[derive(Debug, Clone, Copy)]
pub struct PendingOperations {
    pub puts: usize,
    pub deletes: usize,
    pub cas: usize,
}

impl PendingOperations {
    pub fn total(&self) -> usize {
        self.puts + self.deletes + self.cas
    }

    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }
}
```

#### Step 2: Add rollback support to TransactionManager

Update `crates/concurrency/src/manager.rs`:

```rust
impl TransactionManager {
    /// Explicitly abort a transaction
    ///
    /// Per spec:
    /// - No AbortTxn entry written to WAL in M2
    /// - All buffered operations discarded
    /// - Transaction marked as Aborted
    ///
    /// # Arguments
    /// * `txn` - Transaction to abort
    /// * `reason` - Human-readable reason for abort
    pub fn abort(&self, txn: &mut TransactionContext, reason: String) -> Result<()> {
        txn.abort(reason)
    }

    /// Commit with automatic rollback on failure
    ///
    /// Ensures transaction is properly cleaned up if commit fails.
    pub fn commit_or_rollback<S: Storage>(
        &self,
        txn: &mut TransactionContext,
        store: &S,
        wal: &WAL,
    ) -> std::result::Result<u64, CommitError> {
        match self.commit(txn, store, wal) {
            Ok(version) => Ok(version),
            Err(e) => {
                // Ensure transaction is in Aborted state
                if txn.can_rollback() {
                    let _ = txn.abort(format!("Commit failed: {}", e));
                }
                Err(e)
            }
        }
    }
}
```

#### Step 3: Write unit tests

```rust
#[cfg(test)]
mod rollback_tests {
    use super::*;

    #[test]
    fn test_abort_clears_write_set() {
        let store = UnifiedStore::new();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key");

        let mut txn = create_txn_with_store(&store);
        txn.put(key.clone(), Value::I64(42)).unwrap();

        assert_eq!(txn.write_count(), 1);

        txn.abort("Test abort".to_string()).unwrap();

        assert_eq!(txn.write_count(), 0);
        assert_eq!(txn.status, TransactionStatus::Aborted);
    }

    #[test]
    fn test_abort_clears_delete_set() {
        let store = UnifiedStore::new();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key");

        store.put(key.clone(), Value::I64(100), None).unwrap();

        let mut txn = create_txn_with_store(&store);
        txn.delete(key.clone()).unwrap();

        assert_eq!(txn.delete_count(), 1);

        txn.abort("Test abort".to_string()).unwrap();

        assert_eq!(txn.delete_count(), 0);
    }

    #[test]
    fn test_abort_clears_cas_set() {
        let store = UnifiedStore::new();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"counter");

        store.put(key.clone(), Value::I64(0), None).unwrap();
        let version = store.get(&key).unwrap().unwrap().version;

        let mut txn = create_txn_with_store(&store);
        txn.cas(key.clone(), version, Value::I64(1)).unwrap();

        assert_eq!(txn.cas_count(), 1);

        txn.abort("Test abort".to_string()).unwrap();

        assert_eq!(txn.cas_count(), 0);
    }

    #[test]
    fn test_abort_preserves_abort_reason() {
        let store = UnifiedStore::new();
        let mut txn = create_txn_with_store(&store);

        txn.abort("Custom reason".to_string()).unwrap();

        assert_eq!(txn.abort_reason, Some("Custom reason".to_string()));
    }

    #[test]
    fn test_cannot_abort_committed_transaction() {
        let store = UnifiedStore::new();
        let mut txn = create_txn_with_store(&store);

        txn.commit(&store).unwrap();

        let result = txn.abort("Too late".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_cannot_abort_twice() {
        let store = UnifiedStore::new();
        let mut txn = create_txn_with_store(&store);

        txn.abort("First abort".to_string()).unwrap();

        let result = txn.abort("Second abort".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_can_rollback_from_active() {
        let store = UnifiedStore::new();
        let txn = create_txn_with_store(&store);

        assert!(txn.can_rollback());
    }

    #[test]
    fn test_can_rollback_from_validating() {
        let store = UnifiedStore::new();
        let mut txn = create_txn_with_store(&store);
        txn.status = TransactionStatus::Validating;

        assert!(txn.can_rollback());
    }

    #[test]
    fn test_cannot_rollback_committed() {
        let store = UnifiedStore::new();
        let mut txn = create_txn_with_store(&store);
        txn.status = TransactionStatus::Committed;

        assert!(!txn.can_rollback());
    }

    #[test]
    fn test_pending_operations() {
        let store = UnifiedStore::new();
        let ns = create_test_namespace();
        let key1 = create_key(&ns, b"key1");
        let key2 = create_key(&ns, b"key2");
        let key3 = create_key(&ns, b"key3");

        store.put(key2.clone(), Value::I64(100), None).unwrap();
        store.put(key3.clone(), Value::I64(0), None).unwrap();
        let v3 = store.get(&key3).unwrap().unwrap().version;

        let mut txn = create_txn_with_store(&store);
        txn.put(key1.clone(), Value::I64(1)).unwrap();
        txn.delete(key2.clone()).unwrap();
        txn.cas(key3.clone(), v3, Value::I64(1)).unwrap();

        let pending = txn.pending_operations();
        assert_eq!(pending.puts, 1);
        assert_eq!(pending.deletes, 1);
        assert_eq!(pending.cas, 1);
        assert_eq!(pending.total(), 3);
    }

    #[test]
    fn test_commit_or_rollback_success() {
        let (manager, store, wal, _temp) = setup_test_env();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key");
        let run_id = RunId::new();

        let mut txn = TransactionContext::with_snapshot(
            manager.current_version(),
            run_id,
            Box::new(store.create_snapshot()),
        );
        txn.put(key.clone(), Value::I64(42)).unwrap();

        let result = manager.commit_or_rollback(&mut txn, &store, &wal);

        assert!(result.is_ok());
        assert_eq!(txn.status, TransactionStatus::Committed);
    }

    #[test]
    fn test_commit_or_rollback_failure_cleans_up() {
        let (manager, store, wal, _temp) = setup_test_env();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key");
        let run_id = RunId::new();

        store.put(key.clone(), Value::I64(100), None).unwrap();

        let mut txn = TransactionContext::with_snapshot(
            manager.current_version(),
            run_id,
            Box::new(store.create_snapshot()),
        );
        let _ = txn.get(&key).unwrap();
        txn.put(key.clone(), Value::I64(200)).unwrap();

        // Concurrent modification causes conflict
        store.put(key.clone(), Value::I64(300), None).unwrap();

        let result = manager.commit_or_rollback(&mut txn, &store, &wal);

        assert!(result.is_err());
        assert_eq!(txn.status, TransactionStatus::Aborted);
        // Write set should be cleared
        assert_eq!(txn.write_count(), 0);
    }
}
```

### Testing Requirements

**CRITICAL TESTING RULE**:
- **NEVER adjust tests to make them pass**
- If a test fails, the CODE must be fixed, not the test

Run tests:
```bash
~/.cargo/bin/cargo test -p in-mem-concurrency
~/.cargo/bin/cargo clippy --all -- -D warnings
~/.cargo/bin/cargo fmt --check
```

### Completion

```bash
./scripts/complete-story.sh 92
```

---

## Epic Completion

After all stories are merged to `epic-8-durability-commit`:

### Final Validation

```bash
git checkout epic-8-durability-commit
~/.cargo/bin/cargo test --all
~/.cargo/bin/cargo clippy --all -- -D warnings
~/.cargo/bin/cargo fmt --check
```

### Merge to develop

```bash
git checkout develop
git merge --no-ff epic-8-durability-commit -m "Epic 8: Durability & Commit Complete

Implements Epic 8 (Stories #88-#92):
- #88: Transaction Commit Path
- #89: Write Application
- #90: WAL Integration
- #91: Atomic Commit
- #92: Rollback Support

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
git push origin develop
```

### Update Status

Update `docs/milestones/M2_PROJECT_STATUS.md`:
- Mark Epic 8 as complete
- Update progress: 3/7 epics complete, 15/32 stories complete
- Update "Next Steps" to point to Epic 9

---

## Critical Notes

### 🔴 SPEC COMPLIANCE IS MANDATORY

**Every line of M2 code must comply with `docs/architecture/M2_TRANSACTION_SEMANTICS.md`.**

During code review, verify:
- [ ] All-or-nothing commit (no partial application)
- [ ] WAL written before storage application
- [ ] First-committer-wins based on read-set
- [ ] Monotonic version numbers
- [ ] Aborted transactions write nothing to WAL (M2)
- [ ] All keys in a transaction get same commit version

**If ANY behavior deviates from the spec, the code MUST be rejected.**

### Key Spec Rules for Epic 8

From Core Invariants and Section 5:

1. **All-or-nothing commit**: Either all writes succeed or transaction aborts
2. **WAL before storage**: Durability requires WAL to be written first
3. **CommitTxn = durable**: Transaction is only durable when CommitTxn is in WAL
4. **Recovery discards incomplete**: Transactions without CommitTxn are discarded
5. **Monotonic versions**: Version numbers never decrease
6. **Single commit version**: All keys in a transaction get the same version

### Architecture

Epic 8 adds to `crates/concurrency`:
- `wal_writer.rs`: TransactionWALWriter for WAL integration
- `manager.rs`: TransactionManager for coordinating commit
- Updated `transaction.rs`: commit(), apply_writes(), abort()

### Summary

Epic 8 establishes durable, atomic commits for M2:
- TransactionContext.commit() validates and commits atomically
- TransactionManager coordinates validation, WAL, and storage
- WAL integration ensures durability (WAL before storage)
- Rollback support cleans up failed transactions
- All-or-nothing semantics enforced throughout

**After Epic 8**: Commit path is complete. Ready for Epic 9 (Recovery Support).
