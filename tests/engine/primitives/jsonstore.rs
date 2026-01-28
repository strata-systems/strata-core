//! JsonStore Primitive Tests
//!
//! Tests for JSON document storage with path-based operations.

use crate::common::*;
use std::str::FromStr;

// Helper function to parse path or return root
fn path(s: &str) -> JsonPath {
    if s.is_empty() {
        JsonPath::root()
    } else {
        JsonPath::from_str(s).expect("valid path")
    }
}

// ============================================================================
// Basic CRUD
// ============================================================================

#[test]
fn create_and_get() {
    let test_db = TestDb::new();
    let json = test_db.json();

    let doc = serde_json::json!({"name": "test", "value": 42});
    json.create(&test_db.run_id, "doc1", doc.clone().into()).unwrap();

    let result = json.get(&test_db.run_id, "doc1", &JsonPath::root()).unwrap();
    assert!(result.is_some());
    // Compare the inner value
    let result_json: serde_json::Value = result.unwrap().value.into();
    assert_eq!(result_json, doc);
}

#[test]
fn create_fails_if_exists() {
    let test_db = TestDb::new();
    let json = test_db.json();

    let doc: JsonValue = serde_json::json!({"x": 1}).into();
    json.create(&test_db.run_id, "doc1", doc.clone()).unwrap();

    let result = json.create(&test_db.run_id, "doc1", doc);
    assert!(result.is_err());
}

#[test]
fn get_nonexistent_returns_none() {
    let test_db = TestDb::new();
    let json = test_db.json();

    let result = json.get(&test_db.run_id, "nonexistent", &JsonPath::root()).unwrap();
    assert!(result.is_none());
}

#[test]
fn exists_returns_correct_status() {
    let test_db = TestDb::new();
    let json = test_db.json();

    assert!(!json.exists(&test_db.run_id, "doc1").unwrap());

    json.create(&test_db.run_id, "doc1", serde_json::json!({}).into()).unwrap();
    assert!(json.exists(&test_db.run_id, "doc1").unwrap());
}

#[test]
fn destroy_removes_document() {
    let test_db = TestDb::new();
    let json = test_db.json();

    json.create(&test_db.run_id, "doc1", serde_json::json!({}).into()).unwrap();
    assert!(json.exists(&test_db.run_id, "doc1").unwrap());

    let destroyed = json.destroy(&test_db.run_id, "doc1").unwrap();
    assert!(destroyed);

    assert!(!json.exists(&test_db.run_id, "doc1").unwrap());
}

#[test]
fn destroy_nonexistent_returns_false() {
    let test_db = TestDb::new();
    let json = test_db.json();

    let destroyed = json.destroy(&test_db.run_id, "nonexistent").unwrap();
    assert!(!destroyed);
}

// ============================================================================
// Path Operations
// ============================================================================

#[test]
fn get_at_path() {
    let test_db = TestDb::new();
    let json = test_db.json();

    let doc = serde_json::json!({
        "user": {
            "name": "Alice",
            "age": 30
        }
    });
    json.create(&test_db.run_id, "doc1", doc.into()).unwrap();

    let name = json.get(&test_db.run_id, "doc1", &path("user.name")).unwrap();
    assert!(name.is_some());
    let name_val: serde_json::Value = name.unwrap().value.into();
    assert_eq!(name_val, serde_json::json!("Alice"));

    let age = json.get(&test_db.run_id, "doc1", &path("user.age")).unwrap();
    assert!(age.is_some());
    let age_val: serde_json::Value = age.unwrap().value.into();
    assert_eq!(age_val, serde_json::json!(30));
}

#[test]
fn set_at_path() {
    let test_db = TestDb::new();
    let json = test_db.json();

    let doc = serde_json::json!({"x": 1});
    json.create(&test_db.run_id, "doc1", doc.into()).unwrap();

    json.set(&test_db.run_id, "doc1", &path("y"), serde_json::json!(2).into()).unwrap();

    let result = json.get(&test_db.run_id, "doc1", &JsonPath::root()).unwrap().unwrap();
    assert_eq!(result.value["x"], serde_json::json!(1));
    assert_eq!(result.value["y"], serde_json::json!(2));
}

#[test]
fn set_nested_path() {
    let test_db = TestDb::new();
    let json = test_db.json();

    let doc = serde_json::json!({"a": {"b": 1}});
    json.create(&test_db.run_id, "doc1", doc.into()).unwrap();

    json.set(&test_db.run_id, "doc1", &path("a.c"), serde_json::json!(2).into()).unwrap();

    let result = json.get(&test_db.run_id, "doc1", &JsonPath::root()).unwrap().unwrap();
    assert_eq!(result.value["a"]["b"], serde_json::json!(1));
    assert_eq!(result.value["a"]["c"], serde_json::json!(2));
}

