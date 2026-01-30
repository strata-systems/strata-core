//! Adversarial Tests for strata-durability
//!
//! Tests targeting edge cases, race conditions, and error paths that
//! could expose bugs in the durability layer:
//!
//! 1. Concurrent WAL operations
//! 2. JSON/Vector operation replay correctness
//! 3. Recovery edge cases (orphaned markers, incomplete transactions)
//! 4. Snapshot validation edge cases
//! 5. Batched durability mode thresholds
//!
//! These tests follow the TESTING_METHODOLOGY.md principles:
//! - Test behavior, not implementation
//! - One failure mode per test
//! - Verify values, not just is_ok()

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;

use strata_core::error::Result;
use strata_core::json::JsonPath;
use strata_core::traits::Storage;
use strata_core::types::{JsonDocId, Key, Namespace, RunId};
use strata_core::value::Value;
use strata_core::Timestamp;
use strata_durability::recovery::replay_wal;
use strata_durability::wal::{DurabilityMode, WALEntry, WAL};
use strata_storage::ShardedStore;
use tempfile::TempDir;
use uuid::Uuid;

// ============================================================================
// Test Helpers
// ============================================================================

fn now() -> Timestamp {
    Timestamp::now()
}

fn create_test_key(run_id: RunId, name: &str) -> Key {
    Key::new_kv(Namespace::for_run(run_id), name)
}

fn write_committed_transaction(
    wal: &mut WAL,
    txn_id: u64,
    run_id: RunId,
    key: &Key,
    value: Value,
    version: u64,
) -> Result<()> {
    wal.append(&WALEntry::BeginTxn {
        txn_id,
        run_id,
        timestamp: now(),
    })?;
    wal.append(&WALEntry::Write {
        run_id,
        key: key.clone(),
        value,
        version,
    })?;
    wal.append(&WALEntry::CommitTxn { txn_id, run_id })?;
    Ok(())
}

// ============================================================================
// Module 1: Concurrent WAL Operations
// ============================================================================

