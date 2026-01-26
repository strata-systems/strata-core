//! Parity tests: verify executor produces same results as direct substrate calls.
//!
//! These tests ensure the Executor layer is a faithful proxy to the underlying
//! substrate, with no unexpected transformations or data loss.

use crate::types::*;
use crate::{Command, Executor, Output};
use strata_api::substrate::{EventLog, JsonStore, KVStore, RunIndex, StateCell, VectorStore};
use strata_api::substrate::{ApiRunId, SubstrateImpl};
use strata_api::DistanceMetric as ApiDistanceMetric;
use strata_core::Value;
use strata_engine::Database;
use std::sync::Arc;

/// Create a test executor with a shared substrate for parity comparisons.
fn create_test_environment() -> (Executor, Arc<SubstrateImpl>) {
    let db = Arc::new(Database::builder().no_durability().open_temp().unwrap());
    let substrate = Arc::new(SubstrateImpl::new(db));
    let executor = Executor::new(substrate.clone());
    (executor, substrate)
}

// =============================================================================
// KV Parity Tests
// =============================================================================

#[test]
fn test_kv_put_get_parity() {
    let (executor, substrate) = create_test_environment();
    let run_id = ApiRunId::default();

    // Direct substrate call to write key1
    let _direct_version = substrate.kv_put(&run_id, "key1", Value::String("direct".into())).unwrap();

    // Executor call to write key2
    let exec_result = executor.execute(Command::KvPut {
        run: RunId::from("default"),
        key: "key2".to_string(),
        value: Value::String("executor".into()),
    });

    // Both should succeed with a Version output
    match exec_result {
        Ok(Output::Version(v)) => {
            // Version should be > 0
            assert!(v > 0, "Write should return a positive version");
        }
        _ => panic!("Expected Version output"),
    }

    // Now verify we can read back what was written via both methods
    let direct_value = substrate.kv_get(&run_id, "key1").unwrap();
    let exec_get = executor.execute(Command::KvGet {
        run: RunId::from("default"),
        key: "key2".to_string(),
    });

    assert_eq!(direct_value.unwrap().value, Value::String("direct".into()));
    match exec_get {
        Ok(Output::MaybeVersioned(Some(v))) => {
            assert_eq!(v.value, Value::String("executor".into()));
        }
        _ => panic!("Expected MaybeVersioned output"),
    }

    // Cross-check: executor can read substrate write and vice versa
    let cross_read_exec = executor.execute(Command::KvGet {
        run: RunId::from("default"),
        key: "key1".to_string(),
    });
    match cross_read_exec {
        Ok(Output::MaybeVersioned(Some(v))) => {
            assert_eq!(v.value, Value::String("direct".into()));
        }
        _ => panic!("Cross-read failed"),
    }

    let cross_read_sub = substrate.kv_get(&run_id, "key2").unwrap();
    assert_eq!(cross_read_sub.unwrap().value, Value::String("executor".into()));
}

#[test]
fn test_kv_delete_parity() {
    let (executor, substrate) = create_test_environment();
    let run_id = ApiRunId::default();

    // Set up data via substrate
    substrate.kv_put(&run_id, "to-delete", Value::Int(42)).unwrap();

    // Delete via executor
    let result = executor.execute(Command::KvDelete {
        run: RunId::from("default"),
        key: "to-delete".to_string(),
    });

    // Should succeed
    assert!(result.is_ok());

    // Verify deleted via direct substrate call
    let check = substrate.kv_get(&run_id, "to-delete").unwrap();
    assert!(check.is_none(), "Key should be deleted");
}

#[test]
fn test_kv_exists_parity() {
    let (executor, substrate) = create_test_environment();
    let run_id = ApiRunId::default();

    // Create via substrate
    substrate.kv_put(&run_id, "exists-key", Value::Int(1)).unwrap();

    // Check via executor
    let result = executor.execute(Command::KvExists {
        run: RunId::from("default"),
        key: "exists-key".to_string(),
    });

    match result {
        Ok(Output::Bool(exists)) => assert!(exists),
        _ => panic!("Expected Bool output"),
    }

    // Check non-existent key
    let result2 = executor.execute(Command::KvExists {
        run: RunId::from("default"),
        key: "nonexistent".to_string(),
    });

    match result2 {
        Ok(Output::Bool(exists)) => assert!(!exists),
        _ => panic!("Expected Bool output"),
    }
}

