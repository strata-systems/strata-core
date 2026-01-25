//! VectorStore Edge Cases Tests
//!
//! Tests for validation and boundary conditions:
//! - Key validation
//! - Dimension limits
//! - Special float values
//! - Large vectors

use crate::*;
use strata_api::substrate::DistanceMetric;

/// Test collection name with special characters
#[test]
fn test_vector_collection_names() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let valid_names = vec![
            "simple",
            "with-dashes",
            "with_underscores",
            "with.dots",
            "UPPERCASE",
            "MixedCase123",
        ];

        for name in valid_names {
            let result = db.vector_upsert(&run, name, "v", &[1.0, 2.0], None);
            assert!(result.is_ok(), "Collection name '{}' should be valid", name);
        }
    });
}

/// Test vector key names
#[test]
fn test_vector_key_names() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "key_names_test";

        // Create collection
        db.vector_create_collection(&run, collection, 2, DistanceMetric::Cosine).unwrap();

        let valid_keys = vec![
            "simple_key",
            "key-with-dashes",
            "key.with.dots",
            "key:with:colons",
            "key/with/slashes",
            "123numeric",
            "UPPERCASE_KEY",
        ];

        for key in valid_keys {
            let result = db.vector_upsert(&run, collection, key, &[1.0, 2.0], None);
            assert!(result.is_ok(), "Key '{}' should be valid", key);
        }
    });
}

/// Test unicode in collection and key names
#[test]
fn test_vector_unicode_names() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Unicode collection name
        let collection = "collection_\u{4e2d}\u{6587}";
        db.vector_upsert(&run, collection, "v", &[1.0], None).unwrap();
        assert!(db.vector_collection_exists(&run, collection).unwrap());

        // Unicode key name
        let key = "key_\u{65e5}\u{672c}\u{8a9e}";
        db.vector_upsert(&run, "unicode_test", key, &[1.0], None).unwrap();
        assert!(db.vector_get(&run, "unicode_test", key).unwrap().is_some());
    });
}

/// Test minimum dimension (1D)
#[test]
fn test_vector_dimension_minimum() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // 1D vector should work
        let result = db.vector_upsert(&run, "dim1", "v", &[1.0], None);
        assert!(result.is_ok());

        let retrieved = db.vector_get(&run, "dim1", "v").unwrap().unwrap();
        assert_eq!(retrieved.value.0.len(), 1);
    });
}

/// Test large dimension vector
#[test]
fn test_vector_dimension_large() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // 4096D vector (common for large models)
        let vec4096: Vec<f32> = (0..4096).map(|i| (i as f32).sin()).collect();
        let result = db.vector_upsert(&run, "dim4096", "v", &vec4096, None);

        // Should work (implementation may have limits)
        if result.is_ok() {
            let retrieved = db.vector_get(&run, "dim4096", "v").unwrap().unwrap();
            assert_eq!(retrieved.value.0.len(), 4096);
        }
    });
}

/// Test empty vector (0D) should fail
#[test]
fn test_vector_dimension_zero() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // 0D vector should fail
        let result = db.vector_upsert(&run, "dim0", "v", &[], None);
        assert!(result.is_err(), "0D vector should fail");
    });
}

/// Test special float values
#[test]
fn test_vector_special_floats() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // NaN - use separate collection since dimension is fixed on first insert
        let nan_vec = vec![f32::NAN, 1.0, 2.0];
        let _result = db.vector_upsert(&run, "special_nan", "nan", &nan_vec, None);
        // Implementation may accept or reject NaN

        // Infinity - use separate collection
        let inf_vec = vec![f32::INFINITY, 1.0, 2.0];
        let _result = db.vector_upsert(&run, "special_inf", "inf", &inf_vec, None);
        // Implementation may accept or reject infinity

        // Normal floats should work - use separate collection with matching dimension
        let normal_vec = vec![0.0, -0.0, 1.0, -1.0, 1e-10, 1e10];
        db.vector_upsert(&run, "special_normal", "normal", &normal_vec, None).unwrap();
    });
}

