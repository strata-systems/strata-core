//! Strata API Tests
//!
//! Tests for the high-level Strata typed wrapper API.
//! The Strata struct provides a convenient Rust API that wraps the
//! Executor's command-based interface with typed method calls.

use crate::common::*;
use strata_core::Value;
use strata_executor::{DistanceMetric, RunStatus};

// ============================================================================
// Database Operations
// ============================================================================

#[test]
fn ping_returns_version() {
    let db = create_strata();

    let version = db.ping().unwrap();

    assert!(!version.is_empty());
}

#[test]
fn info_returns_database_info() {
    let db = create_strata();

    let info = db.info().unwrap();

    assert!(!info.version.is_empty());
}

#[test]
fn flush_succeeds() {
    let db = create_strata();

    db.flush().unwrap();
}

#[test]
fn compact_succeeds() {
    let db = create_strata();

    db.compact().unwrap();
}

// ============================================================================
// KV Operations
// ============================================================================

#[test]
fn kv_put_get_cycle() {
    let db = create_strata();

    let version = db.kv_put("key1", Value::String("hello".into())).unwrap();
    assert!(version > 0);

    let value = db.kv_get("key1").unwrap();
    assert!(value.is_some());
    assert_eq!(value.unwrap().value, Value::String("hello".into()));
}

#[test]
fn kv_exists_and_delete() {
    let db = create_strata();

    db.kv_put("key1", Value::Int(42)).unwrap();
    assert!(db.kv_exists("key1").unwrap());

    db.kv_delete("key1").unwrap();
    assert!(!db.kv_exists("key1").unwrap());
}

#[test]
fn kv_incr() {
    let db = create_strata();

    db.kv_put("counter", Value::Int(10)).unwrap();
    let val = db.kv_incr("counter", 5).unwrap();
    assert_eq!(val, 15);

    let val = db.kv_incr("counter", -3).unwrap();
    assert_eq!(val, 12);
}

#[test]
fn kv_keys_with_prefix() {
    let db = create_strata();

    db.kv_put("user:1", Value::Int(1)).unwrap();
    db.kv_put("user:2", Value::Int(2)).unwrap();
    db.kv_put("order:1", Value::Int(3)).unwrap();

    let user_keys = db.kv_keys("user:", None).unwrap();
    assert_eq!(user_keys.len(), 2);

    let order_keys = db.kv_keys("order:", None).unwrap();
    assert_eq!(order_keys.len(), 1);
}

// ============================================================================
// State Operations
// ============================================================================

#[test]
fn state_set_and_read() {
    let db = create_strata();

    db.state_set("cell", Value::String("state".into())).unwrap();
    let value = db.state_read("cell").unwrap();
    assert!(value.is_some());
    assert_eq!(value.unwrap().value, Value::String("state".into()));
}

#[test]
fn state_exists_and_delete() {
    let db = create_strata();

    db.state_set("cell", Value::Int(1)).unwrap();
    assert!(db.state_exists("cell").unwrap());

    db.state_delete("cell").unwrap();
    assert!(!db.state_exists("cell").unwrap());
}

// ============================================================================
// Event Operations
// ============================================================================

#[test]
fn event_append_and_range() {
    let db = create_strata();

    // Event payloads must be Objects
    db.event_append("stream", event_payload("value", Value::Int(1))).unwrap();
    db.event_append("stream", event_payload("value", Value::Int(2))).unwrap();

    let events = db.event_range("stream", None, None, None).unwrap();
    assert_eq!(events.len(), 2);
}

#[test]
fn event_len() {
    let db = create_strata();

    for i in 0..5 {
        db.event_append("counting", event_payload("n", Value::Int(i))).unwrap();
    }

    let len = db.event_len("counting").unwrap();
    assert_eq!(len, 5);
}

// ============================================================================
// Vector Operations
// ============================================================================

#[test]
fn vector_create_collection_and_upsert() {
    let db = create_strata();

    db.vector_create_collection("vecs", 4u64, DistanceMetric::Cosine).unwrap();
    db.vector_upsert("vecs", "v1", vec![1.0, 0.0, 0.0, 0.0], None).unwrap();

    let vector = db.vector_get("vecs", "v1").unwrap();
    assert!(vector.is_some());
}

#[test]
fn vector_search() {
    let db = create_strata();

    db.vector_create_collection("search", 4u64, DistanceMetric::Cosine).unwrap();
    db.vector_upsert("search", "v1", vec![1.0, 0.0, 0.0, 0.0], None).unwrap();
    db.vector_upsert("search", "v2", vec![0.0, 1.0, 0.0, 0.0], None).unwrap();

    let matches = db.vector_search("search", vec![1.0, 0.0, 0.0, 0.0], 10u64).unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].key, "v1");
}

#[test]
fn vector_list_collections() {
    let db = create_strata();

    db.vector_create_collection("coll_a", 4u64, DistanceMetric::Cosine).unwrap();
    db.vector_create_collection("coll_b", 8u64, DistanceMetric::Euclidean).unwrap();

    let collections = db.vector_list_collections().unwrap();
    assert_eq!(collections.len(), 2);
}

