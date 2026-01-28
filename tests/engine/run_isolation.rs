//! Run Isolation Tests
//!
//! Tests that verify data isolation between different runs.

use crate::common::*;
use strata_core::primitives::json::JsonPath;
use std::collections::HashMap;

/// Helper to create an event payload object
fn event_payload(data: Value) -> Value {
    Value::Object(HashMap::from([
        ("data".to_string(), data),
    ]))
}

// ============================================================================
// KVStore Isolation
// ============================================================================

#[test]
fn kv_runs_are_isolated() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    let run_a = RunId::new();
    let run_b = RunId::new();

    // Same key, different runs
    kv.put(&run_a, "key", Value::Int(1)).unwrap();
    kv.put(&run_b, "key", Value::Int(2)).unwrap();

    // Each run sees its own value
    assert_eq!(kv.get(&run_a, "key").unwrap().unwrap().value, Value::Int(1));
    assert_eq!(kv.get(&run_b, "key").unwrap().unwrap().value, Value::Int(2));
}

#[test]
fn kv_delete_doesnt_affect_other_run() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    let run_a = RunId::new();
    let run_b = RunId::new();

    kv.put(&run_a, "key", Value::Int(1)).unwrap();
    kv.put(&run_b, "key", Value::Int(2)).unwrap();

    kv.delete(&run_a, "key").unwrap();

    // Run A's key is gone
    assert!(kv.get(&run_a, "key").unwrap().is_none());

    // Run B's key still exists
    assert_eq!(kv.get(&run_b, "key").unwrap().unwrap().value, Value::Int(2));
}

#[test]
fn kv_list_only_shows_run_keys() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    let run_a = RunId::new();
    let run_b = RunId::new();

    kv.put(&run_a, "a1", Value::Int(1)).unwrap();
    kv.put(&run_a, "a2", Value::Int(2)).unwrap();
    kv.put(&run_b, "b1", Value::Int(3)).unwrap();

    let keys_a = kv.list(&run_a, None).unwrap();
    let keys_b = kv.list(&run_b, None).unwrap();

    assert_eq!(keys_a.len(), 2);
    assert_eq!(keys_b.len(), 1);

    assert!(keys_a.contains(&"a1".to_string()));
    assert!(keys_a.contains(&"a2".to_string()));
    assert!(keys_b.contains(&"b1".to_string()));
}

// ============================================================================
// EventLog Isolation
// ============================================================================

#[test]
fn eventlog_runs_are_isolated() {
    let test_db = TestDb::new();
    let event = test_db.event();

    let run_a = RunId::new();
    let run_b = RunId::new();

    event.append(&run_a, "type", event_payload(Value::String("run_a".into()))).unwrap();
    event.append(&run_a, "type", event_payload(Value::String("run_a_2".into()))).unwrap();
    event.append(&run_b, "type", event_payload(Value::String("run_b".into()))).unwrap();

    assert_eq!(event.len(&run_a).unwrap(), 2);
    assert_eq!(event.len(&run_b).unwrap(), 1);
}

#[test]
fn eventlog_sequence_numbers_per_run() {
    let test_db = TestDb::new();
    let event = test_db.event();

    let run_a = RunId::new();
    let run_b = RunId::new();

    // Both runs start sequence at 0
    let seq_a = event.append(&run_a, "type", event_payload(Value::Int(1))).unwrap();
    let seq_b = event.append(&run_b, "type", event_payload(Value::Int(1))).unwrap();

    assert_eq!(seq_a.as_u64(), 0);
    assert_eq!(seq_b.as_u64(), 0);
}

#[test]
fn eventlog_hash_chain_per_run() {
    let test_db = TestDb::new();
    let event = test_db.event();

    let run_a = RunId::new();
    let run_b = RunId::new();

    for i in 0..5 {
        event.append(&run_a, "type", event_payload(Value::Int(i))).unwrap();
        event.append(&run_b, "type", event_payload(Value::Int(i * 10))).unwrap();
    }

    // Both chains should be valid independently
    let chain_a = event.verify_chain(&run_a).unwrap();
    let chain_b = event.verify_chain(&run_b).unwrap();

    assert!(chain_a.is_valid);
    assert!(chain_b.is_valid);
    assert_eq!(chain_a.length, 5);
    assert_eq!(chain_b.length, 5);
}

// ============================================================================
// StateCell Isolation
// ============================================================================

#[test]
fn statecell_runs_are_isolated() {
    let test_db = TestDb::new();
    let state = test_db.state();

    let run_a = RunId::new();
    let run_b = RunId::new();

    // Same cell name, different runs
    state.init(&run_a, "cell", Value::Int(1)).unwrap();
    state.init(&run_b, "cell", Value::Int(2)).unwrap();

    assert_eq!(state.read(&run_a, "cell").unwrap().unwrap().value.value, Value::Int(1));
    assert_eq!(state.read(&run_b, "cell").unwrap().unwrap().value.value, Value::Int(2));
}

