# Epic 4: Basic Recovery - Claude Implementation Prompts

**Epic Branch**: `epic-4-basic-recovery`
**Stories**: #23, #24, #25, #26, #27
**Dependencies**: Epic 2 (Storage) ✅, Epic 3 (WAL) ✅
**Estimated Duration**: 3-4 days with parallelization

---

## Epic 4 Overview

Epic 4 implements **crash recovery** by replaying the Write-Ahead Log (WAL) to reconstruct storage state after database restart. This is the critical durability mechanism that ensures no committed data is lost.

### What Epic 4 Delivers

1. **WAL Replay Logic** - Scan WAL, group entries by transaction, apply committed transactions
2. **Incomplete Transaction Handling** - Validate WAL entries, discard incomplete transactions, log warnings
3. **Database::open() Integration** - Automatic recovery on database startup
4. **Crash Simulation Tests** - Verify recovery works after process kills at various points
5. **Performance Tests** - Ensure recovery meets targets (>2000 txns/sec, <5 seconds for 10K txns)

### Key Architecture

```
Database::open(path)
    ↓
1. Create UnifiedStore (empty)
2. Open WAL (scan entries)
3. Validate transactions (identify incomplete)
4. Group entries by txn_id
5. Apply committed transactions (preserve versions)
6. Discard incomplete transactions (log warnings)
7. Return ready Database
```

### Recovery Flow Details

**Critical**: Recovery must preserve original version numbers from WAL entries. Do NOT allocate new versions during replay.

**Transaction Validation**:
- Committed: BeginTxn → operations → CommitTxn
- Incomplete: BeginTxn → operations (no CommitTxn)
- Orphaned: Write/Delete without BeginTxn

**Discard Policy**: Conservative fail-safe approach
- Incomplete transactions are discarded entirely
- Log warnings for discarded data
- Only apply transactions with explicit CommitTxn

---

## Dependency Graph

```
Story #23: WAL replay logic (FOUNDATION)
     ↓
Story #24: Incomplete transaction handling
     ↓
Story #25: Database::open() integration
     ↓
     ├──→ Story #26: Crash simulation tests
     └──→ Story #27: Performance tests
```

### Parallelization Analysis

**Phase 1**: Story #23 (Sequential) - ~5-6 hours
- Foundation story, blocks all others
- Creates `recovery.rs` and `replay_wal()` function

**Phase 2**: Story #24 (Sequential) - ~4-5 hours
- Builds on #23's replay logic
- Adds validation and incomplete transaction handling

**Phase 3**: Story #25 (Sequential) - ~5-6 hours
- Creates engine crate and Database::open()
- Integrates #23 and #24

**Phase 4**: Stories #26 and #27 (PARALLEL) - ~5-6 hours wall time
- Both depend only on #25
- No file conflicts (separate test files)
- Can run 2 Claudes in parallel

**Total Sequential**: ~24-28 hours
**Total Parallel**: ~19-23 hours wall time
**Speedup**: ~20% reduction in Phase 4

**Maximum Parallelization**: 2 Claudes in Phase 4 only

---

## File Ownership

### Story #23 (Claude 1)
- `crates/durability/src/recovery.rs` (creates)
- `crates/durability/src/lib.rs` (adds `pub mod recovery;`)
- `crates/durability/tests/replay_test.rs` (creates)

### Story #24 (Claude 2)
- `crates/durability/src/recovery.rs` (updates with validation)
- `crates/durability/tests/incomplete_txn_test.rs` (creates)

### Story #25 (Claude 3)
- `crates/engine/Cargo.toml` (creates)
- `crates/engine/src/lib.rs` (creates)
- `crates/engine/src/database.rs` (creates)
- `crates/engine/tests/integration_test.rs` (creates)
- `Cargo.toml` (adds engine to workspace)

### Story #26 (Claude 4)
- `crates/engine/tests/crash_simulation_test.rs` (creates)

### Story #27 (Claude 5)
- `crates/engine/tests/recovery_performance_test.rs` (creates)

### Potential Conflicts

**Stories #26 and #27 (Phase 4)**:
- Both run in parallel
- Different test files (no conflicts)
- Both depend only on #25 being merged

---

# Story #23: WAL Replay Logic

**Branch**: `epic-4-story-23-wal-replay-logic`
**Estimated**: 5-6 hours
**Depends on**: Epic 3 ✅
**Blocks**: All other Epic 4 stories

## Prompt for Claude

You are implementing **Story #23: WAL Replay Logic** for the in-mem database project.

### Context

Read the GitHub issue for full requirements:

```bash
/opt/homebrew/bin/gh issue view 23
```

Read the architecture documents:

```bash
cat docs/milestones/M1_ARCHITECTURE.md | grep -A 50 "Recovery Protocol"
cat docs/milestones/MILESTONES.md | grep -A 30 "Epic 4"
```

### What You're Building

**File**: `crates/durability/src/recovery.rs`

Implement WAL replay logic that:
1. Scans WAL entries from a file
2. Groups entries by `txn_id`
3. Identifies committed vs incomplete transactions
4. Applies committed transactions to storage
5. Preserves original version numbers from WAL
6. Returns replay statistics

### Key Structures

```rust
pub struct ReplayStats {
    pub txns_applied: usize,
    pub writes_applied: usize,
    pub deletes_applied: usize,
    pub final_version: u64,
}

struct Transaction {
    txn_id: u64,
    run_id: RunId,
    entries: Vec<WALEntry>,
    committed: bool,
}
```

### Implementation Steps

**Step 1**: Create the recovery module

```bash
# Create recovery.rs
touch crates/durability/src/recovery.rs
```

Add to `crates/durability/src/lib.rs`:
```rust
pub mod recovery;
```

**Step 2**: Implement `replay_wal()`

```rust
pub fn replay_wal(
    wal: &WAL,
    storage: &UnifiedStore,
) -> Result<ReplayStats, DurabilityError>
```

