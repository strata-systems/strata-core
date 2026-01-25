//! VectorStore History Tests
//!
//! Tests for history and get_at operations:
//! - Version history retrieval
//! - Pagination
//! - Point-in-time retrieval
//! - Edge cases

use crate::*;

// =============================================================================
// History Tests
// =============================================================================

/// Test basic history retrieval
#[test]
fn test_vector_history_basic() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "history_basic";

        // Create vector and update it multiple times
        db.vector_upsert(&run, collection, "v1", &[1.0, 2.0, 3.0], None).unwrap();
        db.vector_upsert(&run, collection, "v1", &[4.0, 5.0, 6.0], None).unwrap();
        db.vector_upsert(&run, collection, "v1", &[7.0, 8.0, 9.0], None).unwrap();

        // Get history
        let history = db.vector_history(&run, collection, "v1", None, None).unwrap();

        // Should have 3 versions
        assert_eq!(history.len(), 3);

        // Newest first
        assert_eq!(history[0].value.0, vec![7.0, 8.0, 9.0]);
        assert_eq!(history[1].value.0, vec![4.0, 5.0, 6.0]);
        assert_eq!(history[2].value.0, vec![1.0, 2.0, 3.0]);
    });
}

/// Test history with metadata changes
#[test]
fn test_vector_history_with_metadata() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "history_meta";

        let meta1 = obj([("version", Value::String("v1".to_string()))]);
        let meta2 = obj([("version", Value::String("v2".to_string()))]);
        let meta3 = obj([("version", Value::String("v3".to_string()))]);

        db.vector_upsert(&run, collection, "k", &[1.0, 2.0], Some(meta1)).unwrap();
        db.vector_upsert(&run, collection, "k", &[1.0, 2.0], Some(meta2)).unwrap();
        db.vector_upsert(&run, collection, "k", &[1.0, 2.0], Some(meta3)).unwrap();

        let history = db.vector_history(&run, collection, "k", None, None).unwrap();

        assert_eq!(history.len(), 3);

        // Check metadata in each version
        if let Value::Object(m) = &history[0].value.1 {
            assert_eq!(m.get("version"), Some(&Value::String("v3".to_string())));
        } else {
            panic!("Expected object metadata");
        }
    });
}

/// Test history with limit
#[test]
fn test_vector_history_limit() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "history_limit";

        // Create 5 versions
        for i in 0..5 {
            db.vector_upsert(&run, collection, "k", &[i as f32], None).unwrap();
        }

        // Get only 2 most recent
        let history = db.vector_history(&run, collection, "k", Some(2), None).unwrap();

        assert_eq!(history.len(), 2);
        assert_eq!(history[0].value.0, vec![4.0]); // Most recent
        assert_eq!(history[1].value.0, vec![3.0]);
    });
}

/// Test history for nonexistent key
#[test]
fn test_vector_history_nonexistent() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "history_nonexistent";

        // Create collection with a different key
        db.vector_upsert(&run, collection, "existing", &[1.0, 2.0], None).unwrap();

        // History for nonexistent key
        let history = db.vector_history(&run, collection, "nonexistent", None, None).unwrap();

        assert!(history.is_empty());
    });
}

/// Test history for internal collections is blocked
#[test]
fn test_vector_history_internal_blocked() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let result = db.vector_history(&run, "_internal", "key", None, None);
        assert!(result.is_err());
    });
}

// =============================================================================
// Get At Tests
// =============================================================================

/// Test get_at specific version
#[test]
fn test_vector_get_at_basic() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "get_at_basic";

        // Create multiple versions
        db.vector_upsert(&run, collection, "k", &[1.0, 0.0], None).unwrap();
        db.vector_upsert(&run, collection, "k", &[2.0, 0.0], None).unwrap();
        db.vector_upsert(&run, collection, "k", &[3.0, 0.0], None).unwrap();

        // Get version 1 (first)
        let v1 = db.vector_get_at(&run, collection, "k", 1).unwrap();
        assert!(v1.is_some());
        assert_eq!(v1.unwrap().value.0, vec![1.0, 0.0]);

        // Get version 2 (second)
        let v2 = db.vector_get_at(&run, collection, "k", 2).unwrap();
        assert!(v2.is_some());
        assert_eq!(v2.unwrap().value.0, vec![2.0, 0.0]);

        // Get version 3 (third)
        let v3 = db.vector_get_at(&run, collection, "k", 3).unwrap();
        assert!(v3.is_some());
        assert_eq!(v3.unwrap().value.0, vec![3.0, 0.0]);
    });
}

/// Test get_at nonexistent version
#[test]
fn test_vector_get_at_nonexistent_version() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "get_at_nonexistent";

        db.vector_upsert(&run, collection, "k", &[1.0], None).unwrap();
        db.vector_upsert(&run, collection, "k", &[2.0], None).unwrap();

        // Version 99 doesn't exist
        let result = db.vector_get_at(&run, collection, "k", 99).unwrap();
        assert!(result.is_none());
    });
}

/// Test get_at for internal collections is blocked
#[test]
fn test_vector_get_at_internal_blocked() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let result = db.vector_get_at(&run, "_internal", "key", 1);
        assert!(result.is_err());
    });
}

