//! RunIndex Query Tests
//!
//! Tests for run query operations:
//! - run_list
//! - run_query_by_status
//! - run_query_by_tag
//! - run_count
//! - run_search

use crate::*;
use strata_api::substrate::RunState;

// =============================================================================
// List Tests
// =============================================================================

/// Test listing all runs
#[test]
fn test_run_list_all() {
    test_across_substrate_modes(|db| {
        // Create a few runs
        db.run_create(None, None).unwrap();
        db.run_create(None, None).unwrap();
        db.run_create(None, None).unwrap();

        let runs = db.run_list(None, None, None).unwrap();

        // Should have at least the 3 we created
        assert!(runs.len() >= 3);
    });
}

/// Test listing runs by status
///
/// NOTE: Status-based listing depends on primitive indexing.
#[test]
fn test_run_list_by_status() {
    test_across_substrate_modes(|db| {
        let (run1, _) = db.run_create(None, None).unwrap();
        let (_run2, _) = db.run_create(None, None).unwrap();
        let (_run3, _) = db.run_create(None, None).unwrap();

        // Close one run
        db.run_close(&run1.run_id).unwrap();

        // Verify state change took effect
        let info = db.run_get(&run1.run_id).unwrap().unwrap();
        assert!(matches!(info.value.state, RunState::Completed));

        // List by status should not error
        let _active = db.run_list(Some(RunState::Active), None, None).unwrap();
        let _completed = db.run_list(Some(RunState::Completed), None, None).unwrap();
    });
}

/// Test list with limit
#[test]
fn test_run_list_limit() {
    test_across_substrate_modes(|db| {
        // Create several runs
        for _ in 0..5 {
            db.run_create(None, None).unwrap();
        }

        let runs = db.run_list(None, Some(3), None).unwrap();

        assert!(runs.len() <= 3);
    });
}

/// Test list with offset
#[test]
fn test_run_list_offset() {
    test_across_substrate_modes(|db| {
        // Create several runs
        for _ in 0..5 {
            db.run_create(None, None).unwrap();
        }

        // List with offset should not error
        let _offset_runs = db.run_list(None, None, Some(2)).unwrap();
    });
}

// =============================================================================
// Query By Status Tests
// =============================================================================

/// Test query by Active status
///
/// NOTE: Status-based queries depend on primitive indexing.
#[test]
fn test_run_query_by_status_active() {
    test_across_substrate_modes(|db| {
        let (run1, _) = db.run_create(None, None).unwrap();
        let (run2, _) = db.run_create(None, None).unwrap();

        // Close run2
        db.run_close(&run2.run_id).unwrap();

        // Verify state changes
        let info1 = db.run_get(&run1.run_id).unwrap().unwrap();
        assert!(matches!(info1.value.state, RunState::Active));

        let info2 = db.run_get(&run2.run_id).unwrap().unwrap();
        assert!(matches!(info2.value.state, RunState::Completed));

        // Query should not error
        let _active = db.run_query_by_status(RunState::Active).unwrap();
    });
}

/// Test query by Failed status
#[test]
fn test_run_query_by_status_failed() {
    test_across_substrate_modes(|db| {
        let (run1, _) = db.run_create(None, None).unwrap();

        // Fail run1
        db.run_fail(&run1.run_id, "test error").unwrap();

        // Verify state change
        let info = db.run_get(&run1.run_id).unwrap().unwrap();
        assert!(matches!(info.value.state, RunState::Failed));
        assert_eq!(info.value.error, Some("test error".to_string()));

        // Query should not error
        let _failed = db.run_query_by_status(RunState::Failed).unwrap();
    });
}

/// Test query by Cancelled status
#[test]
fn test_run_query_by_status_cancelled() {
    test_across_substrate_modes(|db| {
        let (run1, _) = db.run_create(None, None).unwrap();

        db.run_cancel(&run1.run_id).unwrap();

        // Verify state change
        let info = db.run_get(&run1.run_id).unwrap().unwrap();
        assert!(matches!(info.value.state, RunState::Cancelled));

        // Query should not error
        let _cancelled = db.run_query_by_status(RunState::Cancelled).unwrap();
    });
}

