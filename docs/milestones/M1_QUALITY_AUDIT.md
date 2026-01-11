# M1 Foundation Quality Audit and Testing Plan

**Created**: 2026-01-11
**Status**: DRAFT - Awaiting execution
**Priority**: CRITICAL - Must complete before Epic 5

---

## Executive Summary

**Problem Identified**:
- Epics 1 and 2 may have tests that were adjusted to pass instead of fixing underlying bugs
- No documentation of test failures or workarounds
- Unknown actual test coverage quality
- Risk of silent bugs in M1 foundation

**Impact**:
- **HIGH** - M1 is the foundation for all future work
- Bugs in core types, storage, or recovery will cascade to all features
- Technical debt will compound exponentially

**Proposed Solution**:
Multi-phase quality audit with mutation testing, coverage analysis, and systematic bug hunting before proceeding with Epic 5.

---

## Phase 1: Test Integrity Audit (Git History Analysis)

### Objective
Identify any tests that were modified after initial implementation, especially changes that made tests less strict or worked around bugs.

### Methodology

#### 1.1 Git History Forensics
```bash
# Analyze all test file changes in Epics 1 & 2
git log --all --oneline --graph -- 'crates/core/tests/*.rs' 'crates/core/src/*test*.rs'
git log --all --oneline --graph -- 'crates/storage/tests/*.rs' 'crates/storage/src/*test*.rs'

# Look for suspicious commit messages
git log --all --grep="fix test" --grep="adjust test" --grep="update test" \
  --grep="workaround" --grep="temporary" --grep="TODO" \
  -- 'crates/core/**/*.rs' 'crates/storage/**/*.rs'

# Check for test deletions (red flag!)
git log --all --diff-filter=D -- 'crates/core/tests/*.rs' 'crates/storage/tests/*.rs'

# Find commits that modified tests after implementation
git log --all -p -- 'crates/core/tests/*.rs' 'crates/storage/tests/*.rs' | \
  grep -B10 -A10 "workaround\|bypass\|skip\|ignore\|TODO\|FIXME\|HACK"
```

#### 1.2 Red Flags to Look For
- [ ] Test assertions made less strict (e.g., `assert_eq!` → `assert!`)
- [ ] Test data changed to avoid triggering failures
- [ ] Tests wrapped in `#[ignore]` or `#[should_panic]` after initially passing
- [ ] Error conditions changed to match implementation instead of spec
- [ ] Comments like "temporarily disabled", "TODO: fix", "workaround for bug"
- [ ] Tests deleted entirely
- [ ] Multiple attempts to make a test pass (3+ commits on same test)

#### 1.3 Analysis Output
Create detailed report: `docs/milestones/TEST_INTEGRITY_REPORT.md`
- List of all modified tests
- Reason for each modification (legitimate refactor vs workaround)
- Flagged suspicious changes requiring investigation
- Bugs discovered during audit

### Deliverable
**Test Integrity Report** with:
- ✅ CLEAN - No suspicious test modifications
- ⚠️  CONCERNS - Some tests need review
- ❌ CRITICAL - Tests were adjusted to hide bugs

---

## Phase 2: Full Test Suite Validation

### Objective
Verify all tests still pass in all configurations and identify any flaky or environment-dependent tests.

### Methodology

#### 2.1 Multi-Configuration Testing
```bash
# Debug mode (default)
cargo test --all 2>&1 | tee test-results-debug.log

# Release mode (optimizations enabled)
cargo test --all --release 2>&1 | tee test-results-release.log

# Single-threaded (catches race conditions)
cargo test --all -- --test-threads=1 2>&1 | tee test-results-single-thread.log

# With all features (if any)
cargo test --all --all-features 2>&1 | tee test-results-all-features.log

# Run 10 times to catch flaky tests
for i in {1..10}; do
  echo "=== Run $i ==="
  cargo test --all 2>&1 | tee -a test-results-flaky-check.log
done
```

#### 2.2 Test Failure Analysis
For any failures:
- [ ] Is the test correct (testing the right behavior)?
- [ ] Is the implementation buggy?
- [ ] Is the test flaky (non-deterministic)?
- [ ] Is it a real bug or environmental issue?

