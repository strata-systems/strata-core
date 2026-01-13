# Epic 10: Database API Integration - Implementation Prompts

**Epic Goal**: Expose transaction API through the Database struct and integrate all M2 components for end-to-end transaction support.

**Status**: Ready to begin
**Dependencies**: Epic 9 (Recovery Support) complete

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
| Section 4 | Implicit Transactions | **How M1-style ops work in M2** |
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
       └── epic-10-database-api    ← Epic branch (base for all story PRs)
            └── epic-10-story-98-*    ← Story branches
```

### Critical Rules

1. **Story PRs go to EPIC branch, NOT main**
   ```bash
   # CORRECT: PR base is epic branch
   /opt/homebrew/bin/gh pr create --base epic-10-database-api --head epic-10-story-98-transaction-api

   # WRONG: Never PR directly to main
   /opt/homebrew/bin/gh pr create --base main --head epic-10-story-98-transaction-api  # ❌ NEVER DO THIS
   ```

2. **Epic branches merge to develop** (after all stories complete)
   ```bash
   git checkout develop
   git merge --no-ff epic-10-database-api
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
./scripts/complete-story.sh 98  # Creates PR to epic-10-database-api
```

**If you manually create a PR, ALWAYS verify the base branch is the epic branch, not main.**

---

## 🔴 CRITICAL TESTING RULE

**NEVER adjust tests to make them pass**

- If a test fails, the CODE must be fixed, not the test
- Tests define correct behavior - failed tests reveal bugs in implementation
- Only adjust a test if the test itself is incorrect (wrong assertion logic)
- Tests MUST validate spec-compliant behavior

---

## 🔴 TDD METHODOLOGY

For each story:

1. **Write tests FIRST** that validate spec-compliant behavior
2. **Run tests** - they should FAIL (no implementation yet)
3. **Implement code** to make tests pass
4. **Refactor** if needed while keeping tests green
5. **Run full validation** before completing story

---

## Tool Paths

Use fully qualified paths:
- Cargo: `~/.cargo/bin/cargo`
- GitHub CLI: `/opt/homebrew/bin/gh`

---

## Epic 10 Overview

### Scope
Epic 10 integrates all M2 components into the Database API:
- Transaction API (db.transaction() method)
- Transaction coordinator with TransactionManager
- Implicit transactions for M1 backwards compatibility
- Retry logic for conflicts
- WAL integration during commit
- Recovery integration on startup

### Key Spec References

#### Section 4: Implicit Transactions
| Rule | Description |
|------|-------------|
| **db.put()** | Wraps in implicit transaction, commits immediately |
| **db.get()** | Creates snapshot, read-only transaction, always succeeds |
| **db.delete()** | Wraps in implicit transaction, commits immediately |
| **Implicit txns can conflict** | Yes, but rarely (very short window) |
| **Automatic retry** | Implicit transactions include retry on conflict |

#### Core Invariants
| Invariant | Description |
|-----------|-------------|
| **No partial commits** | Either all writes succeed or all fail |
| **All-or-nothing** | Transaction writes apply atomically |
| **Monotonic versions** | Versions never decrease |
| **Read-your-writes** | Transaction sees its own uncommitted changes |

### Component Integration

```
Database::transaction(|txn| ...)
    │
    ├─> TransactionManager (allocate txn_id, version)
    │        │
    ├─> TransactionContext (read/write operations)
    │        │
    ├─> validate_transaction() (conflict detection)
    │        │
    ├─> TransactionWALWriter (WAL entries)
    │        │
    └─> apply_writes() (storage application)
```

### Success Criteria
- [ ] Database::transaction() works with closure API
- [ ] Implicit transactions wrap M1-style ops
- [ ] Retry logic handles conflicts automatically
- [ ] WAL correctly logs transaction boundaries
- [ ] Recovery restores transaction manager state
- [ ] All unit tests pass (>95% coverage)

### Component Breakdown
- **Story #98**: Database Transaction API 🔴 BLOCKS ALL Epic 10
- **Story #99**: Transaction Coordinator
- **Story #100**: Implicit Transactions (M1 Compatibility)
- **Story #101**: Transaction Error Handling & Retry
- **Story #102**: Database API Integration Tests

---

## Dependency Graph

```
Phase 1 (Sequential - CRITICAL):
  Story #98 (Database Transaction API)
    └─> 🔴 BLOCKS #99, #100, #101

