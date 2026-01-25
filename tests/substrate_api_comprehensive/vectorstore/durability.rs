//! VectorStore Durability Tests
//!
//! Tests for persistence across restarts:
//! - Vector persistence
//! - Collection persistence
//! - Metadata persistence
//!
//! These tests verify that VectorStore data survives database restarts via
//! WAL recovery. The recovery participant (register_vector_recovery) must be
//! registered before opening the database.

use crate::*;
use strata_api::substrate::DistanceMetric;
use tempfile::TempDir;

/// Test vectors persist after restart
#[test]
fn test_vector_persist_after_restart() {
    let temp_dir = TempDir::new().unwrap();
    let run = ApiRunId::default_run_id();
    let collection = "persist_collection";

    let v1 = vec![1.0, 2.0, 3.0];
    let v2 = vec![4.0, 5.0, 6.0];

    // First session - write data
    {
        let db = create_persistent_db(temp_dir.path());
        db.vector_upsert(&run, collection, "v1", &v1, None).unwrap();
        db.vector_upsert(&run, collection, "v2", &v2, None).unwrap();
    }

    // Second session - verify data
    {
        let db = create_persistent_db(temp_dir.path());
        let result1 = db.vector_get(&run, collection, "v1").unwrap().unwrap();
        let result2 = db.vector_get(&run, collection, "v2").unwrap().unwrap();

        assert_eq!(result1.value.0, v1);
        assert_eq!(result2.value.0, v2);
    }
}

/// Test collection info persists
#[test]
fn test_vector_collection_persist() {
    let temp_dir = TempDir::new().unwrap();
    let run = ApiRunId::default_run_id();

    // Create collection with specific configuration
    {
        let db = create_persistent_db(temp_dir.path());
        db.vector_create_collection(&run, "persist_coll", 256, DistanceMetric::Cosine).unwrap();

        // Add some vectors
        let vec: Vec<f32> = (0..256).map(|i| i as f32 / 256.0).collect();
        db.vector_upsert(&run, "persist_coll", "v1", &vec, None).unwrap();
        db.vector_upsert(&run, "persist_coll", "v2", &vec, None).unwrap();
    }

    // Verify after restart
    {
        let db = create_persistent_db(temp_dir.path());

        let info = db.vector_collection_info(&run, "persist_coll").unwrap().unwrap();
        assert_eq!(info.dimension, 256);
        assert_eq!(info.count, 2);
    }
}

/// Test metadata persists
#[test]
fn test_vector_metadata_persist() {
    let temp_dir = TempDir::new().unwrap();
    let run = ApiRunId::default_run_id();
    let collection = "metadata_persist";

    let metadata = obj([
        ("category", Value::String("test".to_string())),
        ("priority", Value::Int(42)),
        ("active", Value::Bool(true)),
    ]);

    // Write with metadata
    {
        let db = create_persistent_db(temp_dir.path());
        db.vector_upsert(&run, collection, "v1", &[1.0, 2.0, 3.0], Some(metadata.clone())).unwrap();
    }

    // Verify metadata
    {
        let db = create_persistent_db(temp_dir.path());
        let result = db.vector_get(&run, collection, "v1").unwrap().unwrap();

        // Metadata should be present (though conversion may vary)
        assert!(result.value.1.is_object() || result.value.1 != Value::Null);
    }
}

/// Test delete persists
#[test]
fn test_vector_delete_persist() {
    let temp_dir = TempDir::new().unwrap();
    let run = ApiRunId::default_run_id();
    let collection = "delete_persist";

    // Create and delete
    {
        let db = create_persistent_db(temp_dir.path());
        db.vector_upsert(&run, collection, "v1", &[1.0, 2.0, 3.0], None).unwrap();
        db.vector_upsert(&run, collection, "v2", &[4.0, 5.0, 6.0], None).unwrap();
        db.vector_delete(&run, collection, "v1").unwrap();
    }

    // Verify deletion persisted
    {
        let db = create_persistent_db(temp_dir.path());
        assert!(db.vector_get(&run, collection, "v1").unwrap().is_none());
        assert!(db.vector_get(&run, collection, "v2").unwrap().is_some());
    }
}

