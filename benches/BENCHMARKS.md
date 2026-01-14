# in-mem Benchmark Suite - Semantic Regression Harness

**Philosophy:** Benchmarks exist to detect semantic regressions, not chase arbitrary numbers.
MVP success is semantic correctness first, performance second.

---

## Benchmark Path Types (Layer Labels)

Every benchmark explicitly labels what layer it measures:

| Prefix | Layer | What It Measures |
|--------|-------|------------------|
| `engine_*` | Engine API | Full path via `Database` API (includes WAL, locks) |
| `wal_*` | WAL/Recovery | Recovery and durability operations |
| `snapshot_*` | Snapshot | Snapshot isolation semantics |
| `conflict_*` | Concurrency | Multi-threaded conflict detection |

**Why this matters:** You must not accidentally compare storage-layer numbers to transaction-layer numbers.

---

## Key Access Patterns

Benchmarks explicitly label their access pattern:

| Pattern | Description | Real Agent Use Case |
|---------|-------------|---------------------|
| `hot_key` | Single key, repeated access | Config reads, counters |
| `uniform` | Random keys from full keyspace | Arbitrary state access |
| `working_set_N` | Small subset (N keys) | Frequently accessed subset |
| `miss` | Key not found | Error path, existence checks |

**Why this matters:** Hot-key benchmarks lie about real-world performance.

---

## Benchmark Structure

```
benches/
  m1_storage.rs         # M1: Storage + WAL primitives
  m2_transactions.rs    # M2: OCC + Snapshot Isolation
  BENCHMARKS.md         # This file
  BENCHMARK_EXECUTION.md # Execution guide
```

### Why milestone-scoped benchmarks?

1. **Focus:** Only benchmark what's implemented
2. **Avoid distraction:** Don't optimize for features that don't exist yet
3. **Clear ownership:** Each benchmark file maps to a feature set
4. **Regression detection:** Changes to M1 run M1 benchmarks

---

## What Each Benchmark Proves

### M1 Storage Benchmarks

| Benchmark | Semantic Guarantee | Regression Detection | Agent Pattern |
|-----------|-------------------|----------------------|---------------|
| `engine_get/hot_key` | Read path correctness | Lock overhead | Config reads |
| `engine_get/uniform` | Random access performance | BTreeMap scaling | State lookups |
| `engine_get/working_set_100` | Hot subset performance | Cache behavior | Frequent state |
| `engine_get/miss` | Miss path efficiency | Error handling | Existence checks |
| `engine_put/insert` | New key + WAL durability | fsync cost | New state creation |
| `engine_put/overwrite_hot_key` | Update + version increment | Update path | Counter updates |
| `engine_put/overwrite_uniform` | Random updates | Write distribution | State updates |
| `engine_delete/existing_key` | Tombstone creation | Delete cost | Cleanup |
| `engine_delete/nonexistent_key` | No-op efficiency | Miss handling | Idempotent cleanup |
| `engine_value_size/put_bytes/*` | Serialization scaling | Large value cost | Blob storage |
| `engine_key_scaling/get_at_scale/*` | O(log n) guarantee | BTreeMap degradation | Large databases |
| `wal_recovery/insert_only/*` | Pure append replay | Recovery scaling | Normal restart |
| `wal_recovery/overwrite_heavy` | Version history replay | MVCC overhead | Long-running agent |
| `wal_recovery/delete_heavy` | Tombstone replay | Delete handling | Cleanup-heavy workload |

### M2 Transaction Benchmarks

| Benchmark | Semantic Guarantee | Regression Detection | Agent Pattern |
|-----------|-------------------|----------------------|---------------|
| `engine_txn_commit/single_put` | Minimal txn overhead | OCC cost | Single state update |
| `engine_txn_commit/multi_put/*` | Atomic batch commit | Write-set scaling | Related state updates |
| `engine_txn_commit/read_modify_write` | RMW atomicity | Read-set + write cost | Counter increment |
| `engine_cas/success_sequential` | CAS happy path | Version check cost | Optimistic updates |
| `engine_cas/failure_version_mismatch` | Fast failure | Conflict detection | Stale read detection |
| `engine_cas/create_new_key` | Atomic creation | Insert-if-absent | Resource claiming |
| `engine_cas/retry_until_success` | Retry pattern | Retry overhead | Coordination |
| `snapshot_isolation/single_read` | Snapshot read | Snapshot creation | State query |
| `snapshot_isolation/multi_read_10` | Consistent multi-read | Read-set tracking | Gathering state |
| `snapshot_isolation/after_versions/*` | Version scaling | MVCC overhead | Long-running system |
| `snapshot_isolation/read_your_writes` | Write visibility | Pending write lookup | Build-up before commit |
| `read_heavy/reads_then_write/*` | Read-dominated txn | Read-set scaling | Typical agent pattern |
| `read_heavy/read_only_10` | Pure read txn | No write-set overhead | Query-only |
| `conflict_detection/disjoint_keys/*` | Parallel scaling | Lock contention | Partitioned agents |
| `conflict_detection/same_key/*` | Contention handling | Conflict resolution | Global counter |
| `conflict_detection/cas_one_winner` | First-committer-wins | CAS race correctness | Lock acquisition |

---

## Target Performance

### Important Context

These targets assume:
- Single-process, in-memory
- RwLock-based concurrency
- BTreeMap-backed storage
- WAL-logged mutations (fsync per operation)
- Versioned values with snapshot isolation

**Stretch goals are optimistic.** Initial implementations may be 2-5x slower. That's fine. Correctness first.

### M1: Storage + WAL