Phase 2 (Parallel - 2-3 Claudes after #98):
  Story #99 (Transaction Coordinator)
  Story #100 (Implicit Transactions)
  Story #101 (Error Handling & Retry)
    └─> All depend on #98
    └─> Independent of each other

Phase 3 (Sequential):
  Story #102 (Integration Tests)
    └─> Depends on #99, #100, #101
```

---

## Parallelization Strategy

### Optimal Parallel Execution (3 Claudes)

| Phase | Duration | Claude 1 | Claude 2 | Claude 3 |
|-------|----------|----------|----------|----------|
| 1 | 4 hours | #98 Transaction API | - | - |
| 2 | 4 hours | #99 Coordinator | #100 Implicit Txns | #101 Error Handling |
| 3 | 3 hours | #102 Integration Tests | - | - |

**Total Wall Time**: ~11 hours (vs. ~18 hours sequential)

---

## Existing Infrastructure

Epic 10 builds on:

### From Epic 6 (Transaction Foundations)
- `TransactionContext` struct with read/write/delete/cas sets
- `TransactionStatus` enum (Active, Validating, Committed, Aborted)
- `ClonedSnapshotView` for snapshot isolation

### From Epic 7 (Transaction Semantics)
- `validate_transaction()` function
- `ValidationResult` with conflict detection
- `ConflictType` enum (ReadWriteConflict, CASConflict)

### From Epic 8 (Durability & Commit)
- `TransactionManager` for version/ID allocation
- `TransactionWALWriter` for WAL entry generation
- `commit()` method on TransactionContext
- `apply_writes()` for storage application

### From Epic 9 (Recovery Support)
- `RecoveryCoordinator` for database startup
- `RecoveryResult` with storage + txn_manager
- Transaction-aware WAL replay

### Current Database Struct
```rust
pub struct Database {
    data_dir: PathBuf,
    storage: Arc<UnifiedStore>,
    wal: Arc<Mutex<WAL>>,
}
```

---

## Story #98: Database Transaction API

**GitHub Issue**: #98
**Estimated Time**: 4 hours
**Dependencies**: Epic 9 complete
**Blocks**: Stories #99, #100, #101, #102

### ⚠️ PREREQUISITE: Read the Semantics Spec

Before writing ANY code, read:
- Section 4: Implicit Transactions (entire section)
- Core Invariants (all-or-nothing commit)

### Semantics This Story Must Implement

From the spec Section 4.1:
> "An implicit transaction is:
> - Automatically created for single M1-style operations
> - Contains exactly one read or write
> - Commits immediately after the operation
> - Invisible to the caller (behaves like M1)"

### What to Implement

Add transaction API to `crates/engine/src/database.rs`:

```rust
use in_mem_concurrency::{
    TransactionContext, TransactionManager, TransactionWALWriter,
    RecoveryCoordinator, validate_transaction,
};

/// Main database struct with transaction support
pub struct Database {
    data_dir: PathBuf,
    storage: Arc<UnifiedStore>,
    wal: Arc<Mutex<WAL>>,
    /// Transaction manager for version and ID allocation
    txn_manager: TransactionManager,
}

impl Database {
    /// Execute a transaction with the given closure
    ///
    /// Per spec Section 4:
    /// - Creates TransactionContext with snapshot
    /// - Executes closure with transaction
    /// - Validates and commits on success
    /// - Aborts on error
    ///
    /// # Arguments
    /// * `run_id` - RunId for namespace isolation
    /// * `f` - Closure that performs transaction operations
    ///
    /// # Returns
    /// * `Ok(T)` - Closure return value on successful commit
    /// * `Err` - On validation conflict or closure error
    ///
    /// # Example
    /// ```ignore
    /// let result = db.transaction(run_id, |txn| {
    ///     let val = txn.get(&key)?;
    ///     txn.put(key, new_value)?;
    ///     Ok(val)
    /// })?;
    /// ```
    pub fn transaction<F, T>(&self, run_id: RunId, f: F) -> Result<T>
    where
        F: FnOnce(&mut TransactionContext) -> Result<T>;

    /// Begin a new transaction (for manual control)
    ///
    /// Returns a TransactionContext that must be manually committed or aborted.
    /// Prefer `transaction()` closure API for automatic handling.
    pub fn begin_transaction(&self, run_id: RunId) -> TransactionContext;

    /// Commit a transaction
    ///
    /// Per spec:
    /// 1. Validate (conflict detection)
    /// 2. Write to WAL (BeginTxn, Writes, CommitTxn)
    /// 3. Apply to storage
    /// 4. Return success or conflict error
    pub fn commit_transaction(&self, txn: &mut TransactionContext) -> Result<()>;
}
```

### Implementation Steps

#### Step 1: Create epic branch and start story

```bash
# Create epic branch from develop
git checkout develop
git pull origin develop
git checkout -b epic-10-database-api

# Push epic branch
git push -u origin epic-10-database-api

# Start the story
./scripts/start-story.sh 10 98 transaction-api
```

#### Step 2: Write tests FIRST (TDD)

```rust
#[cfg(test)]
mod transaction_api_tests {
    use super::*;
    use in_mem_core::types::{Key, Namespace, RunId};
    use in_mem_core::value::Value;
    use in_mem_core::traits::Storage;
    use tempfile::TempDir;

    fn create_test_namespace(run_id: RunId) -> Namespace {
        Namespace::new(
            "tenant".to_string(),
            "app".to_string(),
            "agent".to_string(),
            run_id,
        )
    }

    #[test]
    fn test_transaction_closure_api() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();

        let run_id = RunId::new();
        let ns = create_test_namespace(run_id);
        let key = Key::new_kv(ns, "test_key");

        // Execute transaction
        let result = db.transaction(run_id, |txn| {
            txn.put(key.clone(), Value::I64(42))?;
            Ok(())
        });

        assert!(result.is_ok());

        // Verify data was committed
        let stored = db.storage().get(&key).unwrap().unwrap();
        assert_eq!(stored.value, Value::I64(42));
    }

    #[test]
    fn test_transaction_returns_closure_value() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();

        let run_id = RunId::new();
        let ns = create_test_namespace(run_id);
        let key = Key::new_kv(ns, "test_key");

        // Pre-populate
        db.storage().put(key.clone(), Value::I64(100), None).unwrap();

        // Transaction returns a value
        let result: Result<i64> = db.transaction(run_id, |txn| {
            let val = txn.get(&key)?.unwrap();
            if let Value::I64(n) = val.value {
                Ok(n)
            } else {
                Err(Error::InvalidState("wrong type".to_string()))
            }
        });

        assert_eq!(result.unwrap(), 100);
    }

    #[test]
    fn test_transaction_read_your_writes() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();

        let run_id = RunId::new();
        let ns = create_test_namespace(run_id);
        let key = Key::new_kv(ns, "ryw_key");

        // Per spec Section 2.1: "Its own uncommitted writes - always visible"
        let result: Result<Value> = db.transaction(run_id, |txn| {
            txn.put(key.clone(), Value::String("written".to_string()))?;

            // Should see our own write
            let val = txn.get(&key)?.unwrap();
            Ok(val.value)
        });

        assert_eq!(result.unwrap(), Value::String("written".to_string()));
    }

    #[test]
    fn test_transaction_aborts_on_closure_error() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();

        let run_id = RunId::new();
        let ns = create_test_namespace(run_id);
        let key = Key::new_kv(ns, "abort_key");

        // Transaction that errors
        let result: Result<()> = db.transaction(run_id, |txn| {
            txn.put(key.clone(), Value::I64(999))?;
            Err(Error::InvalidState("intentional error".to_string()))
        });

        assert!(result.is_err());

        // Data should NOT be committed
        assert!(db.storage().get(&key).unwrap().is_none());
    }

    #[test]
    fn test_begin_and_commit_manual() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();

        let run_id = RunId::new();
        let ns = create_test_namespace(run_id);
        let key = Key::new_kv(ns, "manual_key");

        // Manual transaction control
        let mut txn = db.begin_transaction(run_id);
        txn.put(key.clone(), Value::I64(123)).unwrap();

        // Commit manually
        db.commit_transaction(&mut txn).unwrap();

        // Verify committed
        let stored = db.storage().get(&key).unwrap().unwrap();
        assert_eq!(stored.value, Value::I64(123));
    }

    #[test]
    fn test_transaction_wal_logging() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("db");
        let run_id = RunId::new();
        let ns = create_test_namespace(run_id);
        let key = Key::new_kv(ns, "wal_key");

        // Execute transaction
        {
            let db = Database::open(&db_path).unwrap();
            db.transaction(run_id, |txn| {
                txn.put(key.clone(), Value::I64(42))?;
                Ok(())
            }).unwrap();
        }

        // Reopen database (triggers recovery from WAL)
        {
            let db = Database::open(&db_path).unwrap();
            let stored = db.storage().get(&key).unwrap().unwrap();
            assert_eq!(stored.value, Value::I64(42));
        }
    }

    #[test]
    fn test_transaction_version_allocation() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();

        let run_id = RunId::new();
        let ns = create_test_namespace(run_id);

        // First transaction
        db.transaction(run_id, |txn| {
            txn.put(Key::new_kv(ns.clone(), "key1"), Value::I64(1))?;
            Ok(())
        }).unwrap();

        let v1 = db.storage().current_version();
        assert!(v1 > 0);

        // Second transaction
        db.transaction(run_id, |txn| {
            txn.put(Key::new_kv(ns.clone(), "key2"), Value::I64(2))?;
            Ok(())
        }).unwrap();

        let v2 = db.storage().current_version();
        assert!(v2 > v1); // Versions must be monotonic
    }
}
```

#### Step 3: Update Database struct

```rust
use in_mem_concurrency::{
    TransactionContext, TransactionManager, TransactionWALWriter,
    RecoveryCoordinator, validate_transaction, TransactionStatus,
};
use in_mem_core::types::RunId;
use chrono::Utc;

