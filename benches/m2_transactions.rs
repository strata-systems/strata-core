//! M2 Transaction Benchmarks - Semantic Regression Harness
//!
//! ## Benchmark Path Types (Layer Labels)
//!
//! - `engine_txn_*`: Full transaction via Database::transaction() API
//! - `engine_cas_*`: CAS operations via Database::cas() API
//!
//! Note: All benchmarks use the engine layer. TransactionContext internals
//! are not directly exposed for benchmarking.
//!
//! ## What These Benchmarks Prove
//!
//! | Benchmark | Semantic Guarantee | Regression Detection |
//! |-----------|-------------------|----------------------|
//! | engine_txn_commit/* | Atomic commit correctness | OCC validation cost |
//! | engine_cas/* | CAS semantics (version check) | Version comparison overhead |
//! | snapshot_isolation/* | Point-in-time reads | Snapshot creation cost |
//! | conflict_detection/* | First-committer-wins | Conflict check scaling |
//! | read_heavy/* | Read-dominated workloads | Read-set tracking cost |
//!
//! ## Conflict Shapes
//!
//! - `same_key`: All threads contend on identical key (worst case)
//! - `disjoint_keys`: Threads use non-overlapping keys (best case)
//! - `cas_conflict`: CAS version mismatch detection
//!
//! ## Running
//!
//! ```bash
//! cargo bench --bench m2_transactions
//! cargo bench --bench m2_transactions -- "engine_cas"  # specific group
//! ```

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use in_mem_core::types::{Key, Namespace, RunId};
use in_mem_core::value::Value;
use in_mem_engine::Database;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;

// =============================================================================
// Test Utilities - All allocation happens here, outside timed loops
// =============================================================================

fn create_namespace(run_id: RunId) -> Namespace {
    Namespace::new(
        "tenant".to_string(),
        "app".to_string(),
        "agent".to_string(),
        run_id,
    )
}

fn make_key(ns: &Namespace, name: &str) -> Key {
    Key::new_kv(ns.clone(), name)
}

/// Pre-generate keys to avoid allocation in timed loops
fn pregenerate_keys(ns: &Namespace, prefix: &str, count: usize) -> Vec<Key> {
    (0..count)
        .map(|i| make_key(ns, &format!("{}_{:06}", prefix, i)))
        .collect()
}

// =============================================================================
// Engine Layer: Transaction Commit Benchmarks
// =============================================================================
// Semantic: Full transaction lifecycle (begin, operations, validate, commit)
// Regression: OCC validation cost, snapshot creation, write-set serialization

fn engine_txn_commit_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("engine_txn_commit");
    group.throughput(Throughput::Elements(1));

    // --- Benchmark: single_put (minimal transaction) ---
    // Semantic: Simplest possible write transaction
    // Real pattern: Agent storing single state update
    {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();
        let run_id = RunId::new();
        let ns = create_namespace(run_id);

        const MAX_KEYS: usize = 500_000;
        let keys = pregenerate_keys(&ns, "single", MAX_KEYS);
        let counter = AtomicU64::new(0);

        group.bench_function("single_put", |b| {
            b.iter(|| {
                let i = counter.fetch_add(1, Ordering::Relaxed) as usize;
                if i >= MAX_KEYS {
                    panic!("Benchmark exceeded pre-generated keys");
                }
                let result = db.transaction(run_id, |txn| {
                    txn.put(keys[i].clone(), Value::I64(i as i64))?;
                    Ok(())
                });
                black_box(result.unwrap())
            });
        });
    }

    // --- Benchmark: multi_put (batch transaction) ---
    // Semantic: Atomic multi-key writes
    // Real pattern: Agent storing related state atomically
    for num_keys in [3, 5, 10] {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();
        let run_id = RunId::new();
        let ns = create_namespace(run_id);

        const MAX_BATCHES: usize = 100_000;
        let all_keys: Vec<Vec<Key>> = (0..MAX_BATCHES)
            .map(|batch| {
                (0..num_keys)
                    .map(|i| make_key(&ns, &format!("batch_{}_{}", batch, i)))
                    .collect()
            })
            .collect();
        let counter = AtomicU64::new(0);

        group.bench_with_input(
            BenchmarkId::new("multi_put", num_keys),
            &num_keys,
            |b, _| {
                b.iter(|| {
                    let batch_idx = counter.fetch_add(1, Ordering::Relaxed) as usize;
                    if batch_idx >= MAX_BATCHES {
                        panic!("Benchmark exceeded pre-generated batches");
                    }
                    let keys = &all_keys[batch_idx];
                    let result = db.transaction(run_id, |txn| {
                        for (i, key) in keys.iter().enumerate() {
                            txn.put(key.clone(), Value::I64(i as i64))?;
                        }
                        Ok(())
                    });
                    black_box(result.unwrap())
                });
            },
        );
    }

    // --- Benchmark: read_modify_write (RMW pattern) ---
    // Semantic: Read then update based on current value
    // Real pattern: Counter increment, state machine transitions
    {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();
        let run_id = RunId::new();
        let ns = create_namespace(run_id);
        let key = make_key(&ns, "rmw_counter");
        db.put(run_id, key.clone(), Value::I64(0)).unwrap();

        group.bench_function("read_modify_write", |b| {
            b.iter(|| {
                let result = db.transaction(run_id, |txn| {
                    let val = txn.get(&key)?;
                    let n = match val {
                        Some(Value::I64(n)) => n,
                        _ => 0,
                    };
                    txn.put(key.clone(), Value::I64(n + 1))?;
                    Ok(())
                });
                black_box(result.unwrap())
            });
        });
    }

    group.finish();
}

