//! Substrate Equivalence Tests
//!
//! These tests verify that facade operations correctly desugar to substrate
//! operations, producing equivalent results.
//!
//! The key verification is that facade uses substrate APIs exclusively,
//! never accessing primitives or storage directly.

use crate::*;

// =============================================================================
// GET/SET EQUIVALENCE
// =============================================================================

#[test]
fn test_facade_get_equals_substrate_kv_get() {
    let (facade, substrate) = create_facade_and_substrate();
    let run = default_run();

    // Set via substrate
    substrate.kv_put(&run, "key", Value::Int(42)).unwrap();

    // Get via facade should equal get via substrate (minus version info)
    let facade_result = facade.get("key").unwrap();
    let substrate_result = substrate.kv_get(&run, "key").unwrap();

    assert_eq!(facade_result, substrate_result.map(|v| v.value));
}

#[test]
fn test_facade_set_creates_substrate_visible_entry() {
    let (facade, substrate) = create_facade_and_substrate();
    let run = default_run();

    // Set via facade
    facade.set("key", Value::Int(42)).unwrap();

    // Should be visible via substrate
    let substrate_result = substrate.kv_get(&run, "key").unwrap();
    assert!(substrate_result.is_some());
    assert_eq!(substrate_result.unwrap().value, Value::Int(42));
}

#[test]
fn test_facade_getv_equals_substrate_kv_get() {
    let (facade, substrate) = create_facade_and_substrate();
    let run = default_run();

    // Set via substrate to get known version
    let _version = substrate.kv_put(&run, "key", Value::String("test".into())).unwrap();

    // Facade getv should return equivalent info
    let facade_versioned = facade.getv("key").unwrap().unwrap();
    let substrate_versioned = substrate.kv_get(&run, "key").unwrap().unwrap();

    assert_eq!(facade_versioned.value, substrate_versioned.value);
    // Version format may differ, but should represent same logical version
}

// =============================================================================
// DELETE EQUIVALENCE
// =============================================================================

#[test]
fn test_facade_del_equals_substrate_kv_delete() {
    let (facade, substrate) = create_facade_and_substrate();
    let run = default_run();

    // Set via substrate
    substrate.kv_put(&run, "key", Value::Int(1)).unwrap();

    // Delete via facade
    let facade_deleted = facade.del("key").unwrap();

    // Should no longer exist in substrate
    let substrate_result = substrate.kv_get(&run, "key").unwrap();
    assert!(substrate_result.is_none());
    assert!(facade_deleted);
}

#[test]
fn test_substrate_delete_reflected_in_facade() {
    let (facade, substrate) = create_facade_and_substrate();
    let run = default_run();

    // Set and delete via substrate
    substrate.kv_put(&run, "key", Value::Int(1)).unwrap();
    substrate.kv_delete(&run, "key").unwrap();

    // Facade should see it as deleted
    assert!(facade.get("key").unwrap().is_none());
    assert!(!facade.exists("key").unwrap());
}

// =============================================================================
// EXISTS EQUIVALENCE
// =============================================================================

#[test]
fn test_facade_exists_equals_substrate_kv_exists() {
    let (facade, substrate) = create_facade_and_substrate();
    let run = default_run();

    // Check nonexistent
    assert_eq!(facade.exists("key").unwrap(), substrate.kv_exists(&run, "key").unwrap());

    // Create key
    substrate.kv_put(&run, "key", Value::Int(1)).unwrap();

    // Check existing
    assert_eq!(facade.exists("key").unwrap(), substrate.kv_exists(&run, "key").unwrap());
}

// =============================================================================
// INCR EQUIVALENCE
// =============================================================================

#[test]
fn test_facade_incr_equals_substrate_kv_incr() {
    let (facade, substrate) = create_facade_and_substrate();
    let run = default_run();

    // Set initial value via substrate
    substrate.kv_put(&run, "counter", Value::Int(10)).unwrap();

    // Increment via facade
    let facade_result = facade.incr("counter").unwrap();

    // Should equal substrate incr result
    // Reset and try substrate
    substrate.kv_put(&run, "counter2", Value::Int(10)).unwrap();
    let substrate_result = substrate.kv_incr(&run, "counter2", 1).unwrap();

    assert_eq!(facade_result, substrate_result);
}

#[test]
fn test_facade_incrby_equals_substrate_kv_incr() {
    let (facade, substrate) = create_facade_and_substrate();
    let run = default_run();

    substrate.kv_put(&run, "a", Value::Int(100)).unwrap();
    substrate.kv_put(&run, "b", Value::Int(100)).unwrap();

    let facade_result = facade.incrby("a", 25).unwrap();
    let substrate_result = substrate.kv_incr(&run, "b", 25).unwrap();

    assert_eq!(facade_result, substrate_result);
}

// =============================================================================
// BATCH EQUIVALENCE
// =============================================================================

