# Epic 7: Transaction Semantics - Implementation Prompts

**Epic Goal**: Conflict detection and validation logic for M2 OCC.

**Status**: Ready to begin
**Dependencies**: Epic 6 (Transaction Foundations) complete

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
       └── epic-7-transaction-semantics  ← Epic branch (base for all story PRs)
            └── epic-7-story-83-*        ← Story branches
```

### Critical Rules

1. **Story PRs go to EPIC branch, NOT main**
   ```bash
   # CORRECT: PR base is epic branch
   /opt/homebrew/bin/gh pr create --base epic-7-transaction-semantics --head epic-7-story-83-conflict-infrastructure

   # WRONG: Never PR directly to main
   /opt/homebrew/bin/gh pr create --base main --head epic-7-story-83-conflict-infrastructure  # ❌ NEVER DO THIS
   ```

2. **Epic branches merge to develop** (after all stories complete)
   ```bash
   git checkout develop
   git merge --no-ff epic-7-transaction-semantics
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
./scripts/complete-story.sh 83  # Creates PR to epic-7-transaction-semantics
```

**If you manually create a PR, ALWAYS verify the base branch is the epic branch, not main.**

---

## Epic 7 Overview

### Scope
- ConflictType enum and ValidationResult struct
- Read-set validation (read-write conflicts)
- Write-set validation (write-write conflicts with prior read)
- CAS validation (version mismatch)
- Full transaction validation orchestration

### Success Criteria
- [ ] ConflictType enum with all conflict variants
- [ ] ValidationResult struct for reporting
- [ ] validate_read_set() function
- [ ] validate_write_set() function
- [ ] validate_cas_set() function
- [ ] validate_transaction() orchestrator
- [ ] All unit tests pass (>95% coverage)

### Component Breakdown
- **Story #83**: Conflict Detection Infrastructure 🔴 BLOCKS ALL Epic 7
- **Story #84**: Read-Set Validation
- **Story #85**: Write-Set Validation
- **Story #86**: CAS Validation
- **Story #87**: Full Transaction Validation

---

## Dependency Graph

```
Phase 1 (Sequential - CRITICAL):
  Story #83 (Conflict Detection Infrastructure)
    └─> 🔴 BLOCKS #84, #85, #86

Phase 2 (Parallel - 3 Claudes after #83):
  Story #84 (Read-Set Validation)
  Story #85 (Write-Set Validation)
  Story #86 (CAS Validation)
    └─> All depend on #83
    └─> Independent of each other

Phase 3 (Sequential - after #84, #85, #86):
  Story #87 (Full Transaction Validation)
    └─> Depends on all validation functions
```

---

## Parallelization Strategy

### Optimal Parallel Execution (3 Claudes)

| Phase | Duration | Claude 1 | Claude 2 | Claude 3 |
|-------|----------|----------|----------|----------|
| 1 | 3 hours | #83 Conflict Infrastructure | - | - |
| 2 | 4 hours | #84 Read-Set | #85 Write-Set | #86 CAS |
| 3 | 4 hours | #87 Full Validation | - | - |

**Total Wall Time**: ~11 hours (vs. ~18 hours sequential)

---

## Story #83: Conflict Detection Infrastructure

**GitHub Issue**: #83
**Estimated Time**: 3 hours
**Dependencies**: Epic 6 complete
**Blocks**: Stories #84, #85, #86, #87

### ⚠️ PREREQUISITE: Read the Semantics Spec

Before writing ANY code, read Section 3 of `docs/architecture/M2_TRANSACTION_SEMANTICS.md`:

- Section 3.1: When a Transaction ABORTS
- Section 3.2: When a Transaction DOES NOT Conflict
- Section 3.3: First-Committer-Wins Explained
- Section 3.4: CAS Interaction with Read/Write Validation

### Semantics This Story Must Implement

From the spec Section 3:

| Conflict Type | When It Occurs |
|---------------|----------------|
| ReadWriteConflict | T1 read key K at version V, but current version is V' != V |
| WriteWriteConflict | NOT actually a separate type - handled by read-set validation |
| CASConflict | CAS expected_version != current_version |

**IMPORTANT**: Write-write conflict only occurs when the key was ALSO READ. Blind writes (write without read) do NOT conflict.

### Start Story

```bash
./scripts/start-story.sh 7 83 conflict-infrastructure
```

### Implementation Steps

#### Step 1: Create validation module

Create `crates/concurrency/src/validation.rs`:

```rust
//! Transaction validation for OCC
//!
//! This module implements conflict detection per Section 3 of
//! `docs/architecture/M2_TRANSACTION_SEMANTICS.md`.
//!
//! Key rules from the spec:
//! - First-committer-wins based on READ-SET, not write-set
//! - Blind writes (write without read) do NOT conflict
//! - CAS is validated separately from read-set
//! - Write skew is ALLOWED (do not try to prevent it)

use in_mem_core::types::Key;

/// Types of conflicts that can occur during transaction validation
///
/// See spec Section 3.1 for when each conflict type occurs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictType {
    /// Read-write conflict: key was read at one version but current version differs
    ///
    /// From spec Section 3.1 Condition 1:
    /// "T1 read key K and recorded version V in its read_set.
    ///  At commit time, the current storage version of K is V' where V' != V"
    ReadWriteConflict {
        /// The key that has a conflict
        key: Key,
        /// Version recorded in read_set when read
        read_version: u64,
        /// Current version in storage at validation time
        current_version: u64,
    },

    /// CAS conflict: expected version doesn't match current version
    ///
    /// From spec Section 3.1 Condition 3:
    /// "T1 called CAS(K, expected_version=V, new_value).
    ///  At commit time, current storage version of K != V"
    CASConflict {
        /// The key that has a CAS conflict
        key: Key,
        /// Expected version specified in CAS operation
        expected_version: u64,
        /// Current version in storage at validation time
        current_version: u64,
    },
}

/// Result of transaction validation
///
/// Accumulates all conflicts found during validation.
/// A transaction commits only if is_valid() returns true.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// All conflicts detected during validation
    pub conflicts: Vec<ConflictType>,
}