Algorithm:
1. Create HashMap<u64, Transaction> to group by txn_id
2. Scan WAL entries
3. For each entry:
   - BeginTxn: Create new Transaction
   - Write/Delete: Add to transaction's entries
   - CommitTxn: Mark transaction as committed
   - AbortTxn: Mark as aborted
4. For each committed transaction:
   - Call `apply_transaction(storage, txn)`
5. Return stats

**Step 3**: Implement `apply_transaction()`

```rust
fn apply_transaction(
    storage: &UnifiedStore,
    txn: &Transaction,
) -> Result<(), DurabilityError>
```

For each entry in transaction:
- Write: `storage.put_with_version(key, value, version)` - **preserve version from WAL**
- Delete: `storage.delete_with_version(key, version)` - **preserve version from WAL**

**CRITICAL**: Do NOT allocate new versions. Use versions from WAL entries.

**Step 4**: Add version-preserving methods to UnifiedStore

Update `crates/storage/src/unified.rs`:

```rust
impl UnifiedStore {
    /// Put with specific version (for replay only)
    pub fn put_with_version(&self, key: Key, value: Value, version: u64) {
        let mut data = self.data.write().unwrap();
        data.insert(key, VersionedValue { value, version, .. });
        // Update global_version if needed
        self.global_version.fetch_max(version, Ordering::SeqCst);
    }

    /// Delete with specific version (for replay only)
    pub fn delete_with_version(&self, key: &Key, version: u64) -> Option<VersionedValue> {
        let mut data = self.data.write().unwrap();
        let old = data.remove(key);
        self.global_version.fetch_max(version, Ordering::SeqCst);
        old
    }
}
```

**Step 5**: Write tests

Create `crates/durability/tests/replay_test.rs`:

```rust
#[test]
fn test_replay_single_transaction() {
    // Write WAL: BeginTxn → Write → CommitTxn
    // Replay into empty storage
    // Verify key exists with correct value and version
}

#[test]
fn test_replay_multiple_transactions() {
    // Write 3 transactions
    // Replay
    // Verify all applied
}

#[test]
fn test_replay_preserves_versions() {
    // Write WAL with specific versions
    // Replay
    // Verify versions match WAL exactly (not re-allocated)
}
```

### Testing Requirements

**Unit tests** (in `recovery.rs`):
- `group_entries_by_txn()` helper function
- `apply_transaction()` with various entry types

**Integration tests** (in `tests/replay_test.rs`):
- Single transaction replay
- Multiple transactions replay
- Version preservation
- Mixed writes and deletes

**CRITICAL TESTING RULE**:
- **NEVER adjust tests to make them pass**
- If a test fails, the CODE must be fixed, not the test
- Tests define correct behavior - failed tests reveal bugs in implementation
- Only adjust a test if the test itself is incorrect (wrong assertion logic)

**Quality gate**: All tests pass
```bash
~/.cargo/bin/cargo test -p durability
```

### Completion Checklist

Before running `./scripts/complete-story.sh 23`:

- [ ] `crates/durability/src/recovery.rs` created with `replay_wal()` and `apply_transaction()`
- [ ] `crates/storage/src/unified.rs` updated with `put_with_version()` and `delete_with_version()`
- [ ] `crates/durability/tests/replay_test.rs` created with 3+ tests
- [ ] All tests pass: `~/.cargo/bin/cargo test -p durability`
- [ ] No clippy warnings: `~/.cargo/bin/cargo clippy -p durability -- -D warnings`
- [ ] Code formatted: `~/.cargo/bin/cargo fmt -p durability --check`
- [ ] ReplayStats includes txns_applied, writes_applied, deletes_applied, final_version
- [ ] Versions are preserved from WAL (not re-allocated)

### Complete Story

```bash
# Run all checks
~/.cargo/bin/cargo test -p durability
~/.cargo/bin/cargo test -p storage  # Verify storage changes work
~/.cargo/bin/cargo clippy -p durability -- -D warnings
~/.cargo/bin/cargo fmt --check

# Create PR
./scripts/complete-story.sh 23
```

---

# Story #24: Incomplete Transaction Handling

**Branch**: `epic-4-story-24-incomplete-txn-handling`
**Estimated**: 4-5 hours
**Depends on**: #23 ✅
**Blocks**: #25

## Prompt for Claude

You are implementing **Story #24: Incomplete Transaction Handling** for the in-mem database project.

### Context

Read the GitHub issue:

```bash
/opt/homebrew/bin/gh issue view 24
```

Understand Story #23 (dependency):

```bash
/opt/homebrew/bin/gh issue view 23
cat crates/durability/src/recovery.rs
```

### What You're Building

**File**: `crates/durability/src/recovery.rs` (update)

Add validation logic that:
1. Identifies incomplete transactions (no CommitTxn)
2. Identifies orphaned entries (no BeginTxn)
3. Logs warnings for discarded data
4. Updates ReplayStats with discarded counts

### Updated ReplayStats

```rust
pub struct ReplayStats {
    pub txns_applied: usize,
    pub writes_applied: usize,
    pub deletes_applied: usize,
    pub discarded_txns: usize,      // NEW
    pub orphaned_entries: usize,    // NEW
    pub final_version: u64,
}
```

### Implementation Steps

**Step 1**: Read current recovery.rs

```bash
cat crates/durability/src/recovery.rs
```

**Step 2**: Add `validate_transactions()` function

```rust
fn validate_transactions(
    transactions: &HashMap<u64, Transaction>,
) -> ValidationResult {
    let mut incomplete_txns = Vec::new();
    let mut orphaned_entries = Vec::new();

    for (txn_id, txn) in transactions {
        if !txn.committed && !txn.aborted {
            incomplete_txns.push(*txn_id);
        }
    }

    // Check for orphaned entries (entries without BeginTxn)
    // ...

    ValidationResult {
        incomplete_txns,
        orphaned_entries,
    }
}

struct ValidationResult {
    incomplete_txns: Vec<u64>,
    orphaned_entries: Vec<WALEntry>,
}
```

