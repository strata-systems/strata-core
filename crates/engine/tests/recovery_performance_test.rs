//! Recovery Performance Tests
//!
//! Tests that verify recovery performance with realistic workloads:
//! - Realistic payload sizes (JSON-like data, not trivial strings)
//! - Multiple writes per transaction
//! - Full data verification (not just samples)
//! - Stress tests for memory pressure and scalability
//!
//! Performance context: This is an in-memory database. The recovery process:
//! 1. Reads WAL from disk (I/O bound)
//! 2. Deserializes entries (CPU bound)
//! 3. Writes to in-memory HashMap (very fast)
//!
//! Targets are set based on realistic expectations for this architecture.

use chrono::Utc;
use in_mem_core::types::{Key, Namespace, RunId};
use in_mem_core::value::Value;
use in_mem_core::Storage;
use in_mem_durability::wal::{DurabilityMode, WALEntry};
use in_mem_engine::Database;
use std::collections::HashSet;
use std::time::Instant;
use tempfile::TempDir;

fn now() -> i64 {
    Utc::now().timestamp()
}

/// Generate a realistic JSON-like payload of approximately the given size
fn generate_payload(id: u64, target_size: usize) -> Vec<u8> {
    // Simulate a realistic JSON payload with structured data
    let base = format!(
        r#"{{"id":{},"type":"event","user":"user_{}","session":"sess_{}"}}"#,
        id,
        id % 10000,
        id
    );

    if base.len() >= target_size {
        base.into_bytes()
    } else {
        // Pad with realistic-looking data to reach target size
        let padding_needed = target_size.saturating_sub(base.len()).saturating_sub(30);
        let padding = "x".repeat(padding_needed);
        format!(
            r#"{{"id":{},"type":"event","user":"user_{}","payload":"{}"}}"#,
            id,
            id % 10000,
            padding
        )
        .into_bytes()
    }
}

/// Test: 10K transactions with realistic payloads (~500 bytes each)
/// Verifies ALL data after recovery, not just samples
#[test]
fn test_recovery_10k_realistic_payloads() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("perf_10k_realistic");

    let run_id = RunId::new();
    let ns = Namespace::new(
        "tenant".to_string(),
        "app".to_string(),
        "agent".to_string(),
        run_id,
    );

    let txn_count: u64 = 10_000;
    let payload_size = 500; // 500 bytes per value - realistic for JSON events

    // Write 10K committed transactions with realistic payloads
    println!(
        "Writing {} transactions with {}B payloads...",
        txn_count, payload_size
    );
    let write_start = Instant::now();
    {
        let db =
            Database::open_with_mode(&db_path, DurabilityMode::Async { interval_ms: 100 }).unwrap();

        let wal = db.wal();
        let mut wal_guard = wal.lock().unwrap();

        for i in 0..txn_count {
            wal_guard
                .append(&WALEntry::BeginTxn {
                    txn_id: i,
                    run_id,
                    timestamp: now(),
                })
                .unwrap();

            wal_guard
                .append(&WALEntry::Write {
                    run_id,
                    key: Key::new_kv(ns.clone(), format!("event_{}", i)),
                    value: Value::Bytes(generate_payload(i, payload_size)),
                    version: i + 1,
                })
                .unwrap();

            wal_guard
                .append(&WALEntry::CommitTxn { txn_id: i, run_id })
                .unwrap();
        }

        drop(wal_guard);
        db.flush().unwrap();
    }
    let write_duration = write_start.elapsed();
    println!("Write completed in {:?}", write_duration);

    // Check WAL file size
    let wal_path = db_path.join("wal/current.wal");
    let file_size = std::fs::metadata(&wal_path).unwrap().len();
    println!("WAL file size: {:.2} MB", file_size as f64 / 1_048_576.0);

    // Recover and verify ALL entries
    println!("Starting recovery with full verification...");
    let recovery_start = Instant::now();
    let db = Database::open(&db_path).unwrap();
    let recovery_duration = recovery_start.elapsed();

    // Full verification - check every single entry
    let verify_start = Instant::now();
    for i in 0..txn_count {
        let key = Key::new_kv(ns.clone(), format!("event_{}", i));
        let val = db
            .storage()
            .get(&key)
            .unwrap()
            .unwrap_or_else(|| panic!("event_{} missing after recovery", i));

        if let Value::Bytes(bytes) = val.value {
            // Payload should be at least 100 bytes (reasonable JSON size)
            assert!(
                bytes.len() >= 100,
                "event_{} payload too small: {} bytes",
                i,
                bytes.len()
            );
        } else {
            panic!("event_{} has wrong value type", i);
        }
    }
    let verify_duration = verify_start.elapsed();

    assert_eq!(db.storage().current_version(), txn_count);

    let throughput = txn_count as f64 / recovery_duration.as_secs_f64();
    let mb_per_sec = (file_size as f64 / 1_048_576.0) / recovery_duration.as_secs_f64();

    println!("\n=== Results ===");
    println!("Recovery time: {:?}", recovery_duration);
    println!("Verification time: {:?}", verify_duration);
    println!("Throughput: {:.0} txns/sec", throughput);
    println!("Disk read rate: {:.1} MB/sec", mb_per_sec);

    // Performance targets for in-memory DB with realistic payloads
    // WAL reading + deserialization should still be fast
    assert!(
        recovery_duration.as_secs() < 5,
        "Recovery too slow: {:?} (expected < 5s)",
        recovery_duration
    );

    // With 500B payloads, throughput will be lower than trivial payloads
    // but should still be > 5000 txns/sec for an in-memory store
    assert!(
        throughput > 5000.0,
        "Throughput too low: {:.0} txns/sec (expected > 5000)",
        throughput
    );
}