impl ValidationResult {
    /// Create a successful validation result (no conflicts)
    pub fn ok() -> Self {
        ValidationResult {
            conflicts: Vec::new(),
        }
    }

    /// Create a validation result with a single conflict
    pub fn conflict(conflict: ConflictType) -> Self {
        ValidationResult {
            conflicts: vec![conflict],
        }
    }

    /// Check if validation passed (no conflicts)
    pub fn is_valid(&self) -> bool {
        self.conflicts.is_empty()
    }

    /// Merge another validation result into this one
    ///
    /// Used to combine results from different validation phases.
    pub fn merge(&mut self, other: ValidationResult) {
        self.conflicts.extend(other.conflicts);
    }

    /// Get the number of conflicts
    pub fn conflict_count(&self) -> usize {
        self.conflicts.len()
    }
}
```

#### Step 2: Export from lib.rs

Update `crates/concurrency/src/lib.rs`:

```rust
pub mod validation;

pub use validation::{ConflictType, ValidationResult};
```

#### Step 3: Write unit tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use in_mem_core::types::{Key, Namespace, RunId, TypeTag};

    fn create_test_key(name: &[u8]) -> Key {
        let ns = Namespace::new(
            "test".into(),
            "app".into(),
            "agent".into(),
            RunId::new(),
        );
        Key::new(ns, TypeTag::KV, name.to_vec())
    }

    // === ValidationResult Tests ===

    #[test]
    fn test_validation_result_ok() {
        let result = ValidationResult::ok();
        assert!(result.is_valid());
        assert_eq!(result.conflict_count(), 0);
    }

    #[test]
    fn test_validation_result_conflict() {
        let key = create_test_key(b"test_key");
        let conflict = ConflictType::ReadWriteConflict {
            key: key.clone(),
            read_version: 10,
            current_version: 20,
        };
        let result = ValidationResult::conflict(conflict);

        assert!(!result.is_valid());
        assert_eq!(result.conflict_count(), 1);
    }

    #[test]
    fn test_validation_result_merge() {
        let key1 = create_test_key(b"key1");
        let key2 = create_test_key(b"key2");

        let mut result1 = ValidationResult::conflict(ConflictType::ReadWriteConflict {
            key: key1,
            read_version: 10,
            current_version: 20,
        });
        let result2 = ValidationResult::conflict(ConflictType::CASConflict {
            key: key2,
            expected_version: 5,
            current_version: 10,
        });

        result1.merge(result2);

        assert_eq!(result1.conflict_count(), 2);
        assert!(!result1.is_valid());
    }

    #[test]
    fn test_validation_result_merge_ok_with_ok() {
        let mut result1 = ValidationResult::ok();
        let result2 = ValidationResult::ok();

        result1.merge(result2);

        assert!(result1.is_valid());
        assert_eq!(result1.conflict_count(), 0);
    }

    #[test]
    fn test_validation_result_merge_ok_with_conflict() {
        let key = create_test_key(b"key");
        let mut result1 = ValidationResult::ok();
        let result2 = ValidationResult::conflict(ConflictType::CASConflict {
            key,
            expected_version: 0,
            current_version: 5,
        });

        result1.merge(result2);

        assert!(!result1.is_valid());
        assert_eq!(result1.conflict_count(), 1);
    }

    // === ConflictType Tests ===

    #[test]
    fn test_read_write_conflict_creation() {
        let key = create_test_key(b"test");
        let conflict = ConflictType::ReadWriteConflict {
            key: key.clone(),
            read_version: 100,
            current_version: 105,
        };

        match conflict {
            ConflictType::ReadWriteConflict { key: k, read_version, current_version } => {
                assert_eq!(k, key);
                assert_eq!(read_version, 100);
                assert_eq!(current_version, 105);
            }
            _ => panic!("Wrong conflict type"),
        }
    }

    #[test]
    fn test_cas_conflict_creation() {
        let key = create_test_key(b"counter");
        let conflict = ConflictType::CASConflict {
            key: key.clone(),
            expected_version: 0,
            current_version: 1,
        };

        match conflict {
            ConflictType::CASConflict { key: k, expected_version, current_version } => {
                assert_eq!(k, key);
                assert_eq!(expected_version, 0);
                assert_eq!(current_version, 1);
            }
            _ => panic!("Wrong conflict type"),
        }
    }

    #[test]
    fn test_conflict_type_equality() {
        let key1 = create_test_key(b"key1");
        let key2 = create_test_key(b"key1"); // Same content

        let conflict1 = ConflictType::ReadWriteConflict {
            key: key1,
            read_version: 10,
            current_version: 20,
        };
        let conflict2 = ConflictType::ReadWriteConflict {
            key: key2,
            read_version: 10,
            current_version: 20,
        };

        assert_eq!(conflict1, conflict2);
    }

    #[test]
    fn test_conflict_type_debug() {
        let key = create_test_key(b"key");
        let conflict = ConflictType::CASConflict {
            key,
            expected_version: 5,
            current_version: 10,
        };

        let debug_str = format!("{:?}", conflict);
        assert!(debug_str.contains("CASConflict"));
        assert!(debug_str.contains("expected_version: 5"));
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
./scripts/complete-story.sh 83
```

---

## Story #84: Read-Set Validation

**GitHub Issue**: #84
**Estimated Time**: 4 hours
**Dependencies**: Story #83 (Conflict Detection Infrastructure)
**Blocks**: Story #87

### ⚠️ PREREQUISITE: Read the Semantics Spec

Before writing ANY code, read Section 3.1 (Condition 1) of `docs/architecture/M2_TRANSACTION_SEMANTICS.md`:

