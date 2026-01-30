//! RunIndex Primitive Tests
//!
//! Tests for run lifecycle management.

use crate::common::*;
use strata_engine::RunStatus;

// ============================================================================
// Basic CRUD
// ============================================================================

#[test]
fn create_run() {
    let test_db = TestDb::new();
    let run_idx = test_db.run_index();

    let result = run_idx.create_run("test_run").unwrap();
    assert_eq!(result.value.name, "test_run");
    // Initial status is Active
    assert_eq!(result.value.status, RunStatus::Active);
}

#[test]
fn create_run_duplicate_fails() {
    let test_db = TestDb::new();
    let run_idx = test_db.run_index();

    run_idx.create_run("test_run").unwrap();

    let result = run_idx.create_run("test_run");
    assert!(result.is_err());
}

#[test]
fn get_run() {
    let test_db = TestDb::new();
    let run_idx = test_db.run_index();

    run_idx.create_run("test_run").unwrap();

    let result = run_idx.get_run("test_run").unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().value.name, "test_run");
}

#[test]
fn get_nonexistent_returns_none() {
    let test_db = TestDb::new();
    let run_idx = test_db.run_index();

    let result = run_idx.get_run("nonexistent").unwrap();
    assert!(result.is_none());
}

#[test]
fn exists_returns_correct_status() {
    let test_db = TestDb::new();
    let run_idx = test_db.run_index();

    assert!(!run_idx.exists("test_run").unwrap());

    run_idx.create_run("test_run").unwrap();
    assert!(run_idx.exists("test_run").unwrap());
}

#[test]
fn list_runs() {
    let test_db = TestDb::new();
    let run_idx = test_db.run_index();

    run_idx.create_run("run_a").unwrap();
    run_idx.create_run("run_b").unwrap();
    run_idx.create_run("run_c").unwrap();

    let runs = run_idx.list_runs().unwrap();
    assert_eq!(runs.len(), 3);
    assert!(runs.contains(&"run_a".to_string()));
    assert!(runs.contains(&"run_b".to_string()));
    assert!(runs.contains(&"run_c".to_string()));
}

#[test]
fn count_runs() {
    let test_db = TestDb::new();
    let run_idx = test_db.run_index();

    // count rewritten as list_runs().len()
    assert_eq!(run_idx.list_runs().unwrap().len(), 0);

    run_idx.create_run("run_a").unwrap();
    run_idx.create_run("run_b").unwrap();

    assert_eq!(run_idx.list_runs().unwrap().len(), 2);
}

#[test]
fn delete_run() {
    let test_db = TestDb::new();
    let run_idx = test_db.run_index();

    run_idx.create_run("test_run").unwrap();
    assert!(run_idx.exists("test_run").unwrap());

    run_idx.delete_run("test_run").unwrap();
    assert!(!run_idx.exists("test_run").unwrap());
}

// ============================================================================
// Status Transitions
// ============================================================================

#[test]
#[ignore = "requires: RunIndex lifecycle methods"]
fn complete_run() {
    let _test_db = TestDb::new();
    // Status transitions are post-MVP
}

#[test]
#[ignore = "requires: RunIndex lifecycle methods"]
fn fail_run() {
    let _test_db = TestDb::new();
    // Status transitions are post-MVP
}

#[test]
#[ignore = "requires: RunIndex lifecycle methods"]
fn cancel_run() {
    let _test_db = TestDb::new();
    // Status transitions are post-MVP
}

#[test]
#[ignore = "requires: RunIndex lifecycle methods"]
fn pause_and_resume_run() {
    let _test_db = TestDb::new();
    // Status transitions are post-MVP
}

#[test]
#[ignore = "requires: RunIndex lifecycle methods"]
fn archive_completed_run() {
    let _test_db = TestDb::new();
    // Status transitions are post-MVP
}

#[test]
#[ignore = "requires: RunIndex lifecycle methods"]
fn terminal_states_cannot_transition_to_active() {
    let _test_db = TestDb::new();
    // Status transitions are post-MVP
}

// ============================================================================
// Tags
// ============================================================================

#[test]
#[ignore = "requires: RunIndex lifecycle methods"]
fn add_tags() {
    let _test_db = TestDb::new();
    // Tag management is post-MVP
}

#[test]
#[ignore = "requires: RunIndex lifecycle methods"]
fn remove_tags() {
    let _test_db = TestDb::new();
    // Tag management is post-MVP
}

#[test]
#[ignore = "requires: RunIndex lifecycle methods"]
fn query_by_tag() {
    let _test_db = TestDb::new();
    // Tag management is post-MVP
}

// ============================================================================
// Metadata
// ============================================================================

#[test]
#[ignore = "requires: RunIndex lifecycle methods"]
fn update_metadata() {
    let _test_db = TestDb::new();
    // Metadata update is post-MVP
}

// ============================================================================
// Query by Status
// ============================================================================

#[test]
#[ignore = "requires: RunIndex lifecycle methods"]
fn query_by_status() {
    let _test_db = TestDb::new();
    // Status query is post-MVP
}

// ============================================================================
// Run Status State Machine
// ============================================================================

#[test]
#[ignore = "requires: RunStatus state machine methods"]
fn status_is_terminal_check() {
    // RunStatus::is_terminal() does not exist in MVP
}

#[test]
#[ignore = "requires: RunStatus state machine methods"]
fn status_is_finished_check() {
    // RunStatus::is_finished() does not exist in MVP
}

#[test]
#[ignore = "requires: RunStatus state machine methods"]
fn status_can_transition_to() {
    // RunStatus::can_transition_to() does not exist in MVP
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn empty_run_name() {
    let test_db = TestDb::new();
    let run_idx = test_db.run_index();

    // Empty name should work
    run_idx.create_run("").unwrap();
    assert!(run_idx.exists("").unwrap());
}

#[test]
fn special_characters_in_name() {
    let test_db = TestDb::new();
    let run_idx = test_db.run_index();

    let name = "run/with:special@chars";
    run_idx.create_run(name).unwrap();
    assert!(run_idx.exists(name).unwrap());
}
