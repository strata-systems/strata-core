# Benchmark Execution Prompt

Use this prompt to systematically execute the benchmark suite and document results.

---

## Execution Prompt

```
Execute the in-mem benchmark suite in the following order. Document all results.
Do NOT optimize during this run - just measure and record.
```

## Phase 1: Verify Correctness First

Before benchmarking, ensure the system is correct:

```bash
# Run invariant tests
cargo test --test m1_m2_comprehensive invariant -- --nocapture

# If any invariant test fails, STOP. Do not benchmark a broken system.
```

If tests fail, open an issue with label `bug`, `priority:critical` before proceeding.

## Phase 2: M1 Storage Benchmarks

Run M1 benchmarks (single-threaded, storage layer + WAL):

```bash
cargo bench --bench m1_storage -- --noplot
```

Record results for:

### engine_get (Read Performance)
- [ ] `engine_get/hot_key` - Single key repeated access
- [ ] `engine_get/uniform` - Random keys from full keyspace
- [ ] `engine_get/working_set_100` - Hot subset of 100 keys
- [ ] `engine_get/miss` - Key not found path

### engine_put (Write Performance)
- [ ] `engine_put/insert` - New key creation + WAL
- [ ] `engine_put/overwrite_hot_key` - Update single key
- [ ] `engine_put/overwrite_uniform` - Random updates

### engine_delete (Delete Performance)
- [ ] `engine_delete/existing_key` - Tombstone creation
- [ ] `engine_delete/nonexistent_key` - No-op efficiency

### engine_value_size (Serialization Scaling)
- [ ] `engine_value_size/put_bytes/64`
- [ ] `engine_value_size/put_bytes/256`
- [ ] `engine_value_size/put_bytes/1024`
- [ ] `engine_value_size/put_bytes/4096`

### engine_key_scaling (O(log n) Guarantee)
- [ ] `engine_key_scaling/get_at_scale/1000`
- [ ] `engine_key_scaling/get_at_scale/10000`
- [ ] `engine_key_scaling/get_at_scale/100000`

### wal_recovery (Recovery Performance)
- [ ] `wal_recovery/insert_only/10000`
- [ ] `wal_recovery/insert_only/50000`
- [ ] `wal_recovery/overwrite_heavy` - Version history replay
- [ ] `wal_recovery/delete_heavy` - Tombstone replay

### M1 Expected Ranges

| Benchmark | Stretch | Acceptable | Concern |
|-----------|---------|------------|---------|
| engine_get (any pattern) | >1M ops/s | >100K ops/s | <50K ops/s |
| engine_put (insert) | >10K ops/s | >1K ops/s | <500 ops/s |
| engine_put (overwrite) | >50K ops/s | >10K ops/s | <5K ops/s |
| wal_recovery/50K | <500ms | <2s | >5s |
| engine_key_scaling (500K) | <1µs | <5µs | >10µs |

## Phase 3: M2 Transaction Benchmarks

Run M2 benchmarks (transactions, OCC, snapshots):

```bash
cargo bench --bench m2_transactions -- --noplot
```

Record results for:

### engine_txn_commit (Transaction Overhead)
- [ ] `engine_txn_commit/single_put` - Minimal txn cost
- [ ] `engine_txn_commit/multi_put/5`
- [ ] `engine_txn_commit/multi_put/10`
- [ ] `engine_txn_commit/multi_put/50`
- [ ] `engine_txn_commit/read_modify_write` - RMW atomicity

### engine_cas (Compare-and-Swap)
- [ ] `engine_cas/success_sequential` - Happy path
- [ ] `engine_cas/failure_version_mismatch` - Fast failure
- [ ] `engine_cas/create_new_key` - Atomic creation
- [ ] `engine_cas/retry_until_success` - Retry pattern

### snapshot_isolation (MVCC)
- [ ] `snapshot_isolation/single_read` - Snapshot creation cost
- [ ] `snapshot_isolation/multi_read_10` - Multi-key reads
- [ ] `snapshot_isolation/after_versions/10`
- [ ] `snapshot_isolation/after_versions/100`
- [ ] `snapshot_isolation/after_versions/1000`
- [ ] `snapshot_isolation/read_your_writes` - Pending write lookup

### read_heavy (Typical Agent Pattern)
- [ ] `read_heavy/reads_then_write/1_read`
- [ ] `read_heavy/reads_then_write/10_reads`
- [ ] `read_heavy/reads_then_write/100_reads`
- [ ] `read_heavy/read_only_10` - Pure read transaction

### conflict_detection (Concurrency)
- [ ] `conflict_detection/disjoint_keys/2_threads`
- [ ] `conflict_detection/disjoint_keys/4_threads`
- [ ] `conflict_detection/same_key/2_threads`
- [ ] `conflict_detection/same_key/4_threads`
- [ ] `conflict_detection/cas_one_winner`

### M2 Expected Ranges

| Benchmark | Stretch | Acceptable | Concern |
|-----------|---------|------------|---------|
| engine_txn_commit (single) | >5K txns/s | >1K txns/s | <500 txns/s |
| engine_cas (success) | >50K ops/s | >10K ops/s | <5K ops/s |
| snapshot_isolation/single_read | >50K ops/s | >10K ops/s | <5K ops/s |
| conflict_detection/disjoint (4t) | >80% scaling | >50% scaling | <30% scaling |
| conflict_detection/same_key (4t) | >2K txns/s | >500 txns/s | <200 txns/s |

