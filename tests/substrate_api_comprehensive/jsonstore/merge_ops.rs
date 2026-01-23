//! JsonStore Merge Operations Tests
//!
//! Tests for JSON Merge Patch (RFC 7396):
//! - Object merging
//! - Null removes fields
//! - Array replacement
//! - Nested merging

use crate::*;

/// Test basic object merge
#[test]
fn test_json_merge_basic() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "merge_basic";

        // Create initial document
        let document = obj([
            ("a", Value::Int(1)),
            ("b", Value::Int(2)),
        ]);
        db.json_set(&run, key, "$", document).unwrap();

        // Merge: add field and update field
        let patch = obj([
            ("b", Value::Int(20)),
            ("c", Value::Int(3)),
        ]);
        db.json_merge(&run, key, "$", patch).unwrap();

        // Verify result - fields should be merged
        let a = db.json_get(&run, key, "a").unwrap().unwrap();
        let b = db.json_get(&run, key, "b").unwrap().unwrap();
        let c = db.json_get(&run, key, "c").unwrap().unwrap();

        assert_eq!(a.value, Value::Int(1));
        assert_eq!(b.value, Value::Int(20));
        assert_eq!(c.value, Value::Int(3));
    });
}

/// Test merge with null deletes field (RFC 7396)
#[test]
fn test_json_merge_null_deletes() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "merge_delete";

        // Create initial document
        let document = obj([
            ("keep", Value::Int(1)),
            ("remove", Value::Int(2)),
        ]);
        db.json_set(&run, key, "$", document).unwrap();

        // Merge with null to delete
        let patch = obj([("remove", Value::Null)]);
        db.json_merge(&run, key, "$", patch).unwrap();

        // Verify field removed
        assert!(db.json_get(&run, key, "keep").unwrap().is_some());
        assert!(db.json_get(&run, key, "remove").unwrap().is_none());
    });
}

/// Test merge replaces arrays entirely
#[test]
fn test_json_merge_array_replacement() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "merge_array";

        // Create initial document with array
        let document = obj([
            ("items", Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)])),
        ]);
        db.json_set(&run, key, "$", document).unwrap();

        // Merge with new array - should replace, not merge
        let patch = obj([
            ("items", Value::Array(vec![Value::Int(99)])),
        ]);
        db.json_merge(&run, key, "$", patch).unwrap();

        // Verify array replaced
        let result = db.json_get(&run, key, "items").unwrap().unwrap();
        assert_eq!(result.value, Value::Array(vec![Value::Int(99)]));
    });
}

/// Test nested object merge
#[test]
fn test_json_merge_nested() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "merge_nested";

        // Create initial document
        let document = obj([
            ("user", obj([
                ("name", Value::String("Eve".to_string())),
                ("age", Value::Int(25)),
            ])),
        ]);
        db.json_set(&run, key, "$", document).unwrap();

        // Merge nested object
        let patch = obj([
            ("user", obj([
                ("age", Value::Int(26)),
                ("email", Value::String("eve@example.com".to_string())),
            ])),
        ]);
        db.json_merge(&run, key, "$", patch).unwrap();

        // Verify nested merge
        let name = db.json_get(&run, key, "user.name").unwrap().unwrap();
        let age = db.json_get(&run, key, "user.age").unwrap().unwrap();
        let email = db.json_get(&run, key, "user.email").unwrap().unwrap();

        assert_eq!(name.value, Value::String("Eve".to_string())); // unchanged
        assert_eq!(age.value, Value::Int(26)); // updated
        assert_eq!(email.value, Value::String("eve@example.com".to_string())); // added
    });
}

/// Test merge at specific path (not root)
#[test]
fn test_json_merge_at_path() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "merge_path";

        // Create initial document
        let document = obj([
            ("meta", obj([("version", Value::Int(1))])),
            ("data", obj([("value", Value::Int(100))])),
        ]);
        db.json_set(&run, key, "$", document).unwrap();

        // Merge only at $.data path
        let patch = obj([
            ("value", Value::Int(200)),
            ("extra", Value::Int(300)),
        ]);
        db.json_merge(&run, key, "data", patch).unwrap();

        // Verify merge at path
        let value = db.json_get(&run, key, "data.value").unwrap().unwrap();
        let extra = db.json_get(&run, key, "data.extra").unwrap().unwrap();
        let version = db.json_get(&run, key, "meta.version").unwrap().unwrap();

        assert_eq!(value.value, Value::Int(200));
        assert_eq!(extra.value, Value::Int(300));
        assert_eq!(version.value, Value::Int(1)); // unchanged
    });
}

/// Test merge scalar replaces target
#[test]
fn test_json_merge_scalar_replace() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "merge_scalar";

        // Create initial document
        let document = obj([
            ("field", obj([("nested", Value::Int(1))])),
        ]);
        db.json_set(&run, key, "$", document).unwrap();

        // Merge with scalar replaces entire object
        let patch = obj([("field", Value::Int(42))]);
        db.json_merge(&run, key, "$", patch).unwrap();

        // Verify scalar replaced object
        let result = db.json_get(&run, key, "field").unwrap().unwrap();
        assert_eq!(result.value, Value::Int(42));
    });
}

/// Test merge on non-existent document creates it
#[test]
fn test_json_merge_creates_document() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "merge_create";

        // Merge on non-existent document
        let patch = obj([("created", Value::Bool(true))]);
        db.json_merge(&run, key, "$", patch.clone()).unwrap();

        // Verify document created
        let result = db.json_get(&run, key, "$").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, patch);
    });
}

/// Test complex merge scenario
#[test]
fn test_json_merge_complex() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "merge_complex";

        // Create initial document
        let document = obj([
            ("config", obj([
                ("debug", Value::Bool(false)),
                ("timeout", Value::Int(30)),
                ("endpoints", Value::Array(vec![Value::String("http://a.com".to_string())])),
            ])),
            ("metadata", obj([("version", Value::String("1.0".to_string()))])),
        ]);
        db.json_set(&run, key, "$", document).unwrap();

        // Complex merge
        let patch = obj([
            ("config", obj([
                ("debug", Value::Bool(true)),
                ("timeout", Value::Null), // delete
                ("endpoints", Value::Array(vec![ // replace
                    Value::String("http://b.com".to_string()),
                    Value::String("http://c.com".to_string()),
                ])),
                ("new_setting", Value::Int(100)), // add
            ])),
            ("status", Value::String("active".to_string())), // new field
        ]);
        db.json_merge(&run, key, "$", patch).unwrap();

        // Verify complex merge
        let debug = db.json_get(&run, key, "config.debug").unwrap().unwrap();
        assert_eq!(debug.value, Value::Bool(true));

        let timeout = db.json_get(&run, key, "config.timeout").unwrap();
        assert!(timeout.is_none(), "timeout should be deleted");

        let endpoints = db.json_get(&run, key, "config.endpoints").unwrap().unwrap();
        assert_eq!(endpoints.value, Value::Array(vec![
            Value::String("http://b.com".to_string()),
            Value::String("http://c.com".to_string()),
        ]));

        let status = db.json_get(&run, key, "status").unwrap().unwrap();
        assert_eq!(status.value, Value::String("active".to_string()));

        let version = db.json_get(&run, key, "metadata.version").unwrap().unwrap();
        assert_eq!(version.value, Value::String("1.0".to_string()));
    });
}