/// Test: Multi-write transactions (5 writes per txn, 3K txns = 15K writes)
/// Tests transaction grouping and multiple storage operations per txn
#[test]
fn test_recovery_multi_write_transactions() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("perf_multi_write");

    let run_id = RunId::new();
    let ns = Namespace::new(
        "tenant".to_string(),
        "app".to_string(),
        "agent".to_string(),
        run_id,
    );

    let txn_count: u64 = 3_000;
    let writes_per_txn: u64 = 5;
    let payload_size = 200;

    println!(
        "Writing {} transactions with {} writes each ({} total writes)...",
        txn_count,
        writes_per_txn,
        txn_count * writes_per_txn
    );

    // Write multi-write transactions
    {
        let db =
            Database::open_with_mode(&db_path, DurabilityMode::Async { interval_ms: 100 }).unwrap();

        let wal = db.wal();
        let mut wal_guard = wal.lock().unwrap();

        let mut version: u64 = 1;
        for i in 0..txn_count {
            wal_guard
                .append(&WALEntry::BeginTxn {
                    txn_id: i,
                    run_id,
                    timestamp: now(),
                })
                .unwrap();

            // Multiple writes per transaction
            for j in 0..writes_per_txn {
                wal_guard
                    .append(&WALEntry::Write {
                        run_id,
                        key: Key::new_kv(ns.clone(), format!("txn_{}_key_{}", i, j)),
                        value: Value::Bytes(generate_payload(i * writes_per_txn + j, payload_size)),
                        version,
                    })
                    .unwrap();
                version += 1;
            }

            wal_guard
                .append(&WALEntry::CommitTxn { txn_id: i, run_id })
                .unwrap();
        }

        drop(wal_guard);
        db.flush().unwrap();
    }

    // Recover and verify
    let recovery_start = Instant::now();
    let db = Database::open(&db_path).unwrap();
    let recovery_duration = recovery_start.elapsed();

    // Verify all writes
    let mut verified = 0u64;
    for i in 0..txn_count {
        for j in 0..writes_per_txn {
            let key = Key::new_kv(ns.clone(), format!("txn_{}_key_{}", i, j));
            assert!(
                db.storage().get(&key).unwrap().is_some(),
                "txn_{}_key_{} missing",
                i,
                j
            );
            verified += 1;
        }
    }

    let total_writes = txn_count * writes_per_txn;
    assert_eq!(db.storage().current_version(), total_writes);
    assert_eq!(verified, total_writes);

    let writes_per_sec = total_writes as f64 / recovery_duration.as_secs_f64();

    println!("Recovery time: {:?}", recovery_duration);
    println!(
        "Recovered {} writes ({:.0} writes/sec)",
        total_writes, writes_per_sec
    );

    assert!(
        recovery_duration.as_secs() < 5,
        "Multi-write recovery too slow: {:?}",
        recovery_duration
    );
}

