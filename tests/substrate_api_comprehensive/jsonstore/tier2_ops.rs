//! JsonStore Tier 2 Operations Tests
//!
//! Tests for M11B Tier 2 features:
//! - json_count: Document count
//! - json_batch_get: Batch document retrieval
//! - json_batch_create: Atomic batch document creation
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

// =============================================================================
// Count Tests
// =============================================================================

/// Test count on empty run
#[test]
fn test_json_count_empty_run() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let count = db.json_count(&run).unwrap();
        assert_eq!(count, 0, "Empty run should have 0 documents");
    });
}

/// Test count increases with creates using test data
#[test]
fn test_json_count_increases_with_creates() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entities: Vec<_> = data.get_entities(0).iter()
            .filter(|e| !e.key.is_empty())
            .take(5)
            .collect();

        assert_eq!(db.json_count(&run).unwrap(), 0);

        for (i, entity) in entities.iter().enumerate() {
            db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();
            assert_eq!(db.json_count(&run).unwrap(), (i + 1) as u64);
        }
    });
}

/// Test count after creates and deletes
#[test]
fn test_json_count_after_creates_and_deletes() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entities: Vec<_> = data.get_entities(0).iter()
            .filter(|e| !e.key.is_empty())
            .take(5)
            .collect();

        // Create 5 documents
        for entity in &entities {
            db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();
        }
        assert_eq!(db.json_count(&run).unwrap(), 5);

        // Delete 2 documents
        db.json_delete(&run, &entities[1].key, "$").unwrap();
        db.json_delete(&run, &entities[3].key, "$").unwrap();
        assert_eq!(db.json_count(&run).unwrap(), 3);

        // Add one more
        db.json_set(&run, "new_doc", "$", Value::Int(5)).unwrap();
        assert_eq!(db.json_count(&run).unwrap(), 4);
    });
}

/// Test count with run isolation
#[test]
fn test_json_count_run_isolation() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run1 = ApiRunId::default_run_id();
        let run2 = ApiRunId::new();

        let entities_run1: Vec<_> = data.get_entities(0).iter()
            .filter(|e| !e.key.is_empty())
            .take(3)
            .collect();
        let entities_run2: Vec<_> = data.get_entities(1).iter()
            .filter(|e| !e.key.is_empty())
            .take(2)
            .collect();

        // Create documents in run1
        for entity in &entities_run1 {
            db.json_set(&run1, &entity.key, "$", entity.value.clone()).unwrap();
        }

        // Create documents in run2
        for entity in &entities_run2 {
            db.json_set(&run2, &entity.key, "$", entity.value.clone()).unwrap();
        }

        assert_eq!(db.json_count(&run1).unwrap(), entities_run1.len() as u64, "Run1 count mismatch");
        assert_eq!(db.json_count(&run2).unwrap(), entities_run2.len() as u64, "Run2 count mismatch");
    });
}

// =============================================================================
// Batch Get Tests
// =============================================================================

/// Test batch get returns documents from test data
#[test]
fn test_json_batch_get_returns_documents() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entities: Vec<_> = data.get_entities(0).iter()
            .filter(|e| !e.key.is_empty())
            .take(5)
            .collect();

        // Create documents
        for entity in &entities {
            db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();
        }

        // Batch get
        let keys: Vec<_> = entities.iter().map(|e| e.key.as_str()).collect();
        let results = db.json_batch_get(&run, &keys).unwrap();
        assert_eq!(results.len(), entities.len());

        for (i, result) in results.iter().enumerate() {
            assert!(result.is_some(), "Result {} should exist", i);
            assert_eq!(result.as_ref().unwrap().value, entities[i].value);
        }
    });
}

/// Test batch get returns None for missing
#[test]
fn test_json_batch_get_returns_none_for_missing() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entities: Vec<_> = data.get_entities(0).iter()
            .filter(|e| !e.key.is_empty())
            .take(3)
            .collect();

        // Create only first document
        db.json_set(&run, &entities[0].key, "$", entities[0].value.clone()).unwrap();

        // Batch get with mix of existing and missing
        let keys: Vec<_> = entities.iter().map(|e| e.key.as_str()).collect();
        let results = db.json_batch_get(&run, &keys).unwrap();
        assert_eq!(results.len(), 3);

        assert!(results[0].is_some(), "First should exist");
        assert!(results[1].is_none(), "Second should be missing");
        assert!(results[2].is_none(), "Third should be missing");
    });
}

/// Test batch get preserves order
#[test]
fn test_json_batch_get_preserves_order() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entities: Vec<_> = data.get_entities(0).iter()
            .filter(|e| !e.key.is_empty())
            .take(5)
            .collect();

        // Create documents
        for entity in &entities {
            db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();
        }

        // Batch get in reverse order
        let keys: Vec<_> = entities.iter().rev().map(|e| e.key.as_str()).collect();
        let results = db.json_batch_get(&run, &keys).unwrap();
        assert_eq!(results.len(), entities.len());

        // Results should match reversed request order
        for (i, result) in results.iter().enumerate() {
            let expected_entity = &entities[entities.len() - 1 - i];
            assert_eq!(result.as_ref().unwrap().value, expected_entity.value);
        }
    });
}

