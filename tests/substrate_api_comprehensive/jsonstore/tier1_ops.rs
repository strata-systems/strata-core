//! JsonStore Tier 1 Operations Tests
//!
//! Tests for M11B Tier 1 features:
//! - json_list: Document listing with cursor-based pagination
//! - json_cas: Compare-and-swap for optimistic concurrency
//! - json_query: Exact field matching
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
// List Tests
// =============================================================================

/// Test basic list functionality with test data
#[test]
fn test_json_list_returns_documents() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Create documents from test data
        let entities: Vec<_> = data.get_entities(0).iter()
            .filter(|e| !e.key.is_empty())
            .take(10)
            .collect();

        for entity in &entities {
            db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();
        }

        // List all documents
        let result = db.json_list(&run, None, None, 100).unwrap();
        assert_eq!(result.keys.len(), entities.len(), "Should have {} documents", entities.len());
    });
}

/// Test list with limit enforces pagination
#[test]
fn test_json_list_pagination_works() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Create 10 documents from test data
        let entities: Vec<_> = data.get_entities(0).iter()
            .filter(|e| !e.key.is_empty())
            .take(10)
            .collect();

        for entity in &entities {
            db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();
        }

        // List with limit of 3
        let page1 = db.json_list(&run, None, None, 3).unwrap();
        assert_eq!(page1.keys.len(), 3, "First page should have 3 documents");
        assert!(page1.next_cursor.is_some(), "Should have next page cursor");

        // Get next page
        let page2 = db.json_list(&run, None, page1.next_cursor.as_deref(), 3).unwrap();
        assert_eq!(page2.keys.len(), 3, "Second page should have 3 documents");

        // Get remaining pages
        let mut cursor = page2.next_cursor;
        let mut total = page1.keys.len() + page2.keys.len();
        while cursor.is_some() {
            let page = db.json_list(&run, None, cursor.as_deref(), 3).unwrap();
            total += page.keys.len();
            cursor = page.next_cursor;
        }
        assert_eq!(total, entities.len(), "Should get all documents through pagination");
    });
}

/// Test list returns empty for empty store
#[test]
fn test_json_list_empty_store() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let result = db.json_list(&run, None, None, 10).unwrap();
        assert!(result.keys.is_empty(), "Empty store should return empty list");
        assert!(result.next_cursor.is_none(), "Should have no cursor");
    });
}

/// Test list with run isolation
#[test]
fn test_json_list_run_isolation() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run1 = ApiRunId::default_run_id();
        let run2 = ApiRunId::new();

        // Get entities from different test data runs
        let entities_run1: Vec<_> = data.get_entities(0).iter()
            .filter(|e| !e.key.is_empty())
            .take(5)
            .collect();
        let entities_run2: Vec<_> = data.get_entities(1).iter()
            .filter(|e| !e.key.is_empty())
            .take(3)
            .collect();

        // Create in run1
        for entity in &entities_run1 {
            db.json_set(&run1, &entity.key, "$", entity.value.clone()).unwrap();
        }

        // Create in run2
        for entity in &entities_run2 {
            db.json_set(&run2, &entity.key, "$", entity.value.clone()).unwrap();
        }

        // List should be isolated per run
        let result1 = db.json_list(&run1, None, None, 100).unwrap();
        let result2 = db.json_list(&run2, None, None, 100).unwrap();

        assert_eq!(result1.keys.len(), entities_run1.len(), "Run1 should have {} docs", entities_run1.len());
        assert_eq!(result2.keys.len(), entities_run2.len(), "Run2 should have {} docs", entities_run2.len());
    });
}

// =============================================================================
// CAS Tests
// =============================================================================

/// Test CAS succeeds with correct version
#[test]
fn test_json_cas_succeeds_with_correct_version() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Use an entity from test data
        let entity = data.get_entities(0).iter()
            .find(|e| !e.key.is_empty() && matches!(e.value, Value::Object(_)))
            .unwrap();

        // Create document
        db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();

        // Read current version
        let current = db.json_get(&run, &entity.key, "$").unwrap().unwrap();
        let version = match current.version {
            Version::Counter(v) => v,
            Version::Txn(v) => v,
            Version::Sequence(v) => v,
        };

        // CAS with correct version should succeed
        let new_version = db.json_cas(&run, &entity.key, version, "$", obj([("updated", Value::Bool(true))])).unwrap();
        assert!(matches!(new_version, Version::Counter(_) | Version::Txn(_) | Version::Sequence(_)));

        // Verify value was updated
        let updated = db.json_get(&run, &entity.key, "updated").unwrap().unwrap();
        assert_eq!(updated.value, Value::Bool(true));
    });
}