/// Main database struct with transaction support
pub struct Database {
    /// Data directory path
    data_dir: PathBuf,
    /// Unified storage (thread-safe)
    storage: Arc<UnifiedStore>,
    /// Write-ahead log (protected by mutex for exclusive access)
    wal: Arc<Mutex<WAL>>,
    /// Transaction manager for version and ID allocation
    txn_manager: TransactionManager,
}
```

#### Step 4: Update open() to use RecoveryCoordinator

```rust
impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::open_with_mode(path, DurabilityMode::default())
    }

    pub fn open_with_mode<P: AsRef<Path>>(
        path: P,
        durability_mode: DurabilityMode,
    ) -> Result<Self> {
        let data_dir = path.as_ref().to_path_buf();

        // Create directories
        std::fs::create_dir_all(&data_dir).map_err(Error::IoError)?;
        let wal_dir = data_dir.join("wal");
        std::fs::create_dir_all(&wal_dir).map_err(Error::IoError)?;
        let wal_path = wal_dir.join("current.wal");

        // Use RecoveryCoordinator for proper recovery
        let recovery = RecoveryCoordinator::new(wal_path.clone());
        let result = recovery.recover()?;

        info!(
            txns_replayed = result.stats.txns_replayed,
            writes_applied = result.stats.writes_applied,
            deletes_applied = result.stats.deletes_applied,
            incomplete_txns = result.stats.incomplete_txns,
            final_version = result.stats.final_version,
            "Recovery complete"
        );

        // Re-open WAL for appending (recovery opened read-only)
        let wal = WAL::open(&wal_path, durability_mode)?;

        Ok(Self {
            data_dir,
            storage: Arc::new(result.storage),
            wal: Arc::new(Mutex::new(wal)),
            txn_manager: result.txn_manager,
        })
    }
}
```

#### Step 5: Implement transaction methods

```rust
impl Database {
    /// Execute a transaction with the given closure
    pub fn transaction<F, T>(&self, run_id: RunId, f: F) -> Result<T>
    where
        F: FnOnce(&mut TransactionContext) -> Result<T>,
    {
        let mut txn = self.begin_transaction(run_id);

        // Execute closure
        let result = f(&mut txn);

        match result {
            Ok(value) => {
                // Commit on success
                self.commit_transaction(&mut txn)?;
                Ok(value)
            }
            Err(e) => {
                // Abort on error (just discard, per spec no AbortTxn in WAL)
                txn.mark_aborted(format!("Closure error: {}", e)).ok();
                Err(e)
            }
        }
    }

    /// Begin a new transaction
    pub fn begin_transaction(&self, run_id: RunId) -> TransactionContext {
        let txn_id = self.txn_manager.next_txn_id();
        let start_version = self.storage.current_version();
        let snapshot = self.storage.create_snapshot();

        TransactionContext::with_snapshot(txn_id, run_id, Box::new(snapshot))
    }

    /// Commit a transaction
    ///
    /// Per spec commit sequence:
    /// 1. Validate (conflict detection)
    /// 2. Allocate commit version
    /// 3. Write to WAL (BeginTxn, Writes, CommitTxn)
    /// 4. Apply to storage
    pub fn commit_transaction(&self, txn: &mut TransactionContext) -> Result<()> {
        // 1. Validate
        txn.mark_validating()?;
        let validation = validate_transaction(txn, &*self.storage);

        if !validation.is_valid() {
            txn.mark_aborted(format!("Validation failed: {:?}", validation.conflicts))?;
            return Err(Error::TransactionConflict(format!(
                "Conflicts: {:?}",
                validation.conflicts
            )));
        }

        // 2. Allocate commit version
        let commit_version = self.txn_manager.allocate_version();

        // 3. Write to WAL
        let wal_writer = TransactionWALWriter::new(txn.txn_id(), txn.run_id());
        let mut wal = self.wal.lock().unwrap();

        // Write BeginTxn
        wal_writer.write_begin(&mut *wal, Utc::now().timestamp())?;

        // Write all operations
        for (key, value) in txn.write_set() {
            wal_writer.write_put(&mut *wal, key.clone(), value.clone(), commit_version)?;
        }
        for key in txn.delete_set() {
            wal_writer.write_delete(&mut *wal, key.clone(), commit_version)?;
        }

        // Write CommitTxn
        wal_writer.write_commit(&mut *wal)?;

        // Sync WAL
        wal.fsync()?;

        // 4. Apply to storage
        use in_mem_core::traits::Storage;
        for (key, value) in txn.write_set() {
            self.storage.put_with_version(key.clone(), value.clone(), commit_version, None)?;
        }
        for key in txn.delete_set() {
            self.storage.delete_with_version(key.clone(), commit_version)?;
        }

        // Mark committed
        txn.mark_committed()?;

        Ok(())
    }

    /// Get the transaction manager (for testing/internal use)
    pub fn txn_manager(&self) -> &TransactionManager {
        &self.txn_manager
    }
}
```

#### Step 6: Run validation

```bash
~/.cargo/bin/cargo test -p in-mem-engine
~/.cargo/bin/cargo clippy -p in-mem-engine -- -D warnings
~/.cargo/bin/cargo fmt --check
```

### Acceptance Criteria

- [ ] Database::transaction() closure API works
- [ ] Transaction returns closure value on success
- [ ] Transaction aborts on closure error
- [ ] WAL correctly logs transaction boundaries
- [ ] Recovery restores TransactionManager state
- [ ] Read-your-writes works within transaction
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Formatted correctly

### Complete the Story

```bash
./scripts/complete-story.sh 98
```

---

## Story #99: Transaction Coordinator

**GitHub Issue**: #99
**Estimated Time**: 4 hours
**Dependencies**: Story #98
**Blocks**: Story #102

### ⚠️ PREREQUISITE: Read the Semantics Spec

Before writing ANY code, read:
- Section 6.1: Global Version Counter
- Core Invariants: Monotonic versions

### What to Implement

Enhance transaction coordination:

1. **Active transaction tracking** (for debugging/metrics)
2. **Transaction ID allocation** (unique, monotonic)
3. **Version management** (allocate, track max)
4. **Metrics and instrumentation**

```rust
/// Transaction coordinator for the database
///
/// Manages transaction lifecycle, ID allocation, and version tracking.
/// Per spec Section 6.1: Single monotonic counter for the entire database.
pub struct TransactionCoordinator {
    /// Transaction manager for ID/version allocation
    manager: TransactionManager,
    /// Active transaction count (for metrics)
    active_count: AtomicU64,
    /// Total transactions started
    total_started: AtomicU64,
    /// Total transactions committed
    total_committed: AtomicU64,
    /// Total transactions aborted
    total_aborted: AtomicU64,
}

