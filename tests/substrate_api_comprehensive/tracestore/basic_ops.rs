//! TraceStore Basic Operations Tests
//!
//! Tests for core CRUD operations:
//! - trace_create
//! - trace_get
//! - trace_count

use crate::*;
use strata_api::substrate::{TraceStore, TraceType};

/// Test create trace returns ID and version
#[test]
fn test_trace_create_returns_id_and_version() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("message", Value::String("test thought".to_string()))]);

        let (id, version) = db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();

        assert!(!id.is_empty(), "Trace ID should not be empty");
        assert!(matches!(version, Version::Txn(_) | Version::Counter(_)));
    });
}

/// Test create and get trace
#[test]
fn test_trace_create_and_get() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([
            ("message", Value::String("A thought".to_string())),
            ("importance", Value::Int(5)),
        ]);
        let tags = vec!["tag1".to_string(), "tag2".to_string()];

        let (id, _version) = db.trace_create(&run, TraceType::Thought, None, content.clone(), tags.clone()).unwrap();

        let result = db.trace_get(&run, &id).unwrap().unwrap();
        assert_eq!(result.value.id, id);
        assert_eq!(result.value.trace_type, TraceType::Thought);
        assert!(result.value.parent_id.is_none());
        assert_eq!(result.value.tags, tags);
    });
}

/// Test get nonexistent trace
#[test]
fn test_trace_get_nonexistent() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let result = db.trace_get(&run, "nonexistent-id").unwrap();
        assert!(result.is_none());
    });
}

/// Test trace count
#[test]
fn test_trace_count() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Initially zero
        assert_eq!(db.trace_count(&run).unwrap(), 0);

        // Add traces
        let content = obj([("msg", Value::String("test".to_string()))]);
        for i in 0..5 {
            db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![format!("tag{}", i)]).unwrap();
        }

        assert_eq!(db.trace_count(&run).unwrap(), 5);
    });
}

/// Test different trace types
#[test]
fn test_trace_types() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("data", Value::String("test".to_string()))]);

        let types = vec![
            TraceType::Thought,
            TraceType::Action,
            TraceType::Observation,
            TraceType::Tool,
            TraceType::Message,
            TraceType::Custom("MyCustomType".to_string()),
        ];

        for trace_type in types {
            let (id, _) = db.trace_create(&run, trace_type.clone(), None, content.clone(), vec![]).unwrap();
            let result = db.trace_get(&run, &id).unwrap().unwrap();
            assert_eq!(result.value.trace_type, trace_type);
        }
    });
}

/// Test trace with tool content
#[test]
fn test_trace_tool_type() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([
            ("tool_name", Value::String("search".to_string())),
            ("arguments", obj([("query", Value::String("test query".to_string()))])),
        ]);

        let (id, _) = db.trace_create(&run, TraceType::Tool, None, content, vec![]).unwrap();
        let result = db.trace_get(&run, &id).unwrap().unwrap();
        assert_eq!(result.value.trace_type, TraceType::Tool);
    });
}

/// Test trace created_at timestamp
#[test]
fn test_trace_timestamp() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        let before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();

        let after = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        let result = db.trace_get(&run, &id).unwrap().unwrap();
        // Allow some tolerance (within 1 second)
        assert!(result.value.created_at >= before.saturating_sub(1_000_000));
        assert!(result.value.created_at <= after.saturating_add(1_000_000));
    });
}

/// Test trace with empty tags
#[test]
fn test_trace_empty_tags() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();
        let result = db.trace_get(&run, &id).unwrap().unwrap();
        assert!(result.value.tags.is_empty());
    });
}

/// Test trace with multiple tags
#[test]
fn test_trace_multiple_tags() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);
        let tags = vec!["important".to_string(), "debug".to_string(), "category:analysis".to_string()];

        let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, tags.clone()).unwrap();
        let result = db.trace_get(&run, &id).unwrap().unwrap();

        for tag in &tags {
            assert!(result.value.tags.contains(tag));
        }
    });
}