#[test]
fn test_kv_incr_parity() {
    let (executor, substrate) = create_test_environment();
    let run_id = ApiRunId::default();

    // Initialize counter via substrate
    substrate.kv_put(&run_id, "counter", Value::Int(10)).unwrap();

    // Increment via executor
    let result = executor.execute(Command::KvIncr {
        run: RunId::from("default"),
        key: "counter".to_string(),
        delta: 5,
    });

    match result {
        Ok(Output::Int(val)) => assert_eq!(val, 15),
        _ => panic!("Expected Int output"),
    }

    // Verify via direct read
    let check = substrate.kv_get(&run_id, "counter").unwrap().unwrap();
    assert_eq!(check.value, Value::Int(15));
}

// =============================================================================
// JSON Parity Tests
// =============================================================================

#[test]
fn test_json_set_get_parity() {
    let (executor, substrate) = create_test_environment();
    let run_id = ApiRunId::default();

    // Set via executor - use root path (empty string means root)
    let result = executor.execute(Command::JsonSet {
        run: RunId::from("default"),
        key: "doc1".to_string(),
        path: "".to_string(),  // Root path
        value: Value::Object(
            [("name".to_string(), Value::String("Alice".into()))]
                .into_iter()
                .collect(),
        ),
    });

    // JsonSet returns Version
    match result {
        Ok(Output::Version(v)) => assert!(v > 0),
        other => panic!("Expected Version output, got {:?}", other),
    }

    // Get via direct substrate - use empty path for root
    let direct_get = substrate.json_get(&run_id, "doc1", "").unwrap();
    assert!(direct_get.is_some());

    // Get via executor - JsonGet returns MaybeVersioned
    // Use ".name" path format (no $ prefix)
    let exec_get = executor.execute(Command::JsonGet {
        run: RunId::from("default"),
        key: "doc1".to_string(),
        path: ".name".to_string(),
    });

    match exec_get {
        Ok(Output::MaybeVersioned(Some(v))) => {
            assert_eq!(v.value, Value::String("Alice".into()));
        }
        other => panic!("Expected MaybeVersioned output, got {:?}", other),
    }
}

// =============================================================================
// Event Parity Tests
// =============================================================================

#[test]
fn test_event_append_range_parity() {
    let (executor, substrate) = create_test_environment();
    let run_id = ApiRunId::default();

    // Append via executor - EventAppend returns Version
    let result1 = executor.execute(Command::EventAppend {
        run: RunId::from("default"),
        stream: "events".to_string(),
        payload: Value::Object(
            [("type".to_string(), Value::String("click".into()))]
                .into_iter()
                .collect(),
        ),
    });

    // Just verify it returns a Version (sequence numbers may start from 0)
    match result1 {
        Ok(Output::Version(_seq)) => {}
        other => panic!("Expected Version output, got {:?}", other),
    }

    // Append via direct substrate
    let seq2 = substrate
        .event_append(
            &run_id,
            "events",
            Value::Object(
                [("type".to_string(), Value::String("scroll".into()))]
                    .into_iter()
                    .collect(),
            ),
        )
        .unwrap();

    match seq2 {
        strata_core::Version::Sequence(n) => assert!(n > 0),
        _ => panic!("Expected Sequence version"),
    }

    // Range query via executor
    let range_result = executor.execute(Command::EventRange {
        run: RunId::from("default"),
        stream: "events".to_string(),
        start: None,
        end: None,
        limit: None,
    });

    match range_result {
        Ok(Output::VersionedValues(events)) => {
            assert_eq!(events.len(), 2);
        }
        other => panic!("Expected VersionedValues output, got {:?}", other),
    }
}