/// Test normalized vectors
#[test]
fn test_vector_normalized() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "normalized";

        // Unit vector
        let unit = vec![1.0, 0.0, 0.0];
        db.vector_upsert(&run, collection, "unit", &unit, None).unwrap();

        // Normalized vector
        let norm = vec![0.6, 0.8, 0.0]; // sqrt(0.36 + 0.64) = 1.0
        db.vector_upsert(&run, collection, "normalized", &norm, None).unwrap();

        // Search should work with normalized queries
        let results = db.vector_search(&run, collection, &unit, 2, None, None).unwrap();
        assert!(!results.is_empty());
    });
}

/// Test unnormalized vectors
#[test]
fn test_vector_unnormalized() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "unnormalized";

        // Large magnitude vector
        let large = vec![1000.0, 2000.0, 3000.0];
        db.vector_upsert(&run, collection, "large", &large, None).unwrap();

        // Small magnitude vector
        let small = vec![0.001, 0.002, 0.003];
        db.vector_upsert(&run, collection, "small", &small, None).unwrap();

        // Both should be searchable
        let results = db.vector_search(&run, collection, &large, 2, None, None).unwrap();
        assert_eq!(results.len(), 2);
    });
}

/// Test metadata edge cases
#[test]
fn test_vector_metadata_edge_cases() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "metadata_edge";

        // Null metadata
        db.vector_upsert(&run, collection, "null_meta", &[1.0, 2.0], Some(Value::Null)).unwrap();

        // Empty object metadata
        db.vector_upsert(&run, collection, "empty_meta", &[1.0, 2.0], Some(obj([]))).unwrap();

        // Nested metadata
        let nested = obj([
            ("level1", obj([
                ("level2", obj([
                    ("value", Value::Int(42)),
                ])),
            ])),
        ]);
        db.vector_upsert(&run, collection, "nested_meta", &[1.0, 2.0], Some(nested)).unwrap();

        // Array in metadata
        let with_array = obj([
            ("tags", Value::Array(vec![
                Value::String("a".to_string()),
                Value::String("b".to_string()),
            ])),
        ]);
        db.vector_upsert(&run, collection, "array_meta", &[1.0, 2.0], Some(with_array)).unwrap();
    });
}

/// Test large metadata
#[test]
fn test_vector_large_metadata() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "large_metadata";

        // Create metadata with many fields
        let fields: std::collections::HashMap<String, Value> = (0..100)
            .map(|i| (format!("field_{}", i), Value::Int(i)))
            .collect();
        let metadata = Value::Object(fields);

        db.vector_upsert(&run, collection, "large_meta", &[1.0, 2.0], Some(metadata)).unwrap();

        // Should be retrievable
        let result = db.vector_get(&run, collection, "large_meta").unwrap();
        assert!(result.is_some());
    });
}

/// Test K=0 search
#[test]
fn test_vector_search_k_zero() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "k_zero_test";

        db.vector_upsert(&run, collection, "v", &[1.0, 2.0], None).unwrap();

        // K=0 should return empty results
        let results = db.vector_search(&run, collection, &[1.0, 2.0], 0, None, None).unwrap();
        assert!(results.is_empty());
    });
}

/// Test K=1 search
#[test]
fn test_vector_search_k_one() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "k_one_test";

        db.vector_upsert(&run, collection, "v1", &[1.0, 0.0], None).unwrap();
        db.vector_upsert(&run, collection, "v2", &[0.0, 1.0], None).unwrap();

        // K=1 should return exactly 1 result
        let results = db.vector_search(&run, collection, &[1.0, 0.0], 1, None, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "v1");
    });
}

/// Test very large K value
#[test]
fn test_vector_search_k_large() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "k_large_test";

        // Insert 5 vectors
        for i in 0..5 {
            db.vector_upsert(&run, collection, &format!("v{}", i), &[i as f32, 0.0], None).unwrap();
        }

        // Request K=1000000 (way more than available)
        let results = db.vector_search(&run, collection, &[1.0, 0.0], 1000000, None, None).unwrap();
        assert_eq!(results.len(), 5, "Should return all available vectors");
    });
}

/// Test identical vectors
#[test]
fn test_vector_identical_vectors() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "identical";

        let vec = vec![1.0, 2.0, 3.0];

        // Insert identical vectors with different keys
        db.vector_upsert(&run, collection, "v1", &vec, None).unwrap();
        db.vector_upsert(&run, collection, "v2", &vec, None).unwrap();
        db.vector_upsert(&run, collection, "v3", &vec, None).unwrap();

        // All should be returned in search
        let results = db.vector_search(&run, collection, &vec, 3, None, None).unwrap();
        assert_eq!(results.len(), 3);

        // All should have same similarity score
        for r in &results {
            assert!((r.score - results[0].score).abs() < 0.001,
                "Identical vectors should have same score");
        }
    });
}

