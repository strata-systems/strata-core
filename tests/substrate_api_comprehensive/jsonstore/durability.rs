//! JsonStore Durability Tests
//!
//! Tests for durability and crash recovery:
//! - Data persistence across restarts
//! - Complex documents persistence
//!
//! All tests use dirty test data from fixtures/dirty_jsonstore_data.json

use crate::*;
use crate::test_data::{load_jsonstore_test_data, JsonStoreTestData};
use std::sync::OnceLock;
use tempfile::tempdir;

/// Lazily loaded test data (shared across tests)
fn test_data() -> &'static JsonStoreTestData {
    static DATA: OnceLock<JsonStoreTestData> = OnceLock::new();
    DATA.get_or_init(|| load_jsonstore_test_data())
}

/// Test basic document persistence after restart using test data
#[test]
fn test_json_persist_after_restart() {
    let data = test_data();
    let dir = tempdir().unwrap();
    let path = dir.path();

    let run = ApiRunId::default_run_id();

    let entity = data.get_entities(0).iter()
        .find(|e| !e.key.is_empty())
        .unwrap();

    // Write document and close
    {
        let db = create_persistent_db(path);
        db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();
    }

    // Reopen and verify
    {
        let db = create_persistent_db(path);
        let result = db.json_get(&run, &entity.key, "$").unwrap();
        assert!(result.is_some(), "Document should persist after restart");
        assert_eq!(result.unwrap().value, entity.value);
    }
}

/// Test nested document persistence using test data
#[test]
fn test_json_nested_persist() {
    let data = test_data();
    let dir = tempdir().unwrap();
    let path = dir.path();

    let run = ApiRunId::default_run_id();

    // Find an object entity with nested structure
    let entity = data.object_entities().into_iter()
        .find(|(_, e)| !e.key.is_empty())
        .map(|(_, e)| e)
        .unwrap();

    // Write nested document
    {
        let db = create_persistent_db(path);
        db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();
    }

    // Reopen and verify
    {
        let db = create_persistent_db(path);
        let result = db.json_get(&run, &entity.key, "$").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, entity.value);
    }
}

/// Test multiple documents persist using test data
#[test]
fn test_json_multiple_persist() {
    let data = test_data();
    let dir = tempdir().unwrap();
    let path = dir.path();

    let run = ApiRunId::default_run_id();

    let entities: Vec<_> = data.get_entities(0).iter()
        .filter(|e| !e.key.is_empty())
        .take(10)
        .collect();

    // Write multiple documents
    {
        let db = create_persistent_db(path);
        for entity in &entities {
            db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();
        }
    }

    // Reopen and verify all
    {
        let db = create_persistent_db(path);
        for entity in &entities {
            let result = db.json_get(&run, &entity.key, "$").unwrap();
            assert!(result.is_some(), "Entity '{}' should persist", entity.key);
            assert_eq!(result.unwrap().value, entity.value);
        }
    }
}

/// Test document updates persist using test data
#[test]
fn test_json_updates_persist() {
    let data = test_data();
    let dir = tempdir().unwrap();
    let path = dir.path();

    let run = ApiRunId::default_run_id();

    let entities: Vec<_> = data.get_entities(0).iter()
        .filter(|e| !e.key.is_empty())
        .take(3)
        .collect();

    let key = &entities[0].key;

    // Create and update document
    {
        let db = create_persistent_db(path);
        db.json_set(&run, key, "$", entities[0].value.clone()).unwrap();

        // Update with different values
        for entity in entities.iter().skip(1) {
            db.json_set(&run, key, "$", entity.value.clone()).unwrap();
        }
    }

    // Reopen and verify final value
    {
        let db = create_persistent_db(path);
        let result = db.json_get(&run, key, "$").unwrap().unwrap();
        // Final value should be the last entity's value
        assert_eq!(result.value, entities.last().unwrap().value);
    }
}

