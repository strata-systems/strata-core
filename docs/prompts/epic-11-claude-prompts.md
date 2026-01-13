# Epic 11: Backwards Compatibility - Implementation Prompts

**Epic Goal**: Ensure M1-style APIs work seamlessly with M2 transaction infrastructure, validate backwards compatibility, and document migration paths.

**Status**: Ready to begin
**Dependencies**: Epic 10 (Database API Integration) complete

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
       └── epic-11-backwards-compat    ← Epic branch (base for all story PRs)
            └── epic-11-story-103-*    ← Story branches
```

### Critical Rules

1. **Story PRs go to EPIC branch, NOT main**
   ```bash
   # CORRECT: PR base is epic branch
   /opt/homebrew/bin/gh pr create --base epic-11-backwards-compat --head epic-11-story-103-m1-api-tests

   # WRONG: Never PR directly to main
   /opt/homebrew/bin/gh pr create --base main --head epic-11-story-103-m1-api-tests  # ❌ NEVER DO THIS
   ```

2. **Epic branches merge to develop** (after all stories complete)
   ```bash
   git checkout develop
   git merge --no-ff epic-11-backwards-compat
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
./scripts/complete-story.sh 103  # Creates PR to epic-11-backwards-compat
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

## Epic 11 Overview

### Scope
Epic 11 ensures M1 backwards compatibility with M2 transaction infrastructure:
- Validate that all M1-style operations work correctly
- Test run lifecycle integration with transactions
- Enable primitives layer to use transactions
- Document migration paths for users
- Comprehensive backwards compatibility validation

### Key Spec References

#### Section 4: Implicit Transactions
| Rule | Description |
|------|-------------|
| **db.put()** | Wraps in implicit transaction, commits immediately |
| **db.get()** | Creates snapshot, read-only transaction, always succeeds |
| **db.delete()** | Wraps in implicit transaction, commits immediately |
| **Backwards compatible** | M1 code works unchanged in M2 |

#### Core Invariants (Must Be Preserved)
| Invariant | Description |
|-----------|-------------|
| **No partial commits** | M1 single-ops are atomic |
| **All-or-nothing** | Single operation succeeds or fails completely |
| **Monotonic versions** | Versions never decrease |
| **Durability** | All M1 writes are durable (WAL) |

### What "Backwards Compatible" Means

1. **API Compatibility**: M1 code compiles and runs unchanged
2. **Semantic Compatibility**: M1 operations behave the same way
3. **Performance Compatibility**: M1 operations are not significantly slower
4. **Durability Compatibility**: M1 writes are still durable via WAL

### Success Criteria
- [ ] All existing M1 tests pass unchanged
- [ ] M1-style API works identically to M1 behavior
- [ ] Run lifecycle integrates with transactions
- [ ] Primitives can use transaction API
- [ ] Migration documentation complete
- [ ] All validation tests pass

### Component Breakdown
- **Story #103**: M1 API Compatibility Tests 🔴 FOUNDATION
- **Story #104**: Run Lifecycle Integration
- **Story #105**: Primitives Transaction Support
- **Story #106**: Migration Documentation
- **Story #107**: Backwards Compatibility Validation

---

## Dependency Graph

```
Phase 1 (Sequential - CRITICAL):
  Story #103 (M1 API Compatibility Tests)
    └─> 🔴 BLOCKS #104, #105

Phase 2 (Parallel - 2 Claudes after #103):
  Story #104 (Run Lifecycle Integration)
  Story #105 (Primitives Transaction Support)
    └─> Both depend on #103
    └─> Independent of each other

Phase 3 (Sequential):
  Story #106 (Migration Documentation)
    └─> Depends on #104, #105

Phase 4 (Sequential):
  Story #107 (Validation)
    └─> Depends on all previous stories
```

---

## Parallelization Strategy

### Optimal Parallel Execution (2 Claudes)

| Phase | Duration | Claude 1 | Claude 2 |
|-------|----------|----------|----------|
| 1 | 4 hours | #103 M1 API Tests | - |
| 2 | 4 hours | #104 Run Lifecycle | #105 Primitives |
| 3 | 2 hours | #106 Documentation | - |
| 4 | 3 hours | #107 Validation | - |

**Total Wall Time**: ~13 hours (vs. ~17 hours sequential)

---

## Existing Infrastructure

Epic 11 builds on:

### From Epic 10 (Database API Integration)
- `Database::transaction()` closure API
- `Database::put()`, `get()`, `delete()`, `cas()` (implicit transactions)
- `TransactionCoordinator` for metrics
- `RetryConfig` for automatic retry
- Recovery integration on startup

