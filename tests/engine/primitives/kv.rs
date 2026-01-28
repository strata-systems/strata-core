//! KVStore Primitive Tests
//!
//! Tests for key-value storage operations.

use crate::common::*;

// ============================================================================
// Basic CRUD
// ============================================================================

#[test]
fn get_nonexistent_returns_none() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    let result = kv.get(&test_db.run_id, "nonexistent").unwrap();
    assert!(result.is_none());
}

#[test]
fn put_and_get() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    kv.put(&test_db.run_id, "key", Value::Int(42)).unwrap();

    let result = kv.get(&test_db.run_id, "key").unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().value, Value::Int(42));
}

#[test]
fn put_returns_version() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    let version = kv.put(&test_db.run_id, "key", Value::Int(1)).unwrap();
    assert!(version.as_u64() > 0);
}

#[test]
fn put_overwrites_value() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    kv.put(&test_db.run_id, "key", Value::Int(1)).unwrap();
    kv.put(&test_db.run_id, "key", Value::Int(2)).unwrap();

    let result = kv.get(&test_db.run_id, "key").unwrap();
    assert_eq!(result.unwrap().value, Value::Int(2));
}

#[test]
fn put_increments_version() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    let v1 = kv.put(&test_db.run_id, "key", Value::Int(1)).unwrap();
    let v2 = kv.put(&test_db.run_id, "key", Value::Int(2)).unwrap();

    assert!(v2.as_u64() > v1.as_u64());
}

#[test]
fn delete_existing_returns_true() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    kv.put(&test_db.run_id, "key", Value::Int(42)).unwrap();

    let deleted = kv.delete(&test_db.run_id, "key").unwrap();
    assert!(deleted);

    let result = kv.get(&test_db.run_id, "key").unwrap();
    assert!(result.is_none());
}

#[test]
fn delete_nonexistent_returns_false() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    let deleted = kv.delete(&test_db.run_id, "nonexistent").unwrap();
    assert!(!deleted);
}

#[test]
fn exists_returns_correct_status() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    assert!(!kv.exists(&test_db.run_id, "key").unwrap());

    kv.put(&test_db.run_id, "key", Value::Int(1)).unwrap();
    assert!(kv.exists(&test_db.run_id, "key").unwrap());

    kv.delete(&test_db.run_id, "key").unwrap();
    assert!(!kv.exists(&test_db.run_id, "key").unwrap());
}

// ============================================================================
// List Operations
// ============================================================================

#[test]
fn list_empty_returns_empty() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    let keys = kv.list(&test_db.run_id, None).unwrap();
    assert!(keys.is_empty());
}

#[test]
fn list_returns_all_keys() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    kv.put(&test_db.run_id, "a", Value::Int(1)).unwrap();
    kv.put(&test_db.run_id, "b", Value::Int(2)).unwrap();
    kv.put(&test_db.run_id, "c", Value::Int(3)).unwrap();

    let keys = kv.list(&test_db.run_id, None).unwrap();
    assert_eq!(keys.len(), 3);
    assert!(keys.contains(&"a".to_string()));
    assert!(keys.contains(&"b".to_string()));
    assert!(keys.contains(&"c".to_string()));
}

#[test]
fn list_with_prefix_filters() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    kv.put(&test_db.run_id, "user:1", Value::Int(1)).unwrap();
    kv.put(&test_db.run_id, "user:2", Value::Int(2)).unwrap();
    kv.put(&test_db.run_id, "item:1", Value::Int(3)).unwrap();

    let user_keys = kv.list(&test_db.run_id, Some("user:")).unwrap();
    assert_eq!(user_keys.len(), 2);

    let item_keys = kv.list(&test_db.run_id, Some("item:")).unwrap();
    assert_eq!(item_keys.len(), 1);

    let no_match = kv.list(&test_db.run_id, Some("other:")).unwrap();
    assert!(no_match.is_empty());
}

#[test]
fn list_with_values() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    kv.put(&test_db.run_id, "x", Value::Int(10)).unwrap();
    kv.put(&test_db.run_id, "y", Value::Int(20)).unwrap();

    let entries = kv.list_with_values(&test_db.run_id, None).unwrap();
    assert_eq!(entries.len(), 2);

    // Entries have keys and values
    for (key, versioned) in &entries {
        match key.as_str() {
            "x" => assert_eq!(versioned.value, Value::Int(10)),
            "y" => assert_eq!(versioned.value, Value::Int(20)),
            _ => panic!("Unexpected key: {}", key),
        }
    }
}

// ============================================================================
// Batch Operations
// ============================================================================

#[test]
fn get_many_returns_matching_values() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    kv.put(&test_db.run_id, "a", Value::Int(1)).unwrap();
    kv.put(&test_db.run_id, "b", Value::Int(2)).unwrap();

    let results = kv.get_many(&test_db.run_id, &["a", "b", "c"]).unwrap();

    assert_eq!(results.len(), 3);
    assert!(results[0].is_some()); // a
    assert!(results[1].is_some()); // b
    assert!(results[2].is_none()); // c doesn't exist
}

// ============================================================================
// Value Types
// ============================================================================

#[test]
fn supports_int_values() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    kv.put(&test_db.run_id, "int", Value::Int(42)).unwrap();
    let result = kv.get(&test_db.run_id, "int").unwrap();
    assert_eq!(result.unwrap().value, Value::Int(42));
}

#[test]
fn supports_string_values() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    kv.put(&test_db.run_id, "str", Value::String("hello".into())).unwrap();
    let result = kv.get(&test_db.run_id, "str").unwrap();
    assert_eq!(result.unwrap().value, Value::String("hello".into()));
}

#[test]
fn supports_bool_values() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    kv.put(&test_db.run_id, "bool", Value::Bool(true)).unwrap();
    let result = kv.get(&test_db.run_id, "bool").unwrap();
    assert_eq!(result.unwrap().value, Value::Bool(true));
}

#[test]
fn supports_float_values() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    kv.put(&test_db.run_id, "float", Value::Float(3.14.into())).unwrap();
    let result = kv.get(&test_db.run_id, "float").unwrap();

    match result.unwrap().value {
        Value::Float(f) => assert!((f64::from(f) - 3.14).abs() < 0.001),
        _ => panic!("Expected float"),
    }
}

#[test]
fn supports_bytes_values() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    kv.put(&test_db.run_id, "bytes", Value::Bytes(vec![1, 2, 3])).unwrap();
    let result = kv.get(&test_db.run_id, "bytes").unwrap();
    assert_eq!(result.unwrap().value, Value::Bytes(vec![1, 2, 3]));
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn empty_string_key_works() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    kv.put(&test_db.run_id, "", Value::Int(1)).unwrap();
    let result = kv.get(&test_db.run_id, "").unwrap();
    assert!(result.is_some());
}

#[test]
fn long_key_works() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    let long_key = "k".repeat(1000);
    kv.put(&test_db.run_id, &long_key, Value::Int(1)).unwrap();
    let result = kv.get(&test_db.run_id, &long_key).unwrap();
    assert!(result.is_some());
}

#[test]
fn special_characters_in_key() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    let special_key = "key/with:special@chars#and$symbols";
    kv.put(&test_db.run_id, special_key, Value::Int(1)).unwrap();
    let result = kv.get(&test_db.run_id, special_key).unwrap();
    assert!(result.is_some());
}