/// Test that concurrent WAL appends don't lose entries
///
/// Multiple threads appending simultaneously should result in all entries
/// being readable and offset tracking remaining consistent.
#[test]
fn test_concurrent_wal_appends_no_data_loss() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("concurrent.wal");

    let wal = Arc::new(parking_lot::Mutex::new(
        WAL::open(&wal_path, DurabilityMode::Strict).unwrap(),
    ));
    let run_id = RunId::new();

    const NUM_THREADS: usize = 8;
    const ENTRIES_PER_THREAD: usize = 100;

    let barrier = Arc::new(Barrier::new(NUM_THREADS));
    let entries_written = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|thread_id| {
            let wal = Arc::clone(&wal);
            let barrier = Arc::clone(&barrier);
            let entries_written = Arc::clone(&entries_written);

            thread::spawn(move || {
                barrier.wait();

                for i in 0..ENTRIES_PER_THREAD {
                    let txn_id = (thread_id * ENTRIES_PER_THREAD + i) as u64;
                    let entry = WALEntry::BeginTxn {
                        txn_id,
                        run_id,
                        timestamp: now(),
                    };

                    let mut wal_guard = wal.lock();
                    wal_guard.append(&entry).unwrap();
                    entries_written.fetch_add(1, Ordering::SeqCst);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all entries are readable
    let wal_guard = wal.lock();
    let entries = wal_guard.read_all().unwrap();

    let expected = NUM_THREADS * ENTRIES_PER_THREAD;
    assert_eq!(
        entries.len(),
        expected,
        "Expected {} entries, got {}. Data loss detected!",
        expected,
        entries.len()
    );
}

/// Test that offset tracking remains consistent under concurrent appends
#[test]
fn test_concurrent_wal_offset_consistency() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("offset.wal");

    let wal = Arc::new(parking_lot::Mutex::new(
        WAL::open(&wal_path, DurabilityMode::Strict).unwrap(),
    ));
    let run_id = RunId::new();

    const NUM_THREADS: usize = 4;
    const ENTRIES_PER_THREAD: usize = 50;

    let barrier = Arc::new(Barrier::new(NUM_THREADS));
    let offsets: Arc<parking_lot::Mutex<Vec<u64>>> =
        Arc::new(parking_lot::Mutex::new(Vec::new()));

    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|thread_id| {
            let wal = Arc::clone(&wal);
            let barrier = Arc::clone(&barrier);
            let offsets = Arc::clone(&offsets);

            thread::spawn(move || {
                barrier.wait();

                for i in 0..ENTRIES_PER_THREAD {
                    let txn_id = (thread_id * ENTRIES_PER_THREAD + i) as u64;
                    let entry = WALEntry::BeginTxn {
                        txn_id,
                        run_id,
                        timestamp: now(),
                    };

                    let mut wal_guard = wal.lock();
                    let offset = wal_guard.append(&entry).unwrap();
                    offsets.lock().push(offset);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify offsets are unique and monotonically increase when sorted
    let mut all_offsets = offsets.lock().clone();
    let original_len = all_offsets.len();
    all_offsets.sort();
    all_offsets.dedup();

    assert_eq!(
        all_offsets.len(),
        original_len,
        "Duplicate offsets detected! {} unique out of {} total",
        all_offsets.len(),
        original_len
    );

    // Verify offsets are strictly increasing
    for i in 1..all_offsets.len() {
        assert!(
            all_offsets[i] > all_offsets[i - 1],
            "Non-monotonic offsets: {} not > {}",
            all_offsets[i],
            all_offsets[i - 1]
        );
    }
}

// ============================================================================
// Module 2: JSON Operation Replay
// ============================================================================

/// Test JsonSet to non-existent document is handled gracefully
///
/// Replay should skip JsonSet operations when the target document doesn't exist.
#[test]
fn test_json_set_to_nonexistent_document_skipped() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("json_set.wal");

    let run_id = RunId::new();
    let doc_id = JsonDocId::new();
    let path = JsonPath::root().key("missing").key("deeply").key("nested");
    let value_bytes = rmp_serde::to_vec(&serde_json::json!(42)).unwrap();

    // Write JsonSet without creating document first
    {
        let mut wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();

        wal.append(&WALEntry::BeginTxn {
            txn_id: 1,
            run_id,
            timestamp: now(),
        })
        .unwrap();

        wal.append(&WALEntry::JsonSet {
            run_id,
            doc_id,
            path,
            value_bytes,
            version: 1,
        })
        .unwrap();

        wal.append(&WALEntry::CommitTxn { txn_id: 1, run_id })
            .unwrap();
    }

    // Replay should succeed without panic
    let wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();
    let storage = ShardedStore::new();
    let stats = replay_wal(&wal, &storage).unwrap();

    // JsonSet was skipped (document didn't exist)
    assert_eq!(
        stats.json_sets_applied, 0,
        "JsonSet to non-existent document should be skipped"
    );
}

/// Test JsonDelete to non-existent document is handled gracefully
#[test]
fn test_json_delete_to_nonexistent_document_skipped() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("json_del.wal");

    let run_id = RunId::new();
    let doc_id = JsonDocId::new();
    let path = JsonPath::root().key("missing");

    // Write JsonDelete without creating document first
    {
        let mut wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();

        wal.append(&WALEntry::BeginTxn {
            txn_id: 1,
            run_id,
            timestamp: now(),
        })
        .unwrap();

        wal.append(&WALEntry::JsonDelete {
            run_id,
            doc_id,
            path,
            version: 1,
        })
        .unwrap();

        wal.append(&WALEntry::CommitTxn { txn_id: 1, run_id })
            .unwrap();
    }

    // Replay should succeed without panic
    let wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();
    let storage = ShardedStore::new();
    let stats = replay_wal(&wal, &storage).unwrap();

    // JsonDelete was skipped (document didn't exist)
    assert_eq!(
        stats.json_deletes_applied, 0,
        "JsonDelete to non-existent document should be skipped"
    );
}

/// Test JsonCreate followed by JsonSet applies correctly
#[test]
fn test_json_create_then_set_replay_order() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("json_order.wal");

    let run_id = RunId::new();
    let doc_id = JsonDocId::new();
    let initial_value = serde_json::json!({"name": "test"});
    let initial_bytes = rmp_serde::to_vec(&initial_value).unwrap();

    let path = JsonPath::root().key("name");
    let new_value = serde_json::json!("updated");
    let new_bytes = rmp_serde::to_vec(&new_value).unwrap();

    // Write JsonCreate then JsonSet in same transaction
    {
        let mut wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();

        wal.append(&WALEntry::BeginTxn {
            txn_id: 1,
            run_id,
            timestamp: now(),
        })
        .unwrap();

        wal.append(&WALEntry::JsonCreate {
            run_id,
            doc_id,
            value_bytes: initial_bytes,
            version: 1,
            timestamp: now(),
        })
        .unwrap();

        wal.append(&WALEntry::JsonSet {
            run_id,
            doc_id,
            path,
            value_bytes: new_bytes,
            version: 2,
        })
        .unwrap();

        wal.append(&WALEntry::CommitTxn { txn_id: 1, run_id })
            .unwrap();
    }

    // Replay
    let wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();
    let storage = ShardedStore::new();
    let stats = replay_wal(&wal, &storage).unwrap();

    assert_eq!(stats.json_creates_applied, 1, "JsonCreate should be applied");
    assert_eq!(stats.json_sets_applied, 1, "JsonSet should be applied");
}

// ============================================================================
// Module 3: Recovery Edge Cases
// ============================================================================

/// Test orphaned CommitTxn (CommitTxn without corresponding BeginTxn)
///
/// The recovery system should handle this gracefully without crashing
/// and not count it as a recovered transaction.
#[test]
fn test_orphaned_commit_txn_handled_gracefully() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("orphan_commit.wal");

    let run_id = RunId::new();

    // Write CommitTxn without BeginTxn
    {
        let mut wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();

        // Directly write CommitTxn (orphaned)
        wal.append(&WALEntry::CommitTxn { txn_id: 999, run_id })
            .unwrap();

        // Write a valid transaction afterwards
        wal.append(&WALEntry::BeginTxn {
            txn_id: 1,
            run_id,
            timestamp: now(),
        })
        .unwrap();
        let key = create_test_key(run_id, "valid");
        wal.append(&WALEntry::Write {
            run_id,
            key,
            value: Value::Int(42),
            version: 1,
        })
        .unwrap();
        wal.append(&WALEntry::CommitTxn { txn_id: 1, run_id })
            .unwrap();
    }

    // Replay should succeed
    let wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();
    let storage = ShardedStore::new();
    let stats = replay_wal(&wal, &storage).unwrap();

    // Only the valid transaction should be counted
    assert_eq!(
        stats.txns_applied, 1,
        "Only valid transaction should be applied"
    );
}

/// Test Write without BeginTxn (orphaned operation)
#[test]
fn test_orphaned_write_counted_correctly() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("orphan_write.wal");

    let run_id = RunId::new();
    let key = create_test_key(run_id, "orphan");

    // Write operation without transaction context
    {
        let mut wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();

        // Orphaned write (no BeginTxn)
        wal.append(&WALEntry::Write {
            run_id,
            key: key.clone(),
            value: Value::Int(123),
            version: 1,
        })
        .unwrap();

        // Then a valid transaction
        wal.append(&WALEntry::BeginTxn {
            txn_id: 1,
            run_id,
            timestamp: now(),
        })
        .unwrap();
        wal.append(&WALEntry::CommitTxn { txn_id: 1, run_id })
            .unwrap();
    }

    // Replay
    let wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();
    let storage = ShardedStore::new();
    let stats = replay_wal(&wal, &storage).unwrap();

    // Orphaned entry should be counted
    assert_eq!(
        stats.orphaned_entries, 1,
        "Orphaned write should be counted"
    );

    // Orphaned write should NOT be applied
    let result = storage.get(&key).unwrap();
    assert!(
        result.is_none(),
        "Orphaned write should not be applied to storage"
    );
}

/// Test large incomplete transaction is fully discarded
///
/// An incomplete transaction with many writes should have ALL writes discarded,
/// not just some of them.
#[test]
fn test_large_incomplete_transaction_fully_discarded() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("large_incomplete.wal");

    let run_id = RunId::new();
    const NUM_WRITES: usize = 1000;

    // Write large incomplete transaction
    {
        let mut wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();

        wal.append(&WALEntry::BeginTxn {
            txn_id: 1,
            run_id,
            timestamp: now(),
        })
        .unwrap();

        for i in 0..NUM_WRITES {
            let key = create_test_key(run_id, &format!("key_{}", i));
            wal.append(&WALEntry::Write {
                run_id,
                key,
                value: Value::Int(i as i64),
                version: i as u64 + 1,
            })
            .unwrap();
        }

        // NO CommitTxn - simulate crash
    }

    // Replay
    let wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();
    let storage = ShardedStore::new();
    let stats = replay_wal(&wal, &storage).unwrap();

    // ALL writes should be discarded
    assert_eq!(
        stats.incomplete_txns, 1,
        "Should have 1 incomplete transaction"
    );
    assert_eq!(stats.writes_applied, 0, "No writes should be applied");

    // Verify no keys exist
    for i in 0..10 {
        // Spot check
        let key = create_test_key(run_id, &format!("key_{}", i));
        let result = storage.get(&key).unwrap();
        assert!(
            result.is_none(),
            "Key key_{} should not exist after incomplete txn discard",
            i
        );
    }
}