| Operation | Stretch | Acceptable | Concern |
|-----------|---------|------------|---------|
| engine_get (any pattern) | >1M ops/s | >100K ops/s | <50K ops/s |
| engine_put (insert) | >10K ops/s | >1K ops/s | <500 ops/s |
| engine_put (overwrite) | >50K ops/s | >10K ops/s | <5K ops/s |
| wal_recovery/50K ops | <500ms | <2s | >5s |
| engine_key_scaling (500K keys) | <1µs lookup | <5µs lookup | >10µs lookup |

### M2: Transactions + OCC

| Operation | Stretch | Acceptable | Concern |
|-----------|---------|------------|---------|
| engine_txn_commit (single) | >5K txns/s | >1K txns/s | <500 txns/s |
| engine_cas (success) | >50K ops/s | >10K ops/s | <5K ops/s |
| snapshot_isolation/single_read | >50K ops/s | >10K ops/s | <5K ops/s |
| conflict_detection/disjoint (4 threads) | >80% scaling | >50% scaling | <30% scaling |
| conflict_detection/same_key (4 threads) | >2K txns/s | >500 txns/s | <200 txns/s |

---

## Running Benchmarks

### M1 Storage Benchmarks

```bash
# All M1 benchmarks
cargo bench --bench m1_storage

# By category
cargo bench --bench m1_storage -- "engine_get"
cargo bench --bench m1_storage -- "engine_put"
cargo bench --bench m1_storage -- "engine_delete"
cargo bench --bench m1_storage -- "engine_value_size"
cargo bench --bench m1_storage -- "engine_key_scaling"
cargo bench --bench m1_storage -- "wal_recovery"
```

### M2 Transaction Benchmarks

```bash
# All M2 benchmarks
cargo bench --bench m2_transactions

# By category
cargo bench --bench m2_transactions -- "engine_txn_commit"
cargo bench --bench m2_transactions -- "engine_cas"
cargo bench --bench m2_transactions -- "snapshot_isolation"
cargo bench --bench m2_transactions -- "read_heavy"
cargo bench --bench m2_transactions -- "conflict_detection"
```

### Comparison Mode

```bash
# Save baseline
cargo bench --bench m1_storage -- --save-baseline main
cargo bench --bench m2_transactions -- --save-baseline main

# Compare against baseline
cargo bench --bench m1_storage -- --baseline main
cargo bench --bench m2_transactions -- --baseline main
```

---

## Interpreting Results

### Criterion Output

```
engine_get/hot_key
                        time:   [200.45 ns 201.23 ns 202.01 ns]
                        thrpt:  [4.9502 Melem/s 4.9694 Melem/s 4.9887 Melem/s]
```

- Three numbers: [lower bound, estimate, upper bound] at 95% confidence
- `thrpt` = throughput in elements/second
- 4.97M ops/s = well above "acceptable" (>100K ops/s)

### Regression Detection

```
Performance has regressed:
  time:   [200.45 ns 210.23 ns 220.01 ns]
                        change: [+15.234% +18.901% +22.345%] (p = 0.001 < 0.05)
```

- `change` shows percentage difference from baseline
- `p < 0.05` means statistically significant
- Investigate regressions >10% on critical paths

### What to Do About Regressions

1. **<5%:** Noise, likely acceptable
2. **5-15%:** Investigate, may be acceptable tradeoff
3. **>15%:** Likely real regression, prioritize investigation
4. **>50%:** Something is seriously wrong

---

## Benchmark Honesty Checklist

For every benchmark, verify:

1. **All setup is outside the timed loop**
   - No key allocation in `b.iter()`
   - No value construction in `b.iter()`
   - No random number generation in `b.iter()`

2. **Access pattern is explicitly labeled**
   - `hot_key`, `uniform`, `working_set`, or `miss`

3. **Layer is explicitly labeled**
   - `engine_`, `wal_`, `snapshot_`, or `conflict_`

4. **Four questions answered:**
   - What semantic guarantee does this exercise?
   - What layer does it measure?
   - What regression would it detect?
   - What real agent pattern does it approximate?

---

## What's NOT Benchmarked (Yet)

### Tail Latency
- P95, P99 latency under load
- Jitter during concurrent access
- Worst-case pauses

**Why:** Requires more sophisticated harnesses. Add when correctness is proven.

### Comparison to Other Systems
- Redis, SQLite, RocksDB, etc.

**Why:** Comparisons are only meaningful after our system is stable.

---

## Adding New Benchmarks

### Template

```rust
// --- Benchmark: descriptive_name ---
// Semantic: What guarantee does this exercise?
// Real pattern: What agent behavior does this simulate?
{
    // Setup OUTSIDE bench_function
    let temp_dir = TempDir::new().unwrap();
    let db = Database::open(temp_dir.path().join("db")).unwrap();
    let keys = pregenerate_keys(&ns, "prefix", COUNT); // Pre-allocate!

    group.bench_function("descriptive_name", |b| {
        b.iter(|| {
            // ONLY the operation under test
            black_box(db.get(&keys[idx]).unwrap())
        });
    });
}
```

### Checklist for New Benchmarks

- [ ] Layer labeled in name (`engine_`, `wal_`, etc.)
- [ ] Access pattern labeled if applicable (`hot_key`, `uniform`, etc.)
- [ ] All setup outside timed loop
- [ ] Comment explains semantic guarantee
- [ ] Comment explains real agent pattern
- [ ] Four questions can be answered

---

## Invariant Validation After Benchmarks

**Performance without correctness is meaningless.**

After running benchmarks, validate invariants:

```bash
cargo test --test m1_m2_comprehensive invariant -- --nocapture
```

If benchmarks pass but invariant tests fail, the benchmarks are measuring a broken system.