**Step 3**: Update `replay_wal()` to call validation

```rust
pub fn replay_wal(
    wal: &WAL,
    storage: &UnifiedStore,
) -> Result<ReplayStats, DurabilityError> {
    // ... existing grouping logic ...

    // VALIDATE before applying
    let validation = validate_transactions(&transactions);

    // Log warnings
    for txn_id in &validation.incomplete_txns {
        warn!("Discarding incomplete transaction: {}", txn_id);
    }
    for entry in &validation.orphaned_entries {
        warn!("Discarding orphaned entry: {:?}", entry);
    }

    // Apply only committed transactions
    let mut stats = ReplayStats::default();
    for (txn_id, txn) in transactions {
        if txn.committed {
            apply_transaction(storage, &txn)?;
            stats.txns_applied += 1;
            stats.writes_applied += count_writes(&txn);
            stats.deletes_applied += count_deletes(&txn);
        }
    }

    stats.discarded_txns = validation.incomplete_txns.len();
    stats.orphaned_entries = validation.orphaned_entries.len();
    stats.final_version = storage.current_version();

    Ok(stats)
}
```

**Step 4**: Add logging dependency

Update `crates/durability/Cargo.toml`:

```toml
[dependencies]
# ... existing ...
log = "0.4"
```

**Step 5**: Write tests

Create `crates/durability/tests/incomplete_txn_test.rs`:

```rust
#[test]
fn test_discard_incomplete_transaction() {
    // Write WAL: BeginTxn → Write (no CommitTxn)
    // Replay
    // Verify: discarded_txns = 1, key does NOT exist in storage
}

#[test]
fn test_discard_orphaned_entries() {
    // Write WAL: Write without BeginTxn
    // Replay
    // Verify: orphaned_entries = 1, key does NOT exist
}

#[test]
fn test_mixed_committed_and_incomplete() {
    // Write WAL:
    //   Txn 1: BeginTxn → Write → CommitTxn (committed)
    //   Txn 2: BeginTxn → Write (incomplete)
    // Replay
    // Verify: txns_applied = 1, discarded_txns = 1
    // Verify: Txn 1 key exists, Txn 2 key does NOT exist
}

#[test]
fn test_aborted_transactions_discarded() {
    // Write WAL: BeginTxn → Write → AbortTxn
    // Replay
    // Verify: discarded_txns = 1
}
```

### Testing Requirements

**Unit tests**:
- `validate_transactions()` with various scenarios

**Integration tests** (in `tests/incomplete_txn_test.rs`):
- Incomplete transaction discarded
- Orphaned entries discarded
- Mixed committed and incomplete
- Aborted transactions discarded
- Warning logs generated

**CRITICAL TESTING RULE**:
- **NEVER adjust tests to make them pass**
- If a test fails, the CODE must be fixed, not the test
- Tests define correct behavior - failed tests reveal bugs in implementation
- Only adjust a test if the test itself is incorrect (wrong assertion logic)

**Quality gate**: All tests pass
```bash
~/.cargo/bin/cargo test -p durability
```

### Completion Checklist

Before running `./scripts/complete-story.sh 24`:

- [ ] `crates/durability/src/recovery.rs` updated with `validate_transactions()`
- [ ] `replay_wal()` calls validation and logs warnings
- [ ] ReplayStats includes discarded_txns and orphaned_entries
- [ ] `crates/durability/tests/incomplete_txn_test.rs` created with 4+ tests
- [ ] All tests pass: `~/.cargo/bin/cargo test -p durability`
- [ ] No clippy warnings: `~/.cargo/bin/cargo clippy -p durability -- -D warnings`
- [ ] Code formatted: `~/.cargo/bin/cargo fmt -p durability --check`
- [ ] Warning logs generated for discarded data (verify with `RUST_LOG=warn`)

### Complete Story

```bash
# Run all checks
~/.cargo/bin/cargo test -p durability
~/.cargo/bin/cargo clippy -p durability -- -D warnings
~/.cargo/bin/cargo fmt --check

# Create PR
./scripts/complete-story.sh 24
```

---

# Story #25: Database::open() Integration

**Branch**: `epic-4-story-25-database-open-integration`
**Estimated**: 5-6 hours
**Depends on**: #23 ✅, #24 ✅
**Blocks**: #26, #27

## Prompt for Claude

You are implementing **Story #25: Database::open() Integration** for the in-mem database project.

### Context

Read the GitHub issue:

```bash
/opt/homebrew/bin/gh issue view 25
```

Read dependencies:

```bash
cat crates/durability/src/recovery.rs
cat docs/milestones/M1_ARCHITECTURE.md | grep -A 30 "Database Engine"
```

### What You're Building

**New Crate**: `crates/engine/`

Create the main database engine that orchestrates storage, WAL, and recovery.

**Main API**: `Database::open()` - triggers automatic recovery on startup.

### Implementation Steps

**Step 1**: Create engine crate

```bash
mkdir -p crates/engine/src crates/engine/tests
touch crates/engine/Cargo.toml
touch crates/engine/src/lib.rs
touch crates/engine/src/database.rs
```

**Step 2**: Configure engine crate

`crates/engine/Cargo.toml`:

```toml
[package]
name = "engine"
version = "0.1.0"
edition = "2021"

[dependencies]
core = { path = "../core" }
storage = { path = "../storage" }
durability = { path = "../durability" }
thiserror = "1.0"
log = "0.4"

[dev-dependencies]
tempfile = "3.8"
env_logger = "0.11"
```

**Step 3**: Add engine to workspace

Update root `Cargo.toml`:

```toml
[workspace]
members = [
    "crates/core",
    "crates/storage",
    "crates/concurrency",
    "crates/durability",
    "crates/engine",  # ADD THIS
]
```