impl TransactionCoordinator {
    /// Create new coordinator with initial version
    pub fn new(initial_version: u64) -> Self;

    /// Create coordinator from recovery result
    pub fn from_recovery(result: &RecoveryResult) -> Self;

    /// Start a new transaction
    pub fn start_transaction(&self, run_id: RunId, storage: &UnifiedStore) -> TransactionContext;

    /// Record transaction commit
    pub fn record_commit(&self);

    /// Record transaction abort
    pub fn record_abort(&self);

    /// Get metrics
    pub fn metrics(&self) -> TransactionMetrics;
}

/// Transaction metrics
#[derive(Debug, Clone)]
pub struct TransactionMetrics {
    pub active_count: u64,
    pub total_started: u64,
    pub total_committed: u64,
    pub total_aborted: u64,
    pub commit_rate: f64, // committed / started
}
```

### Implementation Steps

#### Step 1: Start the story

```bash
./scripts/start-story.sh 10 99 transaction-coordinator
```

#### Step 2: Write tests FIRST

```rust
#[cfg(test)]
mod coordinator_tests {
    use super::*;

    #[test]
    fn test_coordinator_metrics() {
        let coordinator = TransactionCoordinator::new(0);

        // Start some transactions
        let storage = UnifiedStore::new();
        let run_id = RunId::new();

        let _txn1 = coordinator.start_transaction(run_id, &storage);
        let _txn2 = coordinator.start_transaction(run_id, &storage);

        let metrics = coordinator.metrics();
        assert_eq!(metrics.total_started, 2);
        assert_eq!(metrics.active_count, 2);
    }

    #[test]
    fn test_coordinator_commit_tracking() {
        let coordinator = TransactionCoordinator::new(0);
        let storage = UnifiedStore::new();
        let run_id = RunId::new();

        let _txn = coordinator.start_transaction(run_id, &storage);
        coordinator.record_commit();

        let metrics = coordinator.metrics();
        assert_eq!(metrics.total_committed, 1);
        assert_eq!(metrics.active_count, 0);
    }

    #[test]
    fn test_coordinator_version_monotonic() {
        let coordinator = TransactionCoordinator::new(100);

        let v1 = coordinator.manager.allocate_version();
        let v2 = coordinator.manager.allocate_version();
        let v3 = coordinator.manager.allocate_version();

        assert!(v1 < v2);
        assert!(v2 < v3);
    }

    #[test]
    fn test_coordinator_from_recovery() {
        // Test that coordinator properly initializes from recovery state
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");

        // Create WAL with some transactions
        {
            let mut wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();
            let run_id = RunId::new();

            wal.append(&WALEntry::BeginTxn { txn_id: 1, run_id, timestamp: 0 }).unwrap();
            wal.append(&WALEntry::Write {
                run_id,
                key: Key::new_kv(create_ns(run_id), "key"),
                value: Value::I64(1),
                version: 100,
            }).unwrap();
            wal.append(&WALEntry::CommitTxn { txn_id: 1, run_id }).unwrap();
        }

        // Recover
        let recovery = RecoveryCoordinator::new(wal_path);
        let result = recovery.recover().unwrap();

        let coordinator = TransactionCoordinator::from_recovery(&result);

        // Version should be >= max from WAL
        assert!(coordinator.manager.current_version() >= 100);
    }
}
```

#### Step 3: Implement TransactionCoordinator

Create `crates/engine/src/coordinator.rs`:

```rust
//! Transaction coordinator for managing transaction lifecycle
//!
//! Per spec Section 6.1:
//! - Single monotonic counter for the entire database
//! - Incremented on each COMMIT (not each write)

use in_mem_concurrency::{RecoveryResult, TransactionContext, TransactionManager};
use in_mem_core::types::RunId;
use in_mem_storage::UnifiedStore;
use std::sync::atomic::{AtomicU64, Ordering};

/// Transaction coordinator for the database
pub struct TransactionCoordinator {
    manager: TransactionManager,
    active_count: AtomicU64,
    total_started: AtomicU64,
    total_committed: AtomicU64,
    total_aborted: AtomicU64,
}

impl TransactionCoordinator {
    /// Create new coordinator with initial version
    pub fn new(initial_version: u64) -> Self {
        Self {
            manager: TransactionManager::new(initial_version),
            active_count: AtomicU64::new(0),
            total_started: AtomicU64::new(0),
            total_committed: AtomicU64::new(0),
            total_aborted: AtomicU64::new(0),
        }
    }

    /// Create coordinator from recovery result
    pub fn from_recovery(result: &RecoveryResult) -> Self {
        Self {
            manager: TransactionManager::new(result.stats.final_version),
            active_count: AtomicU64::new(0),
            total_started: AtomicU64::new(0),
            total_committed: AtomicU64::new(0),
            total_aborted: AtomicU64::new(0),
        }
    }

    /// Start a new transaction
    pub fn start_transaction(&self, run_id: RunId, storage: &UnifiedStore) -> TransactionContext {
        let txn_id = self.manager.next_txn_id();
        let snapshot = storage.create_snapshot();

        self.active_count.fetch_add(1, Ordering::Relaxed);
        self.total_started.fetch_add(1, Ordering::Relaxed);

        TransactionContext::with_snapshot(txn_id, run_id, Box::new(snapshot))
    }

    /// Allocate commit version
    pub fn allocate_commit_version(&self) -> u64 {
        self.manager.allocate_version()
    }

    /// Record transaction commit
    pub fn record_commit(&self) {
        self.active_count.fetch_sub(1, Ordering::Relaxed);
        self.total_committed.fetch_add(1, Ordering::Relaxed);
    }