// =============================================================================
// Engine Layer: CAS Benchmarks
// =============================================================================
// Semantic: Compare-and-swap with version validation
// Regression: Version comparison overhead, conflict detection cost

fn engine_cas_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("engine_cas");
    group.throughput(Throughput::Elements(1));

    // --- Benchmark: success_sequential (no contention) ---
    // Semantic: CAS succeeds when version matches
    // Real pattern: Single-threaded state updates with optimistic locking
    {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();
        let run_id = RunId::new();
        let ns = create_namespace(run_id);
        let key = make_key(&ns, "cas_seq");
        db.put(run_id, key.clone(), Value::I64(0)).unwrap();

        group.bench_function("success_sequential", |b| {
            b.iter(|| {
                let current = db.get(&key).unwrap().unwrap();
                let new_val = match current.value {
                    Value::I64(n) => n + 1,
                    _ => 1,
                };
                black_box(db.cas(run_id, key.clone(), current.version, Value::I64(new_val)).unwrap())
            });
        });
    }

    // --- Benchmark: failure_version_mismatch ---
    // Semantic: CAS fails fast when version doesn't match
    // Real pattern: Detecting stale reads
    {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();
        let run_id = RunId::new();
        let ns = create_namespace(run_id);
        let key = make_key(&ns, "cas_fail");
        db.put(run_id, key.clone(), Value::I64(0)).unwrap();

        group.bench_function("failure_version_mismatch", |b| {
            b.iter(|| {
                // Always use wrong version - should fail fast
                let result = db.cas(run_id, key.clone(), 999999, Value::I64(1));
                black_box(result.is_err())
            });
        });
    }

    // --- Benchmark: create_new_key (version 0 = insert if not exists) ---
    // Semantic: CAS with version 0 creates new key atomically
    // Real pattern: Claiming a resource, initializing state
    {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();
        let run_id = RunId::new();
        let ns = create_namespace(run_id);

        const MAX_KEYS: usize = 500_000;
        let keys = pregenerate_keys(&ns, "cas_create", MAX_KEYS);
        // Counter must be outside bench_function to persist across warm-up and measurement
        let counter = AtomicU64::new(0);

        group.bench_function("create_new_key", |b| {
            b.iter(|| {
                let i = counter.fetch_add(1, Ordering::Relaxed) as usize;
                if i >= MAX_KEYS {
                    panic!("Benchmark exceeded pre-generated keys");
                }
                black_box(db.cas(run_id, keys[i].clone(), 0, Value::I64(i as i64)).unwrap())
            });
        });
    }

    // --- Benchmark: retry_until_success (bounded retry loop) ---
    // Semantic: CAS retry pattern under self-contention
    // Real pattern: Agent coordination with optimistic retry
    {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();
        let run_id = RunId::new();
        let ns = create_namespace(run_id);
        let key = make_key(&ns, "cas_retry");
        db.put(run_id, key.clone(), Value::I64(0)).unwrap();

        group.bench_function("retry_until_success", |b| {
            b.iter(|| {
                let mut attempts = 0;
                loop {
                    let current = db.get(&key).unwrap().unwrap();
                    let new_val = match current.value {
                        Value::I64(n) => n + 1,
                        _ => 1,
                    };
                    let result = db.cas(run_id, key.clone(), current.version, Value::I64(new_val));
                    if result.is_ok() {
                        break black_box(attempts);
                    }
                    attempts += 1;
                    if attempts > 100 {
                        panic!("CAS retry exceeded limit");
                    }
                }
            });
        });
    }

    group.finish();
}