**Step 4**: Implement `crates/engine/src/lib.rs`

```rust
pub mod database;

pub use database::Database;
pub use durability::DurabilityMode;

// Re-export core types for convenience
pub use core::{Key, Namespace, RunId, TypeTag, Value};
pub use storage::UnifiedStore;
```

**Step 5**: Implement `crates/engine/src/database.rs`

```rust
use core::*;
use durability::{DurabilityMode, WAL};
use storage::UnifiedStore;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct Database {
    data_dir: PathBuf,
    storage: Arc<UnifiedStore>,
    wal: Arc<Mutex<WAL>>,
}

impl Database {
    /// Open database with default durability mode (batched)
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, EngineError> {
        Self::open_with_mode(path, DurabilityMode::Batched)
    }

    /// Open database with specific durability mode
    pub fn open_with_mode<P: AsRef<Path>>(
        path: P,
        durability_mode: DurabilityMode,
    ) -> Result<Self, EngineError> {
        let data_dir = path.as_ref().to_path_buf();

        // Create data directory if needed
        std::fs::create_dir_all(&data_dir)?;

        // 1. Create empty storage
        let storage = Arc::new(UnifiedStore::new());

        // 2. Open WAL
        let wal_path = data_dir.join("wal.log");
        let wal = WAL::open(&wal_path, durability_mode)?;

        // 3. Replay WAL if it exists
        if wal_path.exists() && std::fs::metadata(&wal_path)?.len() > 0 {
            log::info!("Replaying WAL from {:?}", wal_path);
            let stats = durability::recovery::replay_wal(&wal, &storage)?;
            log::info!("Recovery complete: {:?}", stats);

            if stats.discarded_txns > 0 {
                log::warn!(
                    "Discarded {} incomplete transactions during recovery",
                    stats.discarded_txns
                );
            }
        }

        Ok(Database {
            data_dir,
            storage,
            wal: Arc::new(Mutex::new(wal)),
        })
    }

    /// Get reference to storage (for primitives to use)
    pub fn storage(&self) -> &UnifiedStore {
        &self.storage
    }

    /// Flush WAL to disk (force fsync)
    pub fn flush(&self) -> Result<(), EngineError> {
        self.wal.lock().unwrap().flush()?;
        Ok(())
    }

    /// Close database gracefully
    pub fn close(self) -> Result<(), EngineError> {
        self.flush()?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Storage error: {0}")]
    Storage(#[from] storage::StorageError),

    #[error("Durability error: {0}")]
    Durability(#[from] durability::DurabilityError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

**Step 6**: Write integration test

Create `crates/engine/tests/integration_test.rs`:

```rust
use engine::*;
use tempfile::TempDir;

#[test]
fn test_database_open_creates_directory() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("testdb");

    let db = Database::open(&db_path).unwrap();
    assert!(db_path.exists());

    db.close().unwrap();
}

#[test]
fn test_recovery_after_restart() {
    env_logger::init();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("testdb");

    // Write some data
    {
        let db = Database::open(&db_path).unwrap();
        let storage = db.storage();

        let key = Key::new(
            Namespace::new("tenant", "app", "agent", RunId::new()),
            TypeTag::KV,
            b"test_key".to_vec(),
        );
        let value = Value::Bytes(b"test_value".to_vec());

        storage.put(key.clone(), value.clone(), None);

        // Write to WAL
        let mut wal = db.wal.lock().unwrap();
        wal.append(WALEntry::BeginTxn { ... })?;
        wal.append(WALEntry::Write { key: key.clone(), value, version: 1, ... })?;
        wal.append(WALEntry::CommitTxn { ... })?;

        db.close().unwrap();
    }

    // Reopen and verify data recovered
    {
        let db = Database::open(&db_path).unwrap();
        let storage = db.storage();

        let key = Key::new(...);  // Same key
        let recovered = storage.get(&key).unwrap();
        assert_eq!(recovered.value, Value::Bytes(b"test_value".to_vec()));
    }
}

#[test]
fn test_incomplete_transaction_discarded() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("testdb");

    // Write incomplete transaction (no CommitTxn)
    {
        let db = Database::open(&db_path).unwrap();
        let mut wal = db.wal.lock().unwrap();
        wal.append(WALEntry::BeginTxn { ... })?;
        wal.append(WALEntry::Write { ... })?;
        // Crash here (no CommitTxn)
        drop(wal);
    }

    // Reopen and verify data NOT recovered
    {
        let db = Database::open(&db_path).unwrap();
        let storage = db.storage();

        let key = Key::new(...);
        assert!(storage.get(&key).is_none());
    }
}
```

### Testing Requirements

**Integration tests** (in `tests/integration_test.rs`):
- Database::open() creates directory
- Recovery after restart (write, close, reopen, verify)
- Incomplete transaction discarded
- Multiple transactions recovered
- Empty WAL handled

**CRITICAL TESTING RULE**:
- **NEVER adjust tests to make them pass**
- If a test fails, the CODE must be fixed, not the test
- Tests define correct behavior - failed tests reveal bugs in implementation
- Only adjust a test if the test itself is incorrect (wrong assertion logic)

**Quality gate**: All tests pass
```bash
~/.cargo/bin/cargo test -p engine
```

### Completion Checklist

Before running `./scripts/complete-story.sh 25`:

- [ ] `crates/engine/` crate created
- [ ] `crates/engine/Cargo.toml` configured with dependencies
- [ ] `crates/engine/src/lib.rs` created with exports
- [ ] `crates/engine/src/database.rs` created with Database::open()
- [ ] Root `Cargo.toml` updated with engine in workspace
- [ ] `crates/engine/tests/integration_test.rs` created with 3+ tests
- [ ] All tests pass: `~/.cargo/bin/cargo test -p engine`
- [ ] All tests pass: `~/.cargo/bin/cargo test --all`
- [ ] No clippy warnings: `~/.cargo/bin/cargo clippy -p engine -- -D warnings`
- [ ] Code formatted: `~/.cargo/bin/cargo fmt -p engine --check`
- [ ] Database::open() triggers automatic recovery
- [ ] Recovery logs stats and warnings

### Complete Story

```bash
# Run all checks
~/.cargo/bin/cargo test -p engine
~/.cargo/bin/cargo test --all  # Verify all crates still work
~/.cargo/bin/cargo clippy -p engine -- -D warnings
~/.cargo/bin/cargo fmt --check

