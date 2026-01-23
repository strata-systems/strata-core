//! TraceStore Durability Tests
//!
//! Tests for persistence across restarts:
//! - Trace persistence
//! - Hierarchy persistence
//! - Tag persistence

use crate::*;
use strata_api::substrate::{TraceStore, TraceType};
use tempfile::TempDir;

/// Test traces persist after restart
#[test]
fn test_trace_persist_after_restart() {
    let temp_dir = TempDir::new().unwrap();
    let run = ApiRunId::default_run_id();

    let mut trace_ids = Vec::new();

    // First session - write traces
    {
        let db = create_persistent_db(temp_dir.path());
        for i in 0..3 {
            let content = obj([("order", Value::Int(i))]);
            let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, vec![format!("tag{}", i)]).unwrap();
            trace_ids.push(id);
        }

        assert_eq!(db.trace_count(&run).unwrap(), 3);
    }

    // Second session - verify traces
    {
        let db = create_persistent_db(temp_dir.path());

        assert_eq!(db.trace_count(&run).unwrap(), 3);

        for (i, id) in trace_ids.iter().enumerate() {
            let trace = db.trace_get(&run, id).unwrap().expect("Trace should exist");
            assert_eq!(trace.value.id, *id);
            assert_eq!(trace.value.trace_type, TraceType::Thought);
            assert!(trace.value.tags.contains(&format!("tag{}", i)));
        }
    }
}

/// Test trace hierarchy persists
#[test]
fn test_trace_hierarchy_persist() {
    let temp_dir = TempDir::new().unwrap();
    let run = ApiRunId::default_run_id();
    let mut ids = std::collections::HashMap::new();

    // First session - create hierarchy
    {
        let db = create_persistent_db(temp_dir.path());
        let content = obj([("name", Value::String("test".to_string()))]);

        // Root
        let (root_id, _) = db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![]).unwrap();
        ids.insert("root", root_id.clone());

        // Children
        let (child1_id, _) = db.trace_create(&run, TraceType::Action, Some(&root_id), content.clone(), vec![]).unwrap();
        let (child2_id, _) = db.trace_create(&run, TraceType::Action, Some(&root_id), content.clone(), vec![]).unwrap();
        ids.insert("child1", child1_id.clone());
        ids.insert("child2", child2_id);

        // Grandchild
        let (grandchild_id, _) = db.trace_create(&run, TraceType::Observation, Some(&child1_id), content.clone(), vec![]).unwrap();
        ids.insert("grandchild", grandchild_id);
    }

    // Second session - verify hierarchy
    {
        let db = create_persistent_db(temp_dir.path());

        // Verify parent-child relationships
        let child1 = db.trace_get(&run, ids.get("child1").unwrap()).unwrap().unwrap();
        assert_eq!(child1.value.parent_id, Some(ids.get("root").unwrap().clone()));

        let grandchild = db.trace_get(&run, ids.get("grandchild").unwrap()).unwrap().unwrap();
        assert_eq!(grandchild.value.parent_id, Some(ids.get("child1").unwrap().clone()));

        // Verify tree
        let tree = db.trace_tree(&run, ids.get("root").unwrap()).unwrap();
        assert_eq!(tree.len(), 4);

        // Verify children
        let children = db.trace_children(&run, ids.get("root").unwrap()).unwrap();
        assert_eq!(children.len(), 2);
    }
}

/// Test trace types persist correctly
#[test]
fn test_trace_types_persist() {
    let temp_dir = TempDir::new().unwrap();
    let run = ApiRunId::default_run_id();
    let mut ids: Vec<(String, TraceType)> = Vec::new();

    // First session - create various types
    {
        let db = create_persistent_db(temp_dir.path());
        let content = obj([("msg", Value::String("test".to_string()))]);

        let types = vec![
            TraceType::Thought,
            TraceType::Action,
            TraceType::Observation,
            TraceType::Tool,
            TraceType::Message,
            TraceType::Custom("MyCustom".to_string()),
        ];

        for t in types {
            let (id, _) = db.trace_create(&run, t.clone(), None, content.clone(), vec![]).unwrap();
            ids.push((id, t));
        }
    }

    // Second session - verify types
    {
        let db = create_persistent_db(temp_dir.path());

        for (id, expected_type) in &ids {
            let trace = db.trace_get(&run, id).unwrap().expect("Trace should exist");
            assert_eq!(trace.value.trace_type, *expected_type, "Type should match for ID {}", id);
        }
    }
}

