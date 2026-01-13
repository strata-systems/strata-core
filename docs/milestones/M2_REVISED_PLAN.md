# M2 Revised Implementation Plan

## Response to Architectural Review

This document addresses the architectural feedback on the M2 epic structure and provides a revised, more focused implementation plan.

---

## Key Feedback Points Addressed

### 1. ✅ Snapshot Cloning Strategy (ACCEPTED)

**Feedback**: Snapshot cloning is expensive but acceptable for M2 if properly abstracted.

**Our Approach**:
- Use `SnapshotView` trait abstraction (already designed)
- M2 implementation: `ClonedSnapshotView` (deep copy of BTreeMap)
- Future optimization path: `LazySnapshotView` (version-bounded reads from live store)

**Documented Limitations**:
- O(data_size) memory per active transaction
- O(data_size) time per snapshot creation
- Acceptable for agent workloads (small working sets, short transactions)
- Not suitable for large datasets or long-running analytics queries

**Mitigation**:
```rust
/// M2 Implementation: Clone-based snapshot
///
/// KNOWN LIMITATION: Clones entire BTreeMap at transaction start
/// - Memory: O(data_size) per active transaction
/// - Time: O(data_size) per snapshot
///
/// ACCEPTABLE FOR M2 BECAUSE:
/// - Agent workloads have small working sets (<100MB typical)
/// - Transactions are short-lived (<1 second)
/// - Simplicity enables correctness validation
///
/// FUTURE OPTIMIZATION: LazySnapshotView
/// - Version-bounded reads from live storage
/// - No cloning overhead
/// - Enabled by SnapshotView trait abstraction
pub struct ClonedSnapshotView {
    version: u64,
    data: Arc<BTreeMap<Key, VersionedValue>>,
}
```

---

### 2. ⚠️ Epic Granularity Too Fine (REVISED)

**Feedback**: 14 epics is too heavy for velocity. Consolidate to ~7-8 epics.

**Original Structure** (M2_EPIC_ANALYSIS.md):
- 9 epics, 44 stories
- Average 4.9 stories per epic
- Extremely JIRA-style decomposition

**Revised Structure** (see below):
- **7 epics, 32 stories**
- Average 4.6 stories per epic
- Focused on velocity while maintaining testability

**Consolidation Strategy**:
- Merge Epic 6 (Infrastructure) + Epic 7 (Snapshot) → **Epic 6: Transaction Foundations**
- Merge Epic 8 (Operations) + Epic 9 (Validation) → **Epic 7: Transaction Semantics**
- Merge Epic 10 (Commit) + Epic 11 (WAL) → **Epic 8: Durability & Commit**
- Keep Epic 12 (Recovery) separate (critical path)
- Merge Epic 13 (Database API) + Epic 14 (Testing) → **Epic 9: API & Validation**

---

### 3. ⚠️ WAL Integration Overengineered (PHASED APPROACH)

**Feedback**: Don't build full WAL complexity in M2. Use phased approach.

**Phased WAL Implementation**:

#### Phase A: Basic Transaction Logging (M2 Scope)
```rust
// M2: Minimal transaction WAL entries
pub enum WALEntry {
    BeginTxn { txn_id: u64, run_id: RunId, timestamp: Timestamp },
    Write { key: Key, value: Value, version: u64 },
    CommitTxn { txn_id: u64, commit_version: u64 },
    // AbortTxn NOT implemented in M2 - aborted txns just don't write anything
}
```

**M2 Behavior**:
- Aborted transactions write NO WAL entries (simplest correct behavior)
- Recovery ignores incomplete transactions (no CommitTxn = discard)
- No explicit AbortTxn entry needed

#### Phase B: Advanced Recovery (M3/M4)
- Incomplete transaction detection
- Partial WAL replay
- Explicit AbortTxn entries
- Long-running transaction support

#### Phase C: Production Hardening (M4/M5)
- Corruption tolerance
- Checksums and CRCs
- Recovery fuzzing
- WAL compaction

**M2 Decision**: Implement Phase A only. Defer Phase B/C to later milestones.

---

### 4. 🚨 Missing Transaction Semantics Specification (CRITICAL)

**Feedback**: Write explicit Transaction Semantics Spec before any M2 code.

**Action Required**: Create `docs/architecture/M2_TRANSACTION_SEMANTICS.md`

**Must Answer**:
1. What does snapshot isolation guarantee?
2. What does it NOT guarantee? (e.g., write skew anomalies possible)
3. When does a transaction abort?
4. When does CAS fail?
5. What is visible inside a transaction?
6. What happens on retry?
7. Conflict resolution semantics (first-committer-wins)
8. Cross-primitive transaction atomicity

**Example Questions to Answer**:
```rust
// Q: What happens here?
db.transaction(|txn| {
    let v1 = txn.get(key_a)?;  // Reads version 10
    let v2 = txn.get(key_b)?;  // Reads version 12

    // Meanwhile, another txn commits and updates key_a to version 15

    txn.put(key_c, value)?;    // Does this see version 10 or 15 of key_a?
    Ok(())
})?;

// A: Sees version 10 (snapshot isolation)
```

```rust
// Q: Does this conflict?
// T1: Reads A, writes B
// T2: Reads B, writes A
// Both commit at the same time

// A: NO conflict under snapshot isolation (write skew allowed)
// This is NOT serializable, but is snapshot isolation
```

**This document must be written FIRST, before Epic 6.**

---

### 5. ⚠️ Testing Strategy Too Ambitious (PHASED)

**Feedback**: Don't write all tests upfront. Focus on deterministic tests first.

**M2 Testing Focus**:
1. ✅ Deterministic single-threaded tests
2. ✅ Two-thread conflict tests (manual scheduling)
3. ✅ Basic crash-recovery tests (kill during commit)
4. ❌ Property-based tests (defer to M3)
5. ❌ Fuzzing and corruption tests (defer to M4)
6. ❌ Loom-based concurrency testing (defer to M3)

