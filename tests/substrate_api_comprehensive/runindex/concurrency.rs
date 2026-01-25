//! RunIndex Concurrency Tests
//!
//! Tests for thread safety and concurrent access:
//! - Concurrent creates
//! - Concurrent status updates
//! - Concurrent tag modifications
//! - Read during write
//! - No lost updates
//!
//! NOTE: With optimistic concurrency control (OCC), concurrent writes to the
//! same entity may fail with WriteConflict. The proper pattern is to retry
//! on conflict. Tests use retry logic to handle expected conflicts.

use crate::*;
use strata_core::{StrataError, StrataResult};
use std::sync::Arc;
use std::thread;

/// Retry an operation on WriteConflict errors
fn retry_on_conflict<F, T>(max_retries: u32, mut operation: F) -> StrataResult<T>
where
    F: FnMut() -> StrataResult<T>,
{
    let mut last_err = None;
    for _ in 0..max_retries {
        match operation() {
            Ok(result) => return Ok(result),
            Err(e) => {
                // Check if it's a write conflict
                let err_str = format!("{:?}", e);
                if err_str.contains("WriteConflict") || err_str.contains("ReadWriteConflict") {
                    last_err = Some(e);
                    // Small backoff
                    std::thread::sleep(std::time::Duration::from_micros(100));
                    continue;
                }
                return Err(e);
            }
        }
    }
    // Return last error or a generic internal error
    Err(last_err.unwrap_or_else(|| {
        StrataError::internal("Max retries exceeded on write conflict")
    }))
}

// =============================================================================
// Concurrent Create Tests
// =============================================================================

/// Test concurrent run creation
#[test]
fn test_concurrent_creates() {
    let db = create_inmemory_db();
    let substrate = Arc::new(create_substrate(db));

    let num_threads = 10;
    let runs_per_thread = 10;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let s = Arc::clone(&substrate);
            thread::spawn(move || {
                let mut created = Vec::new();
                for _ in 0..runs_per_thread {
                    let (info, _) = s.run_create(None, None).unwrap();
                    created.push(info.run_id);
                }
                created
            })
        })
        .collect();

    let mut all_runs = Vec::new();
    for handle in handles {
        all_runs.extend(handle.join().unwrap());
    }

    // All runs should be unique
    let unique_count = {
        let mut unique = all_runs.clone();
        unique.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        unique.dedup();
        unique.len()
    };

    assert_eq!(unique_count, num_threads * runs_per_thread);

    // All runs should exist
    for run_id in &all_runs {
        assert!(substrate.run_exists(run_id).unwrap());
    }
}

// =============================================================================
// Concurrent Status Update Tests
// =============================================================================

/// Test concurrent status updates to different runs
#[test]
fn test_concurrent_status_updates() {
    let db = create_inmemory_db();
    let substrate = Arc::new(create_substrate(db));

    // Create runs first
    let runs: Vec<_> = (0..20)
        .map(|_| substrate.run_create(None, None).unwrap().0.run_id)
        .collect();

    let substrate_clone = Arc::clone(&substrate);
    let runs_clone = runs.clone();

    // Half the threads close runs, half pause them
    let close_handle = {
        let s = Arc::clone(&substrate_clone);
        let runs_to_close: Vec<_> = runs_clone.iter().take(10).cloned().collect();
        thread::spawn(move || {
            for run in runs_to_close {
                let _ = s.run_close(&run);
            }
        })
    };

    let pause_handle = {
        let s = Arc::clone(&substrate_clone);
        let runs_to_pause: Vec<_> = runs_clone.iter().skip(10).cloned().collect();
        thread::spawn(move || {
            for run in runs_to_pause {
                let _ = s.run_pause(&run);
            }
        })
    };

    close_handle.join().unwrap();
    pause_handle.join().unwrap();

    // Verify all runs are in expected states
    for (i, run) in runs.iter().enumerate() {
        let info = substrate.run_get(run).unwrap().unwrap();
        if i < 10 {
            assert!(matches!(
                info.value.state,
                strata_api::substrate::RunState::Completed
            ));
        } else {
            assert!(matches!(
                info.value.state,
                strata_api::substrate::RunState::Paused
            ));
        }
    }
}

