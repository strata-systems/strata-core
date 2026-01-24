//! JsonStore Path Operations Tests
//!
//! Tests for path-based navigation:
//! - Object field access ($.field)
//! - Array index access ($.array[0])
//! - Nested path access
//! - Array append ($.array[-])
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

/// Test simple object field access using test data
#[test]
fn test_json_path_object_field() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entity = data.get_entities(0).iter()
            .find(|e| !e.key.is_empty())
            .unwrap();

        let document = obj([
            ("a", Value::Int(1)),
            ("b", Value::Int(2)),
        ]);
        db.json_set(&run, &entity.key, "$", document).unwrap();

        // Access individual fields
        let a = db.json_get(&run, &entity.key, "a").unwrap().unwrap();
        let b = db.json_get(&run, &entity.key, "b").unwrap().unwrap();
        assert_eq!(a.value, Value::Int(1));
        assert_eq!(b.value, Value::Int(2));
    });
}

/// Test array index access using test data
#[test]
fn test_json_path_array_index() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entity = data.get_entities(0).iter()
            .find(|e| !e.key.is_empty())
            .unwrap();

        let document = obj([
            ("items", Value::Array(vec![
                Value::String("first".to_string()),
                Value::String("second".to_string()),
                Value::String("third".to_string()),
            ])),
        ]);
        db.json_set(&run, &entity.key, "$", document).unwrap();

        // Access array elements
        let item0 = db.json_get(&run, &entity.key, "items[0]").unwrap().unwrap();
        let item1 = db.json_get(&run, &entity.key, "items[1]").unwrap().unwrap();
        let item2 = db.json_get(&run, &entity.key, "items[2]").unwrap().unwrap();

        assert_eq!(item0.value, Value::String("first".to_string()));
        assert_eq!(item1.value, Value::String("second".to_string()));
        assert_eq!(item2.value, Value::String("third".to_string()));
    });
}

/// Test deeply nested path access using test data
#[test]
fn test_json_path_deep_nesting() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entity = data.get_entities(0).iter()
            .find(|e| !e.key.is_empty())
            .unwrap();

        let document = obj([
            ("level1", obj([
                ("level2", obj([
                    ("level3", obj([
                        ("value", Value::String("deep".to_string())),
                    ])),
                ])),
            ])),
        ]);
        db.json_set(&run, &entity.key, "$", document).unwrap();

        // Access deeply nested value
        let deep = db.json_get(&run, &entity.key, "level1.level2.level3.value").unwrap().unwrap();
        assert_eq!(deep.value, Value::String("deep".to_string()));
    });
}

/// Test setting value at nested path (creates intermediates) using test data
#[test]
fn test_json_path_set_nested() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entity = data.get_entities(0).iter()
            .find(|e| !e.key.is_empty())
            .unwrap();

        // Create empty document
        let document = obj([]);
        db.json_set(&run, &entity.key, "$", document).unwrap();

        // Set a nested path (should create intermediates)
        db.json_set(&run, &entity.key, "user.profile.name", Value::String("Charlie".to_string())).unwrap();

        // Verify the nested structure was created
        let name = db.json_get(&run, &entity.key, "user.profile.name").unwrap().unwrap();
        assert_eq!(name.value, Value::String("Charlie".to_string()));
    });
}

/// Test array element modification using test data
#[test]
fn test_json_path_modify_array_element() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entity = data.get_entities(0).iter()
            .find(|e| !e.key.is_empty())
            .unwrap();

        let document = obj([
            ("items", Value::Array(vec![
                Value::Int(1),
                Value::Int(2),
                Value::Int(3),
            ])),
        ]);
        db.json_set(&run, &entity.key, "$", document).unwrap();

        // Modify middle element
        db.json_set(&run, &entity.key, "items[1]", Value::Int(200)).unwrap();

        // Verify modification
        let items = db.json_get(&run, &entity.key, "items").unwrap().unwrap();
        assert_eq!(items.value, Value::Array(vec![
            Value::Int(1),
            Value::Int(200),
            Value::Int(3),
        ]));
    });
}

/// Test array append with [-] syntax
///
/// Note: The `[-]` array append syntax is documented in the API but not yet
/// implemented in the path parser. This test is ignored until that feature is added.
#[test]
#[ignore = "array append syntax [-] not yet implemented in path parser"]
fn test_json_path_array_append() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entity = data.get_entities(0).iter()
            .find(|e| !e.key.is_empty())
            .unwrap();

        let document = obj([
            ("items", Value::Array(vec![
                Value::Int(1),
                Value::Int(2),
            ])),
        ]);
        db.json_set(&run, &entity.key, "$", document).unwrap();

        // Append to array
        db.json_set(&run, &entity.key, "items[-]", Value::Int(3)).unwrap();

        // Verify append
        let items = db.json_get(&run, &entity.key, "items").unwrap().unwrap();
        assert_eq!(items.value, Value::Array(vec![
            Value::Int(1),
            Value::Int(2),
            Value::Int(3),
        ]));
    });
}