/// Test CAS fails with wrong version
#[test]
fn test_json_cas_fails_with_wrong_version() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Use an entity from test data
        let entity = data.get_entities(0).iter()
            .find(|e| !e.key.is_empty() && matches!(e.value, Value::Object(_)))
            .unwrap();

        // Create document
        db.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();

        // Try CAS with wrong version (0 when it should be higher)
        let result = db.json_cas(&run, &entity.key, 0, "$", obj([("should_fail", Value::Bool(true))]));
        assert!(result.is_err(), "CAS with wrong version should fail");
    });
}

/// Test CAS fails on non-existent document
#[test]
fn test_json_cas_fails_on_nonexistent() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let result = db.json_cas(&run, "nonexistent_cas_doc", 1, "field", Value::Int(1));
        assert!(result.is_err(), "CAS on non-existent doc should fail");
    });
}

/// Test concurrent CAS - exactly one wins
#[test]
fn test_concurrent_cas_exactly_one_wins() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;

    let data = test_data();
    let db = create_inmemory_db();
    let substrate = SubstrateImpl::new(db);
    let run = ApiRunId::default_run_id();

    // Use an entity from test data
    let entity = data.get_entities(0).iter()
        .find(|e| !e.key.is_empty() && matches!(e.value, Value::Object(_)))
        .unwrap();

    substrate.json_set(&run, &entity.key, "$", entity.value.clone()).unwrap();

    // Get initial version
    let initial = substrate.json_get(&run, &entity.key, "$").unwrap().unwrap();
    let initial_version = match initial.version {
        Version::Counter(v) => v,
        Version::Txn(v) => v,
        Version::Sequence(v) => v,
    };

    let success_count = Arc::new(AtomicUsize::new(0));
    let substrate = Arc::new(substrate);
    let key = entity.key.clone();

    // Spawn multiple threads trying to CAS with the same initial version
    let threads: Vec<_> = (0..5)
        .map(|i| {
            let substrate = substrate.clone();
            let success_count = success_count.clone();
            let run = run.clone();
            let key = key.clone();

            thread::spawn(move || {
                let result = substrate.json_cas(
                    &run,
                    &key,
                    initial_version,
                    "$",
                    obj([("winner", Value::Int(i + 1))]),
                );
                if result.is_ok() {
                    success_count.fetch_add(1, Ordering::SeqCst);
                }
            })
        })
        .collect();

    for t in threads {
        t.join().unwrap();
    }

    // Exactly one CAS should have succeeded
    assert_eq!(success_count.load(Ordering::SeqCst), 1, "Exactly one CAS should win");
}

// =============================================================================
// Query Tests
// =============================================================================

/// Test query returns matching documents using test data
#[test]
fn test_json_query_exact_match() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Create documents with a queryable field
        let entities: Vec<_> = data.get_entities(0).iter()
            .filter(|e| !e.key.is_empty())
            .take(10)
            .collect();

        // Set documents with a status field
        for (i, entity) in entities.iter().enumerate() {
            let status = if i % 2 == 0 { "active" } else { "inactive" };
            let mut doc = obj([("status", Value::String(status.to_string()))]);
            if let Value::Object(ref mut map) = doc {
                if let Value::Object(orig) = &entity.value {
                    map.extend(orig.clone());
                }
            }
            db.json_set(&run, &entity.key, "$", doc).unwrap();
        }

        // Query for active documents
        let results = db.json_query(&run, "status", Value::String("active".into()), 100).unwrap();
        assert_eq!(results.len(), 5, "Should find 5 active documents");

        // Query for inactive documents
        let results = db.json_query(&run, "status", Value::String("inactive".into()), 100).unwrap();
        assert_eq!(results.len(), 5, "Should find 5 inactive documents");
    });
}