# Create PR
./scripts/complete-story.sh 25
```

---

# Story #26: Crash Simulation Tests

**Branch**: `epic-4-story-26-crash-simulation-tests`
**Estimated**: 5-6 hours
**Depends on**: #25 ✅
**Can run in PARALLEL with #27**

## Prompt for Claude

You are implementing **Story #26: Crash Simulation Tests** for the in-mem database project.

### Context

Read the GitHub issue:

```bash
/opt/homebrew/bin/gh issue view 26
```

Read Database::open() implementation:

```bash
cat crates/engine/src/database.rs
```

### What You're Building

**File**: `crates/engine/tests/crash_simulation_test.rs`

Tests that verify recovery works correctly after simulated crashes at various points in transaction lifecycle.

**Approach**: Use real process spawn (not just dropping objects) for realistic crash simulation.

### Implementation Steps

**Step 1**: Create test file

```bash
touch crates/engine/tests/crash_simulation_test.rs
```

**Step 2**: Implement crash simulation helper

```rust
use engine::*;
use std::process::{Command, Stdio};
use std::sync::Arc;
use tempfile::TempDir;

/// Spawn a child process that writes to WAL and crashes at a specific point
fn spawn_and_crash(
    db_path: &Path,
    crash_point: CrashPoint,
) -> std::io::Result<()> {
    let exe = std::env::current_exe()?;

    let output = Command::new(exe)
        .arg("--crash-test")
        .arg(db_path.to_str().unwrap())
        .arg(crash_point.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?
        .wait()?;

    // Crash is expected (non-zero exit or killed)
    Ok(())
}

enum CrashPoint {
    AfterBeginTxn,
    AfterWrite,
    BeforeCommit,
    AfterCommit,
}
```

**Step 3**: Write crash scenarios

```rust
#[test]
fn test_crash_after_begin_txn() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("crashdb");

    // Simulate crash: BeginTxn written, then crash
    // (manually write WAL with just BeginTxn)
    {
        let db = Database::open(&db_path).unwrap();
        let mut wal = db.wal.lock().unwrap();
        wal.append(WALEntry::BeginTxn {
            txn_id: 1,
            run_id: RunId::new(),
            timestamp: Timestamp::now(),
        }).unwrap();
        // Drop without CommitTxn (simulates crash)
    }

    // Reopen and verify no data
    {
        let db = Database::open(&db_path).unwrap();
        let storage = db.storage();
        // Verify storage is empty (incomplete txn discarded)
        assert_eq!(storage.current_version(), 0);
    }
}

#[test]
fn test_crash_after_commit_strict_mode() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("crashdb");

    let key = create_test_key();
    let value = Value::Bytes(b"committed_value".to_vec());

    // Write with strict mode (fsync after commit)
    {
        let db = Database::open_with_mode(&db_path, DurabilityMode::Strict).unwrap();
        let storage = db.storage();
        storage.put(key.clone(), value.clone(), None);

        let mut wal = db.wal.lock().unwrap();
        wal.append(WALEntry::BeginTxn { ... }).unwrap();
        wal.append(WALEntry::Write {
            run_id: RunId::new(),
            key: key.clone(),
            value: value.clone(),
            version: 1,
        }).unwrap();
        wal.append(WALEntry::CommitTxn { ... }).unwrap();
        // Strict mode forces fsync, so crash after this is safe
    }

    // Reopen and verify data is there
    {
        let db = Database::open(&db_path).unwrap();
        let storage = db.storage();
        let recovered = storage.get(&key).unwrap();
        assert_eq!(recovered.value, value);
    }
}

#[test]
fn test_crash_batched_mode_may_lose_recent_writes() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("crashdb");

    // Write with batched mode (no immediate fsync)
    {
        let db = Database::open_with_mode(&db_path, DurabilityMode::Batched).unwrap();
        let storage = db.storage();

        let key = create_test_key();
        let value = Value::Bytes(b"recent_write".to_vec());
        storage.put(key.clone(), value.clone(), None);

        let mut wal = db.wal.lock().unwrap();
        wal.append(WALEntry::BeginTxn { ... }).unwrap();
        wal.append(WALEntry::Write { ... }).unwrap();
        wal.append(WALEntry::CommitTxn { ... }).unwrap();
        // Batched mode: fsync happens later
        // Crash here (don't call flush) might lose data
    }

    // Reopen: data might not be there (acceptable for batched mode)
    {
        let db = Database::open(&db_path).unwrap();
        // This test documents behavior, not requirements
        // In batched mode, crash before fsync may lose recent writes
    }
}

#[test]
fn test_multiple_incomplete_transactions() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("crashdb");

    // Write 3 incomplete transactions
    {
        let db = Database::open(&db_path).unwrap();
        let mut wal = db.wal.lock().unwrap();

        for i in 1..=3 {
            wal.append(WALEntry::BeginTxn { txn_id: i, ... }).unwrap();
            wal.append(WALEntry::Write { ... }).unwrap();
            // No CommitTxn
        }
    }

    // Reopen and verify all discarded
    {
        let db = Database::open(&db_path).unwrap();
        let storage = db.storage();
        assert_eq!(storage.current_version(), 0);
    }
}

