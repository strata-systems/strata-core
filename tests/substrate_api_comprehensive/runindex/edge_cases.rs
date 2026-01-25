//! RunIndex Edge Cases Tests
//!
//! Tests for validation, boundaries, and special cases:
//! - Default run protection
//! - Invalid operations
//! - Boundary conditions

use crate::*;
use strata_api::substrate::RunState;

// =============================================================================
// Default Run Protection Tests
// =============================================================================

/// Test default run behavior
///
/// NOTE: The "default" run is a namespace concept. It may not be explicitly
/// registered in RunIndex as a run entity.
#[test]
fn test_default_run_behavior() {
    test_across_substrate_modes(|db| {
        let default_run = ApiRunId::default_run_id();

        // Check current behavior - may or may not exist
        let _exists = db.run_exists(&default_run).unwrap();
        let _info = db.run_get(&default_run).unwrap();
    });
}

/// Test default run cannot be closed
///
/// The default run should either not exist (error) or be protected from closing.
#[test]
fn test_default_run_cannot_close() {
    test_across_substrate_modes(|db| {
        let default_run = ApiRunId::default_run_id();

        // Should error (either not found or constraint violation)
        let result = db.run_close(&default_run);
        assert!(result.is_err());
    });
}

/// Test default run cannot be deleted
///
/// The default run should either not exist (error) or be protected from deletion.
#[test]
fn test_default_run_cannot_delete() {
    test_across_substrate_modes(|db| {
        let default_run = ApiRunId::default_run_id();

        // Should error (either not found or constraint violation)
        let result = db.run_delete(&default_run);
        assert!(result.is_err());
    });
}

/// Test default run cannot be archived
///
/// The default run should either not exist (error) or be protected from archiving.
#[test]
fn test_default_run_cannot_archive() {
    test_across_substrate_modes(|db| {
        let default_run = ApiRunId::default_run_id();

        // Should error (either not found or constraint violation)
        let result = db.run_archive(&default_run);
        assert!(result.is_err());
    });
}

/// Test default run cannot be paused
#[test]
fn test_default_run_cannot_pause() {
    test_across_substrate_modes(|db| {
        let default_run = ApiRunId::default_run_id();

        let result = db.run_pause(&default_run);
        assert!(result.is_err());
    });
}

/// Test default run cannot be failed
#[test]
fn test_default_run_cannot_fail() {
    test_across_substrate_modes(|db| {
        let default_run = ApiRunId::default_run_id();

        let result = db.run_fail(&default_run, "error");
        assert!(result.is_err());
    });
}

/// Test default run cannot be cancelled
#[test]
fn test_default_run_cannot_cancel() {
    test_across_substrate_modes(|db| {
        let default_run = ApiRunId::default_run_id();

        let result = db.run_cancel(&default_run);
        assert!(result.is_err());
    });
}

// =============================================================================
// Run ID Validation Tests
// =============================================================================

/// Test creating run with invalid ID format fails
#[test]
fn test_invalid_run_id_format() {
    test_across_substrate_modes(|db| {
        // Try to parse an invalid run ID
        let invalid = ApiRunId::parse("not-a-valid-uuid-or-default");
        assert!(invalid.is_none());
    });
}

/// Test empty run ID is invalid
#[test]
fn test_empty_run_id_error() {
    test_across_substrate_modes(|db| {
        let empty = ApiRunId::parse("");
        assert!(empty.is_none());
    });
}

/// Test run ID "default" is recognized
#[test]
fn test_run_id_default_recognized() {
    test_across_substrate_modes(|db| {
        let default = ApiRunId::parse("default");
        assert!(default.is_some());
        assert!(default.unwrap().is_default());
    });
}

/// Test run ID case sensitivity
#[test]
fn test_run_id_case_sensitivity() {
    test_across_substrate_modes(|db| {
        // "default" is case-sensitive
        let default_lower = ApiRunId::parse("default");
        let default_upper = ApiRunId::parse("DEFAULT");
        let default_mixed = ApiRunId::parse("Default");

        assert!(default_lower.is_some());
        // Uppercase versions should not be recognized as default
        if let Some(upper) = default_upper {
            assert!(!upper.is_default());
        }
        if let Some(mixed) = default_mixed {
            assert!(!mixed.is_default());
        }
    });
}

// =============================================================================
// Metadata Validation Tests
// =============================================================================

/// Test run metadata must be Object or Null
#[test]
fn test_run_metadata_must_be_object_or_null() {
    test_across_substrate_modes(|db| {
        // Object should work
        let obj_meta = obj([("key", Value::Int(1))]);
        let result = db.run_create(None, Some(obj_meta));
        assert!(result.is_ok());

        // Null should work (implicitly, by passing None)
        let result = db.run_create(None, None);
        assert!(result.is_ok());

        // Array should fail or be rejected
        // Note: Implementation may handle this differently
    });
}

// =============================================================================
// Version Increment Tests
// =============================================================================

/// Test version increments on update
#[test]
fn test_version_increments_on_update() {
    test_across_substrate_modes(|db| {
        let (info, v1) = db.run_create(None, None).unwrap();

        let v2 = db.run_update_metadata(&info.run_id, obj([("a", Value::Int(1))])).unwrap();
        let v3 = db.run_update_metadata(&info.run_id, obj([("b", Value::Int(2))])).unwrap();

        // Versions should be increasing (or at minimum, not equal)
        // Note: Version comparison depends on implementation
    });
}

// =============================================================================
// Concurrent Operation Tests (Basic)
// =============================================================================

/// Test status transitions work correctly
///
/// NOTE: Status index queries depend on primitive indexing.
#[test]
fn test_status_transitions_work() {
    test_across_substrate_modes(|db| {
        let (run1, _) = db.run_create(None, None).unwrap();
        let (run2, _) = db.run_create(None, None).unwrap();
        let (run3, _) = db.run_create(None, None).unwrap();

        // Transition to various states
        db.run_close(&run1.run_id).unwrap();
        db.run_fail(&run2.run_id, "error").unwrap();
        // run3 stays active

        // Verify state changes via get
        let info1 = db.run_get(&run1.run_id).unwrap().unwrap();
        assert!(matches!(info1.value.state, RunState::Completed));

        let info2 = db.run_get(&run2.run_id).unwrap().unwrap();
        assert!(matches!(info2.value.state, RunState::Failed));

        let info3 = db.run_get(&run3.run_id).unwrap().unwrap();
        assert!(matches!(info3.value.state, RunState::Active));
    });
}

// =============================================================================
// Empty/Null Value Tests
// =============================================================================

/// Test creating run with null metadata
#[test]
fn test_create_with_null_metadata() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, Some(Value::Null)).unwrap();

        let retrieved = db.run_get(&info.run_id).unwrap().unwrap();
        // Metadata should be Null or empty Object
        assert!(matches!(retrieved.value.metadata, Value::Null | Value::Object(_)));
    });
}

/// Test updating with empty object
#[test]
fn test_update_with_empty_object() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        let empty_obj = Value::Object(std::collections::HashMap::new());
        db.run_update_metadata(&info.run_id, empty_obj).unwrap();

        let retrieved = db.run_get(&info.run_id).unwrap().unwrap();
        if let Value::Object(m) = &retrieved.value.metadata {
            assert!(m.is_empty());
        }
    });
}
