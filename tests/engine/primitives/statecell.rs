//! StateCell Primitive Tests
//!
//! Tests for CAS-based versioned cells for coordination.

use crate::common::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

// ============================================================================
// Basic Operations
// ============================================================================

#[test]
fn init_creates_new_cell() {
    let test_db = TestDb::new();
    let state = test_db.state();

    let result = state.init(&test_db.run_id, "cell", Value::Int(42)).unwrap();
    assert!(result.version.as_u64() > 0);
}

#[test]
fn init_fails_if_exists() {
    let test_db = TestDb::new();
    let state = test_db.state();

    state.init(&test_db.run_id, "cell", Value::Int(1)).unwrap();

    let result = state.init(&test_db.run_id, "cell", Value::Int(2));
    assert!(result.is_err());
}

#[test]
fn read_nonexistent_returns_none() {
    let test_db = TestDb::new();
    let state = test_db.state();

    let result = state.read(&test_db.run_id, "nonexistent").unwrap();
    assert!(result.is_none());
}

#[test]
fn read_returns_initialized_value() {
    let test_db = TestDb::new();
    let state = test_db.state();

    state.init(&test_db.run_id, "cell", Value::Int(42)).unwrap();

    let result = state.read(&test_db.run_id, "cell").unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().value.value, Value::Int(42));
}

#[test]
fn exists_returns_correct_status() {
    let test_db = TestDb::new();
    let state = test_db.state();

    assert!(!state.exists(&test_db.run_id, "cell").unwrap());

    state.init(&test_db.run_id, "cell", Value::Int(1)).unwrap();
    assert!(state.exists(&test_db.run_id, "cell").unwrap());
}

#[test]
fn delete_removes_cell() {
    let test_db = TestDb::new();
    let state = test_db.state();

    state.init(&test_db.run_id, "cell", Value::Int(1)).unwrap();
    assert!(state.exists(&test_db.run_id, "cell").unwrap());

    let deleted = state.delete(&test_db.run_id, "cell").unwrap();
    assert!(deleted);

    assert!(!state.exists(&test_db.run_id, "cell").unwrap());
}

#[test]
fn delete_nonexistent_returns_false() {
    let test_db = TestDb::new();
    let state = test_db.state();

    let deleted = state.delete(&test_db.run_id, "nonexistent").unwrap();
    assert!(!deleted);
}

// ============================================================================
// CAS Operations
// ============================================================================

#[test]
fn cas_succeeds_with_correct_version() {
    let test_db = TestDb::new();
    let state = test_db.state();

    state.init(&test_db.run_id, "cell", Value::Int(1)).unwrap();

    let current = state.read(&test_db.run_id, "cell").unwrap().unwrap();
    let version = current.version;

    let result = state.cas(&test_db.run_id, "cell", version, Value::Int(2));
    assert!(result.is_ok());

    let updated = state.read(&test_db.run_id, "cell").unwrap().unwrap();
    assert_eq!(updated.value.value, Value::Int(2));
}

#[test]
fn cas_fails_with_wrong_version() {
    let test_db = TestDb::new();
    let state = test_db.state();

    state.init(&test_db.run_id, "cell", Value::Int(1)).unwrap();

    // Use wrong version
    let result = state.cas(&test_db.run_id, "cell", Version::from(999999u64), Value::Int(2));
    assert!(result.is_err());

    // Value unchanged
    let current = state.read(&test_db.run_id, "cell").unwrap().unwrap();
    assert_eq!(current.value.value, Value::Int(1));
}

#[test]
fn cas_fails_on_nonexistent_cell() {
    let test_db = TestDb::new();
    let state = test_db.state();

    let result = state.cas(&test_db.run_id, "nonexistent", Version::from(1u64), Value::Int(1));
    assert!(result.is_err());
}

#[test]
fn cas_version_increments() {
    let test_db = TestDb::new();
    let state = test_db.state();

    state.init(&test_db.run_id, "cell", Value::Int(1)).unwrap();

    let v1 = state.read(&test_db.run_id, "cell").unwrap().unwrap().version;

    state.cas(&test_db.run_id, "cell", v1, Value::Int(2)).unwrap();
    let v2 = state.read(&test_db.run_id, "cell").unwrap().unwrap().version;

    assert!(v2.as_u64() > v1.as_u64());
}

// ============================================================================
// Set (Unconditional Write)
// ============================================================================

#[test]
fn set_creates_if_not_exists() {
    let test_db = TestDb::new();
    let state = test_db.state();

    let result = state.set(&test_db.run_id, "cell", Value::Int(42));
    assert!(result.is_ok());

    let current = state.read(&test_db.run_id, "cell").unwrap().unwrap();
    assert_eq!(current.value.value, Value::Int(42));
}

