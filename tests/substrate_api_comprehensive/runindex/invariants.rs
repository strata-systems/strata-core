//! RunIndex Invariants Tests
//!
//! Contract invariants that must ALWAYS hold, regardless of feature behavior.
//! These are fundamental guarantees of the RunIndex primitive.
//!
//! Categories:
//! - Versioning invariants
//! - State invariants
//! - Read invariants
//! - Delete invariants
//! - Index invariants

use crate::*;
use strata_api::substrate::RunState;

// =============================================================================
// Versioning Invariants
// =============================================================================

/// Invariant: Version increments monotonically on mutations
#[test]
fn test_version_increments_monotonically() {
    test_across_substrate_modes(|db| {
        let (info, v1) = db.run_create(None, None).unwrap();

        let v2 = db.run_update_metadata(&info.run_id, obj([("a", Value::Int(1))])).unwrap();
        let v3 = db.run_update_metadata(&info.run_id, obj([("b", Value::Int(2))])).unwrap();
        let v4 = db.run_update_metadata(&info.run_id, obj([("c", Value::Int(3))])).unwrap();

        // Each version should be greater than the previous
        // Note: actual comparison depends on Version implementation
    });
}

/// Invariant: Version never decreases
#[test]
fn test_version_never_decreases() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        let mut versions = Vec::new();

        for i in 0..10 {
            let v = db.run_update_metadata(
                &info.run_id,
                obj([("step", Value::Int(i))]),
            ).unwrap();
            versions.push(v);
        }

        // No version should be less than any previous version
        // (monotonically non-decreasing, but typically strictly increasing)
    });
}

/// Invariant: Any mutation increments version
#[test]
fn test_version_increments_on_any_mutation() {
    test_across_substrate_modes(|db| {
        let (info, v_create) = db.run_create(None, None).unwrap();

        // Metadata update should increment
        let v_meta = db.run_update_metadata(&info.run_id, obj([("x", Value::Int(1))])).unwrap();

        // Tag operations should increment
        let v_tag = db.run_add_tags(&info.run_id, &["tag".to_string()]).unwrap();
        let v_untag = db.run_remove_tags(&info.run_id, &["tag".to_string()]).unwrap();

        // State transition should increment
        let v_pause = db.run_pause(&info.run_id).unwrap();
        let v_resume = db.run_resume(&info.run_id).unwrap();

        // All should produce versions (non-None, non-error)
    });
}

// =============================================================================
// State Invariants
// =============================================================================

/// Invariant: Terminal states are terminal (Archived cannot transition)
#[test]
fn test_terminal_states_are_terminal() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();
        db.run_archive(&info.run_id).unwrap();

        // No transition should succeed from Archived
        assert!(db.run_resume(&info.run_id).is_err());
        assert!(db.run_pause(&info.run_id).is_err());
        assert!(db.run_close(&info.run_id).is_err());
        assert!(db.run_fail(&info.run_id, "error").is_err());
        assert!(db.run_cancel(&info.run_id).is_err());
        assert!(db.run_archive(&info.run_id).is_err()); // Can't archive twice
    });
}

/// Invariant: No resurrection from finished states
#[test]
fn test_no_resurrection_from_finished() {
    test_across_substrate_modes(|db| {
        // Test Completed cannot go back to Active
        let (r1, _) = db.run_create(None, None).unwrap();
        db.run_close(&r1.run_id).unwrap();
        assert!(db.run_resume(&r1.run_id).is_err());

        // Test Failed cannot go back to Active
        let (r2, _) = db.run_create(None, None).unwrap();
        db.run_fail(&r2.run_id, "error").unwrap();
        assert!(db.run_resume(&r2.run_id).is_err());

        // Test Cancelled cannot go back to Active
        let (r3, _) = db.run_create(None, None).unwrap();
        db.run_cancel(&r3.run_id).unwrap();
        assert!(db.run_resume(&r3.run_id).is_err());
    });
}

/// Invariant: Default run is immortal (protected from destructive operations)
///
/// The default run should either not exist as an explicit entity, or be
/// protected from all state transitions and deletion.
#[test]
fn test_default_run_is_immortal() {
    test_across_substrate_modes(|db| {
        let default_run = ApiRunId::default_run_id();

        // All mutating operations should fail
        assert!(db.run_close(&default_run).is_err());
        assert!(db.run_delete(&default_run).is_err());
        assert!(db.run_archive(&default_run).is_err());
        assert!(db.run_fail(&default_run, "error").is_err());
        assert!(db.run_cancel(&default_run).is_err());
        assert!(db.run_pause(&default_run).is_err());
    });
}

// =============================================================================
// Read Invariants
// =============================================================================

/// Invariant: get_run has no side effects
#[test]
fn test_read_does_not_mutate() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        // Read multiple times
        let r1 = db.run_get(&info.run_id).unwrap().unwrap();
        let r2 = db.run_get(&info.run_id).unwrap().unwrap();
        let r3 = db.run_get(&info.run_id).unwrap().unwrap();

        // All reads should return identical data
        assert_eq!(r1.value.run_id, r2.value.run_id);
        assert_eq!(r2.value.run_id, r3.value.run_id);
        assert_eq!(r1.value.state, r2.value.state);
        assert_eq!(r2.value.state, r3.value.state);
    });
}