**M2 Test Categories**:
- **Unit Tests**: TransactionContext, SnapshotView, conflict detection logic
- **Integration Tests**: Two-thread conflict scenarios (read-write, write-write, CAS)
- **Recovery Tests**: Basic incomplete transaction handling
- **Regression Tests**: M1 API still works (backwards compatibility)

**Deferred to M3/M4**:
- Advanced multi-threaded tests (>2 threads)
- Property-based testing (proptest)
- Concurrency model checking (loom)
- Fuzzing (corruption, race conditions)

---

## Revised Epic Structure

### Overview: 7 Epics, 32 Stories

| Epic | Name | Stories | Duration | Parallelization |
|------|------|---------|----------|-----------------|
| **Epic 6** | Transaction Foundations | 5 | 2-3 days | After Story #33 |
| **Epic 7** | Transaction Semantics | 6 | 2-3 days | After Story #38 |
| **Epic 8** | Durability & Commit | 5 | 2 days | After Story #44 |
| **Epic 9** | Recovery Support | 4 | 2 days | After Story #49 |
| **Epic 10** | Database API Integration | 5 | 2-3 days | After Story #53 |
| **Epic 11** | Backwards Compatibility | 3 | 1-2 days | After Story #58 |
| **Epic 12** | OCC Validation & Benchmarking | 4 | 2 days | After Story #61 |

**Total**: 7 epics, 32 stories, ~14-18 days (vs original 9 epics, 44 stories)

---

## Epic 6: Transaction Foundations (5 stories, 2-3 days)

**Goal**: Core transaction infrastructure and snapshot isolation

**Dependencies**: None (starts M2)

**Deliverables**:
- TransactionContext with read/write/delete/cas sets
- SnapshotView trait and ClonedSnapshotView implementation
- Transaction lifecycle (begin, active, validating, committed, aborted)

### Story #33: Transaction Semantics Specification (4 hours) 🔴 FOUNDATION
**Blocks**: All M2 stories

**Deliverable**: `docs/architecture/M2_TRANSACTION_SEMANTICS.md`

**Must Define**:
1. Snapshot isolation guarantees
2. Conflict detection rules
3. CAS semantics
4. Abort/retry behavior
5. Visibility rules
6. Cross-primitive atomicity

**Acceptance Criteria**:
- [ ] Document answers all 8 semantic questions
- [ ] Examples for each conflict type
- [ ] Non-goals clearly stated (e.g., "NOT serializable")
- [ ] Reviewed and approved before Story #34

---

### Story #34: TransactionContext Core (4 hours)
**File**: `crates/concurrency/src/transaction.rs`

**Deliverable**: TransactionContext struct with lifecycle management

**Implementation**:
```rust
pub struct TransactionContext {
    // Identity
    pub txn_id: u64,
    pub run_id: RunId,

    // Snapshot isolation
    pub start_version: u64,
    pub snapshot: Box<dyn SnapshotView>,

    // Operation tracking
    pub read_set: HashMap<Key, u64>,        // key → version read
    pub write_set: HashMap<Key, Value>,     // key → new value
    pub delete_set: HashSet<Key>,           // keys to delete
    pub cas_set: Vec<CASOperation>,         // CAS operations

    // State
    pub status: TransactionStatus,
}

pub enum TransactionStatus {
    Active,
    Validating,
    Committed,
    Aborted { reason: String },
}

impl TransactionContext {
    pub fn new(txn_id: u64, run_id: RunId, snapshot: Box<dyn SnapshotView>) -> Self;
    pub fn is_active(&self) -> bool;
    pub fn mark_validating(&mut self);
    pub fn mark_committed(&mut self);
    pub fn mark_aborted(&mut self, reason: String);
}
```

**Tests**:
- [ ] Create transaction in Active state
- [ ] State transitions (Active → Validating → Committed)
- [ ] State transitions (Active → Validating → Aborted)
- [ ] Cannot transition from Committed/Aborted

---

### Story #35: SnapshotView Trait & ClonedSnapshot (5 hours)
**File**: `crates/concurrency/src/snapshot.rs`

**Deliverable**: SnapshotView abstraction with clone-based implementation

**Implementation**:
```rust
/// Snapshot abstraction - version-bounded view of storage
///
/// M2: ClonedSnapshotView (deep clone)
/// Future: LazySnapshotView (version-bounded reads)
pub trait SnapshotView: Send + Sync {
    fn get(&self, key: &Key) -> Result<Option<VersionedValue>>;
    fn scan_prefix(&self, prefix: &Key) -> Result<Vec<(Key, VersionedValue)>>;
    fn version(&self) -> u64;
}

/// M2 Implementation: Clone-based snapshot
///
/// LIMITATIONS (acceptable for M2):
/// - O(data_size) memory per snapshot
/// - O(data_size) time per snapshot creation
///
/// MITIGATION:
/// - SnapshotView trait enables future LazySnapshotView
/// - Acceptable for agent workloads (small datasets, short txns)
pub struct ClonedSnapshotView {
    version: u64,
    data: Arc<BTreeMap<Key, VersionedValue>>,
}

impl ClonedSnapshotView {
    pub fn create(store: &UnifiedStore) -> Result<Self>;
}

impl SnapshotView for ClonedSnapshotView {
    fn get(&self, key: &Key) -> Result<Option<VersionedValue>>;
    fn scan_prefix(&self, prefix: &Key) -> Result<Vec<(Key, VersionedValue)>>;
    fn version(&self) -> u64;
}
```

**Tests**:
- [ ] Create snapshot from UnifiedStore
- [ ] Snapshot sees consistent version
- [ ] Snapshot does NOT see later writes to store
- [ ] Multiple snapshots are independent

---

### Story #36: Transaction Read Operations (4 hours)
**File**: `crates/concurrency/src/transaction.rs`

**Deliverable**: Buffered reads with read-set tracking

