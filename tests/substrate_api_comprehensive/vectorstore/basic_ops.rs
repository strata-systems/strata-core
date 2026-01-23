//! VectorStore Basic Operations Tests
//!
//! Tests for fundamental VectorStore operations:
//! - vector_upsert
//! - vector_get
//! - vector_delete

use crate::*;

/// Test basic upsert and get
#[test]
fn test_vector_upsert_get() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "test_collection";
        let key = "vec1";
        let vector = vec![1.0, 2.0, 3.0];

        // Upsert vector
        let version = db.vector_upsert(&run, collection, key, &vector, None).unwrap();
        assert!(matches!(version, Version::Txn(_) | Version::Counter(_)));

        // Get vector
        let result = db.vector_get(&run, collection, key).unwrap();
        assert!(result.is_some());

        let versioned = result.unwrap();
        assert_eq!(versioned.value.0, vector);
        assert_eq!(versioned.value.1, Value::Null);
    });
}

/// Test upsert with metadata
#[test]
fn test_vector_upsert_with_metadata() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "metadata_collection";
        let key = "vec_meta";
        let vector = vec![0.5, 0.5, 0.5];
        let metadata = obj([
            ("category", Value::String("test".to_string())),
            ("score", Value::Int(42)),
        ]);

        // Upsert with metadata
        db.vector_upsert(&run, collection, key, &vector, Some(metadata.clone())).unwrap();

        // Get and verify metadata
        let result = db.vector_get(&run, collection, key).unwrap().unwrap();
        assert_eq!(result.value.0, vector);
        // Metadata should be preserved (though converted through JSON)
        assert!(result.value.1.is_object());
    });
}

/// Test upsert updates existing vector
#[test]
fn test_vector_upsert_update() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "update_collection";
        let key = "update_vec";

        // Initial insert
        let v1 = vec![1.0, 0.0, 0.0];
        db.vector_upsert(&run, collection, key, &v1, None).unwrap();

        // Update
        let v2 = vec![0.0, 1.0, 0.0];
        db.vector_upsert(&run, collection, key, &v2, None).unwrap();

        // Verify update
        let result = db.vector_get(&run, collection, key).unwrap().unwrap();
        assert_eq!(result.value.0, v2);
    });
}

/// Test get non-existent vector
#[test]
fn test_vector_get_nonexistent() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Get from non-existent collection
        let result = db.vector_get(&run, "nonexistent_collection", "key").unwrap();
        assert!(result.is_none());

        // Create collection, then get non-existent key
        let collection = "exists_collection";
        db.vector_upsert(&run, collection, "exists", &[1.0, 2.0], None).unwrap();

        let result = db.vector_get(&run, collection, "nonexistent_key").unwrap();
        assert!(result.is_none());
    });
}

/// Test delete vector
#[test]
fn test_vector_delete() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "delete_collection";
        let key = "delete_vec";

        // Insert
        db.vector_upsert(&run, collection, key, &[1.0, 2.0, 3.0], None).unwrap();
        assert!(db.vector_get(&run, collection, key).unwrap().is_some());

        // Delete
        let deleted = db.vector_delete(&run, collection, key).unwrap();
        assert!(deleted, "Should return true for existing vector");

        // Verify deleted
        assert!(db.vector_get(&run, collection, key).unwrap().is_none());
    });
}

/// Test delete non-existent vector
#[test]
fn test_vector_delete_nonexistent() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "delete_nonexistent";

        // Create collection with one vector
        db.vector_upsert(&run, collection, "exists", &[1.0, 2.0], None).unwrap();

        // Delete non-existent key
        let deleted = db.vector_delete(&run, collection, "nonexistent").unwrap();
        assert!(!deleted, "Should return false for non-existent vector");
    });
}

/// Test multiple vectors in same collection
#[test]
fn test_vector_multiple_in_collection() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "multi_vec_collection";

        // Insert multiple vectors
        let vectors = vec![
            ("v1", vec![1.0, 0.0, 0.0]),
            ("v2", vec![0.0, 1.0, 0.0]),
            ("v3", vec![0.0, 0.0, 1.0]),
        ];

        for (key, vec) in &vectors {
            db.vector_upsert(&run, collection, key, vec, None).unwrap();
        }

        // Verify all vectors
        for (key, expected) in &vectors {
            let result = db.vector_get(&run, collection, key).unwrap().unwrap();
            assert_eq!(&result.value.0, expected);
        }
    });
}

/// Test run isolation for vectors
#[test]
fn test_vector_run_isolation() {
    test_across_substrate_modes(|db| {
        let run1 = ApiRunId::default_run_id();
        let run2 = ApiRunId::new();
        let collection = "isolated_collection";
        let key = "same_key";

        // Create run2
        db.run_create(Some(&run2), None).unwrap();

        // Insert in run1
        let v1 = vec![1.0, 0.0];
        db.vector_upsert(&run1, collection, key, &v1, None).unwrap();

        // Insert in run2
        let v2 = vec![0.0, 1.0];
        db.vector_upsert(&run2, collection, key, &v2, None).unwrap();

        // Verify isolation
        assert_eq!(db.vector_get(&run1, collection, key).unwrap().unwrap().value.0, v1);
        assert_eq!(db.vector_get(&run2, collection, key).unwrap().unwrap().value.0, v2);
    });
}

/// Test various vector dimensions
#[test]
fn test_vector_various_dimensions() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // 1D vector
        db.vector_upsert(&run, "dim1", "v", &[1.0], None).unwrap();
        assert_eq!(db.vector_get(&run, "dim1", "v").unwrap().unwrap().value.0.len(), 1);

        // 128D vector (common embedding size)
        let vec128: Vec<f32> = (0..128).map(|i| i as f32 / 128.0).collect();
        db.vector_upsert(&run, "dim128", "v", &vec128, None).unwrap();
        assert_eq!(db.vector_get(&run, "dim128", "v").unwrap().unwrap().value.0.len(), 128);

        // 1536D vector (OpenAI embedding size)
        let vec1536: Vec<f32> = (0..1536).map(|i| (i as f32).sin()).collect();
        db.vector_upsert(&run, "dim1536", "v", &vec1536, None).unwrap();
        assert_eq!(db.vector_get(&run, "dim1536", "v").unwrap().unwrap().value.0.len(), 1536);
    });
}

/// Test special float values in vectors
#[test]
fn test_vector_float_values() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "float_values";

        // Zero vector
        db.vector_upsert(&run, collection, "zero", &[0.0, 0.0, 0.0], None).unwrap();
        let result = db.vector_get(&run, collection, "zero").unwrap().unwrap();
        assert_eq!(result.value.0, vec![0.0, 0.0, 0.0]);

        // Negative values
        db.vector_upsert(&run, collection, "negative", &[-1.0, -2.5, -0.001], None).unwrap();
        let result = db.vector_get(&run, collection, "negative").unwrap().unwrap();
        assert_eq!(result.value.0, vec![-1.0, -2.5, -0.001]);

        // Small values
        db.vector_upsert(&run, collection, "small", &[1e-10, 1e-20, 1e-30], None).unwrap();

        // Large values
        db.vector_upsert(&run, collection, "large", &[1e10, 1e20, 1e30], None).unwrap();
    });
}