#[test]
fn test_mixed_committed_and_incomplete() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("crashdb");

    let key1 = create_test_key_with_suffix(b"key1");
    let key2 = create_test_key_with_suffix(b"key2");
    let value = Value::Bytes(b"value".to_vec());

    // Write: Txn 1 committed, Txn 2 incomplete
    {
        let db = Database::open(&db_path).unwrap();
        let mut wal = db.wal.lock().unwrap();

        // Txn 1: committed
        wal.append(WALEntry::BeginTxn { txn_id: 1, ... }).unwrap();
        wal.append(WALEntry::Write {
            key: key1.clone(),
            value: value.clone(),
            version: 1,
            ...
        }).unwrap();
        wal.append(WALEntry::CommitTxn { txn_id: 1, ... }).unwrap();

        // Txn 2: incomplete
        wal.append(WALEntry::BeginTxn { txn_id: 2, ... }).unwrap();
        wal.append(WALEntry::Write {
            key: key2.clone(),
            value: value.clone(),
            version: 2,
            ...
        }).unwrap();
        // No CommitTxn for txn 2
    }

    // Reopen and verify only Txn 1 data exists
    {
        let db = Database::open(&db_path).unwrap();
        let storage = db.storage();

        // Txn 1 data should exist
        assert!(storage.get(&key1).is_some());

        // Txn 2 data should NOT exist
        assert!(storage.get(&key2).is_none());
    }
}

fn create_test_key() -> Key {
    Key::new(
        Namespace::new("tenant", "app", "agent", RunId::new()),
        TypeTag::KV,
        b"test_key".to_vec(),
    )
}

fn create_test_key_with_suffix(suffix: &[u8]) -> Key {
    Key::new(
        Namespace::new("tenant", "app", "agent", RunId::new()),
        TypeTag::KV,
        suffix.to_vec(),
    )
}
```

### Testing Requirements

**Crash simulation tests** (in `tests/crash_simulation_test.rs`):
- Crash after BeginTxn (incomplete transaction discarded)
- Crash after CommitTxn with strict mode (data recovered)
- Crash with batched mode (document behavior)
- Multiple incomplete transactions (all discarded)
- Mixed committed and incomplete (only committed recovered)

**CRITICAL TESTING RULE**:
- **NEVER adjust tests to make them pass**
- If a test fails, there is a BUG in the recovery implementation
- Tests define correct crash recovery behavior - failed tests reveal bugs
- Log ALL test failures as bugs to be fixed
- Only adjust a test if the test itself is incorrect (wrong assertion logic)

**Quality gate**: All tests pass
```bash
~/.cargo/bin/cargo test -p engine crash_simulation
```

### Completion Checklist

Before running `./scripts/complete-story.sh 26`:

- [ ] `crates/engine/tests/crash_simulation_test.rs` created
- [ ] 5+ crash simulation tests implemented
- [ ] Tests cover: incomplete txn, committed txn, batched mode, multiple incomplete, mixed
- [ ] All tests pass: `~/.cargo/bin/cargo test -p engine`
- [ ] No clippy warnings: `~/.cargo/bin/cargo clippy -p engine --tests -- -D warnings`
- [ ] Code formatted: `~/.cargo/bin/cargo fmt --check`

### Complete Story

```bash
# Run all checks
~/.cargo/bin/cargo test -p engine
~/.cargo/bin/cargo clippy -p engine --tests -- -D warnings
~/.cargo/bin/cargo fmt --check

# Create PR
./scripts/complete-story.sh 26
```

---

# Story #27: Large WAL Recovery Performance Tests

**Branch**: `epic-4-story-27-recovery-performance-tests`
**Estimated**: 5-6 hours
**Depends on**: #25 ✅
**Can run in PARALLEL with #26**

## Prompt for Claude

You are implementing **Story #27: Large WAL Recovery Performance Tests** for the in-mem database project.

### Context

Read the GitHub issue:

```bash
/opt/homebrew/bin/gh issue view 27
```

Read Database::open() and recovery implementation:

```bash
cat crates/engine/src/database.rs
cat crates/durability/src/recovery.rs
```

### What You're Building

**File**: `crates/engine/tests/recovery_performance_test.rs`

Performance tests that verify recovery meets targets:
- >2000 transactions/second replay throughput
- <5 seconds to recover 10K transactions
- <100MB memory overhead

### Implementation Steps

**Step 1**: Create test file

```bash
touch crates/engine/tests/recovery_performance_test.rs
```

**Step 2**: Implement performance tests

```rust
use engine::*;
use std::time::Instant;
use tempfile::TempDir;

#[test]
fn test_10k_transactions_in_5_seconds() {
    env_logger::init();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("perfdb");

    let num_txns = 10_000;

    // Write 10K transactions
    {
        let db = Database::open(&db_path).unwrap();
        let storage = db.storage();
        let mut wal = db.wal.lock().unwrap();

        for i in 0..num_txns {
            let key = create_test_key_with_id(i);
            let value = Value::Bytes(format!("value_{}", i).into_bytes());

            storage.put(key.clone(), value.clone(), None);

            wal.append(WALEntry::BeginTxn {
                txn_id: i as u64,
                run_id: RunId::new(),
                timestamp: Timestamp::now(),
            }).unwrap();

            wal.append(WALEntry::Write {
                run_id: RunId::new(),
                key,
                value,
                version: i as u64 + 1,
            }).unwrap();

            wal.append(WALEntry::CommitTxn {
                txn_id: i as u64,
                run_id: RunId::new(),
            }).unwrap();
        }

        db.close().unwrap();
    }

    // Measure recovery time
    let start = Instant::now();
    {
        let db = Database::open(&db_path).unwrap();
        let storage = db.storage();

        // Verify data
        assert_eq!(storage.current_version(), num_txns as u64);
    }
    let elapsed = start.elapsed();

    println!("Recovered {} transactions in {:?}", num_txns, elapsed);
    println!("Throughput: {:.0} txns/sec", num_txns as f64 / elapsed.as_secs_f64());

    // Performance targets
    assert!(elapsed.as_secs() < 5, "Recovery took {:?}, expected <5s", elapsed);

    let throughput = num_txns as f64 / elapsed.as_secs_f64();
    assert!(throughput > 2000.0, "Throughput {:.0} txns/sec, expected >2000", throughput);
}