/// Test: Incomplete transactions are correctly discarded
/// Writes 2K complete + 500 incomplete, verifies only complete are recovered
#[test]
fn test_recovery_incomplete_transactions() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("perf_incomplete");

    let run_id = RunId::new();
    let ns = Namespace::new(
        "tenant".to_string(),
        "app".to_string(),
        "agent".to_string(),
        run_id,
    );

    let complete_count: u64 = 2_000;
    let incomplete_count: u64 = 500;
    let payload_size = 300;

    // Write mix of complete and incomplete
    {
        let db =
            Database::open_with_mode(&db_path, DurabilityMode::Async { interval_ms: 100 }).unwrap();

        let wal = db.wal();
        let mut wal_guard = wal.lock().unwrap();

        // Complete transactions
        for i in 0..complete_count {
            wal_guard
                .append(&WALEntry::BeginTxn {
                    txn_id: i,
                    run_id,
                    timestamp: now(),
                })
                .unwrap();

            wal_guard
                .append(&WALEntry::Write {
                    run_id,
                    key: Key::new_kv(ns.clone(), format!("complete_{}", i)),
                    value: Value::Bytes(generate_payload(i, payload_size)),
                    version: i + 1,
                })
                .unwrap();

            wal_guard
                .append(&WALEntry::CommitTxn { txn_id: i, run_id })
                .unwrap();
        }

        // Incomplete transactions (no CommitTxn - simulates crash)
        for i in 0..incomplete_count {
            let txn_id = complete_count + i;
            wal_guard
                .append(&WALEntry::BeginTxn {
                    txn_id,
                    run_id,
                    timestamp: now(),
                })
                .unwrap();

            wal_guard
                .append(&WALEntry::Write {
                    run_id,
                    key: Key::new_kv(ns.clone(), format!("incomplete_{}", i)),
                    value: Value::Bytes(generate_payload(i, payload_size)),
                    version: txn_id + 1,
                })
                .unwrap();
            // NO CommitTxn
        }

        drop(wal_guard);
        db.flush().unwrap();
    }

    // Recover
    let recovery_start = Instant::now();
    let db = Database::open(&db_path).unwrap();
    let recovery_duration = recovery_start.elapsed();

    // Verify all complete transactions exist
    for i in 0..complete_count {
        let key = Key::new_kv(ns.clone(), format!("complete_{}", i));
        assert!(
            db.storage().get(&key).unwrap().is_some(),
            "complete_{} should exist",
            i
        );
    }

    // Verify all incomplete transactions were discarded
    for i in 0..incomplete_count {
        let key = Key::new_kv(ns.clone(), format!("incomplete_{}", i));
        assert!(
            db.storage().get(&key).unwrap().is_none(),
            "incomplete_{} should NOT exist",
            i
        );
    }

    assert_eq!(db.storage().current_version(), complete_count);

    println!(
        "Recovered {} complete, discarded {} incomplete in {:?}",
        complete_count, incomplete_count, recovery_duration
    );

    assert!(
        recovery_duration.as_secs() < 3,
        "Incomplete txn handling too slow: {:?}",
        recovery_duration
    );
}

