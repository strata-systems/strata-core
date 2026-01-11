# Epic Review Process

Complete quality gate process for reviewing epics before merging to develop.

## Overview

**Purpose**: Ensure quality, integration correctness, and architectural consistency before merging epic to develop.

**When**: After all user stories in an epic are merged to the epic branch.

**Who**: Lead developer or designated reviewer (can be another Claude instance).

---

## Quick Start

```bash
# Run automated review
./scripts/review-epic.sh <epic-number>

# Fill out review template
docs/milestones/EPIC_<N>_REVIEW.md

# If approved, merge to develop
git checkout develop
git merge epic-<N>-<name>
git push origin develop
```

---

## The 5-Phase Review Process

### Phase 1: Pre-Review Validation ✅

**Automated checks that must pass:**

```bash
cargo build --all
cargo test --all
cargo clippy --all -- -D warnings
cargo fmt --all -- --check
```

**Checklist:**
- [ ] All user stories merged to epic branch
- [ ] All story acceptance criteria met
- [ ] Workspace builds without errors
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Code properly formatted
- [ ] Documentation exists for all public APIs

### Phase 2: Integration Testing 🧪

**Run comprehensive tests:**

```bash
# All tests in release mode (optimization bugs)
cargo test --all --release

# Generate coverage report
cargo tarpaulin --all --out Html
```

**Checklist:**
- [ ] All unit tests pass (no flaky tests)
- [ ] Integration tests pass
- [ ] Edge cases tested
- [ ] Error cases tested
- [ ] Serialization roundtrips tested
- [ ] Test coverage meets epic target

**Coverage targets:**
- Epic 1: ≥90% (core types)
- Epic 2-5: ≥85%

### Phase 3: Code Review 👀

**Architecture adherence:**
- [ ] Follows layered architecture (no violations)
- [ ] Dependencies flow correctly (no cycles)
- [ ] Separation of concerns maintained
- [ ] Trait abstractions used correctly
- [ ] Matches architecture specification

**Code quality:**
- [ ] No unwrap() or expect() in production code
- [ ] Comprehensive error handling (Result types)
- [ ] Public APIs documented
- [ ] Complex logic has comments
- [ ] No TODO/FIXME without GitHub issue
- [ ] Consistent naming conventions

**Testing quality:**
- [ ] Tests follow naming: `test_{module}_{behavior}_{expected}`
- [ ] Arrange-Act-Assert pattern
- [ ] One concern per test
- [ ] Descriptive assertions
- [ ] Both happy path AND error cases

### Phase 4: Documentation Review 📚

**Completeness check:**
- [ ] All public types documented
- [ ] All public functions documented
- [ ] Module-level documentation
- [ ] Doc tests compile and pass
- [ ] Examples provided
- [ ] Architecture diagrams updated (if needed)

**Verify:**
```bash
cargo doc --all --open
# Review generated documentation
```

### Phase 5: Epic-Specific Validation