#[test]
fn test_1k_incomplete_transactions_discarded_efficiently() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("perfdb");

    let num_incomplete = 1_000;

    // Write 1K incomplete transactions
    {
        let db = Database::open(&db_path).unwrap();
        let mut wal = db.wal.lock().unwrap();

        for i in 0..num_incomplete {
            wal.append(WALEntry::BeginTxn {
                txn_id: i as u64,
                ...
            }).unwrap();

            wal.append(WALEntry::Write { ... }).unwrap();
            // No CommitTxn
        }
    }

    // Measure recovery time (should be fast even with many incomplete)
    let start = Instant::now();
    {
        let db = Database::open(&db_path).unwrap();
        let storage = db.storage();
        assert_eq!(storage.current_version(), 0);  // All discarded
    }
    let elapsed = start.elapsed();

    println!("Discarded {} incomplete txns in {:?}", num_incomplete, elapsed);
    assert!(elapsed.as_secs() < 2, "Recovery took {:?}, expected <2s", elapsed);
}

#[test]
fn test_large_values_performance() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("perfdb");

    let num_txns = 1_000;
    let value_size = 10_000;  // 10KB values

    // Write 1K transactions with large values
    {
        let db = Database::open(&db_path).unwrap();
        let storage = db.storage();
        let mut wal = db.wal.lock().unwrap();

        for i in 0..num_txns {
            let key = create_test_key_with_id(i);
            let value = Value::Bytes(vec![b'x'; value_size]);

            storage.put(key.clone(), value.clone(), None);

            wal.append(WALEntry::BeginTxn { ... }).unwrap();
            wal.append(WALEntry::Write {
                key,
                value,
                version: i as u64 + 1,
                ...
            }).unwrap();
            wal.append(WALEntry::CommitTxn { ... }).unwrap();
        }

        db.close().unwrap();
    }

    // Measure recovery
    let start = Instant::now();
    {
        let db = Database::open(&db_path).unwrap();
        let storage = db.storage();
        assert_eq!(storage.current_version(), num_txns as u64);
    }
    let elapsed = start.elapsed();

    println!("Recovered {} txns ({}KB each) in {:?}", num_txns, value_size / 1024, elapsed);
    assert!(elapsed.as_secs() < 3, "Recovery took {:?}, expected <3s", elapsed);
}

#[test]
fn test_mixed_workload_performance() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("perfdb");

    let num_txns = 5_000;

    // Mixed: puts, deletes, multiple writes per txn
    {
        let db = Database::open(&db_path).unwrap();
        let storage = db.storage();
        let mut wal = db.wal.lock().unwrap();

        for i in 0..num_txns {
            wal.append(WALEntry::BeginTxn {
                txn_id: i as u64,
                ...
            }).unwrap();

            // 3 writes per transaction
            for j in 0..3 {
                let key = create_test_key_with_id(i * 3 + j);
                let value = Value::Bytes(format!("value_{}_{}", i, j).into_bytes());

                storage.put(key.clone(), value.clone(), None);

                wal.append(WALEntry::Write {
                    key,
                    value,
                    version: (i * 3 + j) as u64 + 1,
                    ...
                }).unwrap();
            }

            // Delete one key
            if i > 0 {
                let delete_key = create_test_key_with_id(i - 1);
                storage.delete(&delete_key);
                wal.append(WALEntry::Delete {
                    run_id: RunId::new(),
                    key: delete_key,
                    version: (i * 3 + 3) as u64,
                }).unwrap();
            }

            wal.append(WALEntry::CommitTxn {
                txn_id: i as u64,
                ...
            }).unwrap();
        }

        db.close().unwrap();
    }

    // Measure recovery
    let start = Instant::now();
    {
        let _db = Database::open(&db_path).unwrap();
    }
    let elapsed = start.elapsed();

    println!("Recovered {} mixed txns in {:?}", num_txns, elapsed);
    assert!(elapsed.as_secs() < 5, "Recovery took {:?}, expected <5s", elapsed);
}

#[test]
fn test_wal_file_size_reasonable() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("perfdb");

    let num_txns = 10_000;

    {
        let db = Database::open(&db_path).unwrap();
        let storage = db.storage();
        let mut wal = db.wal.lock().unwrap();

        for i in 0..num_txns {
            let key = create_test_key_with_id(i);
            let value = Value::Bytes(b"value".to_vec());

            storage.put(key.clone(), value.clone(), None);

            wal.append(WALEntry::BeginTxn { ... }).unwrap();
            wal.append(WALEntry::Write { ... }).unwrap();
            wal.append(WALEntry::CommitTxn { ... }).unwrap();
        }

        db.close().unwrap();
    }

    // Check WAL file size
    let wal_path = db_path.join("wal.log");
    let wal_size = std::fs::metadata(&wal_path).unwrap().len();

    println!("WAL size for {} txns: {} MB", num_txns, wal_size / 1_000_000);

    // Rough check: should be <100MB for 10K small transactions
    assert!(wal_size < 100_000_000, "WAL size {} bytes, expected <100MB", wal_size);
}