#[test]
fn vector_delete_collection() {
    let db = create_strata();

    db.vector_create_collection("to_delete", 4u64, DistanceMetric::Cosine).unwrap();
    assert!(db.vector_collection_exists("to_delete").unwrap());

    db.vector_delete_collection("to_delete").unwrap();
    assert!(!db.vector_collection_exists("to_delete").unwrap());
}

// ============================================================================
// Run Operations
// ============================================================================

#[test]
fn run_create_and_get() {
    let db = create_strata();

    // Users can name runs like git branches
    let (info, _version) = db.run_create(
        Some("my-agent-run".to_string()),
        None,
    ).unwrap();
    assert_eq!(info.id.as_str(), "my-agent-run");

    let run_info = db.run_get(info.id.as_str()).unwrap();
    assert!(run_info.is_some());
    assert_eq!(run_info.unwrap().info.id.as_str(), "my-agent-run");
}

#[test]
fn run_list() {
    let db = create_strata();

    db.run_create(Some("dev".to_string()), None).unwrap();
    db.run_create(Some("prod".to_string()), None).unwrap();

    let runs = db.run_list(None, None, None).unwrap();
    // At least our two runs plus default
    assert!(runs.len() >= 2);
}

#[test]
fn run_complete() {
    let db = create_strata();

    let (info, _) = db.run_create(Some("task-1".to_string()), None).unwrap();

    db.run_complete(info.id.as_str()).unwrap();

    let run_info = db.run_get(info.id.as_str()).unwrap().unwrap();
    assert_eq!(run_info.info.status, RunStatus::Completed);
}

#[test]
fn run_fail() {
    let db = create_strata();

    let (info, _) = db.run_create(Some("task-2".to_string()), None).unwrap();

    db.run_fail(info.id.as_str(), "something went wrong").unwrap();

    let run_info = db.run_get(info.id.as_str()).unwrap().unwrap();
    assert_eq!(run_info.info.status, RunStatus::Failed);
}

#[test]
fn run_tags() {
    let db = create_strata();

    let (info, _) = db.run_create(Some("tagged-run".to_string()), None).unwrap();

    db.run_add_tags(info.id.as_str(), vec!["tag1".into(), "tag2".into()]).unwrap();

    let tags = db.run_get_tags(info.id.as_str()).unwrap();
    assert!(tags.contains(&"tag1".to_string()));
    assert!(tags.contains(&"tag2".to_string()));

    db.run_remove_tags(info.id.as_str(), vec!["tag1".into()]).unwrap();

    let tags = db.run_get_tags(info.id.as_str()).unwrap();
    assert!(!tags.contains(&"tag1".to_string()));
    assert!(tags.contains(&"tag2".to_string()));
}

// ============================================================================
// JSON Operations
// ============================================================================

#[test]
fn json_set_and_get() {
    let db = create_strata();

    let doc = Value::Object([
        ("name".to_string(), Value::String("Alice".into())),
        ("age".to_string(), Value::Int(30)),
    ].into_iter().collect());

    db.json_set("user:1", "$", doc).unwrap();

    let result = db.json_get("user:1", "$").unwrap();
    assert!(result.is_some());

    let value = result.unwrap();
    match &value.value {
        Value::Object(map) => {
            assert_eq!(map.get("name"), Some(&Value::String("Alice".into())));
        }
        _ => panic!("Expected Object"),
    }
}

#[test]
fn json_exists_and_delete() {
    let db = create_strata();

    let doc = Value::Object([
        ("key".to_string(), Value::Int(1)),
    ].into_iter().collect());

    db.json_set("doc1", "$", doc).unwrap();
    assert!(db.json_exists("doc1").unwrap());

    db.json_delete("doc1", "$").unwrap();
    assert!(!db.json_exists("doc1").unwrap());
}

// ============================================================================
// Cross-Primitive Usage
// ============================================================================

#[test]
fn use_all_primitives() {
    let db = create_strata();

    // KV
    db.kv_put("config", Value::String("enabled".into())).unwrap();

    // State
    db.state_set("status", Value::String("running".into())).unwrap();

    // Event
    db.event_append("audit", event_payload("action", Value::String("start".into()))).unwrap();

    // Vector
    db.vector_create_collection("embeddings", 4u64, DistanceMetric::Cosine).unwrap();
    db.vector_upsert("embeddings", "e1", vec![1.0, 0.0, 0.0, 0.0], None).unwrap();

    // JSON
    let doc = Value::Object([
        ("type".to_string(), Value::String("test".into())),
    ].into_iter().collect());
    db.json_set("doc1", "$", doc).unwrap();

    // Run
    let (run_info, _) = db.run_create(Some("integration-test".to_string()), None).unwrap();

    // Verify all data
    assert!(db.kv_exists("config").unwrap());
    assert!(db.state_exists("status").unwrap());
    assert_eq!(db.event_len("audit").unwrap(), 1);
    assert!(db.vector_collection_exists("embeddings").unwrap());
    assert!(db.json_exists("doc1").unwrap());
    assert!(db.run_get(run_info.id.as_str()).unwrap().is_some());
}

// ============================================================================
// Session Access
// ============================================================================

#[test]
fn session_from_strata() {
    let db = create_db();
    let strata = strata_executor::Strata::new(db.clone());

    // Strata provides executor access
    let _executor = strata.executor();

    // Session can be created from the same db
    let _session = strata_executor::Strata::session(db);
}
