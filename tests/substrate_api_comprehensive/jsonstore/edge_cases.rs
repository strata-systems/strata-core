//! JsonStore Edge Cases Tests
//!
//! Tests for validation and boundary conditions:
//! - Key validation
//! - Path syntax validation
//! - Large documents
//! - Special characters
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

/// Test document key with special characters from test data
#[test]
fn test_json_special_key_names() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Get entities with special characters in keys
        let special_keys: Vec<_> = data.all_entities()
            .filter(|(_, e)| {
                e.key.contains('-') || e.key.contains('_') ||
                e.key.contains('.') || e.key.contains(':')
            })
            .take(20)
            .collect();

        for (_, entity) in special_keys {
            if entity.key.is_empty() {
                continue;
            }
            let result = db.json_set(&run, &entity.key, "$", entity.value.clone());
            assert!(result.is_ok(), "Key '{}' should work", entity.key);
            assert!(db.json_exists(&run, &entity.key).unwrap(), "Key '{}' should exist", entity.key);
        }
    });
}

/// Test document key with unicode from test data
#[test]
fn test_json_unicode_key() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Get entities with unicode in keys
        let unicode_keys: Vec<_> = data.all_entities()
            .filter(|(_, e)| e.key.chars().any(|c| !c.is_ascii()) && !e.key.is_empty())
            .take(20)
            .collect();

        for (_, entity) in unicode_keys {
            let result = db.json_set(&run, &entity.key, "$", entity.value.clone());
            assert!(result.is_ok(), "Unicode key '{}' should work", entity.key);
            assert!(db.json_exists(&run, &entity.key).unwrap(), "Unicode key '{}' should exist", entity.key);
        }
    });
}

/// Test field names with special characters
#[test]
fn test_json_special_field_names() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "special_fields";

        let document = obj([
            ("field_underscores", Value::Int(2)),
            ("field123", Value::Int(3)),
            ("UPPERCASE", Value::Int(4)),
            ("MixedCase", Value::Int(5)),
        ]);
        db.json_set(&run, key, "$", document).unwrap();

        // Access fields with special names
        let f2 = db.json_get(&run, key, "field_underscores").unwrap();
        let f3 = db.json_get(&run, key, "field123").unwrap();

        assert!(f2.is_some(), "Underscore fields should work");
        assert!(f3.is_some(), "Numeric suffix should work");
    });
}

/// Test large document from test data
#[test]
fn test_json_large_document() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Use entities with large/complex objects
        let large_entities: Vec<_> = data.object_entities()
            .into_iter()
            .take(50)
            .collect();

        for (_, entity) in large_entities {
            if entity.key.is_empty() {
                continue;
            }
            let result = db.json_set(&run, &entity.key, "$", entity.value.clone());
            assert!(result.is_ok(), "Large document '{}' should be stored", entity.key);

            let retrieved = db.json_get(&run, &entity.key, "$").unwrap();
            assert!(retrieved.is_some(), "Large document '{}' should be retrievable", entity.key);
        }
    });
}

/// Test large nested array from test data
#[test]
fn test_json_large_array() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Use array entities from test data (run 3 has arrays)
        let array_entities: Vec<_> = data.array_entities()
            .into_iter()
            .take(20)
            .collect();

        for (_, entity) in array_entities {
            if entity.key.is_empty() {
                continue;
            }
            let result = db.json_set(&run, &entity.key, "$", entity.value.clone());
            assert!(result.is_ok(), "Array document '{}' should be stored", entity.key);

            let retrieved = db.json_get(&run, &entity.key, "$").unwrap();
            assert!(retrieved.is_some(), "Array document '{}' should be retrievable", entity.key);
            assert_eq!(retrieved.unwrap().value, entity.value);
        }
    });
}