/// Test aborted transaction has its writes discarded
#[test]
fn test_aborted_transaction_writes_discarded() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("aborted.wal");

    let run_id = RunId::new();
    let key = create_test_key(run_id, "aborted_key");

    // Write and abort
    {
        let mut wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();

        wal.append(&WALEntry::BeginTxn {
            txn_id: 1,
            run_id,
            timestamp: now(),
        })
        .unwrap();

        wal.append(&WALEntry::Write {
            run_id,
            key: key.clone(),
            value: Value::Int(999),
            version: 1,
        })
        .unwrap();

        wal.append(&WALEntry::AbortTxn { txn_id: 1, run_id })
            .unwrap();
    }

    // Replay
    let wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();
    let storage = ShardedStore::new();
    let stats = replay_wal(&wal, &storage).unwrap();

    assert_eq!(stats.aborted_txns, 1, "Should count 1 aborted transaction");
    assert_eq!(stats.writes_applied, 0, "No writes should be applied");

    // Key should not exist
    let result = storage.get(&key).unwrap();
    assert!(result.is_none(), "Aborted transaction's write should not exist");
}

// ============================================================================
// Module 4: Version Tracking Edge Cases
// ============================================================================

/// Test that version tracking is correct after replay with gaps
///
/// Transactions may commit out of order, creating gaps in versions.
/// The final_version should still be the maximum version seen.
#[test]
fn test_version_tracking_with_gaps() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("version_gaps.wal");

    let run_id = RunId::new();

    // Write transactions with non-contiguous versions
    {
        let mut wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();

        // Transaction at version 5
        let key1 = create_test_key(run_id, "key1");
        write_committed_transaction(&mut wal, 1, run_id, &key1, Value::Int(1), 5).unwrap();

        // Transaction at version 100 (big gap)
        let key2 = create_test_key(run_id, "key2");
        write_committed_transaction(&mut wal, 2, run_id, &key2, Value::Int(2), 100).unwrap();

        // Transaction at version 50 (in the gap)
        let key3 = create_test_key(run_id, "key3");
        write_committed_transaction(&mut wal, 3, run_id, &key3, Value::Int(3), 50).unwrap();
    }

    // Replay
    let wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();
    let storage = ShardedStore::new();
    let stats = replay_wal(&wal, &storage).unwrap();

    // Final version should be the maximum
    assert_eq!(
        stats.final_version, 100,
        "Final version should be max seen (100)"
    );

    assert_eq!(stats.txns_applied, 3, "All 3 transactions should be applied");
}