### Current M1-Style API (from Epic 10)
```rust
impl Database {
    /// M1-style put - wraps in implicit transaction
    pub fn put(&self, run_id: RunId, key: Key, value: Value) -> Result<()>;

    /// M1-style get - read-only, always succeeds
    pub fn get(&self, key: &Key) -> Result<Option<VersionedValue>>;

    /// M1-style delete - wraps in implicit transaction
    pub fn delete(&self, run_id: RunId, key: Key) -> Result<()>;

    /// M1-style CAS - wraps in implicit transaction
    pub fn cas(&self, run_id: RunId, key: Key, expected_version: u64, new_value: Value) -> Result<()>;
}
```

### Storage Layer (from M1)
```rust
impl UnifiedStore {
    pub fn put(&self, key: Key, value: Value, ttl: Option<Duration>) -> Result<()>;
    pub fn get(&self, key: &Key) -> Result<Option<VersionedValue>>;
    pub fn delete(&self, key: &Key) -> Result<()>;
    pub fn scan_prefix(&self, prefix: &Key) -> Result<Vec<(Key, VersionedValue)>>;
}
```

---

## Story #103: M1 API Compatibility Tests

**GitHub Issue**: #103
**Estimated Time**: 4 hours
**Dependencies**: Epic 10 complete
**Blocks**: Stories #104, #105, #106, #107

### ⚠️ PREREQUISITE: Read the Semantics Spec

Before writing ANY code, read:
- Section 4: Implicit Transactions (entire section)
- Section 4.2: Implicit Transaction Behavior
- Core Invariants (all)

### What This Story Does

Creates comprehensive tests to validate that M1-style API continues to work correctly in M2. These tests serve as a **regression suite** to ensure backwards compatibility.

### What to Implement

Create `crates/engine/tests/m1_compatibility_tests.rs`:

```rust
//! M1 API Compatibility Tests
//!
//! These tests validate that M1-style operations work correctly in M2.
//! Per spec Section 4: Implicit transactions wrap M1-style operations.
//!
//! SUCCESS CRITERIA:
//! - All M1 operations work unchanged
//! - Behavior is identical to M1
//! - Performance is acceptable
//! - Durability is preserved

use in_mem_core::error::Result;
use in_mem_core::traits::Storage;
use in_mem_core::types::{Key, Namespace, RunId};
use in_mem_core::value::Value;
use in_mem_engine::Database;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
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
// Basic M1 Operations - Must Work Unchanged
// ============================================================================

#[test]
fn test_m1_put_get_basic() {
    // M1 pattern: simple put then get
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);
    let key = Key::new_kv(ns, "simple_key");

    // M1-style put
    db.put(run_id, key.clone(), Value::String("hello".to_string())).unwrap();

    // M1-style get
    let result = db.get(&key).unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().value, Value::String("hello".to_string()));
}

#[test]
fn test_m1_put_overwrite() {
    // M1 pattern: put then overwrite
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);
    let key = Key::new_kv(ns, "overwrite_key");

    db.put(run_id, key.clone(), Value::I64(1)).unwrap();
    db.put(run_id, key.clone(), Value::I64(2)).unwrap();
    db.put(run_id, key.clone(), Value::I64(3)).unwrap();

    let result = db.get(&key).unwrap().unwrap();
    assert_eq!(result.value, Value::I64(3));
}

#[test]
fn test_m1_delete_basic() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);
    let key = Key::new_kv(ns, "delete_key");

    db.put(run_id, key.clone(), Value::I64(100)).unwrap();
    assert!(db.get(&key).unwrap().is_some());

    db.delete(run_id, key.clone()).unwrap();
    assert!(db.get(&key).unwrap().is_none());
}

#[test]
fn test_m1_delete_nonexistent() {
    // M1 pattern: delete a key that doesn't exist (should succeed)
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);
    let key = Key::new_kv(ns, "never_existed");

    // Should not error
    db.delete(run_id, key.clone()).unwrap();
}

#[test]
fn test_m1_get_nonexistent() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);
    let key = Key::new_kv(ns, "nonexistent");

    let result = db.get(&key).unwrap();
    assert!(result.is_none());
}

// ============================================================================
// M1 CAS Operations
// ============================================================================

#[test]
fn test_m1_cas_create_new() {
    // CAS with version 0 creates a new key
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);
    let key = Key::new_kv(ns, "cas_new");

    db.cas(run_id, key.clone(), 0, Value::I64(1)).unwrap();

    let result = db.get(&key).unwrap().unwrap();
    assert_eq!(result.value, Value::I64(1));
}

#[test]
fn test_m1_cas_update_existing() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);
    let key = Key::new_kv(ns, "cas_update");

    // Create
    db.put(run_id, key.clone(), Value::I64(1)).unwrap();
    let v1 = db.get(&key).unwrap().unwrap();

    // Update with correct version
    db.cas(run_id, key.clone(), v1.version, Value::I64(2)).unwrap();

    let v2 = db.get(&key).unwrap().unwrap();
    assert_eq!(v2.value, Value::I64(2));
    assert!(v2.version > v1.version);
}

#[test]
fn test_m1_cas_version_mismatch() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);
    let key = Key::new_kv(ns, "cas_mismatch");

    db.put(run_id, key.clone(), Value::I64(1)).unwrap();

    // CAS with wrong version should fail
    let result = db.cas(run_id, key.clone(), 999, Value::I64(2));
    assert!(result.is_err());

    // Value unchanged
    let stored = db.get(&key).unwrap().unwrap();
    assert_eq!(stored.value, Value::I64(1));
}

// ============================================================================
// M1 Data Types - All Value Types Must Work
// ============================================================================

#[test]
fn test_m1_value_types() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);

    // String
    db.put(run_id, Key::new_kv(ns.clone(), "string"), Value::String("test".to_string())).unwrap();
    assert_eq!(db.get(&Key::new_kv(ns.clone(), "string")).unwrap().unwrap().value, Value::String("test".to_string()));

    // I64
    db.put(run_id, Key::new_kv(ns.clone(), "i64"), Value::I64(42)).unwrap();
    assert_eq!(db.get(&Key::new_kv(ns.clone(), "i64")).unwrap().unwrap().value, Value::I64(42));

    // F64
    db.put(run_id, Key::new_kv(ns.clone(), "f64"), Value::F64(3.14)).unwrap();
    assert_eq!(db.get(&Key::new_kv(ns.clone(), "f64")).unwrap().unwrap().value, Value::F64(3.14));

    // Bool
    db.put(run_id, Key::new_kv(ns.clone(), "bool"), Value::Bool(true)).unwrap();
    assert_eq!(db.get(&Key::new_kv(ns.clone(), "bool")).unwrap().unwrap().value, Value::Bool(true));

    // Bytes
    db.put(run_id, Key::new_kv(ns.clone(), "bytes"), Value::Bytes(vec![1, 2, 3])).unwrap();
    assert_eq!(db.get(&Key::new_kv(ns.clone(), "bytes")).unwrap().unwrap().value, Value::Bytes(vec![1, 2, 3]));

    // Null
    db.put(run_id, Key::new_kv(ns.clone(), "null"), Value::Null).unwrap();
    assert_eq!(db.get(&Key::new_kv(ns.clone(), "null")).unwrap().unwrap().value, Value::Null);
}

// ============================================================================
// M1 Durability - Must Survive Restart
// ============================================================================

#[test]
fn test_m1_durability_survives_restart() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("db");

    let run_id = RunId::new();
    let ns = create_ns(run_id);
    let key = Key::new_kv(ns.clone(), "durable");

    // Write and close
    {
        let db = Database::open(&db_path).unwrap();
        db.put(run_id, key.clone(), Value::String("persisted".to_string())).unwrap();
    }

    // Reopen and verify
    {
        let db = Database::open(&db_path).unwrap();
        let result = db.get(&key).unwrap().unwrap();
        assert_eq!(result.value, Value::String("persisted".to_string()));
    }
}

#[test]
fn test_m1_durability_multiple_restarts() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("db");

    let run_id = RunId::new();
    let ns = create_ns(run_id);

    // First session
    {
        let db = Database::open(&db_path).unwrap();
        db.put(run_id, Key::new_kv(ns.clone(), "key1"), Value::I64(1)).unwrap();
    }

    // Second session
    {
        let db = Database::open(&db_path).unwrap();
        assert_eq!(db.get(&Key::new_kv(ns.clone(), "key1")).unwrap().unwrap().value, Value::I64(1));
        db.put(run_id, Key::new_kv(ns.clone(), "key2"), Value::I64(2)).unwrap();
    }

    // Third session
    {
        let db = Database::open(&db_path).unwrap();
        assert_eq!(db.get(&Key::new_kv(ns.clone(), "key1")).unwrap().unwrap().value, Value::I64(1));
        assert_eq!(db.get(&Key::new_kv(ns.clone(), "key2")).unwrap().unwrap().value, Value::I64(2));
    }
}

// ============================================================================
// M1 Concurrency - Multiple Threads
// ============================================================================

#[test]
fn test_m1_concurrent_writes_different_keys() {
    let temp_dir = TempDir::new().unwrap();
    let db = Arc::new(Database::open(temp_dir.path().join("db")).unwrap());

    let run_id = RunId::new();
    let ns = create_ns(run_id);
    let mut handles = vec![];

    // 10 threads, each writing to different keys
    for i in 0..10 {
        let db = Arc::clone(&db);
        let ns = ns.clone();

        handles.push(thread::spawn(move || {
            for j in 0..100 {
                let key = Key::new_kv(ns.clone(), &format!("t{}k{}", i, j));
                db.put(run_id, key, Value::I64((i * 100 + j) as i64)).unwrap();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // Verify all keys
    for i in 0..10 {
        for j in 0..100 {
            let key = Key::new_kv(ns.clone(), &format!("t{}k{}", i, j));
            let val = db.get(&key).unwrap().unwrap();
            assert_eq!(val.value, Value::I64((i * 100 + j) as i64));
        }
    }
}

#[test]
fn test_m1_concurrent_writes_same_key() {
    // Per spec Section 3.2: Blind writes don't conflict
    let temp_dir = TempDir::new().unwrap();
    let db = Arc::new(Database::open(temp_dir.path().join("db")).unwrap());

    let run_id = RunId::new();
    let ns = create_ns(run_id);
    let key = Key::new_kv(ns, "contested");
    let mut handles = vec![];

    // 10 threads writing to the same key
    for i in 0..10 {
        let db = Arc::clone(&db);
        let key = key.clone();

        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                db.put(run_id, key.clone(), Value::I64(i as i64)).unwrap();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // Key should exist (one of the values)
    let val = db.get(&key).unwrap();
    assert!(val.is_some());
}

#[test]
fn test_m1_concurrent_reads() {
    let temp_dir = TempDir::new().unwrap();
    let db = Arc::new(Database::open(temp_dir.path().join("db")).unwrap());

    let run_id = RunId::new();
    let ns = create_ns(run_id);

    // Pre-populate
    for i in 0..100 {
        let key = Key::new_kv(ns.clone(), &format!("key{}", i));
        db.put(run_id, key, Value::I64(i as i64)).unwrap();
    }

    let mut handles = vec![];

    // 10 threads reading concurrently
    for _ in 0..10 {
        let db = Arc::clone(&db);
        let ns = ns.clone();

        handles.push(thread::spawn(move || {
            for i in 0..100 {
                let key = Key::new_kv(ns.clone(), &format!("key{}", i));
                let val = db.get(&key).unwrap().unwrap();
                assert_eq!(val.value, Value::I64(i as i64));
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}

// ============================================================================
// M1 Version Semantics
// ============================================================================

#[test]
fn test_m1_versions_monotonic() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);
    let key = Key::new_kv(ns, "versioned");

    let mut last_version = 0;

    for i in 0..10 {
        db.put(run_id, key.clone(), Value::I64(i)).unwrap();
        let vv = db.get(&key).unwrap().unwrap();
        assert!(vv.version > last_version, "Versions must be monotonic");
        last_version = vv.version;
    }
}

#[test]
fn test_m1_version_zero_semantics() {
    // Per spec Section 6.4: Version 0 = never existed
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);
    let key = Key::new_kv(ns, "version_zero");

    // CAS with version 0 creates new key
    db.cas(run_id, key.clone(), 0, Value::I64(1)).unwrap();
    let v1 = db.get(&key).unwrap().unwrap();
    assert!(v1.version > 0);

    // CAS with version 0 on existing key fails
    let result = db.cas(run_id, key.clone(), 0, Value::I64(2));
    assert!(result.is_err());
}

// ============================================================================
// M1 Large Data
// ============================================================================

#[test]
fn test_m1_large_value() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);
    let key = Key::new_kv(ns, "large");

    // 1MB value
    let large_data = vec![42u8; 1024 * 1024];
    db.put(run_id, key.clone(), Value::Bytes(large_data.clone())).unwrap();

    let result = db.get(&key).unwrap().unwrap();
    assert_eq!(result.value, Value::Bytes(large_data));
}

#[test]
fn test_m1_many_keys() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);

    // 10,000 keys
    for i in 0..10_000 {
        let key = Key::new_kv(ns.clone(), &format!("key_{:05}", i));
        db.put(run_id, key, Value::I64(i as i64)).unwrap();
    }

    // Verify sample
    for i in (0..10_000).step_by(1000) {
        let key = Key::new_kv(ns.clone(), &format!("key_{:05}", i));
        let val = db.get(&key).unwrap().unwrap();
        assert_eq!(val.value, Value::I64(i as i64));
    }
}

// ============================================================================
// M1 Error Handling
// ============================================================================

#[test]
fn test_m1_cas_conflict_error() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);
    let key = Key::new_kv(ns, "cas_error");

    db.put(run_id, key.clone(), Value::I64(1)).unwrap();

    let result = db.cas(run_id, key, 999, Value::I64(2));
    assert!(result.is_err());
    // Should be a TransactionConflict error
    assert!(result.unwrap_err().is_conflict());
}

// ============================================================================
// M1 Mixed Operations
// ============================================================================

#[test]
fn test_m1_mixed_operations_sequence() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();

    let run_id = RunId::new();
    let ns = create_ns(run_id);
    let key = Key::new_kv(ns, "mixed");

    // Create
    db.put(run_id, key.clone(), Value::I64(1)).unwrap();
    assert_eq!(db.get(&key).unwrap().unwrap().value, Value::I64(1));

    // Update
    db.put(run_id, key.clone(), Value::I64(2)).unwrap();
    assert_eq!(db.get(&key).unwrap().unwrap().value, Value::I64(2));

    // CAS update
    let v = db.get(&key).unwrap().unwrap();
    db.cas(run_id, key.clone(), v.version, Value::I64(3)).unwrap();
    assert_eq!(db.get(&key).unwrap().unwrap().value, Value::I64(3));

    // Delete
    db.delete(run_id, key.clone()).unwrap();
    assert!(db.get(&key).unwrap().is_none());

    // Recreate
    db.put(run_id, key.clone(), Value::I64(4)).unwrap();
    assert_eq!(db.get(&key).unwrap().unwrap().value, Value::I64(4));
}
```