Run epic-specific tests and checks. See [Epic-Specific Checklists](#epic-specific-checklists) below.

---

## Epic-Specific Checklists

### Epic 1: Workspace & Core Types

**Critical checks:**
- [ ] All 6 crates compile independently
- [ ] Key ordering supports prefix scans (CRITICAL!)
- [ ] Value enum handles all variants
- [ ] Error types cover all M1 scenarios
- [ ] Storage trait matches spec exactly
- [ ] SnapshotView trait is implementation-agnostic

**Tests to run:**
```bash
cargo test test_key_btree_ordering --nocapture
cargo test test_value_serialization_all_variants --nocapture
cargo test -p in-mem-core --all
```

### Epic 2: Storage Layer

**Critical checks:**
- [ ] UnifiedStore implements Storage trait correctly
- [ ] Version management is monotonic and thread-safe (AtomicU64)
- [ ] Secondary indices stay consistent (run_index, type_index, ttl_index)
- [ ] TTL cleanup is transactional (no direct mutations)
- [ ] TTL expiration is logical (filtered at read time)
- [ ] ClonedSnapshotView implements SnapshotView trait
- [ ] Snapshots are isolated (writes after snapshot don't appear)
- [ ] scan_prefix uses BTreeMap range queries correctly
- [ ] scan_by_run uses run_index (O(run size) not O(total))
- [ ] scan_by_type uses type_index
- [ ] Concurrent writes don't cause version collisions
- [ ] find_expired_keys uses ttl_index (O(expired) not O(total))
- [ ] Known RwLock bottleneck documented
- [ ] Known snapshot cloning cost documented
- [ ] Coverage ≥85% for storage crate

**Tests to run:**
```bash
# Core storage functionality
cargo test -p in-mem-storage test_put_and_get --nocapture
cargo test -p in-mem-storage test_version_monotonicity --nocapture
cargo test -p in-mem-storage test_delete --nocapture

# Concurrent access
cargo test -p in-mem-storage test_concurrent_writes --nocapture

# TTL functionality
cargo test -p in-mem-storage test_ttl_expiration --nocapture
cargo test -p in-mem-storage test_find_expired_keys_uses_index --nocapture
cargo test -p in-mem-storage test_ttl_cleaner --nocapture

# Secondary indices
cargo test -p in-mem-storage test_scan_by_run_uses_index --nocapture
cargo test -p in-mem-storage test_scan_by_type --nocapture
cargo test -p in-mem-storage test_indices_stay_consistent --nocapture

# Snapshots
cargo test -p in-mem-storage test_snapshot_isolation --nocapture
cargo test -p in-mem-storage test_snapshot_is_immutable --nocapture

# Prefix scanning
cargo test -p in-mem-storage test_scan_prefix --nocapture

# Integration and stress tests
cargo test -p in-mem-storage --test integration_tests
cargo test -p in-mem-storage --test stress_tests --release

# All tests in release mode
cargo test -p in-mem-storage --all --release
```

**Epic 2 Specific Validations:**

1. **Version Monotonicity** (CRITICAL!)
   - Verify versions increase monotonically: 1, 2, 3, ...
   - No version collisions under concurrent writes
   - current_version() always returns accurate count
   - Test: 10 threads × 100 writes = 1000 sequential versions

2. **Index Consistency** (CRITICAL!)
   - After random operations, all indices match main storage
   - put() updates all 3 indices atomically
   - delete() removes from all 3 indices atomically
   - Scan via index matches full iteration results

3. **TTL Correctness** (CRITICAL!)
   - Expired values return None on get()
   - Expired values don't appear in scans
   - TTL cleanup doesn't race with active writes
   - TTL index enables O(expired) cleanup, not O(total)

4. **Snapshot Isolation** (CRITICAL!)
   - Snapshots capture version at creation time
   - Writes after snapshot creation don't appear in snapshot
   - Multiple concurrent snapshots work correctly
   - Snapshot cloning doesn't corrupt original store

5. **Scan Correctness**
   - scan_prefix returns only keys with matching prefix
   - scan_prefix uses BTreeMap range (not full iteration)
   - scan_by_run filters by namespace.run_id correctly
   - scan_by_type filters by type_tag correctly

6. **Thread Safety**
   - RwLock properly protects shared state
   - No data races under concurrent access
   - AtomicU64 version counter thread-safe
   - No deadlocks in any operation

### Epic 3: WAL Implementation

**Critical checks:**
- [ ] All 6 WALEntry types defined with correct fields
- [ ] All entries include run_id field (except Checkpoint which tracks active_runs)
- [ ] WALEntry implements Serialize, Deserialize, Debug, Clone, PartialEq
- [ ] Helper methods work: run_id(), txn_id(), version(), is_txn_boundary(), is_checkpoint()
- [ ] Entry format matches spec: [Length(4)][Type(1)][Payload(N)][CRC(4)]
- [ ] Type tags assigned: BeginTxn=1, Write=2, Delete=3, CommitTxn=4, AbortTxn=5, Checkpoint=6
- [ ] encode_entry() produces correct byte format
- [ ] decode_entry() validates CRC and detects corruption
- [ ] CRC calculated over [type][payload] (not including length)
- [ ] WAL file operations work: open(), append(), read_entries(), read_all()
- [ ] WAL supports three durability modes: Strict, Batched, Async
- [ ] Default mode is Batched { interval_ms: 100, batch_size: 1000 }
- [ ] Strict mode: fsync after every append
- [ ] Batched mode: fsync after batch_size commits OR interval_ms elapsed
- [ ] Async mode: background thread fsyncs periodically
- [ ] Drop handler calls final fsync
- [ ] Corruption tests detect all failure modes
- [ ] Recovery stops at first corruption (fail-safe)
- [ ] Error messages include file offset for debugging
- [ ] Coverage ≥95% for durability crate

**Tests to run:**
```bash
# All durability tests
cargo test -p durability

# Serialization tests
cargo test -p durability test_serialization --nocapture
cargo test -p durability test_all_entries_serialize --nocapture

# Encoding tests
cargo test -p durability test_encode_decode_roundtrip --nocapture
cargo test -p durability test_crc_detects_corruption --nocapture

# File operations tests
cargo test -p durability test_append_and_read --nocapture
cargo test -p durability test_reopen_wal --nocapture

# Durability mode tests
cargo test -p durability test_strict_mode --nocapture
cargo test -p durability test_batched_mode --nocapture
cargo test -p durability test_async_mode --nocapture

# Corruption detection tests
cargo test -p durability test_crc_detects_bit_flip --nocapture
cargo test -p durability test_truncated_entry_handling --nocapture
cargo test -p durability test_incomplete_transaction_discarded --nocapture

# Corruption simulation tests
cargo test -p durability --test corruption_test --nocapture
cargo test -p durability --test corruption_simulation_test --nocapture

# All tests in release mode
cargo test -p durability --all --release
```

**Epic 3 Specific Validations:**

1. **WAL Entry Types** (CRITICAL!)
   - All 6 variants defined: BeginTxn, Write, Delete, CommitTxn, AbortTxn, Checkpoint
   - Every entry (except Checkpoint) includes run_id field
   - Checkpoint includes Vec<RunId> for active runs
   - Serialization roundtrip works for all entry types
   - Test: 100 entries of each type serialize/deserialize correctly

2. **Encoding Correctness** (CRITICAL!)
   - Format: [length: u32][type: u8][payload][crc: u32]
   - Length field accurate (enables variable-sized entries)
   - Type tag matches entry variant
   - CRC32 calculated over [type][payload]
   - Test: Encode/decode 1000 entries with no errors

3. **Corruption Detection** (CRITICAL!)
   - Bit flips detected by CRC mismatch
   - Truncated entries handled gracefully (no panic)
   - Incomplete transactions (no CommitTxn) identified
   - Multiple corruption points: stops at first error
   - Power loss simulation: partial writes handled
   - Test: All 10 corruption scenarios in corruption_simulation_test.rs pass

4. **File I/O Correctness**
   - WAL::open() creates file or opens existing
   - append() writes entries to end of file
   - read_entries(offset) reads from specific position
   - read_all() reads entire WAL
   - Reopen preserves all previously written entries
   - Test: Write 10000 entries, close, reopen, verify all present

5. **Durability Modes**
   - Strict: fsync after every append (verified by reopen without flush)
   - Batched: fsync after 1000 commits (count-based)
   - Batched: fsync after 100ms (time-based)
   - Async: background thread fsyncs every interval_ms
   - Drop handler fsyncs before close
   - Test: All 3 modes preserve data after crash simulation

6. **Error Messages**
   - CorruptionError includes file offset
   - Error messages help debugging (not just "corruption detected")
   - Test: Corrupt entry at offset 1234, verify error mentions "1234"

7. **Thread Safety**
   - Async mode background thread shuts down cleanly
   - No deadlocks in fsync operations
   - Arc<Mutex<>> prevents data races
   - Test: 100 concurrent appends with Async mode

### Epic 4: Basic Recovery

**Critical checks:**
- [ ] Committed transactions recovered correctly
- [ ] Incomplete transactions discarded (fail-safe)
- [ ] Crash simulation tests pass
- [ ] Recovery handles corrupted WAL gracefully
- [ ] Large WAL recovery is performant

**Tests to run:**
```bash
cargo test --test crash_simulation
cargo test test_large_wal_recovery --release
```

### Epic 5: Database Engine Shell

**Critical checks:**
- [ ] Database::open() with recovery works
- [ ] Run lifecycle (begin/end) correct
- [ ] Basic put/get operations work
- [ ] WAL appended on writes
- [ ] End-to-end integration test passes

**Tests to run:**
```bash
cargo test test_write_restart_read --nocapture
cargo test -p in-mem-engine --all
```

---

## Approval Criteria

**Must meet ALL of:**

- [ ] All Phase 1-5 checklists complete
- [ ] Test coverage ≥ target
- [ ] No failing tests
- [ ] No clippy warnings
- [ ] Documentation complete
- [ ] Epic-specific validation passed
- [ ] Known limitations documented
- [ ] No performance regressions

---

## Review Process Flow

```
All stories merged to epic
         ↓
Phase 1: Pre-Review Validation
         ↓
Phase 2: Integration Testing
         ↓
Phase 3: Code Review
         ↓
Phase 4: Documentation Review
         ↓
Phase 5: Epic-Specific Checks
         ↓
    All passed? ──No──> Fix issues ──┐
         │                            │
        Yes                           │
         │                            │
         ↓                            │
    APPROVED                          │
         │                            │
         ↓                            │
  Merge to develop <─────────────────┘
```

---

## Tools & Commands

### Run Full Review

```bash
./scripts/review-epic.sh <epic-number>
```

This automates Phases 1-2 and part of Phase 4.

### Test Coverage

```bash
# Install (once)
cargo install cargo-tarpaulin

# Generate report
cargo tarpaulin --all --out Html
open tarpaulin-report.html
```

### Epic-Specific Tests

```bash
# Epic 1
cargo test -p in-mem-core --all

# Epic 2
cargo test -p in-mem-storage --all

# Epic 3
cargo test -p in-mem-durability --all -- wal

# Epic 4
cargo test --test crash_simulation

# Epic 5
cargo test --test integration
```

---

## Review Template

Copy for each epic review:

```markdown
# Epic [N] Review: [NAME]

**Date**: YYYY-MM-DD
**Reviewer**: [Name]
**Branch**: epic-[N]-[name]
**Stories**: #X, #Y, #Z

## Validation Results
- Build: ✅/❌
- Tests: ✅/❌
- Clippy: ✅/❌
- Format: ✅/❌
- Coverage: X% (Target: Y%)

## Code Review
- Architecture: ✅/❌
- Code Quality: ✅/❌
- Testing: ✅/❌
- Documentation: ✅/❌

## Epic-Specific Checks
[List checks]

## Issues Found
[List or "None"]

## Decision
- [ ] ✅ APPROVED
- [ ] ❌ CHANGES REQUESTED

Approved by: [Name]
Date: YYYY-MM-DD
```

---

## Post-Merge Actions

1. **Update PROJECT_STATUS.md** - Mark epic complete
2. **Create Epic Summary** - `docs/milestones/EPIC_[N]_SUMMARY.md`
3. **Close Epic Issue** - `gh issue close [EPIC_NUMBER]`
4. **Tag Release** (optional) - `git tag epic-[N]-complete`

---

## Benefits

1. **Quality Assurance** - Catches integration issues early
2. **Knowledge Sharing** - Reviews keep team informed
3. **Documentation** - Forces complete documentation
4. **Confidence** - Know that epic meets criteria
5. **Learning** - Discussions improve future work

---

**See Also:**
- [Epic 1 Review Template](../milestones/EPIC_1_REVIEW.md)
- [Development Workflow](DEVELOPMENT_WORKFLOW.md)