/// Test max_txn_id tracking is correct
#[test]
fn test_max_txn_id_tracking() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("max_txn_id.wal");

    let run_id = RunId::new();

    // Write transactions with various txn_ids
    {
        let mut wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();

        let key1 = create_test_key(run_id, "key1");
        write_committed_transaction(&mut wal, 10, run_id, &key1, Value::Int(1), 1).unwrap();

        let key2 = create_test_key(run_id, "key2");
        write_committed_transaction(&mut wal, 5, run_id, &key2, Value::Int(2), 2).unwrap();

        let key3 = create_test_key(run_id, "key3");
        write_committed_transaction(&mut wal, 1000, run_id, &key3, Value::Int(3), 3).unwrap();
    }

    // Replay
    let wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();
    let storage = ShardedStore::new();
    let stats = replay_wal(&wal, &storage).unwrap();

    // Max txn_id should be the maximum
    assert_eq!(
        stats.max_txn_id, 1000,
        "Max txn_id should be max seen (1000)"
    );
}

// ============================================================================
// Module 5: Batched Durability Mode
// ============================================================================

/// Test batched mode triggers fsync after batch_size writes
#[test]
fn test_batched_mode_batch_size_trigger() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("batched_size.wal");

    let run_id = RunId::new();

    // Batched mode: fsync every 5 writes
    let mut wal = WAL::open(
        &wal_path,
        DurabilityMode::Batched {
            interval_ms: 10000, // 10 seconds - won't trigger
            batch_size: 5,
        },
    )
    .unwrap();

    // Write exactly 5 entries (should trigger fsync)
    for i in 0..5 {
        let entry = WALEntry::BeginTxn {
            txn_id: i,
            run_id,
            timestamp: now(),
        };
        wal.append(&entry).unwrap();
    }

    // Reopen to verify persistence (fsync should have happened)
    drop(wal);

    let wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();
    let entries = wal.read_all().unwrap();

    assert_eq!(
        entries.len(),
        5,
        "All 5 entries should be persisted after batch_size trigger"
    );
}

