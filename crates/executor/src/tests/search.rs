//! Search command tests: verify executor Search command works end-to-end.

use crate::{Command, Executor, Output};
use strata_core::Value;
use strata_engine::Database;
use std::sync::Arc;

fn create_executor() -> Executor {
    let db = Arc::new(Database::builder().no_durability().open_temp().unwrap());
    Executor::new(db)
}

#[test]
fn test_search_empty_database() {
    let executor = create_executor();

    let result = executor.execute(Command::Search {
        run: None,
        query: "nonexistent".to_string(),
        k: None,
        primitives: None,
    });

    match result {
        Ok(Output::SearchResults(hits)) => {
            assert!(hits.is_empty(), "Empty database should return no results");
        }
        other => panic!("Expected SearchResults, got {:?}", other),
    }
}

#[test]
fn test_search_finds_kv_data() {
    let executor = create_executor();

    // Insert some data
    executor
        .execute(Command::KvPut {
            run: None,
            key: "greeting".to_string(),
            value: Value::String("hello world".into()),
        })
        .unwrap();

    executor
        .execute(Command::KvPut {
            run: None,
            key: "farewell".to_string(),
            value: Value::String("goodbye world".into()),
        })
        .unwrap();

    // Search for "hello"
    let result = executor.execute(Command::Search {
        run: None,
        query: "hello".to_string(),
        k: Some(10),
        primitives: Some(vec!["kv".to_string()]),
    });

    match result {
        Ok(Output::SearchResults(hits)) => {
            assert!(!hits.is_empty(), "Should find at least one result for 'hello'");
            // The greeting key should be in results
            assert!(
                hits.iter().any(|h| h.entity == "greeting"),
                "Should find 'greeting' key, got: {:?}",
                hits
            );
            // All results should be from kv primitive
            for hit in &hits {
                assert_eq!(hit.primitive, "kv");
            }
        }
        other => panic!("Expected SearchResults, got {:?}", other),
    }
}

#[test]
fn test_search_with_primitive_filter() {
    let executor = create_executor();

    // Insert KV data
    executor
        .execute(Command::KvPut {
            run: None,
            key: "test_key".to_string(),
            value: Value::String("searchable data".into()),
        })
        .unwrap();

    // Search only in event primitive (should find nothing since we only put KV data)
    let result = executor.execute(Command::Search {
        run: None,
        query: "searchable".to_string(),
        k: Some(10),
        primitives: Some(vec!["event".to_string()]),
    });

    match result {
        Ok(Output::SearchResults(hits)) => {
            // Should not find KV data when filtering to event only
            assert!(
                !hits.iter().any(|h| h.primitive == "kv"),
                "Should not find KV data when filtering to event primitive"
            );
        }
        other => panic!("Expected SearchResults, got {:?}", other),
    }
}

#[test]
fn test_search_with_k_limit() {
    let executor = create_executor();

    // Insert multiple items with "test" in them
    for i in 0..5 {
        executor
            .execute(Command::KvPut {
                run: None,
                key: format!("test_item_{}", i),
                value: Value::String(format!("test data number {}", i)),
            })
            .unwrap();
    }

    // Search with k=2
    let result = executor.execute(Command::Search {
        run: None,
        query: "test".to_string(),
        k: Some(2),
        primitives: Some(vec!["kv".to_string()]),
    });

    match result {
        Ok(Output::SearchResults(hits)) => {
            assert!(
                hits.len() <= 2,
                "Should return at most 2 results, got {}",
                hits.len()
            );
        }
        other => panic!("Expected SearchResults, got {:?}", other),
    }
}

#[test]
fn test_search_result_has_scores_and_ranks() {
    let executor = create_executor();

    executor
        .execute(Command::KvPut {
            run: None,
            key: "scored_item".to_string(),
            value: Value::String("important data for scoring".into()),
        })
        .unwrap();

    let result = executor.execute(Command::Search {
        run: None,
        query: "important scoring".to_string(),
        k: Some(10),
        primitives: Some(vec!["kv".to_string()]),
    });

    match result {
        Ok(Output::SearchResults(hits)) => {
            for hit in &hits {
                assert!(hit.score >= 0.0, "Score should be non-negative");
                assert!(hit.rank >= 1, "Rank should be 1-indexed");
            }
        }
        other => panic!("Expected SearchResults, got {:?}", other),
    }
}