**Read-Write Conflict Definition**:
```
- T1 read key K and recorded version V in its read_set
- At commit time, the current storage version of K is V' where V' != V
- Result: T1 ABORTS
```

### Semantics This Story Must Implement

| Scenario | Expected Result |
|----------|-----------------|
| Key version unchanged | OK (no conflict) |
| Key version changed | ReadWriteConflict |
| Key was deleted (current version 0) | ReadWriteConflict |
| Key was created after read (read version 0, current > 0) | ReadWriteConflict |
| Empty read_set | OK (nothing to validate) |

**CRITICAL**: A read-set entry with version 0 means "key did not exist when read". If the key now exists, that's a conflict.

### Start Story

```bash
./scripts/start-story.sh 7 84 read-set-validation
```

### Implementation Steps

#### Step 1: Add validate_read_set function

Add to `crates/concurrency/src/validation.rs`:

```rust
use in_mem_core::traits::Storage;
use std::collections::HashMap;

/// Validate the read-set against current storage state
///
/// Per spec Section 3.1 Condition 1:
/// - For each key in read_set, check if current version matches read version
/// - If any version changed, report ReadWriteConflict
///
/// # Arguments
/// * `read_set` - Keys read with their versions at read time
/// * `store` - Storage to check current versions against
///
/// # Returns
/// ValidationResult with any ReadWriteConflicts found
pub fn validate_read_set<S: Storage>(
    read_set: &HashMap<Key, u64>,
    store: &S,
) -> ValidationResult {
    let mut result = ValidationResult::ok();

    for (key, read_version) in read_set {
        // Get current version from storage
        let current_version = match store.get_versioned(key) {
            Ok(Some(vv)) => vv.version,
            Ok(None) => 0, // Key doesn't exist = version 0
            Err(_) => {
                // Storage error - treat as version 0 (conservative)
                0
            }
        };

        // Check if version changed
        if current_version != *read_version {
            result.conflicts.push(ConflictType::ReadWriteConflict {
                key: key.clone(),
                read_version: *read_version,
                current_version,
            });
        }
    }

    result
}
```

#### Step 2: Write tests for read-set validation

```rust
#[cfg(test)]
mod read_set_tests {
    use super::*;
    use in_mem_storage::UnifiedStore;
    use in_mem_core::value::Value;
    use std::sync::Arc;
    use parking_lot::RwLock;

    fn create_test_store() -> UnifiedStore {
        UnifiedStore::new()
    }

    fn create_test_namespace() -> Namespace {
        Namespace::new("t".into(), "a".into(), "g".into(), RunId::new())
    }

    fn create_key(ns: &Namespace, name: &[u8]) -> Key {
        Key::new(ns.clone(), TypeTag::KV, name.to_vec())
    }

    #[test]
    fn test_validate_read_set_empty() {
        let store = create_test_store();
        let read_set: HashMap<Key, u64> = HashMap::new();

        let result = validate_read_set(&read_set, &store);

        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_read_set_version_unchanged() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key1");

        // Put key at version
        store.put(key.clone(), Value::Bytes(b"value".to_vec()), None).unwrap();
        let current_version = store.get_versioned(&key).unwrap().unwrap().version;

        // Read-set records the same version
        let mut read_set = HashMap::new();
        read_set.insert(key.clone(), current_version);

        let result = validate_read_set(&read_set, &store);

        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_read_set_version_changed() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key1");

        // Put key at version 1
        store.put(key.clone(), Value::Bytes(b"v1".to_vec()), None).unwrap();
        let v1 = store.get_versioned(&key).unwrap().unwrap().version;

        // Another transaction modified it (version 2)
        store.put(key.clone(), Value::Bytes(b"v2".to_vec()), None).unwrap();

        // Read-set still has old version
        let mut read_set = HashMap::new();
        read_set.insert(key.clone(), v1);

        let result = validate_read_set(&read_set, &store);

        assert!(!result.is_valid());
        assert_eq!(result.conflict_count(), 1);
        match &result.conflicts[0] {
            ConflictType::ReadWriteConflict { key: k, read_version, current_version } => {
                assert_eq!(k, &key);
                assert_eq!(*read_version, v1);
                assert!(*current_version > v1);
            }
            _ => panic!("Expected ReadWriteConflict"),
        }
    }

    #[test]
    fn test_validate_read_set_key_deleted() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key1");

        // Put then delete
        store.put(key.clone(), Value::Bytes(b"value".to_vec()), None).unwrap();
        let version_when_read = store.get_versioned(&key).unwrap().unwrap().version;
        store.delete(&key).unwrap();

        // Read-set has version from when key existed
        let mut read_set = HashMap::new();
        read_set.insert(key.clone(), version_when_read);

        let result = validate_read_set(&read_set, &store);

        assert!(!result.is_valid());
        match &result.conflicts[0] {
            ConflictType::ReadWriteConflict { current_version, .. } => {
                // Deleted key has version 0 (or higher if tombstone versioned)
                assert_ne!(*current_version, version_when_read);
            }
            _ => panic!("Expected ReadWriteConflict"),
        }
    }

    #[test]
    fn test_validate_read_set_key_created_after_read() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key1");

        // Read-set recorded key as non-existent (version 0)
        let mut read_set = HashMap::new();
        read_set.insert(key.clone(), 0);

        // Another transaction created the key
        store.put(key.clone(), Value::Bytes(b"value".to_vec()), None).unwrap();

        let result = validate_read_set(&read_set, &store);

        assert!(!result.is_valid());
        match &result.conflicts[0] {
            ConflictType::ReadWriteConflict { read_version, current_version, .. } => {
                assert_eq!(*read_version, 0);
                assert!(*current_version > 0);
            }
            _ => panic!("Expected ReadWriteConflict"),
        }
    }

    #[test]
    fn test_validate_read_set_multiple_conflicts() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key1 = create_key(&ns, b"key1");
        let key2 = create_key(&ns, b"key2");

        // Put both keys
        store.put(key1.clone(), Value::Bytes(b"v1".to_vec()), None).unwrap();
        store.put(key2.clone(), Value::Bytes(b"v1".to_vec()), None).unwrap();
        let v1_1 = store.get_versioned(&key1).unwrap().unwrap().version;
        let v1_2 = store.get_versioned(&key2).unwrap().unwrap().version;

        // Both keys modified
        store.put(key1.clone(), Value::Bytes(b"v2".to_vec()), None).unwrap();
        store.put(key2.clone(), Value::Bytes(b"v2".to_vec()), None).unwrap();

        // Read-set has old versions
        let mut read_set = HashMap::new();
        read_set.insert(key1.clone(), v1_1);
        read_set.insert(key2.clone(), v1_2);

        let result = validate_read_set(&read_set, &store);

        assert!(!result.is_valid());
        assert_eq!(result.conflict_count(), 2);
    }

    #[test]
    fn test_validate_read_set_partial_conflict() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key1 = create_key(&ns, b"key1");
        let key2 = create_key(&ns, b"key2");

        // Put both keys
        store.put(key1.clone(), Value::Bytes(b"v1".to_vec()), None).unwrap();
        store.put(key2.clone(), Value::Bytes(b"v1".to_vec()), None).unwrap();
        let v1_1 = store.get_versioned(&key1).unwrap().unwrap().version;
        let v1_2 = store.get_versioned(&key2).unwrap().unwrap().version;

        // Only key1 modified
        store.put(key1.clone(), Value::Bytes(b"v2".to_vec()), None).unwrap();

        // Read-set has old versions for both
        let mut read_set = HashMap::new();
        read_set.insert(key1.clone(), v1_1);
        read_set.insert(key2.clone(), v1_2);

        let result = validate_read_set(&read_set, &store);

        // Only one conflict (key1)
        assert!(!result.is_valid());
        assert_eq!(result.conflict_count(), 1);
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
./scripts/complete-story.sh 84
```

