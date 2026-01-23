//! TraceStore Edge Cases Tests
//!
//! Tests for validation and boundary conditions:
//! - Content validation
//! - Tag validation
//! - Unicode handling
//! - Large data

use crate::*;
use strata_api::substrate::{TraceStore, TraceType};

/// Test trace with empty content
#[test]
fn test_trace_empty_content() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([]);

        // Empty object content should work
        let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();
        let trace = db.trace_get(&run, &id).unwrap().unwrap();
        assert!(trace.value.content.is_object());
    });
}

/// Test trace with nested content
#[test]
fn test_trace_nested_content() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([
            ("level1", obj([
                ("level2", obj([
                    ("level3", obj([
                        ("value", Value::Int(42)),
                    ])),
                ])),
            ])),
        ]);

        let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();
        let trace = db.trace_get(&run, &id).unwrap().unwrap();
        assert!(trace.value.content.is_object());
    });
}

/// Test trace with array in content
#[test]
fn test_trace_array_content() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([
            ("items", Value::Array(vec![
                Value::Int(1),
                Value::Int(2),
                Value::Int(3),
            ])),
            ("strings", Value::Array(vec![
                Value::String("a".to_string()),
                Value::String("b".to_string()),
            ])),
        ]);

        let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();
        let trace = db.trace_get(&run, &id).unwrap().unwrap();
        assert!(trace.value.content.is_object());
    });
}

/// Test trace with all value types in content
#[test]
fn test_trace_all_value_types() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([
            ("null", Value::Null),
            ("bool_true", Value::Bool(true)),
            ("bool_false", Value::Bool(false)),
            ("int", Value::Int(42)),
            ("float", Value::Float(3.14159)),
            ("string", Value::String("hello".to_string())),
            ("array", Value::Array(vec![Value::Int(1)])),
            ("object", obj([("nested", Value::Int(1))])),
        ]);

        let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();
        assert!(db.trace_get(&run, &id).unwrap().is_some());
    });
}

/// Test trace with unicode in content
#[test]
fn test_trace_unicode_content() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([
            ("chinese", Value::String("中文测试".to_string())),
            ("japanese", Value::String("日本語テスト".to_string())),
            ("emoji", Value::String("🤖 AI thinking 🧠".to_string())),
            ("arabic", Value::String("اختبار عربي".to_string())),
            ("mixed", Value::String("Hello 世界 🌍".to_string())),
        ]);

        let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();
        assert!(db.trace_get(&run, &id).unwrap().is_some());
    });
}

/// Test trace with unicode in tags
#[test]
fn test_trace_unicode_tags() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);
        let tags = vec![
            "标签".to_string(),
            "タグ".to_string(),
            "🏷️".to_string(),
        ];

        let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, tags.clone()).unwrap();
        let trace = db.trace_get(&run, &id).unwrap().unwrap();

        for tag in &tags {
            assert!(trace.value.tags.contains(tag));
        }
    });
}

/// Test trace with special characters in tags
#[test]
fn test_trace_special_tags() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);
        let tags = vec![
            "category:sub-category".to_string(),
            "env:prod".to_string(),
            "version:1.0.0".to_string(),
            "user_id:123".to_string(),
            "path/to/item".to_string(),
        ];

        let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, tags.clone()).unwrap();
        let trace = db.trace_get(&run, &id).unwrap().unwrap();

        for tag in &tags {
            assert!(trace.value.tags.contains(tag));
        }
    });
}

/// Test trace with many tags
#[test]
fn test_trace_many_tags() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);
        let tags: Vec<String> = (0..50).map(|i| format!("tag_{}", i)).collect();

        let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, tags.clone()).unwrap();
        let trace = db.trace_get(&run, &id).unwrap().unwrap();

        assert_eq!(trace.value.tags.len(), 50);
    });
}

/// Test trace with duplicate tags
#[test]
fn test_trace_duplicate_tags() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);
        let tags = vec![
            "same".to_string(),
            "same".to_string(),
            "different".to_string(),
        ];

        let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, tags).unwrap();
        let trace = db.trace_get(&run, &id).unwrap().unwrap();

        // Implementation may dedupe or keep duplicates
        assert!(trace.value.tags.contains(&"same".to_string()));
        assert!(trace.value.tags.contains(&"different".to_string()));
    });
}

/// Test custom trace type names
#[test]
fn test_trace_custom_type_names() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        let type_names = vec![
            "Analysis",
            "Summary",
            "Decision",
            "Evaluation",
            "custom_type_with_underscores",
            "CustomType123",
        ];

        for name in type_names {
            let (id, _) = db.trace_create(&run, TraceType::Custom(name.to_string()), None, content.clone(), vec![]).unwrap();
            let trace = db.trace_get(&run, &id).unwrap().unwrap();
            assert_eq!(trace.value.trace_type, TraceType::Custom(name.to_string()));
        }
    });
}

/// Test large content
#[test]
fn test_trace_large_content() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Create large content (about 100KB)
        let large_string = "x".repeat(100_000);
        let content = obj([
            ("large_text", Value::String(large_string)),
        ]);

        let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();
        assert!(db.trace_get(&run, &id).unwrap().is_some());
    });
}