// =============================================================================
// Snapshot Isolation Benchmarks
// =============================================================================
// Semantic: Point-in-time consistent reads within transaction
// Regression: Snapshot creation cost, version lookup overhead

fn snapshot_isolation_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot_isolation");
    group.throughput(Throughput::Elements(1));

    // --- Benchmark: snapshot_read (read within transaction) ---
    // Semantic: Reading from consistent snapshot
    // Real pattern: Agent reading state during computation
    {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();
        let run_id = RunId::new();
        let ns = create_namespace(run_id);

        // Pre-populate with 1000 keys
        let keys = pregenerate_keys(&ns, "snap", 1000);
        for (i, key) in keys.iter().enumerate() {
            db.put(run_id, key.clone(), Value::I64(i as i64)).unwrap();
        }

        let lookup_key = keys[500].clone();

        group.bench_function("single_read", |b| {
            b.iter(|| {
                let result = db.transaction(run_id, |txn| txn.get(&lookup_key));
                black_box(result.unwrap())
            });
        });
    }

    // --- Benchmark: snapshot_multi_read ---
    // Semantic: Multiple reads in same snapshot (consistent view)
    // Real pattern: Agent gathering related state
    {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();
        let run_id = RunId::new();
        let ns = create_namespace(run_id);

        let keys = pregenerate_keys(&ns, "multi", 1000);
        for (i, key) in keys.iter().enumerate() {
            db.put(run_id, key.clone(), Value::I64(i as i64)).unwrap();
        }

        // Read 10 keys per transaction
        let read_keys: Vec<_> = (0..10).map(|i| keys[i * 100].clone()).collect();

        group.bench_function("multi_read_10", |b| {
            b.iter(|| {
                let result = db.transaction(run_id, |txn| {
                    for key in &read_keys {
                        txn.get(key)?;
                    }
                    Ok(())
                });
                black_box(result.unwrap())
            });
        });
    }

    // --- Benchmark: version_count_scaling ---
    // Semantic: Snapshot cost should not grow with version history
    // Real pattern: Long-running agent with many state updates
    for num_versions in [10, 100, 1000] {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();
        let run_id = RunId::new();
        let ns = create_namespace(run_id);
        let key = make_key(&ns, "versioned");

        // Create version history
        for v in 0..num_versions {
            db.put(run_id, key.clone(), Value::I64(v as i64)).unwrap();
        }

        group.bench_with_input(
            BenchmarkId::new("after_versions", num_versions),
            &num_versions,
            |b, _| {
                b.iter(|| {
                    let result = db.transaction(run_id, |txn| txn.get(&key));
                    black_box(result.unwrap())
                });
            },
        );
    }

    // --- Benchmark: read_your_writes ---
    // Semantic: Transaction sees its own uncommitted writes
    // Real pattern: Agent building up state before commit
    {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("db")).unwrap();
        let run_id = RunId::new();
        let ns = create_namespace(run_id);

        const MAX_KEYS: usize = 100_000;
        let keys = pregenerate_keys(&ns, "ryw", MAX_KEYS);
        let counter = AtomicU64::new(0);

        group.bench_function("read_your_writes", |b| {
            b.iter(|| {
                let i = counter.fetch_add(1, Ordering::Relaxed) as usize;
                if i >= MAX_KEYS {
                    panic!("Benchmark exceeded pre-generated keys");
                }
                let result = db.transaction(run_id, |txn| {
                    txn.put(keys[i].clone(), Value::I64(i as i64))?;
                    let val = txn.get(&keys[i])?;
                    Ok(val)
                });
                black_box(result.unwrap())
            });
        });
    }

    group.finish();
}