**Implementation**:
```rust
impl TransactionContext {
    /// Get a value (reads from snapshot, tracks in read_set)
    pub fn get(&mut self, key: &Key) -> Result<Option<Value>> {
        // Check if key is in write_set (read-your-writes)
        if let Some(value) = self.write_set.get(key) {
            return Ok(Some(value.clone()));
        }

        // Check if key is in delete_set
        if self.delete_set.contains(key) {
            return Ok(None);
        }

        // Read from snapshot
        let versioned_value = self.snapshot.get(key)?;

        // Track in read_set for validation
        if let Some(ref vv) = versioned_value {
            self.read_set.insert(key.clone(), vv.version);
        }

        Ok(versioned_value.map(|vv| vv.value))
    }

    /// Scan with prefix (reads from snapshot, tracks all in read_set)
    pub fn scan_prefix(&mut self, prefix: &Key) -> Result<Vec<(Key, Value)>>;
}
```

**Tests**:
- [ ] Read from snapshot
- [ ] Read-your-writes (see uncommitted writes)
- [ ] Read deleted key returns None
- [ ] Read-set tracks all accessed keys

---

### Story #37: Transaction Write Operations (4 hours)
**File**: `crates/concurrency/src/transaction.rs`

**Deliverable**: Buffered writes, deletes, and CAS operations

**Implementation**:
```rust
impl TransactionContext {
    /// Buffer a write (not visible to other txns until commit)
    pub fn put(&mut self, key: Key, value: Value) -> Result<()> {
        self.write_set.insert(key, value);
        Ok(())
    }

    /// Buffer a delete
    pub fn delete(&mut self, key: Key) -> Result<()> {
        self.write_set.remove(&key);
        self.delete_set.insert(key);
        Ok(())
    }

    /// Buffer a compare-and-swap operation
    pub fn cas(&mut self, key: Key, expected_version: u64, new_value: Value) -> Result<()> {
        self.cas_set.push(CASOperation {
            key,
            expected_version,
            new_value,
        });
        Ok(())
    }
}

pub struct CASOperation {
    pub key: Key,
    pub expected_version: u64,
    pub new_value: Value,
}
```

**Tests**:
- [ ] Buffered writes not visible to other txns
- [ ] Overwrite in same txn (latest value wins)
- [ ] Delete removes from write_set
- [ ] CAS operations buffered correctly

---

## Epic 7: Transaction Semantics (6 stories, 2-3 days)

**Goal**: Conflict detection and validation logic

**Dependencies**: Epic 6 complete

**Deliverables**:
- Read-set validation
- Write-set validation
- CAS validation
- Conflict detection with examples

### Story #38: Conflict Detection Infrastructure (3 hours) 🔴 FOUNDATION
**File**: `crates/concurrency/src/validation.rs`

**Deliverable**: Conflict detection types and error handling

**Implementation**:
```rust
pub enum ConflictType {
    ReadWriteConflict { key: Key, expected_version: u64, current_version: u64 },
    WriteWriteConflict { key: Key },
    CASConflict { key: Key, expected_version: u64, current_version: u64 },
}

pub struct ValidationResult {
    pub success: bool,
    pub conflicts: Vec<ConflictType>,
}

impl ValidationResult {
    pub fn ok() -> Self;
    pub fn conflict(conflict_type: ConflictType) -> Self;
    pub fn merge(&mut self, other: ValidationResult);
}
```

**Tests**:
- [ ] Create ValidationResult::ok()
- [ ] Create ValidationResult with conflicts
- [ ] Merge multiple validation results

---

### Story #39: Read-Set Validation (4 hours)
**File**: `crates/concurrency/src/validation.rs`

**Deliverable**: Validate that read versions haven't changed

**Implementation**:
```rust
/// Validate read-set: Check that all read versions are unchanged
pub fn validate_read_set(
    read_set: &HashMap<Key, u64>,
    store: &UnifiedStore,
) -> Result<ValidationResult> {
    let mut result = ValidationResult::ok();

    for (key, expected_version) in read_set {
        let current = store.get(key)?;

        match current {
            Some(vv) if vv.version == *expected_version => {
                // Version unchanged - OK
            }
            Some(vv) => {
                // Version changed - conflict
                result.conflicts.push(ConflictType::ReadWriteConflict {
                    key: key.clone(),
                    expected_version: *expected_version,
                    current_version: vv.version,
                });
            }
            None if *expected_version == 0 => {
                // Key didn't exist and still doesn't - OK
            }
            None => {
                // Key was deleted - conflict
                result.conflicts.push(ConflictType::ReadWriteConflict {
                    key: key.clone(),
                    expected_version: *expected_version,
                    current_version: 0,
                });
            }
        }
    }

    result.success = result.conflicts.is_empty();
    Ok(result)
}
```

**Tests**:
- [ ] Read-set valid (no conflicts)
- [ ] Read-set invalid (version changed)
- [ ] Read-set invalid (key deleted)
- [ ] Multiple conflicts detected

---

### Story #40: Write-Set Validation (3 hours)
**File**: `crates/concurrency/src/validation.rs`

**Deliverable**: Validate that write keys haven't been modified

**Implementation**:
```rust
/// Validate write-set: Check for write-write conflicts
pub fn validate_write_set(
    write_set: &HashMap<Key, Value>,
    start_version: u64,
    store: &UnifiedStore,
) -> Result<ValidationResult> {
    let mut result = ValidationResult::ok();

    for key in write_set.keys() {
        let current = store.get(key)?;

        if let Some(vv) = current {
            if vv.version > start_version {
                // Key was modified after txn started - conflict
                result.conflicts.push(ConflictType::WriteWriteConflict {
                    key: key.clone(),
                });
            }
        }
    }

    result.success = result.conflicts.is_empty();
    Ok(result)
}
```

**Tests**:
- [ ] Write-set valid (no conflicts)
- [ ] Write-set invalid (key modified)
- [ ] Multiple write conflicts detected

---

### Story #41: CAS Validation (4 hours)
**File**: `crates/concurrency/src/validation.rs`

**Deliverable**: Validate CAS operations against expected versions

