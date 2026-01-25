//! RunIndex Lifecycle Tests
//!
//! Tests for run state transitions:
//! - Active -> Completed (close)
//! - Active -> Failed (fail)
//! - Active -> Cancelled (cancel)
//! - Active -> Paused (pause)
//! - Active -> Archived (archive)
//! - Paused -> Active (resume)
//! - Paused -> Cancelled
//! - Paused -> Archived
//! - Completed/Failed/Cancelled -> Archived
//! - Invalid transitions (no resurrection, terminal finality)

use crate::*;
use strata_api::substrate::RunState;

// =============================================================================
// Valid State Transitions
// =============================================================================

/// Test Active -> Completed (close)
#[test]
fn test_active_to_completed() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();
        assert!(matches!(info.state, RunState::Active));

        db.run_close(&info.run_id).unwrap();

        let result = db.run_get(&info.run_id).unwrap().unwrap();
        assert!(matches!(result.value.state, RunState::Completed));
    });
}

/// Test Active -> Failed (fail)
#[test]
fn test_active_to_failed() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        db.run_fail(&info.run_id, "Something went wrong").unwrap();

        let result = db.run_get(&info.run_id).unwrap().unwrap();
        assert!(matches!(result.value.state, RunState::Failed));
        assert_eq!(result.value.error, Some("Something went wrong".to_string()));
    });
}

/// Test Active -> Cancelled (cancel)
#[test]
fn test_active_to_cancelled() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        db.run_cancel(&info.run_id).unwrap();

        let result = db.run_get(&info.run_id).unwrap().unwrap();
        assert!(matches!(result.value.state, RunState::Cancelled));
    });
}

/// Test Active -> Paused (pause)
#[test]
fn test_active_to_paused() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        db.run_pause(&info.run_id).unwrap();

        let result = db.run_get(&info.run_id).unwrap().unwrap();
        assert!(matches!(result.value.state, RunState::Paused));
    });
}

/// Test Active -> Archived (archive)
#[test]
fn test_active_to_archived() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        db.run_archive(&info.run_id).unwrap();

        let result = db.run_get(&info.run_id).unwrap().unwrap();
        assert!(matches!(result.value.state, RunState::Archived));
    });
}

/// Test Paused -> Active (resume)
#[test]
fn test_paused_to_active() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();
        db.run_pause(&info.run_id).unwrap();

        db.run_resume(&info.run_id).unwrap();

        let result = db.run_get(&info.run_id).unwrap().unwrap();
        assert!(matches!(result.value.state, RunState::Active));
    });
}

/// Test Paused -> Cancelled
#[test]
fn test_paused_to_cancelled() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();
        db.run_pause(&info.run_id).unwrap();

        db.run_cancel(&info.run_id).unwrap();

        let result = db.run_get(&info.run_id).unwrap().unwrap();
        assert!(matches!(result.value.state, RunState::Cancelled));
    });
}

/// Test Paused -> Archived
#[test]
fn test_paused_to_archived() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();
        db.run_pause(&info.run_id).unwrap();

        db.run_archive(&info.run_id).unwrap();

        let result = db.run_get(&info.run_id).unwrap().unwrap();
        assert!(matches!(result.value.state, RunState::Archived));
    });
}

/// Test Completed -> Archived
#[test]
fn test_completed_to_archived() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();
        db.run_close(&info.run_id).unwrap();

        db.run_archive(&info.run_id).unwrap();

        let result = db.run_get(&info.run_id).unwrap().unwrap();
        assert!(matches!(result.value.state, RunState::Archived));
    });
}

/// Test Failed -> Archived
#[test]
fn test_failed_to_archived() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();
        db.run_fail(&info.run_id, "error").unwrap();

        db.run_archive(&info.run_id).unwrap();

        let result = db.run_get(&info.run_id).unwrap().unwrap();
        assert!(matches!(result.value.state, RunState::Archived));
    });
}