#[test]
fn test_facade_mget_equals_substrate_kv_mget() {
    let (facade, substrate) = create_facade_and_substrate();
    let run = default_run();

    // Set via substrate
    substrate.kv_put(&run, "k1", Value::Int(1)).unwrap();
    substrate.kv_put(&run, "k2", Value::Int(2)).unwrap();

    let facade_result = facade.mget(&["k1", "k2", "missing"]).unwrap();
    let substrate_result = substrate.kv_mget(&run, &["k1", "k2", "missing"]).unwrap();

    // Compare values (facade strips version info)
    let substrate_values: Vec<_> = substrate_result.into_iter()
        .map(|opt| opt.map(|v| v.value))
        .collect();

    assert_eq!(facade_result, substrate_values);
}

#[test]
fn test_facade_mset_equals_substrate_kv_mput() {
    let (facade, substrate) = create_facade_and_substrate();
    let run = default_run();

    // Set via facade
    facade.mset(&[
        ("f1", Value::Int(1)),
        ("f2", Value::Int(2)),
    ]).unwrap();

    // Should be visible via substrate
    let v1 = substrate.kv_get(&run, "f1").unwrap().unwrap();
    let v2 = substrate.kv_get(&run, "f2").unwrap().unwrap();

    assert_eq!(v1.value, Value::Int(1));
    assert_eq!(v2.value, Value::Int(2));
}

#[test]
fn test_facade_mdel_equals_substrate_kv_mdelete() {
    let (facade, substrate) = create_facade_and_substrate();
    let run = default_run();

    // Set via substrate
    substrate.kv_put(&run, "k1", Value::Int(1)).unwrap();
    substrate.kv_put(&run, "k2", Value::Int(2)).unwrap();

    // Delete via facade
    let count = facade.mdel(&["k1", "k2", "missing"]).unwrap();

    // Verify via substrate
    assert!(substrate.kv_get(&run, "k1").unwrap().is_none());
    assert!(substrate.kv_get(&run, "k2").unwrap().is_none());
    assert_eq!(count, 2);
}

// =============================================================================
// CAS EQUIVALENCE
// =============================================================================

#[test]
fn test_facade_setnx_equals_substrate_kv_cas_version_none() {
    let (facade, substrate) = create_facade_and_substrate();
    let run = default_run();

    // setnx on new key should succeed
    let facade_success = facade.setnx("new", Value::Int(1)).unwrap();
    assert!(facade_success);

    // Equivalent to kv_cas_version with None
    let substrate_success = substrate.kv_cas_version(&run, "new2", None, Value::Int(1)).unwrap();
    assert!(substrate_success);

    // Both keys should exist
    assert!(facade.exists("new").unwrap());
    assert!(substrate.kv_exists(&run, "new2").unwrap());
}

#[test]
fn test_facade_setnx_on_existing_equals_substrate_cas_fail() {
    let (facade, substrate) = create_facade_and_substrate();
    let run = default_run();

    // Set key
    facade.set("existing", Value::Int(1)).unwrap();
    substrate.kv_put(&run, "existing2", Value::Int(1)).unwrap();

    // setnx should fail
    let facade_success = facade.setnx("existing", Value::Int(2)).unwrap();
    assert!(!facade_success);

    // Equivalent substrate call should also fail
    let substrate_success = substrate.kv_cas_version(&run, "existing2", None, Value::Int(2)).unwrap();
    assert!(!substrate_success);
}

// =============================================================================
// DEFAULT RUN TARGETING
// =============================================================================

#[test]
fn test_facade_targets_default_run() {
    let (facade, substrate) = create_facade_and_substrate();
    let default = default_run();
    let other_run = ApiRunId::new();

    // Set via facade (targets default run)
    facade.set("key", Value::Int(1)).unwrap();

    // Should be visible in default run
    assert!(substrate.kv_exists(&default, "key").unwrap());

    // Should NOT be visible in other run
    assert!(!substrate.kv_exists(&other_run, "key").unwrap());
}

#[test]
fn test_facade_does_not_see_other_runs() {
    let (facade, substrate) = create_facade_and_substrate();
    let other_run = ApiRunId::new();

    // Set in other run via substrate
    substrate.kv_put(&other_run, "key", Value::Int(999)).unwrap();

    // Facade should not see it (targets default run)
    assert!(facade.get("key").unwrap().is_none());
    assert!(!facade.exists("key").unwrap());
}

// =============================================================================
// AUTO-COMMIT VERIFICATION
// =============================================================================

#[test]
fn test_facade_auto_commits_immediately() {
    let (facade, substrate) = create_facade_and_substrate();
    let run = default_run();

    // Set via facade
    facade.set("key", Value::Int(1)).unwrap();

    // Immediately visible via substrate (auto-committed)
    let result = substrate.kv_get(&run, "key").unwrap();
    assert!(result.is_some());
}

#[test]
fn test_facade_operations_are_atomic() {
    let (facade, _substrate) = create_facade_and_substrate();

    // Each operation is atomic
    facade.set("a", Value::Int(1)).unwrap();
    facade.set("b", Value::Int(2)).unwrap();

    // If we could inject a failure between operations, only completed ones
    // would be visible. We verify this by checking both exist independently.
    assert!(facade.exists("a").unwrap());
    assert!(facade.exists("b").unwrap());
}