/// Test: Multiple namespaces (simulates multi-tenant workload)
#[test]
fn test_recovery_multiple_namespaces() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("perf_multi_ns");

    let namespace_count = 10;
    let txns_per_namespace: u64 = 500;
    let payload_size = 256;

    // Create namespaces with different run_ids
    let namespaces: Vec<(RunId, Namespace)> = (0..namespace_count)
        .map(|i| {
            let run_id = RunId::new();
            let ns = Namespace::new(
                format!("tenant_{}", i),
                "app".to_string(),
                "agent".to_string(),
                run_id,
            );
            (run_id, ns)
        })
        .collect();

    println!(
        "Writing {} namespaces × {} txns = {} total txns...",
        namespace_count,
        txns_per_namespace,
        namespace_count as u64 * txns_per_namespace
    );

    // Write transactions across all namespaces
    {
        let db =
            Database::open_with_mode(&db_path, DurabilityMode::Async { interval_ms: 100 }).unwrap();

        let wal = db.wal();
        let mut wal_guard = wal.lock().unwrap();

        let mut txn_id: u64 = 0;
        let mut version: u64 = 1;

        for (run_id, ns) in &namespaces {
            for i in 0..txns_per_namespace {
                wal_guard
                    .append(&WALEntry::BeginTxn {
                        txn_id,
                        run_id: *run_id,
                        timestamp: now(),
                    })
                    .unwrap();

                wal_guard
                    .append(&WALEntry::Write {
                        run_id: *run_id,
                        key: Key::new_kv(ns.clone(), format!("key_{}", i)),
                        value: Value::Bytes(generate_payload(txn_id, payload_size)),
                        version,
                    })
                    .unwrap();

                wal_guard
                    .append(&WALEntry::CommitTxn {
                        txn_id,
                        run_id: *run_id,
                    })
                    .unwrap();

                txn_id += 1;
                version += 1;
            }
        }

        drop(wal_guard);
        db.flush().unwrap();
    }

    // Recover
    let recovery_start = Instant::now();
    let db = Database::open(&db_path).unwrap();
    let recovery_duration = recovery_start.elapsed();

    // Verify each namespace has correct data
    for (_, ns) in &namespaces {
        for i in 0..txns_per_namespace {
            let key = Key::new_kv(ns.clone(), format!("key_{}", i));
            assert!(
                db.storage().get(&key).unwrap().is_some(),
                "{:?} key_{} missing",
                ns.tenant,
                i
            );
        }
    }

    let total_txns = namespace_count as u64 * txns_per_namespace;
    assert_eq!(db.storage().current_version(), total_txns);

    println!(
        "Recovered {} txns across {} namespaces in {:?}",
        total_txns, namespace_count, recovery_duration
    );

    assert!(
        recovery_duration.as_secs() < 5,
        "Multi-namespace recovery too slow: {:?}",
        recovery_duration
    );
}

// =============================================================================
// STRESS TESTS
// =============================================================================