**Implementation**:
```rust
/// Validate CAS operations: Check expected versions
pub fn validate_cas_set(
    cas_set: &[CASOperation],
    store: &UnifiedStore,
) -> Result<ValidationResult> {
    let mut result = ValidationResult::ok();

    for cas_op in cas_set {
        let current = store.get(&cas_op.key)?;

        match current {
            Some(vv) if vv.version == cas_op.expected_version => {
                // Version matches - OK
            }
            Some(vv) => {
                // Version mismatch - conflict
                result.conflicts.push(ConflictType::CASConflict {
                    key: cas_op.key.clone(),
                    expected_version: cas_op.expected_version,
                    current_version: vv.version,
                });
            }
            None if cas_op.expected_version == 0 => {
                // Expected not to exist and doesn't - OK
            }
            None => {
                // Expected to exist but doesn't - conflict
                result.conflicts.push(ConflictType::CASConflict {
                    key: cas_op.key.clone(),
                    expected_version: cas_op.expected_version,
                    current_version: 0,
                });
            }
        }
    }

    result.success = result.conflicts.is_empty();
    Ok(result)
}
```

**Tests**:
- [ ] CAS valid (version matches)
- [ ] CAS invalid (version mismatch)
- [ ] CAS on non-existent key (expected version 0)
- [ ] CAS on deleted key (conflict)

---

### Story #42: Full Transaction Validation (4 hours)
**File**: `crates/concurrency/src/validation.rs`

**Deliverable**: Orchestrate all validation phases

**Implementation**:
```rust
/// Validate entire transaction
pub fn validate_transaction(
    txn: &TransactionContext,
    store: &UnifiedStore,
) -> Result<ValidationResult> {
    let mut result = ValidationResult::ok();

    // Phase 1: Validate read-set
    let read_result = validate_read_set(&txn.read_set, store)?;
    result.merge(read_result);

    // Phase 2: Validate write-set
    let write_result = validate_write_set(&txn.write_set, txn.start_version, store)?;
    result.merge(write_result);

    // Phase 3: Validate CAS operations
    let cas_result = validate_cas_set(&txn.cas_set, store)?;
    result.merge(cas_result);

    Ok(result)
}
```

**Tests**:
- [ ] All validations pass
- [ ] Read-set conflict aborts
- [ ] Write-set conflict aborts
- [ ] CAS conflict aborts
- [ ] Multiple conflicts reported

---

### Story #43: Conflict Examples & Documentation (3 hours)
**File**: `crates/concurrency/tests/conflict_examples.rs`

**Deliverable**: Test suite demonstrating all conflict types

**Tests**:
```rust
#[test]
fn test_read_write_conflict() {
    // T1 reads key A (version 5)
    // T2 writes key A (version 6)
    // T1 tries to commit → ABORT (read-write conflict)
}

#[test]
fn test_write_write_conflict() {
    // T1 writes key A
    // T2 writes key A
    // T1 commits (version 6)
    // T2 tries to commit → ABORT (write-write conflict)
}

#[test]
fn test_cas_conflict() {
    // T1 CAS key A (expected version 5)
    // T2 writes key A (version 6)
    // T1 tries to commit → ABORT (CAS conflict)
}

#[test]
fn test_no_conflict_different_keys() {
    // T1 writes key A
    // T2 writes key B
    // Both commit → NO CONFLICT
}

#[test]
fn test_write_skew_allowed() {
    // T1 reads A and B, writes C
    // T2 reads C and D, writes E
    // Both commit → NO CONFLICT (snapshot isolation allows this)
}
```

**Acceptance Criteria**:
- [ ] All 5 conflict scenarios tested
- [ ] Each test has explanation comment
- [ ] Non-conflict cases documented

---

## Epic 8: Durability & Commit (5 stories, 2 days)

**Goal**: Atomic commit with WAL logging (Phase A only)

**Dependencies**: Epic 7 complete

**Deliverables**:
- Commit application logic
- WAL transaction entries (BeginTxn, Write, CommitTxn)
- Atomic commit coordination

**Note**: AbortTxn entries NOT in M2 (Phase B)

### Story #44: WAL Transaction Entries (3 hours) 🔴 FOUNDATION
**File**: `crates/durability/src/wal.rs`

**Deliverable**: WAL entry types for transactions

**Implementation**:
```rust
// M2: Phase A transaction logging (minimal)
pub enum WALEntry {
    // ... existing M1 entries ...

    // M2: Transaction entries
    BeginTxn {
        txn_id: u64,
        run_id: RunId,
        timestamp: Timestamp,
    },
    CommitTxn {
        txn_id: u64,
        commit_version: u64,
    },

    // AbortTxn NOT in M2 - Phase B feature
}
```

**Tests**:
- [ ] Serialize BeginTxn entry
- [ ] Serialize CommitTxn entry
- [ ] Round-trip encoding

---

### Story #45: Commit Application (5 hours)
**File**: `crates/concurrency/src/commit.rs`

**Deliverable**: Apply validated transaction to storage

**Implementation**:
```rust
/// Apply committed transaction to storage
pub fn apply_transaction(
    txn: &TransactionContext,
    store: &mut UnifiedStore,
    wal: &mut WriteAheadLog,
) -> Result<u64> {
    // Must be in Validating state
    assert_eq!(txn.status, TransactionStatus::Validating);

    // Acquire write lock (critical section)
    let mut data = store.data.write();

    // Apply writes
    for (key, value) in &txn.write_set {
        let version = store.next_version();
        let vv = VersionedValue { value: value.clone(), version, timestamp: Timestamp::now() };
        data.insert(key.clone(), vv);

        // Log to WAL
        wal.append(WALEntry::Write {
            key: key.clone(),
            value: value.clone(),
            version,
        })?;
    }

    // Apply deletes
    for key in &txn.delete_set {
        data.remove(key);

        // Log to WAL
        wal.append(WALEntry::Delete {
            key: key.clone(),
        })?;
    }

    // Apply CAS operations
    for cas_op in &txn.cas_set {
        let version = store.next_version();
        let vv = VersionedValue {
            value: cas_op.new_value.clone(),
            version,
            timestamp: Timestamp::now(),
        };
        data.insert(cas_op.key.clone(), vv);

        // Log to WAL
        wal.append(WALEntry::Write {
            key: cas_op.key.clone(),
            value: cas_op.new_value.clone(),
            version,
        })?;
    }

    let commit_version = store.current_version();

    // Log commit
    wal.append(WALEntry::CommitTxn {
        txn_id: txn.txn_id,
        commit_version,
    })?;

    Ok(commit_version)
}
```

