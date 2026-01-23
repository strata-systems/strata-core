//! StateCell Concurrency Tests
//!
//! Tests for thread safety:
//! - Concurrent reads
//! - Concurrent writes
//! - CAS under contention

use crate::*;
use std::sync::{Arc, Barrier};
use std::thread;
use strata_core::Version;

/// Helper to extract counter from Version
fn get_counter(version: &Version) -> u64 {
    match version {
        Version::Counter(c) => *c,
        _ => panic!("Expected Version::Counter"),
    }
}

/// Test concurrent reads don't interfere
#[test]
fn test_state_concurrent_reads() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();
    let cell = "concurrent_read_cell";

    // Set initial value
    substrate.state_set(&run, cell, Value::Int(42)).unwrap();

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
                    let result = substrate.state_get(&run, cell).unwrap().unwrap();
                    assert_eq!(result.value, Value::Int(42));
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}

/// Test concurrent writes to different cells
#[test]
fn test_state_concurrent_writes_different_cells() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();

    const NUM_WRITERS: usize = 10;
    const WRITES_PER_THREAD: usize = 50;

    let barrier = Arc::new(Barrier::new(NUM_WRITERS));
    let handles: Vec<_> = (0..NUM_WRITERS)
        .map(|i| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let run = run.clone();

            thread::spawn(move || {
                barrier.wait();

                let cell = format!("cell_{}", i);
                for j in 0..WRITES_PER_THREAD {
                    substrate.state_set(&run, &cell, Value::Int(j as i64)).unwrap();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Verify all cells exist with final value
    for i in 0..NUM_WRITERS {
        let cell = format!("cell_{}", i);
        let result = substrate.state_get(&run, &cell).unwrap().unwrap();
        assert_eq!(result.value, Value::Int((WRITES_PER_THREAD - 1) as i64), "Cell {} should have value {}", cell, WRITES_PER_THREAD - 1);
    }
}

/// Test CAS under contention - threads race to increment
#[test]
fn test_state_cas_contention() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();
    let cell = "contended_cell";

    // Set initial value
    let initial = substrate.state_set(&run, cell, Value::Int(0)).unwrap();
    let _initial_counter = get_counter(&initial);

    const NUM_THREADS: usize = 5;
    const CAS_ATTEMPTS: usize = 20;

    let barrier = Arc::new(Barrier::new(NUM_THREADS));
    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|i| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let run = run.clone();

            thread::spawn(move || {
                barrier.wait();

                let mut successes = 0;
                for _ in 0..CAS_ATTEMPTS {
                    // Try to increment: read current, CAS with current counter
                    let current = substrate.state_get(&run, cell).unwrap().unwrap();
                    let current_counter = get_counter(&current.version);
                    if let Value::Int(n) = current.value {
                        if substrate.state_cas(&run, cell, Some(current_counter), Value::Int(n + 1)).unwrap().is_some() {
                            successes += 1;
                        }
                    }
                }
                (i, successes)
            })
        })
        .collect();

    let mut total_successes = 0;
    for h in handles {
        let (_, successes) = h.join().unwrap();
        total_successes += successes;
    }

    // The final value should equal total successful CAS operations
    let final_value = substrate.state_get(&run, cell).unwrap().unwrap();
    if let Value::Int(n) = final_value.value {
        assert_eq!(n, total_successes as i64, "Final value should match total successes");
    }
}

/// Test many concurrent operations on same cell
/// Note: In buffered mode, write conflicts are expected when multiple threads
/// write to the same cell concurrently. This test verifies the system handles
/// this correctly (either succeeding or returning WriteConflict error).
#[test]
fn test_state_high_contention_single_cell() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();
    let cell = "hot_cell";

    substrate.state_set(&run, cell, Value::Int(0)).unwrap();

    const NUM_THREADS: usize = 8;
    const WRITES_PER_THREAD: usize = 100;

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

                for j in 0..WRITES_PER_THREAD {
                    // Write conflicts are expected - count successes
                    if substrate.state_set(&run, cell, Value::Int((i * WRITES_PER_THREAD + j) as i64)).is_ok() {
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
    let total_successes = success_count.load(std::sync::atomic::Ordering::Relaxed);
    assert!(total_successes > 0, "At least some writes should succeed");

    // Cell should have a valid value (exact value depends on execution order)
    let result = substrate.state_get(&run, cell).unwrap();
    assert!(result.is_some(), "Cell should exist after concurrent writes");
}

/// Test concurrent CAS to same cell - only one should succeed per round
#[test]
fn test_state_cas_mutual_exclusion() {
    let db = create_buffered_db();
    let substrate = Arc::new(create_substrate(db));
    let run = ApiRunId::default_run_id();
    let cell = "mutex_cell";

    // Set initial value
    substrate.state_set(&run, cell, Value::Int(0)).unwrap();

    const NUM_THREADS: usize = 4;
    const ROUNDS: usize = 10;

    let barrier = Arc::new(Barrier::new(NUM_THREADS));
    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|thread_id| {
            let substrate = Arc::clone(&substrate);
            let barrier = Arc::clone(&barrier);
            let run = run.clone();

            thread::spawn(move || {
                let mut wins = 0;

                for _round in 0..ROUNDS {
                    barrier.wait(); // Sync threads at start of each round

                    // All threads read current state
                    let current = substrate.state_get(&run, cell).unwrap().unwrap();
                    let current_counter = get_counter(&current.version);
                    if let Value::Int(n) = current.value {
                        // All threads try to CAS at the same time
                        if substrate.state_cas(&run, cell, Some(current_counter), Value::Int(n + 1)).unwrap().is_some() {
                            wins += 1;
                        }
                    }
                }
                (thread_id, wins)
            })
        })
        .collect();

    let mut total_wins = 0;
    for h in handles {
        let (_, wins) = h.join().unwrap();
        total_wins += wins;
    }

    // Total wins should equal the value increment (one winner per round)
    let final_result = substrate.state_get(&run, cell).unwrap().unwrap();
    if let Value::Int(n) = final_result.value {
        assert_eq!(n, total_wins as i64, "Total CAS wins should equal final value");
    }
}