// =============================================================================
// List Keys Tests
// =============================================================================

/// Test list_keys basic
#[test]
fn test_vector_list_keys_basic() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "list_keys_basic";

        db.vector_upsert(&run, collection, "charlie", &[1.0], None).unwrap();
        db.vector_upsert(&run, collection, "alpha", &[2.0], None).unwrap();
        db.vector_upsert(&run, collection, "bravo", &[3.0], None).unwrap();

        let keys = db.vector_list_keys(&run, collection, None, None).unwrap();

        // Should be sorted
        assert_eq!(keys, vec!["alpha", "bravo", "charlie"]);
    });
}

/// Test list_keys with limit
#[test]
fn test_vector_list_keys_limit() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "list_keys_limit";

        for c in ['a', 'b', 'c', 'd', 'e'] {
            db.vector_upsert(&run, collection, &c.to_string(), &[1.0], None).unwrap();
        }

        let keys = db.vector_list_keys(&run, collection, Some(3), None).unwrap();

        assert_eq!(keys.len(), 3);
        assert_eq!(keys, vec!["a", "b", "c"]);
    });
}

/// Test list_keys with cursor (pagination)
#[test]
fn test_vector_list_keys_cursor() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "list_keys_cursor";

        for c in ['a', 'b', 'c', 'd', 'e'] {
            db.vector_upsert(&run, collection, &c.to_string(), &[1.0], None).unwrap();
        }

        // Get keys after "b"
        let keys = db.vector_list_keys(&run, collection, None, Some("b")).unwrap();

        assert_eq!(keys, vec!["c", "d", "e"]);
    });
}

/// Test list_keys empty collection
#[test]
fn test_vector_list_keys_empty() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "list_keys_empty";

        // Create empty collection
        db.vector_create_collection(&run, collection, 3, strata_api::substrate::DistanceMetric::Cosine).unwrap();

        let keys = db.vector_list_keys(&run, collection, None, None).unwrap();

        assert!(keys.is_empty());
    });
}

/// Test list_keys internal blocked
#[test]
fn test_vector_list_keys_internal_blocked() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let result = db.vector_list_keys(&run, "_internal", None, None);
        assert!(result.is_err());
    });
}

// =============================================================================
// Scan Tests
// =============================================================================

/// Test scan basic
#[test]
fn test_vector_scan_basic() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "scan_basic";

        db.vector_upsert(&run, collection, "b", &[2.0, 0.0], None).unwrap();
        db.vector_upsert(&run, collection, "a", &[1.0, 0.0], None).unwrap();
        db.vector_upsert(&run, collection, "c", &[3.0, 0.0], None).unwrap();

        let results = db.vector_scan(&run, collection, None, None).unwrap();

        assert_eq!(results.len(), 3);
        // Should be sorted by key
        assert_eq!(results[0].0, "a");
        assert_eq!(results[0].1.0, vec![1.0, 0.0]);
        assert_eq!(results[1].0, "b");
        assert_eq!(results[2].0, "c");
    });
}

/// Test scan with limit
#[test]
fn test_vector_scan_limit() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "scan_limit";

        for c in ['a', 'b', 'c', 'd', 'e'] {
            db.vector_upsert(&run, collection, &c.to_string(), &[1.0], None).unwrap();
        }

        let results = db.vector_scan(&run, collection, Some(2), None).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "a");
        assert_eq!(results[1].0, "b");
    });
}

/// Test scan with cursor
#[test]
fn test_vector_scan_cursor() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "scan_cursor";

        for c in ['a', 'b', 'c', 'd'] {
            db.vector_upsert(&run, collection, &c.to_string(), &[1.0], None).unwrap();
        }

        // Scan starting after "b"
        let results = db.vector_scan(&run, collection, None, Some("b")).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "c");
        assert_eq!(results[1].0, "d");
    });
}

/// Test scan internal blocked
#[test]
fn test_vector_scan_internal_blocked() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let result = db.vector_scan(&run, "_internal", None, None);
        assert!(result.is_err());
    });
}

// =============================================================================
// History Persistence Tests
// =============================================================================

/// Test history persists across restart
#[test]
fn test_vector_history_persist() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let run = ApiRunId::default_run_id();
    let collection = "history_persist";

    // Create and update vector
    {
        let db = create_persistent_db(temp_dir.path());
        db.vector_upsert(&run, collection, "k", &[1.0, 2.0], None).unwrap();
        db.vector_upsert(&run, collection, "k", &[3.0, 4.0], None).unwrap();
        db.vector_upsert(&run, collection, "k", &[5.0, 6.0], None).unwrap();
    }

    // Reopen and check history
    {
        let db = create_persistent_db(temp_dir.path());
        let history = db.vector_history(&run, collection, "k", None, None).unwrap();

        assert_eq!(history.len(), 3);
        assert_eq!(history[0].value.0, vec![5.0, 6.0]);
        assert_eq!(history[1].value.0, vec![3.0, 4.0]);
        assert_eq!(history[2].value.0, vec![1.0, 2.0]);
    }
}