**Tests**:
- [ ] Apply write-set to storage
- [ ] Apply delete-set to storage
- [ ] Apply CAS operations to storage
- [ ] Version increments correctly
- [ ] WAL entries written

---

### Story #46: Commit Coordinator (4 hours)
**File**: `crates/concurrency/src/commit.rs`

**Deliverable**: Orchestrate validation → apply → commit sequence

**Implementation**:
```rust
/// Commit transaction (validate + apply)
pub fn commit_transaction(
    mut txn: TransactionContext,
    store: &mut UnifiedStore,
    wal: &mut WriteAheadLog,
) -> Result<u64, ConflictError> {
    // Mark validating
    txn.mark_validating();

    // Validate
    let validation = validate_transaction(&txn, store)?;

    if !validation.success {
        // Abort on conflict
        txn.mark_aborted(format!("Conflicts: {:?}", validation.conflicts));
        return Err(ConflictError::ValidationFailed(validation.conflicts));
    }

    // Apply to storage (atomic)
    let commit_version = apply_transaction(&txn, store, wal)?;

    // Mark committed
    txn.mark_committed();

    Ok(commit_version)
}

#[derive(Debug)]
pub enum ConflictError {
    ValidationFailed(Vec<ConflictType>),
    StorageError(String),
}
```

**Tests**:
- [ ] Commit succeeds when validation passes
- [ ] Commit aborts when validation fails
- [ ] Transaction state transitions correctly
- [ ] Error propagation works

---

### Story #47: Abort Handling (3 hours)
**File**: `crates/concurrency/src/commit.rs`

**Deliverable**: Abort transaction without WAL logging

**Implementation**:
```rust
/// Abort transaction (M2: no WAL entry, just discard)
pub fn abort_transaction(mut txn: TransactionContext, reason: String) {
    txn.mark_aborted(reason);

    // M2: Aborted transactions write NO WAL entries
    // Just discard the transaction context
    // Recovery will ignore incomplete transactions
}
```

**Note**: This is Phase A abort (no WAL entry). Phase B will add explicit AbortTxn entries.

**Tests**:
- [ ] Abort marks transaction as aborted
- [ ] Aborted transaction writes no WAL entries
- [ ] Storage unchanged after abort

---

### Story #48: Atomic Commit Integration Test (4 hours)
**File**: `crates/concurrency/tests/commit_tests.rs`

**Deliverable**: End-to-end commit tests

**Tests**:
```rust
#[test]
fn test_successful_commit() {
    // Create transaction
    // Write some values
    // Commit
    // Verify storage updated
    // Verify WAL written
}

#[test]
fn test_aborted_commit() {
    // Create two conflicting transactions
    // Commit first → success
    // Commit second → abort (conflict)
    // Verify storage only has first txn's writes
}

#[test]
fn test_commit_atomicity() {
    // Transaction with multiple writes
    // Commit
    // Verify all-or-nothing (no partial writes)
}
```

**Acceptance Criteria**:
- [ ] Commit applies all writes atomically
- [ ] Abort discards all writes
- [ ] WAL reflects committed transactions only

---

## Epic 9: Recovery Support (4 stories, 2 days)

**Goal**: Recover transactions from WAL

**Dependencies**: Epic 8 complete

**Deliverables**:
- Detect incomplete transactions
- Replay committed transactions
- Discard aborted/incomplete transactions

### Story #49: Incomplete Transaction Detection (3 hours) 🔴 FOUNDATION
**File**: `crates/durability/src/recovery.rs`

**Deliverable**: Identify incomplete transactions in WAL

**Implementation**:
```rust
/// Scan WAL and identify transaction boundaries
pub fn scan_transactions(wal: &WriteAheadLog) -> Result<Vec<TransactionScan>> {
    let mut transactions = HashMap::new();

    for entry in wal.iter() {
        match entry {
            WALEntry::BeginTxn { txn_id, run_id, timestamp } => {
                transactions.insert(*txn_id, TransactionScan {
                    txn_id: *txn_id,
                    run_id: *run_id,
                    started_at: *timestamp,
                    committed: false,
                    entries: Vec::new(),
                });
            }
            WALEntry::Write { .. } | WALEntry::Delete { .. } => {
                // Track under active transaction (if any)
                if let Some(txn) = get_active_transaction(&mut transactions) {
                    txn.entries.push(entry.clone());
                }
            }
            WALEntry::CommitTxn { txn_id, .. } => {
                if let Some(txn) = transactions.get_mut(txn_id) {
                    txn.committed = true;
                }
            }
            _ => {}
        }
    }

    Ok(transactions.into_values().collect())
}

pub struct TransactionScan {
    pub txn_id: u64,
    pub run_id: RunId,
    pub started_at: Timestamp,
    pub committed: bool,
    pub entries: Vec<WALEntry>,
}
```

**Tests**:
- [ ] Detect committed transaction
- [ ] Detect incomplete transaction (no CommitTxn)
- [ ] Handle multiple transactions

---

### Story #50: Transaction Replay (4 hours)
**File**: `crates/durability/src/recovery.rs`

**Deliverable**: Replay committed transactions

**Implementation**:
```rust
/// Replay committed transactions from WAL
pub fn replay_transactions(
    scans: &[TransactionScan],
    store: &mut UnifiedStore,
) -> Result<()> {
    for scan in scans {
        if !scan.committed {
            // Skip incomplete transactions (M2: discard)
            log::debug!("Skipping incomplete transaction {}", scan.txn_id);
            continue;
        }

        // Replay committed transaction
        for entry in &scan.entries {
            match entry {
                WALEntry::Write { key, value, version } => {
                    let vv = VersionedValue {
                        value: value.clone(),
                        version: *version,
                        timestamp: Timestamp::now(),
                    };
                    store.insert(key.clone(), vv)?;
                }
                WALEntry::Delete { key } => {
                    store.remove(key)?;
                }
                _ => {}
            }
        }
    }

    Ok(())
}
```

