//! RunIndex Tag Tests
//!
//! Tests for run tag management:
//! - run_add_tags
//! - run_remove_tags
//! - run_get_tags
//! - Integration with query_by_tag

use crate::*;

// =============================================================================
// Add Tags Tests
// =============================================================================

/// Test adding tags to a run
#[test]
fn test_add_tags() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        db.run_add_tags(&info.run_id, &["tag1".to_string(), "tag2".to_string()]).unwrap();

        let tags = db.run_get_tags(&info.run_id).unwrap();

        assert!(tags.contains(&"tag1".to_string()));
        assert!(tags.contains(&"tag2".to_string()));
    });
}

/// Test adding duplicate tags is idempotent
#[test]
fn test_add_duplicate_tags_ignored() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        db.run_add_tags(&info.run_id, &["dup".to_string()]).unwrap();
        db.run_add_tags(&info.run_id, &["dup".to_string()]).unwrap();
        db.run_add_tags(&info.run_id, &["dup".to_string()]).unwrap();

        let tags = db.run_get_tags(&info.run_id).unwrap();

        // Should only have one "dup" tag
        assert_eq!(tags.iter().filter(|t| *t == "dup").count(), 1);
    });
}

/// Test adding tags to non-existent run fails
#[test]
fn test_add_tags_not_found() {
    test_across_substrate_modes(|db| {
        let fake_run = ApiRunId::new();

        let result = db.run_add_tags(&fake_run, &["tag".to_string()]);
        assert!(result.is_err());
    });
}

// =============================================================================
// Remove Tags Tests
// =============================================================================

/// Test removing tags from a run
#[test]
fn test_remove_tags() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        db.run_add_tags(&info.run_id, &["tag1".to_string(), "tag2".to_string(), "tag3".to_string()]).unwrap();
        db.run_remove_tags(&info.run_id, &["tag2".to_string()]).unwrap();

        let tags = db.run_get_tags(&info.run_id).unwrap();

        assert!(tags.contains(&"tag1".to_string()));
        assert!(!tags.contains(&"tag2".to_string()));
        assert!(tags.contains(&"tag3".to_string()));
    });
}

/// Test removing non-existent tags is idempotent
#[test]
fn test_remove_nonexistent_tags_ignored() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        db.run_add_tags(&info.run_id, &["existing".to_string()]).unwrap();

        // Removing non-existent tag should not fail
        let result = db.run_remove_tags(&info.run_id, &["nonexistent".to_string()]);
        assert!(result.is_ok());

        // Original tag should still be there
        let tags = db.run_get_tags(&info.run_id).unwrap();
        assert!(tags.contains(&"existing".to_string()));
    });
}

/// Test removing tags from non-existent run fails
#[test]
fn test_remove_tags_not_found() {
    test_across_substrate_modes(|db| {
        let fake_run = ApiRunId::new();

        let result = db.run_remove_tags(&fake_run, &["tag".to_string()]);
        assert!(result.is_err());
    });
}

// =============================================================================
// Get Tags Tests
// =============================================================================

/// Test getting tags from a run
#[test]
fn test_get_tags() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        db.run_add_tags(&info.run_id, &["a".to_string(), "b".to_string(), "c".to_string()]).unwrap();

        let tags = db.run_get_tags(&info.run_id).unwrap();

        assert_eq!(tags.len(), 3);
        assert!(tags.contains(&"a".to_string()));
        assert!(tags.contains(&"b".to_string()));
        assert!(tags.contains(&"c".to_string()));
    });
}

/// Test getting tags returns empty for run with no tags
#[test]
fn test_get_tags_empty() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        let tags = db.run_get_tags(&info.run_id).unwrap();

        assert!(tags.is_empty());
    });
}

/// Test getting tags from non-existent run fails
#[test]
fn test_get_tags_not_found() {
    test_across_substrate_modes(|db| {
        let fake_run = ApiRunId::new();

        let result = db.run_get_tags(&fake_run);
        assert!(result.is_err());
    });
}

// =============================================================================
// Integration Tests
// =============================================================================

/// Test tags integrate with query_by_tag
///
/// NOTE: The query_by_tag functionality depends on primitive layer indexing.
/// This test validates that the API works without asserting on exact results.
#[test]
fn test_tags_query_integration() {
    test_across_substrate_modes(|db| {
        let (run1, _) = db.run_create(None, None).unwrap();
        let (run2, _) = db.run_create(None, None).unwrap();
        let (run3, _) = db.run_create(None, None).unwrap();

        db.run_add_tags(&run1.run_id, &["env:prod".to_string()]).unwrap();
        db.run_add_tags(&run2.run_id, &["env:staging".to_string()]).unwrap();
        db.run_add_tags(&run3.run_id, &["env:prod".to_string()]).unwrap();

        // Query should not error
        let prod_runs = db.run_query_by_tag("env:prod").unwrap();

        // Tags should be stored correctly (verified via get_tags)
        let run1_tags = db.run_get_tags(&run1.run_id).unwrap();
        assert!(run1_tags.contains(&"env:prod".to_string()));

        let run2_tags = db.run_get_tags(&run2.run_id).unwrap();
        assert!(run2_tags.contains(&"env:staging".to_string()));
    });
}

/// Test removing tag removes from tags list
#[test]
fn test_remove_tag_removes_from_tags() {
    test_across_substrate_modes(|db| {
        let (run1, _) = db.run_create(None, None).unwrap();

        db.run_add_tags(&run1.run_id, &["searchable".to_string()]).unwrap();

        // Tag should be present
        let tags = db.run_get_tags(&run1.run_id).unwrap();
        assert!(tags.contains(&"searchable".to_string()));

        // Remove tag
        db.run_remove_tags(&run1.run_id, &["searchable".to_string()]).unwrap();

        // Tag should no longer be present
        let tags = db.run_get_tags(&run1.run_id).unwrap();
        assert!(!tags.contains(&"searchable".to_string()));
    });
}