/// Test query returns empty for no match
#[test]
fn test_json_query_returns_empty_for_no_match() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Create some documents from test data
        let entities: Vec<_> = data.get_entities(0).iter()
            .filter(|e| !e.key.is_empty())
            .take(5)
            .collect();

        for entity in &entities {
            db.json_set(&run, &entity.key, "$", obj([("status", Value::String("exists".into()))])).unwrap();
        }

        // Query for non-existent value
        let results = db.json_query(&run, "status", Value::String("nonexistent".into()), 10).unwrap();
        assert!(results.is_empty(), "Should find no matching documents");
    });
}

/// Test query respects limit
#[test]
fn test_json_query_respects_limit() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Create many documents with same status
        let entities: Vec<_> = data.get_entities(0).iter()
            .filter(|e| !e.key.is_empty())
            .take(10)
            .collect();

        for entity in &entities {
            db.json_set(&run, &entity.key, "$", obj([("status", Value::String("active".into()))])).unwrap();
        }

        // Query with limit of 3
        let results = db.json_query(&run, "status", Value::String("active".into()), 3).unwrap();
        assert_eq!(results.len(), 3, "Should return only 3 results due to limit");
    });
}

/// Test query with nested path
#[test]
fn test_json_query_nested_path() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Create documents with nested structure from test data
        let entities: Vec<_> = data.get_entities(0).iter()
            .filter(|e| !e.key.is_empty())
            .take(6)
            .collect();

        for (i, entity) in entities.iter().enumerate() {
            let country = if i % 2 == 0 { "USA" } else { "Canada" };
            db.json_set(&run, &entity.key, "$", obj([
                ("profile", obj([
                    ("country", Value::String(country.to_string()))
                ]))
            ])).unwrap();
        }

        // Query nested path
        let results = db.json_query(&run, "profile.country", Value::String("USA".into()), 10).unwrap();
        assert_eq!(results.len(), 3, "Should find 3 users from USA");
    });
}

/// Test query with different value types
#[test]
fn test_json_query_different_types() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Create documents with different types from test data
        let entities: Vec<_> = data.get_entities(0).iter()
            .filter(|e| !e.key.is_empty())
            .take(4)
            .collect();

        db.json_set(&run, &entities[0].key, "$", obj([("value", Value::Int(42))])).unwrap();
        db.json_set(&run, &entities[1].key, "$", obj([("value", Value::Float(42.5))])).unwrap();
        db.json_set(&run, &entities[2].key, "$", obj([("value", Value::String("42".into()))])).unwrap();
        db.json_set(&run, &entities[3].key, "$", obj([("value", Value::Bool(true))])).unwrap();

        // Query for integer 42
        let results = db.json_query(&run, "value", Value::Int(42), 10).unwrap();
        assert_eq!(results.len(), 1, "Should find only the int document");

        // Query for boolean true
        let results = db.json_query(&run, "value", Value::Bool(true), 10).unwrap();
        assert_eq!(results.len(), 1, "Should find only the bool document");
    });
}

/// Test query with run isolation
#[test]
fn test_json_query_run_isolation() {
    let data = test_data();

    test_across_substrate_modes(|db| {
        let run1 = ApiRunId::default_run_id();
        let run2 = ApiRunId::new();

        // Get entities from test data
        let entities: Vec<_> = data.get_entities(0).iter()
            .filter(|e| !e.key.is_empty())
            .take(2)
            .collect();

        // Create documents in both runs with same status
        db.json_set(&run1, &entities[0].key, "$", obj([("status", Value::String("active".into()))])).unwrap();
        db.json_set(&run2, &entities[1].key, "$", obj([("status", Value::String("active".into()))])).unwrap();

        // Query should be isolated per run
        let results1 = db.json_query(&run1, "status", Value::String("active".into()), 10).unwrap();
        let results2 = db.json_query(&run2, "status", Value::String("active".into()), 10).unwrap();

        assert_eq!(results1.len(), 1, "Run1 should have 1 matching doc");
        assert_eq!(results2.len(), 1, "Run2 should have 1 matching doc");
    });
}