#### 2.3 Test Organization Review
```bash
# Count tests per crate
cargo test --all -- --list | grep "test " | wc -l

# List all ignored tests (should be rare)
cargo test --all -- --ignored --list

# Check for tests without assertions (ineffective tests)
rg "^[[:space:]]*fn test_" -A 20 crates/ | grep -L "assert"
```

### Deliverable
**Test Suite Validation Report**:
- Total test count per crate
- Pass/fail rates across configurations
- List of flaky tests (if any)
- Environmental dependencies identified

---

## Phase 3: Code Coverage Analysis (Deep Dive)

### Objective
Measure actual line coverage and identify untested code paths, especially error handling.

### Methodology

#### 3.1 Install Coverage Tools
```bash
# Install tarpaulin (coverage tool)
cargo install cargo-tarpaulin

# Install grcov (alternative coverage tool)
cargo install grcov
rustup component add llvm-tools-preview
```

#### 3.2 Generate Coverage Reports
```bash
# Tarpaulin HTML report
cargo tarpaulin --all --out Html --output-dir coverage/tarpaulin \
  --exclude-files 'tests/*' --timeout 300

# Per-crate coverage
for crate in core storage concurrency durability engine; do
  echo "=== Coverage for $crate ==="
  cargo tarpaulin -p in-mem-$crate --out Html \
    --output-dir coverage/$crate --timeout 300
done

# Generate line-by-line coverage report
cargo tarpaulin --all --out Json --output-dir coverage/json
```

#### 3.3 Coverage Analysis Checklist
- [ ] **Core types** (target: 100%)
  - RunId, Namespace, Key, TypeTag, Value enum
  - All helper methods covered
  - Error types and conversions

- [ ] **Storage layer** (target: ≥95%)
  - UnifiedStore: put, get, delete, scan operations
  - Secondary indices: run_index, type_index
  - TTL expiration logic
  - ClonedSnapshotView

- [ ] **Error paths** (CRITICAL - often untested!)
  - I/O errors (file operations)
  - Serialization errors
  - Validation errors
  - Out-of-bounds errors

#### 3.4 Identify Coverage Gaps
```bash
# Find uncovered lines in core
cargo tarpaulin -p in-mem-core --out Json | \
  jq '.files[] | select(.coverage < 100) | {file: .path, coverage: .coverage}'

# Find uncovered lines in storage
cargo tarpaulin -p in-mem-storage --out Json | \
  jq '.files[] | select(.coverage < 95) | {file: .path, coverage: .coverage}'
```

### Deliverable
**Coverage Report**:
- Per-crate coverage percentages
- Line-by-line coverage visualization (HTML)
- List of uncovered critical paths
- Specific lines/functions needing tests

---

## Phase 4: Mutation Testing (Advanced Bug Detection)

### Objective
Verify that tests actually catch bugs by introducing mutations and checking if tests fail.

**What is Mutation Testing?**
- Automatically modify code (e.g., change `>` to `>=`, flip boolean conditions)
- Run tests against mutated code
- If tests still pass, the mutation "survived" → **weak tests**
- High "kill rate" = strong tests

### Methodology

#### 4.1 Install Mutation Testing Tool
```bash
# Install cargo-mutants
cargo install cargo-mutants

# Or use mutagen (alternative)
cargo install cargo-mutagen
```

#### 4.2 Run Mutation Tests on Core Components
```bash
# Mutate core types
cargo mutants --package in-mem-core \
  --output mutants-core.json \
  --timeout 120

# Mutate storage layer
cargo mutants --package in-mem-storage \
  --output mutants-storage.json \
  --timeout 120

# Analyze mutation results
cargo mutants --list-files mutants-core.json
```

#### 4.3 Mutation Analysis
For each survived mutant:
- [ ] Why didn't tests catch this mutation?
- [ ] Is the mutated code actually reachable?
- [ ] Is there a missing test case?
- [ ] Is the code redundant/dead?

**Example Mutations**:
```rust
// Original
if version > 0 { ... }

// Mutation 1: Boundary condition
if version >= 0 { ... }  // Should fail if tests check version=0

// Mutation 2: Logic flip
if version <= 0 { ... }  // Should fail if tests check version>0

// Mutation 3: Constant change
if version > 1 { ... }   // Should fail if tests check version=1
```