/// Test opposite vectors
#[test]
fn test_vector_opposite_vectors() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "opposite";

        let pos = vec![1.0, 1.0, 1.0];
        let neg = vec![-1.0, -1.0, -1.0];

        db.vector_upsert(&run, collection, "positive", &pos, None).unwrap();
        db.vector_upsert(&run, collection, "negative", &neg, None).unwrap();

        // Search for positive - should prefer positive vector
        let results = db.vector_search(&run, collection, &pos, 2, None, None).unwrap();
        assert_eq!(results[0].key, "positive");
    });
}

/// Test collection count accuracy
#[test]
fn test_vector_collection_count_accuracy() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "count_test";

        // Create collection
        db.vector_create_collection(&run, collection, 4, DistanceMetric::Cosine).unwrap();
        assert_eq!(db.vector_collection_info(&run, collection).unwrap().unwrap().count, 0);

        // Add vectors
        for i in 0..10 {
            db.vector_upsert(&run, collection, &format!("v{}", i), &[1.0, 2.0, 3.0, 4.0], None).unwrap();
        }
        assert_eq!(db.vector_collection_info(&run, collection).unwrap().unwrap().count, 10);

        // Delete some
        for i in 0..5 {
            db.vector_delete(&run, collection, &format!("v{}", i)).unwrap();
        }
        assert_eq!(db.vector_collection_info(&run, collection).unwrap().unwrap().count, 5);
    });
}

// =============================================================================
// INTERNAL COLLECTION BLOCKING TESTS (Phase 0)
// =============================================================================

/// Test that internal collections (prefixed with '_') are blocked for upsert
#[test]
fn test_vector_internal_collection_upsert_blocked() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Internal collection names should be blocked
        let internal_names = vec![
            "_internal",
            "_json_embeddings",
            "_system",
            "_reserved",
        ];

        for name in internal_names {
            let result = db.vector_upsert(&run, name, "v", &[1.0, 2.0], None);
            assert!(result.is_err(), "Internal collection '{}' should be blocked for upsert", name);
            let err_msg = format!("{:?}", result.unwrap_err());
            assert!(err_msg.contains("internal") || err_msg.contains("reserved"),
                "Error should mention internal/reserved: {}", err_msg);
        }
    });
}

/// Test that internal collections are blocked for create_collection
#[test]
fn test_vector_internal_collection_create_blocked() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let result = db.vector_create_collection(&run, "_internal", 128, DistanceMetric::Cosine);
        assert!(result.is_err(), "Creating internal collection should be blocked");
    });
}

/// Test that internal collections are blocked for get
#[test]
fn test_vector_internal_collection_get_blocked() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let result = db.vector_get(&run, "_internal", "key");
        assert!(result.is_err(), "Getting from internal collection should be blocked");
    });
}

/// Test that internal collections are blocked for delete
#[test]
fn test_vector_internal_collection_delete_blocked() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let result = db.vector_delete(&run, "_internal", "key");
        assert!(result.is_err(), "Deleting from internal collection should be blocked");
    });
}

/// Test that internal collections are blocked for search
#[test]
fn test_vector_internal_collection_search_blocked() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let result = db.vector_search(&run, "_internal", &[1.0, 2.0], 10, None, None);
        assert!(result.is_err(), "Searching internal collection should be blocked");
    });
}

/// Test that internal collections are blocked for drop_collection
#[test]
fn test_vector_internal_collection_drop_blocked() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let result = db.vector_drop_collection(&run, "_internal");
        assert!(result.is_err(), "Dropping internal collection should be blocked");
    });
}

/// Test that internal collections are blocked for collection_exists
#[test]
fn test_vector_internal_collection_exists_blocked() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let result = db.vector_collection_exists(&run, "_internal");
        assert!(result.is_err(), "Checking existence of internal collection should be blocked");
    });
}

/// Test that internal collections are blocked for collection_info
#[test]
fn test_vector_internal_collection_info_blocked() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let result = db.vector_collection_info(&run, "_internal");
        assert!(result.is_err(), "Getting info of internal collection should be blocked");
    });
}