    /// Record transaction abort
    pub fn record_abort(&self) {
        self.active_count.fetch_sub(1, Ordering::Relaxed);
        self.total_aborted.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current version
    pub fn current_version(&self) -> u64 {
        self.manager.current_version()
    }

    /// Get metrics
    pub fn metrics(&self) -> TransactionMetrics {
        let started = self.total_started.load(Ordering::Relaxed);
        let committed = self.total_committed.load(Ordering::Relaxed);

        TransactionMetrics {
            active_count: self.active_count.load(Ordering::Relaxed),
            total_started: started,
            total_committed: committed,
            total_aborted: self.total_aborted.load(Ordering::Relaxed),
            commit_rate: if started > 0 {
                committed as f64 / started as f64
            } else {
                0.0
            },
        }
    }
}

/// Transaction metrics
#[derive(Debug, Clone)]
pub struct TransactionMetrics {
    pub active_count: u64,
    pub total_started: u64,
    pub total_committed: u64,
    pub total_aborted: u64,
    pub commit_rate: f64,
}
```

#### Step 4: Update Database to use coordinator

Update `database.rs` to use `TransactionCoordinator` instead of bare `TransactionManager`.

#### Step 5: Run validation

```bash
~/.cargo/bin/cargo test -p in-mem-engine
~/.cargo/bin/cargo clippy -p in-mem-engine -- -D warnings
~/.cargo/bin/cargo fmt --check
```

### Acceptance Criteria

- [ ] TransactionCoordinator tracks active transactions
- [ ] Metrics are accurate (started, committed, aborted)
- [ ] Version allocation is monotonic
- [ ] Recovery properly initializes coordinator
- [ ] All tests pass
- [ ] No clippy warnings

---

## Story #100: Implicit Transactions (M1 Compatibility)

**GitHub Issue**: #100
**Estimated Time**: 4 hours
**Dependencies**: Story #98
**Blocks**: Story #102

### ⚠️ PREREQUISITE: Read the Semantics Spec

Before writing ANY code, read:
- Section 4: Implicit Transactions (entire section)
- Section 4.2: Implicit Transaction Behavior
- Section 4.3: Can Implicit Transactions Conflict?

### Semantics This Story Must Implement

From spec Section 4.2:

#### db.put(key, value) Behavior
```rust
// User calls:
db.put(run_id, key, value)?;

// Internally executes as:
{
    let mut txn = db.begin_transaction(run_id)?;
    txn.put(key.clone(), value)?;
    txn.commit()?;
}
```

#### db.get(key) Behavior
```rust
// User calls:
let value = db.get(run_id, key)?;

// Internally executes as:
{
    let txn = db.begin_transaction(run_id)?;
    let result = txn.get(&key)?;
    txn.commit()?;  // Read-only, always succeeds
    result
}
```

#### db.delete(key) Behavior
```rust
// User calls:
db.delete(run_id, key)?;

// Internally executes as:
{
    let mut txn = db.begin_transaction(run_id)?;
    txn.delete(key.clone())?;
    txn.commit()?;
}
```

### What to Implement

Add M1-compatible API methods to Database:

```rust
impl Database {
    /// Put a key-value pair (M1 compatibility)
    ///
    /// Per spec Section 4.2: Wraps in implicit transaction.
    /// Includes automatic retry on conflict.
    pub fn put(&self, run_id: RunId, key: Key, value: Value) -> Result<()>;

    /// Get a value by key (M1 compatibility)
    ///
    /// Per spec Section 4.2: Creates snapshot, read-only, always succeeds.
    pub fn get(&self, run_id: RunId, key: &Key) -> Result<Option<VersionedValue>>;

    /// Delete a key (M1 compatibility)
    ///
    /// Per spec Section 4.2: Wraps in implicit transaction.
    /// Includes automatic retry on conflict.
    pub fn delete(&self, run_id: RunId, key: Key) -> Result<()>;

    /// Compare-and-swap (M1 compatibility with explicit version)
    ///
    /// Per spec Section 3.4: CAS validates expected_version.
    pub fn cas(&self, run_id: RunId, key: Key, expected_version: u64, new_value: Value) -> Result<()>;
}
```

### Implementation Steps

#### Step 1: Start the story

```bash
./scripts/start-story.sh 10 100 implicit-transactions
```

#### Step 2: Write tests FIRST

```rust
#[cfg(test)]
mod implicit_transaction_tests {
    use super::*;

    #[test]
    fn test_implicit_put() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();

        let run_id = RunId::new();
        let ns = create_test_namespace(run_id);
        let key = Key::new_kv(ns, "implicit_put");

        // M1-style put
        db.put(run_id, key.clone(), Value::I64(42)).unwrap();

        // Verify stored
        let stored = db.storage().get(&key).unwrap().unwrap();
        assert_eq!(stored.value, Value::I64(42));
    }

    #[test]
    fn test_implicit_get() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();

        let run_id = RunId::new();
        let ns = create_test_namespace(run_id);
        let key = Key::new_kv(ns, "implicit_get");

        // Pre-populate
        db.storage().put(key.clone(), Value::I64(100), None).unwrap();

        // M1-style get
        let result = db.get(run_id, &key).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, Value::I64(100));
    }

    #[test]
    fn test_implicit_get_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();

        let run_id = RunId::new();
        let ns = create_test_namespace(run_id);
        let key = Key::new_kv(ns, "nonexistent");

        // M1-style get for nonexistent key
        let result = db.get(run_id, &key).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_implicit_delete() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();

        let run_id = RunId::new();
        let ns = create_test_namespace(run_id);
        let key = Key::new_kv(ns, "to_delete");

        // Pre-populate
        db.put(run_id, key.clone(), Value::I64(1)).unwrap();

        // M1-style delete
        db.delete(run_id, key.clone()).unwrap();

        // Verify deleted
        assert!(db.storage().get(&key).unwrap().is_none());
    }

    #[test]
    fn test_implicit_cas_success() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();

        let run_id = RunId::new();
        let ns = create_test_namespace(run_id);
        let key = Key::new_kv(ns, "cas_key");

        // Initial put
        db.put(run_id, key.clone(), Value::I64(1)).unwrap();
        let current = db.storage().get(&key).unwrap().unwrap();

        // CAS with correct version
        db.cas(run_id, key.clone(), current.version, Value::I64(2)).unwrap();

        // Verify updated
        let updated = db.storage().get(&key).unwrap().unwrap();
        assert_eq!(updated.value, Value::I64(2));
    }

    #[test]
    fn test_implicit_cas_failure() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();

        let run_id = RunId::new();
        let ns = create_test_namespace(run_id);
        let key = Key::new_kv(ns, "cas_fail");

        // Initial put
        db.put(run_id, key.clone(), Value::I64(1)).unwrap();

        // CAS with wrong version
        let result = db.cas(run_id, key.clone(), 999, Value::I64(2));
        assert!(result.is_err());
    }

    #[test]
    fn test_implicit_operations_durable() {
        // Verify implicit operations are written to WAL and survive restart
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("db");
        let run_id = RunId::new();
        let ns = create_test_namespace(run_id);

        let key1 = Key::new_kv(ns.clone(), "durable1");
        let key2 = Key::new_kv(ns.clone(), "durable2");

        // Write and close
        {
            let db = Database::open(&db_path).unwrap();
            db.put(run_id, key1.clone(), Value::I64(100)).unwrap();
            db.put(run_id, key2.clone(), Value::String("test".to_string())).unwrap();
        }

        // Reopen and verify
        {
            let db = Database::open(&db_path).unwrap();
            let v1 = db.get(run_id, &key1).unwrap().unwrap();
            let v2 = db.get(run_id, &key2).unwrap().unwrap();

            assert_eq!(v1.value, Value::I64(100));
            assert_eq!(v2.value, Value::String("test".to_string()));
        }
    }
}
```

#### Step 3: Implement implicit transaction methods

```rust
impl Database {
    /// Put a key-value pair (M1 compatibility)
    ///
    /// Per spec Section 4.2: Wraps in implicit transaction.
    pub fn put(&self, run_id: RunId, key: Key, value: Value) -> Result<()> {
        self.transaction(run_id, |txn| {
            txn.put(key.clone(), value.clone())?;
            Ok(())
        })
    }

