//! JsonStore Merge Operations Tests
//!
//! Tests for JSON Merge Patch (RFC 7396):
//! - Object merging
//! - Null removes fields
//! - Array replacement
//! - Nested merging
//!
//! All tests use dirty test data from fixtures/dirty_jsonstore_data.json

use crate::*;
use crate::test_data::{load_jsonstore_test_data, JsonStoreTestData};
use std::sync::OnceLock;

/// Lazily loaded test data (shared across tests)
fn test_data() -> &'static JsonStoreTestData {
    static DATA: OnceLock<JsonStoreTestData> = OnceLock::new();
    DATA.get_or_init(|| load_jsonstore_test_data())
}

/// Test basic object merge using test data
#[test]
fn test_json_merge_basic() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let objects: Vec<_> = data.object_entities().into_iter()
            .filter(|(_, e)| !e.key.is_empty())
            .map(|(_, e)| e)
            .take(2)
            .collect();

        if objects.len() < 2 {
            return; // Skip if not enough objects
        }

        let key = &objects[0].key;

        // Create initial document
        db.json_set(&run, key, "$", objects[0].value.clone()).unwrap();

        // Merge with patch
        let patch = obj([
            ("merged_field", Value::Int(42)),
            ("extra", Value::String("added".to_string())),
        ]);
        db.json_merge(&run, key, "$", patch).unwrap();

        // Verify merged result exists
        let result = db.json_get(&run, key, "$").unwrap();
        assert!(result.is_some(), "Merged document should exist");

        // Verify new field was added
        let extra = db.json_get(&run, key, "extra").unwrap();
        assert!(extra.is_some(), "Merged field should exist");
        assert_eq!(extra.unwrap().value, Value::String("added".to_string()));
    });
}

/// Test merge with null deletes field (RFC 7396)
#[test]
fn test_json_merge_null_deletes() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entity = data.get_entities(0).iter()
            .find(|e| !e.key.is_empty())
            .unwrap();

        // Create initial document with fields
        let document = obj([
            ("keep", Value::Int(1)),
            ("remove", Value::Int(2)),
        ]);
        db.json_set(&run, &entity.key, "$", document).unwrap();

        // Merge with null to delete
        let patch = obj([("remove", Value::Null)]);
        db.json_merge(&run, &entity.key, "$", patch).unwrap();

        // Verify field removed
        assert!(db.json_get(&run, &entity.key, "keep").unwrap().is_some());
        assert!(db.json_get(&run, &entity.key, "remove").unwrap().is_none());
    });
}

/// Test merge replaces arrays entirely
#[test]
fn test_json_merge_array_replacement() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Get some array entities from test data
        let arrays: Vec<_> = data.array_entities().into_iter()
            .filter(|(_, e)| !e.key.is_empty())
            .map(|(_, e)| e)
            .take(2)
            .collect();

        let entity = data.get_entities(0).iter()
            .find(|e| !e.key.is_empty())
            .unwrap();

        // Create initial document with array
        let document = obj([
            ("items", Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)])),
        ]);
        db.json_set(&run, &entity.key, "$", document).unwrap();

        // Merge with new array - should replace, not merge
        let new_array = if !arrays.is_empty() {
            arrays[0].value.clone()
        } else {
            Value::Array(vec![Value::Int(99)])
        };
        let patch = obj([("items", new_array.clone())]);
        db.json_merge(&run, &entity.key, "$", patch).unwrap();

        // Verify array replaced
        let result = db.json_get(&run, &entity.key, "items").unwrap().unwrap();
        assert_eq!(result.value, new_array);
    });
}

/// Test nested object merge using test data
#[test]
fn test_json_merge_nested() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entity = data.get_entities(0).iter()
            .find(|e| !e.key.is_empty())
            .unwrap();

        // Create initial document
        let document = obj([
            ("user", obj([
                ("name", Value::String("Eve".to_string())),
                ("age", Value::Int(25)),
            ])),
        ]);
        db.json_set(&run, &entity.key, "$", document).unwrap();

        // Merge nested object
        let patch = obj([
            ("user", obj([
                ("age", Value::Int(26)),
                ("email", Value::String("eve@example.com".to_string())),
            ])),
        ]);
        db.json_merge(&run, &entity.key, "$", patch).unwrap();

        // Verify nested merge
        let name = db.json_get(&run, &entity.key, "user.name").unwrap().unwrap();
        let age = db.json_get(&run, &entity.key, "user.age").unwrap().unwrap();
        let email = db.json_get(&run, &entity.key, "user.email").unwrap().unwrap();

        assert_eq!(name.value, Value::String("Eve".to_string())); // unchanged
        assert_eq!(age.value, Value::Int(26)); // updated
        assert_eq!(email.value, Value::String("eve@example.com".to_string())); // added
    });
}