### Deliverable
**Mutation Testing Report**:
- Mutation score per crate (% of mutants killed)
- List of survived mutants (weak tests)
- Recommended test additions

**Target Mutation Score**: ≥80% for core and storage

---

## Phase 5: Property-Based Testing (Fuzzing)

### Objective
Use property-based testing to find edge cases and unexpected behavior.

### Methodology

#### 5.1 Identify Properties to Test

**Core Types Properties**:
- RunId serialization roundtrip: `deserialize(serialize(x)) == x`
- Key ordering: `a < b ⇒ a.cmp(b) == Less`
- Namespace equality: `a == b ⇒ a.hash() == b.hash()`

**Storage Properties**:
- Put-Get consistency: `storage.put(k, v); storage.get(k) == Some(v)`
- Delete idempotence: `storage.delete(k); storage.delete(k) == None`
- Scan ordering: Results sorted by key
- TTL expiration: Value absent after TTL expires

#### 5.2 Write Property Tests
```rust
// Example property test for Key ordering
use proptest::prelude::*;

proptest! {
    #[test]
    fn key_ordering_transitive(
        k1 in arbitrary_key(),
        k2 in arbitrary_key(),
        k3 in arbitrary_key()
    ) {
        if k1 < k2 && k2 < k3 {
            assert!(k1 < k3, "Key ordering not transitive");
        }
    }

    #[test]
    fn put_get_roundtrip(
        key in arbitrary_key(),
        value in arbitrary_value()
    ) {
        let storage = UnifiedStore::new();
        storage.put(key.clone(), value.clone(), None).unwrap();
        let retrieved = storage.get(&key).unwrap().unwrap();
        assert_eq!(retrieved.value, value);
    }
}
```

#### 5.3 Run Property Tests
```bash
# Run with many iterations to find edge cases
PROPTEST_CASES=10000 cargo test --all proptest

# Run with different random seeds
for seed in {1..100}; do
  PROPTEST_RNG_SEED=$seed cargo test --all proptest
done
```

### Deliverable
**Property Testing Report**:
- List of properties tested
- Any violations found
- Edge cases discovered

---

## Phase 6: Integration Testing Scenarios

### Objective
Test realistic end-to-end scenarios that cross module boundaries.

### Methodology

#### 6.1 Define Critical Scenarios

**Scenario 1: Concurrent Access**
- Multiple threads writing to same run
- Verify no data races or corruption
- Check version numbers sequential

**Scenario 2: Large Dataset**
- 1M key-value pairs
- Verify performance acceptable
- Check memory usage reasonable

**Scenario 3: Crash Recovery**
- Write 10K entries
- Simulate crash (kill process)
- Reopen database
- Verify all committed data recovered

**Scenario 4: TTL Under Load**
- Write 100K entries with short TTL
- Verify TTL cleanup doesn't interfere with operations
- Check no memory leaks

**Scenario 5: Secondary Index Consistency**
- Run multiple operations
- Verify run_index and type_index always consistent with main store
- Delete entries and verify indices updated

#### 6.2 Implement Integration Tests
Create: `crates/integration-tests/tests/scenarios.rs`

```rust
#[test]
fn scenario_concurrent_writes() {
    // 10 threads, 1000 writes each
    // Verify all 10K entries present
    // Verify no corruption
}

#[test]
fn scenario_large_dataset() {
    // Write 1M entries
    // Measure time (should be <60s)
    // Measure memory (should be <500MB)
}

#[test]
#[ignore] // Run manually
fn scenario_crash_recovery() {
    // Requires actual process fork/kill
}
```

### Deliverable
**Integration Testing Report**:
- All scenarios pass/fail status
- Performance metrics
- Any unexpected behavior

---

## Phase 7: Manual Code Review (Human Review)

### Objective
Human review of critical code sections that tools can't catch.

### Focus Areas

#### 7.1 Core Types Review
**File**: `crates/core/src/types.rs`
- [ ] RunId generation truly unique (UUID v4)
- [ ] Namespace fields validated (no empty strings)
- [ ] Key ordering matches BTreeMap semantics
- [ ] TypeTag exhaustive (all variants handled)