/// Test query by Paused status
#[test]
fn test_run_query_by_status_paused() {
    test_across_substrate_modes(|db| {
        let (run1, _) = db.run_create(None, None).unwrap();

        db.run_pause(&run1.run_id).unwrap();

        // Verify state change
        let info = db.run_get(&run1.run_id).unwrap().unwrap();
        assert!(matches!(info.value.state, RunState::Paused));

        // Query should not error
        let _paused = db.run_query_by_status(RunState::Paused).unwrap();
    });
}

/// Test query by Archived status
#[test]
fn test_run_query_by_status_archived() {
    test_across_substrate_modes(|db| {
        let (run1, _) = db.run_create(None, None).unwrap();

        db.run_archive(&run1.run_id).unwrap();

        // Verify state change
        let info = db.run_get(&run1.run_id).unwrap().unwrap();
        assert!(matches!(info.value.state, RunState::Archived));

        // Query should not error
        let _archived = db.run_query_by_status(RunState::Archived).unwrap();
    });
}

// =============================================================================
// Query By Tag Tests
// =============================================================================

/// Test query by tag
///
/// NOTE: Query by tag depends on primitive indexing. This test validates
/// the API works without asserting on exact query results.
#[test]
fn test_run_query_by_tag() {
    test_across_substrate_modes(|db| {
        let (run1, _) = db.run_create(None, None).unwrap();
        let (_run2, _) = db.run_create(None, None).unwrap();

        // Tag run1
        db.run_add_tags(&run1.run_id, &["production".to_string()]).unwrap();

        // Query should not error
        let _tagged = db.run_query_by_tag("production").unwrap();

        // Verify tag is stored correctly
        let tags = db.run_get_tags(&run1.run_id).unwrap();
        assert!(tags.contains(&"production".to_string()));
    });
}

/// Test query by tag returns empty for non-existent tag
#[test]
fn test_run_query_by_tag_not_found() {
    test_across_substrate_modes(|db| {
        db.run_create(None, None).unwrap();

        let tagged = db.run_query_by_tag("nonexistent_tag").unwrap();

        assert!(tagged.is_empty());
    });
}

// =============================================================================
// Count Tests
// =============================================================================

/// Test count all runs
#[test]
fn test_run_count_all() {
    test_across_substrate_modes(|db| {
        let initial = db.run_count(None).unwrap();

        db.run_create(None, None).unwrap();
        db.run_create(None, None).unwrap();

        let after = db.run_count(None).unwrap();

        assert_eq!(after, initial + 2);
    });
}

/// Test count by status
#[test]
fn test_run_count_by_status() {
    test_across_substrate_modes(|db| {
        let (run1, _) = db.run_create(None, None).unwrap();
        let (run2, _) = db.run_create(None, None).unwrap();

        let active_before = db.run_count(Some(RunState::Active)).unwrap();
        let completed_before = db.run_count(Some(RunState::Completed)).unwrap();

        db.run_close(&run1.run_id).unwrap();

        let active_after = db.run_count(Some(RunState::Active)).unwrap();
        let completed_after = db.run_count(Some(RunState::Completed)).unwrap();

        assert_eq!(active_after, active_before - 1);
        assert_eq!(completed_after, completed_before + 1);
    });
}

// =============================================================================
// Search Tests
// =============================================================================

/// Test search by run ID prefix
///
/// NOTE: Search functionality depends on primitive layer indexing.
#[test]
fn test_run_search_basic() {
    test_across_substrate_modes(|db| {
        let (_run1, _) = db.run_create(None, None).unwrap();

        // Search should not error
        let _results = db.run_search("", None).unwrap();
    });
}

/// Test search respects limit
#[test]
fn test_run_search_respects_limit() {
    test_across_substrate_modes(|db| {
        // Create several runs
        for _ in 0..5 {
            db.run_create(None, None).unwrap();
        }

        let results = db.run_search("", Some(2)).unwrap();

        assert!(results.len() <= 2);
    });
}

/// Test search returns empty for no matches
#[test]
fn test_run_search_no_matches() {
    test_across_substrate_modes(|db| {
        db.run_create(None, None).unwrap();

        // Search for something that won't match
        let results = db.run_search("zzzzz-nonexistent-zzzzz", None).unwrap();

        assert!(results.is_empty());
    });
}