**Tests**:
- [ ] Replay committed transaction
- [ ] Skip incomplete transaction
- [ ] Replay multiple transactions in order

---

### Story #51: Recovery Integration (4 hours)
**File**: `crates/durability/src/recovery.rs`

**Deliverable**: Full recovery flow with transaction support

**Implementation**:
```rust
/// Recover database from WAL (M2: with transaction support)
pub fn recover_from_wal(
    wal_path: &Path,
    store: &mut UnifiedStore,
) -> Result<RecoveryStats> {
    let wal = WriteAheadLog::open(wal_path)?;

    // Scan WAL for transactions
    let scans = scan_transactions(&wal)?;

    // Replay committed transactions
    replay_transactions(&scans, store)?;

    // Gather stats
    let stats = RecoveryStats {
        total_transactions: scans.len(),
        committed: scans.iter().filter(|s| s.committed).count(),
        incomplete: scans.iter().filter(|s| !s.committed).count(),
    };

    Ok(stats)
}

pub struct RecoveryStats {
    pub total_transactions: usize,
    pub committed: usize,
    pub incomplete: usize,
}
```

**Tests**:
- [ ] Recover committed transactions
- [ ] Discard incomplete transactions
- [ ] Recovery stats correct

---

### Story #52: Recovery Crash Tests (4 hours)
**File**: `crates/durability/tests/recovery_crash_tests.rs`

**Deliverable**: Simulate crashes during commit

**Tests**:
```rust
#[test]
fn test_crash_before_commit() {
    // Begin transaction
    // Write values
    // Write BeginTxn to WAL
    // CRASH (no CommitTxn)
    // Recover
    // Verify transaction discarded
}

#[test]
fn test_crash_after_commit() {
    // Begin transaction
    // Write values
    // Write BeginTxn + Write + CommitTxn to WAL
    // CRASH
    // Recover
    // Verify transaction replayed
}

#[test]
fn test_partial_wal_write() {
    // Begin transaction
    // Write BeginTxn
    // Write partial entry (corrupted)
    // CRASH
    // Recover
    // Verify incomplete transaction discarded
}
```

**Acceptance Criteria**:
- [ ] Crash before commit discards transaction
- [ ] Crash after commit replays transaction
- [ ] Partial WAL writes detected and handled

---

## Epic 10: Database API Integration (5 stories, 2-3 days)

**Goal**: Expose transaction API through Database

**Dependencies**: Epic 9 complete

**Deliverables**:
- `Database::transaction(closure)` API
- Automatic retry on conflict
- Cross-primitive transaction support

### Story #53: Database Transaction API (4 hours) 🔴 FOUNDATION
**File**: `crates/engine/src/database.rs`

**Deliverable**: High-level transaction API

**Implementation**:
```rust
impl Database {
    /// Execute closure in transaction (automatic retries)
    pub fn transaction<F, T>(&self, run_id: RunId, f: F) -> Result<T>
    where
        F: Fn(&mut TransactionContext) -> Result<T>,
    {
        let max_retries = 10;
        let mut attempt = 0;

        loop {
            attempt += 1;

            // Begin transaction
            let mut txn = self.begin_transaction(run_id)?;

            // Execute closure
            let result = f(&mut txn);

            match result {
                Ok(value) => {
                    // Try to commit
                    match self.commit_transaction(txn) {
                        Ok(_) => return Ok(value),
                        Err(ConflictError::ValidationFailed(_)) if attempt < max_retries => {
                            // Retry on conflict
                            log::debug!("Transaction conflict, retrying (attempt {})", attempt);
                            std::thread::sleep(Duration::from_micros(100 * attempt));
                            continue;
                        }
                        Err(e) => return Err(e.into()),
                    }
                }
                Err(e) => {
                    // Abort on error
                    self.abort_transaction(txn, e.to_string());
                    return Err(e);
                }
            }
        }
    }

    fn begin_transaction(&self, run_id: RunId) -> Result<TransactionContext>;
    fn commit_transaction(&self, txn: TransactionContext) -> Result<u64, ConflictError>;
    fn abort_transaction(&self, txn: TransactionContext, reason: String);
}
```

**Tests**:
- [ ] Transaction commits successfully
- [ ] Transaction retries on conflict
- [ ] Transaction aborts on error
- [ ] Max retries enforced

---

### Story #54: Cross-Primitive Transactions (4 hours)
**File**: `crates/engine/tests/cross_primitive_tests.rs`

**Deliverable**: Atomic operations across primitives

**Tests**:
```rust
#[test]
fn test_atomic_kv_and_event() {
    // Transaction writes to KV and Event Log
    db.transaction(run_id, |txn| {
        txn.put(kv_key, kv_value)?;
        txn.put(event_key, event_value)?;
        Ok(())
    })?;

    // Verify both written atomically
}

#[test]
fn test_cross_primitive_rollback() {
    // Transaction writes to KV and State Machine
    // Conflict on State Machine
    // Verify KV write also rolled back
}
```

**Acceptance Criteria**:
- [ ] Atomic writes across KV + Events
- [ ] Atomic writes across KV + State Machine
- [ ] Rollback affects all primitives

---

### Story #55: Transaction Context Lifecycle (3 hours)
**File**: `crates/engine/src/database.rs`

**Deliverable**: Proper transaction resource management

**Implementation**:
```rust
impl Database {
    fn begin_transaction(&self, run_id: RunId) -> Result<TransactionContext> {
        let txn_id = self.next_txn_id();
        let start_version = self.storage.current_version();

        // Create snapshot
        let snapshot = ClonedSnapshotView::create(&self.storage)?;

        // Log BeginTxn
        self.wal.append(WALEntry::BeginTxn {
            txn_id,
            run_id,
            timestamp: Timestamp::now(),
        })?;

        Ok(TransactionContext::new(txn_id, run_id, Box::new(snapshot)))
    }
}
```

**Tests**:
- [ ] Begin assigns unique txn_id
- [ ] Snapshot created at start_version
- [ ] BeginTxn logged to WAL

---

