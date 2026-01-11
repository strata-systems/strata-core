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
- [ ] Version management is monotonic and thread-safe
- [ ] Secondary indices stay consistent
- [ ] TTL cleanup is transactional (no races)
- [ ] Known RwLock bottleneck documented

**Tests to run:**
```bash
cargo test test_concurrent_reads --nocapture
cargo test test_version_monotonic --nocapture
cargo test test_ttl_cleanup_transactional --nocapture
```

### Epic 3: WAL Implementation

**Critical checks:**
- [ ] All WALEntry types serialize correctly
- [ ] CRC validation detects corruption
- [ ] Durability modes work as specified
- [ ] File I/O is correct (no data loss)
- [ ] Corruption simulation tests pass

**Tests to run:**
```bash
cargo test test_corrupted_entry_detected --nocapture
cargo test test_strict_mode_survives_crash --nocapture
cargo test --test corruption_simulation
```

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