## Phase 4: Save Baseline

If results are acceptable, save as baseline:

```bash
cargo bench --bench m1_storage -- --save-baseline current
cargo bench --bench m2_transactions -- --save-baseline current
```

## Phase 5: Document Results

Create a benchmark report with this format:

```markdown
# Benchmark Results - [DATE]

## Environment
- OS: [uname -a]
- CPU: [model, cores]
- Memory: [total RAM]
- Rust version: [rustc --version]

## M1 Storage Results

| Benchmark | Result | vs Acceptable | Status |
|-----------|--------|---------------|--------|
| engine_get/hot_key | X ops/s | +Y% | OK/CONCERN |
| engine_get/uniform | X ops/s | +Y% | OK/CONCERN |
| engine_put/insert | X ops/s | +Y% | OK/CONCERN |
| wal_recovery/insert_only/50000 | Xms | +Y% | OK/CONCERN |
| ... | ... | ... | ... |

## M2 Transaction Results

| Benchmark | Result | vs Acceptable | Status |
|-----------|--------|---------------|--------|
| engine_txn_commit/single_put | X txns/s | +Y% | OK/CONCERN |
| engine_cas/success_sequential | X ops/s | +Y% | OK/CONCERN |
| snapshot_isolation/single_read | X ops/s | +Y% | OK/CONCERN |
| conflict_detection/disjoint_keys/4_threads | X% scaling | +Y% | OK/CONCERN |
| ... | ... | ... | ... |

## Observations

- [Any unexpected results]
- [Bottlenecks identified]
- [Access pattern insights]

## Action Items

- [ ] [Any issues to investigate]
- [ ] [Optimizations to consider later]
```

## Phase 6: Re-verify Correctness

After benchmarking, run invariant tests again:

```bash
cargo test --test m1_m2_comprehensive invariant -- --nocapture
```

If tests pass: benchmark results are valid.
If tests fail: benchmark results are INVALID. Something broke during the run.

---

## Interpretation Guide

### Reading Criterion Output

```
engine_get/hot_key
                        time:   [200.45 ns 201.23 ns 202.01 ns]
                        thrpt:  [4.9502 Melem/s 4.9694 Melem/s 4.9887 Melem/s]
```

- Three numbers: [lower bound, estimate, upper bound] at 95% confidence
- Use the **middle number** (estimate) for reporting
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

### Status Categories

- **OK**: Meets or exceeds "acceptable" threshold
- **MARGINAL**: Within 20% of "acceptable" threshold
- **CONCERN**: Below "acceptable" threshold
- **CRITICAL**: Below 50% of "acceptable" threshold

### What NOT to Do

1. Do NOT optimize based on a single benchmark run
2. Do NOT compare to other systems yet (we're not stable)
3. Do NOT chase "stretch" goals before "acceptable" is met
4. Do NOT ignore invariant test failures

---

## Quick Commands

```bash
# Full suite (both M1 and M2)
cargo bench --bench m1_storage --bench m2_transactions -- --noplot

# Just M1
cargo bench --bench m1_storage -- --noplot

# Just M2
cargo bench --bench m2_transactions -- --noplot

# By category
cargo bench --bench m1_storage -- "engine_get"
cargo bench --bench m1_storage -- "engine_put"
cargo bench --bench m1_storage -- "wal_recovery"
cargo bench --bench m2_transactions -- "engine_txn_commit"
cargo bench --bench m2_transactions -- "engine_cas"
cargo bench --bench m2_transactions -- "snapshot_isolation"
cargo bench --bench m2_transactions -- "conflict_detection"

# By access pattern
cargo bench --bench m1_storage -- "hot_key"
cargo bench --bench m1_storage -- "uniform"
cargo bench --bench m1_storage -- "working_set"

# Compare to baseline
cargo bench --bench m1_storage -- --baseline current
cargo bench --bench m2_transactions -- --baseline current

# Run with more samples (slower, more accurate)
cargo bench --bench m1_storage -- --sample-size 200

# Run invariant tests
cargo test --test m1_m2_comprehensive invariant
```

---

## Issue Template (for concerns)

If any benchmark shows "CONCERN" or "CRITICAL" status:

```markdown
## Benchmark Performance Issue

**Benchmark**: [name, e.g., engine_get/uniform]
**Result**: [X ops/s]
**Expected**: [>Y ops/s (acceptable)]
**Gap**: [Z% below acceptable]
**Layer**: [engine/wal/snapshot/conflict]
**Access Pattern**: [hot_key/uniform/working_set/miss]

### Environment
- OS:
- Rust version:

### Reproduction
```bash
cargo bench --bench [m1_storage|m2_transactions] -- "[benchmark_name]"
```

### Notes
[Any observations about the result]
```

Labels: `performance`, `needs-investigation`
```

---

## Success Criteria

A benchmark run is successful if:

- [ ] All invariant tests pass before AND after benchmarking
- [ ] All M1 benchmarks meet "acceptable" thresholds
- [ ] All M2 benchmarks meet "acceptable" thresholds
- [ ] No benchmark shows >20% regression from baseline (if baseline exists)
- [ ] Results are documented with layer and access pattern context

If any criterion is not met, document the gap and create issues for investigation.
Do NOT block on performance issues - correctness comes first.