---

## Story #85: Write-Set Validation

**GitHub Issue**: #85
**Estimated Time**: 3 hours
**Dependencies**: Story #83 (Conflict Detection Infrastructure)
**Blocks**: Story #87

### ⚠️ PREREQUISITE: Read the Semantics Spec

From the spec Section 3.2 (Scenario 1 - Blind Write):

**CRITICAL**: Blind writes (write without reading first) do NOT conflict!

```
Definition:
  - T1 writes key K without ever reading it first
  - T2 also writes key K and commits first

Result: T1 COMMITS successfully (overwrites T2's value)

Why no conflict: Neither transaction read key_a, so neither has it in their
read_set. Write-write conflict only applies when the key was also read.
```

**Important**: First-committer-wins is based on the READ-SET, not the write-set.

### Semantics This Story Must Implement

| Scenario | Expected Result |
|----------|-----------------|
| Blind write (key not in read_set) | OK - always succeeds |
| Key in both read_set and write_set | Use read-set validation |
| Write to non-existent key | OK - no conflict |

**This function should always return OK** because write-write conflicts are detected by the read-set validation when the key was also read. There is no separate "write-write conflict" in pure OCC with snapshot isolation.

### Start Story

```bash
./scripts/start-story.sh 7 85 write-set-validation
```

### Implementation Steps

#### Step 1: Add validate_write_set function

Add to `crates/concurrency/src/validation.rs`:

```rust
/// Validate the write-set against current storage state
///
/// Per spec Section 3.2 Scenario 1 (Blind Write):
/// - Blind writes (write without read) do NOT conflict
/// - First-committer-wins is based on READ-SET, not write-set
///
/// This function always returns OK because:
/// - If key was read → conflict detected by validate_read_set()
/// - If key was NOT read (blind write) → no conflict
///
/// # Arguments
/// * `write_set` - Keys to be written
/// * `read_set` - Keys that were read (for context)
/// * `start_version` - Transaction's start version
/// * `store` - Storage to check
///
/// # Returns
/// ValidationResult (always valid for pure blind writes)
pub fn validate_write_set<S: Storage>(
    write_set: &HashMap<Key, in_mem_core::value::Value>,
    _read_set: &HashMap<Key, u64>,
    _start_version: u64,
    _store: &S,
) -> ValidationResult {
    // Per spec: Blind writes do NOT conflict
    // Write-write conflict is only detected when the key was ALSO READ
    // That case is handled by validate_read_set()
    //
    // From spec Section 3.2:
    // "First-committer-wins is based on the READ-SET, not the write-set."

    // Note: We could add optional write-write conflict detection here
    // for keys in BOTH write_set AND read_set, but that's redundant
    // with read-set validation. Keeping this simple per spec.

    let _ = write_set; // Acknowledge parameter (used for type checking)

    ValidationResult::ok()
}
```

#### Step 2: Write tests proving blind writes don't conflict