/// Test merge operations persist using test data
#[test]
fn test_json_merge_persist() {
    let data = test_data();
    let dir = tempdir().unwrap();
    let path = dir.path();

    let run = ApiRunId::default_run_id();

    // Get object entities for merge testing
    let objects: Vec<_> = data.object_entities().into_iter()
        .filter(|(_, e)| !e.key.is_empty())
        .map(|(_, e)| e)
        .take(2)
        .collect();

    if objects.len() < 2 {
        return; // Skip if not enough objects
    }

    let key = &objects[0].key;

    // Create and merge
    {
        let db = create_persistent_db(path);
        db.json_set(&run, key, "$", objects[0].value.clone()).unwrap();

        // Merge with another object
        if let Value::Object(_) = &objects[1].value {
            db.json_merge(&run, key, "$", objects[1].value.clone()).unwrap();
        }
    }

    // Reopen and verify document exists
    {
        let db = create_persistent_db(path);
        let result = db.json_get(&run, key, "$").unwrap();
        assert!(result.is_some(), "Merged document should persist");
    }
}

/// Test run isolation persists using test data
#[test]
fn test_json_run_isolation_persists() {
    let data = test_data();
    let dir = tempdir().unwrap();
    let path = dir.path();

    let run1 = ApiRunId::default_run_id();
    let run2 = ApiRunId::new();

    let entities_run1: Vec<_> = data.get_entities(0).iter()
        .filter(|e| !e.key.is_empty())
        .take(3)
        .collect();
    let entities_run2: Vec<_> = data.get_entities(1).iter()
        .filter(|e| !e.key.is_empty())
        .take(3)
        .collect();

    // Create documents in different runs
    {
        let db = create_persistent_db(path);
        db.run_create(Some(&run2), None).unwrap();

        for entity in &entities_run1 {
            db.json_set(&run1, &entity.key, "$", entity.value.clone()).unwrap();
        }

        for entity in &entities_run2 {
            db.json_set(&run2, &entity.key, "$", entity.value.clone()).unwrap();
        }
    }

    // Reopen and verify isolation
    {
        let db = create_persistent_db(path);

        for entity in &entities_run1 {
            let result = db.json_get(&run1, &entity.key, "$").unwrap();
            assert!(result.is_some(), "Run1 entity '{}' should persist", entity.key);
            assert_eq!(result.unwrap().value, entity.value);
        }

        for entity in &entities_run2 {
            let result = db.json_get(&run2, &entity.key, "$").unwrap();
            assert!(result.is_some(), "Run2 entity '{}' should persist", entity.key);
            assert_eq!(result.unwrap().value, entity.value);
        }
    }
}

/// Test delete operations persist using test data
#[test]
fn test_json_delete_persist() {
    let data = test_data();
    let dir = tempdir().unwrap();
    let path = dir.path();

    let run = ApiRunId::default_run_id();

    let entities: Vec<_> = data.get_entities(0).iter()
        .filter(|e| !e.key.is_empty())
        .take(3)
        .collect();

    // Create documents and delete one
    {
        let db = create_persistent_db(path);

        for entity in &entities {
            db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();
        }

        // Delete the second document
        db.json_delete(&run, &entities[1].key, "$").unwrap();
    }

    // Reopen and verify deletion persisted
    {
        let db = create_persistent_db(path);
        assert!(db.json_get(&run, &entities[0].key, "$").unwrap().is_some());
        assert!(db.json_get(&run, &entities[1].key, "$").unwrap().is_none(), "Deleted doc should stay deleted");
        assert!(db.json_get(&run, &entities[2].key, "$").unwrap().is_some());
    }
}

/// Test dirty data persists correctly (unicode, special chars)
#[test]
fn test_json_dirty_data_persist() {
    let data = test_data();
    let dir = tempdir().unwrap();
    let path = dir.path();

    let run = ApiRunId::default_run_id();

    // Get dirty entities with unicode, special chars, etc.
    let dirty_entities: Vec<_> = data.dirty_entities().into_iter()
        .filter(|(_, e)| !e.key.is_empty())
        .take(20)
        .collect();

    // Write dirty documents
    {
        let db = create_persistent_db(path);
        for (_, entity) in &dirty_entities {
            db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();
        }
    }

    // Reopen and verify all dirty data persists correctly
    {
        let db = create_persistent_db(path);
        for (_, entity) in &dirty_entities {
            let result = db.json_get(&run, &entity.key, "$").unwrap();
            assert!(result.is_some(), "Dirty entity '{}' should persist", entity.key);
            assert_eq!(result.unwrap().value, entity.value, "Dirty data should match for '{}'", entity.key);
        }
    }
}