/// Test mixed object and array path using test data
#[test]
fn test_json_path_mixed() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entity = data.get_entities(0).iter()
            .find(|e| !e.key.is_empty())
            .unwrap();

        let document = obj([
            ("users", Value::Array(vec![
                obj([("name", Value::String("User1".to_string()))]),
                obj([("name", Value::String("User2".to_string()))]),
            ])),
        ]);
        db.json_set(&run, &entity.key, "$", document).unwrap();

        // Access object within array
        let name1 = db.json_get(&run, &entity.key, "users[0].name").unwrap().unwrap();
        let name2 = db.json_get(&run, &entity.key, "users[1].name").unwrap().unwrap();

        assert_eq!(name1.value, Value::String("User1".to_string()));
        assert_eq!(name2.value, Value::String("User2".to_string()));
    });
}

/// Test deleting array element (shifts remaining) using test data
#[test]
fn test_json_path_delete_array_element() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entity = data.get_entities(0).iter()
            .find(|e| !e.key.is_empty())
            .unwrap();

        let document = obj([
            ("items", Value::Array(vec![
                Value::String("a".to_string()),
                Value::String("b".to_string()),
                Value::String("c".to_string()),
            ])),
        ]);
        db.json_set(&run, &entity.key, "$", document).unwrap();

        // Delete middle element
        db.json_delete(&run, &entity.key, "items[1]").unwrap();

        // Verify remaining elements shifted
        let items = db.json_get(&run, &entity.key, "items").unwrap().unwrap();
        assert_eq!(items.value, Value::Array(vec![
            Value::String("a".to_string()),
            Value::String("c".to_string()),
        ]));
    });
}

/// Test deleting nested object field using test data
#[test]
fn test_json_path_delete_nested_field() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entity = data.get_entities(0).iter()
            .find(|e| !e.key.is_empty())
            .unwrap();

        let document = obj([
            ("user", obj([
                ("name", Value::String("Dave".to_string())),
                ("email", Value::String("dave@example.com".to_string())),
            ])),
        ]);
        db.json_set(&run, &entity.key, "$", document).unwrap();

        // Delete nested field
        db.json_delete(&run, &entity.key, "user.email").unwrap();

        // Verify field deleted but parent remains
        assert!(db.json_get(&run, &entity.key, "user.email").unwrap().is_none());
        assert!(db.json_get(&run, &entity.key, "user.name").unwrap().is_some());
    });
}

/// Test out-of-bounds array access using test data
#[test]
fn test_json_path_array_out_of_bounds() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entity = data.get_entities(0).iter()
            .find(|e| !e.key.is_empty())
            .unwrap();

        let document = obj([
            ("items", Value::Array(vec![
                Value::Int(1),
                Value::Int(2),
            ])),
        ]);
        db.json_set(&run, &entity.key, "$", document).unwrap();

        // Access out of bounds - should return None
        let result = db.json_get(&run, &entity.key, "items[99]").unwrap();
        assert!(result.is_none(), "Out of bounds access should return None");
    });
}

/// Test replacing entire array using test data
#[test]
fn test_json_path_replace_array() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entity = data.get_entities(0).iter()
            .find(|e| !e.key.is_empty())
            .unwrap();

        let document = obj([
            ("items", Value::Array(vec![
                Value::Int(1),
                Value::Int(2),
            ])),
        ]);
        db.json_set(&run, &entity.key, "$", document).unwrap();

        // Replace entire array
        let new_array = Value::Array(vec![
            Value::String("new".to_string()),
        ]);
        db.json_set(&run, &entity.key, "items", new_array.clone()).unwrap();

        let result = db.json_get(&run, &entity.key, "items").unwrap().unwrap();
        assert_eq!(result.value, new_array);
    });
}

/// Test path operations with dirty data (unicode keys)
#[test]
fn test_json_path_dirty_data() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Get dirty entities with unicode
        let dirty: Vec<_> = data.dirty_entities().into_iter()
            .filter(|(_, e)| !e.key.is_empty())
            .take(10)
            .collect();

        for (_, entity) in &dirty {
            // Create document from dirty entity
            db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();

            // Verify we can read it back at root
            let result = db.json_get(&run, &entity.key, "$").unwrap();
            assert!(result.is_some(), "Should be able to read dirty data for key '{}'", entity.key);
            assert_eq!(result.unwrap().value, entity.value);
        }
    });
}

/// Test path operations with arrays from test data
#[test]
fn test_json_path_array_from_test_data() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Get array entities
        let arrays: Vec<_> = data.array_entities().into_iter()
            .filter(|(_, e)| !e.key.is_empty())
            .take(5)
            .collect();

        for (_, entity) in &arrays {
            if let Value::Array(arr) = &entity.value {
                if arr.is_empty() {
                    continue;
                }

                // Create document with array
                db.json_set(&run, &entity.key, "$", obj([
                    ("data", entity.value.clone())
                ])).unwrap();

                // Access first element
                let first = db.json_get(&run, &entity.key, "data[0]").unwrap();
                assert!(first.is_some(), "First element should exist for key '{}'", entity.key);
                assert_eq!(first.unwrap().value, arr[0]);
            }
        }
    });
}