```rust
#[cfg(test)]
mod write_set_tests {
    use super::*;
    use in_mem_storage::UnifiedStore;
    use in_mem_core::value::Value;

    fn create_test_store() -> UnifiedStore {
        UnifiedStore::new()
    }

    fn create_test_namespace() -> Namespace {
        Namespace::new("t".into(), "a".into(), "g".into(), RunId::new())
    }

    fn create_key(ns: &Namespace, name: &[u8]) -> Key {
        Key::new(ns.clone(), TypeTag::KV, name.to_vec())
    }

    #[test]
    fn test_validate_write_set_empty() {
        let store = create_test_store();
        let write_set: HashMap<Key, Value> = HashMap::new();
        let read_set: HashMap<Key, u64> = HashMap::new();

        let result = validate_write_set(&write_set, &read_set, 100, &store);

        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_write_set_blind_write_no_conflict() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key1");

        // Put initial value
        store.put(key.clone(), Value::Bytes(b"initial".to_vec()), None).unwrap();
        let start_version = store.current_version();

        // Another transaction modified the key
        store.put(key.clone(), Value::Bytes(b"concurrent".to_vec()), None).unwrap();

        // Our write_set has the key (blind write - not in read_set)
        let mut write_set = HashMap::new();
        write_set.insert(key.clone(), Value::Bytes(b"our_write".to_vec()));
        let read_set: HashMap<Key, u64> = HashMap::new(); // Empty - blind write

        // Per spec: Blind writes do NOT conflict
        let result = validate_write_set(&write_set, &read_set, start_version, &store);

        assert!(result.is_valid(), "Blind writes should not conflict");
    }

    #[test]
    fn test_validate_write_set_multiple_blind_writes() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key1 = create_key(&ns, b"key1");
        let key2 = create_key(&ns, b"key2");

        // Put initial values
        store.put(key1.clone(), Value::Bytes(b"v1".to_vec()), None).unwrap();
        store.put(key2.clone(), Value::Bytes(b"v1".to_vec()), None).unwrap();
        let start_version = store.current_version();

        // Both modified by concurrent transaction
        store.put(key1.clone(), Value::Bytes(b"v2".to_vec()), None).unwrap();
        store.put(key2.clone(), Value::Bytes(b"v2".to_vec()), None).unwrap();

        // Blind writes to both
        let mut write_set = HashMap::new();
        write_set.insert(key1, Value::Bytes(b"our1".to_vec()));
        write_set.insert(key2, Value::Bytes(b"our2".to_vec()));
        let read_set: HashMap<Key, u64> = HashMap::new();

        let result = validate_write_set(&write_set, &read_set, start_version, &store);

        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_write_set_to_new_key() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"new_key");

        let mut write_set = HashMap::new();
        write_set.insert(key, Value::Bytes(b"new_value".to_vec()));
        let read_set: HashMap<Key, u64> = HashMap::new();

        let result = validate_write_set(&write_set, &read_set, 100, &store);

        assert!(result.is_valid());
    }

    /// This test documents that write-set validation alone doesn't detect conflicts.
    /// The read-set validation is what catches write-write conflicts on read keys.
    #[test]
    fn test_write_set_validation_does_not_detect_read_key_conflicts() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key1");

        store.put(key.clone(), Value::Bytes(b"initial".to_vec()), None).unwrap();
        let read_version = store.get_versioned(&key).unwrap().unwrap().version;
        let start_version = store.current_version();

        // Key modified by concurrent transaction
        store.put(key.clone(), Value::Bytes(b"concurrent".to_vec()), None).unwrap();

        // Key in BOTH read_set AND write_set
        let mut write_set = HashMap::new();
        write_set.insert(key.clone(), Value::Bytes(b"our_write".to_vec()));
        let mut read_set = HashMap::new();
        read_set.insert(key.clone(), read_version);

        // Write-set validation still returns OK
        let write_result = validate_write_set(&write_set, &read_set, start_version, &store);
        assert!(write_result.is_valid());

        // But read-set validation catches the conflict
        let read_result = validate_read_set(&read_set, &store);
        assert!(!read_result.is_valid());
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
./scripts/complete-story.sh 85
```

---

## Story #86: CAS Validation

**GitHub Issue**: #86
**Estimated Time**: 4 hours
**Dependencies**: Story #83 (Conflict Detection Infrastructure)
**Blocks**: Story #87

### ⚠️ PREREQUISITE: Read the Semantics Spec

From the spec Section 3.1 (Condition 3):

```
Definition:
  - T1 called CAS(K, expected_version=V, new_value)
  - At commit time, current storage version of K != V

Result: T1 ABORTS
```

From the spec Section 3.4:

**CRITICAL**: CAS does NOT auto-add to read_set!

| Operation | Read-Set Entry? | CAS Validation |
|-----------|-----------------|----------------|
| txn.get(key) | Yes | N/A |
| txn.cas(key, version, value) | **NO** | Checks expected_version |

### Semantics This Story Must Implement

| Scenario | Expected Result |
|----------|-----------------|
| expected_version matches current_version | OK |
| expected_version != current_version | CASConflict |
| expected_version = 0, key doesn't exist | OK |
| expected_version = 0, key exists | CASConflict |
| expected_version > 0, key doesn't exist | CASConflict |

### Start Story

```bash
./scripts/start-story.sh 7 86 cas-validation
```

### Implementation Steps

#### Step 1: Add validate_cas_set function

Add to `crates/concurrency/src/validation.rs`:

```rust
use crate::transaction::CASOperation;

/// Validate CAS operations against current storage state
///
/// Per spec Section 3.1 Condition 3:
/// - For each CAS op, check if current version matches expected_version
/// - If versions don't match, report CASConflict
///
/// Per spec Section 3.4:
/// - CAS does NOT add to read_set (validated separately)
/// - expected_version=0 means "key must not exist"
///
/// # Arguments
/// * `cas_set` - CAS operations to validate
/// * `store` - Storage to check current versions against
///
/// # Returns
/// ValidationResult with any CASConflicts found
pub fn validate_cas_set<S: Storage>(
    cas_set: &[CASOperation],
    store: &S,
) -> ValidationResult {
    let mut result = ValidationResult::ok();

    for cas_op in cas_set {
        // Get current version from storage
        let current_version = match store.get_versioned(&cas_op.key) {
            Ok(Some(vv)) => vv.version,
            Ok(None) => 0, // Key doesn't exist = version 0
            Err(_) => 0,   // Storage error = treat as non-existent
        };

        // Check if expected version matches
        if current_version != cas_op.expected_version {
            result.conflicts.push(ConflictType::CASConflict {
                key: cas_op.key.clone(),
                expected_version: cas_op.expected_version,
                current_version,
            });
        }
    }

    result
}
```