### Implementation Steps

#### Step 1: Create epic branch and start story

```bash
# Create epic branch from develop
git checkout develop
git pull origin develop
git checkout -b epic-11-backwards-compat

# Push epic branch
git push -u origin epic-11-backwards-compat

# Start the story
./scripts/start-story.sh 11 103 m1-api-tests
```

#### Step 2: Create the test file

Create `crates/engine/tests/m1_compatibility_tests.rs` with the tests above.

#### Step 3: Run validation

```bash
~/.cargo/bin/cargo test -p in-mem-engine --test m1_compatibility_tests
~/.cargo/bin/cargo clippy -p in-mem-engine -- -D warnings
~/.cargo/bin/cargo fmt --check
```

### Acceptance Criteria

- [ ] All M1 basic operations pass (put, get, delete)
- [ ] All M1 CAS operations pass
- [ ] All value types work correctly
- [ ] Durability survives restarts
- [ ] Concurrent operations work correctly
- [ ] Version semantics preserved
- [ ] Large data works
- [ ] Error handling correct
- [ ] All tests pass
- [ ] No clippy warnings

### Complete the Story

```bash
./scripts/complete-story.sh 103
```

---

## Story #104: Run Lifecycle Integration

**GitHub Issue**: #104
**Estimated Time**: 4 hours
**Dependencies**: Story #103
**Blocks**: Story #106, #107

