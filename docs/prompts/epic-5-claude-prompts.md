# Epic 5: Database Engine Shell - Implementation Prompts

**Epic Goal**: Create basic Database struct that orchestrates storage and WAL (transactions come in M2).

**Status**: Ready to begin
**Dependencies**: Epics #1-4 (all complete)

---

## Epic 5 Overview

### Scope
- Database struct with UnifiedStore and WAL
- Database::open() with recovery (already implemented in Epic 4, will enhance)
- Basic run tracking (begin_run, end_run)
- Simple put/get operations (no transactions yet)
- KV primitive facade (basic version)

### Success Criteria
- [x] Database::open() succeeds with recovery (from Epic 4)
- [ ] Can begin/end runs
- [ ] Basic put/get works
- [ ] WAL is appended on writes
- [ ] Integration test: write, restart, read

### Component Breakdown
- **Story #28**: Database struct (update existing from Epic 4)
- **Story #29**: Run tracking (RunTracker + begin_run/end_run)
- **Story #30**: Basic put/get operations (non-transactional)
- **Story #31**: KV primitive facade
- **Story #32**: Integration tests

---

## Dependency Graph

```
Phase 1 (Sequential):
  Story #28 (Database struct)
    └─> BLOCKS all other stories

Phase 2 (Parallel - 2 Claudes):
  Story #29 (Run tracking)
  Story #30 (Basic put/get)
    └─> Both depend on #28
    └─> Independent of each other

Phase 3 (Sequential):
  Story #31 (KV primitive)
    └─> Depends on #28, #29, #30

Phase 4 (Sequential):
  Story #32 (Integration test)
    └─> Depends on ALL previous stories
```

---

## Parallelization Strategy

### Phase 1: Foundation (Sequential) - ~3-4 hours
- **Story #28**: Database struct enhancements
  - Updates existing Database from Epic 4
  - Adds thread-safety guarantees
  - **BLOCKS** all other Epic 5 stories

### Phase 2: Core Operations (2 Claudes in PARALLEL) - ~4-5 hours wall time
Once #28 merges to `epic-5-database-engine-shell`:

| Story | Component | Claude | Dependencies | Estimated | Conflicts |
|-------|-----------|--------|--------------|-----------|-----------|
| #29 | Run tracking | Available | #28 | 4-5 hours | Creates run.rs (new file) |
| #30 | Basic put/get | Available | #28 | 4-5 hours | Updates database.rs (different sections) |

**Why parallel**:
- #29 creates new file `run.rs` (no conflicts)
- #30 updates `database.rs` with put/get methods
- #28 updates `database.rs` with struct enhancements
- Different concerns, minimal merge conflicts

### ⚠️  CRITICAL: Core Crate Coordination

Stories #29 and #30 both modify `core` crate:
- **#29 adds**: `RunMetadataEntry`, `Value::RunMetadata`, `TypeTag::RunMetadata`, `Key::new_run_metadata()`
- **#30 adds**: `Namespace::new()`, `Key::new_kv()`
- **Files affected**: Both modify `core/src/types.rs` and `core/src/value.rs`

**If running in parallel**:
1. Coordinate who modifies core first
2. Second story must pull latest before starting core changes
3. Resolve merge conflicts carefully during PR to `epic-5-database-engine-shell`

**Alternative**: Run #29 → #30 sequentially to avoid conflicts (adds ~4 hours to wall time)

### Phase 3: Primitive Layer (Sequential) - ~3-4 hours
After #29 and #30 merge:

- **Story #31**: KV primitive facade
  - Creates new `primitives` crate
  - Depends on Database having put/get (#30)
  - Depends on run tracking (#29)

### Phase 4: Validation (Sequential) - ~4-5 hours
After #31 merges:

- **Story #32**: Integration tests
  - Tests all components together
  - **FINAL VALIDATION** for M1 MVP

**Epic 5 Total**: ~18-22 hours sequential, ~13-16 hours wall time with 2 Claudes

---

## Story #28: Database struct

**Branch**: `story-28-database-struct`
**Estimated Time**: 3-4 hours
**Dependencies**: Epic 4 complete (Database exists)

### Start Command
```bash
/opt/homebrew/bin/gh issue view 28
./scripts/start-story.sh 5 28 database-struct
```

### Context
The Database struct was created in Epic 4 with basic functionality. This story enhances it with:
- Additional public API methods (storage(), flush(), drop handler)
- Thread-safety verification
- Support for different durability modes
- Better documentation

**Files from Epic 4 to update**:
- `crates/engine/src/database.rs` - Existing Database struct

### Implementation Steps

1. **Read Epic 4 Database implementation**
   ```bash
   cat crates/engine/src/database.rs
   ```

2. **Enhance Database struct** (update `crates/engine/src/database.rs`)
   - Add `storage()` method to return `&UnifiedStore`
   - Add `data_dir()` method
   - Add `durability_mode()` method
   - Add `flush()` method for explicit WAL fsync
   - Add `wal()` internal method for engine use
   - Implement `Drop` trait to flush WAL on shutdown

3. **Add thread-safety fields**
   ```rust
   pub struct Database {
       data_dir: PathBuf,
       storage: Arc<UnifiedStore>,           // Already Arc
       wal: Arc<std::sync::Mutex<WAL>>,      // Already Mutex
       durability_mode: DurabilityMode,
   }
   ```

4. **Write comprehensive tests** (`crates/engine/src/database.rs` mod tests)
   - `test_database_creation` - Verify directories created
   - `test_database_storage_access` - storage() returns valid ref
   - `test_database_flush` - flush() works
   - `test_database_thread_safe` - Arc/Mutex thread-safety
   - `test_database_drop_flushes` - Drop flushes WAL
   - `test_database_with_different_modes` - All durability modes
   - `test_database_reopen_same_path` - Multiple open/close cycles

5. **Run tests**
   ```bash
   ~/.cargo/bin/cargo test -p in-mem-engine --lib database
   ```

6. **Update documentation**
   - Add rustdoc to all public methods
   - Document thread-safety guarantees
   - Document Drop behavior

### Testing Requirements

**CRITICAL TESTING RULE**:
- **NEVER adjust tests to make them pass**
- If a test fails, the CODE must be fixed, not the test
- Tests define correct behavior - failed tests reveal bugs in implementation
- Only adjust a test if the test itself is incorrect (wrong assertion logic)

**Coverage target**: ≥90% for Database struct

**Test checklist**:
- [ ] Database creation succeeds
- [ ] storage() returns valid reference
- [ ] flush() forces WAL fsync
- [ ] Drop handler flushes on shutdown
- [ ] Thread-safe from multiple threads
- [ ] Different durability modes work
- [ ] Reopen same path succeeds multiple times

### Validation
```bash
# Run all engine tests
~/.cargo/bin/cargo test -p in-mem-engine

# Check build
~/.cargo/bin/cargo build --all

# Check clippy
~/.cargo/bin/cargo clippy -p in-mem-engine -- -D warnings

# Check formatting
~/.cargo/bin/cargo fmt --check
```

### Complete Story
```bash
./scripts/complete-story.sh 28
```

---

## Story #29: Add run tracking (begin_run, end_run)

**Branch**: `story-29-run-tracking`
**Estimated Time**: 4-5 hours
**Dependencies**: Story #28 (Database struct)

### Start Command
```bash
/opt/homebrew/bin/gh issue view 29
./scripts/start-story.sh 5 29 run-tracking
```

### Context
Runs are the fundamental unit of organization. This story implements:
- RunTracker struct for in-memory active run tracking
- begin_run() to register runs and create metadata
- end_run() to mark completion and update metadata
- Run metadata storage in UnifiedStore (survives restart)

### Implementation Steps

1. **Create RunTracker** (new file: `crates/engine/src/run.rs`)
   ```rust
   use core::{
       error::Result,
       types::{Key, Namespace, RunId},
       value::{RunMetadataEntry, Timestamp, Value},
   };
   use std::collections::HashMap;
   use std::sync::RwLock;

   pub struct RunTracker {
       active_runs: RwLock<HashMap<RunId, RunMetadataEntry>>,
   }

   impl RunTracker {
       pub fn new() -> Self { ... }
       pub fn begin_run(&self, metadata: RunMetadataEntry) -> Result<()> { ... }
       pub fn end_run(&self, run_id: RunId) -> Result<Option<RunMetadataEntry>> { ... }
       pub fn get_active(&self, run_id: RunId) -> Option<RunMetadataEntry> { ... }
       pub fn list_active(&self) -> Vec<RunId> { ... }
       pub fn is_active(&self, run_id: RunId) -> bool { ... }
   }
   ```

2. **Add RunMetadataEntry to core/value.rs** (if not exists)
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct RunMetadataEntry {
       pub run_id: RunId,
       pub parent_run_id: Option<RunId>,
       pub status: String,
       pub created_at: Timestamp,
       pub completed_at: Option<Timestamp>,
       pub first_version: u64,
       pub last_version: u64,
       pub tags: Vec<(String, String)>,
   }
   ```

3. **Add helper to core/value.rs**
   ```rust
   pub fn now() -> Timestamp {
       use std::time::{SystemTime, UNIX_EPOCH};
       SystemTime::now()
           .duration_since(UNIX_EPOCH)
           .unwrap()
           .as_millis() as u64
   }
   ```

4. **Add RunMetadata to Value enum** (`core/src/value.rs`)
   ```rust
   pub enum Value {
       Bytes(Vec<u8>),
       EventEntry(EventEntry),
       StateMachineEntry(StateMachineEntry),
       TraceEntry(TraceEntry),
       RunMetadata(RunMetadataEntry),  // NEW
   }
   ```

5. **Add RunMetadata to TypeTag** (`core/src/types.rs`)
   ```rust
   pub enum TypeTag {
       KV,
       Event,
       StateMachine,
       Trace,
       RunMetadata,  // NEW
       Vector,
   }
   ```

6. **Add Key helper** (`core/src/types.rs`)
   ```rust
   impl Key {
       pub fn new_run_metadata(ns: Namespace, run_id: RunId) -> Self {
           Key {
               namespace: ns,
               type_tag: TypeTag::RunMetadata,
               user_key: run_id.as_bytes().to_vec(),
           }
       }
   }
   ```

7. **Update Database struct** (`crates/engine/src/database.rs`)
   - Add `run_tracker: Arc<RunTracker>` field
   - Add `begin_run(run_id, tags)` method
   - Add `end_run(run_id)` method
   - Add `get_run(run_id)` method
   - Add `list_active_runs()` method
   - Add `is_run_active(run_id)` method

8. **Update engine/src/lib.rs**
   ```rust
   pub mod database;
   pub mod run;

   pub use database::Database;
   pub use run::RunTracker;
   ```

9. **Write RunTracker tests** (`crates/engine/src/run.rs` mod tests)
   - `test_begin_run` - Register run
   - `test_end_run` - End run removes from active
   - `test_list_active_runs` - Multiple active runs

10. **Write Database run tests** (`crates/engine/src/database.rs` mod tests)
    - `test_run_lifecycle` - begin → get → end → get
    - `test_multiple_active_runs` - Multiple concurrent runs
    - `test_run_metadata_persistence` - Restart test

### Testing Requirements

**CRITICAL TESTING RULE**:
- **NEVER adjust tests to make them pass**
- If a test fails, the CODE must be fixed, not the test
- Tests define correct behavior - failed tests reveal bugs in implementation
- Only adjust a test if the test itself is incorrect (wrong assertion logic)

**Coverage target**: ≥90% for run tracking code

**Test checklist**:
- [ ] begin_run() creates metadata and marks active
- [ ] end_run() updates metadata and removes from active
- [ ] get_run() retrieves from active or storage
- [ ] list_active_runs() returns all active
- [ ] Run metadata persists across restart
- [ ] Multiple concurrent runs tracked correctly

### Validation
```bash
# Run engine tests
~/.cargo/bin/cargo test -p in-mem-engine

# Run core tests (for new types)
~/.cargo/bin/cargo test -p in-mem-core

# Check build
~/.cargo/bin/cargo build --all

# Check clippy
~/.cargo/bin/cargo clippy --all -- -D warnings
```

### Complete Story
```bash
./scripts/complete-story.sh 29
```

---

## Story #30: Implement basic put/get (non-transactional)

**Branch**: `story-30-basic-put-get`
**Estimated Time**: 4-5 hours
**Dependencies**: Story #28 (Database struct)

### Start Command
```bash
/opt/homebrew/bin/gh issue view 30
./scripts/start-story.sh 5 30 basic-put-get
```

### Context
Simple key-value operations without transactions. Each operation is wrapped in an implicit single-operation transaction:
- BeginTxn(auto_id) → Write/Delete → CommitTxn(auto_id)
- Operations are atomic (storage + WAL updated together)
- Thread-safe via existing Storage and WAL locking

### Implementation Steps

1. **Update Database struct** (`crates/engine/src/database.rs`)
   - Add `next_txn_id: AtomicU64` field
   - Add `next_txn_id() -> u64` private method

2. **Implement put operations**
   ```rust
   pub fn put(
       &self,
       run_id: RunId,
       key: impl AsRef<[u8]>,
       value: impl Into<Value>,
   ) -> Result<u64>

   pub fn put_with_ttl(
       &self,
       run_id: RunId,
       key: impl AsRef<[u8]>,
       value: impl Into<Value>,
       ttl: Option<Duration>,
   ) -> Result<u64>
   ```

   **Critical atomicity pattern**:
   ```rust
   let mut wal = self.wal.lock().unwrap();  // Lock FIRST
   wal.append(BeginTxn)?;
   let version = self.storage.put(...)?;    // Storage write
   wal.append(Write)?;                      // WAL write
   wal.append(CommitTxn)?;
   drop(wal);                               // Release lock
   ```

3. **Implement get operation**
   ```rust
   pub fn get(
       &self,
       run_id: RunId,
       key: impl AsRef<[u8]>,
   ) -> Result<Option<Value>>
   ```

4. **Implement delete operation**
   ```rust
   pub fn delete(
       &self,
       run_id: RunId,
       key: impl AsRef<[u8]>,
   ) -> Result<Option<Value>>
   ```

5. **Implement list operation**
   ```rust
   pub fn list(
       &self,
       run_id: RunId,
       prefix: impl AsRef<[u8]>,
   ) -> Result<Vec<(Vec<u8>, Value)>>
   ```

6. **Add Key helper** (`core/src/types.rs`)
   ```rust
   impl Key {
       pub fn new_kv(ns: Namespace, user_key: &[u8]) -> Self {
           Key {
               namespace: ns,
               type_tag: TypeTag::KV,
               user_key: user_key.to_vec(),
           }
       }
   }
   ```

7. **Add Namespace helper** (`core/src/types.rs`)
   ```rust
   impl Namespace {
       pub fn new(tenant: &str, app: &str, agent: &str, run_id: RunId) -> Self {
           Namespace {
               tenant: tenant.to_string(),
               app: app.to_string(),
               agent: agent.to_string(),
               run: run_id,
           }
       }
   }
   ```

8. **Write comprehensive tests** (`crates/engine/src/database.rs` mod tests)
   - `test_put_and_get` - Basic put/get
   - `test_get_nonexistent` - Get missing key
   - `test_delete` - Delete key
   - `test_put_with_ttl` - TTL expiration
   - `test_list_with_prefix` - Prefix scan
   - `test_persistence` - Write, restart, read
   - `test_multiple_puts_same_key` - Updates
   - `test_concurrent_operations` - Thread-safety

### Testing Requirements

**CRITICAL TESTING RULE**:
- **NEVER adjust tests to make them pass**
- If a test fails, the CODE must be fixed, not the test
- Tests define correct behavior - failed tests reveal bugs in implementation
- Only adjust a test if the test itself is incorrect (wrong assertion logic)

**Coverage target**: ≥90% for put/get/delete/list code

**Test checklist**:
- [ ] put() writes value and returns version
- [ ] get() retrieves correct value
- [ ] delete() removes key and returns old value
- [ ] put_with_ttl() expires after TTL
- [ ] list() returns keys with matching prefix
- [ ] Multiple puts update value
- [ ] Operations persist across restart
- [ ] Concurrent operations work correctly

### Validation
```bash
# Run engine tests
~/.cargo/bin/cargo test -p in-mem-engine

# Run with release mode
~/.cargo/bin/cargo test -p in-mem-engine --release

# Check build
~/.cargo/bin/cargo build --all

# Check clippy
~/.cargo/bin/cargo clippy -p in-mem-engine -- -D warnings
```

### Complete Story
```bash
./scripts/complete-story.sh 30
```

---

## Story #31: Create KV primitive facade (basic version)

**Branch**: `story-31-kv-primitive`
**Estimated Time**: 3-4 hours
**Dependencies**: Stories #28, #29, #30

### Start Command
```bash
/opt/homebrew/bin/gh issue view 31
./scripts/start-story.sh 5 31 kv-primitive
```

### Context
The KV primitive is a **stateless facade** over the Database engine. It establishes the pattern for all future primitives:
- No state (just Arc<Database> reference)
- Domain-specific API
- Delegates to Database methods
- Cheap to clone (Arc clone)

### Implementation Steps

1. **Create primitives crate**
   ```bash
   mkdir -p crates/primitives/src
   ```

2. **Create Cargo.toml** (`crates/primitives/Cargo.toml`)
   ```toml
   [package]
   name = "primitives"
   version.workspace = true
   edition.workspace = true
   rust-version.workspace = true
   authors.workspace = true
   license.workspace = true
   repository.workspace = true

   [dependencies]
   core = { path = "../core" }
   engine = { path = "../engine" }

   [dev-dependencies]
   tempfile = { workspace = true }
   ```

3. **Update workspace Cargo.toml**
   ```toml
   [workspace]
   members = [
       "crates/core",
       "crates/storage",
       "crates/concurrency",
       "crates/durability",
       "crates/engine",
       "crates/primitives",  # NEW
   ]
   ```

4. **Create lib.rs** (`crates/primitives/src/lib.rs`)
   ```rust
   pub mod kv;
   // Future primitives:
   // pub mod event_log;
   // pub mod state_machine;
   // pub mod trace;
   // pub mod run_index;

   pub use kv::KVStore;
   ```

5. **Implement KVStore** (`crates/primitives/src/kv.rs`)
   ```rust
   use core::{error::Result, types::RunId, value::Value};
   use engine::Database;
   use std::sync::Arc;
   use std::time::Duration;

   #[derive(Clone)]
   pub struct KVStore {
       db: Arc<Database>,
   }

   impl KVStore {
       pub fn new(db: Arc<Database>) -> Self { ... }
       pub fn get(&self, run_id: RunId, key: impl AsRef<[u8]>) -> Result<Option<Vec<u8>>> { ... }
       pub fn put(&self, run_id: RunId, key: impl AsRef<[u8]>, value: impl Into<Vec<u8>>) -> Result<u64> { ... }
       pub fn put_with_ttl(&self, run_id: RunId, key: impl AsRef<[u8]>, value: impl Into<Vec<u8>>, ttl: Duration) -> Result<u64> { ... }
       pub fn delete(&self, run_id: RunId, key: impl AsRef<[u8]>) -> Result<Option<Vec<u8>>> { ... }
       pub fn list(&self, run_id: RunId, prefix: impl AsRef<[u8]>) -> Result<Vec<(Vec<u8>, Vec<u8>)>> { ... }
       pub fn exists(&self, run_id: RunId, key: impl AsRef<[u8]>) -> Result<bool> { ... }
   }
   ```

6. **Write comprehensive tests** (`crates/primitives/src/kv.rs` mod tests)
   - `test_kv_put_and_get` - Basic operations
   - `test_kv_get_nonexistent` - Missing keys
   - `test_kv_delete` - Delete operation
   - `test_kv_put_with_ttl` - TTL expiration
   - `test_kv_list` - Prefix scanning
   - `test_kv_exists` - Existence check
   - `test_kv_update` - Multiple puts
   - `test_kv_is_stateless` - Multiple KV instances share data
   - `test_kv_clone` - Clone is cheap
   - `test_kv_different_runs` - Run isolation

7. **Add helper function for tests**
   ```rust
   fn setup_kv() -> (TempDir, KVStore, RunId) {
       let temp_dir = TempDir::new().unwrap();
       let db = Arc::new(Database::open(temp_dir.path()).unwrap());
       let kv = KVStore::new(db.clone());
       let run_id = RunId::new();
       db.begin_run(run_id, vec![]).unwrap();
       (temp_dir, kv, run_id)
   }
   ```

### Testing Requirements

**CRITICAL TESTING RULE**:
- **NEVER adjust tests to make them pass**
- If a test fails, the CODE must be fixed, not the test
- Tests define correct behavior - failed tests reveal bugs in implementation
- Only adjust a test if the test itself is incorrect (wrong assertion logic)

**Coverage target**: ≥90% for KVStore

**Test checklist**:
- [ ] KVStore delegates to Database correctly
- [ ] get/put/delete/list operations work
- [ ] put_with_ttl expires correctly
- [ ] exists() checks key presence
- [ ] KVStore is stateless (multiple instances share data)
- [ ] Clone is cheap (Arc clone)
- [ ] Different runs are isolated

### Validation
```bash
# Run primitives tests
~/.cargo/bin/cargo test -p primitives

# Run all tests
~/.cargo/bin/cargo test --all

# Check build
~/.cargo/bin/cargo build --all

# Check clippy
~/.cargo/bin/cargo clippy -p primitives -- -D warnings
```

### Complete Story
```bash
./scripts/complete-story.sh 31
```

---

## Story #32: Write integration test (write, restart, read)

**Branch**: `story-32-integration-test`
**Estimated Time**: 4-5 hours
**Dependencies**: ALL previous stories (#28-31)

### Start Command
```bash
/opt/homebrew/bin/gh issue view 32
./scripts/start-story.sh 5 32 integration-test
```

### Context
The **final M1 MVP validation test**. This integration test proves all M1 components work together:
- Database initialization and recovery
- Run lifecycle (begin_run, end_run)
- WAL logging and replay
- Storage operations
- KV primitive facade

**Success = M1 Foundation Complete**

### Implementation Steps

1. **Create integration test directory**
   ```bash
   mkdir -p crates/engine/tests
   ```

2. **Create integration_test.rs** (`crates/engine/tests/integration_test.rs`)

3. **Implement tests** (see full test suite in issue #32):
   - `test_end_to_end_write_restart_read` - Basic end-to-end
   - `test_multiple_runs_isolation_across_restart` - Run isolation
   - `test_large_dataset_survives_restart` - 1000 keys
   - `test_ttl_across_restart` - TTL behavior
   - `test_run_metadata_completeness` - Metadata persistence
   - `test_list_operations_across_restart` - List/scan operations
   - `test_m1_complete_workflow` - **Full M1 validation**

4. **Add comprehensive assertions**
   - Data survives restart
   - Run metadata persists
   - Isolation maintained
   - TTL works correctly
   - All operations functional

5. **Add helpful debug output**
   ```rust
   println!("=== M1 Integration Test: Complete Workflow ===");
   println!("Phase 1: Writing data...");
   println!("  Wrote 3 KV pairs, ended run");
   println!("Phase 2: Simulating restart...");
   println!("Phase 3: Recovering...");
   println!("  Recovery complete");
   println!("  All data verified");
   println!("=== M1 Integration Test: PASSED ===");
   ```

### Testing Requirements

**CRITICAL TESTING RULE**:
- **NEVER adjust tests to make them pass**
- If a test fails, the CODE must be fixed, not the test
- Tests define correct behavior - failed tests reveal bugs in implementation
- Only adjust a test if the test itself is incorrect (wrong assertion logic)

**This is the M1 validation suite** - if these tests pass, M1 is complete.

**Test checklist**:
- [ ] End-to-end write → restart → read works
- [ ] Multiple runs maintain isolation across restart
- [ ] Large datasets (1000 keys) survive restart
- [ ] TTL expiration works correctly across restart
- [ ] Run metadata fully persists and restores
- [ ] List operations work after recovery
- [ ] Complete M1 workflow test passes

### Validation
```bash
# Run integration tests
~/.cargo/bin/cargo test -p in-mem-engine --test integration_test

# Run all tests
~/.cargo/bin/cargo test --all

# Run in release mode
~/.cargo/bin/cargo test --all --release

# Check build
~/.cargo/bin/cargo build --release --all

# Final clippy check
~/.cargo/bin/cargo clippy --all -- -D warnings

# Final format check
~/.cargo/bin/cargo fmt --check
```

### Complete Story
```bash
./scripts/complete-story.sh 32
```

---

## Epic 5 Completion Checklist

Once ALL 5 stories are complete and merged to `epic-5-database-engine-shell`:

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

**Phase 2: Integration Testing**
```bash
~/.cargo/bin/cargo test --all --release
~/.cargo/bin/cargo test -p in-mem-engine --test integration_test -- --nocapture
```

**Phase 3: Code Review**
- Check TDD integrity (no test modifications to hide bugs)
- Verify architecture adherence (primitives stateless, layer boundaries clean)
- Check error handling
- Verify no unwrap() in production code

**Phase 4: Documentation Review**
```bash
~/.cargo/bin/cargo doc --all --no-deps
```

**Phase 5: Epic-Specific Validation**
- [ ] Database::open() triggers recovery correctly
- [ ] Run tracking works (begin/end/get)
- [ ] Basic put/get operations work
- [ ] WAL appended on writes
- [ ] Integration test passes (write, restart, read)
- [ ] KV primitive properly delegates to Database
- [ ] All M1 components integrate correctly

### 3. Create Epic Review Document
```bash
# Create EPIC_5_REVIEW.md
cat > docs/milestones/EPIC_5_REVIEW.md << 'EOF'
# Epic 5 Review: Database Engine Shell

**Status**: [APPROVED/NEEDS_WORK]
**Reviewed**: [DATE]
**Reviewer**: [NAME]

## Phase 1: Pre-Review Validation
[Results]

## Phase 2: Integration Testing
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
git merge --no-ff epic-5-database-engine-shell -m "Epic 5: Database Engine Shell

Complete:
- Database struct enhancements (thread-safety, public API)
- Run tracking (begin_run, end_run, metadata persistence)
- Basic put/get operations (non-transactional, WAL-logged)
- KV primitive facade (stateless pattern established)
- Integration tests (M1 validation suite)

Test Results:
- [X] tests passing
- Coverage: [Y]%
- All integration tests passing
- M1 complete workflow validated

This completes M1 Foundation. All core components working:
✅ Storage layer
✅ WAL logging and recovery
✅ Run tracking
✅ Basic operations
✅ Primitive layer (KV)

Ready for M2: Transactions (OCC)
"

# Push to develop
git push origin develop

# Tag the release
git tag -a epic-5-complete -m "Epic 5: Database Engine Shell - Complete

M1 FOUNDATION COMPLETE ✅

Components:
- Database orchestration layer
- Run lifecycle management
- Non-transactional operations
- KV primitive facade
- Complete integration test suite

Statistics:
- [X] total tests
- [Y]% coverage
- All integration tests passing

Next: M2 - Transactions (OCC)
"

git push origin epic-5-complete
```

### 5. Close Epic Issue
```bash
/opt/homebrew/bin/gh issue close 5 --comment "Epic 5: Database Engine Shell - COMPLETE ✅

All 5 user stories completed:
- ✅ Story #28: Database struct enhancements
- ✅ Story #29: Run tracking (begin_run, end_run)
- ✅ Story #30: Basic put/get operations
- ✅ Story #31: KV primitive facade
- ✅ Story #32: Integration tests

**M1 FOUNDATION COMPLETE** 🎉

Test Results:
- [X] tests passing
- Coverage: [Y]%
- All integration tests passing
- M1 complete workflow validated

The integration test proves all M1 components work together:
- ✅ Storage layer
- ✅ WAL logging
- ✅ Recovery
- ✅ Run tracking
- ✅ KV primitive
- ✅ End-to-end workflow (write, restart, read)

Next milestone: M2 - Transactions (OCC)

Review document: docs/milestones/EPIC_5_REVIEW.md
"
```

### 6. Update Project Status
Update `docs/milestones/PROJECT_STATUS.md`:
- Change current phase from "Epic 4 Complete" to "M1 FOUNDATION COMPLETE"
- Mark all Epic 5 stories complete
- Update totals: 27/27 stories (100%), 5/5 epics (100%)
- Add Epic 5 results section
- Update "Next Steps" to point to M2

---

## Critical Notes

### Architecture Principles
1. **Primitives are stateless facades** - KV just wraps Database, no state
2. **Layer boundaries are strict** - Primitives → Engine → Storage
3. **Operations are atomic** - WAL lock held during storage + WAL update
4. **Run-scoped operations** - Every operation tagged with run_id

### Testing Philosophy
- Integration tests validate **complete workflows**
- If `test_m1_complete_workflow` passes, M1 is proven working
- Tests must not be adjusted to pass - code must be fixed

### M1 Completion Validation
**M1 is complete when**:
- All 5 Epic 5 tests pass
- Integration test passes: write → restart → read
- All 27 M1 stories complete
- Coverage >90% for core components
- Ready to begin M2 (Transactions)

---

## Summary

Epic 5 completes the M1 Foundation by creating the Database engine shell that orchestrates all M1 components:
- Database struct manages storage, WAL, and run tracking
- Run lifecycle enables run-scoped operations
- Basic put/get operations with WAL durability
- KV primitive establishes stateless facade pattern
- Integration tests validate complete M1 system

**After Epic 5**: M1 Foundation is complete. All components proven working together. Ready to begin M2 (Transactions with OCC).
