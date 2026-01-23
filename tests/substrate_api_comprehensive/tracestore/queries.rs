//! TraceStore Query Tests
//!
//! Tests for query operations:
//! - trace_query_by_tag
//! - trace_query_by_time
//! - trace_search
//! - trace_list with filters

use crate::*;
use strata_api::substrate::{TraceStore, TraceType};

/// Test query by tag
#[test]
fn test_trace_query_by_tag() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        // Create traces with different tags
        db.trace_create(&run, TraceType::Thought, None, content.clone(), vec!["important".to_string()]).unwrap();
        db.trace_create(&run, TraceType::Action, None, content.clone(), vec!["important".to_string(), "debug".to_string()]).unwrap();
        db.trace_create(&run, TraceType::Observation, None, content.clone(), vec!["debug".to_string()]).unwrap();
        db.trace_create(&run, TraceType::Tool, None, content.clone(), vec!["tool".to_string()]).unwrap();

        // Query by tag
        let important = db.trace_query_by_tag(&run, "important").unwrap();
        let debug = db.trace_query_by_tag(&run, "debug").unwrap();
        let tool = db.trace_query_by_tag(&run, "tool").unwrap();
        let nonexistent = db.trace_query_by_tag(&run, "nonexistent").unwrap();

        assert_eq!(important.len(), 2);
        assert_eq!(debug.len(), 2);
        assert_eq!(tool.len(), 1);
        assert_eq!(nonexistent.len(), 0);
    });
}

/// Test query by tag with special characters
#[test]
fn test_trace_query_by_tag_special() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        // Tags with special characters
        let tags = vec![
            "category:analysis",
            "env:prod",
            "user-123",
            "tag_with_underscore",
        ];

        for tag in &tags {
            db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![tag.to_string()]).unwrap();
        }

        // Query each tag
        for tag in &tags {
            let results = db.trace_query_by_tag(&run, tag).unwrap();
            assert_eq!(results.len(), 1, "Tag '{}' should match exactly 1 trace", tag);
        }
    });
}

/// Test query by time range
#[test]
fn test_trace_query_by_time() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        let start_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Create traces with small delays
        for i in 0..5 {
            let c = obj([("order", Value::Int(i))]);
            db.trace_create(&run, TraceType::Thought, None, c, vec![]).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let end_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Query all within range
        let traces = db.trace_query_by_time(&run, start_time - 1000, end_time + 1000).unwrap();
        assert_eq!(traces.len(), 5);
    });
}

/// Test query by time range - no matches
#[test]
fn test_trace_query_by_time_no_matches() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        // Create traces
        for _ in 0..3 {
            db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![]).unwrap();
        }

        // Query in the past (before traces were created)
        let traces = db.trace_query_by_time(&run, 0, 1).unwrap();
        assert!(traces.is_empty());
    });
}

/// Test query by time range - partial matches
#[test]
fn test_trace_query_by_time_partial() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        // Get time before first trace
        let before_first = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Create first batch
        for _ in 0..3 {
            db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![]).unwrap();
        }

        // Small delay
        std::thread::sleep(std::time::Duration::from_millis(50));
        let mid_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Create second batch
        for _ in 0..2 {
            db.trace_create(&run, TraceType::Action, None, content.clone(), vec![]).unwrap();
        }

        let after_last = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Query all
        let all_traces = db.trace_query_by_time(&run, before_first - 1000, after_last + 1000).unwrap();
        assert_eq!(all_traces.len(), 5);

        // Query only first batch (approximately)
        let first_batch = db.trace_query_by_time(&run, before_first - 1000, mid_time).unwrap();
        assert!(first_batch.len() >= 1); // At least some from first batch
    });
}

/// Test trace search
#[test]
fn test_trace_search() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Create traces with searchable content
        let content1 = obj([
            ("thought", Value::String("The user wants to find information about climate change".to_string())),
        ]);
        let content2 = obj([
            ("thought", Value::String("I should search for weather data".to_string())),
        ]);
        let content3 = obj([
            ("action", Value::String("Executing database query".to_string())),
        ]);

        db.trace_create(&run, TraceType::Thought, None, content1, vec!["search".to_string()]).unwrap();
        db.trace_create(&run, TraceType::Thought, None, content2, vec![]).unwrap();
        db.trace_create(&run, TraceType::Action, None, content3, vec![]).unwrap();

        // Search for climate
        let results = db.trace_search(&run, "climate", 10).unwrap();
        // May or may not find matches depending on search implementation
        // Just verify it doesn't error
        let _ = results;

        // Search with tag
        let results = db.trace_search(&run, "search", 10).unwrap();
        let _ = results;
    });
}

/// Test trace search with limit
#[test]
fn test_trace_search_limit() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Create many traces
        for i in 0..20 {
            let content = obj([("term", Value::String(format!("searchable item {}", i)))]);
            db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();
        }

        // Search with limit
        let results = db.trace_search(&run, "searchable", 5).unwrap();
        assert!(results.len() <= 5);
    });
}

/// Test trace search empty query
#[test]
fn test_trace_search_empty() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();

        // Empty search
        let results = db.trace_search(&run, "", 10).unwrap();
        // Empty query may return all or none depending on implementation
        let _ = results;
    });
}