### What This Story Does

Integrates run lifecycle (begin_run, end_run, fork_run) with the M2 transaction infrastructure. Ensures runs can use transactions.

### What to Implement

Update `crates/engine/src/database.rs` to support run lifecycle with transactions:

```rust
impl Database {
    /// Begin a new run
    ///
    /// Creates run metadata and returns a RunId.
    /// All subsequent operations with this RunId are isolated.
    pub fn begin_run(&self, parent: Option<RunId>, metadata: RunMetadata) -> Result<RunId>;

    /// End a run
    ///
    /// Marks the run as complete. Data remains accessible.
    pub fn end_run(&self, run_id: RunId) -> Result<()>;

    /// Fork a run
    ///
    /// Creates a new run based on an existing run's state.
    /// Uses transactions to ensure atomic copy.
    pub fn fork_run(&self, source: RunId, metadata: RunMetadata) -> Result<RunId>;

    /// Get run metadata
    pub fn get_run_metadata(&self, run_id: RunId) -> Result<Option<RunMetadata>>;

    /// List active runs
    pub fn list_runs(&self) -> Result<Vec<RunId>>;
}
```

### Implementation Steps

#### Step 1: Start the story

```bash
./scripts/start-story.sh 11 104 run-lifecycle
```

#### Step 2: Write tests FIRST