/// Test batched mode with mixed committed and incomplete transactions
///
/// Even with batched mode, only committed transactions should survive recovery.
#[test]
fn test_batched_mode_recovery_semantics() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("batched_recovery.wal");

    let run_id = RunId::new();

    // Write with batched mode
    {
        let mut wal = WAL::open(
            &wal_path,
            DurabilityMode::Batched {
                interval_ms: 100,
                batch_size: 10,
            },
        )
        .unwrap();

        // Committed transaction
        let key1 = create_test_key(run_id, "committed");
        write_committed_transaction(&mut wal, 1, run_id, &key1, Value::Int(100), 1).unwrap();

        // Incomplete transaction (no CommitTxn)
        wal.append(&WALEntry::BeginTxn {
            txn_id: 2,
            run_id,
            timestamp: now(),
        })
        .unwrap();
        let key2 = create_test_key(run_id, "incomplete");
        wal.append(&WALEntry::Write {
            run_id,
            key: key2,
            value: Value::Int(999),
            version: 2,
        })
        .unwrap();

        // Force flush to ensure all entries are on disk
        wal.fsync().unwrap();
    }

    // Replay
    let wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();
    let storage = ShardedStore::new();
    let stats = replay_wal(&wal, &storage).unwrap();

    // Only committed transaction should be applied
    assert_eq!(stats.txns_applied, 1, "Only 1 committed transaction");
    assert_eq!(stats.incomplete_txns, 1, "1 incomplete transaction");
    assert_eq!(stats.writes_applied, 1, "Only 1 write applied");
}

// ============================================================================
// Module 6: Multiple Run Isolation
// ============================================================================

/// Test that transactions from different runs don't interfere
#[test]
fn test_multiple_run_isolation_during_replay() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("multi_run.wal");

    let run_id_1 = RunId::new();
    let run_id_2 = RunId::new();

    // Write interleaved transactions from different runs
    {
        let mut wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();

        // Run 1: Complete transaction
        let key1 = create_test_key(run_id_1, "run1_key");
        write_committed_transaction(&mut wal, 1, run_id_1, &key1, Value::Int(1), 1).unwrap();

        // Run 2: Incomplete transaction
        wal.append(&WALEntry::BeginTxn {
            txn_id: 1, // Same txn_id but different run
            run_id: run_id_2,
            timestamp: now(),
        })
        .unwrap();
        let key2 = create_test_key(run_id_2, "run2_key");
        wal.append(&WALEntry::Write {
            run_id: run_id_2,
            key: key2.clone(),
            value: Value::Int(2),
            version: 1,
        })
        .unwrap();
        // No CommitTxn for run_id_2

        // Run 1: Another complete transaction
        let key3 = create_test_key(run_id_1, "run1_key2");
        write_committed_transaction(&mut wal, 2, run_id_1, &key3, Value::Int(3), 2).unwrap();
    }

    // Replay
    let wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();
    let storage = ShardedStore::new();
    let stats = replay_wal(&wal, &storage).unwrap();

    // Run 1's transactions should be applied
    assert_eq!(stats.txns_applied, 2, "2 transactions from run1 applied");

    // Run 2's incomplete transaction should be discarded
    assert_eq!(stats.incomplete_txns, 1, "1 incomplete transaction from run2");

    // Verify run1's keys exist
    let key1 = create_test_key(run_id_1, "run1_key");
    let result1 = storage.get(&key1).unwrap();
    assert!(result1.is_some(), "run1_key should exist");

    // Verify run2's key doesn't exist
    let key2 = create_test_key(run_id_2, "run2_key");
    let result2 = storage.get(&key2).unwrap();
    assert!(result2.is_none(), "run2_key should not exist (incomplete)");
}