/// Test unique trace IDs
#[test]
fn test_trace_unique_ids() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);
        let mut ids = std::collections::HashSet::new();

        for _ in 0..50 {
            let (id, _) = db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![]).unwrap();
            assert!(ids.insert(id), "Trace IDs should be unique");
        }
    });
}

/// Test trace run isolation
#[test]
fn test_trace_run_isolation() {
    test_across_substrate_modes(|db| {
        let run1 = ApiRunId::default_run_id();
        let run2 = ApiRunId::new();
        db.run_create(Some(&run2), None).unwrap();

        let content = obj([("msg", Value::String("test".to_string()))]);

        // Create traces in run1
        let (id1, _) = db.trace_create(&run1, TraceType::Thought, None, content.clone(), vec![]).unwrap();
        let (id2, _) = db.trace_create(&run1, TraceType::Action, None, content.clone(), vec![]).unwrap();

        // Create trace in run2
        let (id3, _) = db.trace_create(&run2, TraceType::Observation, None, content.clone(), vec![]).unwrap();

        // Verify isolation
        assert_eq!(db.trace_count(&run1).unwrap(), 2);
        assert_eq!(db.trace_count(&run2).unwrap(), 1);

        // Get from own run works
        assert!(db.trace_get(&run1, &id1).unwrap().is_some());
        assert!(db.trace_get(&run1, &id2).unwrap().is_some());
        assert!(db.trace_get(&run2, &id3).unwrap().is_some());

        // Get from other run fails
        assert!(db.trace_get(&run2, &id1).unwrap().is_none());
        assert!(db.trace_get(&run1, &id3).unwrap().is_none());
    });
}

/// Test trace_create_with_id is not supported
#[test]
fn test_trace_create_with_id_not_supported() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        // This API is documented as not supported (returns error)
        let result = db.trace_create_with_id(&run, "my-custom-id", TraceType::Thought, None, content, vec![]);
        assert!(result.is_err(), "trace_create_with_id should fail (not implemented)");
    });
}

/// Test trace_update_tags is not supported
#[test]
fn test_trace_update_tags_not_supported() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, vec!["initial".to_string()]).unwrap();

        // This API is documented as not supported (append-only traces)
        let result = db.trace_update_tags(&run, &id, vec!["new_tag".to_string()], vec![]);
        assert!(result.is_err(), "trace_update_tags should fail (traces are append-only)");
    });
}

/// Test trace list basic
#[test]
fn test_trace_list_basic() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        // Create some traces
        for _ in 0..5 {
            db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![]).unwrap();
        }

        let traces = db.trace_list(&run, None, None, None, None, None).unwrap();
        assert_eq!(traces.len(), 5);
    });
}

/// Test trace list with limit
#[test]
fn test_trace_list_limit() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        for _ in 0..10 {
            db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![]).unwrap();
        }

        let traces = db.trace_list(&run, None, None, None, Some(5), None).unwrap();
        assert_eq!(traces.len(), 5);
    });
}

/// Test trace list by type
#[test]
fn test_trace_list_by_type() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        // Create mixed traces
        for _ in 0..3 {
            db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![]).unwrap();
        }
        for _ in 0..2 {
            db.trace_create(&run, TraceType::Action, None, content.clone(), vec![]).unwrap();
        }

        // Filter by type
        let thoughts = db.trace_list(&run, Some(TraceType::Thought), None, None, None, None).unwrap();
        let actions = db.trace_list(&run, Some(TraceType::Action), None, None, None, None).unwrap();

        assert_eq!(thoughts.len(), 3);
        assert_eq!(actions.len(), 2);
    });
}

/// Test trace list returns newest first
#[test]
fn test_trace_list_order() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Create traces with small delay
        let mut ids = Vec::new();
        for i in 0..3 {
            let content = obj([("order", Value::Int(i))]);
            let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();
            ids.push(id);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let traces = db.trace_list(&run, None, None, None, None, None).unwrap();

        // Newest first - so the last created should be first in the list
        assert_eq!(traces[0].value.id, ids[2]);
        assert_eq!(traces[1].value.id, ids[1]);
        assert_eq!(traces[2].value.id, ids[0]);
    });
}
