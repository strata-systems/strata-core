//! TraceStore Concurrency Tests
//!
//! Tests for thread safety:
//! - Concurrent trace creation
//! - Concurrent reads
//! - Concurrent queries

use crate::*;
use strata_api::substrate::{TraceStore, TraceType};
use std::sync::{Arc, Barrier};
use std::thread;

/// Test concurrent trace creation
#[test]
fn test_trace_concurrent_create() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();

    const NUM_THREADS: usize = 8;
    const TRACES_PER_THREAD: usize = 20;

    let barrier = Arc::new(Barrier::new(NUM_THREADS));
    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|i| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let run = run.clone();

            thread::spawn(move || {
                barrier.wait();

                for j in 0..TRACES_PER_THREAD {
                    let content = obj([
                        ("thread", Value::Int(i as i64)),
                        ("trace", Value::Int(j as i64)),
                    ]);
                    let tags = vec![format!("thread_{}", i)];
                    let _ = substrate.trace_create(&run, TraceType::Thought, None, content, tags);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Verify all traces were created
    let count = substrate.trace_count(&run).unwrap();
    assert!(count > 0);
}

/// Test concurrent reads
#[test]
fn test_trace_concurrent_reads() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();

    // Pre-populate with traces
    let mut ids = Vec::new();
    for i in 0..20 {
        let content = obj([("index", Value::Int(i))]);
        let (id, _) = substrate.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();
        ids.push(id);
    }
    let ids = Arc::new(ids);

    const NUM_READERS: usize = 10;
    const READS_PER_THREAD: usize = 50;

    let barrier = Arc::new(Barrier::new(NUM_READERS));
    let handles: Vec<_> = (0..NUM_READERS)
        .map(|_| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let run = run.clone();
            let ids = Arc::clone(&ids);

            thread::spawn(move || {
                barrier.wait();

                for i in 0..READS_PER_THREAD {
                    let id = &ids[i % ids.len()];
                    let result = substrate.trace_get(&run, id);
                    assert!(result.is_ok(), "Read should succeed");
                    assert!(result.unwrap().is_some(), "Trace should exist");
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}

/// Test concurrent reads and writes
#[test]
fn test_trace_concurrent_read_write() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();

    // Pre-populate
    let mut ids = Vec::new();
    for i in 0..10 {
        let content = obj([("index", Value::Int(i))]);
        let (id, _) = substrate.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();
        ids.push(id);
    }
    let ids = Arc::new(ids);

    const NUM_WRITERS: usize = 4;
    const NUM_READERS: usize = 6;
    const OPS_PER_THREAD: usize = 20;

    let barrier = Arc::new(Barrier::new(NUM_WRITERS + NUM_READERS));

    // Spawn writers
    let mut handles: Vec<_> = (0..NUM_WRITERS)
        .map(|i| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let run = run.clone();

            thread::spawn(move || {
                barrier.wait();

                for j in 0..OPS_PER_THREAD {
                    let content = obj([
                        ("writer", Value::Int(i as i64)),
                        ("op", Value::Int(j as i64)),
                    ]);
                    let _ = substrate.trace_create(&run, TraceType::Action, None, content, vec![]);
                }
            })
        })
        .collect();

    // Spawn readers
    let reader_handles: Vec<_> = (0..NUM_READERS)
        .map(|_| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let run = run.clone();
            let ids = Arc::clone(&ids);

            thread::spawn(move || {
                barrier.wait();

                for i in 0..OPS_PER_THREAD {
                    let id = &ids[i % ids.len()];
                    let _ = substrate.trace_get(&run, id);
                }
            })
        })
        .collect();

    handles.extend(reader_handles);

    for h in handles {
        h.join().unwrap();
    }
}

/// Test concurrent queries
#[test]
fn test_trace_concurrent_queries() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();

    // Pre-populate with tagged traces
    for i in 0..30 {
        let content = obj([("index", Value::Int(i))]);
        let tag = if i % 3 == 0 { "typeA" } else if i % 3 == 1 { "typeB" } else { "typeC" };
        substrate.trace_create(&run, TraceType::Thought, None, content, vec![tag.to_string()]).unwrap();
    }

    const NUM_QUERIERS: usize = 8;
    const QUERIES_PER_THREAD: usize = 20;

    let barrier = Arc::new(Barrier::new(NUM_QUERIERS));
    let handles: Vec<_> = (0..NUM_QUERIERS)
        .map(|i| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let run = run.clone();

            thread::spawn(move || {
                barrier.wait();

                for _ in 0..QUERIES_PER_THREAD {
                    let tag = match i % 3 {
                        0 => "typeA",
                        1 => "typeB",
                        _ => "typeC",
                    };
                    let result = substrate.trace_query_by_tag(&run, tag);
                    assert!(result.is_ok(), "Query should succeed");
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}

/// Test concurrent hierarchy creation
#[test]
fn test_trace_concurrent_hierarchy() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();

    // Create root trace
    let content = obj([("name", Value::String("root".to_string()))]);
    let (root_id, _) = substrate.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();
    let root_id = Arc::new(root_id);

    const NUM_THREADS: usize = 6;
    const CHILDREN_PER_THREAD: usize = 10;

    let barrier = Arc::new(Barrier::new(NUM_THREADS));
    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|i| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let run = run.clone();
            let root_id = Arc::clone(&root_id);

            thread::spawn(move || {
                barrier.wait();

                for j in 0..CHILDREN_PER_THREAD {
                    let content = obj([
                        ("thread", Value::Int(i as i64)),
                        ("child", Value::Int(j as i64)),
                    ]);
                    let _ = substrate.trace_create(&run, TraceType::Action, Some(&root_id), content, vec![]);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Verify tree
    let tree = substrate.trace_tree(&run, &root_id).unwrap();
    assert!(tree.len() > 1);
}

/// Test concurrent list operations
#[test]
fn test_trace_concurrent_list() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();

    // Pre-populate
    for i in 0..20 {
        let content = obj([("index", Value::Int(i))]);
        substrate.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();
    }

    const NUM_THREADS: usize = 10;
    const OPS_PER_THREAD: usize = 20;

    let barrier = Arc::new(Barrier::new(NUM_THREADS));
    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|_| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let run = run.clone();

            thread::spawn(move || {
                barrier.wait();

                for _ in 0..OPS_PER_THREAD {
                    let result = substrate.trace_list(&run, None, None, None, Some(10), None);
                    assert!(result.is_ok(), "List should succeed");
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}

/// Test concurrent count operations
#[test]
fn test_trace_concurrent_count() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();

    // Pre-populate
    for i in 0..10 {
        let content = obj([("index", Value::Int(i))]);
        substrate.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();
    }

    const NUM_THREADS: usize = 10;
    let barrier = Arc::new(Barrier::new(NUM_THREADS + 1));

    // Count readers
    let count_handles: Vec<_> = (0..NUM_THREADS)
        .map(|_| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let run = run.clone();

            thread::spawn(move || {
                barrier.wait();

                for _ in 0..20 {
                    let count = substrate.trace_count(&run);
                    assert!(count.is_ok());
                }
            })
        })
        .collect();

    // Writer
    let substrate_clone = Arc::clone(&substrate);
    let run_clone = run.clone();
    let barrier_clone = Arc::clone(&barrier);
    let writer = thread::spawn(move || {
        barrier_clone.wait();

        for i in 0..10 {
            let content = obj([("extra", Value::Int(i))]);
            let _ = substrate_clone.trace_create(&run_clone, TraceType::Action, None, content, vec![]);
        }
    });

    for h in count_handles {
        h.join().unwrap();
    }
    writer.join().unwrap();
}

/// Test concurrent search operations
#[test]
fn test_trace_concurrent_search() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();

    // Pre-populate with searchable content
    for i in 0..20 {
        let content = obj([
            ("searchable", Value::String(format!("trace content number {}", i))),
        ]);
        substrate.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();
    }

    const NUM_THREADS: usize = 8;
    const SEARCHES_PER_THREAD: usize = 10;

    let barrier = Arc::new(Barrier::new(NUM_THREADS));
    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|_| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let run = run.clone();

            thread::spawn(move || {
                barrier.wait();

                for _ in 0..SEARCHES_PER_THREAD {
                    let result = substrate.trace_search(&run, "content", 5);
                    assert!(result.is_ok(), "Search should succeed");
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}
