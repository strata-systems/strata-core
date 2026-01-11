# Epic 3 Coordination: WAL Implementation

**Epic**: #3 - WAL Implementation
**Branch**: `epic-3-wal-implementation`
**Stories**: #17, #18, #19, #20, #21, #22
**Target**: Complete WAL with durability modes and corruption detection

---

## Overview

Epic 3 implements the Write-Ahead Log (WAL) with:
- WAL entry types with run_id in all entries
- Encoding/decoding with CRC checksums
- File operations (open, append, read)
- Durability modes (strict, batched, async)
- Comprehensive corruption detection and simulation tests

---

## Dependency Graph

```
Story #17: WAL entry types
     ↓
     ├───→ Story #18: Encoding/decoding
     │
     └───→ Story #19: File operations
              ↓
         Story #20: Durability modes
              ↓
         Story #21: CRC/checksums
              ↓
         Story #22: Corruption simulation
```

### Critical Path

**Story #17 is the blocker** - ALL other stories depend on WALEntry types being defined.

After #17:
- #18 and #19 can run **in parallel** (no dependencies between them)
- Remaining stories are sequential

---

## Parallelization Analysis

### Phase 1: Foundation (Sequential) - ~3-4 hours
**Story #17**: WAL entry types
- **Blocks**: ALL other stories
- **Assignee**: Claude 1 (Available)
- **Estimated**: 3-4 hours
- **Files**: Create durability crate, define WALEntry enum

**Why sequential**: WALEntry is the foundation type used by all subsequent stories.

### Phase 2: Encoding & File I/O (Parallel) - ~5-6 hours wall time
Once #17 is merged to `epic-3-wal-implementation`:

| Story | Component | Assignee | Estimated | Dependencies |
|-------|-----------|----------|-----------|--------------|
| #18 | Encoding/decoding | Claude 2 | 4-5 hours | #17 only |
| #19 | File operations | Claude 3 | 5-6 hours | #17 only |

**Why parallel**:
- #18 works on encoding.rs (no file I/O)
- #19 works on WAL struct and file ops
- No file conflicts
- Both only depend on #17 (WALEntry definition)

**Wall time**: ~5-6 hours (max of the two)

### Phase 3: Durability (Sequential) - ~5-6 hours
After #18 and #19 are merged:

**Story #20**: Durability modes
- **Depends on**: #19 (WAL file operations)
- **Assignee**: Claude 4 (Available)
- **Estimated**: 5-6 hours
- **Files**: Update wal.rs with fsync and modes

**Why sequential**: Needs completed WAL file operations to add fsync logic.

### Phase 4: Testing (Sequential) - ~9-11 hours
After #20 is merged:

**Story #21**: CRC/checksums
- **Depends on**: #18 (CRC already implemented), #19, #20
- **Assignee**: Claude 5 (Available)
- **Estimated**: 4-5 hours
- **Files**: Create corruption_test.rs

After #21 is merged:

**Story #22**: Corruption simulation
- **Depends on**: #21 (builds on basic corruption tests)
- **Assignee**: Claude 6 (Available)
- **Estimated**: 5-6 hours
- **Files**: Create corruption_simulation_test.rs

**Why sequential**: Each testing story builds on the previous one.

---

## Timeline Summary

### Sequential Execution
- Story #17: 3-4 hours
- Story #18: 4-5 hours
- Story #19: 5-6 hours
- Story #20: 5-6 hours
- Story #21: 4-5 hours
- Story #22: 5-6 hours
- **Total**: ~27-32 hours sequential

