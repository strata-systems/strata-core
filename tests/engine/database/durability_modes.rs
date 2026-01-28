//! Durability Mode Tests
//!
//! Tests that operations produce the same results across all durability modes.
//! Only persistence behavior should differ.

use crate::common::*;
use strata_core::primitives::json::JsonPath;
use strata_engine::KVStoreExt;
use std::collections::HashMap;

/// Helper to create an event payload object
fn event_payload(data: Value) -> Value {
    Value::Object(HashMap::from([
        ("data".to_string(), data),
    ]))
}

// ============================================================================
// Mode Equivalence
// ============================================================================

#[test]
fn kv_put_get_same_across_modes() {
    test_across_modes("kv_put_get", |db| {
        let run_id = RunId::new();
        let kv = KVStore::new(db);

        kv.put(&run_id, "key", Value::Int(42)).unwrap();
        let result = kv.get(&run_id, "key").unwrap();

        result.map(|v| v.value)
    });
}

#[test]
fn kv_delete_same_across_modes() {
    test_across_modes("kv_delete", |db| {
        let run_id = RunId::new();
        let kv = KVStore::new(db);

        kv.put(&run_id, "key", Value::Int(1)).unwrap();
        let deleted = kv.delete(&run_id, "key").unwrap();

        (deleted, kv.get(&run_id, "key").unwrap().is_none())
    });
}

#[test]
fn eventlog_append_same_across_modes() {
    test_across_modes("eventlog_append", |db| {
        let run_id = RunId::new();
        let event = EventLog::new(db);

        event.append(&run_id, "test_type", event_payload(Value::String("payload".into()))).unwrap();
        let len = event.len(&run_id).unwrap();
        let head = event.head(&run_id).unwrap();

        (len, head.map(|e| e.value.event_type.clone()))
    });
}

#[test]
fn statecell_cas_same_across_modes() {
    test_across_modes("statecell_cas", |db| {
        let run_id = RunId::new();
        let state = StateCell::new(db);

        state.init(&run_id, "cell", Value::Int(1)).unwrap();
        let read = state.read(&run_id, "cell").unwrap();
        let version = read.as_ref().map(|v| v.version).unwrap_or(Version::from(0u64));

        let cas_result = state.cas(&run_id, "cell", version, Value::Int(2));

        (cas_result.is_ok(), state.read(&run_id, "cell").unwrap().map(|v| v.value.value.clone()))
    });
}

#[test]
fn json_create_get_same_across_modes() {
    test_across_modes("json_create_get", |db| {
        let run_id = RunId::new();
        let json = JsonStore::new(db);

        let doc_value = serde_json::json!({"name": "test", "count": 42});
        json.create(&run_id, "doc1", doc_value.clone().into()).unwrap();

        let result = json.get(&run_id, "doc1", &JsonPath::root()).unwrap();

        // Return serialized JSON for comparison
        result.map(|v| serde_json::to_string(&v.value).unwrap_or_default())
    });
}

// ============================================================================
// Mode-Specific Behavior
// ============================================================================

#[test]
fn ephemeral_mode_is_ephemeral() {
    // Database::ephemeral() creates a truly in-memory database with no files
    let db = Database::ephemeral().expect("ephemeral database");
    assert!(db.is_ephemeral());
}

#[test]
fn no_durability_temp_is_not_ephemeral() {
    // no_durability().open_temp() still creates temp files on disk
    let db = create_test_db();
    assert!(!db.is_ephemeral()); // Not truly ephemeral - uses temp files
}

#[test]
fn buffered_mode_is_persistent() {
    let db = Database::builder()
        .buffered()
        .open_temp()
        .expect("buffered database");

    // Buffered mode is NOT ephemeral (has durability)
    // But with open_temp() it uses temp dir
    assert!(!db.is_ephemeral());
}

#[test]
fn strict_mode_is_persistent() {
    let db = Database::builder()
        .strict()
        .open_temp()
        .expect("strict database");

    assert!(!db.is_ephemeral());
}

// ============================================================================
// Cross-Mode Transaction Semantics
// ============================================================================

#[test]
fn transaction_atomicity_in_memory() {
    let test_db = TestDb::new_in_memory();
    let run_id = test_db.run_id;

    // Atomic transaction using extension trait
    test_db.db.transaction(run_id, |txn| {
        txn.kv_put("a", Value::Int(1))?;
        txn.kv_put("b", Value::Int(2))?;
        Ok(())
    }).unwrap();

    let kv = test_db.kv();
    assert!(kv.get(&run_id, "a").unwrap().is_some());
    assert!(kv.get(&run_id, "b").unwrap().is_some());
}

#[test]
fn transaction_atomicity_buffered() {
    let test_db = TestDb::new(); // TestDb::new() uses temp dir with durability
    let run_id = test_db.run_id;

    test_db.db.transaction(run_id, |txn| {
        txn.kv_put("a", Value::Int(1))?;
        txn.kv_put("b", Value::Int(2))?;
        Ok(())
    }).unwrap();

    let kv = test_db.kv();
    assert!(kv.get(&run_id, "a").unwrap().is_some());
    assert!(kv.get(&run_id, "b").unwrap().is_some());
}

#[test]
fn transaction_atomicity_strict() {
    let db = std::sync::Arc::new(Database::builder()
        .strict()
        .open_temp()
        .expect("strict database"));
    let run_id = RunId::new();

    db.transaction(run_id, |txn| {
        txn.kv_put("a", Value::Int(1))?;
        txn.kv_put("b", Value::Int(2))?;
        Ok(())
    }).unwrap();

    let kv = KVStore::new(db);
    assert!(kv.get(&run_id, "a").unwrap().is_some());
    assert!(kv.get(&run_id, "b").unwrap().is_some());
}

// ============================================================================
// Multi-Primitive Consistency
// ============================================================================

#[test]
fn all_primitives_work_in_all_modes() {
    test_across_modes("all_primitives", |db| {
        let run_id = RunId::new();

        let kv = KVStore::new(db.clone());
        let event = EventLog::new(db.clone());
        let state = StateCell::new(db.clone());
        let json = JsonStore::new(db.clone());
        let run_idx = RunIndex::new(db.clone());

        // KV
        kv.put(&run_id, "k", Value::Int(1)).unwrap();

        // Event
        event.append(&run_id, "e", event_payload(Value::Int(2))).unwrap();

        // State
        state.init(&run_id, "s", Value::Int(3)).unwrap();

        // JSON
        json.create(&run_id, "j", serde_json::json!({"x": 4}).into()).unwrap();

        // RunIndex
        run_idx.create_run("test_run").unwrap();

        // All succeeded
        true
    });
}