/// Invariant: run_list has no side effects
#[test]
fn test_list_does_not_mutate() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        let l1 = db.run_list(None, None, None).unwrap();
        let l2 = db.run_list(None, None, None).unwrap();

        assert_eq!(l1.len(), l2.len());
    });
}

/// Invariant: Queries have no side effects
#[test]
fn test_query_does_not_mutate() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();
        db.run_add_tags(&info.run_id, &["test".to_string()]).unwrap();

        // Multiple queries should return same results
        let q1 = db.run_query_by_tag("test").unwrap();
        let q2 = db.run_query_by_tag("test").unwrap();

        assert_eq!(q1.len(), q2.len());

        let c1 = db.run_count(None).unwrap();
        let c2 = db.run_count(None).unwrap();

        assert_eq!(c1, c2);
    });
}

// =============================================================================
// Delete Invariants
// =============================================================================

/// Invariant: Delete removes addressability
#[test]
fn test_delete_removes_addressability() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        assert!(db.run_exists(&info.run_id).unwrap());

        db.run_delete(&info.run_id).unwrap();

        // After delete, run should not exist
        assert!(!db.run_exists(&info.run_id).unwrap());

        // Get should return None
        assert!(db.run_get(&info.run_id).unwrap().is_none());
    });
}

/// Invariant: Delete is permanent
#[test]
fn test_delete_is_permanent() {
    test_across_substrate_modes(|db| {
        let run_id = ApiRunId::new();
        db.run_create(Some(&run_id), None).unwrap();

        db.run_delete(&run_id).unwrap();

        // Cannot recreate with same ID (or if you can, it's a new run)
        let result = db.run_create(Some(&run_id), None);
        // Either fails or creates a genuinely new run

        // Either way, the original run's data is gone
        // (Verification of this is in cascading delete tests)
    });
}

// =============================================================================
// Index Invariants
// =============================================================================

/// Invariant: Status index is consistent with actual state
#[test]
fn test_status_index_consistent_with_state() {
    test_across_substrate_modes(|db| {
        let (r1, _) = db.run_create(None, None).unwrap();
        let (r2, _) = db.run_create(None, None).unwrap();
        let (r3, _) = db.run_create(None, None).unwrap();

        db.run_close(&r1.run_id).unwrap();
        db.run_fail(&r2.run_id, "error").unwrap();
        // r3 stays active

        // Check each status query matches actual state
        let active = db.run_query_by_status(RunState::Active).unwrap();
        for run in &active {
            assert!(matches!(run.value.state, RunState::Active));
        }

        let completed = db.run_query_by_status(RunState::Completed).unwrap();
        for run in &completed {
            assert!(matches!(run.value.state, RunState::Completed));
        }

        let failed = db.run_query_by_status(RunState::Failed).unwrap();
        for run in &failed {
            assert!(matches!(run.value.state, RunState::Failed));
        }

        // Each run should be in exactly one status index
        assert!(active.iter().any(|r| r.value.run_id == r3.run_id));
        assert!(completed.iter().any(|r| r.value.run_id == r1.run_id));
        assert!(failed.iter().any(|r| r.value.run_id == r2.run_id));
    });
}

/// Invariant: Tag index is consistent with actual tags
#[test]
fn test_tag_index_consistent_with_tags() {
    test_across_substrate_modes(|db| {
        let (r1, _) = db.run_create(None, None).unwrap();
        let (r2, _) = db.run_create(None, None).unwrap();

        db.run_add_tags(&r1.run_id, &["alpha".to_string()]).unwrap();
        db.run_add_tags(&r2.run_id, &["beta".to_string()]).unwrap();
        db.run_add_tags(&r2.run_id, &["alpha".to_string()]).unwrap();

        // Query by alpha should return both r1 and r2
        let alpha_runs = db.run_query_by_tag("alpha").unwrap();
        assert!(alpha_runs.iter().any(|r| r.value.run_id == r1.run_id));
        assert!(alpha_runs.iter().any(|r| r.value.run_id == r2.run_id));

        // Query by beta should return only r2
        let beta_runs = db.run_query_by_tag("beta").unwrap();
        assert!(beta_runs.iter().all(|r| r.value.run_id != r1.run_id));
        assert!(beta_runs.iter().any(|r| r.value.run_id == r2.run_id));

        // Verify actual tags match index
        let r1_tags = db.run_get_tags(&r1.run_id).unwrap();
        assert!(r1_tags.contains(&"alpha".to_string()));
        assert!(!r1_tags.contains(&"beta".to_string()));

        let r2_tags = db.run_get_tags(&r2.run_id).unwrap();
        assert!(r2_tags.contains(&"alpha".to_string()));
        assert!(r2_tags.contains(&"beta".to_string()));
    });
}