### Story #56: Retry Backoff Strategy (3 hours)
**File**: `crates/concurrency/src/retry.rs`

**Deliverable**: Exponential backoff for retries

**Implementation**:
```rust
pub struct RetryStrategy {
    max_attempts: usize,
    initial_delay_us: u64,
    max_delay_us: u64,
}

impl RetryStrategy {
    pub fn default() -> Self {
        Self {
            max_attempts: 10,
            initial_delay_us: 100,
            max_delay_us: 10_000,
        }
    }

    pub fn delay_for_attempt(&self, attempt: usize) -> Duration {
        let delay = self.initial_delay_us * 2u64.pow(attempt as u32);
        Duration::from_micros(delay.min(self.max_delay_us))
    }

    pub fn should_retry(&self, attempt: usize) -> bool {
        attempt < self.max_attempts
    }
}
```

**Tests**:
- [ ] Exponential backoff calculation
- [ ] Max delay cap enforced
- [ ] Max attempts enforced

---

### Story #57: Transaction Timeout Support (3 hours)
**File**: `crates/concurrency/src/transaction.rs`

**Deliverable**: Abort long-running transactions

**Implementation**:
```rust
impl TransactionContext {
    pub fn is_expired(&self, timeout: Duration) -> bool {
        self.started_at.elapsed() > timeout
    }
}

impl Database {
    pub fn transaction_with_timeout<F, T>(
        &self,
        run_id: RunId,
        timeout: Duration,
        f: F,
    ) -> Result<T>
    where
        F: Fn(&mut TransactionContext) -> Result<T>,
    {
        // Same as transaction(), but check timeout before commit
    }
}
```

**Tests**:
- [ ] Transaction aborts on timeout
- [ ] Timeout enforced before commit
- [ ] Normal transactions unaffected

---

## Epic 11: Backwards Compatibility (3 stories, 1-2 days)

**Goal**: Ensure M1 API still works

**Dependencies**: Epic 10 complete

**Deliverables**:
- M1 implicit transaction API
- M1 tests pass unchanged
- Migration guide

### Story #58: Implicit Transaction Wrapper (3 hours) 🔴 FOUNDATION
**File**: `crates/engine/src/database.rs`

**Deliverable**: M1 API delegates to M2 transactions

**Implementation**:
```rust
impl Database {
    /// M1 API: get (implicit transaction)
    pub fn get(&self, run_id: RunId, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.transaction(run_id, |txn| {
            txn.get(&Key::from_user_key(run_id, key))
                .map(|opt| opt.map(|v| v.into_bytes()))
        })
    }

    /// M1 API: put (implicit transaction)
    pub fn put(&self, run_id: RunId, key: &[u8], value: &[u8]) -> Result<()> {
        self.transaction(run_id, |txn| {
            txn.put(
                Key::from_user_key(run_id, key),
                Value::from_bytes(value),
            )
        })
    }

    /// M1 API: delete (implicit transaction)
    pub fn delete(&self, run_id: RunId, key: &[u8]) -> Result<()> {
        self.transaction(run_id, |txn| {
            txn.delete(Key::from_user_key(run_id, key))
        })
    }
}
```

**Tests**:
- [ ] M1 get() works
- [ ] M1 put() works
- [ ] M1 delete() works
- [ ] Automatic conflict retry works

---

### Story #59: M1 Test Suite Verification (4 hours)
**File**: `tests/m1_compatibility_tests.rs`

**Deliverable**: Run ALL M1 tests against M2

**Implementation**:
- Copy all M1 integration tests
- Run against M2 Database
- Verify no regressions

**Acceptance Criteria**:
- [ ] All M1 KV tests pass
- [ ] All M1 recovery tests pass
- [ ] All M1 benchmarks run (may be slower due to OCC overhead)

---

### Story #60: Migration Guide (3 hours)
**File**: `docs/guides/M1_TO_M2_MIGRATION.md`

**Deliverable**: User migration guide

**Content**:
- M1 API still works (no code changes required)
- New M2 transaction API available
- Performance considerations (snapshot overhead)
- When to use explicit vs implicit transactions

**Acceptance Criteria**:
- [ ] Migration guide covers all APIs
- [ ] Examples for both M1 and M2 usage
- [ ] Performance trade-offs documented

---

## Epic 12: OCC Validation & Benchmarking (4 stories, 2 days)

**Goal**: Validate correctness and measure performance

**Dependencies**: Epic 11 complete

**Deliverables**:
- Multi-threaded conflict tests
- Performance benchmarks
- M2 completion validation

### Story #61: Multi-Threaded Conflict Tests (5 hours) 🔴 FOUNDATION
**File**: `tests/concurrency_tests.rs`

**Deliverable**: Two-thread conflict scenarios

**Tests**:
```rust
#[test]
fn test_concurrent_read_write_conflict() {
    // Thread 1: Read key A, write key B
    // Thread 2: Write key A
    // Thread 2 commits first
    // Thread 1 should abort (read-write conflict on A)
}

#[test]
fn test_concurrent_write_write_conflict() {
    // Thread 1: Write key A
    // Thread 2: Write key A
    // First to commit wins
    // Second should abort (write-write conflict)
}

#[test]
fn test_concurrent_cas_conflict() {
    // Thread 1: CAS key A (expected version 5)
    // Thread 2: Write key A (changes version to 6)
    // Thread 2 commits first
    // Thread 1 should abort (CAS conflict)
}

#[test]
fn test_no_conflict_different_keys() {
    // Thread 1: Write key A
    // Thread 2: Write key B
    // Both should commit (no conflict)
}
```

**Acceptance Criteria**:
- [ ] All conflict types tested with 2 threads
- [ ] First-committer-wins verified
- [ ] Retries work correctly

---

### Story #62: Transaction Performance Benchmarks (4 hours)
**File**: `benches/transaction_benchmarks.rs`

**Deliverable**: Benchmark transaction throughput