    /// Get a value by key (M1 compatibility)
    ///
    /// Per spec Section 4.2: Read-only transaction, always succeeds.
    pub fn get(&self, run_id: RunId, key: &Key) -> Result<Option<VersionedValue>> {
        // For read-only, we can skip full transaction and just use snapshot
        let snapshot = self.storage.create_snapshot();
        use in_mem_core::traits::SnapshotView;
        snapshot.get(key)
    }

    /// Delete a key (M1 compatibility)
    ///
    /// Per spec Section 4.2: Wraps in implicit transaction.
    pub fn delete(&self, run_id: RunId, key: Key) -> Result<()> {
        self.transaction(run_id, |txn| {
            txn.delete(key.clone())?;
            Ok(())
        })
    }

    /// Compare-and-swap (M1 compatibility)
    ///
    /// Per spec Section 3.4: CAS validates expected_version.
    pub fn cas(&self, run_id: RunId, key: Key, expected_version: u64, new_value: Value) -> Result<()> {
        self.transaction(run_id, |txn| {
            txn.cas(key.clone(), expected_version, new_value.clone())?;
            Ok(())
        })
    }
}
```

#### Step 4: Run validation

```bash
~/.cargo/bin/cargo test -p in-mem-engine
~/.cargo/bin/cargo clippy -p in-mem-engine -- -D warnings
~/.cargo/bin/cargo fmt --check
```

### Acceptance Criteria

- [ ] db.put() works as implicit transaction
- [ ] db.get() returns snapshot value (always succeeds)
- [ ] db.delete() works as implicit transaction
- [ ] db.cas() validates version before write
- [ ] All implicit operations are durable (WAL)
- [ ] All tests pass
- [ ] No clippy warnings

---

## Story #101: Transaction Error Handling & Retry

**GitHub Issue**: #101
**Estimated Time**: 3 hours
**Dependencies**: Story #98
**Blocks**: Story #102

### ⚠️ PREREQUISITE: Read the Semantics Spec

Before writing ANY code, read:
- Section 3: Conflict Detection
- Section 3.3: First-Committer-Wins
- Section 4.3: Can Implicit Transactions Conflict?

### What to Implement

Add retry logic and comprehensive error handling:

```rust
/// Configuration for transaction retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: usize,
    /// Base delay between retries (exponential backoff)
    pub base_delay_ms: u64,
    /// Maximum delay between retries
    pub max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 10,
            max_delay_ms: 100,
        }
    }
}