/// Test Cancelled -> Archived
#[test]
fn test_cancelled_to_archived() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();
        db.run_cancel(&info.run_id).unwrap();

        db.run_archive(&info.run_id).unwrap();

        let result = db.run_get(&info.run_id).unwrap().unwrap();
        assert!(matches!(result.value.state, RunState::Archived));
    });
}

// =============================================================================
// Invalid Transitions (No Resurrection)
// =============================================================================

/// Test Completed cannot transition back to Active
#[test]
fn test_completed_cannot_activate() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();
        db.run_close(&info.run_id).unwrap();

        // Cannot resume a completed run
        let result = db.run_resume(&info.run_id);
        assert!(result.is_err());
    });
}

/// Test Failed cannot transition back to Active
#[test]
fn test_failed_cannot_activate() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();
        db.run_fail(&info.run_id, "error").unwrap();

        // Cannot resume a failed run
        let result = db.run_resume(&info.run_id);
        assert!(result.is_err());
    });
}

/// Test Cancelled cannot transition back to Active
#[test]
fn test_cancelled_cannot_activate() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();
        db.run_cancel(&info.run_id).unwrap();

        // Cannot resume a cancelled run
        let result = db.run_resume(&info.run_id);
        assert!(result.is_err());
    });
}

/// Test Archived is terminal - no transitions allowed
#[test]
fn test_archived_is_terminal() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();
        db.run_archive(&info.run_id).unwrap();

        // None of these should work on archived run
        assert!(db.run_resume(&info.run_id).is_err());
        assert!(db.run_pause(&info.run_id).is_err());
        assert!(db.run_close(&info.run_id).is_err());
        assert!(db.run_fail(&info.run_id, "error").is_err());
        assert!(db.run_cancel(&info.run_id).is_err());
        // Can't archive twice
        assert!(db.run_archive(&info.run_id).is_err());
    });
}

/// Test Paused cannot directly transition to Completed
#[test]
fn test_paused_cannot_complete() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();
        db.run_pause(&info.run_id).unwrap();

        // Cannot close a paused run
        let result = db.run_close(&info.run_id);
        assert!(result.is_err());
    });
}

// =============================================================================
// Error Cases
// =============================================================================

/// Test closing default run fails
#[test]
fn test_close_default_run_error() {
    test_across_substrate_modes(|db| {
        let default_run = ApiRunId::default_run_id();

        let result = db.run_close(&default_run);
        assert!(result.is_err());
    });
}

/// Test transition on non-existent run fails
#[test]
fn test_transition_not_found_error() {
    test_across_substrate_modes(|db| {
        let fake_run = ApiRunId::new();

        assert!(db.run_close(&fake_run).is_err());
        assert!(db.run_pause(&fake_run).is_err());
        assert!(db.run_resume(&fake_run).is_err());
        assert!(db.run_fail(&fake_run, "error").is_err());
        assert!(db.run_cancel(&fake_run).is_err());
        assert!(db.run_archive(&fake_run).is_err());
    });
}

/// Test fail requires error message
#[test]
fn test_fail_captures_error_message() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        let error_msg = "Connection timeout after 30s";
        db.run_fail(&info.run_id, error_msg).unwrap();

        let result = db.run_get(&info.run_id).unwrap().unwrap();
        assert_eq!(result.value.error, Some(error_msg.to_string()));
    });
}

/// Test pause and resume cycle
#[test]
fn test_pause_resume_cycle() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        // Multiple pause/resume cycles should work
        for _ in 0..3 {
            db.run_pause(&info.run_id).unwrap();
            let result = db.run_get(&info.run_id).unwrap().unwrap();
            assert!(matches!(result.value.state, RunState::Paused));

            db.run_resume(&info.run_id).unwrap();
            let result = db.run_get(&info.run_id).unwrap().unwrap();
            assert!(matches!(result.value.state, RunState::Active));
        }
    });
}
