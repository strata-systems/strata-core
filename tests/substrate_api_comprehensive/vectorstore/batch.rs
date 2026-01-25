//! VectorStore Batch Operations Tests
//!
//! Tests for batch operations:
//! - vector_upsert_batch
//! - vector_get_batch
//! - vector_delete_batch

use crate::*;

// =============================================================================
// Batch Upsert Tests
// =============================================================================

/// Test basic batch upsert
#[test]
fn test_vector_upsert_batch_basic() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "batch_upsert";

        let vectors: Vec<(&str, &[f32], Option<Value>)> = vec![
            ("v1", &[1.0, 0.0, 0.0][..], None),
            ("v2", &[0.0, 1.0, 0.0][..], None),
            ("v3", &[0.0, 0.0, 1.0][..], None),
        ];

        let results = db.vector_upsert_batch(&run, collection, vectors).unwrap();

        assert_eq!(results.len(), 3);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
        assert!(results[2].is_ok());

        // Verify all vectors were inserted
        assert!(db.vector_get(&run, collection, "v1").unwrap().is_some());
        assert!(db.vector_get(&run, collection, "v2").unwrap().is_some());
        assert!(db.vector_get(&run, collection, "v3").unwrap().is_some());
    });
}

/// Test batch upsert with metadata
#[test]
fn test_vector_upsert_batch_with_metadata() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "batch_meta";

        let meta1 = obj([("type", Value::String("a".to_string()))]);
        let meta2 = obj([("type", Value::String("b".to_string()))]);

        let vectors: Vec<(&str, &[f32], Option<Value>)> = vec![
            ("v1", &[1.0, 2.0][..], Some(meta1)),
            ("v2", &[3.0, 4.0][..], Some(meta2)),
            ("v3", &[5.0, 6.0][..], None),
        ];

        let results = db.vector_upsert_batch(&run, collection, vectors).unwrap();
        assert!(results.iter().all(|r| r.is_ok()));

        // Verify metadata
        let r1 = db.vector_get(&run, collection, "v1").unwrap().unwrap();
        assert!(r1.value.1.is_object());

        let r3 = db.vector_get(&run, collection, "v3").unwrap().unwrap();
        assert_eq!(r3.value.1, Value::Null);
    });
}

/// Test batch upsert empty batch
#[test]
fn test_vector_upsert_batch_empty() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "batch_empty";

        let vectors: Vec<(&str, &[f32], Option<Value>)> = vec![];
        let results = db.vector_upsert_batch(&run, collection, vectors).unwrap();

        assert!(results.is_empty());
    });
}

/// Test batch upsert auto-creates collection
#[test]
fn test_vector_upsert_batch_auto_creates_collection() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "batch_autocreate";

        // Collection doesn't exist
        assert!(!db.vector_collection_exists(&run, collection).unwrap());

        let vectors: Vec<(&str, &[f32], Option<Value>)> = vec![
            ("v1", &[1.0, 2.0, 3.0][..], None),
        ];

        db.vector_upsert_batch(&run, collection, vectors).unwrap();

        // Collection should exist now
        assert!(db.vector_collection_exists(&run, collection).unwrap());
        let info = db.vector_collection_info(&run, collection).unwrap().unwrap();
        assert_eq!(info.dimension, 3);
    });
}

/// Test batch upsert with dimension mismatch
#[test]
fn test_vector_upsert_batch_dimension_mismatch() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "batch_dim_mismatch";

        // Create collection with dimension 3
        db.vector_upsert(&run, collection, "existing", &[1.0, 2.0, 3.0], None).unwrap();

        // Batch with wrong dimension
        let vectors: Vec<(&str, &[f32], Option<Value>)> = vec![
            ("v1", &[1.0, 2.0, 3.0][..], None),  // Correct
            ("v2", &[1.0, 2.0][..], None),       // Wrong dimension
            ("v3", &[1.0, 2.0, 3.0][..], None),  // Correct
        ];

        let results = db.vector_upsert_batch(&run, collection, vectors).unwrap();

        assert!(results[0].is_ok());
        assert!(results[1].is_err()); // Dimension mismatch
        assert!(results[2].is_ok());

        // Correct vectors should still be inserted
        assert!(db.vector_get(&run, collection, "v1").unwrap().is_some());
        assert!(db.vector_get(&run, collection, "v2").unwrap().is_none());
        assert!(db.vector_get(&run, collection, "v3").unwrap().is_some());
    });
}