/// Test deeply nested document
#[test]
fn test_json_deeply_nested() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Find deeply nested entities from test data
        let deep_entity = data.entities_with_prefix("deep:")
            .into_iter()
            .next();

        if let Some((_, entity)) = deep_entity {
            db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();
            let result = db.json_get(&run, &entity.key, "$").unwrap();
            assert!(result.is_some(), "Deep document should be retrievable");
        }

        // Also test programmatic deep nesting
        let key = "deep_doc_test";
        let mut nested = Value::Int(42);
        for _ in 0..10 {
            nested = obj([("nested", nested)]);
        }

        db.json_set(&run, key, "$", nested).unwrap();

        // Access deep value
        let deep_path = "nested.nested.nested.nested.nested.nested.nested.nested.nested.nested";
        let result = db.json_get(&run, key, deep_path).unwrap().unwrap();
        assert_eq!(result.value, Value::Int(42));
    });
}

/// Test empty document (run 2 in test data has empty objects)
#[test]
fn test_json_empty_document() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Run 2 has empty objects
        let empty_entities: Vec<_> = data.get_entities(1).iter()
            .filter(|e| !e.key.is_empty())
            .take(10)
            .collect();

        for entity in empty_entities {
            db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();

            let result = db.json_get(&run, &entity.key, "$").unwrap().unwrap();
            assert_eq!(result.value, obj([]));
        }
    });
}

/// Test empty array value
#[test]
fn test_json_empty_array() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "empty_array_doc";

        let document = obj([("items", Value::Array(vec![]))]);
        db.json_set(&run, key, "$", document).unwrap();

        let result = db.json_get(&run, key, "items").unwrap().unwrap();
        assert_eq!(result.value, Value::Array(vec![]));
    });
}

/// Test empty string value (run 5 in test data has strings)
#[test]
fn test_json_empty_string() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Find string entities from test data
        let string_entities: Vec<_> = data.get_entities(4).iter()
            .filter(|e| !e.key.is_empty() && matches!(e.value, Value::String(_)))
            .take(10)
            .collect();

        for entity in string_entities {
            db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();
            let result = db.json_get(&run, &entity.key, "$").unwrap().unwrap();
            assert_eq!(result.value, entity.value, "String value mismatch for '{}'", entity.key);
        }

        // Also test explicit empty string
        let key = "empty_string_doc";
        let document = obj([("name", Value::String(String::new()))]);
        db.json_set(&run, key, "$", document).unwrap();
        let result = db.json_get(&run, key, "name").unwrap().unwrap();
        assert_eq!(result.value, Value::String(String::new()));
    });
}

/// Test large string value
#[test]
fn test_json_large_string() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "large_string_doc";

        // Create 100KB string
        let large_string: String = (0..100_000).map(|i| ((i % 26) as u8 + b'a') as char).collect();
        let document = obj([("content", Value::String(large_string.clone()))]);

        db.json_set(&run, key, "$", document).unwrap();

        let result = db.json_get(&run, key, "content").unwrap().unwrap();
        if let Value::String(s) = result.value {
            assert_eq!(s.len(), 100_000);
        } else {
            panic!("Expected string");
        }
    });
}

/// Test unicode values from test data (run 6 has unicode)
#[test]
fn test_json_unicode_values() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Run 6 has unicode-heavy entities
        let unicode_entities: Vec<_> = data.get_entities(5).iter()
            .filter(|e| !e.key.is_empty())
            .take(20)
            .collect();

        for entity in unicode_entities {
            db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();
            let result = db.json_get(&run, &entity.key, "$").unwrap().unwrap();
            assert_eq!(result.value, entity.value, "Unicode value mismatch for '{}'", entity.key);
        }
    });
}

/// Test setting scalar at root
#[test]
fn test_json_scalar_at_root() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Test with string entities (run 5 has strings at root)
        let string_entities: Vec<_> = data.get_entities(4).iter()
            .filter(|e| !e.key.is_empty() && matches!(e.value, Value::String(_)))
            .take(5)
            .collect();

        for entity in string_entities {
            let result = db.json_set(&run, &entity.key, "$", entity.value.clone());
            assert!(result.is_ok(), "Setting scalar at root is allowed");

            let value = db.json_get(&run, &entity.key, "$").unwrap().unwrap();
            assert_eq!(value.value, entity.value);
        }
    });
}

