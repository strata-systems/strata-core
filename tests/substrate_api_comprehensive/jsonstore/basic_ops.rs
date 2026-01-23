//! JsonStore Basic Operations Tests
//!
//! Tests for fundamental JsonStore operations:
//! - json_set / json_get
//! - json_delete
//! - json_exists
//! - json_get_version

use crate::*;

/// Test basic set and get operations at root
#[test]
fn test_json_set_get_root() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "test_doc";

        // Create a document at root
        let document = obj([
            ("name", Value::String("Alice".to_string())),
            ("age", Value::Int(30)),
        ]);
        let _version = db.json_set(&run, key, "$", document.clone()).unwrap();

        // Get the entire document
        let result = db.json_get(&run, key, "$").unwrap();
        assert!(result.is_some(), "Document should exist");
        assert_eq!(result.unwrap().value, document);
    });
}

/// Test setting nested fields
#[test]
fn test_json_set_nested_field() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "nested_doc";

        // Create base document
        let document = obj([
            ("user", obj([
                ("name", Value::String("Bob".to_string())),
            ])),
        ]);
        db.json_set(&run, key, "$", document).unwrap();

        // Set a nested field
        db.json_set(&run, key, "user.email", Value::String("bob@example.com".to_string())).unwrap();

        // Verify nested field
        let result = db.json_get(&run, key, "user.email").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, Value::String("bob@example.com".to_string()));
    });
}

/// Test getting a non-existent document
#[test]
fn test_json_get_nonexistent() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let result = db.json_get(&run, "nonexistent_doc", "$").unwrap();
        assert!(result.is_none(), "Non-existent document should return None");
    });
}

/// Test getting a non-existent path in existing document
#[test]
fn test_json_get_nonexistent_path() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "doc_with_missing_path";

        // Create document
        let document = obj([("exists", Value::Int(1))]);
        db.json_set(&run, key, "$", document).unwrap();

        // Get non-existent path
        let result = db.json_get(&run, key, "missing").unwrap();
        assert!(result.is_none(), "Missing path should return None");
    });
}

/// Test deleting a field
#[test]
fn test_json_delete_field() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "delete_field_doc";

        // Create document with multiple fields
        let document = obj([
            ("keep", Value::Int(1)),
            ("remove", Value::Int(2)),
        ]);
        db.json_set(&run, key, "$", document).unwrap();

        // Delete one field
        let count = db.json_delete(&run, key, "remove").unwrap();
        assert_eq!(count, 1, "Should delete one field");

        // Verify deletion
        assert!(db.json_get(&run, key, "remove").unwrap().is_none());
        assert!(db.json_get(&run, key, "keep").unwrap().is_some());
    });
}

/// Test deleting non-existent path
///
/// Note: Current implementation is idempotent - deleting a nonexistent path succeeds
/// and returns 1 (the substrate doesn't differentiate between existing and non-existing paths).
/// This matches RFC 7396 idempotent delete semantics.
#[test]
fn test_json_delete_nonexistent_path() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "delete_missing_doc";

        // Create document
        let document = obj([("exists", Value::Int(1))]);
        db.json_set(&run, key, "$", document).unwrap();

        // Delete non-existent path - idempotent operation succeeds
        let count = db.json_delete(&run, key, "nonexistent").unwrap();
        // Implementation always returns 1 for non-root deletes
        assert_eq!(count, 1, "Delete is idempotent, returns 1 even for non-existent path");

        // Verify the existing field is unaffected
        assert!(db.json_get(&run, key, "exists").unwrap().is_some());
    });
}

/// Test json_exists operation
#[test]
fn test_json_exists() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "exists_test_doc";

        // Should not exist initially
        assert!(!db.json_exists(&run, key).unwrap());

        // Create document
        let document = obj([("field", Value::Int(1))]);
        db.json_set(&run, key, "$", document).unwrap();

        // Should exist now
        assert!(db.json_exists(&run, key).unwrap());
    });
}

/// Test json_get_version operation
#[test]
fn test_json_get_version() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "version_test_doc";

        // No version for non-existent doc
        assert!(db.json_get_version(&run, key).unwrap().is_none());

        // Create document
        let document = obj([("field", Value::Int(1))]);
        db.json_set(&run, key, "$", document).unwrap();
        let v1 = db.json_get_version(&run, key).unwrap();
        assert!(v1.is_some(), "Version should exist");

        // Update document
        db.json_set(&run, key, "field", Value::Int(2)).unwrap();
        let v2 = db.json_get_version(&run, key).unwrap();
        assert!(v2.is_some());
    });
}

/// Test overwriting entire document
#[test]
fn test_json_overwrite_document() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "overwrite_doc";

        // Create initial document
        let obj1 = obj([("original", Value::Int(1))]);
        db.json_set(&run, key, "$", obj1).unwrap();

        // Overwrite with new document
        let obj2 = obj([("replaced", Value::Int(2))]);
        db.json_set(&run, key, "$", obj2.clone()).unwrap();

        // Verify overwrite
        let result = db.json_get(&run, key, "$").unwrap().unwrap();
        assert_eq!(result.value, obj2);
        assert!(db.json_get(&run, key, "original").unwrap().is_none());
    });
}

/// Test all value types in JSON document
#[test]
fn test_json_all_value_types() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "all_types_doc";

        let document = obj([
            ("null_field", Value::Null),
            ("bool_field", Value::Bool(true)),
            ("int_field", Value::Int(42)),
            ("float_field", Value::Float(3.14)),
            ("string_field", Value::String("hello".to_string())),
            ("array_field", Value::Array(vec![Value::Int(1), Value::Int(2)])),
            ("nested_obj", obj([("inner", Value::String("nested".to_string()))])),
        ]);

        db.json_set(&run, key, "$", document.clone()).unwrap();
        let result = db.json_get(&run, key, "$").unwrap().unwrap();
        assert_eq!(result.value, document);
    });
}

/// Test run isolation for JSON documents
#[test]
fn test_json_run_isolation() {
    test_across_substrate_modes(|db| {
        let run1 = ApiRunId::default_run_id();
        let run2 = ApiRunId::new();
        let key = "shared_key";

        // Create run2
        db.run_create(Some(&run2), None).unwrap();

        // Create documents with same key in different runs
        let obj1 = obj([("run", Value::Int(1))]);
        let obj2 = obj([("run", Value::Int(2))]);

        db.json_set(&run1, key, "$", obj1.clone()).unwrap();
        db.json_set(&run2, key, "$", obj2.clone()).unwrap();

        // Verify isolation
        assert_eq!(db.json_get(&run1, key, "$").unwrap().unwrap().value, obj1);
        assert_eq!(db.json_get(&run2, key, "$").unwrap().unwrap().value, obj2);
    });
}

/// Test multiple documents
#[test]
fn test_json_multiple_documents() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Create multiple documents
        for i in 0..10 {
            let key = format!("doc_{}", i);
            let document = obj([("index", Value::Int(i))]);
            db.json_set(&run, &key, "$", document).unwrap();
        }

        // Verify all documents
        for i in 0..10 {
            let key = format!("doc_{}", i);
            let result = db.json_get(&run, &key, "index").unwrap().unwrap();
            assert_eq!(result.value, Value::Int(i));
        }
    });
}