/// Stress test: Large values (64KB each) - tests memory allocation pressure
#[test]
fn stress_test_large_values() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("stress_large_values");

    let run_id = RunId::new();
    let ns = Namespace::new(
        "tenant".to_string(),
        "app".to_string(),
        "agent".to_string(),
        run_id,
    );

    let txn_count: u64 = 500;
    let value_size = 64 * 1024; // 64KB per value = ~32MB total

    println!(
        "Stress test: {} transactions with {}KB values (~{}MB total)...",
        txn_count,
        value_size / 1024,
        (txn_count as usize * value_size) / 1_048_576
    );

    // Write large values
    {
        let db =
            Database::open_with_mode(&db_path, DurabilityMode::Async { interval_ms: 100 }).unwrap();

        let wal = db.wal();
        let mut wal_guard = wal.lock().unwrap();

        for i in 0..txn_count {
            wal_guard
                .append(&WALEntry::BeginTxn {
                    txn_id: i,
                    run_id,
                    timestamp: now(),
                })
                .unwrap();

            // Generate large value with verifiable pattern
            let mut value = vec![0u8; value_size];
            // First 8 bytes = transaction ID for verification
            value[..8].copy_from_slice(&i.to_le_bytes());
            // Fill rest with pattern
            for (j, byte) in value.iter_mut().enumerate().skip(8) {
                *byte = ((i + j as u64) % 256) as u8;
            }

            wal_guard
                .append(&WALEntry::Write {
                    run_id,
                    key: Key::new_kv(ns.clone(), format!("large_{}", i)),
                    value: Value::Bytes(value),
                    version: i + 1,
                })
                .unwrap();

            wal_guard
                .append(&WALEntry::CommitTxn { txn_id: i, run_id })
                .unwrap();
        }

        drop(wal_guard);
        db.flush().unwrap();
    }

    // Check WAL size
    let wal_path = db_path.join("wal/current.wal");
    let file_size = std::fs::metadata(&wal_path).unwrap().len();
    println!("WAL file size: {:.1} MB", file_size as f64 / 1_048_576.0);

    // Recover
    let recovery_start = Instant::now();
    let db = Database::open(&db_path).unwrap();
    let recovery_duration = recovery_start.elapsed();

    // Verify ALL large values with content check
    for i in 0..txn_count {
        let key = Key::new_kv(ns.clone(), format!("large_{}", i));
        let val = db
            .storage()
            .get(&key)
            .unwrap()
            .unwrap_or_else(|| panic!("large_{} missing", i));

        if let Value::Bytes(bytes) = val.value {
            assert_eq!(bytes.len(), value_size, "large_{} wrong size", i);

            // Verify transaction ID in first 8 bytes
            let stored_id = u64::from_le_bytes(bytes[..8].try_into().unwrap());
            assert_eq!(stored_id, i, "large_{} content corrupted", i);
        } else {
            panic!("large_{} wrong type", i);
        }
    }

    assert_eq!(db.storage().current_version(), txn_count);

    let mb_per_sec = (file_size as f64 / 1_048_576.0) / recovery_duration.as_secs_f64();

    println!("Recovery time: {:?}", recovery_duration);
    println!("Throughput: {:.1} MB/sec", mb_per_sec);

    // Large value recovery is I/O bound
    // In debug mode, expect at least 10 MB/sec (release mode would be faster)
    assert!(
        mb_per_sec > 10.0,
        "Large value throughput too low: {:.1} MB/sec (expected > 10)",
        mb_per_sec
    );
}

/// Stress test: Many unique keys - tests hash table scaling
#[test]
fn stress_test_many_keys() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("stress_many_keys");

    let run_id = RunId::new();
    let ns = Namespace::new(
        "tenant".to_string(),
        "app".to_string(),
        "agent".to_string(),
        run_id,
    );

    let txn_count: u64 = 20_000;
    let writes_per_txn: u64 = 3;
    let total_keys = txn_count * writes_per_txn; // 60K unique keys

    println!(
        "Stress test: {} unique keys ({} txns × {} writes)...",
        total_keys, txn_count, writes_per_txn
    );

    // Write many keys
    {
        let db =
            Database::open_with_mode(&db_path, DurabilityMode::Async { interval_ms: 100 }).unwrap();

        let wal = db.wal();
        let mut wal_guard = wal.lock().unwrap();

        let mut version: u64 = 1;
        for i in 0..txn_count {
            wal_guard
                .append(&WALEntry::BeginTxn {
                    txn_id: i,
                    run_id,
                    timestamp: now(),
                })
                .unwrap();

            for j in 0..writes_per_txn {
                let key_id = i * writes_per_txn + j;
                wal_guard
                    .append(&WALEntry::Write {
                        run_id,
                        key: Key::new_kv(ns.clone(), format!("k{:08}", key_id)),
                        value: Value::I64(key_id as i64),
                        version,
                    })
                    .unwrap();
                version += 1;
            }

            wal_guard
                .append(&WALEntry::CommitTxn { txn_id: i, run_id })
                .unwrap();
        }

        drop(wal_guard);
        db.flush().unwrap();
    }

    // Recover
    let recovery_start = Instant::now();
    let db = Database::open(&db_path).unwrap();
    let recovery_duration = recovery_start.elapsed();

    // Verify key count using sampling + version
    let mut verified = 0u64;
    for i in (0..total_keys).step_by(100) {
        // Sample every 100th key
        let key = Key::new_kv(ns.clone(), format!("k{:08}", i));
        assert!(
            db.storage().get(&key).unwrap().is_some(),
            "k{:08} missing",
            i
        );
        verified += 1;
    }

    assert_eq!(db.storage().current_version(), total_keys);

    let keys_per_sec = total_keys as f64 / recovery_duration.as_secs_f64();

    println!("Recovery time: {:?}", recovery_duration);
    println!("Verified {} sample keys", verified);
    println!("Throughput: {:.0} keys/sec", keys_per_sec);

    // Should handle 60K keys efficiently
    assert!(
        recovery_duration.as_secs() < 10,
        "Many keys recovery too slow: {:?}",
        recovery_duration
    );
}

