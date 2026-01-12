# Epic 6: Transaction Foundations - Implementation Prompts

**Epic Goal**: Core transaction infrastructure and snapshot isolation for M2 OCC.

**Status**: Ready to begin (blocked by Story #78)
**Dependencies**: M1 Foundation complete (Epics #1-5)

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
       └── epic-N-name        ← Epic branch (base for all story PRs)
            └── epic-N-story-X-desc  ← Story branches
```

### Critical Rules

1. **Story PRs go to EPIC branch, NOT main**
   ```bash
   # CORRECT: PR base is epic branch
   gh pr create --base epic-6-transaction-foundations --head epic-6-story-79-transaction-context

   # WRONG: Never PR directly to main
   gh pr create --base main --head epic-6-story-79-transaction-context  # ❌ NEVER DO THIS
   ```

2. **Epic branches merge to develop** (after all stories complete)
   ```bash
   git checkout develop
   git merge --no-ff epic-6-transaction-foundations
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
./scripts/complete-story.sh 79  # Creates PR to epic-6-transaction-foundations
```

**If you manually create a PR, ALWAYS verify the base branch is the epic branch, not main.**

---

## Epic 6 Overview

### Scope
- Transaction semantics specification (MUST be first)
- TransactionContext with read/write/delete/cas sets
- SnapshotView trait and ClonedSnapshotView implementation
- Transaction read operations with read-your-writes
- Transaction write operations with buffering

### Success Criteria
- [ ] Transaction semantics documented and approved
- [ ] TransactionContext struct with lifecycle management
- [ ] SnapshotView trait abstraction with ClonedSnapshotView
- [ ] Read operations with read-your-writes and read-set tracking
- [ ] Write operations buffered until commit
- [ ] All unit tests pass (>95% coverage)

### Component Breakdown
- **Story #78**: Transaction Semantics Specification 🔴 BLOCKS ALL M2
- **Story #79**: TransactionContext Core
- **Story #80**: SnapshotView Trait & ClonedSnapshot
- **Story #81**: Transaction Read Operations
- **Story #82**: Transaction Write Operations

---

## Dependency Graph

```
Phase 1 (Sequential - CRITICAL):
  Story #78 (Transaction Semantics Specification)
    └─> 🔴 BLOCKS ALL M2 IMPLEMENTATION

Phase 2 (Parallel - 3 Claudes after #78):
  Story #79 (TransactionContext Core)
  Story #80 (SnapshotView Trait & ClonedSnapshot)
    └─> Both depend on #78
    └─> Independent of each other

Phase 3 (Parallel - 2 Claudes after #79 + #80):
  Story #81 (Transaction Read Operations)
  Story #82 (Transaction Write Operations)
    └─> Both depend on #79 (TransactionContext)
    └─> #81 depends on #80 (SnapshotView)
    └─> Can run in parallel with coordination
```

---

## Parallelization Strategy

### Phase 1: Semantics Specification (Sequential) - ~4 hours
- **Story #78**: Transaction Semantics Specification
  - Defines OCC behavior, conflict rules, CAS semantics
  - **🔴 MUST BE APPROVED before any M2 code**
  - No code written until review complete

### Phase 2: Core Infrastructure (3 Claudes in PARALLEL) - ~4-5 hours wall time
Once #78 is approved:

| Story | Component | Claude | Dependencies | Estimated | Conflicts |
|-------|-----------|--------|--------------|-----------|-----------|
| #79 | TransactionContext Core | Available | #78 | 4 hours | Creates transaction.rs (new file) |
| #80 | SnapshotView & ClonedSnapshot | Available | #78 | 5 hours | Creates snapshot.rs (new file) |

**Why parallel**:
- #79 creates new file `transaction.rs` (no conflicts)
- #80 creates new file `snapshot.rs` (no conflicts)
- Both start from concurrency crate skeleton
- Different concerns, no merge conflicts

### Phase 3: Operations (2 Claudes in PARALLEL) - ~4 hours wall time
After #79 and #80 merge:

| Story | Component | Claude | Dependencies | Estimated | Conflicts |
|-------|-----------|--------|--------------|-----------|-----------|
| #81 | Transaction Read Operations | Available | #79, #80 | 4 hours | Adds to transaction.rs |
| #82 | Transaction Write Operations | Available | #79 | 4 hours | Adds to transaction.rs |

**⚠️ CRITICAL: transaction.rs Coordination**

Stories #81 and #82 both modify `transaction.rs`:
- **#81 adds**: `get()`, `scan_prefix()`, read-set tracking
- **#82 adds**: `put()`, `delete()`, `cas()`, write buffering

**If running in parallel**:
1. Coordinate who modifies transaction.rs first
2. Second story must pull latest before starting
3. Resolve merge conflicts carefully during PR to `epic-6-transaction-foundations`

**Alternative**: Run #81 → #82 sequentially to avoid conflicts (adds ~4 hours to wall time)

**Epic 6 Total**: ~17-21 hours sequential, ~12-14 hours wall time with 3 Claudes

---

## Story #78: Transaction Semantics Specification

**Branch**: `story-78-transaction-semantics`
**Estimated Time**: 6 hours (increased - this is critical)
**Dependencies**: None
**🔴 CRITICAL**: This story BLOCKS ALL M2 implementation

### Start Command
```bash
/opt/homebrew/bin/gh issue view 78
./scripts/start-story.sh 6 78 transaction-semantics
```

### Context
This is the **CRITICAL FIRST STORY** for M2. It defines the semantic contract that ALL M2 code must follow. **NO M2 CODE should be written until this specification is reviewed and approved.**

**Why this matters**: Ambiguous semantics lead to bugs that look like correct behavior. Future developers will try to "fix" things that aren't broken. This document prevents that.

### Required Sections

The document **MUST** explicitly define ALL of the following:

---

#### Section 1: Isolation Level Declaration

**EXPLICITLY STATE**:
```markdown
## 1. Isolation Level: Snapshot Isolation

**We implement Snapshot Isolation (SI), NOT Serializability.**

This is a deliberate design choice. Snapshot Isolation:
- ✅ Prevents dirty reads
- ✅ Prevents non-repeatable reads
- ✅ Prevents lost updates
- ❌ Does NOT prevent write skew anomalies
- ❌ Does NOT prevent phantom reads
- ❌ Is NOT serializable

**We explicitly do NOT guarantee serializability.**

This means some anomalies are ALLOWED and CORRECT behavior:
- Write skew (T1 reads A, writes B; T2 reads B, writes A - both commit)
- Phantom reads (new keys appearing in range scans)

Do NOT attempt to "fix" these behaviors - they are by design.
```

---

#### Section 2: Visibility Rules (Exhaustive)

**MUST answer every visibility question**:

```markdown
## 2. Visibility Rules

### 2.1 What a Transaction ALWAYS Sees
- Committed data as of `start_version` (the snapshot)
- Its own uncommitted writes (read-your-writes)
- Its own uncommitted deletes (deleted keys return None)

### 2.2 What a Transaction NEVER Sees
- Uncommitted writes from other transactions
- Writes committed AFTER this transaction's `start_version`
- Partial writes (atomicity guarantee)

### 2.3 What a Transaction MAY See (Anomalies)
- Phantom keys in range scans (keys added by concurrent txns after commit)
- Write skew results (reads that would conflict under serializability)

### 2.4 Visibility Examples

**Example 1: Snapshot Isolation**
```
Timeline:
  T1: BEGIN (start_version=100)
  T2: BEGIN (start_version=100)
  T2: PUT(key_a, "new_value")
  T2: COMMIT (now version=101)
  T1: GET(key_a)  → Returns value at version 100, NOT "new_value"
  T1: COMMIT      → May succeed (no conflict if T1 didn't read key_a before)
```

**Example 2: Read-Your-Writes**
```
  T1: BEGIN
  T1: PUT(key_a, "value1")
  T1: GET(key_a)  → Returns "value1" (own uncommitted write)
  T1: DELETE(key_a)
  T1: GET(key_a)  → Returns None (own uncommitted delete)
```

**Example 3: Write Skew (ALLOWED)**
```
Initial: balance_a = 100, balance_b = 100
Constraint: balance_a + balance_b >= 100

  T1: BEGIN (sees balance_a=100, balance_b=100)
  T2: BEGIN (sees balance_a=100, balance_b=100)
  T1: READ(balance_a) → 100, checks 100+100 >= 100 ✓
  T2: READ(balance_b) → 100, checks 100+100 >= 100 ✓
  T1: WRITE(balance_b = 0)
  T2: WRITE(balance_a = 0)
  T1: COMMIT → SUCCESS (balance_b changed, but T1 didn't read it)
  T2: COMMIT → SUCCESS (balance_a changed, but T2 didn't read it)

Final: balance_a = 0, balance_b = 0  → VIOLATES CONSTRAINT

This is CORRECT behavior under Snapshot Isolation.
Do NOT try to prevent this - it's by design.
```
```

---

#### Section 3: Conflict Semantics (Precise)

**MUST define exactly when conflicts occur**:

```markdown
## 3. Conflict Detection

### 3.1 When a Transaction ABORTS

A transaction aborts at COMMIT time if ANY of these are true:

1. **Read-Write Conflict**:
   - T1 read key K at version V
   - Current version of K is now V' where V' != V
   - Result: T1 aborts

2. **Write-Write Conflict** (with read):
   - T1 read key K, then wrote key K
   - Another transaction modified K after T1's start_version
   - Result: T1 aborts

3. **CAS Conflict**:
   - T1 called CAS(K, expected_version=V, new_value)
   - Current version of K != V
   - Result: T1 aborts

4. **Delete Conflict**:
   - T1 read key K at version V, then deleted K
   - Current version of K != V
   - Result: T1 aborts

### 3.2 When a Transaction DOES NOT Conflict

These are NOT conflicts:

1. **Blind Write** (write without read):
   - T1 writes key K without ever reading it
   - T2 also writes key K and commits first
   - Result: T1 commits successfully (overwrites T2's value)
   - **This is first-committer-wins for the READ, not the write**

2. **Different Keys**:
   - T1 reads/writes key A
   - T2 reads/writes key B
   - Result: Both commit (no conflict)

3. **Read-Only Transaction**:
   - T1 only reads, never writes
   - Result: Always commits (nothing to validate for writes)

### 3.3 First-Committer-Wins Explained

"First committer wins" means:
- The first transaction to COMMIT gets its writes applied
- Later transactions that CONFLICT with those writes must abort
- Conflict is based on READ-SET, not write-set

```
T1: BEGIN, READ(K), WRITE(K)
T2: BEGIN, READ(K), WRITE(K)
T1: COMMIT → SUCCESS (first to commit)
T2: COMMIT → ABORT (read K, but K changed since T2's read)
```

vs.

```
T1: BEGIN, WRITE(K)  // blind write, no read
T2: BEGIN, WRITE(K)  // blind write, no read
T1: COMMIT → SUCCESS
T2: COMMIT → SUCCESS (no read conflict, overwrites T1)
```

### 3.4 CAS Interaction with Read/Write Validation

CAS operations are validated SEPARATELY from read-set:
- CAS checks expected_version against CURRENT storage version
- CAS does NOT automatically add to read_set
- If you want read-set protection, explicitly READ before CAS

```
// CAS without read - only version check
txn.cas(key, expected_version=5, new_value)?;
// Fails if current version != 5, succeeds otherwise

// CAS with read protection
let val = txn.get(key)?;  // Adds to read_set
txn.cas(key, val.version, new_value)?;
// Fails if version changed since read OR if cas version wrong
```
```

---

#### Section 4: Implicit Transaction Semantics

**MUST specify how M1-style operations work in M2**:

```markdown
## 4. Implicit Transactions

### 4.1 What is an Implicit Transaction?

M1-style operations (`db.put()`, `db.get()`) continue to work in M2.
Each operation is wrapped in an implicit single-operation transaction.

### 4.2 Implicit Transaction Behavior

**db.put(key, value)**:
```rust
// Equivalent to:
db.transaction(|txn| {
    txn.put(key, value)?;
    Ok(())
})?;
```
- Opens a transaction
- Writes to write_set
- Commits immediately
- **IS atomic** (all-or-nothing)
- **CAN conflict** if another txn modifies same key between begin/commit
- In practice: very short window, conflicts rare

**db.get(key)**:
```rust
// Equivalent to:
db.transaction(|txn| {
    txn.get(key)
})?;
```
- Opens a transaction
- Creates a snapshot at current version
- Reads from snapshot
- Commits (read-only, always succeeds)
- Returns consistent point-in-time value

**db.delete(key)**:
```rust
// Equivalent to:
db.transaction(|txn| {
    txn.delete(key)?;
    Ok(())
})?;
```
- Same as put, but deletes

### 4.3 Can Implicit Transactions Conflict?

**Yes**, but rarely in practice:

```
Thread 1: db.put(key, "A")  // BEGIN, PUT, COMMIT
Thread 2: db.put(key, "B")  // BEGIN, PUT, COMMIT

Possible outcomes:
1. T1 commits, then T2 commits → key = "B"
2. T2 commits, then T1 commits → key = "A"
3. Both try to commit simultaneously → one wins, one retries
```

Implicit transactions use the same retry logic as explicit transactions.

### 4.4 Mixing Implicit and Explicit

```rust
// This is safe:
db.put(key_a, "value")?;  // Implicit txn, commits

db.transaction(|txn| {
    let v = txn.get(key_a)?;  // Sees committed value from above
    txn.put(key_b, v)?;
    Ok(())
})?;
```

```rust
// This is NOT recommended:
db.transaction(|txn| {
    txn.put(key_a, "in_txn")?;
    db.put(key_b, "implicit")?;  // DIFFERENT transaction! Commits immediately!
    // key_b is now visible to other transactions
    // but key_a is not (still in txn's write_set)
    Ok(())
})?;
```
```

---

#### Section 5: Replay Semantics

**MUST define how deterministic replay works**:

```markdown
## 5. Replay Semantics

### 5.1 What is Replay?

Replay reconstructs database state by re-applying WAL entries.
This is used for:
- Crash recovery
- Point-in-time recovery
- Debugging/auditing

### 5.2 Replay Rules

**Rule 1: Replays do NOT re-run conflict detection**
- WAL contains only COMMITTED transactions
- If a txn is in the WAL, it already passed validation
- Replay applies writes directly, no validation

**Rule 2: Replays apply commit decisions, not re-execute logic**
- WAL entry: `Write { key: K, value: V, version: 10 }`
- Replay: `storage.put(K, V, version=10)`
- We don't re-run the transaction closure

**Rule 3: Replays are single-threaded**
- WAL is sequential
- Replay processes entries in order
- No concurrency during replay

**Rule 4: Versions are preserved exactly**
- WAL records the version assigned at commit time
- Replay uses that same version
- After replay, `storage.current_version()` matches pre-crash state

### 5.3 WAL Entry Format (M2 Phase A)

```rust
enum WALEntry {
    BeginTxn { txn_id: u64, run_id: RunId, timestamp: u64 },
    Write { key: Key, value: Value, version: u64 },
    Delete { key: Key, version: u64 },
    CommitTxn { txn_id: u64, commit_version: u64 },
    // No AbortTxn in M2 - aborted txns write nothing
}
```

### 5.4 Recovery Algorithm

```
1. Load snapshot (if exists)
2. Open WAL, find entries after snapshot
3. Track incomplete transactions (BeginTxn without CommitTxn)
4. For each complete transaction (has CommitTxn):
   - Apply all Write/Delete entries
   - Update version counter
5. Discard incomplete transactions (crash during commit)
6. Database ready
```

### 5.5 Incomplete Transaction Handling

If crash occurs during commit:
- Some Write entries may be in WAL
- But no CommitTxn entry
- Recovery: **DISCARD all writes from that txn**
- This is why we don't need AbortTxn entries in M2
```

---

#### Section 6: Version Semantics

**MUST define how versions work**:

```markdown
## 6. Version Semantics

### 6.1 Global Version Counter

- Single monotonic counter for entire database
- Incremented on each COMMIT (not each write)
- Used for snapshot isolation

### 6.2 Key Versions

Each key has its own version:
- Incremented when that key is written
- Stored with the value: `VersionedValue { value, version, expires_at }`

### 6.3 Snapshot Version vs Key Version

```
Global version: 100
  key_a: version 50  (written at global version 50)
  key_b: version 80  (written at global version 80)
  key_c: version 100 (written at global version 100)

T1 begins at global version 100 (start_version = 100)
T1 sees:
  key_a at version 50
  key_b at version 80
  key_c at version 100

If T2 commits and writes key_a (now version 101):
T1 still sees key_a at version 50 (snapshot)
T1's read_set has { key_a: 50 }
At T1 commit: current key_a version (101) != read version (50) → CONFLICT
```

### 6.4 Version 0 Semantics

Version 0 has special meaning:
- `key.version == 0` means "key does not exist"
- `CAS(key, expected_version=0, value)` means "create only if not exists"
- Reading non-existent key records version 0 in read_set
```

---

### Implementation Steps

1. **Create the specification document**
   ```bash
   mkdir -p docs/architecture
   ```

2. **Write `docs/architecture/M2_TRANSACTION_SEMANTICS.md`** with ALL sections above

3. **Include conflict scenario examples from issue #78**

4. **Add decision log section**
   ```markdown
   ## Appendix: Design Decisions

   ### Why Snapshot Isolation (not Serializable)?
   - Simpler implementation
   - Better performance (no predicate locking)
   - Acceptable for agent workloads
   - Write skew rare in practice

   ### Why First-Committer-Wins?
   - No deadlocks (optimistic)
   - Simple conflict resolution
   - Natural for OCC

   ### Why No AbortTxn WAL Entry (M2)?
   - Aborted txns write nothing anyway
   - Recovery just discards incomplete txns
   - Simpler WAL format
   - Phase B (M3+) may add explicit AbortTxn
   ```

### Testing Requirements

**CRITICAL TESTING RULE**:
- **NEVER adjust tests to make them pass**
- If a test fails, the CODE must be fixed, not the test
- Tests define correct behavior - failed tests reveal bugs in implementation
- Only adjust a test if the test itself is incorrect (wrong assertion logic)

**Validation checklist**:
- [ ] Section 1: Isolation level explicitly declared as "NOT serializable"
- [ ] Section 2: All visibility rules defined (ALWAYS/NEVER/MAY see)
- [ ] Section 2: Write skew example included and marked as CORRECT behavior
- [ ] Section 3: All conflict conditions precisely defined
- [ ] Section 3: First-committer-wins explained with examples
- [ ] Section 3: CAS interaction with read-set documented
- [ ] Section 4: Implicit transactions fully specified
- [ ] Section 4: db.put(), db.get(), db.delete() behavior defined
- [ ] Section 4: Implicit transaction conflict behavior documented
- [ ] Section 5: Replay rules defined (no re-validation)
- [ ] Section 5: Single-threaded replay stated
- [ ] Section 5: Version preservation documented
- [ ] Section 6: Version semantics (global vs key) explained
- [ ] Section 6: Version 0 meaning documented
- [ ] No ambiguous language ("proper", "correct", "good")
- [ ] Every statement is testable

### Validation
```bash
# Review document structure
cat docs/architecture/M2_TRANSACTION_SEMANTICS.md

# Check for ambiguous words that should be removed
grep -i "proper\|correct\|good\|appropriate" docs/architecture/M2_TRANSACTION_SEMANTICS.md
# Should return nothing - these words are too vague

# Verify document has all required sections
grep "^## " docs/architecture/M2_TRANSACTION_SEMANTICS.md
```

### Complete Story
```bash
./scripts/complete-story.sh 78
```

**⚠️ APPROVAL GATE**: Do not proceed to Story #79 until this document is reviewed and approved. The semantics document is the CONTRACT that all M2 code must follow.

---

## Story #79: TransactionContext Core

**Branch**: `story-79-transaction-context`
**Estimated Time**: 4 hours
**Dependencies**: Story #78 (Transaction Semantics Spec approved)

### Start Command
```bash
/opt/homebrew/bin/gh issue view 79
./scripts/start-story.sh 6 79 transaction-context
```

### ⚠️ PREREQUISITE: Read the Semantics Spec

**Before writing ANY code**, read and understand:
- `docs/architecture/M2_TRANSACTION_SEMANTICS.md`

This story implements the **data structures** defined in that spec. If anything is unclear, refer to the spec. If the spec is ambiguous, clarify it BEFORE coding.

### Context
TransactionContext is the core type for M2 OCC. It holds read_set, write_set, delete_set, cas_set and manages transaction state (Active, Validating, Committed, Aborted). This story creates the struct and lifecycle management only - read/write operations come in stories #81/#82.

### Semantics This Story Must Implement

From `M2_TRANSACTION_SEMANTICS.md`:

1. **Transaction Lifecycle States** (Section 6):
   - `Active` → can read/write
   - `Validating` → checking for conflicts
   - `Committed` → successfully applied
   - `Aborted { reason }` → failed, buffered writes discarded

2. **State Transitions**:
   - `Active → Validating` (begin commit)
   - `Validating → Committed` (validation passed)
   - `Validating → Aborted` (conflict detected)
   - `Active → Aborted` (user abort or error)
   - Cannot transition FROM `Committed` or `Aborted`

3. **Version Semantics** (Section 6):
   - `start_version` = global version at transaction begin
   - Used for snapshot isolation

### Implementation Steps

1. **Create concurrency crate module structure**
   ```bash
   # Verify crate exists
   ls crates/concurrency/src/
   ```

2. **Update `crates/concurrency/src/lib.rs`**
   ```rust
   //! Concurrency layer for in-mem
   //!
   //! This crate implements optimistic concurrency control (OCC) with:
   //! - TransactionContext: Read/write set tracking
   //! - Snapshot isolation
   //! - Conflict detection at commit time
   //! - Compare-and-swap (CAS) operations

   #![warn(missing_docs)]
   #![warn(clippy::all)]

   pub mod transaction;
   pub mod snapshot;  // Created in Story #80

   pub use transaction::{TransactionContext, TransactionStatus, CASOperation};
   ```

3. **Create `crates/concurrency/src/transaction.rs`**
   ```rust
   //! Transaction context for OCC

   use in_mem_core::error::Result;
   use in_mem_core::types::{Key, RunId};
   use in_mem_core::value::Value;
   use std::collections::{HashMap, HashSet};

   /// Status of a transaction in its lifecycle
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub enum TransactionStatus {
       /// Transaction is executing, can read/write
       Active,
       /// Transaction is being validated for conflicts
       Validating,
       /// Transaction committed successfully
       Committed,
       /// Transaction was aborted
       Aborted { reason: String },
   }

   /// A compare-and-swap operation to be validated at commit
   #[derive(Debug, Clone)]
   pub struct CASOperation {
       /// Key to CAS
       pub key: Key,
       /// Expected version (0 = key must not exist)
       pub expected_version: u64,
       /// New value to write if version matches
       pub new_value: Value,
   }

   /// Transaction context for OCC with snapshot isolation
   ///
   /// Tracks all reads, writes, deletes, and CAS operations for a transaction.
   /// Validation and commit happen at transaction end.
   pub struct TransactionContext {
       // Identity
       /// Unique transaction ID
       pub txn_id: u64,
       /// Run this transaction belongs to
       pub run_id: RunId,

       // Snapshot isolation
       /// Version at transaction start (snapshot version)
       pub start_version: u64,

       // Operation tracking
       /// Keys read and their versions (for validation)
       pub read_set: HashMap<Key, u64>,
       /// Keys written with their new values (buffered)
       pub write_set: HashMap<Key, Value>,
       /// Keys to delete (buffered)
       pub delete_set: HashSet<Key>,
       /// CAS operations to validate and apply
       pub cas_set: Vec<CASOperation>,

       // State
       /// Current transaction status
       pub status: TransactionStatus,
   }

   impl TransactionContext {
       /// Create a new transaction context
       ///
       /// # Arguments
       /// * `txn_id` - Unique transaction identifier
       /// * `run_id` - Run this transaction belongs to
       /// * `start_version` - Snapshot version at transaction start
       pub fn new(txn_id: u64, run_id: RunId, start_version: u64) -> Self {
           TransactionContext {
               txn_id,
               run_id,
               start_version,
               read_set: HashMap::new(),
               write_set: HashMap::new(),
               delete_set: HashSet::new(),
               cas_set: Vec::new(),
               status: TransactionStatus::Active,
           }
       }

       /// Check if transaction is in Active state
       pub fn is_active(&self) -> bool {
           matches!(self.status, TransactionStatus::Active)
       }

       /// Check if transaction can accept operations
       pub fn ensure_active(&self) -> Result<()> {
           if self.is_active() {
               Ok(())
           } else {
               Err(in_mem_core::error::Error::InvalidState(
                   format!("Transaction {} is not active: {:?}", self.txn_id, self.status)
               ))
           }
       }

       /// Transition to Validating state
       ///
       /// # Errors
       /// Returns error if not in Active state
       pub fn mark_validating(&mut self) -> Result<()> {
           self.ensure_active()?;
           self.status = TransactionStatus::Validating;
           Ok(())
       }

       /// Transition to Committed state
       ///
       /// # Errors
       /// Returns error if not in Validating state
       pub fn mark_committed(&mut self) -> Result<()> {
           match &self.status {
               TransactionStatus::Validating => {
                   self.status = TransactionStatus::Committed;
                   Ok(())
               }
               _ => Err(in_mem_core::error::Error::InvalidState(
                   format!("Cannot commit transaction {} from state {:?}", self.txn_id, self.status)
               )),
           }
       }

       /// Transition to Aborted state
       ///
       /// # Arguments
       /// * `reason` - Human-readable reason for abort
       ///
       /// # Errors
       /// Returns error if already Committed or Aborted
       pub fn mark_aborted(&mut self, reason: String) -> Result<()> {
           match &self.status {
               TransactionStatus::Committed => Err(in_mem_core::error::Error::InvalidState(
                   format!("Cannot abort committed transaction {}", self.txn_id)
               )),
               TransactionStatus::Aborted { .. } => Err(in_mem_core::error::Error::InvalidState(
                   format!("Transaction {} already aborted", self.txn_id)
               )),
               _ => {
                   self.status = TransactionStatus::Aborted { reason };
                   Ok(())
               }
           }
       }

       /// Get the number of keys in the read set
       pub fn read_count(&self) -> usize {
           self.read_set.len()
       }

       /// Get the number of keys in the write set
       pub fn write_count(&self) -> usize {
           self.write_set.len()
       }

       /// Get the number of keys in the delete set
       pub fn delete_count(&self) -> usize {
           self.delete_set.len()
       }

       /// Get the number of CAS operations
       pub fn cas_count(&self) -> usize {
           self.cas_set.len()
       }

       /// Check if transaction has any pending operations
       pub fn has_pending_operations(&self) -> bool {
           !self.write_set.is_empty() || !self.delete_set.is_empty() || !self.cas_set.is_empty()
       }
   }

   #[cfg(test)]
   mod tests {
       use super::*;
       use in_mem_core::types::RunId;

       fn create_test_txn() -> TransactionContext {
           let run_id = RunId::new();
           TransactionContext::new(1, run_id, 100)
       }

       #[test]
       fn test_new_transaction_is_active() {
           let txn = create_test_txn();
           assert!(txn.is_active());
           assert_eq!(txn.txn_id, 1);
           assert_eq!(txn.start_version, 100);
       }

       #[test]
       fn test_new_transaction_has_empty_sets() {
           let txn = create_test_txn();
           assert_eq!(txn.read_count(), 0);
           assert_eq!(txn.write_count(), 0);
           assert_eq!(txn.delete_count(), 0);
           assert_eq!(txn.cas_count(), 0);
           assert!(!txn.has_pending_operations());
       }

       #[test]
       fn test_state_transition_active_to_validating() {
           let mut txn = create_test_txn();
           assert!(txn.mark_validating().is_ok());
           assert!(!txn.is_active());
           assert!(matches!(txn.status, TransactionStatus::Validating));
       }

       #[test]
       fn test_state_transition_validating_to_committed() {
           let mut txn = create_test_txn();
           txn.mark_validating().unwrap();
           assert!(txn.mark_committed().is_ok());
           assert!(matches!(txn.status, TransactionStatus::Committed));
       }

       #[test]
       fn test_state_transition_active_to_aborted() {
           let mut txn = create_test_txn();
           assert!(txn.mark_aborted("test abort".to_string()).is_ok());
           assert!(matches!(txn.status, TransactionStatus::Aborted { .. }));
       }

       #[test]
       fn test_state_transition_validating_to_aborted() {
           let mut txn = create_test_txn();
           txn.mark_validating().unwrap();
           assert!(txn.mark_aborted("conflict detected".to_string()).is_ok());
           assert!(matches!(txn.status, TransactionStatus::Aborted { .. }));
       }

       #[test]
       fn test_cannot_validating_from_committed() {
           let mut txn = create_test_txn();
           txn.mark_validating().unwrap();
           txn.mark_committed().unwrap();
           assert!(txn.mark_validating().is_err());
       }

       #[test]
       fn test_cannot_commit_from_active() {
           let mut txn = create_test_txn();
           // Cannot commit directly from Active, must validate first
           assert!(txn.mark_committed().is_err());
       }

       #[test]
       fn test_cannot_commit_from_aborted() {
           let mut txn = create_test_txn();
           txn.mark_aborted("test".to_string()).unwrap();
           assert!(txn.mark_committed().is_err());
       }

       #[test]
       fn test_cannot_abort_committed_transaction() {
           let mut txn = create_test_txn();
           txn.mark_validating().unwrap();
           txn.mark_committed().unwrap();
           assert!(txn.mark_aborted("too late".to_string()).is_err());
       }

       #[test]
       fn test_cannot_abort_already_aborted() {
           let mut txn = create_test_txn();
           txn.mark_aborted("first abort".to_string()).unwrap();
           assert!(txn.mark_aborted("second abort".to_string()).is_err());
       }

       #[test]
       fn test_ensure_active_succeeds_when_active() {
           let txn = create_test_txn();
           assert!(txn.ensure_active().is_ok());
       }

       #[test]
       fn test_ensure_active_fails_when_not_active() {
           let mut txn = create_test_txn();
           txn.mark_validating().unwrap();
           assert!(txn.ensure_active().is_err());
       }
   }
   ```

4. **Add InvalidState error variant to core if needed**
   Check `crates/core/src/error.rs` and add:
   ```rust
   /// Invalid state transition
   InvalidState(String),
   ```

5. **Run tests**
   ```bash
   ~/.cargo/bin/cargo test -p in-mem-concurrency --lib transaction
   ```

### Testing Requirements

**CRITICAL TESTING RULE**:
- **NEVER adjust tests to make them pass**
- If a test fails, the CODE must be fixed, not the test
- Tests define correct behavior - failed tests reveal bugs in implementation
- Only adjust a test if the test itself is incorrect (wrong assertion logic)

**Coverage target**: ≥95% for TransactionContext

**Test checklist**:
- [ ] New transaction is in Active state
- [ ] New transaction has empty sets
- [ ] State transition: Active → Validating works
- [ ] State transition: Validating → Committed works
- [ ] State transition: Active → Aborted works
- [ ] State transition: Validating → Aborted works
- [ ] Cannot transition from Committed state
- [ ] Cannot transition from Aborted state
- [ ] ensure_active() works correctly

### Validation
```bash
# Run concurrency tests
~/.cargo/bin/cargo test -p in-mem-concurrency

# Check build
~/.cargo/bin/cargo build --all

# Check clippy
~/.cargo/bin/cargo clippy -p in-mem-concurrency -- -D warnings

# Check formatting
~/.cargo/bin/cargo fmt --check
```

### Complete Story
```bash
./scripts/complete-story.sh 79
```

---

## Story #80: SnapshotView Trait & ClonedSnapshot

**Branch**: `story-80-snapshot-view`
**Estimated Time**: 5 hours
**Dependencies**: Story #78 (Transaction Semantics Spec approved)

### Start Command
```bash
/opt/homebrew/bin/gh issue view 80
./scripts/start-story.sh 6 80 snapshot-view
```

### ⚠️ PREREQUISITE: Read the Semantics Spec

**Before writing ANY code**, read and understand:
- `docs/architecture/M2_TRANSACTION_SEMANTICS.md` (Sections 1, 2)

This story implements **snapshot isolation**. The spec defines exactly what a snapshot sees and doesn't see.

### Semantics This Story Must Implement

From `M2_TRANSACTION_SEMANTICS.md`:

1. **What a Snapshot ALWAYS Provides** (Section 2.1):
   - Committed data as of `start_version`
   - Consistent point-in-time view
   - Repeatable reads (same key returns same value)

2. **What a Snapshot NEVER Shows** (Section 2.2):
   - Writes committed AFTER `start_version`
   - Uncommitted writes from other transactions
   - Partial writes

3. **Isolation Guarantee** (Section 1):
   - This is Snapshot Isolation, NOT Serializability
   - Phantoms are allowed (but not relevant for ClonedSnapshot)

### Context
SnapshotView abstracts the snapshot mechanism. M2 uses ClonedSnapshotView (deep copy of BTreeMap). This follows TDD - trait first, then implementation. The trait abstraction enables future LazySnapshotView optimization.

**Known Limitations (Acceptable for M2)**:
- O(data_size) memory per active transaction
- O(data_size) time per snapshot creation

### Implementation Steps

1. **Create `crates/concurrency/src/snapshot.rs`**
   ```rust
   //! Snapshot isolation for OCC transactions
   //!
   //! This module provides snapshot views for transactions. M2 uses
   //! ClonedSnapshotView (deep copy). Future: LazySnapshotView.

   use in_mem_core::error::Result;
   use in_mem_core::types::Key;
   use in_mem_storage::versioned::VersionedValue;
   use std::collections::BTreeMap;
   use std::sync::Arc;

   /// Trait for snapshot implementations
   ///
   /// A snapshot provides a consistent, version-bounded view of storage.
   /// Implementations may use different strategies:
   /// - ClonedSnapshotView: Deep copy (M2)
   /// - LazySnapshotView: Version-bounded reads (future)
   pub trait SnapshotView: Send + Sync {
       /// Get a value from the snapshot
       fn get(&self, key: &Key) -> Result<Option<VersionedValue>>;

       /// Scan keys with a prefix
       fn scan_prefix(&self, prefix: &Key) -> Result<Vec<(Key, VersionedValue)>>;

       /// Get the snapshot version
       fn version(&self) -> u64;
   }

   /// M2 Implementation: Clone-based snapshot
   ///
   /// KNOWN LIMITATION: Clones entire BTreeMap at snapshot creation
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

   impl ClonedSnapshotView {
       /// Create a new snapshot by cloning the provided data
       ///
       /// # Arguments
       /// * `version` - The snapshot version
       /// * `data` - The data to clone
       ///
       /// # Note
       /// This is O(data_size) in both time and memory.
       pub fn new(version: u64, data: BTreeMap<Key, VersionedValue>) -> Self {
           ClonedSnapshotView {
               version,
               data: Arc::new(data),
           }
       }

       /// Create an empty snapshot at a given version
       pub fn empty(version: u64) -> Self {
           ClonedSnapshotView {
               version,
               data: Arc::new(BTreeMap::new()),
           }
       }

       /// Get the number of keys in the snapshot
       pub fn len(&self) -> usize {
           self.data.len()
       }

       /// Check if the snapshot is empty
       pub fn is_empty(&self) -> bool {
           self.data.is_empty()
       }
   }

   impl SnapshotView for ClonedSnapshotView {
       fn get(&self, key: &Key) -> Result<Option<VersionedValue>> {
           Ok(self.data.get(key).cloned())
       }

       fn scan_prefix(&self, prefix: &Key) -> Result<Vec<(Key, VersionedValue)>> {
           // Use BTreeMap range to efficiently scan by prefix
           let prefix_bytes = prefix.to_bytes();

           let results: Vec<(Key, VersionedValue)> = self.data
               .iter()
               .filter(|(k, _)| k.to_bytes().starts_with(&prefix_bytes))
               .map(|(k, v)| (k.clone(), v.clone()))
               .collect();

           Ok(results)
       }

       fn version(&self) -> u64 {
           self.version
       }
   }

   #[cfg(test)]
   mod tests {
       use super::*;
       use in_mem_core::types::{Key, Namespace, RunId, TypeTag};
       use in_mem_core::value::Value;

       fn create_test_key(user_key: &[u8]) -> Key {
           let ns = Namespace::new("test", "app", "agent", RunId::new());
           Key::new_kv(ns, user_key)
       }

       fn create_test_value(data: &[u8]) -> VersionedValue {
           VersionedValue {
               value: Value::Bytes(data.to_vec()),
               version: 1,
               expires_at: None,
           }
       }

       fn create_test_snapshot() -> ClonedSnapshotView {
           let mut data = BTreeMap::new();

           let key1 = create_test_key(b"key1");
           let key2 = create_test_key(b"key2");
           let key3 = create_test_key(b"other");

           data.insert(key1, VersionedValue {
               value: Value::Bytes(b"value1".to_vec()),
               version: 10,
               expires_at: None,
           });
           data.insert(key2, VersionedValue {
               value: Value::Bytes(b"value2".to_vec()),
               version: 20,
               expires_at: None,
           });
           data.insert(key3, VersionedValue {
               value: Value::Bytes(b"other_value".to_vec()),
               version: 30,
               expires_at: None,
           });

           ClonedSnapshotView::new(100, data)
       }

       #[test]
       fn test_empty_snapshot() {
           let snapshot = ClonedSnapshotView::empty(50);
           assert_eq!(snapshot.version(), 50);
           assert!(snapshot.is_empty());
           assert_eq!(snapshot.len(), 0);
       }

       #[test]
       fn test_snapshot_version() {
           let snapshot = create_test_snapshot();
           assert_eq!(snapshot.version(), 100);
       }

       #[test]
       fn test_snapshot_len() {
           let snapshot = create_test_snapshot();
           assert_eq!(snapshot.len(), 3);
           assert!(!snapshot.is_empty());
       }

       #[test]
       fn test_get_existing_key() {
           let snapshot = create_test_snapshot();
           let key1 = create_test_key(b"key1");

           let result = snapshot.get(&key1).unwrap();
           assert!(result.is_some());

           let vv = result.unwrap();
           assert_eq!(vv.version, 10);
           match &vv.value {
               Value::Bytes(data) => assert_eq!(data, b"value1"),
               _ => panic!("Expected Bytes value"),
           }
       }

       #[test]
       fn test_get_nonexistent_key() {
           let snapshot = create_test_snapshot();
           let key = create_test_key(b"nonexistent");

           let result = snapshot.get(&key).unwrap();
           assert!(result.is_none());
       }

       #[test]
       fn test_scan_prefix() {
           let snapshot = create_test_snapshot();
           let prefix = create_test_key(b"key");

           let results = snapshot.scan_prefix(&prefix).unwrap();
           // Should find key1 and key2, but not "other"
           assert_eq!(results.len(), 2);
       }

       #[test]
       fn test_scan_prefix_no_matches() {
           let snapshot = create_test_snapshot();
           let prefix = create_test_key(b"nonexistent");

           let results = snapshot.scan_prefix(&prefix).unwrap();
           assert!(results.is_empty());
       }

       #[test]
       fn test_snapshot_is_independent() {
           // Create original data
           let mut data = BTreeMap::new();
           let key = create_test_key(b"key");
           data.insert(key.clone(), VersionedValue {
               value: Value::Bytes(b"original".to_vec()),
               version: 1,
               expires_at: None,
           });

           // Create snapshot
           let snapshot = ClonedSnapshotView::new(100, data.clone());

           // Modify original data (simulating another transaction)
           data.insert(key.clone(), VersionedValue {
               value: Value::Bytes(b"modified".to_vec()),
               version: 2,
               expires_at: None,
           });

           // Snapshot should still see original value
           let result = snapshot.get(&key).unwrap().unwrap();
           match &result.value {
               Value::Bytes(d) => assert_eq!(d, b"original"),
               _ => panic!("Expected Bytes value"),
           }
       }

       #[test]
       fn test_multiple_snapshots_independent() {
           let mut data = BTreeMap::new();
           let key = create_test_key(b"key");
           data.insert(key.clone(), VersionedValue {
               value: Value::Bytes(b"v1".to_vec()),
               version: 1,
               expires_at: None,
           });

           let snapshot1 = ClonedSnapshotView::new(100, data.clone());

           // Update data for second snapshot
           data.insert(key.clone(), VersionedValue {
               value: Value::Bytes(b"v2".to_vec()),
               version: 2,
               expires_at: None,
           });

           let snapshot2 = ClonedSnapshotView::new(200, data);

           // Each snapshot sees its own version
           let v1 = snapshot1.get(&key).unwrap().unwrap();
           let v2 = snapshot2.get(&key).unwrap().unwrap();

           match (&v1.value, &v2.value) {
               (Value::Bytes(d1), Value::Bytes(d2)) => {
                   assert_eq!(d1, b"v1");
                   assert_eq!(d2, b"v2");
               }
               _ => panic!("Expected Bytes values"),
           }

           assert_eq!(snapshot1.version(), 100);
           assert_eq!(snapshot2.version(), 200);
       }
   }
   ```

2. **Update `crates/concurrency/src/lib.rs`**
   ```rust
   pub mod snapshot;
   pub mod transaction;

   pub use snapshot::{ClonedSnapshotView, SnapshotView};
   pub use transaction::{TransactionContext, TransactionStatus, CASOperation};
   ```

3. **Add Key::to_bytes() method to core if needed**
   Check `crates/core/src/types.rs` and ensure Key has serialization support.

4. **Run tests**
   ```bash
   ~/.cargo/bin/cargo test -p in-mem-concurrency --lib snapshot
   ```

### Testing Requirements

**CRITICAL TESTING RULE**:
- **NEVER adjust tests to make them pass**
- If a test fails, the CODE must be fixed, not the test
- Tests define correct behavior - failed tests reveal bugs in implementation
- Only adjust a test if the test itself is incorrect (wrong assertion logic)

**Coverage target**: ≥95% for SnapshotView implementations

**Test checklist**:
- [ ] Empty snapshot has correct version and is empty
- [ ] Snapshot version() returns correct value
- [ ] get() returns existing key with correct value
- [ ] get() returns None for non-existent key
- [ ] scan_prefix() returns matching keys
- [ ] scan_prefix() returns empty for no matches
- [ ] Snapshot is independent of original data
- [ ] Multiple snapshots are independent of each other

### Validation
```bash
# Run concurrency tests
~/.cargo/bin/cargo test -p in-mem-concurrency

# Check build
~/.cargo/bin/cargo build --all

# Check clippy
~/.cargo/bin/cargo clippy -p in-mem-concurrency -- -D warnings

# Check formatting
~/.cargo/bin/cargo fmt --check
```

### Complete Story
```bash
./scripts/complete-story.sh 80
```

---

## Story #81: Transaction Read Operations

**Branch**: `story-81-transaction-read-ops`
**Estimated Time**: 4 hours
**Dependencies**: Stories #79 (TransactionContext), #80 (SnapshotView)

### Start Command
```bash
/opt/homebrew/bin/gh issue view 81
./scripts/start-story.sh 6 81 transaction-read-ops
```

### ⚠️ PREREQUISITE: Read the Semantics Spec

**Before writing ANY code**, read and understand:
- `docs/architecture/M2_TRANSACTION_SEMANTICS.md` (Sections 2, 3, 6)

This story implements **read-your-writes** and **read-set tracking**. Both are critical for OCC.

### Semantics This Story Must Implement

From `M2_TRANSACTION_SEMANTICS.md`:

1. **Read-Your-Writes** (Section 2.1, 2.4 Example 2):
   - Transaction sees its own uncommitted writes
   - Transaction sees its own uncommitted deletes (returns None)
   - Priority: write_set > delete_set > snapshot

2. **Read-Set Tracking** (Section 3.1):
   - Every read from snapshot MUST be tracked in read_set
   - Track: `{ key → version_read }`
   - Version 0 means "key did not exist"
   - Used for conflict detection at commit time

3. **What Reads Return**:
   - If key in write_set → return buffered value (NO read_set entry)
   - If key in delete_set → return None (NO read_set entry)
   - If key in snapshot → return value AND add to read_set
   - If key not in snapshot → return None AND add to read_set with version 0

4. **Visibility Rules** (Section 2):
   - NEVER see uncommitted writes from other transactions
   - NEVER see writes committed after start_version
   - ALWAYS see consistent snapshot

### Context
Read operations must check write_set first (read-your-writes), then read from snapshot. All reads tracked in read_set for conflict detection. This implements the core read path for OCC transactions.

### Implementation Steps

1. **Update `crates/concurrency/src/transaction.rs`**

   Add snapshot field to TransactionContext:
   ```rust
   use crate::snapshot::SnapshotView;

   pub struct TransactionContext {
       // ... existing fields ...

       /// Snapshot view for this transaction
       snapshot: Box<dyn SnapshotView>,
   }
   ```

   Add constructor that accepts snapshot:
   ```rust
   impl TransactionContext {
       /// Create a new transaction context with a snapshot
       pub fn with_snapshot(
           txn_id: u64,
           run_id: RunId,
           snapshot: Box<dyn SnapshotView>,
       ) -> Self {
           let start_version = snapshot.version();
           TransactionContext {
               txn_id,
               run_id,
               start_version,
               snapshot,
               read_set: HashMap::new(),
               write_set: HashMap::new(),
               delete_set: HashSet::new(),
               cas_set: Vec::new(),
               status: TransactionStatus::Active,
           }
       }
   }
   ```

   Implement read operations:
   ```rust
   impl TransactionContext {
       /// Get a value from the transaction
       ///
       /// Implements read-your-writes:
       /// 1. Check write_set (uncommitted writes from this txn)
       /// 2. Check delete_set (uncommitted deletes from this txn)
       /// 3. Read from snapshot
       /// 4. Track in read_set for validation
       pub fn get(&mut self, key: &Key) -> Result<Option<Value>> {
           self.ensure_active()?;

           // 1. Check write_set first (read-your-writes)
           if let Some(value) = self.write_set.get(key) {
               return Ok(Some(value.clone()));
           }

           // 2. Check delete_set (return None if deleted in this txn)
           if self.delete_set.contains(key) {
               return Ok(None);
           }

           // 3. Read from snapshot
           let versioned = self.snapshot.get(key)?;

           // 4. Track in read_set for conflict detection
           if let Some(ref vv) = versioned {
               self.read_set.insert(key.clone(), vv.version);
               Ok(Some(vv.value.clone()))
           } else {
               // Track read of non-existent key with version 0
               self.read_set.insert(key.clone(), 0);
               Ok(None)
           }
       }

       /// Scan keys with a prefix
       ///
       /// Returns all keys matching the prefix, implementing read-your-writes:
       /// - Includes uncommitted writes from this transaction
       /// - Excludes uncommitted deletes from this transaction
       /// - Tracks all scanned keys in read_set
       pub fn scan_prefix(&mut self, prefix: &Key) -> Result<Vec<(Key, Value)>> {
           self.ensure_active()?;

           // Get all matching keys from snapshot
           let snapshot_results = self.snapshot.scan_prefix(prefix)?;

           // Build result set with read-your-writes
           let mut results: BTreeMap<Key, Value> = BTreeMap::new();

           // Add snapshot results (excluding deleted keys)
           for (key, vv) in snapshot_results {
               if !self.delete_set.contains(&key) {
                   self.read_set.insert(key.clone(), vv.version);
                   results.insert(key, vv.value);
               }
           }

           // Add/overwrite with write_set entries matching prefix
           let prefix_bytes = prefix.to_bytes();
           for (key, value) in &self.write_set {
               if key.to_bytes().starts_with(&prefix_bytes) {
                   results.insert(key.clone(), value.clone());
               }
           }

           Ok(results.into_iter().collect())
       }

       /// Check if a key exists in the transaction's view
       pub fn exists(&mut self, key: &Key) -> Result<bool> {
           Ok(self.get(key)?.is_some())
       }
   }
   ```

2. **Add necessary imports**
   ```rust
   use std::collections::BTreeMap;
   ```

3. **Write comprehensive tests**
   ```rust
   #[cfg(test)]
   mod read_tests {
       use super::*;
       use crate::snapshot::ClonedSnapshotView;

       fn create_snapshot_with_data() -> Box<dyn SnapshotView> {
           let mut data = BTreeMap::new();
           // ... add test data ...
           Box::new(ClonedSnapshotView::new(100, data))
       }

       #[test]
       fn test_get_from_snapshot() {
           // Read key that exists in snapshot
       }

       #[test]
       fn test_get_nonexistent_key() {
           // Read key that doesn't exist
       }

       #[test]
       fn test_read_your_writes() {
           // Write then read same key - should see uncommitted write
       }

       #[test]
       fn test_read_deleted_key_returns_none() {
           // Delete then read same key - should return None
       }

       #[test]
       fn test_read_set_tracking() {
           // Verify reads are tracked in read_set
       }

       #[test]
       fn test_scan_prefix_from_snapshot() {
           // Scan keys matching prefix
       }

       #[test]
       fn test_scan_prefix_with_uncommitted_writes() {
           // Scan should include uncommitted writes
       }

       #[test]
       fn test_scan_prefix_excludes_deleted() {
           // Scan should exclude deleted keys
       }
   }
   ```

4. **Run tests**
   ```bash
   ~/.cargo/bin/cargo test -p in-mem-concurrency --lib transaction
   ```

### Testing Requirements

**CRITICAL TESTING RULE**:
- **NEVER adjust tests to make them pass**
- If a test fails, the CODE must be fixed, not the test
- Tests define correct behavior - failed tests reveal bugs in implementation
- Only adjust a test if the test itself is incorrect (wrong assertion logic)

**Coverage target**: ≥95% for read operations

**Test checklist**:
- [ ] get() reads from snapshot
- [ ] get() returns None for non-existent key
- [ ] get() returns uncommitted write (read-your-writes)
- [ ] get() returns None for deleted key
- [ ] get() tracks reads in read_set
- [ ] get() tracks non-existent key reads with version 0
- [ ] scan_prefix() returns matching keys from snapshot
- [ ] scan_prefix() includes uncommitted writes
- [ ] scan_prefix() excludes deleted keys
- [ ] scan_prefix() tracks all scanned keys
- [ ] exists() returns correct boolean

### Validation
```bash
# Run concurrency tests
~/.cargo/bin/cargo test -p in-mem-concurrency

# Check build
~/.cargo/bin/cargo build --all

# Check clippy
~/.cargo/bin/cargo clippy -p in-mem-concurrency -- -D warnings

# Check formatting
~/.cargo/bin/cargo fmt --check
```

### Complete Story
```bash
./scripts/complete-story.sh 81
```

---

## Story #82: Transaction Write Operations

**Branch**: `story-82-transaction-write-ops`
**Estimated Time**: 4 hours
**Dependencies**: Story #79 (TransactionContext)

### Start Command
```bash
/opt/homebrew/bin/gh issue view 82
./scripts/start-story.sh 6 82 transaction-write-ops
```

### ⚠️ PREREQUISITE: Read the Semantics Spec

**Before writing ANY code**, read and understand:
- `docs/architecture/M2_TRANSACTION_SEMANTICS.md` (Sections 2, 3, 5, 6)

This story implements **write buffering** and **CAS operations**. Critical for OCC isolation.

### Semantics This Story Must Implement

From `M2_TRANSACTION_SEMANTICS.md`:

1. **Write Buffering** (Section 2.2):
   - Writes go to write_set, NOT storage
   - Other transactions NEVER see buffered writes
   - Writes applied only at commit time

2. **Delete Buffering**:
   - Deletes go to delete_set
   - If key was in write_set, remove it
   - Delete + Put = key in write_set (not delete_set)

3. **CAS Semantics** (Section 3.4, 6.4):
   - `expected_version = 0` means "key must not exist"
   - `expected_version = N` means "key must be at version N"
   - CAS does NOT automatically add to read_set
   - CAS is validated at COMMIT time, not call time
   - CAS operations accumulate in cas_set

4. **Blind Writes** (Section 3.2):
   - Writing without reading is allowed
   - Blind writes do NOT conflict (first-committer-wins applies to reads)

5. **Isolation Guarantee**:
   - Buffered writes are INVISIBLE to other transactions
   - This is fundamental to OCC

### Context
Write operations buffer to write_set/delete_set/cas_set. NOT applied to storage until commit. This story implements the write path for OCC transactions - all operations are buffered and validated at commit time.

### Implementation Steps

1. **Update `crates/concurrency/src/transaction.rs`**

   Implement write operations:
   ```rust
   impl TransactionContext {
       /// Buffer a write operation
       ///
       /// The write is NOT applied to storage until commit.
       /// Other transactions will NOT see this write.
       /// If the key was previously deleted in this txn, remove from delete_set.
       pub fn put(&mut self, key: Key, value: Value) -> Result<()> {
           self.ensure_active()?;

           // Remove from delete_set if previously deleted in this txn
           self.delete_set.remove(&key);

           // Add to write_set
           self.write_set.insert(key, value);
           Ok(())
       }

       /// Buffer a delete operation
       ///
       /// The delete is NOT applied to storage until commit.
       /// If the key was previously written in this txn, remove from write_set.
       pub fn delete(&mut self, key: Key) -> Result<()> {
           self.ensure_active()?;

           // Remove from write_set if previously written in this txn
           self.write_set.remove(&key);

           // Add to delete_set
           self.delete_set.insert(key);
           Ok(())
       }

       /// Buffer a compare-and-swap operation
       ///
       /// CAS will be validated at commit time:
       /// - expected_version = 0 means "key must not exist"
       /// - expected_version = N means "key must be at version N"
       ///
       /// The new value is NOT applied until commit succeeds.
       pub fn cas(&mut self, key: Key, expected_version: u64, new_value: Value) -> Result<()> {
           self.ensure_active()?;

           self.cas_set.push(CASOperation {
               key,
               expected_version,
               new_value,
           });
           Ok(())
       }

       /// Clear all buffered operations (used for retry)
       pub fn clear_operations(&mut self) -> Result<()> {
           self.ensure_active()?;

           self.read_set.clear();
           self.write_set.clear();
           self.delete_set.clear();
           self.cas_set.clear();
           Ok(())
       }
   }
   ```

2. **Write comprehensive tests**
   ```rust
   #[cfg(test)]
   mod write_tests {
       use super::*;
       use crate::snapshot::ClonedSnapshotView;

       fn create_test_txn() -> TransactionContext {
           let snapshot = Box::new(ClonedSnapshotView::empty(100));
           TransactionContext::with_snapshot(1, RunId::new(), snapshot)
       }

       fn create_test_key(name: &[u8]) -> Key {
           let ns = Namespace::new("test", "app", "agent", RunId::new());
           Key::new_kv(ns, name)
       }

       #[test]
       fn test_put_adds_to_write_set() {
           let mut txn = create_test_txn();
           let key = create_test_key(b"key1");
           let value = Value::Bytes(b"value1".to_vec());

           txn.put(key.clone(), value).unwrap();

           assert_eq!(txn.write_count(), 1);
           assert!(txn.write_set.contains_key(&key));
       }

       #[test]
       fn test_put_overwrites_in_write_set() {
           let mut txn = create_test_txn();
           let key = create_test_key(b"key1");

           txn.put(key.clone(), Value::Bytes(b"v1".to_vec())).unwrap();
           txn.put(key.clone(), Value::Bytes(b"v2".to_vec())).unwrap();

           assert_eq!(txn.write_count(), 1);
           let stored = txn.write_set.get(&key).unwrap();
           match stored {
               Value::Bytes(data) => assert_eq!(data, b"v2"),
               _ => panic!("Expected Bytes"),
           }
       }

       #[test]
       fn test_put_removes_from_delete_set() {
           let mut txn = create_test_txn();
           let key = create_test_key(b"key1");

           txn.delete(key.clone()).unwrap();
           assert!(txn.delete_set.contains(&key));

           txn.put(key.clone(), Value::Bytes(b"v1".to_vec())).unwrap();

           assert!(!txn.delete_set.contains(&key));
           assert!(txn.write_set.contains_key(&key));
       }

       #[test]
       fn test_delete_adds_to_delete_set() {
           let mut txn = create_test_txn();
           let key = create_test_key(b"key1");

           txn.delete(key.clone()).unwrap();

           assert_eq!(txn.delete_count(), 1);
           assert!(txn.delete_set.contains(&key));
       }

       #[test]
       fn test_delete_removes_from_write_set() {
           let mut txn = create_test_txn();
           let key = create_test_key(b"key1");

           txn.put(key.clone(), Value::Bytes(b"v1".to_vec())).unwrap();
           assert!(txn.write_set.contains_key(&key));

           txn.delete(key.clone()).unwrap();

           assert!(!txn.write_set.contains_key(&key));
           assert!(txn.delete_set.contains(&key));
       }

       #[test]
       fn test_cas_adds_to_cas_set() {
           let mut txn = create_test_txn();
           let key = create_test_key(b"key1");
           let value = Value::Bytes(b"new_value".to_vec());

           txn.cas(key.clone(), 50, value).unwrap();

           assert_eq!(txn.cas_count(), 1);
           let cas_op = &txn.cas_set[0];
           assert_eq!(cas_op.key, key);
           assert_eq!(cas_op.expected_version, 50);
       }

       #[test]
       fn test_multiple_cas_operations() {
           let mut txn = create_test_txn();

           txn.cas(create_test_key(b"k1"), 10, Value::Bytes(b"v1".to_vec())).unwrap();
           txn.cas(create_test_key(b"k2"), 20, Value::Bytes(b"v2".to_vec())).unwrap();
           txn.cas(create_test_key(b"k3"), 0, Value::Bytes(b"v3".to_vec())).unwrap();

           assert_eq!(txn.cas_count(), 3);
       }

       #[test]
       fn test_operations_fail_when_not_active() {
           let mut txn = create_test_txn();
           txn.mark_validating().unwrap();

           let key = create_test_key(b"key");
           assert!(txn.put(key.clone(), Value::Bytes(b"v".to_vec())).is_err());
           assert!(txn.delete(key.clone()).is_err());
           assert!(txn.cas(key, 0, Value::Bytes(b"v".to_vec())).is_err());
       }

       #[test]
       fn test_has_pending_operations() {
           let mut txn = create_test_txn();
           assert!(!txn.has_pending_operations());

           txn.put(create_test_key(b"k"), Value::Bytes(b"v".to_vec())).unwrap();
           assert!(txn.has_pending_operations());
       }

       #[test]
       fn test_clear_operations() {
           let mut txn = create_test_txn();

           txn.put(create_test_key(b"k1"), Value::Bytes(b"v1".to_vec())).unwrap();
           txn.delete(create_test_key(b"k2")).unwrap();
           txn.cas(create_test_key(b"k3"), 0, Value::Bytes(b"v3".to_vec())).unwrap();

           assert!(txn.has_pending_operations());

           txn.clear_operations().unwrap();

           assert!(!txn.has_pending_operations());
           assert_eq!(txn.write_count(), 0);
           assert_eq!(txn.delete_count(), 0);
           assert_eq!(txn.cas_count(), 0);
       }
   }
   ```

3. **Run tests**
   ```bash
   ~/.cargo/bin/cargo test -p in-mem-concurrency --lib transaction
   ```

### Testing Requirements

**CRITICAL TESTING RULE**:
- **NEVER adjust tests to make them pass**
- If a test fails, the CODE must be fixed, not the test
- Tests define correct behavior - failed tests reveal bugs in implementation
- Only adjust a test if the test itself is incorrect (wrong assertion logic)

**Coverage target**: ≥95% for write operations

**Test checklist**:
- [ ] put() adds to write_set
- [ ] put() overwrites previous put (latest value wins)
- [ ] put() removes key from delete_set if deleted in this txn
- [ ] delete() adds to delete_set
- [ ] delete() removes key from write_set if written in this txn
- [ ] cas() adds to cas_set
- [ ] Multiple CAS operations accumulate
- [ ] Operations fail when transaction not active
- [ ] has_pending_operations() correctly reflects state
- [ ] clear_operations() clears all sets

### Validation
```bash
# Run concurrency tests
~/.cargo/bin/cargo test -p in-mem-concurrency

# Check build
~/.cargo/bin/cargo build --all

# Check clippy
~/.cargo/bin/cargo clippy -p in-mem-concurrency -- -D warnings

# Check formatting
~/.cargo/bin/cargo fmt --check
```

### Complete Story
```bash
./scripts/complete-story.sh 82
```

---

## Epic 6 Completion Checklist

Once ALL 5 stories are complete and merged to `epic-6-transaction-foundations`:

### 1. Final Validation
```bash
# All tests pass
~/.cargo/bin/cargo test --all

# Release build clean
~/.cargo/bin/cargo build --release --all

# No clippy warnings
~/.cargo/bin/cargo clippy --all -- -D warnings

# Formatting clean
~/.cargo/bin/cargo fmt --check
```

### 2. Epic Review
Run the comprehensive 5-phase review:

**Phase 1: Pre-Review Validation**
```bash
~/.cargo/bin/cargo build --all
~/.cargo/bin/cargo test --all
~/.cargo/bin/cargo clippy --all -- -D warnings
~/.cargo/bin/cargo fmt --all -- --check
```

**Phase 2: Unit Testing**
```bash
~/.cargo/bin/cargo test -p in-mem-concurrency -- --nocapture
~/.cargo/bin/cargo test --all --release
```

**Phase 3: Code Review**
- Check TDD integrity (no test modifications to hide bugs)
- Verify architecture adherence (snapshot isolation, OCC semantics)
- Check error handling
- Verify no unwrap() in production code

**Phase 4: Documentation Review**
```bash
~/.cargo/bin/cargo doc --all --no-deps
```

**Phase 5: Epic-Specific Validation**
- [ ] Transaction semantics documented and approved
- [ ] TransactionContext lifecycle works correctly
- [ ] SnapshotView trait abstraction in place
- [ ] ClonedSnapshotView provides proper isolation
- [ ] Read operations with read-your-writes working
- [ ] Write operations properly buffered
- [ ] All sets (read, write, delete, cas) tracked correctly

### 3. Create Epic Review Document
```bash
# Create EPIC_6_REVIEW.md
cat > docs/milestones/EPIC_6_REVIEW.md << 'EOF'
# Epic 6 Review: Transaction Foundations

**Status**: [APPROVED/NEEDS_WORK]
**Reviewed**: [DATE]
**Reviewer**: [NAME]

## Phase 1: Pre-Review Validation
[Results]

## Phase 2: Unit Testing
[Results]

## Phase 3: Code Review
[Results]

## Phase 4: Documentation Review
[Results]

## Phase 5: Epic-Specific Validation
[Results]

## Overall Assessment
[Summary]
EOF
```

### 4. Merge to Develop
```bash
# Switch to develop
git checkout develop
git pull origin develop

# Merge epic branch (no fast-forward)
git merge --no-ff epic-6-transaction-foundations -m "Epic 6: Transaction Foundations

Complete:
- Transaction semantics specification (M2_TRANSACTION_SEMANTICS.md)
- TransactionContext with lifecycle management
- SnapshotView trait & ClonedSnapshotView implementation
- Transaction read operations (read-your-writes, read-set tracking)
- Transaction write operations (buffered put/delete/cas)

Test Results:
- [X] tests passing
- Coverage: [Y]%
- All unit tests passing

This completes Epic 6 of M2: Transactions.
Ready for Epic 7: Transaction Semantics (conflict detection).
"

# Push to develop
git push origin develop

# Tag the release
git tag -a epic-6-complete -m "Epic 6: Transaction Foundations - Complete

M2 Progress: Epic 6 of 7 complete

Components:
- TransactionContext (lifecycle, read/write/delete/cas sets)
- SnapshotView trait abstraction
- ClonedSnapshotView (O(n) clone, acceptable for M2)
- Read operations with read-your-writes
- Write buffering (not visible until commit)

Statistics:
- [X] total tests
- [Y]% coverage

Next: Epic 7 - Transaction Semantics (conflict detection)
"

git push origin epic-6-complete
```

### 5. Close Epic Issue
```bash
/opt/homebrew/bin/gh issue close 71 --comment "Epic 6: Transaction Foundations - COMPLETE ✅

All 5 user stories completed:
- ✅ Story #78: Transaction Semantics Specification
- ✅ Story #79: TransactionContext Core
- ✅ Story #80: SnapshotView Trait & ClonedSnapshot
- ✅ Story #81: Transaction Read Operations
- ✅ Story #82: Transaction Write Operations

Test Results:
- [X] tests passing
- Coverage: [Y]%
- All unit tests passing

Key Deliverables:
- ✅ M2_TRANSACTION_SEMANTICS.md (defines OCC behavior)
- ✅ TransactionContext with full lifecycle
- ✅ SnapshotView abstraction (future-proof for LazySnapshotView)
- ✅ ClonedSnapshotView (O(n) acceptable for M2)
- ✅ Read-your-writes semantics
- ✅ Buffered write operations

Next: Epic 7 - Transaction Semantics (conflict detection)

Review document: docs/milestones/EPIC_6_REVIEW.md
"
```

### 6. Update Project Status
Update `docs/milestones/M2_PROJECT_STATUS.md`:
- Mark Epic 6 as complete
- Update progress: 1/7 epics complete, 5/32 stories complete
- Update "Next Steps" to point to Epic 7

---

## Critical Notes

### 🔴 SPEC COMPLIANCE IS MANDATORY

**Every line of M2 code must comply with `docs/architecture/M2_TRANSACTION_SEMANTICS.md`.**

During code review, verify:
- [ ] Isolation level is Snapshot Isolation (NOT Serializability)
- [ ] Visibility rules match spec exactly (ALWAYS/NEVER/MAY see)
- [ ] Conflict detection follows spec (read-set based, first-committer-wins)
- [ ] CAS does NOT auto-add to read_set
- [ ] Version 0 means "key never existed"
- [ ] Write skew is ALLOWED (do not try to prevent it)
- [ ] Phantom reads are ALLOWED (do not try to prevent them)
- [ ] Replay is single-threaded, no re-validation
- [ ] Implicit transactions wrap M1 ops atomically

**If ANY behavior deviates from the spec, the code MUST be rejected.**

### Architecture Principles
1. **Snapshot isolation** - Each transaction sees consistent view at start
2. **Read-your-writes** - Transaction sees its own uncommitted writes
3. **Buffered operations** - Writes not applied until commit
4. **Trait abstraction** - SnapshotView enables future LazySnapshotView

### Known Limitations (Acceptable for M2)
1. **ClonedSnapshotView**: O(data_size) memory and time
   - Acceptable for agent workloads (small datasets, short txns)
   - Future: LazySnapshotView will eliminate this overhead
2. **No commit/validation yet** - Epic 7 adds conflict detection
3. **No WAL integration yet** - Epic 8 adds durability

### Testing Philosophy
- Unit tests validate each component independently
- TDD approach: tests written before/during implementation
- Tests must not be adjusted to pass - code must be fixed

### M2 Progress After Epic 6
**Epic 6 delivers**:
- Core transaction infrastructure
- Snapshot isolation (ClonedSnapshotView)
- Read/write operation buffering
- Foundation for conflict detection (Epic 7)

**Still needed for M2**:
- Epic 7: Conflict detection and validation
- Epic 8: Durability & commit (WAL integration)
- Epic 9: Recovery support
- Epic 10: Database API integration
- Epic 11: Backwards compatibility
- Epic 12: OCC validation & benchmarking

---

## Summary

Epic 6 establishes the transaction foundation for M2:
- Transaction semantics documented in M2_TRANSACTION_SEMANTICS.md
- TransactionContext manages transaction state and operation tracking
- SnapshotView trait abstracts snapshot mechanism
- ClonedSnapshotView provides snapshot isolation (deep copy)
- Read operations implement read-your-writes semantics
- Write operations buffer changes until commit

**After Epic 6**: Transaction infrastructure is complete. Ready for Epic 7 (conflict detection and validation).