#[test]
fn delete_at_path() {
    let test_db = TestDb::new();
    let json = test_db.json();

    let doc = serde_json::json!({"x": 1, "y": 2});
    json.create(&test_db.run_id, "doc1", doc.into()).unwrap();

    json.delete_at_path(&test_db.run_id, "doc1", &path("y")).unwrap();

    let result = json.get(&test_db.run_id, "doc1", &JsonPath::root()).unwrap().unwrap();
    let result_json: serde_json::Value = result.value.into();
    assert_eq!(result_json, serde_json::json!({"x": 1}));
}

// ============================================================================
// Array Operations
// ============================================================================

#[test]
fn array_push() {
    let test_db = TestDb::new();
    let json = test_db.json();

    let doc = serde_json::json!({"items": [1, 2]});
    json.create(&test_db.run_id, "doc1", doc.into()).unwrap();

    json.array_push(&test_db.run_id, "doc1", &path("items"), vec![serde_json::json!(3).into()]).unwrap();

    let result = json.get(&test_db.run_id, "doc1", &path("items")).unwrap().unwrap();
    let result_json: serde_json::Value = result.value.into();
    assert_eq!(result_json, serde_json::json!([1, 2, 3]));
}

#[test]
fn array_pop() {
    let test_db = TestDb::new();
    let json = test_db.json();

    let doc = serde_json::json!({"items": [1, 2, 3]});
    json.create(&test_db.run_id, "doc1", doc.into()).unwrap();

    let (_, popped) = json.array_pop(&test_db.run_id, "doc1", &path("items")).unwrap();
    assert!(popped.is_some());
    let popped_json: serde_json::Value = popped.unwrap().into();
    assert_eq!(popped_json, serde_json::json!(3));

    let result = json.get(&test_db.run_id, "doc1", &path("items")).unwrap().unwrap();
    let result_json: serde_json::Value = result.value.into();
    assert_eq!(result_json, serde_json::json!([1, 2]));
}

// ============================================================================
// Merge Operations
// ============================================================================

#[test]
fn merge_adds_new_fields() {
    let test_db = TestDb::new();
    let json = test_db.json();

    let doc = serde_json::json!({"x": 1});
    json.create(&test_db.run_id, "doc1", doc.into()).unwrap();

    json.merge(&test_db.run_id, "doc1", &JsonPath::root(), serde_json::json!({"y": 2}).into()).unwrap();

    let result = json.get(&test_db.run_id, "doc1", &JsonPath::root()).unwrap().unwrap();
    let result_json: serde_json::Value = result.value.into();
    assert_eq!(result_json, serde_json::json!({"x": 1, "y": 2}));
}

#[test]
fn merge_overwrites_existing_fields() {
    let test_db = TestDb::new();
    let json = test_db.json();

    let doc = serde_json::json!({"x": 1});
    json.create(&test_db.run_id, "doc1", doc.into()).unwrap();

    json.merge(&test_db.run_id, "doc1", &JsonPath::root(), serde_json::json!({"x": 100}).into()).unwrap();

    let result = json.get(&test_db.run_id, "doc1", &JsonPath::root()).unwrap().unwrap();
    let result_json: serde_json::Value = result.value.into();
    assert_eq!(result_json, serde_json::json!({"x": 100}));
}

#[test]
fn merge_null_removes_field() {
    let test_db = TestDb::new();
    let json = test_db.json();

    let doc = serde_json::json!({"x": 1, "y": 2});
    json.create(&test_db.run_id, "doc1", doc.into()).unwrap();

    // RFC 7396: null in patch means delete
    json.merge(&test_db.run_id, "doc1", &JsonPath::root(), serde_json::json!({"y": null}).into()).unwrap();

    let result = json.get(&test_db.run_id, "doc1", &JsonPath::root()).unwrap().unwrap();
    let result_json: serde_json::Value = result.value.into();
    assert_eq!(result_json, serde_json::json!({"x": 1}));
}

// ============================================================================
// CAS Operations
// ============================================================================