**File**: `crates/core/src/value.rs`
- [ ] Value enum serialization correct for all variants
- [ ] VersionedValue version always incremented
- [ ] Timestamp generation monotonic

#### 7.2 Storage Layer Review
**File**: `crates/storage/src/unified.rs`
- [ ] RwLock usage correct (no deadlocks possible)
- [ ] Version counter atomic (no races)
- [ ] Secondary indices always updated atomically with main store
- [ ] TTL expiration doesn't race with gets

**File**: `crates/storage/src/ttl.rs` (if exists)
- [ ] TTL cleanup uses transactions (not direct mutations)
- [ ] Expired entries don't cause crashes
- [ ] TTL thread shutdown clean

#### 7.3 Error Handling Review
- [ ] No `unwrap()` or `expect()` in production code
- [ ] All `Result` types propagated correctly
- [ ] Error messages informative (include context)
- [ ] No silent failures (errors logged or returned)

#### 7.4 Unsafe Code Review (if any)
```bash
# Find all unsafe blocks
rg "unsafe" crates/ --type rust

# Each unsafe block MUST have:
# - Comment explaining why unsafe needed
# - Comment explaining safety invariants
# - Proof that invariants upheld
```

### Deliverable
**Manual Review Report**:
- Issues found per category
- Risk assessment (HIGH/MEDIUM/LOW)
- Recommended fixes

---

## Phase 8: Bug Reproduction Test Suite

### Objective
Create tests for any bugs found during audit, ensuring they're fixed and stay fixed.

### Methodology

#### 8.1 Create Regression Test File
`crates/core/tests/regression_tests.rs`
`crates/storage/tests/regression_tests.rs`

#### 8.2 Document Bug Pattern
For each bug found:
```rust
/// Bug #1: [Brief description]
///
/// **Discovered**: [Date]
/// **Symptom**: [What went wrong]
/// **Root Cause**: [Why it happened]
/// **Fix**: [How it was fixed]
/// **Test**: Ensures bug doesn't reoccur
#[test]
fn regression_bug_001_description() {
    // Minimal reproduction case
    // Assert fix works
}
```

### Deliverable
**Regression Test Suite**:
- One test per bug found
- All tests passing
- Documentation of each bug

---

## Phase 9: Documentation Audit

### Objective
Ensure code behavior matches documentation.

### Checklist
- [ ] README.md accuracy
- [ ] Rustdoc comments correct (no outdated docs)
- [ ] Architecture docs match implementation
- [ ] TDD_METHODOLOGY.md followed
- [ ] No conflicting information

### Methodology
```bash
# Check rustdoc builds without warnings
cargo doc --all --no-deps 2>&1 | tee rustdoc-warnings.log

# Check for TODO/FIXME in docs
rg "TODO|FIXME" crates/ --type rust -g '!tests'

# Verify doc examples compile
cargo test --doc --all
```

### Deliverable
**Documentation Audit Report**:
- List of documentation issues
- Outdated sections
- Missing documentation

---

## Quality Gates (Must Pass Before Epic 5)

### Gate 1: Test Integrity ✅ or ❌
- [ ] No tests adjusted to hide bugs (Phase 1)
- [ ] All known bugs documented with regression tests (Phase 8)

**If FAIL**: Stop immediately, fix bugs, restore proper tests.

### Gate 2: Test Coverage ✅ or ❌
- [ ] Core types: ≥100% line coverage (Phase 3)
- [ ] Storage layer: ≥95% line coverage (Phase 3)
- [ ] All error paths tested (Phase 3)

**If FAIL**: Write missing tests until targets met.

### Gate 3: Test Quality ✅ or ❌
- [ ] Mutation score ≥80% for core and storage (Phase 4)
- [ ] No flaky tests (Phase 2)
- [ ] All integration scenarios pass (Phase 6)

**If FAIL**: Strengthen tests, remove flaky tests.

### Gate 4: Code Quality ✅ or ❌
- [ ] No `unwrap()`/`expect()` in production code (Phase 7)
- [ ] All `unsafe` justified and documented (Phase 7)
- [ ] No silent failures (Phase 7)

