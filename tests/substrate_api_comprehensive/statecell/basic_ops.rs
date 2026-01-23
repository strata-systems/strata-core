//! StateCell Basic Operations Tests
//!
//! Tests for fundamental StateCell operations:
//! - state_set / state_get
//! - state_delete
//! - state_exists

use crate::*;
use strata_core::Version;

/// Test basic set and get operations
#[test]
fn test_state_set_get() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "test_cell";

        // Set a value
        let version = db.state_set(&run, cell, Value::Int(42)).unwrap();
        assert!(matches!(version, Version::Counter(_)));

        // Get the value back
        let result = db.state_get(&run, cell).unwrap();
        assert!(result.is_some());
        let versioned = result.unwrap();
        assert_eq!(versioned.value, Value::Int(42));
    });
}

/// Test that set returns incrementing versions
#[test]
fn test_state_set_incrementing_versions() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "counter_cell";

        let v1 = db.state_set(&run, cell, Value::Int(1)).unwrap();
        let v2 = db.state_set(&run, cell, Value::Int(2)).unwrap();
        let v3 = db.state_set(&run, cell, Value::Int(3)).unwrap();

        // Versions should be incrementing
        if let (Version::Counter(c1), Version::Counter(c2), Version::Counter(c3)) = (v1, v2, v3) {
            assert!(c2 > c1, "v2 should be greater than v1");
            assert!(c3 > c2, "v3 should be greater than v2");
        }
    });
}

/// Test getting a non-existent cell
#[test]
fn test_state_get_nonexistent() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let result = db.state_get(&run, "nonexistent_cell").unwrap();
        assert!(result.is_none());
    });
}

/// Test deleting a cell
#[test]
fn test_state_delete() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "delete_cell";

        // Set a value
        db.state_set(&run, cell, Value::String("hello".to_string())).unwrap();
        assert!(db.state_get(&run, cell).unwrap().is_some());

        // Delete the cell
        let deleted = db.state_delete(&run, cell).unwrap();
        assert!(deleted);

        // Verify it's gone
        assert!(db.state_get(&run, cell).unwrap().is_none());
    });
}

/// Test deleting a non-existent cell
#[test]
fn test_state_delete_nonexistent() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let deleted = db.state_delete(&run, "never_existed").unwrap();
        assert!(!deleted);
    });
}

/// Test exists operation
#[test]
fn test_state_exists() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "exists_cell";

        // Should not exist initially
        assert!(!db.state_exists(&run, cell).unwrap());

        // Create the cell
        db.state_set(&run, cell, Value::Bool(true)).unwrap();
        assert!(db.state_exists(&run, cell).unwrap());

        // Delete the cell
        db.state_delete(&run, cell).unwrap();
        assert!(!db.state_exists(&run, cell).unwrap());
    });
}

/// Test all value types
#[test]
fn test_state_all_value_types() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Test each value type
        let test_cases = vec![
            ("null_cell", Value::Null),
            ("bool_cell", Value::Bool(true)),
            ("int_cell", Value::Int(-9999)),
            ("float_cell", Value::Float(3.14159)),
            ("string_cell", Value::String("hello world".to_string())),
            ("bytes_cell", Value::Bytes(vec![0xDE, 0xAD, 0xBE, 0xEF])),
            ("array_cell", Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)])),
        ];

        for (cell, value) in test_cases {
            db.state_set(&run, cell, value.clone()).unwrap();
            let result = db.state_get(&run, cell).unwrap().unwrap();
            assert_eq!(result.value, value, "Failed for cell: {}", cell);
        }
    });
}

/// Test updating a cell (overwrite)
#[test]
fn test_state_overwrite() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "overwrite_cell";

        // Set initial value
        db.state_set(&run, cell, Value::Int(1)).unwrap();
        assert_eq!(db.state_get(&run, cell).unwrap().unwrap().value, Value::Int(1));

        // Overwrite with new value
        db.state_set(&run, cell, Value::Int(999)).unwrap();
        assert_eq!(db.state_get(&run, cell).unwrap().unwrap().value, Value::Int(999));

        // Overwrite with different type
        db.state_set(&run, cell, Value::String("changed".to_string())).unwrap();
        assert_eq!(
            db.state_get(&run, cell).unwrap().unwrap().value,
            Value::String("changed".to_string())
        );
    });
}

/// Test multiple cells
#[test]
fn test_state_multiple_cells() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Create multiple cells
        for i in 0..10 {
            let cell = format!("cell_{}", i);
            db.state_set(&run, &cell, Value::Int(i)).unwrap();
        }

        // Verify all cells
        for i in 0..10 {
            let cell = format!("cell_{}", i);
            let result = db.state_get(&run, &cell).unwrap().unwrap();
            assert_eq!(result.value, Value::Int(i));
        }
    });
}

/// Test cell isolation between runs
#[test]
fn test_state_run_isolation() {
    test_across_substrate_modes(|db| {
        let run1 = ApiRunId::default_run_id();
        let run2 = ApiRunId::new();
        let cell = "shared_name";

        // Ensure run2 exists
        db.run_create(Some(&run2), None).unwrap();

        // Set different values in different runs
        db.state_set(&run1, cell, Value::Int(100)).unwrap();
        db.state_set(&run2, cell, Value::Int(200)).unwrap();

        // Verify isolation
        assert_eq!(db.state_get(&run1, cell).unwrap().unwrap().value, Value::Int(100));
        assert_eq!(db.state_get(&run2, cell).unwrap().unwrap().value, Value::Int(200));
    });
}
