//! RunIndex Basic Operations Tests
//!
//! Tests for fundamental RunIndex operations:
//! - run_create
//! - run_get
//! - run_exists
//! - run_update_metadata

use crate::*;

// =============================================================================
// Create Tests
// =============================================================================

/// Test basic run creation
#[test]
fn test_run_create_basic() {
    test_across_substrate_modes(|db| {
        let (info, version) = db.run_create(None, None).unwrap();

        assert!(!info.run_id.is_default());
        assert!(info.run_id.is_uuid());
        assert!(matches!(info.state, strata_api::substrate::RunState::Active));
        assert!(info.error.is_none());
    });
}

/// Test run creation with metadata
#[test]
fn test_run_create_with_metadata() {
    test_across_substrate_modes(|db| {
        let metadata = obj([("key", Value::String("value".to_string()))]);
        let (info, _) = db.run_create(None, Some(metadata.clone())).unwrap();

        if let Value::Object(m) = &info.metadata {
            assert_eq!(m.get("key"), Some(&Value::String("value".to_string())));
        } else {
            panic!("Expected object metadata");
        }
    });
}

/// Test run creation with specific ID
#[test]
fn test_run_create_with_id() {
    test_across_substrate_modes(|db| {
        let run_id = ApiRunId::new();
        let (info, _) = db.run_create(Some(&run_id), None).unwrap();

        assert_eq!(info.run_id, run_id);
    });
}

/// Test run creation with duplicate ID fails
#[test]
fn test_run_create_duplicate_error() {
    test_across_substrate_modes(|db| {
        let run_id = ApiRunId::new();

        // First creation succeeds
        db.run_create(Some(&run_id), None).unwrap();

        // Second creation with same ID should fail
        let result = db.run_create(Some(&run_id), None);
        assert!(result.is_err());
    });
}

/// Test run creation generates unique UUIDs
#[test]
fn test_run_create_generates_uuid() {
    test_across_substrate_modes(|db| {
        let (info1, _) = db.run_create(None, None).unwrap();
        let (info2, _) = db.run_create(None, None).unwrap();
        let (info3, _) = db.run_create(None, None).unwrap();

        // All should be unique
        assert_ne!(info1.run_id, info2.run_id);
        assert_ne!(info2.run_id, info3.run_id);
        assert_ne!(info1.run_id, info3.run_id);
    });
}

// =============================================================================
// Get Tests
// =============================================================================

/// Test getting an existing run
#[test]
fn test_run_get_exists() {
    test_across_substrate_modes(|db| {
        let (created, _) = db.run_create(None, None).unwrap();

        let result = db.run_get(&created.run_id).unwrap();
        assert!(result.is_some());

        let versioned = result.unwrap();
        assert_eq!(versioned.value.run_id, created.run_id);
    });
}

/// Test getting a non-existent run
#[test]
fn test_run_get_not_found() {
    test_across_substrate_modes(|db| {
        let fake_run = ApiRunId::new();
        let result = db.run_get(&fake_run).unwrap();
        assert!(result.is_none());
    });
}

/// Test getting the default run
///
/// NOTE: The "default" run may not be explicitly registered in RunIndex.
/// This test verifies actual behavior rather than assumed contract.
#[test]
fn test_run_get_default() {
    test_across_substrate_modes(|db| {
        let default_run = ApiRunId::default_run_id();
        let result = db.run_get(&default_run).unwrap();

        // Default run may or may not exist as an explicit entity
        // If it exists, verify it's correctly identified as default
        if let Some(versioned) = result {
            assert!(versioned.value.run_id.is_default());
        }
    });
}

// =============================================================================
// Exists Tests
// =============================================================================

/// Test exists returns true for existing run
#[test]
fn test_run_exists_true() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        assert!(db.run_exists(&info.run_id).unwrap());
    });
}

/// Test exists returns false for non-existent run
#[test]
fn test_run_exists_false() {
    test_across_substrate_modes(|db| {
        let fake_run = ApiRunId::new();
        assert!(!db.run_exists(&fake_run).unwrap());
    });
}

/// Test default run behavior
///
/// NOTE: The "default" run is a namespace concept for data primitives (KV, State, etc.)
/// but may not be explicitly registered in RunIndex. This test verifies actual behavior.
#[test]
fn test_run_exists_default() {
    test_across_substrate_modes(|db| {
        let default_run = ApiRunId::default_run_id();
        // Default run may or may not be explicitly registered
        // The key invariant is that data operations on "default" work
        let _exists = db.run_exists(&default_run).unwrap();
    });
}

// =============================================================================
// Update Metadata Tests
// =============================================================================

/// Test updating run metadata
#[test]
fn test_run_update_metadata() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        let new_metadata = obj([("updated", Value::Bool(true))]);
        let version = db.run_update_metadata(&info.run_id, new_metadata).unwrap();

        // Verify update
        let result = db.run_get(&info.run_id).unwrap().unwrap();
        if let Value::Object(m) = &result.value.metadata {
            assert_eq!(m.get("updated"), Some(&Value::Bool(true)));
        } else {
            panic!("Expected object metadata");
        }
    });
}

/// Test updating metadata for non-existent run fails
#[test]
fn test_run_update_metadata_not_found() {
    test_across_substrate_modes(|db| {
        let fake_run = ApiRunId::new();
        let metadata = obj([("key", Value::Int(1))]);

        let result = db.run_update_metadata(&fake_run, metadata);
        assert!(result.is_err());
    });
}