// =============================================================================
// Concurrent Tag Tests
// =============================================================================

/// Test concurrent tag modifications
///
/// NOTE: With OCC, concurrent modifications may conflict. Uses retry logic.
#[test]
fn test_concurrent_tag_modifications() {
    let db = create_inmemory_db();
    let substrate = Arc::new(create_substrate(db));

    let (info, _) = substrate.run_create(None, None).unwrap();
    let run_id = info.run_id;

    let num_threads = 5;
    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let s = Arc::clone(&substrate);
            let rid = run_id.clone();
            thread::spawn(move || -> StrataResult<strata_core::Version> {
                let tag = format!("tag_{}", i);
                // Retry on conflict - OCC may reject concurrent writes
                retry_on_conflict(10, || s.run_add_tags(&rid, &[tag.clone()]))
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap().unwrap();
    }

    // All tags should be present
    let tags = substrate.run_get_tags(&run_id).unwrap();
    for i in 0..num_threads {
        let expected = format!("tag_{}", i);
        assert!(tags.contains(&expected), "Missing tag: {}", expected);
    }
}

// =============================================================================
// Read During Write Tests
// =============================================================================

/// Test reading while writing
#[test]
fn test_read_during_write() {
    let db = create_inmemory_db();
    let substrate = Arc::new(create_substrate(db));

    let (info, _) = substrate.run_create(None, None).unwrap();
    let run_id = info.run_id;

    let read_substrate = Arc::clone(&substrate);
    let write_substrate = Arc::clone(&substrate);
    let read_run_id = run_id.clone();
    let write_run_id = run_id.clone();

    // Writer thread
    let write_handle = thread::spawn(move || {
        for i in 0..50 {
            let meta = obj([("counter", Value::Int(i))]);
            write_substrate.run_update_metadata(&write_run_id, meta).unwrap();
        }
    });

    // Reader thread
    let read_handle = thread::spawn(move || {
        let mut read_count = 0;
        for _ in 0..100 {
            if read_substrate.run_get(&read_run_id).unwrap().is_some() {
                read_count += 1;
            }
        }
        read_count
    });

    write_handle.join().unwrap();
    let read_count = read_handle.join().unwrap();

    // All reads should succeed
    assert_eq!(read_count, 100);
}

// =============================================================================
// Lost Update Tests
// =============================================================================

/// Test no lost updates with concurrent modifications
///
/// NOTE: With OCC, concurrent modifications may conflict. Uses retry logic
/// to ensure all updates eventually succeed. This tests that with proper
/// retry handling, no updates are lost.
#[test]
fn test_no_lost_updates() {
    let db = create_inmemory_db();
    let substrate = Arc::new(create_substrate(db));

    // Create a run to update
    let (info, _) = substrate.run_create(None, None).unwrap();
    let run_id = info.run_id;

    // Add initial tag
    substrate.run_add_tags(&run_id, &["initial".to_string()]).unwrap();

    let num_threads = 10;
    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let s = Arc::clone(&substrate);
            let rid = run_id.clone();
            thread::spawn(move || -> StrataResult<strata_core::Version> {
                let tag = format!("concurrent_{}", i);
                // Retry on conflict - OCC may reject concurrent writes
                retry_on_conflict(10, || s.run_add_tags(&rid, &[tag.clone()]))
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap().unwrap();
    }

    // All tags should be present (no lost updates with retry)
    let tags = substrate.run_get_tags(&run_id).unwrap();

    assert!(tags.contains(&"initial".to_string()));
    for i in 0..num_threads {
        let expected = format!("concurrent_{}", i);
        assert!(tags.contains(&expected), "Lost update: {}", expected);
    }
}