**If FAIL**: Refactor code to meet quality standards.

### Gate 5: Documentation ✅ or ❌
- [ ] All public APIs documented (Phase 9)
- [ ] No outdated documentation (Phase 9)
- [ ] Doc tests compile (Phase 9)

**If FAIL**: Update documentation.

---

## Execution Plan

### Timeline
**Estimated**: 3-4 days (with 1-2 people)

| Phase | Time | Owner | Blocker |
|-------|------|-------|---------|
| 1. Test Integrity Audit | 4 hours | You/Claude | None |
| 2. Full Test Validation | 2 hours | You/Claude | None |
| 3. Coverage Analysis | 3 hours | You/Claude | None |
| 4. Mutation Testing | 6 hours | You/Claude | Phase 2 |
| 5. Property Testing | 4 hours | You/Claude | Phase 3 |
| 6. Integration Tests | 6 hours | You/Claude | Phase 2 |
| 7. Manual Review | 8 hours | You | None |
| 8. Bug Fixes + Regression Tests | Variable | You/Claude | Phases 1-7 |
| 9. Documentation Audit | 2 hours | You/Claude | None |

**Total**: ~35 hours (can parallelize some phases)

### Parallelization
- Phase 1, 2, 3, 9 can run in parallel (different tools)
- Phase 4, 5, 6 depend on Phase 2 (need passing tests)
- Phase 8 runs as bugs are found (ongoing)

### Resources Needed
- `cargo-tarpaulin` (coverage)
- `cargo-mutants` or `cargo-mutagen` (mutation testing)
- `proptest` (property-based testing)
- Human reviewer for Phase 7

---

## Success Criteria

**M1 Foundation is TRUSTED when**:
1. ✅ All 5 quality gates passed
2. ✅ Test coverage ≥95% for core components
3. ✅ Mutation score ≥80%
4. ✅ No known bugs without regression tests
5. ✅ All integration scenarios pass
6. ✅ Documentation matches implementation
7. ✅ Clean git history (no test workarounds)

**Only then** proceed to Epic 5.

---

## Risk Assessment

### If We Skip This Audit

**Likelihood**: HIGH that bugs exist in Epics 1-2

**Impact**:
- 🔴 **CRITICAL** - Foundation bugs will cascade
- 🔴 **CRITICAL** - Technical debt compounds exponentially
- 🔴 **CRITICAL** - Later bugs much harder to fix
- 🔴 **CRITICAL** - May require rewriting M1 later

**Cost of Skipping**: 10x more expensive to fix later

### If We Execute This Audit

**Likelihood**: MEDIUM that we find bugs

**Impact**:
- 🟢 **POSITIVE** - Bugs caught early
- 🟢 **POSITIVE** - Confidence in foundation
- 🟢 **POSITIVE** - Clean base for M2-M5
- 🟢 **POSITIVE** - Better testing culture established

**Cost of Executing**: 3-4 days now vs weeks later

---

## Recommendation

**PAUSE Epic 5 implementation.**

**EXECUTE this quality audit immediately.**

**Rationale**:
- M1 is the foundation - it MUST be solid
- Epics 3-4 had good testing discipline (95%+ coverage, mutation testing)
- Epics 1-2 predate TDD rigor - need verification
- 3-4 day investment now saves weeks later
- Sets quality bar for all future work

---

## Appendix: Tools and Commands

### Coverage Tools
```bash
# Install
cargo install cargo-tarpaulin cargo-llvm-cov

# HTML report
cargo tarpaulin --all --out Html --output-dir coverage

# JSON for analysis
cargo tarpaulin --all --out Json --output-dir coverage
```

### Mutation Testing
```bash
# Install
cargo install cargo-mutants

# Run
cargo mutants --package in-mem-core
cargo mutants --package in-mem-storage
```

### Property Testing
```bash
# Add to Cargo.toml
[dev-dependencies]
proptest = "1.0"

# Run with many cases
PROPTEST_CASES=10000 cargo test
```

### Benchmark Tools
```bash
# Install
cargo install cargo-criterion

# Run benchmarks
cargo criterion
```

---

**Document Version**: 1.0
**Last Updated**: 2026-01-11
**Next Review**: After Phase 8 completion