### Parallel Execution (6 Claudes)
- Phase 1 (#17): 3-4 hours
- Phase 2 (#18, #19 in parallel): 5-6 hours (wall time)
- Phase 3 (#20): 5-6 hours
- Phase 4a (#21): 4-5 hours
- Phase 4b (#22): 5-6 hours
- **Total**: ~22-27 hours wall time

**Speedup**: ~20% reduction in wall time (minimal due to sequential dependencies)

**Maximum parallelization**: 2 Claudes in Phase 2 only

---

## File Ownership

To minimize merge conflicts:

### Story #17 (Claude 1)
- `crates/durability/Cargo.toml` (creates)
- `crates/durability/src/lib.rs` (creates)
- `crates/durability/src/wal.rs` (creates WALEntry enum only)

### Story #18 (Claude 2)
- `crates/durability/src/encoding.rs` (creates)
- `crates/durability/src/lib.rs` (adds `pub mod encoding;`)

### Story #19 (Claude 3)
- `crates/durability/src/wal.rs` (adds WAL struct, file operations)
- `crates/durability/Cargo.toml` (adds tempfile dev-dependency)

### Story #20 (Claude 4)
- `crates/durability/src/wal.rs` (updates with durability modes)

### Story #21 (Claude 5)
- `crates/durability/tests/corruption_test.rs` (creates)

### Story #22 (Claude 6)
- `crates/durability/tests/corruption_simulation_test.rs` (creates)

### Potential Conflicts

**Stories #18 and #19 (Phase 2)**:
- Both touch `wal.rs` and `lib.rs`
- **Resolution**:
  - #18 only adds encoding module import to lib.rs (2 lines)
  - #19 adds WAL struct to wal.rs (separate from WALEntry)
  - Minimal conflict - easy merge

---

## Communication Protocol

### Starting Work
When assigned a story:
1. Comment on GitHub issue: "Starting work on this story"
2. Run: `./scripts/start-story.sh 3 <story-num> <description>`
3. Update coordination status in issue comments

### Blocked on Dependencies
If waiting for another story:
1. Comment on blocking story's issue: "Story #X blocked on this"
2. Wait for merge notification
3. Sync your branch: `./scripts/sync-epic.sh 3`

### Completing Work
When story is complete:
1. Run: `./scripts/complete-story.sh <story-num>`
2. Comment on issue: "PR #X created, ready for review"
3. Notify dependent stories

### Merge Conflicts
If merge conflict occurs:
1. Comment on issue: "Merge conflict with story #X"
2. Pull latest epic branch: `git pull origin epic-3-wal-implementation`
3. Resolve conflict
4. Re-run tests
5. Update PR

---

## Story Readiness Checklist

### Story #17: WAL entry types
- [ ] Assigned to: _______________
- [ ] Branch created: epic-3-story-17-wal-entry-types
- [ ] Dependencies: NONE (can start immediately)
- [ ] Status: Ready to start

### Story #18: Encoding/decoding
- [ ] Assigned to: _______________
- [ ] Branch created: epic-3-story-18-encoding-decoding
- [ ] Dependencies: #17 merged ✅
- [ ] Status: Waiting for #17

### Story #19: File operations
- [ ] Assigned to: _______________
- [ ] Branch created: epic-3-story-19-file-operations
- [ ] Dependencies: #17 merged ✅
- [ ] Status: Waiting for #17

### Story #20: Durability modes
- [ ] Assigned to: _______________
- [ ] Branch created: epic-3-story-20-durability-modes
- [ ] Dependencies: #19 merged ✅
- [ ] Status: Waiting for #19

### Story #21: CRC/checksums
- [ ] Assigned to: _______________
- [ ] Branch created: epic-3-story-21-crc-checksums
- [ ] Dependencies: #18, #19, #20 merged ✅
- [ ] Status: Waiting for #20

### Story #22: Corruption simulation
- [ ] Assigned to: _______________
- [ ] Branch created: epic-3-story-22-corruption-simulation
- [ ] Dependencies: #21 merged ✅
- [ ] Status: Waiting for #21

---

## Progress Tracking

### Completion Status

**Epic 3: WAL Implementation**
- [ ] Story #17: WAL entry types (3-4 hours)
- [ ] Story #18: Encoding/decoding (4-5 hours)
- [ ] Story #19: File operations (5-6 hours)
- [ ] Story #20: Durability modes (5-6 hours)
- [ ] Story #21: CRC/checksums (4-5 hours)
- [ ] Story #22: Corruption simulation (5-6 hours)

**Total**: 0/6 stories complete (0%)

---

## Quality Gates

Before merging each story to epic-3-wal-implementation:

1. **All tests pass**: `cargo test -p durability`
2. **No clippy warnings**: `cargo clippy -p durability -- -D warnings`
3. **Formatted**: `cargo fmt -p durability --check`
4. **Acceptance criteria met**: All checkboxes in issue marked ✅
5. **Code reviewed**: At least 1 reviewer approval (self-review acceptable for MVP)

Before merging epic to develop:

1. **All 6 stories complete**
2. **Epic review process** (EPIC_3_REVIEW.md)
3. **95%+ test coverage** for durability crate
4. **All corruption scenarios tested**
5. **Integration tests pass**

---

## Risk Mitigation

### Risk: Story #17 takes longer than expected
**Impact**: Blocks all other stories
**Mitigation**: Prioritize #17, assign to most experienced Claude
**Fallback**: If blocked >1 day, escalate to user

### Risk: Merge conflicts in Phase 2
**Impact**: #18 and #19 both touch wal.rs and lib.rs
**Mitigation**:
- #18 finishes first: add only encoding module
- #19 finishes first: add only WAL struct
- Clear file ownership documented above
**Fallback**: Manual merge by user if conflict too complex

### Risk: Corruption tests are flaky
**Impact**: Tests fail intermittently, block merge
**Mitigation**: Run tests multiple times before completing story
**Requirement**: Tests must pass 5 consecutive runs
**Fallback**: Identify and fix flaky test, re-run

### Risk: WAL file I/O platform-specific issues
**Impact**: Tests pass on macOS but might fail on Linux/Windows
**Mitigation**: Use std::fs abstractions, avoid platform-specific code
**Note**: MVP targets macOS only, cross-platform testing in M2

---

## Epic Completion Criteria

Epic 3 is complete when:

1. ✅ All 6 stories merged to epic-3-wal-implementation
2. ✅ `cargo build --all` passes
3. ✅ `cargo test -p durability` passes (all tests)
4. ✅ `cargo clippy -p durability -- -D warnings` passes
5. ✅ Test coverage ≥95% for durability crate
6. ✅ All corruption scenarios tested:
   - Bit flips detected by CRC
   - Truncated entries handled gracefully
   - Incomplete transactions discarded
   - Multiple corruption points handled
   - Power loss simulation passes
   - Filesystem bug simulation passes
7. ✅ All durability modes tested:
   - Strict mode works
   - Batched mode works
   - Async mode works
8. ✅ Epic review process completed (EPIC_3_REVIEW.md)
9. ✅ Ready to merge to develop

---

## Next Steps After Epic 3

Once Epic 3 is complete and merged to develop:

1. **Epic 4: Basic Recovery**
   - WAL replay
   - Incomplete transaction handling
   - Database::open() with recovery
   - Crash simulation tests

2. **Epic 5: Database Engine Shell**
   - Database struct
   - Run tracking
   - Basic put/get operations
   - Integration tests

**M1 completion**: After Epic 5, all M1 milestones are complete.

---

## Contact

For questions or coordination:
- Read [EPIC_REVIEW_PROCESS.md](../development/EPIC_REVIEW_PROCESS.md) for quality gates
- Read [DEVELOPMENT_WORKFLOW.md](../development/DEVELOPMENT_WORKFLOW.md) for Git workflow
- Read [M1_ARCHITECTURE.md](M1_ARCHITECTURE.md) for WAL design
- Comment on GitHub issues for story-specific questions
- Check this file for dependency status

---

**Epic 3 Coordination Version**: 1.0
**Last Updated**: 2026-01-10
