//! StateCell Compare-and-Swap Operations Tests
//!
//! Tests for CAS operations:
//! - state_cas (compare-and-swap with expected version counter)
//! - Counter semantics and conflict detection

use crate::*;
use strata_core::Version;

/// Helper to extract counter from Version
fn get_counter(version: &Version) -> u64 {
    match version {
        Version::Counter(c) => *c,
        _ => panic!("Expected Version::Counter"),
    }
}

/// Test successful CAS operation
#[test]
fn test_state_cas_success() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "cas_cell";

        // Set initial value
        let v1 = db.state_set(&run, cell, Value::Int(1)).unwrap();
        let counter = get_counter(&v1);

        // CAS with correct expected counter
        let result = db.state_cas(&run, cell, Some(counter), Value::Int(2)).unwrap();
        assert!(result.is_some(), "CAS should succeed with correct expected counter");

        // Verify new value
        let result = db.state_get(&run, cell).unwrap().unwrap();
        assert_eq!(result.value, Value::Int(2));
    });
}

/// Test CAS failure due to wrong expected counter
#[test]
fn test_state_cas_failure_wrong_counter() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "cas_fail_cell";

        // Set initial value
        db.state_set(&run, cell, Value::Int(1)).unwrap();

        // CAS with wrong expected counter
        let result = db.state_cas(&run, cell, Some(999), Value::Int(2)).unwrap();
        assert!(result.is_none(), "CAS should fail with wrong expected counter");

        // Verify value unchanged
        let result = db.state_get(&run, cell).unwrap().unwrap();
        assert_eq!(result.value, Value::Int(1));
    });
}

/// Test CAS on non-existent cell with None expected counter (create if not exists)
#[test]
fn test_state_cas_create_if_not_exists() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "cas_new_cell";

        // CAS with None expected counter (cell doesn't exist)
        let result = db.state_cas(&run, cell, None, Value::Int(42)).unwrap();
        assert!(result.is_some(), "CAS should succeed for non-existent cell with None expected");

        // Verify value was created
        let result = db.state_get(&run, cell).unwrap().unwrap();
        assert_eq!(result.value, Value::Int(42));
    });
}

/// Test CAS fails when expecting None but cell exists
#[test]
fn test_state_cas_fail_expected_none_but_exists() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "cas_exists_cell";

        // Create the cell first
        db.state_set(&run, cell, Value::Int(100)).unwrap();

        // CAS with None expected counter should fail because cell exists
        let result = db.state_cas(&run, cell, None, Value::Int(999)).unwrap();
        assert!(result.is_none(), "CAS should fail when expecting None but cell exists");

        // Verify value unchanged
        let result = db.state_get(&run, cell).unwrap().unwrap();
        assert_eq!(result.value, Value::Int(100));
    });
}

/// Test CAS fails when expecting counter but cell doesn't exist
#[test]
fn test_state_cas_fail_expected_counter_but_not_exists() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "cas_not_exists_cell";

        // CAS with expected counter but cell doesn't exist
        let result = db.state_cas(&run, cell, Some(1), Value::Int(2)).unwrap();
        assert!(result.is_none(), "CAS should fail when expecting counter but cell doesn't exist");

        // Verify cell still doesn't exist
        assert!(db.state_get(&run, cell).unwrap().is_none());
    });
}

/// Test multiple sequential CAS operations
#[test]
fn test_state_cas_sequential() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "sequential_cas";

        // Create with CAS - state_cas returns Option<Version>
        let v0 = db.state_cas(&run, cell, None, Value::Int(0)).unwrap();
        assert!(v0.is_some(), "Initial CAS should succeed");
        let mut counter = get_counter(&v0.unwrap());

        // Increment pattern with CAS
        for i in 0..5 {
            let result = db.state_cas(&run, cell, Some(counter), Value::Int(i + 1)).unwrap();
            assert!(result.is_some(), "CAS should succeed at iteration {}", i);
            counter = get_counter(&result.unwrap());
        }

        // Final value should be 5
        let result = db.state_get(&run, cell).unwrap().unwrap();
        assert_eq!(result.value, Value::Int(5));
    });
}

/// Test CAS with different types
#[test]
fn test_state_cas_type_change() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "type_change_cell";

        // Set string value
        let v1 = db.state_set(&run, cell, Value::String("hello".to_string())).unwrap();
        let counter = get_counter(&v1);

        // CAS to change type to Int
        let result = db.state_cas(&run, cell, Some(counter), Value::Int(42)).unwrap();
        assert!(result.is_some());

        // Verify type changed
        let result = db.state_get(&run, cell).unwrap().unwrap();
        assert_eq!(result.value, Value::Int(42));
    });
}

/// Test CAS with complex values
#[test]
fn test_state_cas_complex_values() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "complex_cas";

        let v1 = Value::Array(vec![Value::Int(1), Value::Int(2)]);
        let v2 = Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);

        // Set initial array
        let version = db.state_set(&run, cell, v1).unwrap();
        let counter = get_counter(&version);

        // CAS to append
        let result = db.state_cas(&run, cell, Some(counter), v2.clone()).unwrap();
        assert!(result.is_some());

        // Verify
        let result = db.state_get(&run, cell).unwrap().unwrap();
        assert_eq!(result.value, v2);
    });
}

/// Test CAS counter increments correctly
#[test]
fn test_state_cas_counter_increments() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "cas_counter";

        // Create with CAS - state_cas returns Option<Version>
        let c1 = get_counter(&db.state_cas(&run, cell, None, Value::Int(0)).unwrap().unwrap());

        // Each successful CAS should return incrementing counters
        let c2 = get_counter(&db.state_cas(&run, cell, Some(c1), Value::Int(1)).unwrap().unwrap());
        let c3 = get_counter(&db.state_cas(&run, cell, Some(c2), Value::Int(2)).unwrap().unwrap());

        assert!(c2 > c1, "Counter should increment");
        assert!(c3 > c2, "Counter should increment");
    });
}

/// Test CAS with stale counter after concurrent update
#[test]
fn test_state_cas_stale_counter() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "stale_cas";

        // Set initial value
        let v1 = db.state_set(&run, cell, Value::Int(1)).unwrap();
        let stale_counter = get_counter(&v1);

        // Update the cell, making our counter stale
        db.state_set(&run, cell, Value::Int(2)).unwrap();

        // CAS with stale counter should fail
        let result = db.state_cas(&run, cell, Some(stale_counter), Value::Int(999)).unwrap();
        assert!(result.is_none(), "CAS with stale counter should fail");

        // Value should remain at 2, not 999
        let result = db.state_get(&run, cell).unwrap().unwrap();
        assert_eq!(result.value, Value::Int(2));
    });
}
