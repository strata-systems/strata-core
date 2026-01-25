//! RunIndex Delete Tests
//!
//! Tests for run deletion:
//! - run_delete
//! - Cascading delete to run-scoped entities
//!
//! ## Cascading Delete Scope
//!
//! Cascading delete affects ONLY entities scoped to the run:
//! - KV entries in the run's namespace
//! - Events in the run's namespace
//! - State cells in the run's namespace
//! - JSON documents in the run's namespace
//! - Vector entries in the run's namespace
//!
//! Cascading delete does NOT:
//! - Delete other runs (including children)
//! - Delete entities in other runs
//! - Affect global indices (except removing this run from them)
//! - Propagate to parent runs

use crate::*;
use strata_api::substrate::{EventLog, JsonStore, KVStore, StateCell};

// =============================================================================
// Basic Delete Tests
// =============================================================================

/// Test deleting a run
#[test]
fn test_delete_run() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        assert!(db.run_exists(&info.run_id).unwrap());

        db.run_delete(&info.run_id).unwrap();

        assert!(!db.run_exists(&info.run_id).unwrap());
    });
}

/// Test deleting non-existent run fails
#[test]
fn test_delete_run_not_found() {
    test_across_substrate_modes(|db| {
        let fake_run = ApiRunId::new();

        let result = db.run_delete(&fake_run);
        assert!(result.is_err());
    });
}

/// Test deleting default run fails
///
/// NOTE: The default run is a namespace concept, not necessarily an explicit
/// entity in RunIndex. Delete should fail (either "not found" or "constraint violation").
#[test]
fn test_delete_default_run_error() {
    test_across_substrate_modes(|db| {
        let default_run = ApiRunId::default_run_id();

        // Should error (either not found or constraint violation)
        let result = db.run_delete(&default_run);
        assert!(result.is_err());

        // Note: run_exists may return false if default isn't an explicit entity
        // This is acceptable - the key invariant is that delete fails
    });
}

// =============================================================================
// Cascading Delete Tests
// =============================================================================

/// Test delete cascades to KV entries
#[test]
fn test_delete_cascades_kv() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        // Create KV entries in the run
        db.kv_put(&info.run_id, "key1", Value::Int(1)).unwrap();
        db.kv_put(&info.run_id, "key2", Value::Int(2)).unwrap();

        // Verify entries exist
        assert!(db.kv_get(&info.run_id, "key1").unwrap().is_some());
        assert!(db.kv_get(&info.run_id, "key2").unwrap().is_some());

        // Delete run
        db.run_delete(&info.run_id).unwrap();

        // KV entries should be gone (run doesn't exist, so get should fail)
        // Note: exact behavior depends on implementation - may error or return None
        let result = db.kv_get(&info.run_id, "key1");
        assert!(result.is_err() || result.unwrap().is_none());
    });
}

/// Test delete cascades to events
#[test]
fn test_delete_cascades_events() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        // Create events in the run (events require Object payloads)
        let event1 = obj([("msg", Value::String("event1".to_string()))]);
        let event2 = obj([("msg", Value::String("event2".to_string()))]);
        db.event_append(&info.run_id, "stream1", event1).unwrap();
        db.event_append(&info.run_id, "stream1", event2).unwrap();

        // Verify events exist
        let events = db.event_range(&info.run_id, "stream1", None, None, None).unwrap();
        assert_eq!(events.len(), 2);

        // Delete run
        db.run_delete(&info.run_id).unwrap();

        // Events should be gone
        let result = db.event_range(&info.run_id, "stream1", None, None, None);
        assert!(result.is_err() || result.unwrap().is_empty());
    });
}

/// Test delete cascades to state cells
#[test]
fn test_delete_cascades_state() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        // Create state cells in the run
        db.state_set(&info.run_id, "cell1", Value::Int(100)).unwrap();
        db.state_set(&info.run_id, "cell2", Value::Int(200)).unwrap();

        // Verify cells exist
        assert!(db.state_get(&info.run_id, "cell1").unwrap().is_some());

        // Delete run
        db.run_delete(&info.run_id).unwrap();

        // State cells should be gone
        let result = db.state_get(&info.run_id, "cell1");
        assert!(result.is_err() || result.unwrap().is_none());
    });
}

/// Test delete cascades to JSON documents
#[test]
fn test_delete_cascades_json() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        // Create JSON documents in the run
        let doc = obj([("key", Value::String("value".to_string()))]);
        db.json_set(&info.run_id, "doc1", "$", doc).unwrap();

        // Verify document exists
        assert!(db.json_get(&info.run_id, "doc1", "$").unwrap().is_some());

        // Delete run
        db.run_delete(&info.run_id).unwrap();

        // JSON documents should be gone
        let result = db.json_get(&info.run_id, "doc1", "$");
        assert!(result.is_err() || result.unwrap().is_none());
    });
}

/// Test delete removes run from indices
#[test]
fn test_delete_removes_indices() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        // Add tags (creates index entries)
        db.run_add_tags(&info.run_id, &["indexed_tag".to_string()]).unwrap();

        // Verify in index
        let tagged = db.run_query_by_tag("indexed_tag").unwrap();
        assert!(tagged.iter().any(|r| r.value.run_id == info.run_id));

        // Delete run
        db.run_delete(&info.run_id).unwrap();

        // Should no longer be in tag index
        let tagged = db.run_query_by_tag("indexed_tag").unwrap();
        assert!(tagged.iter().all(|r| r.value.run_id != info.run_id));
    });
}

// =============================================================================
// Delete Scope Boundary Tests
// =============================================================================

/// Test delete does not affect children (they become orphaned)
#[test]
fn test_delete_does_not_affect_children() {
    test_across_substrate_modes(|db| {
        let (parent, _) = db.run_create(None, None).unwrap();
        let (child1, _) = db.run_create_child(&parent.run_id, None).unwrap();
        let (child2, _) = db.run_create_child(&parent.run_id, None).unwrap();

        // Delete parent
        db.run_delete(&parent.run_id).unwrap();

        // Children should still exist
        assert!(db.run_exists(&child1.run_id).unwrap());
        assert!(db.run_exists(&child2.run_id).unwrap());
    });
}

/// Test delete does not affect parent
#[test]
fn test_delete_does_not_affect_parent() {
    test_across_substrate_modes(|db| {
        let (parent, _) = db.run_create(None, None).unwrap();
        let (child, _) = db.run_create_child(&parent.run_id, None).unwrap();

        // Delete child
        db.run_delete(&child.run_id).unwrap();

        // Parent should still exist
        assert!(db.run_exists(&parent.run_id).unwrap());

        // Parent's children should no longer include deleted child
        let children = db.run_get_children(&parent.run_id).unwrap();
        assert!(children.iter().all(|c| c.value.run_id != child.run_id));
    });
}

/// Test delete does not affect other runs' data
#[test]
fn test_delete_does_not_affect_other_runs() {
    test_across_substrate_modes(|db| {
        let (run1, _) = db.run_create(None, None).unwrap();
        let (run2, _) = db.run_create(None, None).unwrap();

        // Create data in both runs
        db.kv_put(&run1.run_id, "key", Value::Int(1)).unwrap();
        db.kv_put(&run2.run_id, "key", Value::Int(2)).unwrap();

        // Delete run1
        db.run_delete(&run1.run_id).unwrap();

        // run2's data should be intact
        let value = db.kv_get(&run2.run_id, "key").unwrap().unwrap();
        assert_eq!(value.value, Value::Int(2));
    });
}
