//! JsonStore Concurrency Tests
//!
//! Tests for thread safety:
//! - Concurrent reads
//! - Concurrent writes to different documents
//! - Concurrent path updates

use crate::*;
use std::sync::{Arc, Barrier};
use std::thread;

/// Test concurrent reads to same document
#[test]
fn test_json_concurrent_reads() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();
    let key = "concurrent_read_doc";

    // Create document
    let document = obj([("value", Value::Int(42))]);
    substrate.json_set(&run, key, "$", document).unwrap();

    const NUM_READERS: usize = 10;
    const READS_PER_THREAD: usize = 100;

    let barrier = Arc::new(Barrier::new(NUM_READERS));
    let handles: Vec<_> = (0..NUM_READERS)
        .map(|_| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let run = run.clone();

            thread::spawn(move || {
                barrier.wait();

                for _ in 0..READS_PER_THREAD {
                    let result = substrate.json_get(&run, key, "value").unwrap().unwrap();
                    assert_eq!(result.value, Value::Int(42));
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}

/// Test concurrent writes to different documents
#[test]
fn test_json_concurrent_writes_different_docs() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();

    const NUM_WRITERS: usize = 10;
    const WRITES_PER_THREAD: usize = 20;

    let barrier = Arc::new(Barrier::new(NUM_WRITERS));
    let handles: Vec<_> = (0..NUM_WRITERS)
        .map(|i| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let run = run.clone();

            thread::spawn(move || {
                barrier.wait();

                let key = format!("doc_{}", i);
                for j in 0..WRITES_PER_THREAD {
                    let document = obj([("count", Value::Int(j as i64))]);
                    substrate.json_set(&run, &key, "$", document).unwrap();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Verify all documents exist with final value
    for i in 0..NUM_WRITERS {
        let key = format!("doc_{}", i);
        let result = substrate.json_get(&run, &key, "count").unwrap().unwrap();
        assert_eq!(
            result.value,
            Value::Int((WRITES_PER_THREAD - 1) as i64),
            "Doc {} should have final count",
            i
        );
    }
}

/// Test concurrent writes to same document (different paths)
#[test]
fn test_json_concurrent_path_writes() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();
    let key = "concurrent_paths";

    // Create initial document
    let document = obj([]);
    substrate.json_set(&run, key, "$", document).unwrap();

    const NUM_WRITERS: usize = 5;
    const WRITES_PER_THREAD: usize = 10;

    let barrier = Arc::new(Barrier::new(NUM_WRITERS));
    let success_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let handles: Vec<_> = (0..NUM_WRITERS)
        .map(|i| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let success_count = Arc::clone(&success_count);
            let run = run.clone();

            thread::spawn(move || {
                barrier.wait();

                let path = format!("field_{}", i);
                for j in 0..WRITES_PER_THREAD {
                    // Handle potential write conflicts
                    if substrate.json_set(&run, key, &path, Value::Int(j as i64)).is_ok() {
                        success_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // At least some writes should have succeeded
    let total = success_count.load(std::sync::atomic::Ordering::Relaxed);
    assert!(total > 0, "Some writes should succeed");
}

/// Test concurrent merges to same document
#[test]
fn test_json_concurrent_merges() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();
    let key = "concurrent_merge";

    // Create initial document
    let document = obj([("initial", Value::Int(0))]);
    substrate.json_set(&run, key, "$", document).unwrap();

    const NUM_MERGERS: usize = 4;
    const MERGES_PER_THREAD: usize = 10;

    let barrier = Arc::new(Barrier::new(NUM_MERGERS));
    let success_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let handles: Vec<_> = (0..NUM_MERGERS)
        .map(|i| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let success_count = Arc::clone(&success_count);
            let run = run.clone();

            thread::spawn(move || {
                barrier.wait();

                for j in 0..MERGES_PER_THREAD {
                    let field_name = format!("thread_{}_iter_{}", i, j);
                    let patch = obj_owned([(field_name, Value::Int((i * MERGES_PER_THREAD + j) as i64))]);
                    // Handle potential write conflicts
                    if substrate.json_merge(&run, key, "$", patch).is_ok() {
                        success_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Document should still exist with initial field
    let result = substrate.json_get(&run, key, "initial").unwrap().unwrap();
    assert_eq!(result.value, Value::Int(0));

    // At least some merges should have succeeded
    let total = success_count.load(std::sync::atomic::Ordering::Relaxed);
    assert!(total > 0, "Some merges should succeed");
}

/// Test concurrent reads and writes
#[test]
fn test_json_concurrent_read_write() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();
    let key = "read_write_doc";

    // Create initial document
    let document = obj([("counter", Value::Int(0))]);
    substrate.json_set(&run, key, "$", document).unwrap();

    const NUM_READERS: usize = 4;
    const NUM_WRITERS: usize = 2;
    const OPS_PER_THREAD: usize = 50;

    let barrier = Arc::new(Barrier::new(NUM_READERS + NUM_WRITERS));

    // Spawn readers
    let mut handles: Vec<_> = (0..NUM_READERS)
        .map(|_| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let run = run.clone();

            thread::spawn(move || {
                barrier.wait();

                for _ in 0..OPS_PER_THREAD {
                    // Read should always succeed
                    let result = substrate.json_get(&run, key, "counter");
                    assert!(result.is_ok(), "Read should succeed");
                }
            })
        })
        .collect();

    // Spawn writers
    let write_handles: Vec<_> = (0..NUM_WRITERS)
        .map(|_| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let run = run.clone();

            thread::spawn(move || {
                barrier.wait();

                for i in 0..OPS_PER_THREAD {
                    // Writes may conflict, which is acceptable
                    let _ = substrate.json_set(&run, key, "counter", Value::Int(i as i64));
                }
            })
        })
        .collect();

    handles.extend(write_handles);

    for h in handles {
        h.join().unwrap();
    }

    // Document should still exist
    let result = substrate.json_get(&run, key, "$").unwrap();
    assert!(result.is_some(), "Document should still exist");
}