// =============================================================================
// State Parity Tests
// =============================================================================

#[test]
fn test_state_set_get_parity() {
    let (executor, substrate) = create_test_environment();
    let run_id = ApiRunId::default();

    // Set via executor
    let result = executor.execute(Command::StateSet {
        run: RunId::from("default"),
        cell: "cell1".to_string(),
        value: Value::Int(100),
    });

    let counter1 = match result {
        Ok(Output::Version(c)) => c,
        _ => panic!("Expected Version output"),
    };

    // Get via direct substrate
    let direct_get = substrate.state_get(&run_id, "cell1").unwrap();
    assert!(direct_get.is_some());
    assert_eq!(direct_get.unwrap().value, Value::Int(100));

    // Set via direct substrate
    let counter2 = substrate.state_set(&run_id, "cell2", Value::Int(200)).unwrap();

    // Both should have counter 1 (first write to each cell)
    assert_eq!(counter1, 1);
    match counter2 {
        strata_core::Version::Counter(n) => assert_eq!(n, 1),
        _ => panic!("Expected Counter version"),
    }

    // Get cell2 via executor
    let exec_get = executor.execute(Command::StateGet {
        run: RunId::from("default"),
        cell: "cell2".to_string(),
    });

    match exec_get {
        Ok(Output::MaybeVersioned(Some(state))) => {
            assert_eq!(state.value, Value::Int(200));
        }
        _ => panic!("Expected MaybeVersioned output"),
    }
}

// =============================================================================
// Vector Parity Tests
// =============================================================================

#[test]
fn test_vector_create_collection_parity() {
    let (executor, substrate) = create_test_environment();
    let run_id = ApiRunId::default();

    // Create collection via executor
    let result = executor.execute(Command::VectorCreateCollection {
        run: RunId::from("default"),
        collection: "embeddings".to_string(),
        dimension: 4,
        metric: DistanceMetric::Cosine,
    });

    assert!(result.is_ok());

    // Verify via direct substrate
    let info = substrate
        .vector_collection_info(&run_id, "embeddings")
        .unwrap();

    assert!(info.is_some());
    let info = info.unwrap();
    assert_eq!(info.dimension, 4);
}

#[test]
fn test_vector_upsert_search_parity() {
    let (executor, substrate) = create_test_environment();
    let run_id = ApiRunId::default();

    // Create collection first
    substrate
        .vector_create_collection(
            &run_id,
            "vecs",
            4,
            ApiDistanceMetric::Cosine,
        )
        .unwrap();

    // Upsert via executor
    executor
        .execute(Command::VectorUpsert {
            run: RunId::from("default"),
            collection: "vecs".to_string(),
            key: "v1".to_string(),
            vector: vec![1.0, 0.0, 0.0, 0.0],
            metadata: None,
        })
        .unwrap();

    // Upsert via direct substrate
    substrate
        .vector_upsert(
            &run_id,
            "vecs",
            "v2",
            &[0.0, 1.0, 0.0, 0.0],
            None,
        )
        .unwrap();

    // Search via executor
    let search_result = executor.execute(Command::VectorSearch {
        run: RunId::from("default"),
        collection: "vecs".to_string(),
        query: vec![1.0, 0.0, 0.0, 0.0],
        k: 10,
        filter: None,
        metric: None,
    });

    match search_result {
        Ok(Output::VectorMatches(matches)) => {
            assert_eq!(matches.len(), 2);
            // v1 should be the closest match (exact match)
            assert_eq!(matches[0].key, "v1");
        }
        _ => panic!("Expected VectorMatches output"),
    }
}

// =============================================================================
// Run Parity Tests
// =============================================================================