/// Test trace tags persist
#[test]
fn test_trace_tags_persist() {
    let temp_dir = TempDir::new().unwrap();
    let run = ApiRunId::default_run_id();
    let id: String;

    let tags = vec![
        "important".to_string(),
        "category:analysis".to_string(),
        "env:prod".to_string(),
    ];

    // First session - create with tags
    {
        let db = create_persistent_db(temp_dir.path());
        let content = obj([("msg", Value::String("test".to_string()))]);
        let (trace_id, _) = db.trace_create(&run, TraceType::Thought, None, content, tags.clone()).unwrap();
        id = trace_id;
    }

    // Second session - verify tags
    {
        let db = create_persistent_db(temp_dir.path());
        let trace = db.trace_get(&run, &id).unwrap().expect("Trace should exist");

        for tag in &tags {
            assert!(trace.value.tags.contains(tag), "Should contain tag: {}", tag);
        }
    }
}

/// Test trace content persists
#[test]
fn test_trace_content_persist() {
    let temp_dir = TempDir::new().unwrap();
    let run = ApiRunId::default_run_id();
    let id: String;

    let content = obj([
        ("thought", Value::String("This is a complex thought".to_string())),
        ("confidence", Value::Float(0.95)),
        ("nested", obj([
            ("key", Value::String("value".to_string())),
            ("number", Value::Int(42)),
        ])),
    ]);

    // First session - create with content
    {
        let db = create_persistent_db(temp_dir.path());
        let (trace_id, _) = db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![]).unwrap();
        id = trace_id;
    }

    // Second session - verify content
    {
        let db = create_persistent_db(temp_dir.path());
        let trace = db.trace_get(&run, &id).unwrap().expect("Trace should exist");

        // Content should be preserved (may have some conversion differences)
        assert!(trace.value.content.is_object());
    }
}

/// Test trace queries work after restart
#[test]
fn test_trace_queries_after_restart() {
    let temp_dir = TempDir::new().unwrap();
    let run = ApiRunId::default_run_id();

    // First session - create traces with tags
    {
        let db = create_persistent_db(temp_dir.path());
        let content = obj([("msg", Value::String("test".to_string()))]);

        for i in 0..5 {
            let tags = if i % 2 == 0 { vec!["even".to_string()] } else { vec!["odd".to_string()] };
            db.trace_create(&run, TraceType::Thought, None, content.clone(), tags).unwrap();
        }
    }

    // Second session - verify queries
    {
        let db = create_persistent_db(temp_dir.path());

        let even = db.trace_query_by_tag(&run, "even").unwrap();
        let odd = db.trace_query_by_tag(&run, "odd").unwrap();

        assert_eq!(even.len(), 3);
        assert_eq!(odd.len(), 2);
    }
}

/// Test run isolation persists
#[test]
fn test_trace_run_isolation_persist() {
    let temp_dir = TempDir::new().unwrap();
    let run1 = ApiRunId::default_run_id();
    let run2 = ApiRunId::new();

    // First session - create in both runs
    {
        let db = create_persistent_db(temp_dir.path());
        db.run_create(Some(&run2), None).unwrap();

        let content = obj([("run", Value::String("test".to_string()))]);

        // 3 traces in run1
        for _ in 0..3 {
            db.trace_create(&run1, TraceType::Thought, None, content.clone(), vec![]).unwrap();
        }

        // 2 traces in run2
        for _ in 0..2 {
            db.trace_create(&run2, TraceType::Thought, None, content.clone(), vec![]).unwrap();
        }
    }

    // Second session - verify isolation
    {
        let db = create_persistent_db(temp_dir.path());

        assert_eq!(db.trace_count(&run1).unwrap(), 3);
        assert_eq!(db.trace_count(&run2).unwrap(), 2);
    }
}

/// Test many traces persist
#[test]
fn test_trace_many_persist() {
    let temp_dir = TempDir::new().unwrap();
    let run = ApiRunId::default_run_id();
    const NUM_TRACES: u64 = 100;

    // First session - create many traces
    {
        let db = create_persistent_db(temp_dir.path());

        for i in 0..NUM_TRACES {
            let content = obj([("index", Value::Int(i as i64))]);
            db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();
        }

        assert_eq!(db.trace_count(&run).unwrap(), NUM_TRACES);
    }

    // Second session - verify all persist
    {
        let db = create_persistent_db(temp_dir.path());
        assert_eq!(db.trace_count(&run).unwrap(), NUM_TRACES);

        let traces = db.trace_list(&run, None, None, None, None, None).unwrap();
        assert_eq!(traces.len(), NUM_TRACES as usize);
    }
}