/// Stress test: Key overwrites - same keys written multiple times
#[test]
fn stress_test_key_overwrites() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("stress_overwrites");

    let run_id = RunId::new();
    let ns = Namespace::new(
        "tenant".to_string(),
        "app".to_string(),
        "agent".to_string(),
        run_id,
    );

    let unique_keys = 1_000;
    let overwrites_per_key = 10;
    let total_txns = unique_keys * overwrites_per_key;

    println!(
        "Stress test: {} keys overwritten {} times each ({} total txns)...",
        unique_keys, overwrites_per_key, total_txns
    );

    // Write with overwrites
    {
        let db =
            Database::open_with_mode(&db_path, DurabilityMode::Async { interval_ms: 100 }).unwrap();

        let wal = db.wal();
        let mut wal_guard = wal.lock().unwrap();

        let mut txn_id: u64 = 0;
        let mut version: u64 = 1;

        for round in 0..overwrites_per_key {
            for key_idx in 0..unique_keys {
                wal_guard
                    .append(&WALEntry::BeginTxn {
                        txn_id,
                        run_id,
                        timestamp: now(),
                    })
                    .unwrap();

                wal_guard
                    .append(&WALEntry::Write {
                        run_id,
                        key: Key::new_kv(ns.clone(), format!("key_{}", key_idx)),
                        value: Value::I64((round * unique_keys + key_idx) as i64),
                        version,
                    })
                    .unwrap();

                wal_guard
                    .append(&WALEntry::CommitTxn { txn_id, run_id })
                    .unwrap();

                txn_id += 1;
                version += 1;
            }
        }

        drop(wal_guard);
        db.flush().unwrap();
    }

    // Recover
    let recovery_start = Instant::now();
    let db = Database::open(&db_path).unwrap();
    let recovery_duration = recovery_start.elapsed();

    // Verify final values (should be from last round)
    let last_round = overwrites_per_key - 1;
    for key_idx in 0..unique_keys {
        let key = Key::new_kv(ns.clone(), format!("key_{}", key_idx));
        let val = db
            .storage()
            .get(&key)
            .unwrap()
            .unwrap_or_else(|| panic!("key_{} missing", key_idx));

        if let Value::I64(v) = val.value {
            let expected = (last_round * unique_keys + key_idx) as i64;
            assert_eq!(v, expected, "key_{} has wrong value", key_idx);
        } else {
            panic!("key_{} wrong type", key_idx);
        }
    }

    // Should have unique_keys entries, not total_txns
    // Version should be total writes though
    assert_eq!(db.storage().current_version(), total_txns as u64);

    println!("Recovery time: {:?}", recovery_duration);
    println!(
        "Verified {} keys have final values from round {}",
        unique_keys, last_round
    );

    assert!(
        recovery_duration.as_secs() < 5,
        "Overwrite recovery too slow: {:?}",
        recovery_duration
    );
}