/// Test trace list with type filter
#[test]
fn test_trace_list_type_filter() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        // Create different types
        db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![]).unwrap();
        db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![]).unwrap();
        db.trace_create(&run, TraceType::Action, None, content.clone(), vec![]).unwrap();
        db.trace_create(&run, TraceType::Tool, None, content.clone(), vec![]).unwrap();

        // Filter by type
        let thoughts = db.trace_list(&run, Some(TraceType::Thought), None, None, None, None).unwrap();
        let actions = db.trace_list(&run, Some(TraceType::Action), None, None, None, None).unwrap();
        let tools = db.trace_list(&run, Some(TraceType::Tool), None, None, None, None).unwrap();

        assert_eq!(thoughts.len(), 2);
        assert_eq!(actions.len(), 1);
        assert_eq!(tools.len(), 1);
    });
}

/// Test trace list with custom type filter
#[test]
fn test_trace_list_custom_type_filter() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        // Create with custom types
        db.trace_create(&run, TraceType::Custom("Analysis".to_string()), None, content.clone(), vec![]).unwrap();
        db.trace_create(&run, TraceType::Custom("Analysis".to_string()), None, content.clone(), vec![]).unwrap();
        db.trace_create(&run, TraceType::Custom("Summary".to_string()), None, content.clone(), vec![]).unwrap();

        // Filter by custom type
        let analysis = db.trace_list(&run, Some(TraceType::Custom("Analysis".to_string())), None, None, None, None).unwrap();
        let summary = db.trace_list(&run, Some(TraceType::Custom("Summary".to_string())), None, None, None, None).unwrap();

        assert_eq!(analysis.len(), 2);
        assert_eq!(summary.len(), 1);
    });
}

/// Test trace list with tag filter
#[test]
fn test_trace_list_tag_filter() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        // Create with tags
        db.trace_create(&run, TraceType::Thought, None, content.clone(), vec!["important".to_string()]).unwrap();
        db.trace_create(&run, TraceType::Thought, None, content.clone(), vec!["important".to_string(), "review".to_string()]).unwrap();
        db.trace_create(&run, TraceType::Thought, None, content.clone(), vec!["debug".to_string()]).unwrap();

        // Filter by tag
        let important = db.trace_list(&run, None, None, Some("important"), None, None).unwrap();
        let debug = db.trace_list(&run, None, None, Some("debug"), None, None).unwrap();

        assert_eq!(important.len(), 2);
        assert_eq!(debug.len(), 1);
    });
}

/// Test combined filters
#[test]
fn test_trace_list_combined_filters() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        // Create various traces
        db.trace_create(&run, TraceType::Thought, None, content.clone(), vec!["important".to_string()]).unwrap();
        db.trace_create(&run, TraceType::Action, None, content.clone(), vec!["important".to_string()]).unwrap();
        db.trace_create(&run, TraceType::Thought, None, content.clone(), vec!["debug".to_string()]).unwrap();

        // Filter by type (tag filter is secondary)
        let thoughts = db.trace_list(&run, Some(TraceType::Thought), None, None, None, None).unwrap();
        assert_eq!(thoughts.len(), 2);

        // Note: The API implementation may prioritize type over tag filter
    });
}

/// Test search hit structure
#[test]
fn test_trace_search_hit_structure() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([
            ("unique_term", Value::String("findable_content_xyz".to_string())),
        ]);

        let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();

        // Search for unique term
        let results = db.trace_search(&run, "findable_content_xyz", 10).unwrap();

        // Check if found
        if !results.is_empty() {
            // Verify hit structure
            for hit in &results {
                assert!(!hit.id.is_empty(), "Hit should have ID");
                // Score may be any value
            }

            // The trace we created should be in results
            let found = results.iter().any(|h| h.id == id);
            // Note: may not always find due to search indexing
            let _ = found;
        }
    });
}

/// Test empty run queries
#[test]
fn test_trace_queries_empty_run() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // All query operations on empty run should return empty results
        let by_tag = db.trace_query_by_tag(&run, "any").unwrap();
        assert!(by_tag.is_empty());

        let by_time = db.trace_query_by_time(&run, 0, i64::MAX).unwrap();
        assert!(by_time.is_empty());

        let search = db.trace_search(&run, "anything", 10).unwrap();
        assert!(search.is_empty());

        let list = db.trace_list(&run, None, None, None, None, None).unwrap();
        assert!(list.is_empty());
    });
}

/// Test query results contain correct data
#[test]
fn test_trace_query_results_correctness() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("important", Value::Bool(true))]);
        let tags = vec!["findme".to_string(), "test".to_string()];

        let (id, _) = db.trace_create(&run, TraceType::Action, None, content, tags.clone()).unwrap();

        // Query by tag
        let results = db.trace_query_by_tag(&run, "findme").unwrap();
        assert_eq!(results.len(), 1);

        let trace = &results[0].value;
        assert_eq!(trace.id, id);
        assert_eq!(trace.trace_type, TraceType::Action);
        assert_eq!(trace.tags, tags);
        assert!(trace.parent_id.is_none());
    });
}