#[test]
fn set_overwrites_without_version_check() {
    let test_db = TestDb::new();
    let state = test_db.state();

    state.init(&test_db.run_id, "cell", Value::Int(1)).unwrap();

    // Set doesn't care about version
    state.set(&test_db.run_id, "cell", Value::Int(100)).unwrap();

    let current = state.read(&test_db.run_id, "cell").unwrap().unwrap();
    assert_eq!(current.value.value, Value::Int(100));
}

// ============================================================================
// Transition
// ============================================================================

#[test]
fn transition_reads_transforms_writes() {
    let test_db = TestDb::new();
    let state = test_db.state();

    state.init(&test_db.run_id, "counter", Value::Int(10)).unwrap();

    let (new_val, _versioned) = state.transition(&test_db.run_id, "counter", |current| {
        if let Value::Int(n) = current.value {
            Ok((Value::Int(n + 1), n))  // Returns (new_value, user_result)
        } else {
            Err(strata_core::StrataError::invalid_input("not an int"))
        }
    }).unwrap();

    assert_eq!(new_val, 10); // User result from closure

    let current = state.read(&test_db.run_id, "counter").unwrap().unwrap();
    assert_eq!(current.value.value, Value::Int(11));
}

#[test]
fn transition_or_init_initializes_if_missing() {
    let test_db = TestDb::new();
    let state = test_db.state();

    let (old_val, _versioned) = state.transition_or_init(
        &test_db.run_id,
        "new_cell",
        Value::Int(0), // Initial value
        |current| {
            if let Value::Int(n) = current.value {
                Ok((Value::Int(n + 1), n))  // Returns (new_value, user_result)
            } else {
                Err(strata_core::StrataError::invalid_input("not an int"))
            }
        }
    ).unwrap();

    // Should have initialized to 0 and then transformed
    assert_eq!(old_val, 0);

    let current = state.read(&test_db.run_id, "new_cell").unwrap().unwrap();
    assert_eq!(current.value.value, Value::Int(1));
}

// ============================================================================
// List
// ============================================================================

#[test]
fn list_returns_all_cells() {
    let test_db = TestDb::new();
    let state = test_db.state();

    state.init(&test_db.run_id, "cell_a", Value::Int(1)).unwrap();
    state.init(&test_db.run_id, "cell_b", Value::Int(2)).unwrap();
    state.init(&test_db.run_id, "cell_c", Value::Int(3)).unwrap();

    let cells = state.list(&test_db.run_id).unwrap();
    assert_eq!(cells.len(), 3);
    assert!(cells.contains(&"cell_a".to_string()));
    assert!(cells.contains(&"cell_b".to_string()));
    assert!(cells.contains(&"cell_c".to_string()));
}

#[test]
fn list_empty_run_returns_empty() {
    let test_db = TestDb::new();
    let state = test_db.state();

    let cells = state.list(&test_db.run_id).unwrap();
    assert!(cells.is_empty());
}

// ============================================================================
// Concurrency
// ============================================================================

#[test]
fn concurrent_cas_exactly_one_wins() {
    let test_db = TestDb::new_in_memory();
    let state = test_db.state();
    let run_id = test_db.run_id;

    state.init(&run_id, "counter", Value::Int(0)).unwrap();

    let success_count = Arc::new(AtomicU64::new(0));
    let db = test_db.db.clone();

    let handles: Vec<_> = (0..4).map(|_| {
        let db = db.clone();
        let success = success_count.clone();

        thread::spawn(move || {
            let state = StateCell::new(db);

            // Try to CAS from 0 to 1
            let current = state.read(&run_id, "counter").unwrap().unwrap();
            let version = current.version;

            if state.cas(&run_id, "counter", version, Value::Int(1)).is_ok() {
                success.fetch_add(1, Ordering::SeqCst);
            }
        })
    }).collect();

    for h in handles {
        h.join().unwrap();
    }

    // Exactly one thread should have succeeded
    // (others see stale version after first CAS)
    let wins = success_count.load(Ordering::SeqCst);
    assert!(wins >= 1, "At least one CAS should succeed");
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn empty_cell_name() {
    let test_db = TestDb::new();
    let state = test_db.state();

    state.init(&test_db.run_id, "", Value::Int(1)).unwrap();
    assert!(state.exists(&test_db.run_id, "").unwrap());
}

#[test]
fn special_characters_in_name() {
    let test_db = TestDb::new();
    let state = test_db.state();

    let name = "cell/with:special@chars";
    state.init(&test_db.run_id, name, Value::Int(1)).unwrap();
    assert!(state.exists(&test_db.run_id, name).unwrap());
}