```rust
#[cfg(test)]
mod run_lifecycle_tests {
    use super::*;

    #[test]
    fn test_begin_run() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();

        let run_id = db.begin_run(None, RunMetadata::default()).unwrap();
        assert!(!run_id.is_nil());
    }

    #[test]
    fn test_run_isolation() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();

        let run1 = db.begin_run(None, RunMetadata::default()).unwrap();
        let run2 = db.begin_run(None, RunMetadata::default()).unwrap();

        let ns1 = Namespace::for_run(run1);
        let ns2 = Namespace::for_run(run2);

        // Write to run1
        db.put(run1, Key::new_kv(ns1.clone(), "key"), Value::I64(1)).unwrap();

        // Write to run2
        db.put(run2, Key::new_kv(ns2.clone(), "key"), Value::I64(2)).unwrap();

        // Each run sees only its own data
        assert_eq!(db.get(&Key::new_kv(ns1, "key")).unwrap().unwrap().value, Value::I64(1));
        assert_eq!(db.get(&Key::new_kv(ns2, "key")).unwrap().unwrap().value, Value::I64(2));
    }

    #[test]
    fn test_fork_run() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();

        let parent = db.begin_run(None, RunMetadata::default()).unwrap();
        let parent_ns = Namespace::for_run(parent);

        // Add data to parent
        db.put(parent, Key::new_kv(parent_ns.clone(), "key"), Value::I64(100)).unwrap();

        // Fork
        let child = db.fork_run(parent, RunMetadata::default()).unwrap();
        let child_ns = Namespace::for_run(child);

        // Child should have parent's data
        // (Note: Namespace includes run_id, so this needs proper key mapping)
    }
}
```

#### Step 3: Implement run lifecycle

This requires implementing `RunMetadata` and run management structures.

#### Step 4: Run validation

```bash
~/.cargo/bin/cargo test -p in-mem-engine
~/.cargo/bin/cargo clippy -p in-mem-engine -- -D warnings
~/.cargo/bin/cargo fmt --check
```

### Acceptance Criteria

- [ ] begin_run() creates new run with RunId
- [ ] end_run() marks run complete
- [ ] Runs are isolated by Namespace
- [ ] fork_run() copies data atomically using transactions
- [ ] Run metadata can be retrieved
- [ ] All tests pass
- [ ] No clippy warnings

---

## Story #105: Primitives Transaction Support

**GitHub Issue**: #105
**Estimated Time**: 4 hours
**Dependencies**: Story #103
**Blocks**: Story #106, #107

### What This Story Does

Enables the primitives layer (KV Store, etc.) to use the transaction API. This ensures higher-level abstractions can benefit from M2 transactions.

### What to Implement

Create `crates/primitives/src/kv.rs`:

```rust
//! KV Store Primitive
//!
//! Provides a simple key-value interface over the Database.
//! Supports both M1-style implicit transactions and M2 explicit transactions.

use in_mem_core::error::Result;
use in_mem_core::types::{Key, Namespace, RunId};
use in_mem_core::value::Value;
use in_mem_core::VersionedValue;
use in_mem_engine::Database;
use std::sync::Arc;

/// KV Store primitive
pub struct KVStore {
    db: Arc<Database>,
    run_id: RunId,
    namespace: Namespace,
}

impl KVStore {
    /// Create a new KV store for a run
    pub fn new(db: Arc<Database>, run_id: RunId) -> Self {
        let namespace = Namespace::new(
            "default".to_string(),
            "default".to_string(),
            "default".to_string(),
            run_id,
        );
        Self { db, run_id, namespace }
    }

    /// Set a key-value pair
    pub fn set(&self, key: &str, value: Value) -> Result<()> {
        let k = Key::new_kv(self.namespace.clone(), key);
        self.db.put(self.run_id, k, value)
    }

    /// Get a value by key
    pub fn get(&self, key: &str) -> Result<Option<Value>> {
        let k = Key::new_kv(self.namespace.clone(), key);
        Ok(self.db.get(&k)?.map(|vv| vv.value))
    }

    /// Delete a key
    pub fn delete(&self, key: &str) -> Result<()> {
        let k = Key::new_kv(self.namespace.clone(), key);
        self.db.delete(self.run_id, k)
    }

    /// Atomic batch operation using transaction
    pub fn batch<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&BatchContext) -> Result<T>,
    {
        self.db.transaction(self.run_id, |txn| {
            let ctx = BatchContext {
                txn,
                namespace: &self.namespace,
            };
            f(&ctx)
        })
    }
}

/// Context for batch operations within a transaction
pub struct BatchContext<'a> {
    txn: &'a mut in_mem_concurrency::TransactionContext,
    namespace: &'a Namespace,
}

impl<'a> BatchContext<'a> {
    /// Set a key within the batch
    pub fn set(&mut self, key: &str, value: Value) -> Result<()> {
        let k = Key::new_kv(self.namespace.clone(), key);
        self.txn.put(k, value)
    }

    /// Get a key within the batch (reads from snapshot + uncommitted writes)
    pub fn get(&self, key: &str) -> Result<Option<Value>> {
        let k = Key::new_kv(self.namespace.clone(), key);
        self.txn.get(&k)
    }

    /// Delete a key within the batch
    pub fn delete(&mut self, key: &str) -> Result<()> {
        let k = Key::new_kv(self.namespace.clone(), key);
        self.txn.delete(k)
    }
}
```

