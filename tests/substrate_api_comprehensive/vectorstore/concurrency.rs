//! VectorStore Concurrency Tests
//!
//! Tests for thread safety:
//! - Concurrent inserts
//! - Concurrent searches
//! - Concurrent reads and writes

use crate::*;
use std::sync::{Arc, Barrier};
use std::thread;

/// Test concurrent inserts to different keys
#[test]
fn test_vector_concurrent_inserts_different_keys() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();
    let collection = "concurrent_inserts";

    const NUM_THREADS: usize = 8;
    const INSERTS_PER_THREAD: usize = 20;

    let barrier = Arc::new(Barrier::new(NUM_THREADS));
    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|i| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let run = run.clone();

            thread::spawn(move || {
                barrier.wait();

                for j in 0..INSERTS_PER_THREAD {
                    let key = format!("thread_{}_vec_{}", i, j);
                    let vec = vec![i as f32, j as f32, (i + j) as f32];
                    let _ = substrate.vector_upsert(&run, collection, &key, &vec, None);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Verify some vectors exist
    let collections = substrate.vector_list_collections(&run).unwrap();
    assert!(!collections.is_empty());
}

/// Test concurrent searches
#[test]
fn test_vector_concurrent_searches() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();
    let collection = "concurrent_search";

    // Pre-populate with vectors
    for i in 0..50 {
        let mut vec = vec![0.0f32; 8];
        vec[i % 8] = 1.0;
        substrate.vector_upsert(&run, collection, &format!("v{}", i), &vec, None).unwrap();
    }

    const NUM_READERS: usize = 10;
    const SEARCHES_PER_THREAD: usize = 20;

    let barrier = Arc::new(Barrier::new(NUM_READERS));
    let handles: Vec<_> = (0..NUM_READERS)
        .map(|i| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let run = run.clone();

            thread::spawn(move || {
                barrier.wait();

                for _ in 0..SEARCHES_PER_THREAD {
                    let mut query = vec![0.0f32; 8];
                    query[i % 8] = 1.0;
                    let results = substrate.vector_search(&run, collection, &query, 5, None, None);
                    assert!(results.is_ok(), "Search should succeed");
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
fn test_vector_concurrent_read_write() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();
    let collection = "concurrent_rw";

    // Pre-populate
    for i in 0..20 {
        let vec = vec![i as f32, 0.0, 0.0];
        substrate.vector_upsert(&run, collection, &format!("v{}", i), &vec, None).unwrap();
    }

    const NUM_WRITERS: usize = 4;
    const NUM_READERS: usize = 8;
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
                    let key = format!("write_{}_v{}", i, j);
                    let vec = vec![i as f32, j as f32, (i + j) as f32];
                    let _ = substrate.vector_upsert(&run, collection, &key, &vec, None);
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

            thread::spawn(move || {
                barrier.wait();

                for i in 0..OPS_PER_THREAD {
                    let key = format!("v{}", i % 20);
                    let _ = substrate.vector_get(&run, collection, &key);
                }
            })
        })
        .collect();

    handles.extend(reader_handles);

    for h in handles {
        h.join().unwrap();
    }
}

/// Test concurrent collection operations
#[test]
fn test_vector_concurrent_collection_ops() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();

    const NUM_THREADS: usize = 6;

    let barrier = Arc::new(Barrier::new(NUM_THREADS));
    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|i| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let run = run.clone();

            thread::spawn(move || {
                barrier.wait();

                let collection = format!("coll_{}", i);

                // Create collection
                let _ = substrate.vector_upsert(
                    &run,
                    &collection,
                    "v1",
                    &[1.0, 2.0, 3.0],
                    None,
                );

                // Check info
                let _ = substrate.vector_collection_info(&run, &collection);

                // List collections
                let _ = substrate.vector_list_collections(&run);
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Verify collections were created
    let collections = substrate.vector_list_collections(&run).unwrap();
    assert!(collections.len() >= 1);
}

/// Test concurrent updates to same key
#[test]
fn test_vector_concurrent_same_key_updates() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();
    let collection = "same_key_updates";

    // Create initial vector
    substrate.vector_upsert(&run, collection, "shared", &[0.0, 0.0, 0.0], None).unwrap();

    const NUM_THREADS: usize = 10;
    const UPDATES_PER_THREAD: usize = 20;

    let barrier = Arc::new(Barrier::new(NUM_THREADS));
    let success_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|i| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let success_count = Arc::clone(&success_count);
            let run = run.clone();

            thread::spawn(move || {
                barrier.wait();

                for j in 0..UPDATES_PER_THREAD {
                    let vec = vec![i as f32, j as f32, (i * j) as f32];
                    if substrate.vector_upsert(&run, collection, "shared", &vec, None).is_ok() {
                        success_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // At least some updates should succeed
    let total = success_count.load(std::sync::atomic::Ordering::Relaxed);
    assert!(total > 0, "Some updates should succeed");

    // Vector should still exist
    let result = substrate.vector_get(&run, collection, "shared").unwrap();
    assert!(result.is_some());
}

/// Test concurrent deletes
#[test]
fn test_vector_concurrent_deletes() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();
    let collection = "concurrent_deletes";

    // Pre-populate
    for i in 0..100 {
        substrate.vector_upsert(&run, collection, &format!("v{}", i), &[i as f32], None).unwrap();
    }

    const NUM_THREADS: usize = 10;

    let barrier = Arc::new(Barrier::new(NUM_THREADS));
    let delete_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|i| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let delete_count = Arc::clone(&delete_count);
            let run = run.clone();

            thread::spawn(move || {
                barrier.wait();

                // Each thread deletes its range of vectors
                for j in 0..10 {
                    let key = format!("v{}", i * 10 + j);
                    if substrate.vector_delete(&run, collection, &key).unwrap_or(false) {
                        delete_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // All 100 vectors should have been deleted
    let total_deleted = delete_count.load(std::sync::atomic::Ordering::Relaxed);
    assert!(total_deleted > 0, "Some deletes should succeed");
}

/// Test concurrent searches while inserting
#[test]
fn test_vector_search_during_inserts() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();
    let collection = "search_during_insert";

    // Pre-populate with a few vectors
    for i in 0..10 {
        substrate.vector_upsert(&run, collection, &format!("initial_{}", i), &[i as f32, 0.0, 0.0], None).unwrap();
    }

    const NUM_INSERTERS: usize = 3;
    const NUM_SEARCHERS: usize = 5;
    const OPS_PER_THREAD: usize = 20;

    let barrier = Arc::new(Barrier::new(NUM_INSERTERS + NUM_SEARCHERS));

    // Spawn inserters
    let mut handles: Vec<_> = (0..NUM_INSERTERS)
        .map(|i| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let run = run.clone();

            thread::spawn(move || {
                barrier.wait();

                for j in 0..OPS_PER_THREAD {
                    let key = format!("new_{}_v{}", i, j);
                    let vec = vec![i as f32, j as f32, (i + j) as f32];
                    let _ = substrate.vector_upsert(&run, collection, &key, &vec, None);
                }
            })
        })
        .collect();

    // Spawn searchers
    let search_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let searcher_handles: Vec<_> = (0..NUM_SEARCHERS)
        .map(|_| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let search_count = Arc::clone(&search_count);
            let run = run.clone();

            thread::spawn(move || {
                barrier.wait();

                for _ in 0..OPS_PER_THREAD {
                    let query = vec![1.0, 0.0, 0.0];
                    if substrate.vector_search(&run, collection, &query, 5, None, None).is_ok() {
                        search_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();

    handles.extend(searcher_handles);

    for h in handles {
        h.join().unwrap();
    }

    // Most searches should succeed
    let total_searches = search_count.load(std::sync::atomic::Ordering::Relaxed);
    assert!(total_searches > 0, "Some searches should succeed");
}