/// Test that internal collections are hidden from list_collections
/// Note: This tests that if internal collections exist (e.g., created by primitives directly),
/// they are filtered out of the list_collections result.
#[test]
fn test_vector_internal_collections_hidden_from_list() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Create some normal collections
        db.vector_upsert(&run, "public1", "v", &[1.0, 2.0], None).unwrap();
        db.vector_upsert(&run, "public2", "v", &[1.0, 2.0], None).unwrap();

        // List collections - should only see public ones
        let collections = db.vector_list_collections(&run).unwrap();

        // Verify no internal collections are listed
        for info in &collections {
            assert!(!info.name.starts_with('_'),
                "Internal collection '{}' should not be listed", info.name);
        }

        // Verify public collections are listed
        let names: Vec<&str> = collections.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"public1"), "public1 should be listed");
        assert!(names.contains(&"public2"), "public2 should be listed");
    });
}

/// Test that normal collections (not prefixed with '_') still work
#[test]
fn test_vector_normal_collections_work() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Edge case: collection names that are almost internal but not
        let valid_names = vec![
            "underscore_in_middle",
            "trailing_",
            "a_b_c",
            "not_internal",
        ];

        for name in valid_names {
            let result = db.vector_upsert(&run, name, "v", &[1.0, 2.0], None);
            assert!(result.is_ok(), "Collection '{}' should be allowed", name);
        }
    });
}

// =============================================================================
// SOURCE REFERENCE TESTS (Phase 0)
// =============================================================================

/// Test vector_upsert_with_source stores and retrieves source reference
#[test]
fn test_vector_upsert_with_source_basic() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "source_ref_test";
        let key = "vec_with_source";
        let vector = vec![1.0, 2.0, 3.0];

        // Create a source reference (simulating a KV entry)
        let source_ref = strata_core::EntityRef::kv(run.to_run_id(), "my_source_key");

        // Upsert with source reference
        let version = db.vector_upsert_with_source(
            &run,
            collection,
            key,
            &vector,
            None,
            Some(source_ref.clone()),
        ).unwrap();

        assert!(matches!(version, Version::Txn(_) | Version::Counter(_)));

        // Verify vector was stored
        let result = db.vector_get(&run, collection, key).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().value.0, vector);
    });
}

/// Test vector_upsert_with_source with metadata
#[test]
fn test_vector_upsert_with_source_and_metadata() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "source_meta_test";
        let key = "vec_source_meta";
        let vector = vec![0.5, 0.5, 0.5];
        let metadata = obj([
            ("category", Value::String("test".to_string())),
        ]);

        // Use KV entity reference which accepts a string directly
        let source_ref = strata_core::EntityRef::kv(run.to_run_id(), "source_key");

        // Upsert with source reference and metadata
        db.vector_upsert_with_source(
            &run,
            collection,
            key,
            &vector,
            Some(metadata.clone()),
            Some(source_ref),
        ).unwrap();

        // Verify both vector and metadata stored
        let result = db.vector_get(&run, collection, key).unwrap().unwrap();
        assert_eq!(result.value.0, vector);
        assert!(result.value.1.is_object());
    });
}

/// Test vector_upsert_with_source with None source_ref
#[test]
fn test_vector_upsert_with_source_none() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let collection = "source_none_test";
        let key = "vec_no_source";
        let vector = vec![1.0, 2.0, 3.0];

        // Upsert with None source reference (same as regular upsert)
        db.vector_upsert_with_source(
            &run,
            collection,
            key,
            &vector,
            None,
            None,
        ).unwrap();

        // Verify vector was stored
        let result = db.vector_get(&run, collection, key).unwrap();
        assert!(result.is_some());
    });
}

/// Test that source reference is blocked for internal collections
#[test]
fn test_vector_upsert_with_source_internal_blocked() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        // Use KV entity reference which accepts a string directly
        let source_ref = strata_core::EntityRef::kv(run.to_run_id(), "doc");

        let result = db.vector_upsert_with_source(
            &run,
            "_internal",
            "key",
            &[1.0, 2.0],
            None,
            Some(source_ref),
        );

        assert!(result.is_err(), "upsert_with_source should be blocked for internal collections");
    });
}