#[test]
fn test_run_create_get_parity() {
    let (executor, substrate) = create_test_environment();

    // Create run via executor with a UUID
    let result = executor.execute(Command::RunCreate {
        run_id: Some("550e8400-e29b-41d4-a716-446655440001".to_string()),
        metadata: Some(Value::Object(
            [("name".to_string(), Value::String("Test".into()))]
                .into_iter()
                .collect(),
        )),
    });

    match result {
        Ok(Output::RunWithVersion { info, .. }) => {
            assert_eq!(info.id.as_str(), "550e8400-e29b-41d4-a716-446655440001");
        }
        other => panic!("Expected RunWithVersion output, got {:?}", other),
    }

    // Create via direct substrate
    let run_id_2 = ApiRunId::parse("550e8400-e29b-41d4-a716-446655440002").unwrap();
    let (direct_run, _version) = substrate
        .run_create(Some(&run_id_2), None)
        .unwrap();

    assert_eq!(direct_run.run_id.to_string(), "550e8400-e29b-41d4-a716-446655440002");

    // List runs via executor
    let list_result = executor.execute(Command::RunList {
        state: None,
        limit: None,
        offset: None,
    });

    match list_result {
        Ok(Output::RunInfoList(runs)) => {
            // Should have at least 2 runs (the 2 we created)
            // Note: default run may or may not be listed
            assert!(runs.len() >= 2, "Expected at least 2 runs, got {}", runs.len());
        }
        other => panic!("Expected RunInfoList output, got {:?}", other),
    }
}

// =============================================================================
// Database Parity Tests
// =============================================================================

#[test]
fn test_ping_parity() {
    let (executor, _substrate) = create_test_environment();

    let result = executor.execute(Command::Ping);

    match result {
        Ok(Output::Pong { version }) => {
            assert!(!version.is_empty());
        }
        _ => panic!("Expected Pong output"),
    }
}

#[test]
fn test_info_parity() {
    let (executor, _substrate) = create_test_environment();

    let result = executor.execute(Command::Info);

    match result {
        Ok(Output::DatabaseInfo(info)) => {
            assert!(!info.version.is_empty());
        }
        _ => panic!("Expected DatabaseInfo output"),
    }
}

#[test]
fn test_flush_compact_parity() {
    let (executor, _substrate) = create_test_environment();

    // These should not error
    let flush_result = executor.execute(Command::Flush);
    assert!(flush_result.is_ok());

    let compact_result = executor.execute(Command::Compact);
    assert!(compact_result.is_ok());
}

// =============================================================================
// Cross-category Integration Tests
// =============================================================================

#[test]
fn test_run_isolation_parity() {
    let (executor, _substrate) = create_test_environment();

    // Create two runs with valid UUIDs
    executor
        .execute(Command::RunCreate {
            run_id: Some("550e8400-e29b-41d4-a716-446655440003".to_string()),
            metadata: None,
        })
        .unwrap();

    executor
        .execute(Command::RunCreate {
            run_id: Some("550e8400-e29b-41d4-a716-446655440004".to_string()),
            metadata: None,
        })
        .unwrap();

    // Write to run-a
    executor
        .execute(Command::KvPut {
            run: RunId::from("550e8400-e29b-41d4-a716-446655440003"),
            key: "shared-key".to_string(),
            value: Value::String("from-a".into()),
        })
        .unwrap();

    // Write to run-b
    executor
        .execute(Command::KvPut {
            run: RunId::from("550e8400-e29b-41d4-a716-446655440004"),
            key: "shared-key".to_string(),
            value: Value::String("from-b".into()),
        })
        .unwrap();

    // Read from run-a
    let result_a = executor.execute(Command::KvGet {
        run: RunId::from("550e8400-e29b-41d4-a716-446655440003"),
        key: "shared-key".to_string(),
    });

    match result_a {
        Ok(Output::MaybeVersioned(Some(v))) => {
            assert_eq!(v.value, Value::String("from-a".into()));
        }
        _ => panic!("Expected value from run-a"),
    }

    // Read from run-b
    let result_b = executor.execute(Command::KvGet {
        run: RunId::from("550e8400-e29b-41d4-a716-446655440004"),
        key: "shared-key".to_string(),
    });

    match result_b {
        Ok(Output::MaybeVersioned(Some(v))) => {
            assert_eq!(v.value, Value::String("from-b".into()));
        }
        _ => panic!("Expected value from run-b"),
    }
}