#[test]
#[ignore]  // Run with --ignored for benchmarking
fn bench_recovery_100k_transactions() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("benchdb");

    let num_txns = 100_000;

    // Write 100K transactions
    println!("Writing {} transactions...", num_txns);
    let write_start = Instant::now();
    {
        let db = Database::open(&db_path).unwrap();
        let storage = db.storage();
        let mut wal = db.wal.lock().unwrap();

        for i in 0..num_txns {
            let key = create_test_key_with_id(i);
            let value = Value::Bytes(format!("value_{}", i).into_bytes());

            storage.put(key.clone(), value.clone(), None);

            wal.append(WALEntry::BeginTxn { ... }).unwrap();
            wal.append(WALEntry::Write { ... }).unwrap();
            wal.append(WALEntry::CommitTxn { ... }).unwrap();

            if i % 10_000 == 0 {
                println!("  Wrote {} txns", i);
            }
        }

        db.close().unwrap();
    }
    println!("Write complete in {:?}", write_start.elapsed());

    // Benchmark recovery
    println!("Recovering {} transactions...", num_txns);
    let recovery_start = Instant::now();
    {
        let db = Database::open(&db_path).unwrap();
        let storage = db.storage();
        assert_eq!(storage.current_version(), num_txns as u64);
    }
    let recovery_elapsed = recovery_start.elapsed();

    println!("Recovery complete in {:?}", recovery_elapsed);
    println!("Throughput: {:.0} txns/sec", num_txns as f64 / recovery_elapsed.as_secs_f64());
}

fn create_test_key_with_id(id: usize) -> Key {
    Key::new(
        Namespace::new("tenant", "app", "agent", RunId::new()),
        TypeTag::KV,
        format!("key_{}", id).into_bytes(),
    )
}
```

### Testing Requirements

**Performance tests** (in `tests/recovery_performance_test.rs`):
- 10K transactions in <5 seconds
- 1K incomplete transactions discarded efficiently
- Large values (10KB) handled
- Mixed workload (puts, deletes, multiple writes per txn)
- WAL file size reasonable (<100MB for 10K txns)
- Benchmark with 100K transactions (--ignored)

**CRITICAL TESTING RULE**:
- **NEVER adjust tests to make them pass**
- If a performance test fails, there is a PERFORMANCE BUG in the implementation
- Tests define performance targets - failed tests reveal performance issues
- Log ALL performance test failures with actual vs expected metrics
- DO NOT relax performance targets to make tests pass
- Only adjust a test if the target itself is incorrect (documented design change)

**Quality gate**: All tests pass (except ignored benchmark)
```bash
~/.cargo/bin/cargo test -p engine recovery_performance
```

**Optional benchmark**:
```bash
~/.cargo/bin/cargo test -p engine bench_recovery_100k_transactions -- --ignored --nocapture
```

### Completion Checklist

Before running `./scripts/complete-story.sh 27`:

- [ ] `crates/engine/tests/recovery_performance_test.rs` created
- [ ] 6+ performance tests implemented
- [ ] Tests meet targets: >2000 txns/sec, <5 seconds for 10K txns
- [ ] All tests pass: `~/.cargo/bin/cargo test -p engine recovery_performance`
- [ ] No clippy warnings: `~/.cargo/bin/cargo clippy -p engine --tests -- -D warnings`
- [ ] Code formatted: `~/.cargo/bin/cargo fmt --check`
- [ ] Optional benchmark runs successfully (100K txns)

### Complete Story

```bash
# Run all checks
~/.cargo/bin/cargo test -p engine
~/.cargo/bin/cargo clippy -p engine --tests -- -D warnings
~/.cargo/bin/cargo fmt --check

# Optional: run benchmark
~/.cargo/bin/cargo test -p engine bench_recovery_100k_transactions -- --ignored --nocapture

# Create PR
./scripts/complete-story.sh 27
```

---

## Epic 4 Completion Checklist

Before merging `epic-4-basic-recovery` to `develop`:

### All Stories Complete
- [ ] Story #23: WAL replay logic ✅
- [ ] Story #24: Incomplete transaction handling ✅
- [ ] Story #25: Database::open() integration ✅
- [ ] Story #26: Crash simulation tests ✅
- [ ] Story #27: Performance tests ✅

### Quality Gates
- [ ] All tests pass: `~/.cargo/bin/cargo test --all`
- [ ] No clippy warnings: `~/.cargo/bin/cargo clippy --all -- -D warnings`
- [ ] Code formatted: `~/.cargo/bin/cargo fmt --check`
- [ ] Test coverage ≥90% for durability and engine crates

### Recovery Validation
- [ ] Database::open() triggers automatic recovery
- [ ] Incomplete transactions are discarded
- [ ] Committed transactions are fully recovered
- [ ] Versions are preserved from WAL (not re-allocated)
- [ ] Recovery logs stats and warnings
- [ ] Crash simulation tests pass (all scenarios)
- [ ] Performance targets met (>2000 txns/sec, <5 seconds for 10K txns)

### Integration
- [ ] Engine crate added to workspace
- [ ] Engine exports clean API (Database, DurabilityMode)
- [ ] Storage updated with version-preserving methods
- [ ] Recovery module fully integrated

### Documentation
- [ ] Epic 4 review process completed (EPIC_4_REVIEW.md)
- [ ] PROJECT_STATUS.md updated
- [ ] All 5 stories closed on GitHub

### Epic Merge Commands

```bash
# Switch to develop
git checkout develop
git pull origin develop

# Merge epic branch
git merge --no-ff epic-4-basic-recovery

# Run final validation
~/.cargo/bin/cargo test --all
~/.cargo/bin/cargo clippy --all -- -D warnings
~/.cargo/bin/cargo build --release

# Push to develop
git push origin develop

# Tag release
git tag -a epic-4-complete -m "Epic 4: Basic Recovery complete"
git push origin epic-4-complete

# Close epic issue
/opt/homebrew/bin/gh issue close 4 --comment "Epic 4 complete and merged to develop. All recovery functionality implemented and tested."
```

---

## Summary

Epic 4 implements the **critical durability mechanism**: crash recovery via WAL replay.

**Key deliverables**:
1. `crates/durability/src/recovery.rs` - WAL replay and validation
2. `crates/engine/` - Main database with Database::open()
3. Crash simulation tests - Verify recovery correctness
4. Performance tests - Verify recovery efficiency

**Parallelization**: Limited to Phase 4 (stories #26 and #27 in parallel)

**Next**: Epic 5 will build the remaining engine features (run tracking, primitives integration, KV facade).