#### Step 2: Write tests for CAS validation

```rust
#[cfg(test)]
mod cas_tests {
    use super::*;
    use crate::CASOperation;
    use in_mem_storage::UnifiedStore;
    use in_mem_core::value::Value;

    fn create_test_store() -> UnifiedStore {
        UnifiedStore::new()
    }

    fn create_test_namespace() -> Namespace {
        Namespace::new("t".into(), "a".into(), "g".into(), RunId::new())
    }

    fn create_key(ns: &Namespace, name: &[u8]) -> Key {
        Key::new(ns.clone(), TypeTag::KV, name.to_vec())
    }

    #[test]
    fn test_validate_cas_set_empty() {
        let store = create_test_store();
        let cas_set: Vec<CASOperation> = Vec::new();

        let result = validate_cas_set(&cas_set, &store);

        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_cas_version_matches() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"counter");

        // Put key
        store.put(key.clone(), Value::I64(100), None).unwrap();
        let current_version = store.get_versioned(&key).unwrap().unwrap().version;

        // CAS with matching version
        let cas_set = vec![CASOperation {
            key: key.clone(),
            expected_version: current_version,
            new_value: Value::I64(101),
        }];

        let result = validate_cas_set(&cas_set, &store);

        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_cas_version_mismatch() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"counter");

        // Put key
        store.put(key.clone(), Value::I64(100), None).unwrap();
        let v1 = store.get_versioned(&key).unwrap().unwrap().version;

        // Concurrent transaction modifies it
        store.put(key.clone(), Value::I64(200), None).unwrap();

        // CAS with old version
        let cas_set = vec![CASOperation {
            key: key.clone(),
            expected_version: v1,
            new_value: Value::I64(101),
        }];

        let result = validate_cas_set(&cas_set, &store);

        assert!(!result.is_valid());
        assert_eq!(result.conflict_count(), 1);
        match &result.conflicts[0] {
            ConflictType::CASConflict { expected_version, current_version, .. } => {
                assert_eq!(*expected_version, v1);
                assert!(*current_version > v1);
            }
            _ => panic!("Expected CASConflict"),
        }
    }

    #[test]
    fn test_validate_cas_version_zero_key_not_exists() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"new_key");

        // CAS with expected_version=0 on non-existent key (should succeed)
        let cas_set = vec![CASOperation {
            key: key.clone(),
            expected_version: 0,
            new_value: Value::String("initial".into()),
        }];

        let result = validate_cas_set(&cas_set, &store);

        assert!(result.is_valid(), "CAS with version 0 on non-existent key should succeed");
    }

    #[test]
    fn test_validate_cas_version_zero_key_exists() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"existing_key");

        // Key exists
        store.put(key.clone(), Value::String("exists".into()), None).unwrap();

        // CAS with expected_version=0 should fail (key exists)
        let cas_set = vec![CASOperation {
            key: key.clone(),
            expected_version: 0,
            new_value: Value::String("new".into()),
        }];

        let result = validate_cas_set(&cas_set, &store);

        assert!(!result.is_valid());
        match &result.conflicts[0] {
            ConflictType::CASConflict { expected_version, current_version, .. } => {
                assert_eq!(*expected_version, 0);
                assert!(*current_version > 0);
            }
            _ => panic!("Expected CASConflict"),
        }
    }

    #[test]
    fn test_validate_cas_nonzero_version_key_not_exists() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"missing_key");

        // CAS expecting version 5 on non-existent key (should fail)
        let cas_set = vec![CASOperation {
            key: key.clone(),
            expected_version: 5,
            new_value: Value::I64(10),
        }];

        let result = validate_cas_set(&cas_set, &store);

        assert!(!result.is_valid());
        match &result.conflicts[0] {
            ConflictType::CASConflict { expected_version, current_version, .. } => {
                assert_eq!(*expected_version, 5);
                assert_eq!(*current_version, 0); // Key doesn't exist
            }
            _ => panic!("Expected CASConflict"),
        }
    }

    #[test]
    fn test_validate_cas_multiple_operations() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key1 = create_key(&ns, b"key1");
        let key2 = create_key(&ns, b"key2");

        store.put(key1.clone(), Value::I64(1), None).unwrap();
        store.put(key2.clone(), Value::I64(2), None).unwrap();
        let v1 = store.get_versioned(&key1).unwrap().unwrap().version;
        let v2 = store.get_versioned(&key2).unwrap().unwrap().version;

        let cas_set = vec![
            CASOperation {
                key: key1.clone(),
                expected_version: v1,
                new_value: Value::I64(10),
            },
            CASOperation {
                key: key2.clone(),
                expected_version: v2,
                new_value: Value::I64(20),
            },
        ];

        let result = validate_cas_set(&cas_set, &store);

        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_cas_multiple_partial_conflict() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key1 = create_key(&ns, b"key1");
        let key2 = create_key(&ns, b"key2");

        store.put(key1.clone(), Value::I64(1), None).unwrap();
        store.put(key2.clone(), Value::I64(2), None).unwrap();
        let v1 = store.get_versioned(&key1).unwrap().unwrap().version;
        let v2 = store.get_versioned(&key2).unwrap().unwrap().version;

        // Modify only key1
        store.put(key1.clone(), Value::I64(10), None).unwrap();

        let cas_set = vec![
            CASOperation {
                key: key1.clone(),
                expected_version: v1, // Old version - will conflict
                new_value: Value::I64(100),
            },
            CASOperation {
                key: key2.clone(),
                expected_version: v2, // Current version - OK
                new_value: Value::I64(200),
            },
        ];

        let result = validate_cas_set(&cas_set, &store);

        assert!(!result.is_valid());
        assert_eq!(result.conflict_count(), 1); // Only key1 conflicts
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
./scripts/complete-story.sh 86
```