/// Stress test: Mixed operations (writes + deletes)
#[test]
fn stress_test_mixed_operations() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("stress_mixed");

    let run_id = RunId::new();
    let ns = Namespace::new(
        "tenant".to_string(),
        "app".to_string(),
        "agent".to_string(),
        run_id,
    );

    let initial_writes = 5_000u64;
    let deletes = 2_000u64;
    let final_writes = 1_000u64;

    println!(
        "Stress test: {} writes, {} deletes, {} more writes...",
        initial_writes, deletes, final_writes
    );

    // Track which keys should exist at end
    let mut expected_keys: HashSet<u64> = HashSet::new();

    {
        let db =
            Database::open_with_mode(&db_path, DurabilityMode::Async { interval_ms: 100 }).unwrap();

        let wal = db.wal();
        let mut wal_guard = wal.lock().unwrap();

        let mut txn_id: u64 = 0;
        let mut version: u64 = 1;

        // Initial writes
        for i in 0..initial_writes {
            wal_guard
                .append(&WALEntry::BeginTxn {
                    txn_id,
                    run_id,
                    timestamp: now(),
                })
                .unwrap();

            wal_guard
                .append(&WALEntry::Write {
                    run_id,
                    key: Key::new_kv(ns.clone(), format!("key_{}", i)),
                    value: Value::I64(i as i64),
                    version,
                })
                .unwrap();

            wal_guard
                .append(&WALEntry::CommitTxn { txn_id, run_id })
                .unwrap();

            expected_keys.insert(i);
            txn_id += 1;
            version += 1;
        }

        // Delete some keys (every other key in first 4000)
        for i in (0..deletes * 2).step_by(2) {
            wal_guard
                .append(&WALEntry::BeginTxn {
                    txn_id,
                    run_id,
                    timestamp: now(),
                })
                .unwrap();

            wal_guard
                .append(&WALEntry::Delete {
                    run_id,
                    key: Key::new_kv(ns.clone(), format!("key_{}", i)),
                    version,
                })
                .unwrap();

            wal_guard
                .append(&WALEntry::CommitTxn { txn_id, run_id })
                .unwrap();

            expected_keys.remove(&i);
            txn_id += 1;
            version += 1;
        }

        // More writes (new keys)
        for i in 0..final_writes {
            let key_id = initial_writes + i;
            wal_guard
                .append(&WALEntry::BeginTxn {
                    txn_id,
                    run_id,
                    timestamp: now(),
                })
                .unwrap();

            wal_guard
                .append(&WALEntry::Write {
                    run_id,
                    key: Key::new_kv(ns.clone(), format!("key_{}", key_id)),
                    value: Value::I64(key_id as i64),
                    version,
                })
                .unwrap();

            wal_guard
                .append(&WALEntry::CommitTxn { txn_id, run_id })
                .unwrap();

            expected_keys.insert(key_id);
            txn_id += 1;
            version += 1;
        }

        drop(wal_guard);
        db.flush().unwrap();
    }

    // Recover
    let recovery_start = Instant::now();
    let db = Database::open(&db_path).unwrap();
    let recovery_duration = recovery_start.elapsed();

    // Verify expected keys exist
    for &key_id in &expected_keys {
        let key = Key::new_kv(ns.clone(), format!("key_{}", key_id));
        assert!(
            db.storage().get(&key).unwrap().is_some(),
            "key_{} should exist",
            key_id
        );
    }

    // Verify deleted keys don't exist (sample check)
    for i in (0..100).step_by(2) {
        let key = Key::new_kv(ns.clone(), format!("key_{}", i));
        assert!(
            db.storage().get(&key).unwrap().is_none(),
            "key_{} should be deleted",
            i
        );
    }

    println!("Recovery time: {:?}", recovery_duration);
    println!("Expected {} keys, verified presence", expected_keys.len());

    assert!(
        recovery_duration.as_secs() < 5,
        "Mixed ops recovery too slow: {:?}",
        recovery_duration
    );
}