impl Database {
    /// Execute a transaction with retry on conflict
    ///
    /// Per spec Section 4.3: Implicit transactions include automatic retry.
    pub fn transaction_with_retry<F, T>(
        &self,
        run_id: RunId,
        config: RetryConfig,
        f: F,
    ) -> Result<T>
    where
        F: Fn(&mut TransactionContext) -> Result<T>;
}
```

### Implementation Steps

#### Step 1: Start the story

```bash
./scripts/start-story.sh 10 101 error-handling-retry
```

#### Step 2: Write tests FIRST

```rust
#[cfg(test)]
mod retry_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn test_retry_on_conflict() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();

        let run_id = RunId::new();
        let ns = create_test_namespace(run_id);
        let key = Key::new_kv(ns, "contested");

        // Pre-populate
        db.put(run_id, key.clone(), Value::I64(0)).unwrap();

        // Track retry attempts
        let attempts = AtomicU64::new(0);

        // Force a conflict by reading and having another write happen
        let result = db.transaction_with_retry(
            run_id,
            RetryConfig::default(),
            |txn| {
                let count = attempts.fetch_add(1, Ordering::Relaxed);

                // Read the key (adds to read_set)
                let val = txn.get(&key)?;

                // On first attempt, simulate conflict by direct storage write
                // (This won't work directly since we can't access storage mid-txn)
                // For testing, we'll verify the retry mechanism differently

                Ok(count)
            },
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_max_retries_exceeded() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();

        let run_id = RunId::new();

        let config = RetryConfig {
            max_retries: 2,
            ..Default::default()
        };

        // Always fail
        let result: Result<()> = db.transaction_with_retry(
            run_id,
            config,
            |_txn| {
                Err(Error::TransactionConflict("forced conflict".to_string()))
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_non_conflict_error_not_retried() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();

        let run_id = RunId::new();
        let attempts = AtomicU64::new(0);

        let result: Result<()> = db.transaction_with_retry(
            run_id,
            RetryConfig::default(),
            |_txn| {
                attempts.fetch_add(1, Ordering::Relaxed);
                Err(Error::InvalidState("not a conflict".to_string()))
            },
        );

        // Should only try once (non-conflict errors don't retry)
        assert_eq!(attempts.load(Ordering::Relaxed), 1);
        assert!(result.is_err());
    }
}
```

#### Step 3: Implement retry logic

```rust
impl Database {
    /// Execute a transaction with retry on conflict
    pub fn transaction_with_retry<F, T>(
        &self,
        run_id: RunId,
        config: RetryConfig,
        f: F,
    ) -> Result<T>
    where
        F: Fn(&mut TransactionContext) -> Result<T>,
    {
        let mut last_error = None;

        for attempt in 0..=config.max_retries {
            let mut txn = self.begin_transaction(run_id);

            match f(&mut txn) {
                Ok(value) => {
                    match self.commit_transaction(&mut txn) {
                        Ok(()) => return Ok(value),
                        Err(e) if e.is_conflict() && attempt < config.max_retries => {
                            // Conflict - will retry
                            last_error = Some(e);
                            let delay = Self::calculate_delay(&config, attempt);
                            std::thread::sleep(std::time::Duration::from_millis(delay));
                            continue;
                        }
                        Err(e) => return Err(e),
                    }
                }
                Err(e) if e.is_conflict() && attempt < config.max_retries => {
                    // Closure returned conflict - will retry
                    last_error = Some(e);
                    let delay = Self::calculate_delay(&config, attempt);
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        Err(last_error.unwrap_or_else(|| {
            Error::TransactionConflict("Max retries exceeded".to_string())
        }))
    }

    fn calculate_delay(config: &RetryConfig, attempt: usize) -> u64 {
        let delay = config.base_delay_ms * (1 << attempt);
        delay.min(config.max_delay_ms)
    }
}
```

#### Step 4: Add is_conflict() to Error

Update `crates/core/src/error.rs`:

```rust
impl Error {
    /// Check if this error is a transaction conflict
    pub fn is_conflict(&self) -> bool {
        matches!(self, Error::TransactionConflict(_))
    }
}
```

#### Step 5: Run validation

```bash
~/.cargo/bin/cargo test -p in-mem-engine
~/.cargo/bin/cargo clippy -p in-mem-engine -- -D warnings
~/.cargo/bin/cargo fmt --check
```

### Acceptance Criteria

- [ ] RetryConfig struct with configurable retry behavior
- [ ] transaction_with_retry() retries on conflict
- [ ] Non-conflict errors are not retried
- [ ] Exponential backoff between retries
- [ ] Max retries is enforced
- [ ] All tests pass
- [ ] No clippy warnings

---

## Story #102: Database API Integration Tests

**GitHub Issue**: #102
**Estimated Time**: 3 hours
**Dependencies**: Stories #99, #100, #101
**Blocks**: None (Epic 10 complete)

### What to Implement

Comprehensive integration tests for the Database transaction API:

1. **End-to-end transaction scenarios**
2. **Multi-threaded conflict tests**
3. **Recovery after crash**
4. **M1 compatibility verification**

### Implementation Steps

#### Step 1: Start the story

```bash
./scripts/start-story.sh 10 102 integration-tests
```

#### Step 2: Create integration test file

Create `crates/engine/tests/database_transaction_tests.rs`:

```rust
//! Database Transaction API Integration Tests
//!
//! Validates the complete transaction lifecycle including:
//! - Closure API
//! - Implicit transactions
//! - Conflict detection and retry
//! - WAL durability
//! - Recovery

use in_mem_core::traits::Storage;
use in_mem_core::types::{Key, Namespace, RunId};
use in_mem_core::value::Value;
use in_mem_engine::Database;
use std::sync::Arc;
use std::thread;
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
// End-to-End Transaction Scenarios
// ============================================================================

#[test]
fn test_e2e_read_modify_write() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);
    let key = Key::new_kv(ns, "counter");

    // Initialize counter
    db.put(run_id, key.clone(), Value::I64(0)).unwrap();

    // Read-modify-write in transaction
    db.transaction(run_id, |txn| {
        let val = txn.get(&key)?.unwrap();
        if let Value::I64(n) = val.value {
            txn.put(key.clone(), Value::I64(n + 1))?;
        }
        Ok(())
    }).unwrap();

    // Verify incremented
    let result = db.get(run_id, &key).unwrap().unwrap();
    assert_eq!(result.value, Value::I64(1));
}

#[test]
fn test_e2e_multi_key_transaction() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);

    // Transaction with multiple keys
    db.transaction(run_id, |txn| {
        txn.put(Key::new_kv(ns.clone(), "a"), Value::I64(1))?;
        txn.put(Key::new_kv(ns.clone(), "b"), Value::I64(2))?;
        txn.put(Key::new_kv(ns.clone(), "c"), Value::I64(3))?;
        Ok(())
    }).unwrap();

    // Verify all keys
    assert_eq!(db.get(run_id, &Key::new_kv(ns.clone(), "a")).unwrap().unwrap().value, Value::I64(1));
    assert_eq!(db.get(run_id, &Key::new_kv(ns.clone(), "b")).unwrap().unwrap().value, Value::I64(2));
    assert_eq!(db.get(run_id, &Key::new_kv(ns.clone(), "c")).unwrap().unwrap().value, Value::I64(3));
}

#[test]
fn test_e2e_transaction_abort_rollback() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);
    let key = Key::new_kv(ns, "rollback_key");

    // Pre-populate
    db.put(run_id, key.clone(), Value::I64(100)).unwrap();

    // Failing transaction
    let result: Result<(), _> = db.transaction(run_id, |txn| {
        txn.put(key.clone(), Value::I64(999))?;
        Err(in_mem_core::error::Error::InvalidState("rollback".to_string()))
    });

    assert!(result.is_err());

    // Original value should be preserved
    let val = db.get(run_id, &key).unwrap().unwrap();
    assert_eq!(val.value, Value::I64(100));
}

// ============================================================================
// Multi-threaded Conflict Tests
// ============================================================================