---

## Story #87: Full Transaction Validation

**GitHub Issue**: #87
**Estimated Time**: 4 hours
**Dependencies**: Stories #84, #85, #86
**Blocks**: Epic 8

### ⚠️ PREREQUISITE: Read the Semantics Spec

Full transaction validation orchestrates all validation phases in order:
1. Read-set validation (detects read-write conflicts)
2. Write-set validation (currently no-op per spec - blind writes OK)
3. CAS validation (detects CAS conflicts)

### Semantics This Story Must Implement

| Phase | What it Checks | Conflict Type |
|-------|----------------|---------------|
| 1 | Read versions unchanged | ReadWriteConflict |
| 2 | (No-op per spec) | N/A |
| 3 | CAS versions match | CASConflict |

All conflicts are accumulated and returned together.

### Start Story

```bash
./scripts/start-story.sh 7 87 full-validation
```

### Implementation Steps

#### Step 1: Add validate_transaction function

Add to `crates/concurrency/src/validation.rs`:

```rust
use crate::TransactionContext;

/// Validate a transaction for conflicts before commit
///
/// Orchestrates all validation phases per spec Section 3:
/// 1. Validate read-set (read-write conflicts)
/// 2. Validate write-set (blind writes always OK)
/// 3. Validate CAS set (CAS conflicts)
///
/// All conflicts are accumulated and returned together.
///
/// # Arguments
/// * `txn` - Transaction context with read/write/cas sets
/// * `store` - Storage to validate against
///
/// # Returns
/// ValidationResult with all conflicts found (empty if valid)
///
/// # Note
/// Transaction must be in Validating state before calling this.
pub fn validate_transaction<S: Storage>(
    txn: &TransactionContext,
    store: &S,
) -> ValidationResult {
    let mut result = ValidationResult::ok();

    // Phase 1: Read-set validation
    let read_result = validate_read_set(&txn.read_set, store);
    result.merge(read_result);

    // Phase 2: Write-set validation (no-op per spec)
    let write_result = validate_write_set(
        &txn.write_set,
        &txn.read_set,
        txn.start_version,
        store,
    );
    result.merge(write_result);

    // Phase 3: CAS validation
    let cas_result = validate_cas_set(&txn.cas_set, store);
    result.merge(cas_result);

    result
}
```

#### Step 2: Write integration tests

```rust
#[cfg(test)]
mod transaction_validation_tests {
    use super::*;
    use crate::{ClonedSnapshotView, TransactionContext};
    use in_mem_storage::UnifiedStore;
    use in_mem_core::value::Value;
    use std::collections::BTreeMap;

    fn create_test_store() -> UnifiedStore {
        UnifiedStore::new()
    }

    fn create_test_namespace() -> Namespace {
        Namespace::new("t".into(), "a".into(), "g".into(), RunId::new())
    }

    fn create_key(ns: &Namespace, name: &[u8]) -> Key {
        Key::new(ns.clone(), TypeTag::KV, name.to_vec())
    }

    fn create_txn_with_snapshot(store: &UnifiedStore) -> TransactionContext {
        let snapshot = store.create_snapshot();
        let run_id = RunId::new();
        TransactionContext::with_snapshot(1, run_id, Box::new(snapshot))
    }

    #[test]
    fn test_validate_transaction_empty() {
        let store = create_test_store();
        let txn = create_txn_with_snapshot(&store);

        let result = validate_transaction(&txn, &store);

        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_transaction_read_only_always_valid() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key");

        store.put(key.clone(), Value::I64(100), None).unwrap();

        let mut txn = create_txn_with_snapshot(&store);
        let _ = txn.get(&key).unwrap(); // Read the key

        // Another transaction modifies the key
        store.put(key.clone(), Value::I64(200), None).unwrap();

        // Read-only transactions DON'T always succeed if read-set changed!
        // Per spec: read-write conflict occurs
        let result = validate_transaction(&txn, &store);

        // This is NOT valid because the read key changed
        assert!(!result.is_valid());
    }

    #[test]
    fn test_validate_transaction_read_only_version_unchanged() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key");

        store.put(key.clone(), Value::I64(100), None).unwrap();

        let mut txn = create_txn_with_snapshot(&store);
        let _ = txn.get(&key).unwrap(); // Read the key

        // No concurrent modifications

        let result = validate_transaction(&txn, &store);

        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_transaction_read_write_conflict() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key");

        store.put(key.clone(), Value::I64(100), None).unwrap();

        let mut txn = create_txn_with_snapshot(&store);
        let _ = txn.get(&key).unwrap(); // Read
        txn.put(key.clone(), Value::I64(101)).unwrap(); // Write

        // Concurrent modification
        store.put(key.clone(), Value::I64(200), None).unwrap();

        let result = validate_transaction(&txn, &store);

        assert!(!result.is_valid());
        assert!(result.conflicts.iter().any(|c| matches!(c, ConflictType::ReadWriteConflict { .. })));
    }

    #[test]
    fn test_validate_transaction_blind_write_no_conflict() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key");

        store.put(key.clone(), Value::I64(100), None).unwrap();

        let mut txn = create_txn_with_snapshot(&store);
        // Blind write - no read first
        txn.put(key.clone(), Value::I64(101)).unwrap();

        // Concurrent modification
        store.put(key.clone(), Value::I64(200), None).unwrap();

        // Per spec: blind writes do NOT conflict
        let result = validate_transaction(&txn, &store);

        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_transaction_cas_conflict() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"counter");

        store.put(key.clone(), Value::I64(0), None).unwrap();
        let v1 = store.get_versioned(&key).unwrap().unwrap().version;

        let mut txn = create_txn_with_snapshot(&store);
        txn.cas(key.clone(), v1, Value::I64(1)).unwrap();

        // Concurrent modification
        store.put(key.clone(), Value::I64(100), None).unwrap();

        let result = validate_transaction(&txn, &store);

        assert!(!result.is_valid());
        assert!(result.conflicts.iter().any(|c| matches!(c, ConflictType::CASConflict { .. })));
    }

    #[test]
    fn test_validate_transaction_multiple_conflicts() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key1 = create_key(&ns, b"key1");
        let key2 = create_key(&ns, b"key2");

        store.put(key1.clone(), Value::I64(1), None).unwrap();
        store.put(key2.clone(), Value::I64(2), None).unwrap();
        let v2 = store.get_versioned(&key2).unwrap().unwrap().version;

        let mut txn = create_txn_with_snapshot(&store);
        let _ = txn.get(&key1).unwrap(); // Read key1
        txn.cas(key2.clone(), v2, Value::I64(20)).unwrap(); // CAS key2

        // Concurrent modifications to both
        store.put(key1.clone(), Value::I64(10), None).unwrap();
        store.put(key2.clone(), Value::I64(20), None).unwrap();

        let result = validate_transaction(&txn, &store);

        assert!(!result.is_valid());
        assert_eq!(result.conflict_count(), 2); // Both read-write and CAS conflict
    }

    #[test]
    fn test_validate_transaction_first_committer_wins() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"shared");

        store.put(key.clone(), Value::String("initial".into()), None).unwrap();

        // T1 reads and writes
        let mut txn1 = create_txn_with_snapshot(&store);
        let _ = txn1.get(&key).unwrap();
        txn1.put(key.clone(), Value::String("from_t1".into())).unwrap();

        // T2 reads and writes (same key)
        let mut txn2 = create_txn_with_snapshot(&store);
        let _ = txn2.get(&key).unwrap();
        txn2.put(key.clone(), Value::String("from_t2".into())).unwrap();

        // T1 commits first (simulated by applying to store)
        // In real implementation, this would be atomic
        let result1 = validate_transaction(&txn1, &store);
        assert!(result1.is_valid(), "T1 should commit (first committer)");

        // Apply T1's write
        store.put(key.clone(), Value::String("from_t1".into()), None).unwrap();

        // T2 tries to commit - should fail (read version changed)
        let result2 = validate_transaction(&txn2, &store);
        assert!(!result2.is_valid(), "T2 should abort (read key was modified)");
    }

    #[test]
    fn test_validate_transaction_cas_without_read_protection() {
        let store = create_test_store();
        let ns = create_test_namespace();
        let key = create_key(&ns, b"key");

        store.put(key.clone(), Value::I64(1), None).unwrap();
        let v1 = store.get_versioned(&key).unwrap().unwrap().version;

        let mut txn = create_txn_with_snapshot(&store);
        // CAS without reading first - per spec, CAS does NOT add to read_set
        txn.cas(key.clone(), v1, Value::I64(2)).unwrap();

        // Verify read_set is empty (CAS doesn't add to it)
        assert!(txn.read_set.is_empty());

        // No concurrent modification - CAS should succeed
        let result = validate_transaction(&txn, &store);
        assert!(result.is_valid());
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
~/.cargo/bin/cargo fmt --check
```