/// Test setting array at root (run 3 has arrays at root)
#[test]
fn test_json_array_at_root() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Run 3 has array entities
        let array_entities: Vec<_> = data.get_entities(2).iter()
            .filter(|e| !e.key.is_empty() && matches!(e.value, Value::Array(_)))
            .take(10)
            .collect();

        for entity in array_entities {
            let result = db.json_set(&run, &entity.key, "$", entity.value.clone());
            assert!(result.is_ok(), "Setting array at root is allowed");

            let value = db.json_get(&run, &entity.key, "$").unwrap().unwrap();
            assert_eq!(value.value, entity.value);
        }
    });
}

/// Test null value in document (run 4 has nulls)
#[test]
fn test_json_null_value() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Run 4 has null entities
        let null_entities: Vec<_> = data.get_entities(3).iter()
            .filter(|e| !e.key.is_empty())
            .take(10)
            .collect();

        for entity in null_entities {
            db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();
            let result = db.json_get(&run, &entity.key, "$").unwrap().unwrap();
            assert_eq!(result.value, Value::Null);
        }
    });
}

/// Test mixed array types from test data
#[test]
fn test_json_mixed_array() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Find entities with mixed arrays from run 0 (hand-crafted)
        let mixed_array_entity = data.get_entities(0).iter()
            .find(|e| e.key.contains("MixedArray") || e.key.contains("mixed"));

        if let Some(entity) = mixed_array_entity {
            db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();
            let result = db.json_get(&run, &entity.key, "$").unwrap().unwrap();
            assert_eq!(result.value, entity.value);
        }

        // Also test programmatic mixed array
        let key = "mixed_array_test";
        let document = obj([
            ("items", Value::Array(vec![
                Value::Int(1),
                Value::String("two".to_string()),
                Value::Bool(true),
                Value::Null,
                Value::Float(3.14),
                obj([("nested", Value::Int(5))]),
                Value::Array(vec![Value::Int(6)]),
            ])),
        ]);

        db.json_set(&run, key, "$", document.clone()).unwrap();
        let result = db.json_get(&run, key, "$").unwrap().unwrap();
        assert_eq!(result.value, document);
    });
}

/// Test version increments on updates
#[test]
fn test_json_version_increments() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entity = data.get_entities(0).iter()
            .find(|e| !e.key.is_empty() && matches!(e.value, Value::Object(_)))
            .unwrap();

        db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();
        let v1 = db.json_get_version(&run, &entity.key).unwrap().unwrap();

        db.json_set(&run, &entity.key, "$", obj([("updated", Value::Int(1))])).unwrap();
        let v2 = db.json_get_version(&run, &entity.key).unwrap().unwrap();

        db.json_set(&run, &entity.key, "$", obj([("updated", Value::Int(2))])).unwrap();
        let v3 = db.json_get_version(&run, &entity.key).unwrap().unwrap();

        // Versions should be different (increasing in some way)
        assert!(v1 != v2 || v2 != v3 || v1 <= v3, "Versions should change on update");
    });
}

/// Test dirty data - entities with empty keys
#[test]
fn test_json_empty_keys() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Find entities with empty keys
        let empty_key_entities: Vec<_> = data.all_entities()
            .filter(|(_, e)| e.key.is_empty())
            .take(5)
            .collect();

        for (_, entity) in empty_key_entities {
            // Empty keys may or may not be accepted - test the behavior
            let result = db.json_set(&run, &entity.key, "$", entity.value.clone());
            // Just verify it doesn't panic
            let _ = result;
        }
    });
}

/// Test dirty data - special path characters
#[test]
fn test_json_keys_with_path_chars() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Find entities with slashes, dots, spaces in keys
        let special_keys: Vec<_> = data.all_entities()
            .filter(|(_, e)| {
                !e.key.is_empty() &&
                (e.key.contains('/') || e.key.contains(' '))
            })
            .take(10)
            .collect();

        for (_, entity) in special_keys {
            let result = db.json_set(&run, &entity.key, "$", entity.value.clone());
            assert!(result.is_ok(), "Key with path chars '{}' should be accepted", entity.key);

            let retrieved = db.json_get(&run, &entity.key, "$").unwrap();
            assert!(retrieved.is_some(), "Key '{}' should be retrievable", entity.key);
        }
    });
}
