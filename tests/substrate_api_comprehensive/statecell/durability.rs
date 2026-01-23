//! StateCell Durability Tests
//!
//! Tests for durability and crash recovery:
//! - Data persistence across restarts
//! - WAL replay
//! - Durability modes (Strict vs Buffered)

use crate::*;
use tempfile::tempdir;

/// Test basic persistence after restart
#[test]
fn test_state_persist_after_restart() {
    let dir = tempdir().unwrap();
    let path = dir.path();

    let run = ApiRunId::default_run_id();
    let cell = "persistent_cell";

    // Write data and close
    {
        let db = create_persistent_db(path);
        db.state_set(&run, cell, Value::String("persisted".to_string())).unwrap();
        // db dropped here, should flush
    }

    // Reopen and verify
    {
        let db = create_persistent_db(path);
        let result = db.state_get(&run, cell).unwrap();
        assert!(result.is_some(), "Data should persist after restart");
        assert_eq!(
            result.unwrap().value,
            Value::String("persisted".to_string())
        );
    }
}

/// Test multiple values persist
#[test]
fn test_state_multiple_persist() {
    let dir = tempdir().unwrap();
    let path = dir.path();

    let run = ApiRunId::default_run_id();

    // Write multiple cells
    {
        let db = create_persistent_db(path);
        for i in 0..5 {
            let cell = format!("cell_{}", i);
            db.state_set(&run, &cell, Value::Int(i as i64 * 100)).unwrap();
        }
    }

    // Reopen and verify all
    {
        let db = create_persistent_db(path);
        for i in 0..5 {
            let cell = format!("cell_{}", i);
            let result = db.state_get(&run, &cell).unwrap().unwrap();
            assert_eq!(result.value, Value::Int(i as i64 * 100));
        }
    }
}

/// Test version persists correctly
#[test]
fn test_state_version_persists() {
    let dir = tempdir().unwrap();
    let path = dir.path();

    let run = ApiRunId::default_run_id();
    let cell = "versioned_cell";

    // Set value multiple times to increment version
    {
        let db = create_persistent_db(path);
        db.state_set(&run, cell, Value::Int(1)).unwrap();
        db.state_set(&run, cell, Value::Int(2)).unwrap();
        db.state_set(&run, cell, Value::Int(3)).unwrap();
    }

    // Reopen and verify version is > 1
    {
        let db = create_persistent_db(path);
        let result = db.state_get(&run, cell).unwrap().unwrap();
        assert_eq!(result.value, Value::Int(3));
        // Version should be at least 3 (possibly higher depending on implementation)
        if let strata_core::Version::Counter(c) = result.version {
            assert!(c >= 3, "Version should be at least 3, got {}", c);
        }
    }
}

/// Test delete persists
#[test]
fn test_state_delete_persists() {
    let dir = tempdir().unwrap();
    let path = dir.path();

    let run = ApiRunId::default_run_id();
    let cell = "deleted_cell";

    // Create and delete
    {
        let db = create_persistent_db(path);
        db.state_set(&run, cell, Value::Int(42)).unwrap();
        db.state_delete(&run, cell).unwrap();
    }

    // Reopen and verify deleted
    {
        let db = create_persistent_db(path);
        assert!(db.state_get(&run, cell).unwrap().is_none());
    }
}

/// Test run isolation persists
#[test]
fn test_state_run_isolation_persists() {
    let dir = tempdir().unwrap();
    let path = dir.path();

    let run1 = ApiRunId::default_run_id();
    let run2 = ApiRunId::new();
    let cell = "isolated_cell";

    // Create cells in different runs
    {
        let db = create_persistent_db(path);
        db.run_create(Some(&run2), None).unwrap();
        db.state_set(&run1, cell, Value::Int(111)).unwrap();
        db.state_set(&run2, cell, Value::Int(222)).unwrap();
    }

    // Reopen and verify isolation
    {
        let db = create_persistent_db(path);
        assert_eq!(db.state_get(&run1, cell).unwrap().unwrap().value, Value::Int(111));
        assert_eq!(db.state_get(&run2, cell).unwrap().unwrap().value, Value::Int(222));
    }
}