### Completion

```bash
./scripts/complete-story.sh 87
```

---

## Epic Completion

After all stories are merged to `epic-7-transaction-semantics`:

### Final Validation

```bash
git checkout epic-7-transaction-semantics
~/.cargo/bin/cargo test --all
~/.cargo/bin/cargo clippy --all -- -D warnings
~/.cargo/bin/cargo fmt --check
```

### Merge to develop

```bash
git checkout develop
git merge --no-ff epic-7-transaction-semantics -m "Epic 7: Transaction Semantics Complete

Implements Epic 7 (Stories #83-#87):
- #83: Conflict Detection Infrastructure
- #84: Read-Set Validation
- #85: Write-Set Validation
- #86: CAS Validation
- #87: Full Transaction Validation

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
git push origin develop
```

### Update Status

Update `docs/milestones/M2_PROJECT_STATUS.md`:
- Mark Epic 7 as complete
- Update progress: 2/7 epics complete, 10/32 stories complete
- Update "Next Steps" to point to Epic 8

---

## Critical Notes

### 🔴 SPEC COMPLIANCE IS MANDATORY

**Every line of M2 code must comply with `docs/architecture/M2_TRANSACTION_SEMANTICS.md`.**

During code review, verify:
- [ ] First-committer-wins based on READ-SET, not write-set
- [ ] Blind writes (write without read) do NOT conflict
- [ ] CAS does NOT auto-add to read_set
- [ ] Version 0 means "key did not exist"
- [ ] Write skew is ALLOWED (do not try to prevent it)
- [ ] All conflicts are accumulated (not early-exit)

**If ANY behavior deviates from the spec, the code MUST be rejected.**

### Key Spec Rules for Epic 7

From Section 3 of the spec:

1. **Read-Write Conflict**: Key read at version V, current version V' != V → ABORT
2. **Blind Writes OK**: Write without read → NO conflict
3. **CAS Conflict**: expected_version != current_version → ABORT
4. **Version 0**: Means "key never existed" (not deleted)
5. **First-Committer-Wins**: Based on read-set, not write-set

### Architecture

Epic 7 adds to `crates/concurrency`:
- `validation.rs`: ConflictType, ValidationResult, validation functions
- Exports: validate_read_set, validate_write_set, validate_cas_set, validate_transaction

### Summary

Epic 7 establishes conflict detection for M2:
- ConflictType enum for ReadWriteConflict and CASConflict
- ValidationResult for accumulating conflicts
- validate_read_set() - detects when read keys changed
- validate_write_set() - no-op (blind writes OK per spec)
- validate_cas_set() - detects CAS version mismatches
- validate_transaction() - orchestrates all phases

**After Epic 7**: Validation infrastructure is complete. Ready for Epic 8 (Durability & Commit).
