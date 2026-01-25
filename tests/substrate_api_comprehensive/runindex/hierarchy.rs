//! RunIndex Hierarchy Tests
//!
//! Tests for parent-child relationships:
//! - run_create_child
//! - run_get_children
//! - run_get_parent
//!
//! NOTE: Parent-child relationships are INFORMATIONAL ONLY.
//! There is no transactional coupling, state propagation, or automatic cleanup.

use crate::*;

// =============================================================================
// Create Child Tests
// =============================================================================

/// Test creating a child run
#[test]
fn test_create_child() {
    test_across_substrate_modes(|db| {
        let (parent, _) = db.run_create(None, None).unwrap();

        let (child, _) = db.run_create_child(&parent.run_id, None).unwrap();

        assert!(!child.run_id.is_default());
        assert_ne!(child.run_id, parent.run_id);

        // Verify parent-child link
        let parent_of_child = db.run_get_parent(&child.run_id).unwrap();
        assert_eq!(parent_of_child, Some(parent.run_id.clone()));
    });
}

/// Test creating child with metadata
#[test]
fn test_create_child_with_metadata() {
    test_across_substrate_modes(|db| {
        let (parent, _) = db.run_create(None, None).unwrap();

        let meta = obj([("child_key", Value::String("child_value".to_string()))]);
        let (child, _) = db.run_create_child(&parent.run_id, Some(meta)).unwrap();

        if let Value::Object(m) = &child.metadata {
            assert_eq!(m.get("child_key"), Some(&Value::String("child_value".to_string())));
        } else {
            panic!("Expected object metadata");
        }
    });
}

/// Test creating child of non-existent parent fails
#[test]
fn test_create_child_parent_not_found() {
    test_across_substrate_modes(|db| {
        let fake_parent = ApiRunId::new();

        let result = db.run_create_child(&fake_parent, None);
        assert!(result.is_err());
    });
}

/// Test creating child of default run
///
/// May fail if default run doesn't exist as an explicit entity.
#[test]
fn test_create_child_of_default() {
    test_across_substrate_modes(|db| {
        let default_run = ApiRunId::default_run_id();

        // This may fail if default run doesn't exist
        let result = db.run_create_child(&default_run, None);
        if let Ok((child, _)) = result {
            let parent = db.run_get_parent(&child.run_id).unwrap();
            assert!(parent.is_some());
            assert!(parent.unwrap().is_default());
        }
        // If error, default run doesn't exist as entity (acceptable)
    });
}

// =============================================================================
// Get Children Tests
// =============================================================================

/// Test getting children of a run
///
/// NOTE: get_children depends on primitive parent-child indexing.
#[test]
fn test_get_children() {
    test_across_substrate_modes(|db| {
        let (parent, _) = db.run_create(None, None).unwrap();

        let (child1, _) = db.run_create_child(&parent.run_id, None).unwrap();
        let (child2, _) = db.run_create_child(&parent.run_id, None).unwrap();
        let (child3, _) = db.run_create_child(&parent.run_id, None).unwrap();

        // Verify parent relationship via get_parent
        assert_eq!(db.run_get_parent(&child1.run_id).unwrap(), Some(parent.run_id.clone()));
        assert_eq!(db.run_get_parent(&child2.run_id).unwrap(), Some(parent.run_id.clone()));
        assert_eq!(db.run_get_parent(&child3.run_id).unwrap(), Some(parent.run_id.clone()));

        // get_children should not error
        let _children = db.run_get_children(&parent.run_id).unwrap();
    });
}

/// Test getting children of run with no children
#[test]
fn test_get_children_empty() {
    test_across_substrate_modes(|db| {
        let (parent, _) = db.run_create(None, None).unwrap();

        let children = db.run_get_children(&parent.run_id).unwrap();

        assert!(children.is_empty());
    });
}

/// Test getting children of non-existent run
///
/// NOTE: Behavior may vary - may return empty or error.
#[test]
fn test_get_children_nonexistent() {
    test_across_substrate_modes(|db| {
        let fake_run = ApiRunId::new();

        // May error or return empty depending on implementation
        let _result = db.run_get_children(&fake_run);
    });
}