#[test]
fn cas_succeeds_with_correct_version() {
    let test_db = TestDb::new();
    let json = test_db.json();

    json.create(&test_db.run_id, "doc1", serde_json::json!({"x": 1}).into()).unwrap();

    let current = json.get(&test_db.run_id, "doc1", &JsonPath::root()).unwrap().unwrap();
    let version = current.version.as_u64();

    let result = json.cas(&test_db.run_id, "doc1", version, &JsonPath::root(), serde_json::json!({"x": 2}).into());
    assert!(result.is_ok());

    let doc = json.get(&test_db.run_id, "doc1", &JsonPath::root()).unwrap().unwrap();
    assert_eq!(doc.value["x"], serde_json::json!(2));
}

#[test]
fn cas_fails_with_wrong_version() {
    let test_db = TestDb::new();
    let json = test_db.json();

    json.create(&test_db.run_id, "doc1", serde_json::json!({"x": 1}).into()).unwrap();

    let result = json.cas(&test_db.run_id, "doc1", 999999u64, &JsonPath::root(), serde_json::json!({"x": 2}).into());
    assert!(result.is_err());

    // Value unchanged
    let doc = json.get(&test_db.run_id, "doc1", &JsonPath::root()).unwrap().unwrap();
    assert_eq!(doc.value["x"], serde_json::json!(1));
}

// ============================================================================
// List & Count
// ============================================================================

#[test]
fn list_returns_all_documents() {
    let test_db = TestDb::new();
    let json = test_db.json();

    json.create(&test_db.run_id, "doc1", serde_json::json!({}).into()).unwrap();
    json.create(&test_db.run_id, "doc2", serde_json::json!({}).into()).unwrap();
    json.create(&test_db.run_id, "doc3", serde_json::json!({}).into()).unwrap();

    let docs = json.list(&test_db.run_id, None, None, 100).unwrap();
    assert_eq!(docs.doc_ids.len(), 3);
}

#[test]
fn count_returns_document_count() {
    let test_db = TestDb::new();
    let json = test_db.json();

    assert_eq!(json.count(&test_db.run_id).unwrap(), 0);

    json.create(&test_db.run_id, "doc1", serde_json::json!({}).into()).unwrap();
    json.create(&test_db.run_id, "doc2", serde_json::json!({}).into()).unwrap();

    assert_eq!(json.count(&test_db.run_id).unwrap(), 2);
}

// ============================================================================
// Increment
// ============================================================================

#[test]
fn increment_numeric_field() {
    let test_db = TestDb::new();
    let json = test_db.json();

    let doc = serde_json::json!({"counter": 10});
    json.create(&test_db.run_id, "doc1", doc.into()).unwrap();

    let (_, new_val) = json.increment(&test_db.run_id, "doc1", &path("counter"), 5.0).unwrap();

    assert_eq!(new_val, 15.0);
}

#[test]
fn increment_negative() {
    let test_db = TestDb::new();
    let json = test_db.json();

    let doc = serde_json::json!({"counter": 10});
    json.create(&test_db.run_id, "doc1", doc.into()).unwrap();

    let (_, new_val) = json.increment(&test_db.run_id, "doc1", &path("counter"), -3.0).unwrap();

    assert_eq!(new_val, 7.0);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn empty_document() {
    let test_db = TestDb::new();
    let json = test_db.json();

    json.create(&test_db.run_id, "doc1", serde_json::json!({}).into()).unwrap();

    let result = json.get(&test_db.run_id, "doc1", &JsonPath::root()).unwrap().unwrap();
    let result_json: serde_json::Value = result.value.into();
    assert_eq!(result_json, serde_json::json!({}));
}

#[test]
fn deeply_nested_document() {
    let test_db = TestDb::new();
    let json = test_db.json();

    let doc = serde_json::json!({
        "a": {"b": {"c": {"d": {"e": 42}}}}
    });
    json.create(&test_db.run_id, "doc1", doc.into()).unwrap();

    let result = json.get(&test_db.run_id, "doc1", &path("a.b.c.d.e")).unwrap();
    assert!(result.is_some());
    let result_json: serde_json::Value = result.unwrap().value.into();
    assert_eq!(result_json, serde_json::json!(42));
}

#[test]
fn various_json_types() {
    let test_db = TestDb::new();
    let json = test_db.json();

    let doc = serde_json::json!({
        "string": "hello",
        "number": 42,
        "float": 3.14,
        "bool": true,
        "null": null,
        "array": [1, 2, 3],
        "object": {"nested": true}
    });
    json.create(&test_db.run_id, "doc1", doc.clone().into()).unwrap();

    let result = json.get(&test_db.run_id, "doc1", &JsonPath::root()).unwrap().unwrap();
    let result_json: serde_json::Value = result.value.into();
    assert_eq!(result_json, doc);
}
