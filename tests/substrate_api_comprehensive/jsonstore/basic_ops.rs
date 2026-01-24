//! JsonStore Basic Operations Tests
//!
//! Tests for fundamental JsonStore operations:
//! - json_set / json_get
//! - json_delete
//! - json_exists
//! - json_get_version
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

/// Test basic set and get operations using test data entities
#[test]
fn test_json_set_get_from_test_data() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Use first 50 entities from run 0 (hand-crafted edge cases)
        for entity in data.sample(0, 50) {
            // Skip empty keys for this basic test
            if entity.key.is_empty() {
                continue;
            }

            // Set the document
            let result = db.json_set(&run, &entity.key, "$", entity.value.clone());
            assert!(result.is_ok(), "Should set key '{}': {:?}", entity.key, result);

            // Get and verify
            let retrieved = db.json_get(&run, &entity.key, "$").unwrap();
            assert!(retrieved.is_some(), "Document '{}' should exist", entity.key);
            assert_eq!(retrieved.unwrap().value, entity.value, "Value mismatch for key '{}'", entity.key);
        }
    });
}

/// Test setting nested fields with test data
#[test]
fn test_json_set_nested_field() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Use object entities that have nested structure
        for (_, entity) in data.object_entities().into_iter().take(20) {
            if entity.key.is_empty() {
                continue;
            }

            // Set the base document
            db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();

            // Try to add a nested field
            db.json_set(&run, &entity.key, "test_nested", Value::String("added".to_string())).unwrap();

            // Verify nested field was added
            let result = db.json_get(&run, &entity.key, "test_nested").unwrap();
            assert!(result.is_some(), "Nested field should exist for '{}'", entity.key);
            assert_eq!(result.unwrap().value, Value::String("added".to_string()));
        }
    });
}

/// Test getting a non-existent document
#[test]
fn test_json_get_nonexistent() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let result = db.json_get(&run, "nonexistent_doc_xyz_123", "$").unwrap();
        assert!(result.is_none(), "Non-existent document should return None");
    });
}

/// Test getting a non-existent path in existing document
#[test]
fn test_json_get_nonexistent_path() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Use an entity from test data
        let entity = &data.get_entities(0)[0];
        db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();

        // Get non-existent path
        let result = db.json_get(&run, &entity.key, "nonexistent_path_xyz").unwrap();
        assert!(result.is_none(), "Missing path should return None");
    });
}

/// Test deleting a field using test data
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

/// Test deleting non-existent path (idempotent operation)
#[test]
fn test_json_delete_nonexistent_path() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Use an entity from test data
        let entity = &data.get_entities(0)[0];
        db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();

        // Delete non-existent path - idempotent operation succeeds
        let count = db.json_delete(&run, &entity.key, "nonexistent_field_xyz").unwrap();
        assert_eq!(count, 1, "Delete is idempotent, returns 1 even for non-existent path");
    });
}

/// Test json_exists operation with test data
#[test]
fn test_json_exists() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Use entities from test data with unique prefix to avoid key collision
        for (i, entity) in data.sample(0, 30).into_iter().enumerate() {
            if entity.key.is_empty() {
                continue;
            }

            // Create unique key for this test
            let key = format!("exists_test_{}_{}", i, entity.key);

            // Should not exist initially
            assert!(!db.json_exists(&run, &key).unwrap(), "Key '{}' should not exist initially", key);

            // Create document
            db.json_set(&run, &key, "$", entity.value.clone()).unwrap();

            // Should exist now
            assert!(db.json_exists(&run, &key).unwrap(), "Key '{}' should exist after set", key);
        }
    });
}