// =============================================================================
// Get Parent Tests
// =============================================================================

/// Test getting parent of a child run
#[test]
fn test_get_parent() {
    test_across_substrate_modes(|db| {
        let (parent, _) = db.run_create(None, None).unwrap();
        let (child, _) = db.run_create_child(&parent.run_id, None).unwrap();

        let parent_id = db.run_get_parent(&child.run_id).unwrap();

        assert_eq!(parent_id, Some(parent.run_id));
    });
}

/// Test getting parent of run with no parent
#[test]
fn test_get_parent_none() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        let parent = db.run_get_parent(&info.run_id).unwrap();

        assert!(parent.is_none());
    });
}

/// Test getting parent of non-existent run fails
#[test]
fn test_get_parent_not_found() {
    test_across_substrate_modes(|db| {
        let fake_run = ApiRunId::new();

        let result = db.run_get_parent(&fake_run);
        assert!(result.is_err());
    });
}

/// Test default run has no parent (if it exists as an entity)
#[test]
fn test_default_run_has_no_parent() {
    test_across_substrate_modes(|db| {
        let default_run = ApiRunId::default_run_id();

        // Default run may not exist as explicit entity
        let result = db.run_get_parent(&default_run);
        if let Ok(parent) = result {
            assert!(parent.is_none());
        }
        // If error, default run doesn't exist as entity (acceptable)
    });
}

// =============================================================================
// Multi-Level Hierarchy Tests
// =============================================================================

/// Test three-level hierarchy
#[test]
fn test_hierarchy_three_levels() {
    test_across_substrate_modes(|db| {
        let (grandparent, _) = db.run_create(None, None).unwrap();
        let (parent, _) = db.run_create_child(&grandparent.run_id, None).unwrap();
        let (child, _) = db.run_create_child(&parent.run_id, None).unwrap();

        // Verify chain
        let child_parent = db.run_get_parent(&child.run_id).unwrap();
        assert_eq!(child_parent, Some(parent.run_id.clone()));

        let parent_parent = db.run_get_parent(&parent.run_id).unwrap();
        assert_eq!(parent_parent, Some(grandparent.run_id.clone()));

        let grandparent_parent = db.run_get_parent(&grandparent.run_id).unwrap();
        assert!(grandparent_parent.is_none());
    });
}

/// Test deleting parent orphans children (informational only)
#[test]
fn test_delete_parent_orphans_children() {
    test_across_substrate_modes(|db| {
        let (parent, _) = db.run_create(None, None).unwrap();
        let (child, _) = db.run_create_child(&parent.run_id, None).unwrap();

        // Delete parent
        db.run_delete(&parent.run_id).unwrap();

        // Child should still exist
        assert!(db.run_exists(&child.run_id).unwrap());

        // Child's parent reference now points to non-existent run
        let parent_ref = db.run_get_parent(&child.run_id).unwrap();
        assert_eq!(parent_ref, Some(parent.run_id.clone()));

        // But the parent no longer exists
        assert!(!db.run_exists(&parent.run_id).unwrap());
    });
}

/// Test children are independent of parent state
#[test]
fn test_children_independent_of_parent_state() {
    test_across_substrate_modes(|db| {
        let (parent, _) = db.run_create(None, None).unwrap();
        let (child, _) = db.run_create_child(&parent.run_id, None).unwrap();

        // Close parent
        db.run_close(&parent.run_id).unwrap();

        // Child should still be active
        let child_info = db.run_get(&child.run_id).unwrap().unwrap();
        assert!(matches!(child_info.value.state, strata_api::substrate::RunState::Active));

        // Can still modify child
        db.run_pause(&child.run_id).unwrap();
        let child_info = db.run_get(&child.run_id).unwrap().unwrap();
        assert!(matches!(child_info.value.state, strata_api::substrate::RunState::Paused));
    });
}