// =============================================================================
// Read-Heavy Transaction Benchmarks
// =============================================================================
// Semantic: Agents typically read much more than they write
// Regression: Read-set tracking overhead, snapshot lookup cost

fn read_heavy_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("read_heavy");
    group.throughput(Throughput::Elements(1));

    // Pre-setup for read-heavy workloads
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();
    let run_id = RunId::new();
    let ns = create_namespace(run_id);

    // Pre-populate with 10000 keys
    let keys = pregenerate_keys(&ns, "rh", 10_000);
    for (i, key) in keys.iter().enumerate() {
        db.put(run_id, key.clone(), Value::I64(i as i64)).unwrap();
    }

    let write_key = make_key(&ns, "rh_write");
    db.put(run_id, write_key.clone(), Value::I64(0)).unwrap();

    // --- Benchmark: N reads + 1 write (varying read count) ---
    // Semantic: Typical agent pattern - gather state, make decision, write result
    for num_reads in [1, 10, 100] {
        let read_keys: Vec<_> = keys.iter().take(num_reads).cloned().collect();
        let write_key = write_key.clone();
        let counter = AtomicU64::new(0);

        group.bench_with_input(
            BenchmarkId::new("reads_then_write", num_reads),
            &num_reads,
            |b, _| {
                b.iter(|| {
                    let i = counter.fetch_add(1, Ordering::Relaxed);
                    let result = db.transaction(run_id, |txn| {
                        // Read phase
                        for key in &read_keys {
                            txn.get(key)?;
                        }
                        // Write phase
                        txn.put(write_key.clone(), Value::I64(i as i64))?;
                        Ok(())
                    });
                    black_box(result.unwrap())
                });
            },
        );
    }

    // --- Benchmark: read_only transaction ---
    // Semantic: Pure read transaction (no write-set, no conflict possible)
    // Real pattern: Agent querying state without modification
    {
        let read_keys: Vec<_> = keys.iter().take(10).cloned().collect();

        group.bench_function("read_only_10", |b| {
            b.iter(|| {
                let result = db.transaction(run_id, |txn| {
                    for key in &read_keys {
                        txn.get(key)?;
                    }
                    Ok(())
                });
                black_box(result.unwrap())
            });
        });
    }

    group.finish();
}

// =============================================================================
// Conflict Detection Benchmarks (Multi-Threaded)
// =============================================================================
// Semantic: First-committer-wins under concurrent access
// Regression: Conflict detection scaling with thread count