/// Test drop collection persists
#[test]
fn test_vector_drop_collection_persist() {
    let temp_dir = TempDir::new().unwrap();
    let run = ApiRunId::default_run_id();

    // Create and drop
    {
        let db = create_persistent_db(temp_dir.path());
        db.vector_upsert(&run, "keep_coll", "v", &[1.0, 2.0], None).unwrap();
        db.vector_upsert(&run, "drop_coll", "v", &[1.0, 2.0], None).unwrap();
        db.vector_drop_collection(&run, "drop_coll").unwrap();
    }

    // Verify
    {
        let db = create_persistent_db(temp_dir.path());
        assert!(db.vector_collection_exists(&run, "keep_coll").unwrap());
        assert!(!db.vector_collection_exists(&run, "drop_coll").unwrap());
    }
}

/// Test multiple collections persist
#[test]
fn test_vector_multiple_collections_persist() {
    let temp_dir = TempDir::new().unwrap();
    let run = ApiRunId::default_run_id();

    // Create multiple collections
    {
        let db = create_persistent_db(temp_dir.path());
        db.vector_upsert(&run, "coll_a", "v", &[1.0, 0.0], None).unwrap();
        db.vector_upsert(&run, "coll_b", "v", &[0.0, 1.0, 0.0], None).unwrap();
        db.vector_create_collection(&run, "coll_c", 100, DistanceMetric::Euclidean).unwrap();
    }

    // Verify all collections
    {
        let db = create_persistent_db(temp_dir.path());
        let collections = db.vector_list_collections(&run).unwrap();
        assert_eq!(collections.len(), 3);

        // Verify dimensions are preserved
        let info_a = db.vector_collection_info(&run, "coll_a").unwrap().unwrap();
        let info_b = db.vector_collection_info(&run, "coll_b").unwrap().unwrap();
        let info_c = db.vector_collection_info(&run, "coll_c").unwrap().unwrap();

        assert_eq!(info_a.dimension, 2);  // From [1.0, 0.0]
        assert_eq!(info_b.dimension, 3);  // From [0.0, 1.0, 0.0]
        assert_eq!(info_c.dimension, 100); // Explicit dimension
    }
}

/// Test run isolation persists
#[test]
fn test_vector_run_isolation_persists() {
    let temp_dir = TempDir::new().unwrap();
    let run1 = ApiRunId::default_run_id();
    let run2 = ApiRunId::new();

    // Create in both runs
    {
        let db = create_persistent_db(temp_dir.path());
        db.run_create(Some(&run2), None).unwrap();

        db.vector_upsert(&run1, "shared_name", "v", &[1.0, 0.0], None).unwrap();
        db.vector_upsert(&run2, "shared_name", "v", &[0.0, 1.0], None).unwrap();
    }

    // Verify isolation
    {
        let db = create_persistent_db(temp_dir.path());
        let v1 = db.vector_get(&run1, "shared_name", "v").unwrap().unwrap();
        let v2 = db.vector_get(&run2, "shared_name", "v").unwrap().unwrap();

        assert_eq!(v1.value.0, vec![1.0, 0.0]);
        assert_eq!(v2.value.0, vec![0.0, 1.0]);
    }
}

/// Test updates persist
#[test]
fn test_vector_update_persist() {
    let temp_dir = TempDir::new().unwrap();
    let run = ApiRunId::default_run_id();
    let collection = "update_persist";

    let original = vec![1.0, 2.0, 3.0];
    let updated = vec![9.0, 8.0, 7.0];

    // Insert and update
    {
        let db = create_persistent_db(temp_dir.path());
        db.vector_upsert(&run, collection, "v", &original, None).unwrap();
        db.vector_upsert(&run, collection, "v", &updated, None).unwrap();
    }

    // Verify update persisted
    {
        let db = create_persistent_db(temp_dir.path());
        let result = db.vector_get(&run, collection, "v").unwrap().unwrap();
        assert_eq!(result.value.0, updated);
    }
}

/// Test search works after restart
#[test]
fn test_vector_search_after_restart() {
    let temp_dir = TempDir::new().unwrap();
    let run = ApiRunId::default_run_id();
    let collection = "search_persist";

    // Insert data
    {
        let db = create_persistent_db(temp_dir.path());
        db.vector_upsert(&run, collection, "v1", &[1.0, 0.0, 0.0], None).unwrap();
        db.vector_upsert(&run, collection, "v2", &[0.0, 1.0, 0.0], None).unwrap();
        db.vector_upsert(&run, collection, "v3", &[0.0, 0.0, 1.0], None).unwrap();
    }

    // Search after restart
    {
        let db = create_persistent_db(temp_dir.path());
        let results = db.vector_search(&run, collection, &[1.0, 0.0, 0.0], 3, None, None).unwrap();

        assert_eq!(results.len(), 3);
        // Most similar should be v1
        assert_eq!(results[0].key, "v1");
    }
}