/// Test batch upsert updates existing vectors
#[test]
fn test_vector_upsert_batch_updates() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "batch_updates";

        // Insert initial vectors
        db.vector_upsert(&run, collection, "v1", &[1.0, 0.0], None).unwrap();
        db.vector_upsert(&run, collection, "v2", &[0.0, 1.0], None).unwrap();

        // Update via batch
        let vectors: Vec<(&str, &[f32], Option<Value>)> = vec![
            ("v1", &[9.0, 9.0][..], None),
            ("v2", &[8.0, 8.0][..], None),
        ];

        db.vector_upsert_batch(&run, collection, vectors).unwrap();

        // Verify updates
        let r1 = db.vector_get(&run, collection, "v1").unwrap().unwrap();
        assert_eq!(r1.value.0, vec![9.0, 9.0]);

        let r2 = db.vector_get(&run, collection, "v2").unwrap().unwrap();
        assert_eq!(r2.value.0, vec![8.0, 8.0]);
    });
}

/// Test batch upsert blocked for internal collections
#[test]
fn test_vector_upsert_batch_internal_blocked() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let vectors: Vec<(&str, &[f32], Option<Value>)> = vec![
            ("v1", &[1.0, 2.0][..], None),
        ];

        let result = db.vector_upsert_batch(&run, "_internal", vectors);
        assert!(result.is_err());
    });
}

// =============================================================================
// Batch Get Tests
// =============================================================================

/// Test basic batch get
#[test]
fn test_vector_get_batch_basic() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "batch_get";

        // Insert vectors
        db.vector_upsert(&run, collection, "v1", &[1.0, 0.0], None).unwrap();
        db.vector_upsert(&run, collection, "v2", &[0.0, 1.0], None).unwrap();
        db.vector_upsert(&run, collection, "v3", &[1.0, 1.0], None).unwrap();

        // Batch get
        let keys = vec!["v1", "v2", "v3"];
        let results = db.vector_get_batch(&run, collection, &keys).unwrap();

        assert_eq!(results.len(), 3);
        assert!(results[0].is_some());
        assert!(results[1].is_some());
        assert!(results[2].is_some());

        assert_eq!(results[0].as_ref().unwrap().value.0, vec![1.0, 0.0]);
        assert_eq!(results[1].as_ref().unwrap().value.0, vec![0.0, 1.0]);
        assert_eq!(results[2].as_ref().unwrap().value.0, vec![1.0, 1.0]);
    });
}

/// Test batch get with missing keys
#[test]
fn test_vector_get_batch_missing_keys() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "batch_get_missing";

        // Insert some vectors
        db.vector_upsert(&run, collection, "v1", &[1.0, 2.0], None).unwrap();
        db.vector_upsert(&run, collection, "v3", &[3.0, 4.0], None).unwrap();

        // Batch get including missing key
        let keys = vec!["v1", "v2", "v3"];  // v2 doesn't exist
        let results = db.vector_get_batch(&run, collection, &keys).unwrap();

        assert_eq!(results.len(), 3);
        assert!(results[0].is_some());
        assert!(results[1].is_none());  // v2 missing
        assert!(results[2].is_some());
    });
}

/// Test batch get empty keys
#[test]
fn test_vector_get_batch_empty() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "batch_get_empty";

        db.vector_upsert(&run, collection, "v1", &[1.0], None).unwrap();

        let keys: Vec<&str> = vec![];
        let results = db.vector_get_batch(&run, collection, &keys).unwrap();

        assert!(results.is_empty());
    });
}

/// Test batch get nonexistent collection
#[test]
fn test_vector_get_batch_nonexistent_collection() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let keys = vec!["v1", "v2"];
        let results = db.vector_get_batch(&run, "nonexistent", &keys).unwrap();

        assert_eq!(results.len(), 2);
        assert!(results[0].is_none());
        assert!(results[1].is_none());
    });
}

/// Test batch get preserves order
#[test]
fn test_vector_get_batch_preserves_order() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "batch_get_order";

        // Insert in reverse order
        db.vector_upsert(&run, collection, "c", &[3.0], None).unwrap();
        db.vector_upsert(&run, collection, "b", &[2.0], None).unwrap();
        db.vector_upsert(&run, collection, "a", &[1.0], None).unwrap();

        // Get in specific order
        let keys = vec!["a", "b", "c"];
        let results = db.vector_get_batch(&run, collection, &keys).unwrap();

        assert_eq!(results[0].as_ref().unwrap().value.0, vec![1.0]);
        assert_eq!(results[1].as_ref().unwrap().value.0, vec![2.0]);
        assert_eq!(results[2].as_ref().unwrap().value.0, vec![3.0]);
    });
}

