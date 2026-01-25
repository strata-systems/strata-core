//! RunIndex Retention Tests
//!
//! Tests for retention policy management:
//! - run_set_retention
//! - run_get_retention
//!
//! NOTE: Setting retention does NOT immediately delete data.
//! Enforcement occurs during compaction/garbage collection.

use crate::*;
use strata_api::substrate::RetentionPolicy;
use std::time::Duration;

// =============================================================================
// Set Retention Tests
// =============================================================================

/// Test setting KeepAll retention policy
#[test]
fn test_set_retention_keep_all() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        db.run_set_retention(&info.run_id, RetentionPolicy::KeepAll).unwrap();

        let policy = db.run_get_retention(&info.run_id).unwrap();
        assert!(matches!(policy, RetentionPolicy::KeepAll));
    });
}

/// Test setting KeepLast retention policy
#[test]
fn test_set_retention_keep_last() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        db.run_set_retention(&info.run_id, RetentionPolicy::KeepLast(100)).unwrap();

        let policy = db.run_get_retention(&info.run_id).unwrap();
        if let RetentionPolicy::KeepLast(n) = policy {
            assert_eq!(n, 100);
        } else {
            panic!("Expected KeepLast policy");
        }
    });
}

/// Test setting KeepFor retention policy
#[test]
fn test_set_retention_keep_duration() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        let duration = Duration::from_secs(7 * 24 * 60 * 60); // 7 days
        db.run_set_retention(&info.run_id, RetentionPolicy::KeepFor(duration)).unwrap();

        let policy = db.run_get_retention(&info.run_id).unwrap();
        if let RetentionPolicy::KeepFor(d) = policy {
            assert_eq!(d, duration);
        } else {
            panic!("Expected KeepFor policy");
        }
    });
}

/// Test setting composite retention policy
#[test]
fn test_set_retention_composite() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        let policy = RetentionPolicy::Composite(vec![
            RetentionPolicy::KeepLast(50),
            RetentionPolicy::KeepFor(Duration::from_secs(3600)),
        ]);
        db.run_set_retention(&info.run_id, policy.clone()).unwrap();

        let retrieved = db.run_get_retention(&info.run_id).unwrap();
        assert_eq!(retrieved, policy);
    });
}

/// Test setting retention on non-existent run fails
#[test]
fn test_set_retention_not_found() {
    test_across_substrate_modes(|db| {
        let fake_run = ApiRunId::new();

        let result = db.run_set_retention(&fake_run, RetentionPolicy::KeepAll);
        assert!(result.is_err());
    });
}

// =============================================================================
// Get Retention Tests
// =============================================================================

/// Test getting default retention policy
#[test]
fn test_get_retention_default() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        // Should return KeepAll by default (no policy set)
        let policy = db.run_get_retention(&info.run_id).unwrap();
        assert!(matches!(policy, RetentionPolicy::KeepAll));
    });
}

/// Test getting retention after setting it
#[test]
fn test_get_retention_after_set() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        // Set policy
        db.run_set_retention(&info.run_id, RetentionPolicy::KeepLast(10)).unwrap();

        // Get should return what was set
        let policy = db.run_get_retention(&info.run_id).unwrap();
        if let RetentionPolicy::KeepLast(n) = policy {
            assert_eq!(n, 10);
        } else {
            panic!("Expected KeepLast policy");
        }
    });
}

/// Test getting retention from non-existent run fails
#[test]
fn test_get_retention_not_found() {
    test_across_substrate_modes(|db| {
        let fake_run = ApiRunId::new();

        let result = db.run_get_retention(&fake_run);
        assert!(result.is_err());
    });
}

// =============================================================================
// Retention Persistence Tests
// =============================================================================

/// Test retention is stored in metadata
#[test]
fn test_retention_persists_metadata() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        db.run_set_retention(&info.run_id, RetentionPolicy::KeepLast(25)).unwrap();

        // Check that metadata contains _strata_retention as a JSON string
        let run_info = db.run_get(&info.run_id).unwrap().unwrap();
        if let Value::Object(m) = &run_info.value.metadata {
            match m.get("_strata_retention") {
                Some(Value::String(s)) => {
                    assert!(s.contains("keep_last"));
                }
                _ => panic!("Expected string _strata_retention key"),
            }
        } else {
            panic!("Expected object metadata");
        }
    });
}

/// Test updating retention replaces previous policy
#[test]
fn test_update_retention_replaces() {
    test_across_substrate_modes(|db| {
        let (info, _) = db.run_create(None, None).unwrap();

        // Set initial policy
        db.run_set_retention(&info.run_id, RetentionPolicy::KeepLast(10)).unwrap();

        // Update to different policy
        db.run_set_retention(&info.run_id, RetentionPolicy::KeepLast(50)).unwrap();

        // Should have new policy
        let policy = db.run_get_retention(&info.run_id).unwrap();
        if let RetentionPolicy::KeepLast(n) = policy {
            assert_eq!(n, 50);
        } else {
            panic!("Expected KeepLast policy");
        }
    });
}

/// Test retention coexists with user metadata
#[test]
fn test_retention_coexists_with_user_metadata() {
    test_across_substrate_modes(|db| {
        let meta = obj([("user_key", Value::String("user_value".to_string()))]);
        let (info, _) = db.run_create(None, Some(meta)).unwrap();

        // Set retention
        db.run_set_retention(&info.run_id, RetentionPolicy::KeepLast(5)).unwrap();

        // Both should be present
        let run_info = db.run_get(&info.run_id).unwrap().unwrap();
        if let Value::Object(m) = &run_info.value.metadata {
            assert!(matches!(m.get("_strata_retention"), Some(Value::String(_))));
            assert_eq!(m.get("user_key"), Some(&Value::String("user_value".to_string())));
        } else {
            panic!("Expected object metadata");
        }
    });
}