// =============================================================================
// BENCHMARK (run with --ignored)
// =============================================================================

/// Benchmark: 50K realistic transactions
/// Run with: cargo test -p in-mem-engine benchmark_50k --release -- --ignored --nocapture
#[test]
#[ignore]
fn benchmark_50k_realistic() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("bench_50k");

    let run_id = RunId::new();
    let ns = Namespace::new(
        "tenant".to_string(),
        "app".to_string(),
        "agent".to_string(),
        run_id,
    );

    let txn_count: u64 = 50_000;
    let payload_size = 512;

    println!("=== Benchmark: {} Realistic Transactions ===\n", txn_count);
    println!("Payload size: {} bytes", payload_size);

    // Write phase
    println!("\nWriting transactions...");
    let write_start = Instant::now();
    {
        let db =
            Database::open_with_mode(&db_path, DurabilityMode::Async { interval_ms: 100 }).unwrap();

        let wal = db.wal();
        let mut wal_guard = wal.lock().unwrap();

        for i in 0..txn_count {
            wal_guard
                .append(&WALEntry::BeginTxn {
                    txn_id: i,
                    run_id,
                    timestamp: now(),
                })
                .unwrap();

            wal_guard
                .append(&WALEntry::Write {
                    run_id,
                    key: Key::new_kv(ns.clone(), format!("event_{}", i)),
                    value: Value::Bytes(generate_payload(i, payload_size)),
                    version: i + 1,
                })
                .unwrap();

            wal_guard
                .append(&WALEntry::CommitTxn { txn_id: i, run_id })
                .unwrap();

            if i > 0 && i % 10_000 == 0 {
                println!("  Written {} txns...", i);
            }
        }

        drop(wal_guard);
        db.flush().unwrap();
    }
    let write_duration = write_start.elapsed();

    // File stats
    let wal_path = db_path.join("wal/current.wal");
    let file_size = std::fs::metadata(&wal_path).unwrap().len();

    println!("Write complete: {:?}", write_duration);
    println!("WAL size: {:.2} MB", file_size as f64 / 1_048_576.0);

    // Recovery phase
    println!("\nRecovering...");
    let recovery_start = Instant::now();
    let db = Database::open(&db_path).unwrap();
    let recovery_duration = recovery_start.elapsed();

    // Verification phase
    println!("Verifying all {} entries...", txn_count);
    let verify_start = Instant::now();
    for i in 0..txn_count {
        let key = Key::new_kv(ns.clone(), format!("event_{}", i));
        assert!(db.storage().get(&key).unwrap().is_some());
    }
    let verify_duration = verify_start.elapsed();

    assert_eq!(db.storage().current_version(), txn_count);

    // Results
    let txn_throughput = txn_count as f64 / recovery_duration.as_secs_f64();
    let mb_throughput = (file_size as f64 / 1_048_576.0) / recovery_duration.as_secs_f64();
    let latency_us = recovery_duration.as_micros() as f64 / txn_count as f64;

    println!("\n=== Results ===");
    println!("Transactions:     {}", txn_count);
    println!("Payload size:     {} bytes", payload_size);
    println!("WAL file size:    {:.2} MB", file_size as f64 / 1_048_576.0);
    println!("Write time:       {:?}", write_duration);
    println!("Recovery time:    {:?}", recovery_duration);
    println!("Verify time:      {:?}", verify_duration);
    println!("Txn throughput:   {:.0} txns/sec", txn_throughput);
    println!("Data throughput:  {:.1} MB/sec", mb_throughput);
    println!("Latency per txn:  {:.2} µs", latency_us);
}