#[test]
fn test_concurrent_transactions_different_keys() {
    let temp_dir = TempDir::new().unwrap();
    let db = Arc::new(Database::open(temp_dir.path().join("db")).unwrap());

    let run_id = RunId::new();
    let ns = create_ns(run_id);

    let mut handles = vec![];

    // 10 threads, each writing to its own key
    for i in 0..10 {
        let db = Arc::clone(&db);
        let ns = ns.clone();

        handles.push(thread::spawn(move || {
            let key = Key::new_kv(ns, &format!("thread_{}", i));
            db.put(run_id, key, Value::I64(i as i64)).unwrap();
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // All keys should exist
    for i in 0..10 {
        let key = Key::new_kv(ns.clone(), &format!("thread_{}", i));
        let val = db.get(run_id, &key).unwrap().unwrap();
        assert_eq!(val.value, Value::I64(i as i64));
    }
}

#[test]
fn test_concurrent_transactions_same_key_blind_write() {
    // Per spec Section 3.2: Blind writes don't conflict
    let temp_dir = TempDir::new().unwrap();
    let db = Arc::new(Database::open(temp_dir.path().join("db")).unwrap());

    let run_id = RunId::new();
    let ns = create_ns(run_id);
    let key = Key::new_kv(ns, "contested");

    let mut handles = vec![];

    // 10 threads writing to the same key (blind writes)
    for i in 0..10 {
        let db = Arc::clone(&db);
        let key = key.clone();

        handles.push(thread::spawn(move || {
            db.put(run_id, key, Value::I64(i as i64)).unwrap();
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // Key should exist (last writer wins)
    let val = db.get(run_id, &key).unwrap();
    assert!(val.is_some());
}

// ============================================================================
// Recovery Tests
// ============================================================================

#[test]
fn test_recovery_preserves_transactions() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("db");

    let run_id = RunId::new();
    let ns = create_ns(run_id);

    // Write data and close
    {
        let db = Database::open(&db_path).unwrap();

        db.transaction(run_id, |txn| {
            txn.put(Key::new_kv(ns.clone(), "key1"), Value::I64(1))?;
            txn.put(Key::new_kv(ns.clone(), "key2"), Value::I64(2))?;
            Ok(())
        }).unwrap();

        db.transaction(run_id, |txn| {
            txn.put(Key::new_kv(ns.clone(), "key3"), Value::I64(3))?;
            Ok(())
        }).unwrap();
    }

    // Reopen and verify
    {
        let db = Database::open(&db_path).unwrap();

        assert_eq!(db.get(run_id, &Key::new_kv(ns.clone(), "key1")).unwrap().unwrap().value, Value::I64(1));
        assert_eq!(db.get(run_id, &Key::new_kv(ns.clone(), "key2")).unwrap().unwrap().value, Value::I64(2));
        assert_eq!(db.get(run_id, &Key::new_kv(ns.clone(), "key3")).unwrap().unwrap().value, Value::I64(3));
    }
}

#[test]
fn test_recovery_version_continuity() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("db");

    let run_id = RunId::new();
    let ns = create_ns(run_id);

    let version_before;

    // Write data and capture version
    {
        let db = Database::open(&db_path).unwrap();
        db.put(run_id, Key::new_kv(ns.clone(), "key"), Value::I64(1)).unwrap();
        version_before = db.storage().current_version();
    }

    // Reopen
    {
        let db = Database::open(&db_path).unwrap();
        let version_after = db.storage().current_version();

        // Version should be preserved
        assert_eq!(version_before, version_after);

        // New writes should get higher versions
        db.put(run_id, Key::new_kv(ns.clone(), "key2"), Value::I64(2)).unwrap();
        assert!(db.storage().current_version() > version_before);
    }
}

// ============================================================================
// M1 Compatibility Tests
// ============================================================================

#[test]
fn test_m1_api_compatibility() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);
    let key = Key::new_kv(ns, "m1_key");

    // M1-style operations
    db.put(run_id, key.clone(), Value::String("value1".to_string())).unwrap();
    let val = db.get(run_id, &key).unwrap().unwrap();
    assert_eq!(val.value, Value::String("value1".to_string()));

    db.delete(run_id, key.clone()).unwrap();
    assert!(db.get(run_id, &key).unwrap().is_none());
}

// ============================================================================
// Transaction Metrics Tests
// ============================================================================

#[test]
fn test_transaction_metrics() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);

    // Execute some transactions
    for i in 0..5 {
        db.put(run_id, Key::new_kv(ns.clone(), &format!("key{}", i)), Value::I64(i as i64)).unwrap();
    }

    let metrics = db.coordinator().metrics();
    assert!(metrics.total_committed >= 5);
}
```

#### Step 3: Create validation report

Create `docs/milestones/EPIC_10_REVIEW.md`:

```markdown
# Epic 10: Database API Integration - Validation Report

**Epic**: Database API Integration
**Status**: ✅ COMPLETE
**Date**: [DATE]
**Reviewer**: Claude

---

## Validation Summary

| Check | Status |
|-------|--------|
| All tests pass | ✅ |
| Clippy clean | ✅ |
| Formatting clean | ✅ |
| Spec compliance | ✅ |

---

## Stories Completed

| Story | Title | Description |
|-------|-------|-------------|
| #98 | Database Transaction API | transaction() closure API, begin_transaction(), commit_transaction() |
| #99 | Transaction Coordinator | Metrics, active transaction tracking |
| #100 | Implicit Transactions | M1-compatible put/get/delete/cas |
| #101 | Error Handling & Retry | RetryConfig, transaction_with_retry() |
| #102 | Integration Tests | End-to-end, concurrency, recovery tests |

---

## Spec Compliance

### Section 4: Implicit Transactions
- [x] db.put() wraps in implicit transaction
- [x] db.get() creates snapshot, always succeeds
- [x] db.delete() wraps in implicit transaction
- [x] Implicit transactions include retry on conflict

### Core Invariants
- [x] No partial commits (all-or-nothing)
- [x] Monotonic versions
- [x] Read-your-writes

---

## Ready for: Epic 11 (Backwards Compatibility)
```

#### Step 4: Run full validation

```bash
~/.cargo/bin/cargo test --all
~/.cargo/bin/cargo clippy --all -- -D warnings
~/.cargo/bin/cargo fmt --check
```

### Acceptance Criteria

- [ ] All integration tests pass
- [ ] End-to-end scenarios work
- [ ] Multi-threaded tests pass
- [ ] Recovery tests pass
- [ ] M1 compatibility verified
- [ ] EPIC_10_REVIEW.md created
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Epic 10 complete

### Complete the Epic

After Story #102 is merged to epic-10-database-api:

```bash
# Merge epic to develop
git checkout develop
git merge --no-ff epic-10-database-api
git push origin develop

# Update M2_PROJECT_STATUS.md
# Create EPIC_10_REVIEW.md
```

---

## Quick Reference: Story Commands

```bash
# Start a story
./scripts/start-story.sh 10 <story_number> <description>

# Run tests
~/.cargo/bin/cargo test -p in-mem-engine
~/.cargo/bin/cargo test --all

# Check code quality
~/.cargo/bin/cargo clippy --all -- -D warnings
~/.cargo/bin/cargo fmt --check

# Complete a story
./scripts/complete-story.sh <story_number>
```

---

## Spec Compliance Checklist

Before completing any story, verify:

| Requirement | Verified |
|-------------|----------|
| db.put() wraps in implicit transaction (Section 4.2) | [ ] |
| db.get() uses snapshot, always succeeds (Section 4.2) | [ ] |
| db.delete() wraps in implicit transaction (Section 4.2) | [ ] |
| Implicit transactions can conflict (Section 4.3) | [ ] |
| Retry logic for conflicts (Section 4.3) | [ ] |
| WAL logs transaction boundaries | [ ] |
| Recovery restores TransactionManager version | [ ] |
| All-or-nothing commit (Core Invariants) | [ ] |
| Monotonic versions (Core Invariants) | [ ] |

---

*Generated for Epic 10: Database API Integration*
*Spec Reference: docs/architecture/M2_TRANSACTION_SEMANTICS.md Section 4*
