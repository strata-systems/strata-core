//! Basic KV Facade Operations Tests
//!
//! Tests for get, getv, set, del, exists operations.

use crate::*;

// =============================================================================
// GET / SET TESTS
// =============================================================================

#[test]
fn test_set_and_get_roundtrip() {
    let facade = create_facade();

    facade.set("key1", Value::Int(42)).unwrap();
    let value = facade.get("key1").unwrap();

    assert_eq!(value, Some(Value::Int(42)));
}

#[test]
fn test_get_nonexistent_returns_none() {
    let facade = create_facade();

    let value = facade.get("nonexistent").unwrap();
    assert!(value.is_none());
}

#[test]
fn test_set_overwrites_existing() {
    let facade = create_facade();

    facade.set("key", Value::Int(1)).unwrap();
    facade.set("key", Value::Int(2)).unwrap();

    let value = facade.get("key").unwrap();
    assert_eq!(value, Some(Value::Int(2)));
}

#[test]
fn test_set_all_value_types() {
    let facade = create_facade();

    for (name, value) in standard_test_values() {
        let key = format!("type:{}", name);
        facade.set(&key, value.clone()).unwrap();

        let retrieved = facade.get(&key).unwrap();
        assert_eq!(retrieved, Some(value), "Failed for type: {}", name);
    }
}

// =============================================================================
// GETV TESTS (Versioned Get)
// =============================================================================

#[test]
fn test_getv_returns_version_info() {
    let facade = create_facade();

    facade.set("versioned", Value::Int(100)).unwrap();
    let versioned = facade.getv("versioned").unwrap();

    assert!(versioned.is_some());
    let v = versioned.unwrap();
    assert_eq!(v.value, Value::Int(100));
    assert!(v.version > 0, "Version should be positive");
    assert!(v.timestamp > 0, "Timestamp should be positive");
}

#[test]
fn test_getv_nonexistent_returns_none() {
    let facade = create_facade();

    let versioned = facade.getv("nonexistent").unwrap();
    assert!(versioned.is_none());
}

#[test]
fn test_getv_version_increases_on_update() {
    let facade = create_facade();

    facade.set("counter", Value::Int(1)).unwrap();
    let v1 = facade.getv("counter").unwrap().unwrap();

    facade.set("counter", Value::Int(2)).unwrap();
    let v2 = facade.getv("counter").unwrap().unwrap();

    assert!(v2.version > v1.version, "Version should increase on update");
}

// =============================================================================
// DELETE TESTS
// =============================================================================

#[test]
fn test_del_existing_returns_true() {
    let facade = create_facade();

    facade.set("to_delete", Value::Int(1)).unwrap();
    let deleted = facade.del("to_delete").unwrap();

    assert!(deleted, "Should return true for existing key");
}

#[test]
fn test_del_nonexistent_returns_false() {
    let facade = create_facade();

    let deleted = facade.del("nonexistent").unwrap();
    assert!(!deleted, "Should return false for nonexistent key");
}

#[test]
fn test_del_removes_key() {
    let facade = create_facade();

    facade.set("key", Value::Int(1)).unwrap();
    facade.del("key").unwrap();

    let value = facade.get("key").unwrap();
    assert!(value.is_none(), "Key should be deleted");
}

// =============================================================================
// EXISTS TESTS
// =============================================================================

#[test]
fn test_exists_returns_true_for_existing() {
    let facade = create_facade();

    facade.set("exists_key", Value::Int(1)).unwrap();
    let exists = facade.exists("exists_key").unwrap();

    assert!(exists);
}

#[test]
fn test_exists_returns_false_for_nonexistent() {
    let facade = create_facade();

    let exists = facade.exists("nonexistent").unwrap();
    assert!(!exists);
}

#[test]
fn test_exists_after_delete() {
    let facade = create_facade();

    facade.set("key", Value::Int(1)).unwrap();
    assert!(facade.exists("key").unwrap());

    facade.del("key").unwrap();
    assert!(!facade.exists("key").unwrap());
}

// =============================================================================
// GETSET TESTS
// =============================================================================

#[test]
fn test_getset_returns_old_value() {
    let facade = create_facade();

    facade.set("key", Value::Int(1)).unwrap();
    let old = facade.getset("key", Value::Int(2)).unwrap();

    assert_eq!(old, Some(Value::Int(1)));

    let current = facade.get("key").unwrap();
    assert_eq!(current, Some(Value::Int(2)));
}

#[test]
fn test_getset_nonexistent_returns_none() {
    let facade = create_facade();

    let old = facade.getset("new_key", Value::Int(1)).unwrap();
    assert!(old.is_none());

    let current = facade.get("new_key").unwrap();
    assert_eq!(current, Some(Value::Int(1)));
}

// =============================================================================
// SETNX TESTS (Set if Not eXists)
// =============================================================================

#[test]
fn test_setnx_creates_new_key() {
    let facade = create_facade();

    let success = facade.setnx("new_key", Value::Int(1)).unwrap();
    assert!(success, "Should succeed for new key");

    let value = facade.get("new_key").unwrap();
    assert_eq!(value, Some(Value::Int(1)));
}

#[test]
fn test_setnx_fails_for_existing() {
    let facade = create_facade();

    facade.set("existing", Value::Int(1)).unwrap();
    let success = facade.setnx("existing", Value::Int(2)).unwrap();

    assert!(!success, "Should fail for existing key");

    // Value should be unchanged
    let value = facade.get("existing").unwrap();
    assert_eq!(value, Some(Value::Int(1)));
}

// =============================================================================
// ERROR CASES
// =============================================================================

#[test]
fn test_empty_key_rejected() {
    let facade = create_facade();

    let result = facade.set("", Value::Int(1));
    assert!(result.is_err(), "Empty key should be rejected");
}

#[test]
fn test_reserved_prefix_rejected() {
    let facade = create_facade();

    let result = facade.set("_strata/internal", Value::Int(1));
    assert!(result.is_err(), "Reserved prefix should be rejected");
}

#[test]
fn test_key_with_nul_rejected() {
    let facade = create_facade();

    let result = facade.set("has\0nul", Value::Int(1));
    assert!(result.is_err(), "Key with NUL byte should be rejected");
}

#[test]
fn test_key_too_long_rejected() {
    let facade = create_facade();

    let long_key = "k".repeat(1025);
    let result = facade.set(&long_key, Value::Int(1));
    assert!(result.is_err(), "Key over 1024 bytes should be rejected");
}

#[test]
fn test_key_at_max_length_accepted() {
    let facade = create_facade();

    let max_key = "k".repeat(1024);
    let result = facade.set(&max_key, Value::Int(1));
    assert!(result.is_ok(), "Key at exactly 1024 bytes should succeed");
}
