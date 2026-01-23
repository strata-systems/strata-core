//! Batch KV Facade Operations Tests
//!
//! Tests for mget, mset, mdel, mexists operations.

use crate::*;

// =============================================================================
// MGET TESTS
// =============================================================================

#[test]
fn test_mget_all_existing() {
    let facade = create_facade();

    facade.set("k1", Value::Int(1)).unwrap();
    facade.set("k2", Value::Int(2)).unwrap();
    facade.set("k3", Value::Int(3)).unwrap();

    let values = facade.mget(&["k1", "k2", "k3"]).unwrap();

    assert_eq!(values.len(), 3);
    assert_eq!(values[0], Some(Value::Int(1)));
    assert_eq!(values[1], Some(Value::Int(2)));
    assert_eq!(values[2], Some(Value::Int(3)));
}

#[test]
fn test_mget_some_missing() {
    let facade = create_facade();

    facade.set("exists", Value::Int(1)).unwrap();

    let values = facade.mget(&["exists", "missing", "also_missing"]).unwrap();

    assert_eq!(values.len(), 3);
    assert_eq!(values[0], Some(Value::Int(1)));
    assert_eq!(values[1], None);
    assert_eq!(values[2], None);
}

#[test]
fn test_mget_empty_keys() {
    let facade = create_facade();

    let values = facade.mget(&[]).unwrap();
    assert!(values.is_empty());
}

#[test]
fn test_mget_preserves_order() {
    let facade = create_facade();

    facade.set("a", Value::String("A".into())).unwrap();
    facade.set("b", Value::String("B".into())).unwrap();
    facade.set("c", Value::String("C".into())).unwrap();

    // Request in different order
    let values = facade.mget(&["c", "a", "b"]).unwrap();

    assert_eq!(values[0], Some(Value::String("C".into())));
    assert_eq!(values[1], Some(Value::String("A".into())));
    assert_eq!(values[2], Some(Value::String("B".into())));
}

// =============================================================================
// MSET TESTS
// =============================================================================

#[test]
fn test_mset_creates_all_keys() {
    let facade = create_facade();

    facade.mset(&[
        ("k1", Value::Int(1)),
        ("k2", Value::Int(2)),
        ("k3", Value::Int(3)),
    ]).unwrap();

    assert_eq!(facade.get("k1").unwrap(), Some(Value::Int(1)));
    assert_eq!(facade.get("k2").unwrap(), Some(Value::Int(2)));
    assert_eq!(facade.get("k3").unwrap(), Some(Value::Int(3)));
}

#[test]
fn test_mset_overwrites_existing() {
    let facade = create_facade();

    facade.set("existing", Value::Int(0)).unwrap();

    facade.mset(&[
        ("existing", Value::Int(100)),
        ("new_key", Value::Int(200)),
    ]).unwrap();

    assert_eq!(facade.get("existing").unwrap(), Some(Value::Int(100)));
    assert_eq!(facade.get("new_key").unwrap(), Some(Value::Int(200)));
}

#[test]
fn test_mset_empty_entries() {
    let facade = create_facade();

    // Should succeed with no entries
    let result = facade.mset(&[]);
    assert!(result.is_ok());
}

#[test]
fn test_mset_atomic_on_validation_failure() {
    let facade = create_facade();

    // One invalid key should fail the entire operation
    let result = facade.mset(&[
        ("valid1", Value::Int(1)),
        ("", Value::Int(2)),  // Invalid: empty key
        ("valid2", Value::Int(3)),
    ]);

    assert!(result.is_err());

    // None of the keys should exist
    assert!(facade.get("valid1").unwrap().is_none());
    assert!(facade.get("valid2").unwrap().is_none());
}

// =============================================================================
// MDEL TESTS
// =============================================================================

#[test]
fn test_mdel_returns_count() {
    let facade = create_facade();

    facade.set("k1", Value::Int(1)).unwrap();
    facade.set("k2", Value::Int(2)).unwrap();

    let count = facade.mdel(&["k1", "k2", "nonexistent"]).unwrap();

    assert_eq!(count, 2, "Should return count of existing keys");
}

#[test]
fn test_mdel_removes_keys() {
    let facade = create_facade();

    facade.set("k1", Value::Int(1)).unwrap();
    facade.set("k2", Value::Int(2)).unwrap();
    facade.set("k3", Value::Int(3)).unwrap();

    facade.mdel(&["k1", "k3"]).unwrap();

    assert!(facade.get("k1").unwrap().is_none());
    assert!(facade.get("k2").unwrap().is_some()); // Not deleted
    assert!(facade.get("k3").unwrap().is_none());
}

#[test]
fn test_mdel_empty_keys() {
    let facade = create_facade();

    let count = facade.mdel(&[]).unwrap();
    assert_eq!(count, 0);
}

#[test]
fn test_mdel_all_nonexistent() {
    let facade = create_facade();

    let count = facade.mdel(&["a", "b", "c"]).unwrap();
    assert_eq!(count, 0);
}

// =============================================================================
// MEXISTS TESTS
// =============================================================================

#[test]
fn test_mexists_counts_existing() {
    let facade = create_facade();

    facade.set("k1", Value::Int(1)).unwrap();
    facade.set("k2", Value::Int(2)).unwrap();

    let count = facade.mexists(&["k1", "k2", "nonexistent"]).unwrap();
    assert_eq!(count, 2);
}

#[test]
fn test_mexists_empty_keys() {
    let facade = create_facade();

    let count = facade.mexists(&[]).unwrap();
    assert_eq!(count, 0);
}

#[test]
fn test_mexists_all_nonexistent() {
    let facade = create_facade();

    let count = facade.mexists(&["a", "b", "c"]).unwrap();
    assert_eq!(count, 0);
}

#[test]
fn test_mexists_all_exist() {
    let facade = create_facade();

    facade.set("k1", Value::Int(1)).unwrap();
    facade.set("k2", Value::Int(2)).unwrap();
    facade.set("k3", Value::Int(3)).unwrap();

    let count = facade.mexists(&["k1", "k2", "k3"]).unwrap();
    assert_eq!(count, 3);
}