**Benchmarks**:
```rust
fn bench_single_threaded_transactions(b: &mut Bencher) {
    // Measure: Transactions/sec with no contention
}

fn bench_concurrent_transactions_no_conflict(b: &mut Bencher) {
    // Measure: Transactions/sec with 4 threads, different keys
}

fn bench_concurrent_transactions_with_conflicts(b: &mut Bencher) {
    // Measure: Transactions/sec with 4 threads, overlapping keys
}

fn bench_snapshot_creation(b: &mut Bencher) {
    // Measure: Snapshot creation time vs data size
}
```

**Acceptance Criteria**:
- [ ] Baseline: >5K txns/sec single-threaded
- [ ] No-conflict: >10K txns/sec multi-threaded
- [ ] With conflicts: >2K txns/sec (retries reduce throughput)
- [ ] Snapshot overhead: <1ms for <10MB dataset

---

### Story #63: Memory Usage Profiling (3 hours)
**File**: `tests/memory_profiling.rs`

**Deliverable**: Measure snapshot memory overhead

**Tests**:
```rust
#[test]
fn test_snapshot_memory_overhead() {
    // Measure: Memory usage with N concurrent transactions
    // Expected: O(N * data_size) due to ClonedSnapshotView
}

#[test]
fn test_transaction_context_size() {
    // Measure: TransactionContext memory footprint
    // Ensure read/write sets don't grow unbounded
}
```

**Acceptance Criteria**:
- [ ] Memory usage documented
- [ ] Snapshot overhead measured
- [ ] Cleanup verified (no memory leaks)

---

### Story #64: M2 Completion Validation (4 hours)
**File**: `docs/milestones/M2_COMPLETION_REPORT.md`

**Deliverable**: M2 completion checklist

**Must Verify**:
- [ ] All 7 epics complete
- [ ] All 32 stories delivered
- [ ] All tests pass (unit + integration)
- [ ] Benchmarks meet targets
- [ ] M1 backwards compatibility verified
- [ ] Documentation complete

**Deliverable**: M2 completion report (similar to M1)

---

## Summary: Revised M2 Structure

### Comparison

| Metric | Original (M2_EPIC_ANALYSIS) | Revised (This Plan) |
|--------|----------------------------|---------------------|
| **Epics** | 9 | 7 |
| **Stories** | 44 | 32 |
| **Duration** | ~20 days | ~14-18 days |
| **Foundation Stories** | 9 | 7 |
| **Parallelization** | 3 Claudes/epic after foundation | 3-4 Claudes/epic after foundation |
| **WAL Scope** | Full (Phase A+B+C) | Phase A only |
| **Testing Scope** | All tests upfront | Phased (deterministic first) |

### Key Improvements

1. **Consolidated Epics**: 9 → 7 (merged related work)
2. **Phased WAL**: Only Phase A in M2 (BeginTxn, Write, CommitTxn)
3. **Phased Testing**: Deterministic tests first, fuzzing/property-based in M3
4. **Explicit Semantics**: Story #33 defines transaction semantics FIRST
5. **Backwards Compatibility**: Dedicated epic ensures M1 API works

### Critical Path

```
Epic 6 (Foundations)
  ↓
Epic 7 (Semantics) ← Can partially parallelize after Story #38
  ↓
Epic 8 (Commit) ← Can partially parallelize after Story #44
  ↓
Epic 9 (Recovery)
  ↓
Epic 10 (API) ← Can partially parallelize after Story #53
  ↓
Epic 11 (Compatibility)
  ↓
Epic 12 (Validation)
```

**Estimated Timeline**: 14-18 days (vs 20+ days original)

---

## Next Steps

1. **Review this plan** - User approval required
2. **Create Transaction Semantics Spec** - Story #33 (BLOCKS all M2 work)
3. **Update M2_PROJECT_STATUS.md** - Reflect revised epic structure
4. **Create GitHub Issues** - 7 epic issues, 32 story issues
5. **Begin Epic 6** - After semantics spec approved

**Critical**: Do NOT start Story #34 until Story #33 (Transaction Semantics Spec) is reviewed and approved.

---

## Appendix: What Changed From Original Plan

### Consolidated Epics

**Original Epic 6 + 7 → Revised Epic 6**
- Combined Infrastructure + Snapshot Management
- Rationale: Both are foundation, can't parallelize anyway

**Original Epic 8 + 9 → Revised Epic 7**
- Combined Transaction Operations + Conflict Detection
- Rationale: Validation is part of operations, tightly coupled

**Original Epic 10 + 11 → Revised Epic 8**
- Combined Commit + WAL Support
- Rationale: Commit requires WAL, can't separate

**Original Epic 12 → Revised Epic 9**
- Recovery unchanged

**Original Epic 13 + 14 → Revised Epic 10, 11, 12**
- Split into: API Integration, Backwards Compatibility, Validation
- Rationale: Compatibility deserves explicit epic for M1 verification

### Reduced Scope

**WAL**: Only Phase A in M2
- Removed: AbortTxn entries, partial transaction replay, corruption handling
- Deferred to: M3/M4

**Testing**: Only deterministic tests in M2
- Removed: Property-based tests, fuzzing, loom concurrency testing
- Deferred to: M3

### Added Scope

**Transaction Semantics Specification**: Story #33
- NEW requirement based on architectural review
- BLOCKS all M2 implementation work
- Critical for preventing inconsistent behavior

**Migration Guide**: Story #60
- Explicit backwards compatibility documentation
- Ensures M1 users can upgrade smoothly

---

## Open Questions

1. **Snapshot Memory Limits**: At what dataset size does ClonedSnapshotView become unacceptable?
   - Need to define threshold for LazySnapshotView migration
   - Suggest: >100MB dataset = warning, >1GB = error

2. **Retry Strategy Tuning**: Are 10 retries with exponential backoff optimal?
   - May need to adjust based on benchmarks
   - Consider adaptive retry based on conflict rate

3. **Transaction Timeout Default**: What is a reasonable default timeout?
   - Agent transactions expected to be <1 second
   - Suggest: Default 5 seconds, configurable

4. **Snapshot Isolation vs Serializability**: Should we document write skew scenarios?
   - Snapshot isolation allows write skew anomalies
   - May need user education on when to use explicit locking

These questions should be answered during implementation, not upfront.