/// Test batch get with empty keys
#[test]
fn test_json_batch_get_empty_keys() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let results = db.json_batch_get(&run, &[]).unwrap();
        assert!(results.is_empty(), "Empty input should return empty output");
    });
}

/// Test batch get with duplicate keys
#[test]
fn test_json_batch_get_duplicate_keys() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entity = data.get_entities(0).iter()
            .find(|e| !e.key.is_empty())
            .unwrap();

        db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();

        let results = db.json_batch_get(&run, &[&entity.key, &entity.key, &entity.key]).unwrap();
        assert_eq!(results.len(), 3);

        // All should be the same document
        for result in &results {
            assert!(result.is_some());
            assert_eq!(result.as_ref().unwrap().value, entity.value);
        }
    });
}

// =============================================================================
// Batch Create Tests
// =============================================================================

/// Test batch create creates documents from test data
#[test]
fn test_json_batch_create_creates_documents() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entities: Vec<_> = data.get_entities(0).iter()
            .filter(|e| !e.key.is_empty())
            .take(5)
            .collect();

        let docs: Vec<_> = entities.iter()
            .map(|e| (e.key.as_str(), e.value.clone()))
            .collect();

        let versions = db.json_batch_create(&run, docs).unwrap();
        assert_eq!(versions.len(), entities.len());

        // Verify all documents exist with correct values
        for entity in &entities {
            assert!(db.json_exists(&run, &entity.key).unwrap(), "Doc '{}' should exist", entity.key);
            let value = db.json_get(&run, &entity.key, "$").unwrap().unwrap().value;
            assert_eq!(value, entity.value, "Value mismatch for '{}'", entity.key);
        }
    });
}

/// Test batch create is atomic - fails if any exists
#[test]
fn test_json_batch_create_fails_if_any_exists() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entities: Vec<_> = data.get_entities(0).iter()
            .filter(|e| !e.key.is_empty())
            .take(3)
            .collect();

        // Pre-create one document
        db.json_set(&run, &entities[1].key, "$", Value::Int(0)).unwrap();

        // Try to batch create with one existing
        let docs: Vec<_> = entities.iter()
            .map(|e| (e.key.as_str(), e.value.clone()))
            .collect();

        let result = db.json_batch_create(&run, docs);
        assert!(result.is_err(), "Should fail because one key already exists");

        // Verify atomicity - first document should not exist
        assert!(!db.json_exists(&run, &entities[0].key).unwrap(), "First doc should not exist");
        assert!(!db.json_exists(&run, &entities[2].key).unwrap(), "Third doc should not exist");
    });
}

/// Test batch create is atomic - all or nothing
#[test]
fn test_json_batch_create_is_atomic() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let entities: Vec<_> = data.get_entities(0).iter()
            .filter(|e| !e.key.is_empty())
            .take(3)
            .collect();

        let docs: Vec<_> = entities.iter()
            .map(|e| (e.key.as_str(), e.value.clone()))
            .collect();

        db.json_batch_create(&run, docs).unwrap();

        // All should exist
        for entity in &entities {
            assert!(db.json_exists(&run, &entity.key).unwrap(), "Doc '{}' should exist", entity.key);
        }
    });
}

/// Test batch create with empty input
#[test]
fn test_json_batch_create_empty() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let versions = db.json_batch_create(&run, vec![]).unwrap();
        assert!(versions.is_empty(), "Empty input should return empty versions");
    });
}

/// Test batch create with dirty data from test file
#[test]
fn test_json_batch_create_dirty_data() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Mix of different value types from test data
        let mut docs: Vec<(&str, Value)> = Vec::new();

        // Add some null entities
        for (_, e) in data.null_entities().into_iter().filter(|(_, e)| !e.key.is_empty()).take(2) {
            docs.push((e.key.as_str(), e.value.clone()));
        }

        // Add some array entities
        for (_, e) in data.array_entities().into_iter().filter(|(_, e)| !e.key.is_empty()).take(2) {
            docs.push((e.key.as_str(), e.value.clone()));
        }

        // Add some object entities
        for (_, e) in data.object_entities().into_iter().filter(|(_, e)| !e.key.is_empty()).take(2) {
            docs.push((e.key.as_str(), e.value.clone()));
        }

        if !docs.is_empty() {
            let result = db.json_batch_create(&run, docs.clone());
            assert!(result.is_ok(), "Batch create with dirty data should succeed: {:?}", result);

            // Verify count
            let count = db.json_count(&run).unwrap();
            assert_eq!(count, docs.len() as u64);
        }
    });
}