/// Test batch get blocked for internal collections
#[test]
fn test_vector_get_batch_internal_blocked() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let keys = vec!["v1"];
        let result = db.vector_get_batch(&run, "_internal", &keys);
        assert!(result.is_err());
    });
}

// =============================================================================
// Batch Delete Tests
// =============================================================================

/// Test basic batch delete
#[test]
fn test_vector_delete_batch_basic() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "batch_delete";

        // Insert vectors
        db.vector_upsert(&run, collection, "v1", &[1.0, 2.0], None).unwrap();
        db.vector_upsert(&run, collection, "v2", &[3.0, 4.0], None).unwrap();
        db.vector_upsert(&run, collection, "v3", &[5.0, 6.0], None).unwrap();

        // Delete some
        let keys = vec!["v1", "v3"];
        let results = db.vector_delete_batch(&run, collection, &keys).unwrap();

        assert_eq!(results.len(), 2);
        assert!(results[0]); // v1 deleted
        assert!(results[1]); // v3 deleted

        // Verify
        assert!(db.vector_get(&run, collection, "v1").unwrap().is_none());
        assert!(db.vector_get(&run, collection, "v2").unwrap().is_some());  // Not deleted
        assert!(db.vector_get(&run, collection, "v3").unwrap().is_none());
    });
}

/// Test batch delete with missing keys
#[test]
fn test_vector_delete_batch_missing_keys() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "batch_delete_missing";

        // Insert only one
        db.vector_upsert(&run, collection, "v1", &[1.0, 2.0], None).unwrap();

        // Delete including missing
        let keys = vec!["v1", "v2", "v3"];
        let results = db.vector_delete_batch(&run, collection, &keys).unwrap();

        assert_eq!(results.len(), 3);
        assert!(results[0]);   // v1 existed and deleted
        assert!(!results[1]);  // v2 didn't exist
        assert!(!results[2]);  // v3 didn't exist
    });
}

/// Test batch delete empty keys
#[test]
fn test_vector_delete_batch_empty() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "batch_delete_empty";

        db.vector_upsert(&run, collection, "v1", &[1.0], None).unwrap();

        let keys: Vec<&str> = vec![];
        let results = db.vector_delete_batch(&run, collection, &keys).unwrap();

        assert!(results.is_empty());

        // Vector should still exist
        assert!(db.vector_get(&run, collection, "v1").unwrap().is_some());
    });
}

/// Test batch delete nonexistent collection
#[test]
fn test_vector_delete_batch_nonexistent_collection() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let keys = vec!["v1", "v2"];
        let results = db.vector_delete_batch(&run, "nonexistent", &keys).unwrap();

        assert_eq!(results.len(), 2);
        assert!(!results[0]);
        assert!(!results[1]);
    });
}

/// Test batch delete blocked for internal collections
#[test]
fn test_vector_delete_batch_internal_blocked() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let keys = vec!["v1"];
        let result = db.vector_delete_batch(&run, "_internal", &keys);
        assert!(result.is_err());
    });
}

// =============================================================================
// Large Batch Tests
// =============================================================================

/// Test large batch upsert
#[test]
fn test_vector_batch_large() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "batch_large";
        let batch_size = 100;

        // Create large batch
        let vectors: Vec<(String, Vec<f32>)> = (0..batch_size)
            .map(|i| (format!("v{}", i), vec![i as f32 / 100.0, 0.5, 0.5]))
            .collect();

        let batch: Vec<(&str, &[f32], Option<Value>)> = vectors
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_slice(), None))
            .collect();

        let results = db.vector_upsert_batch(&run, collection, batch).unwrap();

        assert_eq!(results.len(), batch_size);
        assert!(results.iter().all(|r| r.is_ok()));

        // Verify count
        assert_eq!(db.vector_count(&run, collection).unwrap(), batch_size as u64);

        // Batch get all
        let keys: Vec<&str> = vectors.iter().map(|(k, _)| k.as_str()).collect();
        let get_results = db.vector_get_batch(&run, collection, &keys).unwrap();
        assert!(get_results.iter().all(|r| r.is_some()));

        // Batch delete all
        let delete_results = db.vector_delete_batch(&run, collection, &keys).unwrap();
        assert!(delete_results.iter().all(|&deleted| deleted));

        // Verify empty
        assert_eq!(db.vector_count(&run, collection).unwrap(), 0);
    });
}