/// Test json_get_version operation
#[test]
fn test_json_get_version() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Use an entity from test data
        let entity = &data.get_entities(0)[0];

        // No version for non-existent doc
        assert!(db.json_get_version(&run, &entity.key).unwrap().is_none());

        // Create document
        db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();
        let v1 = db.json_get_version(&run, &entity.key).unwrap();
        assert!(v1.is_some(), "Version should exist");

        // Update document
        db.json_set(&run, &entity.key, "$", obj([("updated", Value::Int(1))])).unwrap();
        let v2 = db.json_get_version(&run, &entity.key).unwrap();
        assert!(v2.is_some());
    });
}

/// Test overwriting entire document with test data
#[test]
fn test_json_overwrite_document() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Get two different entities
        let entities = data.get_entities(0);
        if entities.len() < 2 {
            return;
        }

        let key = "overwrite_test_key";

        // Create initial document from first entity
        db.json_set(&run, key, "$", entities[0].value.clone()).unwrap();

        // Overwrite with second entity's value
        db.json_set(&run, key, "$", entities[1].value.clone()).unwrap();

        // Verify overwrite
        let result = db.json_get(&run, key, "$").unwrap().unwrap();
        assert_eq!(result.value, entities[1].value);
    });
}

/// Test all value types from dirty test data
#[test]
fn test_json_all_value_types() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Test null entities
        for (_, entity) in data.null_entities().into_iter().take(5) {
            if entity.key.is_empty() {
                continue;
            }
            db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();
            let result = db.json_get(&run, &entity.key, "$").unwrap().unwrap();
            assert_eq!(result.value, Value::Null, "Null value mismatch for '{}'", entity.key);
        }

        // Test array entities
        for (_, entity) in data.array_entities().into_iter().take(5) {
            if entity.key.is_empty() {
                continue;
            }
            db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();
            let result = db.json_get(&run, &entity.key, "$").unwrap().unwrap();
            assert_eq!(result.value, entity.value, "Array value mismatch for '{}'", entity.key);
        }

        // Test object entities
        for (_, entity) in data.object_entities().into_iter().take(5) {
            if entity.key.is_empty() {
                continue;
            }
            db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();
            let result = db.json_get(&run, &entity.key, "$").unwrap().unwrap();
            assert_eq!(result.value, entity.value, "Object value mismatch for '{}'", entity.key);
        }
    });
}

/// Test run isolation for JSON documents using multiple runs from test data
#[test]
fn test_json_run_isolation() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run1 = ApiRunId::default_run_id();
        let run2 = ApiRunId::new();

        // Get entities from different runs in test data
        let entities_run1 = data.get_entities(0);
        let entities_run2 = data.get_entities(1);

        if entities_run1.is_empty() || entities_run2.is_empty() {
            return;
        }

        let shared_key = "shared_isolation_key";

        // Create documents with same key in different runs
        db.json_set(&run1, shared_key, "$", entities_run1[0].value.clone()).unwrap();
        db.json_set(&run2, shared_key, "$", entities_run2[0].value.clone()).unwrap();

        // Verify isolation
        assert_eq!(db.json_get(&run1, shared_key, "$").unwrap().unwrap().value, entities_run1[0].value);
        assert_eq!(db.json_get(&run2, shared_key, "$").unwrap().unwrap().value, entities_run2[0].value);
    });
}

/// Test multiple documents from test data
#[test]
fn test_json_multiple_documents() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Load 100 entities from test data with unique keys
        let entities: Vec<_> = data.get_entities(0).iter()
            .filter(|e| !e.key.is_empty())
            .take(100)
            .enumerate()
            .collect();

        // Create all documents with unique prefixed keys
        for (i, entity) in &entities {
            let key = format!("multi_doc_{}_{}", i, entity.key);
            db.json_set(&run, &key, "$", entity.value.clone()).unwrap();
        }

        // Verify all documents
        for (i, entity) in &entities {
            let key = format!("multi_doc_{}_{}", i, entity.key);
            let result = db.json_get(&run, &key, "$").unwrap();
            assert!(result.is_some(), "Document '{}' should exist", key);
            assert_eq!(result.unwrap().value, entity.value, "Value mismatch for '{}'", key);
        }
    });
}
