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

    assert_eq!(run_idx.count().unwrap(), 0);

    run_idx.create_run("run_a").unwrap();
    run_idx.create_run("run_b").unwrap();

    assert_eq!(run_idx.count().unwrap(), 2);
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
fn complete_run() {
    let test_db = TestDb::new();
    let run_idx = test_db.run_index();

    run_idx.create_run("run").unwrap();
    // Run starts as Active, can complete directly
    let result = run_idx.complete_run("run").unwrap();
    assert_eq!(result.value.status, RunStatus::Completed);
}

#[test]
fn fail_run() {
    let test_db = TestDb::new();
    let run_idx = test_db.run_index();

    run_idx.create_run("run").unwrap();

    let result = run_idx.fail_run("run", "Something went wrong").unwrap();
    assert_eq!(result.value.status, RunStatus::Failed);
}

#[test]
fn cancel_run() {
    let test_db = TestDb::new();
    let run_idx = test_db.run_index();

    run_idx.create_run("run").unwrap();

    let result = run_idx.cancel_run("run").unwrap();
    assert_eq!(result.value.status, RunStatus::Cancelled);
}

#[test]
fn pause_and_resume_run() {
    let test_db = TestDb::new();
    let run_idx = test_db.run_index();

    run_idx.create_run("run").unwrap();

    let paused = run_idx.pause_run("run").unwrap();
    assert_eq!(paused.value.status, RunStatus::Paused);

    let resumed = run_idx.resume_run("run").unwrap();
    assert_eq!(resumed.value.status, RunStatus::Active);
}

#[test]
fn archive_completed_run() {
    let test_db = TestDb::new();
    let run_idx = test_db.run_index();

    run_idx.create_run("run").unwrap();
    run_idx.complete_run("run").unwrap();

    let archived = run_idx.archive_run("run").unwrap();
    assert_eq!(archived.value.status, RunStatus::Archived);
}

#[test]
fn terminal_states_cannot_transition_to_active() {
    let test_db = TestDb::new();
    let run_idx = test_db.run_index();

    run_idx.create_run("run").unwrap();
    run_idx.complete_run("run").unwrap();

    // Cannot go from Completed back to Active
    let result = run_idx.update_status("run", RunStatus::Active);
    assert!(result.is_err());
}

// ============================================================================
// Tags
// ============================================================================

#[test]
fn add_tags() {
    let test_db = TestDb::new();
    let run_idx = test_db.run_index();

    run_idx.create_run("run").unwrap();

    let result = run_idx.add_tags("run", vec!["tag1".to_string(), "tag2".to_string()]).unwrap();
    assert!(result.value.tags.contains(&"tag1".to_string()));
    assert!(result.value.tags.contains(&"tag2".to_string()));
}

#[test]
fn remove_tags() {
    let test_db = TestDb::new();
    let run_idx = test_db.run_index();

    run_idx.create_run("run").unwrap();
    run_idx.add_tags("run", vec!["tag1".to_string(), "tag2".to_string(), "tag3".to_string()]).unwrap();

    let result = run_idx.remove_tags("run", vec!["tag2".to_string()]).unwrap();
    assert!(result.value.tags.contains(&"tag1".to_string()));
    assert!(!result.value.tags.contains(&"tag2".to_string()));
    assert!(result.value.tags.contains(&"tag3".to_string()));
}

#[test]
fn query_by_tag() {
    let test_db = TestDb::new();
    let run_idx = test_db.run_index();

    run_idx.create_run("run_a").unwrap();
    run_idx.create_run("run_b").unwrap();
    run_idx.create_run("run_c").unwrap();

    run_idx.add_tags("run_a", vec!["important".to_string()]).unwrap();
    run_idx.add_tags("run_c", vec!["important".to_string()]).unwrap();

    let important_runs = run_idx.query_by_tag("important").unwrap();
    assert_eq!(important_runs.len(), 2);

    let names: Vec<_> = important_runs.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"run_a"));
    assert!(names.contains(&"run_c"));
}

// ============================================================================
// Metadata
// ============================================================================

#[test]
fn update_metadata() {
    let test_db = TestDb::new();
    let run_idx = test_db.run_index();

    run_idx.create_run("run").unwrap();

    let metadata = Value::String("custom data".into());
    let result = run_idx.update_metadata("run", metadata.clone()).unwrap();

    assert_eq!(result.value.metadata, metadata);
}

// ============================================================================
// Query by Status
// ============================================================================

#[test]
fn query_by_status() {
    let test_db = TestDb::new();
    let run_idx = test_db.run_index();

    run_idx.create_run("run_a").unwrap();
    run_idx.create_run("run_b").unwrap();
    run_idx.create_run("run_c").unwrap();

    // Complete one run
    run_idx.complete_run("run_a").unwrap();

    let active = run_idx.query_by_status(RunStatus::Active).unwrap();
    assert_eq!(active.len(), 2);

    let completed = run_idx.query_by_status(RunStatus::Completed).unwrap();
    assert_eq!(completed.len(), 1);
}

// ============================================================================
// Run Status State Machine
// ============================================================================

#[test]
fn status_is_terminal_check() {
    // Only Archived is truly terminal (cannot transition further)
    assert!(!RunStatus::Active.is_terminal());
    assert!(!RunStatus::Paused.is_terminal());
    assert!(!RunStatus::Completed.is_terminal()); // Can still archive
    assert!(!RunStatus::Failed.is_terminal());    // Can still archive
    assert!(!RunStatus::Cancelled.is_terminal()); // Can still archive
    assert!(RunStatus::Archived.is_terminal());   // Truly terminal
}

#[test]
fn status_is_finished_check() {
    // Finished means the run won't produce more data
    assert!(!RunStatus::Active.is_finished());
    assert!(!RunStatus::Paused.is_finished());
    assert!(RunStatus::Completed.is_finished());
    assert!(RunStatus::Failed.is_finished());
    assert!(RunStatus::Cancelled.is_finished());
}

#[test]
fn status_can_transition_to() {
    // Active can transition to any non-Active state
    assert!(RunStatus::Active.can_transition_to(RunStatus::Completed));
    assert!(RunStatus::Active.can_transition_to(RunStatus::Failed));
    assert!(RunStatus::Active.can_transition_to(RunStatus::Paused));
    assert!(RunStatus::Active.can_transition_to(RunStatus::Cancelled));

    // Paused can resume to Active
    assert!(RunStatus::Paused.can_transition_to(RunStatus::Active));

    // Terminal states cannot transition to Active
    assert!(!RunStatus::Completed.can_transition_to(RunStatus::Active));
    assert!(!RunStatus::Failed.can_transition_to(RunStatus::Active));
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