#[test]
fn statecell_cas_isolated() {
    let test_db = TestDb::new();
    let state = test_db.state();

    let run_a = RunId::new();
    let run_b = RunId::new();

    state.init(&run_a, "cell", Value::Int(0)).unwrap();
    state.init(&run_b, "cell", Value::Int(0)).unwrap();

    let version_a = state.read(&run_a, "cell").unwrap().unwrap().version;
    let version_b = state.read(&run_b, "cell").unwrap().unwrap().version;

    // CAS on run A
    state.cas(&run_a, "cell", version_a, Value::Int(100)).unwrap();

    // Run B unchanged
    assert_eq!(state.read(&run_b, "cell").unwrap().unwrap().value.value, Value::Int(0));

    // CAS on run B still works with its original version
    state.cas(&run_b, "cell", version_b, Value::Int(200)).unwrap();

    // Both have their own values
    assert_eq!(state.read(&run_a, "cell").unwrap().unwrap().value.value, Value::Int(100));
    assert_eq!(state.read(&run_b, "cell").unwrap().unwrap().value.value, Value::Int(200));
}

// ============================================================================
// JsonStore Isolation
// ============================================================================

#[test]
fn jsonstore_runs_are_isolated() {
    let test_db = TestDb::new();
    let json = test_db.json();

    let run_a = RunId::new();
    let run_b = RunId::new();

    json.create(&run_a, "doc", serde_json::json!({"run": "a"}).into()).unwrap();
    json.create(&run_b, "doc", serde_json::json!({"run": "b"}).into()).unwrap();

    let a_doc = json.get(&run_a, "doc", &JsonPath::root()).unwrap().unwrap();
    let b_doc = json.get(&run_b, "doc", &JsonPath::root()).unwrap().unwrap();

    assert_eq!(a_doc.value["run"], "a");
    assert_eq!(b_doc.value["run"], "b");
}

#[test]
fn jsonstore_count_per_run() {
    let test_db = TestDb::new();
    let json = test_db.json();

    let run_a = RunId::new();
    let run_b = RunId::new();

    json.create(&run_a, "doc1", serde_json::json!({}).into()).unwrap();
    json.create(&run_a, "doc2", serde_json::json!({}).into()).unwrap();
    json.create(&run_b, "doc1", serde_json::json!({}).into()).unwrap();

    assert_eq!(json.count(&run_a).unwrap(), 2);
    assert_eq!(json.count(&run_b).unwrap(), 1);
}

// ============================================================================
// VectorStore Isolation
// ============================================================================

#[test]
fn vectorstore_collections_per_run() {
    let test_db = TestDb::new();
    let vector = test_db.vector();

    let run_a = RunId::new();
    let run_b = RunId::new();

    let config = config_small();

    // Same collection name, different runs
    vector.create_collection(run_a, "coll", config.clone()).unwrap();
    vector.create_collection(run_b, "coll", config.clone()).unwrap();

    // Both exist independently
    assert!(vector.collection_exists(run_a, "coll").unwrap());
    assert!(vector.collection_exists(run_b, "coll").unwrap());

    // Delete from run A doesn't affect run B
    vector.delete_collection(run_a, "coll").unwrap();

    assert!(!vector.collection_exists(run_a, "coll").unwrap());
    assert!(vector.collection_exists(run_b, "coll").unwrap());
}

#[test]
fn vectorstore_vectors_per_run() {
    let test_db = TestDb::new();
    let vector = test_db.vector();

    let run_a = RunId::new();
    let run_b = RunId::new();

    let config = config_small();
    vector.create_collection(run_a, "coll", config.clone()).unwrap();
    vector.create_collection(run_b, "coll", config.clone()).unwrap();

    // Insert different vectors with same key
    vector.insert(run_a, "coll", "vec", &[1.0f32, 0.0, 0.0], None).unwrap();
    vector.insert(run_b, "coll", "vec", &[0.0f32, 1.0, 0.0], None).unwrap();

    let a_vec = vector.get(run_a, "coll", "vec").unwrap().unwrap();
    let b_vec = vector.get(run_b, "coll", "vec").unwrap().unwrap();

    assert_eq!(a_vec.value.embedding, vec![1.0f32, 0.0, 0.0]);
    assert_eq!(b_vec.value.embedding, vec![0.0f32, 1.0, 0.0]);
}

// ============================================================================
// Cross-Primitive Run Isolation
// ============================================================================

#[test]
fn all_primitives_isolated_by_run() {
    let test_db = TestDb::new();
    let prims = test_db.all_primitives();

    let run_a = RunId::new();
    let run_b = RunId::new();

    // Write to all primitives in run A
    prims.kv.put(&run_a, "key", Value::Int(1)).unwrap();
    prims.event.append(&run_a, "type", event_payload(Value::Int(1))).unwrap();
    prims.state.init(&run_a, "cell", Value::Int(1)).unwrap();
    prims.json.create(&run_a, "doc", serde_json::json!({"n": 1}).into()).unwrap();

    // Run B should see nothing
    assert!(prims.kv.get(&run_b, "key").unwrap().is_none());
    assert_eq!(prims.event.len(&run_b).unwrap(), 0);
    assert!(!prims.state.exists(&run_b, "cell").unwrap());
    assert!(!prims.json.exists(&run_b, "doc").unwrap());
}

#[test]
fn many_runs_no_interference() {
    let test_db = TestDb::new();
    let kv = test_db.kv();

    // Create 10 runs, each with its own data
    let runs: Vec<RunId> = (0..10).map(|_| RunId::new()).collect();

    for (i, run_id) in runs.iter().enumerate() {
        kv.put(run_id, "data", Value::Int(i as i64)).unwrap();
    }

    // Each run sees only its own data
    for (i, run_id) in runs.iter().enumerate() {
        let val = kv.get(run_id, "data").unwrap().unwrap();
        assert_eq!(val.value, Value::Int(i as i64));
    }
}