/// Test many fields in content
#[test]
fn test_trace_many_fields() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Create content with many fields
        let fields: std::collections::HashMap<String, Value> = (0..100)
            .map(|i| (format!("field_{}", i), Value::Int(i)))
            .collect();
        let content = Value::Object(fields);

        let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();
        assert!(db.trace_get(&run, &id).unwrap().is_some());
    });
}

/// Test float special values in content
#[test]
fn test_trace_float_special_values() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Normal floats
        let content = obj([
            ("positive", Value::Float(1.5)),
            ("negative", Value::Float(-1.5)),
            ("small", Value::Float(1e-10)),
            ("large", Value::Float(1e10)),
            ("zero", Value::Float(0.0)),
        ]);

        let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();
        assert!(db.trace_get(&run, &id).unwrap().is_some());
    });
}

/// Test integer boundary values in content
#[test]
fn test_trace_integer_boundaries() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([
            ("max", Value::Int(i64::MAX)),
            ("min", Value::Int(i64::MIN)),
            ("zero", Value::Int(0)),
            ("positive", Value::Int(42)),
            ("negative", Value::Int(-42)),
        ]);

        let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();
        assert!(db.trace_get(&run, &id).unwrap().is_some());
    });
}

/// Test empty string values
#[test]
fn test_trace_empty_strings() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([
            ("empty", Value::String("".to_string())),
            ("normal", Value::String("hello".to_string())),
        ]);

        let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();
        assert!(db.trace_get(&run, &id).unwrap().is_some());
    });
}

/// Test query with limit 0
#[test]
fn test_trace_list_limit_zero() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        // Create traces
        for _ in 0..5 {
            db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![]).unwrap();
        }

        // Limit 0
        let traces = db.trace_list(&run, None, None, None, Some(0), None).unwrap();
        assert!(traces.is_empty());
    });
}

/// Test query with limit 1
#[test]
fn test_trace_list_limit_one() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        for _ in 0..5 {
            db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![]).unwrap();
        }

        let traces = db.trace_list(&run, None, None, None, Some(1), None).unwrap();
        assert_eq!(traces.len(), 1);
    });
}

/// Test query with large limit
#[test]
fn test_trace_list_large_limit() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        for _ in 0..10 {
            db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![]).unwrap();
        }

        // Request more than available
        let traces = db.trace_list(&run, None, None, None, Some(1000), None).unwrap();
        assert_eq!(traces.len(), 10);
    });
}

/// Test search with limit 0
#[test]
fn test_trace_search_limit_zero() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("text", Value::String("searchable content".to_string()))]);

        db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();

        // Search with k=0
        let results = db.trace_search(&run, "searchable", 0).unwrap();
        assert!(results.is_empty());
    });
}

/// Test time range with invalid bounds
#[test]
fn test_trace_time_range_invalid() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();

        // Start > End (should return empty or handle gracefully)
        let traces = db.trace_query_by_time(&run, 1000, 500).unwrap();
        // Result depends on implementation
        let _ = traces;
    });
}

/// Test time range with negative timestamps
#[test]
fn test_trace_time_range_negative() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();

        // Negative range
        let traces = db.trace_query_by_time(&run, -1000, -500).unwrap();
        assert!(traces.is_empty());
    });
}

/// Test tool trace with all fields
#[test]
fn test_trace_tool_full() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([
            ("tool_name", Value::String("search_engine".to_string())),
            ("arguments", obj([
                ("query", Value::String("test query".to_string())),
                ("limit", Value::Int(10)),
            ])),
            ("result", Value::String("search results here".to_string())),
            ("duration_ms", Value::Int(150)),
        ]);

        let (id, _) = db.trace_create(&run, TraceType::Tool, None, content, vec!["tool-call".to_string()]).unwrap();
        let trace = db.trace_get(&run, &id).unwrap().unwrap();
        assert_eq!(trace.value.trace_type, TraceType::Tool);
    });
}

/// Test message trace types
#[test]
fn test_trace_message_types() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // User message
        let user_content = obj([
            ("role", Value::String("user".to_string())),
            ("content", Value::String("Hello, assistant!".to_string())),
        ]);
        let (user_id, _) = db.trace_create(&run, TraceType::Message, None, user_content, vec!["user".to_string()]).unwrap();

        // Assistant message
        let assistant_content = obj([
            ("role", Value::String("assistant".to_string())),
            ("content", Value::String("Hello! How can I help?".to_string())),
        ]);
        let (assistant_id, _) = db.trace_create(&run, TraceType::Message, Some(&user_id), assistant_content, vec!["assistant".to_string()]).unwrap();

        let user_trace = db.trace_get(&run, &user_id).unwrap().unwrap();
        let assistant_trace = db.trace_get(&run, &assistant_id).unwrap().unwrap();

        assert_eq!(user_trace.value.trace_type, TraceType::Message);
        assert_eq!(assistant_trace.value.trace_type, TraceType::Message);
        assert_eq!(assistant_trace.value.parent_id, Some(user_id));
    });
}