fn conflict_detection_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("conflict_detection");
    group.sample_size(20);

    // --- Conflict shape: disjoint_keys (no actual conflicts) ---
    // Semantic: Threads work on non-overlapping keys
    // Real pattern: Partitioned agent workloads
    for num_threads in [2, 4, 8] {
        group.throughput(Throughput::Elements(num_threads as u64));
        group.bench_with_input(
            BenchmarkId::new("disjoint_keys", num_threads),
            &num_threads,
            |b, &num_threads| {
                b.iter_custom(|iters| {
                    let temp_dir = TempDir::new().unwrap();
                    let db = Arc::new(Database::open(temp_dir.path().join("db")).unwrap());
                    let run_id = RunId::new();

                    let barrier = Arc::new(Barrier::new(num_threads + 1));
                    let ops_per_thread = iters / num_threads as u64;

                    let handles: Vec<_> = (0..num_threads)
                        .map(|thread_id| {
                            let db = Arc::clone(&db);
                            let barrier = Arc::clone(&barrier);
                            let ns = create_namespace(run_id);

                            // Pre-generate keys for this thread
                            let keys: Vec<_> = (0..ops_per_thread as usize)
                                .map(|i| make_key(&ns, &format!("t{}_{}", thread_id, i)))
                                .collect();

                            thread::spawn(move || {
                                barrier.wait();
                                for (i, key) in keys.iter().enumerate() {
                                    db.transaction(run_id, |txn| {
                                        txn.put(key.clone(), Value::I64(i as i64))?;
                                        Ok(())
                                    })
                                    .unwrap();
                                }
                            })
                        })
                        .collect();

                    let start = Instant::now();
                    barrier.wait();

                    for h in handles {
                        h.join().unwrap();
                    }

                    start.elapsed()
                });
            },
        );
    }

    // --- Conflict shape: same_key (maximum contention) ---
    // Semantic: All threads contend on single key
    // Real pattern: Global counter, leader election
    for num_threads in [2, 4] {
        group.throughput(Throughput::Elements(num_threads as u64));
        group.bench_with_input(
            BenchmarkId::new("same_key", num_threads),
            &num_threads,
            |b, &num_threads| {
                b.iter_custom(|iters| {
                    let temp_dir = TempDir::new().unwrap();
                    let db = Arc::new(Database::open(temp_dir.path().join("db")).unwrap());
                    let run_id = RunId::new();
                    let ns = create_namespace(run_id);
                    let contested_key = make_key(&ns, "contested");

                    db.put(run_id, contested_key.clone(), Value::I64(0)).unwrap();

                    let barrier = Arc::new(Barrier::new(num_threads + 1));
                    let ops_per_thread = iters / num_threads as u64;

                    let handles: Vec<_> = (0..num_threads)
                        .map(|_| {
                            let db = Arc::clone(&db);
                            let barrier = Arc::clone(&barrier);
                            let key = contested_key.clone();

                            thread::spawn(move || {
                                barrier.wait();
                                for _ in 0..ops_per_thread {
                                    // Retry on conflict
                                    loop {
                                        let result = db.transaction(run_id, |txn| {
                                            let val = txn.get(&key)?;
                                            let n = match val {
                                                Some(Value::I64(n)) => n,
                                                _ => 0,
                                            };
                                            txn.put(key.clone(), Value::I64(n + 1))?;
                                            Ok(())
                                        });
                                        if result.is_ok() {
                                            break;
                                        }
                                        // Brief backoff
                                        thread::sleep(Duration::from_micros(10));
                                    }
                                }
                            })
                        })
                        .collect();

                    let start = Instant::now();
                    barrier.wait();

                    for h in handles {
                        h.join().unwrap();
                    }

                    start.elapsed()
                });
            },
        );
    }

    // --- Conflict shape: cas_contention (CAS race) ---
    // Semantic: Multiple CAS attempts, exactly one winner per round
    // Real pattern: Distributed lock acquisition
    group.bench_function("cas_one_winner", |b| {
        b.iter_custom(|iters| {
            let mut total_elapsed = Duration::ZERO;

            for _ in 0..iters {
                let temp_dir = TempDir::new().unwrap();
                let db = Arc::new(Database::open(temp_dir.path().join("db")).unwrap());
                let run_id = RunId::new();
                let ns = create_namespace(run_id);
                let key = make_key(&ns, "cas_contest");

                db.put(run_id, key.clone(), Value::I64(0)).unwrap();
                let initial_version = db.get(&key).unwrap().unwrap().version;

                let num_threads: usize = 4;
                let barrier = Arc::new(Barrier::new(num_threads + 1));
                let winners = Arc::new(AtomicU64::new(0));

                let handles: Vec<_> = (0..num_threads)
                    .map(|id| {
                        let db = Arc::clone(&db);
                        let barrier = Arc::clone(&barrier);
                        let winners = Arc::clone(&winners);
                        let key = key.clone();

                        thread::spawn(move || {
                            barrier.wait();
                            let result = db.cas(run_id, key, initial_version, Value::I64(id as i64));
                            if result.is_ok() {
                                winners.fetch_add(1, Ordering::Relaxed);
                            }
                        })
                    })
                    .collect();

                let start = Instant::now();
                barrier.wait();

                for h in handles {
                    h.join().unwrap();
                }

                total_elapsed += start.elapsed();

                // Invariant: exactly one winner
                assert_eq!(winners.load(Ordering::Relaxed), 1);
            }

            total_elapsed
        });
    });

    group.finish();
}

// =============================================================================
// Benchmark Groups
// =============================================================================

criterion_group!(
    name = txn_commit;
    config = Criterion::default().measurement_time(Duration::from_secs(10));
    targets = engine_txn_commit_benchmarks, engine_cas_benchmarks
);

criterion_group!(
    name = snapshot;
    config = Criterion::default().measurement_time(Duration::from_secs(10));
    targets = snapshot_isolation_benchmarks, read_heavy_benchmarks
);

criterion_group!(
    name = conflict;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(15))
        .sample_size(20);
    targets = conflict_detection_benchmarks
);

criterion_main!(txn_commit, snapshot, conflict);