/// Test merge at specific path (not root) using test data
#[test]
fn test_json_merge_at_path() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entity = data.get_entities(0).iter()
            .find(|e| !e.key.is_empty())
            .unwrap();

        // Create initial document
        let document = obj([
            ("meta", obj([("version", Value::Int(1))])),
            ("data", obj([("value", Value::Int(100))])),
        ]);
        db.json_set(&run, &entity.key, "$", document).unwrap();

        // Merge only at $.data path
        let patch = obj([
            ("value", Value::Int(200)),
            ("extra", Value::Int(300)),
        ]);
        db.json_merge(&run, &entity.key, "data", patch).unwrap();

        // Verify merge at path
        let value = db.json_get(&run, &entity.key, "data.value").unwrap().unwrap();
        let extra = db.json_get(&run, &entity.key, "data.extra").unwrap().unwrap();
        let version = db.json_get(&run, &entity.key, "meta.version").unwrap().unwrap();

        assert_eq!(value.value, Value::Int(200));
        assert_eq!(extra.value, Value::Int(300));
        assert_eq!(version.value, Value::Int(1)); // unchanged
    });
}

/// Test merge scalar replaces target
#[test]
fn test_json_merge_scalar_replace() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entity = data.get_entities(0).iter()
            .find(|e| !e.key.is_empty())
            .unwrap();

        // Create initial document
        let document = obj([
            ("field", obj([("nested", Value::Int(1))])),
        ]);
        db.json_set(&run, &entity.key, "$", document).unwrap();

        // Merge with scalar replaces entire object
        let patch = obj([("field", Value::Int(42))]);
        db.json_merge(&run, &entity.key, "$", patch).unwrap();

        // Verify scalar replaced object
        let result = db.json_get(&run, &entity.key, "field").unwrap().unwrap();
        assert_eq!(result.value, Value::Int(42));
    });
}

/// Test merge on non-existent document creates it using test data
#[test]
fn test_json_merge_creates_document() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entity = data.get_entities(0).iter()
            .find(|e| !e.key.is_empty())
            .unwrap();

        // Use unique key that doesn't exist
        let unique_key = format!("{}_merge_create", entity.key);

        // Merge on non-existent document
        let patch = obj([("created", Value::Bool(true))]);
        db.json_merge(&run, &unique_key, "$", patch.clone()).unwrap();

        // Verify document created
        let result = db.json_get(&run, &unique_key, "$").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, patch);
    });
}

/// Test complex merge scenario using test data
#[test]
fn test_json_merge_complex() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entity = data.get_entities(0).iter()
            .find(|e| !e.key.is_empty())
            .unwrap();

        // Create initial document
        let document = obj([
            ("config", obj([
                ("debug", Value::Bool(false)),
                ("timeout", Value::Int(30)),
                ("endpoints", Value::Array(vec![Value::String("http://a.com".to_string())])),
            ])),
            ("metadata", obj([("version", Value::String("1.0".to_string()))])),
        ]);
        db.json_set(&run, &entity.key, "$", document).unwrap();

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
        db.json_merge(&run, &entity.key, "$", patch).unwrap();

        // Verify complex merge
        let debug = db.json_get(&run, &entity.key, "config.debug").unwrap().unwrap();
        assert_eq!(debug.value, Value::Bool(true));

        let timeout = db.json_get(&run, &entity.key, "config.timeout").unwrap();
        assert!(timeout.is_none(), "timeout should be deleted");

        let endpoints = db.json_get(&run, &entity.key, "config.endpoints").unwrap().unwrap();
        assert_eq!(endpoints.value, Value::Array(vec![
            Value::String("http://b.com".to_string()),
            Value::String("http://c.com".to_string()),
        ]));

        let status = db.json_get(&run, &entity.key, "status").unwrap().unwrap();
        assert_eq!(status.value, Value::String("active".to_string()));

        let version = db.json_get(&run, &entity.key, "metadata.version").unwrap().unwrap();
        assert_eq!(version.value, Value::String("1.0".to_string()));
    });
}

/// Test merge with dirty data (unicode, special chars)
#[test]
fn test_json_merge_dirty_data() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Get dirty entities
        let dirty: Vec<_> = data.dirty_entities().into_iter()
            .filter(|(_, e)| !e.key.is_empty())
            .take(5)
            .collect();

        if dirty.is_empty() {
            return;
        }

        let (_, base_entity) = &dirty[0];

        // Create base document
        db.json_set(&run, &base_entity.key, "$", obj([
            ("base", Value::Int(1)),
        ])).unwrap();

        // Merge dirty data
        for (_, entity) in dirty.iter().skip(1) {
            if let Value::Object(_) = &entity.value {
                db.json_merge(&run, &base_entity.key, "$", entity.value.clone()).unwrap();
            }
        }

        // Verify document still valid
        let result = db.json_get(&run, &base_entity.key, "$").unwrap();
        assert!(result.is_some(), "Merged dirty document should exist");
    });
}
