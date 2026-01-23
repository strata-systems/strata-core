//! JsonStore Durability Tests
//!
//! Tests for durability and crash recovery:
//! - Data persistence across restarts
//! - Complex documents persistence

use crate::*;
use tempfile::tempdir;

/// Test basic document persistence after restart
#[test]
fn test_json_persist_after_restart() {
    let dir = tempdir().unwrap();
    let path = dir.path();

    let run = ApiRunId::default_run_id();
    let key = "persistent_doc";

    // Write document and close
    {
        let db = create_persistent_db(path);
        let document = obj([("message", Value::String("persisted".to_string()))]);
        db.json_set(&run, key, "$", document).unwrap();
    }

    // Reopen and verify
    {
        let db = create_persistent_db(path);
        let result = db.json_get(&run, key, "$").unwrap();
        assert!(result.is_some(), "Document should persist after restart");
        let doc = result.unwrap();
        if let Value::Object(fields) = doc.value {
            let msg = fields.get("message");
            assert!(msg.is_some());
            assert_eq!(msg.unwrap(), &Value::String("persisted".to_string()));
        } else {
            panic!("Expected object");
        }
    }
}

/// Test nested document persistence
#[test]
fn test_json_nested_persist() {
    let dir = tempdir().unwrap();
    let path = dir.path();

    let run = ApiRunId::default_run_id();
    let key = "nested_persistent";

    // Write nested document
    {
        let db = create_persistent_db(path);
        let document = obj([
            ("level1", obj([
                ("level2", obj([
                    ("value", Value::Int(42)),
                ])),
            ])),
        ]);
        db.json_set(&run, key, "$", document).unwrap();
    }

    // Reopen and verify nested structure
    {
        let db = create_persistent_db(path);
        let result = db.json_get(&run, key, "level1.level2.value").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, Value::Int(42));
    }
}

/// Test multiple documents persist
#[test]
fn test_json_multiple_persist() {
    let dir = tempdir().unwrap();
    let path = dir.path();

    let run = ApiRunId::default_run_id();

    // Write multiple documents
    {
        let db = create_persistent_db(path);
        for i in 0..5 {
            let key = format!("doc_{}", i);
            let document = obj([("index", Value::Int(i))]);
            db.json_set(&run, &key, "$", document).unwrap();
        }
    }

    // Reopen and verify all
    {
        let db = create_persistent_db(path);
        for i in 0..5 {
            let key = format!("doc_{}", i);
            let result = db.json_get(&run, &key, "index").unwrap().unwrap();
            assert_eq!(result.value, Value::Int(i));
        }
    }
}

/// Test document updates persist
#[test]
fn test_json_updates_persist() {
    let dir = tempdir().unwrap();
    let path = dir.path();

    let run = ApiRunId::default_run_id();
    let key = "update_persist";

    // Create and update document
    {
        let db = create_persistent_db(path);
        let document = obj([("counter", Value::Int(0))]);
        db.json_set(&run, key, "$", document).unwrap();

        // Update multiple times
        for i in 1..=5 {
            db.json_set(&run, key, "counter", Value::Int(i)).unwrap();
        }
    }

    // Reopen and verify final value
    {
        let db = create_persistent_db(path);
        let result = db.json_get(&run, key, "counter").unwrap().unwrap();
        assert_eq!(result.value, Value::Int(5));
    }
}

/// Test merge operations persist
#[test]
fn test_json_merge_persist() {
    let dir = tempdir().unwrap();
    let path = dir.path();

    let run = ApiRunId::default_run_id();
    let key = "merge_persist";

    // Create and merge
    {
        let db = create_persistent_db(path);
        let document = obj([("a", Value::Int(1))]);
        db.json_set(&run, key, "$", document).unwrap();

        let patch = obj([("b", Value::Int(2))]);
        db.json_merge(&run, key, "$", patch).unwrap();
    }

    // Reopen and verify merge result
    {
        let db = create_persistent_db(path);
        let a = db.json_get(&run, key, "a").unwrap().unwrap();
        let b = db.json_get(&run, key, "b").unwrap().unwrap();
        assert_eq!(a.value, Value::Int(1));
        assert_eq!(b.value, Value::Int(2));
    }
}

/// Test run isolation persists
#[test]
fn test_json_run_isolation_persists() {
    let dir = tempdir().unwrap();
    let path = dir.path();

    let run1 = ApiRunId::default_run_id();
    let run2 = ApiRunId::new();
    let key = "isolated_doc";

    // Create documents in different runs
    {
        let db = create_persistent_db(path);
        db.run_create(Some(&run2), None).unwrap();

        let obj1 = obj([("run", Value::Int(1))]);
        let obj2 = obj([("run", Value::Int(2))]);

        db.json_set(&run1, key, "$", obj1).unwrap();
        db.json_set(&run2, key, "$", obj2).unwrap();
    }

    // Reopen and verify isolation
    {
        let db = create_persistent_db(path);
        let r1 = db.json_get(&run1, key, "run").unwrap().unwrap();
        let r2 = db.json_get(&run2, key, "run").unwrap().unwrap();
        assert_eq!(r1.value, Value::Int(1));
        assert_eq!(r2.value, Value::Int(2));
    }
}

/// Test delete operations persist
#[test]
fn test_json_delete_persist() {
    let dir = tempdir().unwrap();
    let path = dir.path();

    let run = ApiRunId::default_run_id();
    let key = "delete_persist";

    // Create document and delete field
    {
        let db = create_persistent_db(path);
        let document = obj([
            ("keep", Value::Int(1)),
            ("remove", Value::Int(2)),
        ]);
        db.json_set(&run, key, "$", document).unwrap();
        db.json_delete(&run, key, "remove").unwrap();
    }

    // Reopen and verify deletion persisted
    {
        let db = create_persistent_db(path);
        assert!(db.json_get(&run, key, "keep").unwrap().is_some());
        assert!(db.json_get(&run, key, "remove").unwrap().is_none());
    }
}