### Implementation Steps

#### Step 1: Start the story

```bash
./scripts/start-story.sh 11 105 primitives-txn
```

#### Step 2: Write tests FIRST

```rust
#[cfg(test)]
mod kv_store_tests {
    use super::*;

    #[test]
    fn test_kv_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let db = Arc::new(Database::open(temp_dir.path().join("db")).unwrap());
        let run_id = RunId::new();

        let kv = KVStore::new(db, run_id);

        kv.set("key1", Value::I64(42)).unwrap();
        assert_eq!(kv.get("key1").unwrap(), Some(Value::I64(42)));

        kv.delete("key1").unwrap();
        assert!(kv.get("key1").unwrap().is_none());
    }

    #[test]
    fn test_kv_batch_atomic() {
        let temp_dir = TempDir::new().unwrap();
        let db = Arc::new(Database::open(temp_dir.path().join("db")).unwrap());
        let run_id = RunId::new();

        let kv = KVStore::new(db, run_id);

        // Batch operation
        kv.batch(|ctx| {
            ctx.set("a", Value::I64(1))?;
            ctx.set("b", Value::I64(2))?;
            ctx.set("c", Value::I64(3))?;
            Ok(())
        }).unwrap();

        // All keys should exist
        assert_eq!(kv.get("a").unwrap(), Some(Value::I64(1)));
        assert_eq!(kv.get("b").unwrap(), Some(Value::I64(2)));
        assert_eq!(kv.get("c").unwrap(), Some(Value::I64(3)));
    }

    #[test]
    fn test_kv_batch_rollback_on_error() {
        let temp_dir = TempDir::new().unwrap();
        let db = Arc::new(Database::open(temp_dir.path().join("db")).unwrap());
        let run_id = RunId::new();

        let kv = KVStore::new(db, run_id);

        // Failing batch
        let result = kv.batch(|ctx| {
            ctx.set("x", Value::I64(100))?;
            Err(in_mem_core::error::Error::InvalidState("rollback".to_string()))
        });

        assert!(result.is_err());

        // Key should not exist (rolled back)
        assert!(kv.get("x").unwrap().is_none());
    }
}
```

#### Step 3: Implement KVStore primitive

#### Step 4: Update primitives lib.rs to export KVStore

```rust
pub mod kv;
pub use kv::KVStore;
```

#### Step 5: Run validation

```bash
~/.cargo/bin/cargo test -p in-mem-primitives
~/.cargo/bin/cargo clippy -p in-mem-primitives -- -D warnings
~/.cargo/bin/cargo fmt --check
```

### Acceptance Criteria

- [ ] KVStore provides simple key-value API
- [ ] set/get/delete work with M1-style implicit transactions
- [ ] batch() enables atomic multi-key operations
- [ ] batch rollback on error works correctly
- [ ] All tests pass
- [ ] No clippy warnings

---

## Story #106: Migration Documentation

**GitHub Issue**: #106
**Estimated Time**: 2 hours
**Dependencies**: Stories #104, #105
**Blocks**: Story #107

### What This Story Does

Creates comprehensive documentation for migrating from M1 to M2. Documents any API changes, new capabilities, and best practices.

### What to Create

Create `docs/M1_TO_M2_MIGRATION.md`:

```markdown
# M1 to M2 Migration Guide

## Overview

M2 adds transaction support while maintaining full backwards compatibility with M1.
All M1 code continues to work unchanged.

## What's New in M2

### Transaction API

```rust
// M2 explicit transaction
db.transaction(run_id, |txn| {
    let val = txn.get(&key)?;
    txn.put(key, new_value)?;
    Ok(val)
})?;
```

### Automatic Retry

```rust
// Retry on conflict
db.transaction_with_retry(run_id, RetryConfig::default(), |txn| {
    // ...
})?;
```

## M1 Operations in M2

All M1 operations work unchanged:

| M1 Operation | M2 Behavior |
|--------------|-------------|
| `db.put(...)` | Wraps in implicit transaction |
| `db.get(...)` | Read-only snapshot |
| `db.delete(...)` | Wraps in implicit transaction |
| `db.cas(...)` | Wraps in implicit transaction |

## When to Use Transactions

| Use Case | Recommendation |
|----------|----------------|
| Single key read/write | M1 style (implicit) |
| Multiple related keys | Explicit transaction |
| Read-modify-write | Explicit transaction with retry |
| Batch operations | Explicit transaction |

## Breaking Changes

**None.** M2 is fully backwards compatible with M1.

## Best Practices

1. Use explicit transactions for multi-key atomicity
2. Use retry for high-contention scenarios
3. Keep transactions short
4. Read all keys before writing if you need constraint checking
```

### Implementation Steps

#### Step 1: Start the story

```bash
./scripts/start-story.sh 11 106 migration-docs
```

#### Step 2: Create the documentation file

#### Step 3: Update other docs to reference migration guide

### Acceptance Criteria

- [ ] Migration guide created
- [ ] All M2 features documented
- [ ] M1 behavior in M2 explained
- [ ] Best practices included
- [ ] No breaking changes documented

---

## Story #107: Backwards Compatibility Validation

**GitHub Issue**: #107
**Estimated Time**: 3 hours
**Dependencies**: All previous stories
**Blocks**: None (Epic 11 complete)

### What This Story Does

Final validation of backwards compatibility. Runs all tests, creates validation report, and ensures Epic 11 is complete.

### What to Implement

1. **Run full test suite**
2. **Create EPIC_11_REVIEW.md**
3. **Validate all acceptance criteria**

### Implementation Steps

#### Step 1: Start the story

```bash
./scripts/start-story.sh 11 107 validation
```

#### Step 2: Run full validation

```bash
~/.cargo/bin/cargo test --all
~/.cargo/bin/cargo clippy --all -- -D warnings
~/.cargo/bin/cargo fmt --check
```

#### Step 3: Create validation report

Create `docs/milestones/EPIC_11_REVIEW.md`:

```markdown
# Epic 11: Backwards Compatibility - Validation Report

**Epic**: Backwards Compatibility
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
| M1 API compatible | ✅ |

---

## Stories Completed

| Story | Title | Description |
|-------|-------|-------------|
| #103 | M1 API Compatibility Tests | Comprehensive M1 regression tests |
| #104 | Run Lifecycle Integration | Runs work with transactions |
| #105 | Primitives Transaction Support | KVStore with transaction API |
| #106 | Migration Documentation | M1 to M2 migration guide |
| #107 | Validation | This report |

---

## Backwards Compatibility Verification

### M1 API Unchanged
- [x] db.put() works as before
- [x] db.get() works as before
- [x] db.delete() works as before
- [x] db.cas() works as before

### Semantic Compatibility
- [x] All value types work
- [x] Durability preserved
- [x] Concurrent operations work
- [x] Version semantics preserved

---

## Ready for: Epic 12 (OCC Validation & Benchmarking)
```

### Acceptance Criteria

- [ ] All ~650+ tests pass
- [ ] Clippy clean
- [ ] Formatting clean
- [ ] EPIC_11_REVIEW.md created
- [ ] M1 API verified compatible
- [ ] Migration documentation complete
- [ ] Epic 11 complete

### Complete the Epic

After Story #107 is merged to epic-11-backwards-compat:

```bash
# Merge epic to develop
git checkout develop
git merge --no-ff epic-11-backwards-compat
git push origin develop
```

---

## Quick Reference: Story Commands

```bash
# Start a story
./scripts/start-story.sh 11 <story_number> <description>

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
| M1 db.put() works unchanged | [ ] |
| M1 db.get() works unchanged | [ ] |
| M1 db.delete() works unchanged | [ ] |
| M1 db.cas() works unchanged | [ ] |
| Implicit transactions wrap M1 ops (Section 4) | [ ] |
| All value types work | [ ] |
| Durability preserved (WAL) | [ ] |
| Version semantics correct (Section 6) | [ ] |
| Concurrent operations work | [ ] |

---

*Generated for Epic 11: Backwards Compatibility*
*Spec Reference: docs/architecture/M2_TRANSACTION_SEMANTICS.md Section 4*
