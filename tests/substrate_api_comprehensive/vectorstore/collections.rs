//! VectorStore Collection Management Tests
//!
//! Tests for collection operations:
//! - vector_create_collection
//! - vector_drop_collection
//! - vector_collection_info
//! - vector_collection_exists
//! - vector_list_collections

use crate::*;
use strata_api::substrate::DistanceMetric;

/// Test create collection
#[test]
fn test_vector_create_collection() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "created_collection";

        // Create collection
        let version = db.vector_create_collection(&run, collection, 128, DistanceMetric::Cosine).unwrap();
        assert!(matches!(version, Version::Txn(_) | Version::Counter(_)));

        // Verify it exists
        assert!(db.vector_collection_exists(&run, collection).unwrap());
    });
}

/// Test collection exists
#[test]
fn test_vector_collection_exists() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Non-existent collection
        assert!(!db.vector_collection_exists(&run, "nonexistent").unwrap());

        // Create via explicit creation
        db.vector_create_collection(&run, "explicit", 64, DistanceMetric::Cosine).unwrap();
        assert!(db.vector_collection_exists(&run, "explicit").unwrap());

        // Create via first insert
        db.vector_upsert(&run, "implicit", "v", &[1.0, 2.0], None).unwrap();
        assert!(db.vector_collection_exists(&run, "implicit").unwrap());
    });
}

/// Test collection info
#[test]
fn test_vector_collection_info() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "info_collection";

        // No info for non-existent
        assert!(db.vector_collection_info(&run, "nonexistent").unwrap().is_none());

        // Create collection with specific dimension
        db.vector_create_collection(&run, collection, 256, DistanceMetric::Cosine).unwrap();

        let info = db.vector_collection_info(&run, collection).unwrap().unwrap();
        assert_eq!(info.dimension, 256);
        assert_eq!(info.count, 0); // No vectors yet

        // Add vectors and check count
        let vec: Vec<f32> = (0..256).map(|i| i as f32 / 256.0).collect();
        db.vector_upsert(&run, collection, "v1", &vec, None).unwrap();
        db.vector_upsert(&run, collection, "v2", &vec, None).unwrap();

        let info = db.vector_collection_info(&run, collection).unwrap().unwrap();
        assert_eq!(info.count, 2);
    });
}

/// Test drop collection
#[test]
fn test_vector_drop_collection() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "drop_collection";

        // Create and populate collection
        db.vector_upsert(&run, collection, "v1", &[1.0, 2.0, 3.0], None).unwrap();
        db.vector_upsert(&run, collection, "v2", &[4.0, 5.0, 6.0], None).unwrap();
        assert!(db.vector_collection_exists(&run, collection).unwrap());

        // Drop collection
        let dropped = db.vector_drop_collection(&run, collection).unwrap();
        assert!(dropped, "Should return true for existing collection");

        // Verify it's gone
        assert!(!db.vector_collection_exists(&run, collection).unwrap());
        assert!(db.vector_get(&run, collection, "v1").unwrap().is_none());
    });
}

/// Test drop non-existent collection
#[test]
fn test_vector_drop_nonexistent_collection() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let dropped = db.vector_drop_collection(&run, "nonexistent").unwrap();
        assert!(!dropped, "Should return false for non-existent collection");
    });
}

/// Test list collections
#[test]
fn test_vector_list_collections() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Initially empty
        let collections = db.vector_list_collections(&run).unwrap();
        assert!(collections.is_empty());

        // Create some collections
        db.vector_create_collection(&run, "coll_a", 64, DistanceMetric::Cosine).unwrap();
        db.vector_upsert(&run, "coll_b", "v", &[1.0, 2.0], None).unwrap();
        db.vector_create_collection(&run, "coll_c", 128, DistanceMetric::Euclidean).unwrap();

        // List collections
        let collections = db.vector_list_collections(&run).unwrap();
        assert_eq!(collections.len(), 3);

        // Verify names are present
        let names: Vec<&str> = collections.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"coll_a"));
        assert!(names.contains(&"coll_b"));
        assert!(names.contains(&"coll_c"));

        // Verify dimensions
        let coll_a = collections.iter().find(|c| c.name == "coll_a").unwrap();
        assert_eq!(coll_a.dimension, 64);

        let coll_b = collections.iter().find(|c| c.name == "coll_b").unwrap();
        assert_eq!(coll_b.dimension, 2); // From first insert
    });
}