// ============================================================================
// Module 7: Edge Cases in Entry Processing
// ============================================================================

/// Test checkpoint entry doesn't affect transaction state
#[test]
fn test_checkpoint_doesnt_affect_transactions() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("checkpoint.wal");

    let run_id = RunId::new();

    // Write transaction with checkpoint in the middle
    {
        let mut wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();

        wal.append(&WALEntry::BeginTxn {
            txn_id: 1,
            run_id,
            timestamp: now(),
        })
        .unwrap();

        let key1 = create_test_key(run_id, "before_checkpoint");
        wal.append(&WALEntry::Write {
            run_id,
            key: key1,
            value: Value::Int(1),
            version: 1,
        })
        .unwrap();

        // Checkpoint in the middle of transaction
        wal.append(&WALEntry::Checkpoint {
            snapshot_id: Uuid::new_v4(),
            version: 1,
            active_runs: vec![run_id],
        })
        .unwrap();

        let key2 = create_test_key(run_id, "after_checkpoint");
        wal.append(&WALEntry::Write {
            run_id,
            key: key2,
            value: Value::Int(2),
            version: 2,
        })
        .unwrap();

        wal.append(&WALEntry::CommitTxn { txn_id: 1, run_id })
            .unwrap();
    }

    // Replay
    let wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();
    let storage = ShardedStore::new();
    let stats = replay_wal(&wal, &storage).unwrap();

    // Both writes should be applied despite checkpoint
    assert_eq!(stats.writes_applied, 2, "Both writes should be applied");

    let key1 = create_test_key(run_id, "before_checkpoint");
    let key2 = create_test_key(run_id, "after_checkpoint");

    assert!(
        storage.get(&key1).unwrap().is_some(),
        "before_checkpoint key should exist"
    );
    assert!(
        storage.get(&key2).unwrap().is_some(),
        "after_checkpoint key should exist"
    );
}

/// Test empty transaction (BeginTxn + CommitTxn with no operations)
#[test]
fn test_empty_transaction_counted_correctly() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("empty_txn.wal");

    let run_id = RunId::new();

    // Write empty transaction
    {
        let mut wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();

        wal.append(&WALEntry::BeginTxn {
            txn_id: 1,
            run_id,
            timestamp: now(),
        })
        .unwrap();
        wal.append(&WALEntry::CommitTxn { txn_id: 1, run_id })
            .unwrap();
    }

    // Replay
    let wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();
    let storage = ShardedStore::new();
    let stats = replay_wal(&wal, &storage).unwrap();

    // Empty transaction should still be counted
    assert_eq!(
        stats.txns_applied, 1,
        "Empty transaction should be counted as applied"
    );
    assert_eq!(stats.writes_applied, 0, "No writes in empty transaction");
}

/// Test delete operation preserves version correctly
#[test]
fn test_delete_version_preserved_during_replay() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().join("delete_version.wal");

    let run_id = RunId::new();
    let key = create_test_key(run_id, "to_delete");

    // Write then delete
    {
        let mut wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();

        // Create
        write_committed_transaction(&mut wal, 1, run_id, &key, Value::Int(42), 1).unwrap();

        // Delete at version 5
        wal.append(&WALEntry::BeginTxn {
            txn_id: 2,
            run_id,
            timestamp: now(),
        })
        .unwrap();
        wal.append(&WALEntry::Delete {
            run_id,
            key: key.clone(),
            version: 5,
        })
        .unwrap();
        wal.append(&WALEntry::CommitTxn { txn_id: 2, run_id })
            .unwrap();
    }

    // Replay
    let wal = WAL::open(&wal_path, DurabilityMode::Strict).unwrap();
    let storage = ShardedStore::new();
    let stats = replay_wal(&wal, &storage).unwrap();

    assert_eq!(stats.deletes_applied, 1, "Delete should be applied");
    assert_eq!(
        stats.final_version, 5,
        "Final version should be 5 (from delete)"
    );
}