/// Test collection dimension enforcement
#[test]
fn test_vector_dimension_enforcement() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "fixed_dimension";

        // First insert sets dimension
        db.vector_upsert(&run, collection, "v1", &[1.0, 2.0, 3.0], None).unwrap();

        // Same dimension works
        db.vector_upsert(&run, collection, "v2", &[4.0, 5.0, 6.0], None).unwrap();

        // Different dimension should fail
        let result = db.vector_upsert(&run, collection, "v3", &[1.0, 2.0], None);
        assert!(result.is_err(), "Different dimension should fail");
    });
}

/// Test explicit collection creation vs implicit
#[test]
fn test_vector_explicit_vs_implicit_creation() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Explicit creation
        db.vector_create_collection(&run, "explicit", 100, DistanceMetric::Cosine).unwrap();
        let info = db.vector_collection_info(&run, "explicit").unwrap().unwrap();
        assert_eq!(info.dimension, 100);

        // Implicit creation via insert
        let vec50: Vec<f32> = vec![0.0; 50];
        db.vector_upsert(&run, "implicit", "v", &vec50, None).unwrap();
        let info = db.vector_collection_info(&run, "implicit").unwrap().unwrap();
        assert_eq!(info.dimension, 50);
    });
}

/// Test collection isolation between runs
#[test]
fn test_vector_collection_run_isolation() {
    test_across_substrate_modes(|db| {
        let run1 = ApiRunId::default_run_id();
        let run2 = ApiRunId::new();

        db.run_create(Some(&run2), None).unwrap();

        // Create collection in run1
        db.vector_create_collection(&run1, "isolated", 32, DistanceMetric::Cosine).unwrap();

        // Should not exist in run2
        assert!(db.vector_collection_exists(&run1, "isolated").unwrap());
        assert!(!db.vector_collection_exists(&run2, "isolated").unwrap());

        // Create same name in run2 with different dimension
        db.vector_create_collection(&run2, "isolated", 64, DistanceMetric::Euclidean).unwrap();

        // Both should exist independently
        let info1 = db.vector_collection_info(&run1, "isolated").unwrap().unwrap();
        let info2 = db.vector_collection_info(&run2, "isolated").unwrap().unwrap();
        assert_eq!(info1.dimension, 32);
        assert_eq!(info2.dimension, 64);
    });
}

/// Test re-create collection after drop
#[test]
fn test_vector_recreate_after_drop() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "recreate_collection";

        // Create with dimension 32
        db.vector_create_collection(&run, collection, 32, DistanceMetric::Cosine).unwrap();
        db.vector_upsert(&run, collection, "v", &vec![0.0; 32], None).unwrap();

        // Drop
        db.vector_drop_collection(&run, collection).unwrap();

        // Re-create with different dimension - should work
        db.vector_create_collection(&run, collection, 64, DistanceMetric::Euclidean).unwrap();

        let info = db.vector_collection_info(&run, collection).unwrap().unwrap();
        assert_eq!(info.dimension, 64);
    });
}

/// Test vector_count method
#[test]
fn test_vector_count() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "count_test";

        // Non-existent collection returns 0
        assert_eq!(db.vector_count(&run, "nonexistent").unwrap(), 0);

        // Create collection
        db.vector_create_collection(&run, collection, 3, DistanceMetric::Cosine).unwrap();
        assert_eq!(db.vector_count(&run, collection).unwrap(), 0);

        // Add vectors
        db.vector_upsert(&run, collection, "v1", &[1.0, 0.0, 0.0], None).unwrap();
        assert_eq!(db.vector_count(&run, collection).unwrap(), 1);

        db.vector_upsert(&run, collection, "v2", &[0.0, 1.0, 0.0], None).unwrap();
        db.vector_upsert(&run, collection, "v3", &[0.0, 0.0, 1.0], None).unwrap();
        assert_eq!(db.vector_count(&run, collection).unwrap(), 3);

        // Upsert existing key doesn't increase count
        db.vector_upsert(&run, collection, "v1", &[0.5, 0.5, 0.0], None).unwrap();
        assert_eq!(db.vector_count(&run, collection).unwrap(), 3);

        // Delete decreases count
        db.vector_delete(&run, collection, "v2").unwrap();
        assert_eq!(db.vector_count(&run, collection).unwrap(), 2);
    });
}

/// Test vector_count is blocked for internal collections
#[test]
fn test_vector_count_internal_blocked() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let result = db.vector_count(&run, "_internal");
        assert!(result.is_err(), "vector_count should be blocked for internal collections");
    });
}
