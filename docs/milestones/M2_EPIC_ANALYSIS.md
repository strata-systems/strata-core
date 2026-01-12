# M2 Epic Breakdown Analysis

**Date**: 2026-01-11
**Analyst**: Claude Sonnet 4.5
**Purpose**: Deep analysis of M2 epic structure before creating GitHub issues

---

## Executive Summary

After thorough analysis comparing M2 proposed structure against M1's successful pattern, I have identified **significant issues** with the current 4-epic breakdown. The epics are too coarse-grained and don't match M1's granularity, which will lead to:

1. **Massive stories** (8-16 hours each vs M1's 2-6 hours)
2. **Reduced parallelization** (artificially sequential due to broad epic boundaries)
3. **Unclear dependencies** (hidden within large stories)
4. **Testing gaps** (integration tests separated from implementation)

**Recommendation**: Restructure M2 into **5-6 focused epics** with **30-35 granular stories** following M1's proven pattern.

---

## Analysis Methodology

### Data Examined

1. **M1 Actual Implementation**:
   - 5 epics, 27 stories (avg 5.4 stories/epic)
   - Story size: 2-6 hours (avg ~4 hours)
   - Epic size: 2-3 days with parallelization

2. **M1 Story Pattern** (Example: Story #12 - UnifiedStore):
   - ~300 lines of detailed spec
   - Complete code examples
   - Unit tests included in same story
   - Clear acceptance criteria (8-10 items)
   - Dependencies explicitly stated

3. **M2 Architecture Specification**:
   - 850 lines across 13 sections
   - 4 modules: transaction, snapshot, validation, cas
   - Engine integration requirements
   - WAL modifications needed
   - Testing requirements

### Key Metrics Comparison

| Metric | M1 Actual | M2 Proposed | Issue |
|--------|-----------|-------------|-------|
| **Epics** | 5 | 4 | ⚠️ M2 has more scope but fewer epics |
| **Stories** | 27 | ~24 | ⚠️ Similar count but M2 scope is larger |
| **Avg Story Size** | 4 hours | 8-12 hours | ❌ M2 stories 2-3x larger |
| **Epic Focus** | Single component | Mixed concerns | ❌ M2 epics too broad |
| **Parallelization** | 3-4 Claudes/epic | 2-3 Claudes/epic | ⚠️ Less efficient |

---

## Critical Issues Identified

### Issue 1: Epic 6 Mixes Foundation with Implementation

**Current**: "Concurrency Crate Foundation" (Stories #33-38)

**Problem**: Combines infrastructure (crate setup) with core types AND implementation logic.

**Evidence from M1**: Epic 1 separated workspace setup (#6) from type definitions (#7-11).

**Impact**:
- Story #33 (crate setup) is 1 hour
- Stories #34-37 (types + logic) are 4-8 hours each
- Epic appears to have 6 stories but actually has 1 foundation + 5 implementation
- Poor parallelization (only 2-3 can actually run in parallel)

**Fix**: Split into two epics:
- **Epic 6A**: Concurrency Infrastructure (1-2 stories, 1 day)
- **Epic 6B**: OCC Core Types (4-5 stories, 2 days, 3 Claudes)

### Issue 2: Epic 7 Is Monolithic (Transaction Lifecycle)

**Current**: "Transaction Lifecycle" (Stories #39-45, 7 stories)

**Problem**: Bundles BEGIN + READ + WRITE + VALIDATE + COMMIT into one epic. This is analogous to M1 putting all of Epic 2 (Storage), Epic 3 (WAL), AND Epic 4 (Recovery) together.

**Evidence from M2 Architecture**:
```
## 3. Component Architecture
### 3.1 Concurrency Crate
  - transaction.rs
  - snapshot.rs
  - validation.rs
  - cas.rs
```

Each module is substantial (100-200 lines). Should be separate stories.

**Impact**:
- Limited parallelization (sequential dependencies)
- Hard to test incrementally
- 3-4 day epic blocks everything downstream

**Fix**: Split into focused epics:
- **Epic 7A**: Snapshot Management (3-4 stories)
- **Epic 7B**: Transaction Operations (4-5 stories)
- **Epic 7C**: Validation & Conflict Detection (3-4 stories)

### Issue 3: Epic 8 Conflates Engine + WAL + Recovery

**Current**: "Database Engine Integration" (Stories #46-51)

**Problem**: Combines three M1 epics' worth of work:
- Engine changes (like M1 Epic 5)
- WAL modifications (like M1 Epic 3)
- Recovery changes (like M1 Epic 4)

**Evidence from M1**:
- Epic 3 (WAL): 6 stories, 2-3 days
- Epic 4 (Recovery): 5 stories, 2-3 days
- Epic 5 (Engine): 5 stories, 2-3 days

M2 Epic 8 tries to do all three in 6 stories!

**Impact**:
- Massive stories (8-16 hours each)
- Hidden dependencies
- Cannot parallelize effectively
- High risk (touching multiple critical systems)

**Fix**: Split into separate epics:
- **Epic 8A**: WAL Transaction Support (3-4 stories)
- **Epic 8B**: Recovery for Transactions (3-4 stories)
- **Epic 8C**: Engine Transaction API (4-5 stories)

### Issue 4: Epic 9 Defers All Testing

**Current**: "OCC Testing & Validation" (Stories #52-56, Epic 9)

**Problem**: Following M1 anti-pattern of separating tests from implementation. M1 learned that Epic 2 Story #16 (tests separated from #12-15) created coordination issues.

**Evidence from M1 TDD Methodology**:
> "Story #16 (Storage Tests) waits for #12-15 to complete. This created a gap where implementation finished but testing lagged."

M1 fixed this by including unit tests in each story's acceptance criteria.

**Impact**:
- Implementation can finish without proper testing
- Integration tests can't run until Epic 8 complete
- Property-based tests delayed to end
- Risk of bugs discovered late

**Fix**: Distribute testing across implementation epics:
- Unit tests: Include in each story (M1 pattern)
- Multi-threaded tests: Epic-level integration stories
- Property-based tests: Separate epic after implementation
- Stress tests: Final validation epic

---

## Root Cause Analysis

### Why Did This Happen?

1. **Top-Down Planning**: Started with "what are the big components?" instead of "what are the atomic tasks?"

2. **Architecture Document Structure**: M2_ARCHITECTURE.md is organized by component (Concurrency, Engine, WAL), not by implementation sequence.

3. **Missing M1 Reference**: Didn't analyze M1's actual GitHub issues before planning M2.

4. **Underestimating Scope**: OCC is complex. Each piece (snapshot, validation, CAS, WAL integration) deserves its own focus.

### Lessons from M1

**M1's Success Pattern**:
- **Epic 1**: Foundation only (workspace + types) - enables parallelization
- **Epic 2**: Single layer (Storage) - 5 stories, clear scope
- **Epic 3**: Single subsystem (WAL) - 6 stories, self-contained
- **Epic 4**: Single concern (Recovery) - 5 stories, sequential
- **Epic 5**: Integration (Engine) - 5 stories, brings it together

**Why M1 Worked**:
- Each epic had **one clear objective**
- Stories were **4-6 hours** (completable in one session)
- **Tests included** in acceptance criteria
- **Clear dependencies** between epics (not within)
- **Parallelization within epic** (3-4 Claudes after foundation story)

---

## Proposed Revised M2 Epic Structure

Based on analysis, here's the corrected structure:

### Epic 6: Concurrency Infrastructure (Foundation)
**Goal**: Set up concurrency crate structure and error types
**Duration**: 1 day
**Stories**: 3 stories
**Parallelization**: Sequential (foundation work)

1. **Story #33**: Enable concurrency crate (remove placeholder)
   - Uncomment module declarations
   - Add dependencies (core, storage)
   - Update lib.rs structure
   - **Estimated**: 1 hour

2. **Story #34**: Define transaction types and enums
   - TransactionContext struct (no methods yet)
   - TransactionStatus enum
   - CASOperation struct
   - **Estimated**: 2-3 hours

3. **Story #35**: Define conflict and error types
   - ConflictInfo enum (ReadConflict, WriteConflict, CASConflict)
   - ConcurrencyError types
   - Extend core Error enum
   - **Estimated**: 2-3 hours

**Epic 6 Total**: 5-7 hours (1 day, sequential)

---

### Epic 7: Snapshot Management
**Goal**: Implement snapshot isolation infrastructure
**Duration**: 2-3 days with 3 Claudes
**Stories**: 4-5 stories
**Parallelization**: After #36, stories #37-40 can run in parallel

4. **Story #36**: Define SnapshotView trait
   - Trait definition (get, scan_prefix, version)
   - Documentation for future LazySnapshotView
   - Unit tests for trait contract
   - **Estimated**: 2-3 hours
   - **BLOCKS**: Stories #37-40

5. **Story #37**: Implement ClonedSnapshotView
   - ClonedSnapshotView struct
   - create() method (clone BTreeMap)
   - SnapshotView trait implementation
   - Unit tests for snapshot creation
   - **Estimated**: 4-5 hours

6. **Story #38**: Add storage snapshot support
   - UnifiedStore::clone_data_at_version() method
   - Version filtering during clone
   - TTL handling during snapshot
   - Unit tests for storage cloning
   - **Estimated**: 3-4 hours

7. **Story #39**: Snapshot memory management
   - Arc<BTreeMap> sharing
   - Snapshot lifecycle tests
   - Memory usage tests
   - Concurrent snapshot creation tests
   - **Estimated**: 3-4 hours

8. **Story #40**: Snapshot integration tests
   - Multi-threaded snapshot tests
   - Version isolation tests
   - Performance benchmarks (snapshot creation time)
   - **Estimated**: 2-3 hours

**Epic 7 Total**: 14-19 hours (2-3 days with 3 Claudes after #36)

---

### Epic 8: Transaction Read/Write Operations
**Goal**: Implement transaction operations (get, put, delete, CAS)
**Duration**: 2-3 days with 3 Claudes
**Stories**: 5-6 stories
**Parallelization**: After #41, limited parallelization

9. **Story #41**: Transaction initialization (begin)
   - TransactionContext::new() method
   - Allocate txn_id
   - Create snapshot
   - Initialize tracking sets
   - Unit tests for initialization
   - **Estimated**: 3-4 hours
   - **BLOCKS**: Stories #42-46

10. **Story #42**: Implement transaction get()
    - Read from write_set first (read-your-writes)
    - Check delete_set
    - Read from snapshot
    - Track in read_set
    - Unit tests for read operations
    - **Estimated**: 4-5 hours

11. **Story #43**: Implement transaction put()
    - Buffer writes to write_set
    - ensure_active() check
    - Unit tests for write buffering
    - **Estimated**: 2-3 hours

12. **Story #44**: Implement transaction delete()
    - Buffer deletes to delete_set
    - Remove from write_set if exists
    - Unit tests for delete buffering
    - **Estimated**: 2-3 hours

13. **Story #45**: Implement transaction CAS()
    - CASOperation creation
    - Add to cas_set
    - Unit tests for CAS buffering
    - **Estimated**: 2-3 hours

14. **Story #46**: Transaction operations integration tests
    - Read-your-writes validation
    - Multiple operations in one transaction
    - Operation ordering tests
    - **Estimated**: 3-4 hours

**Epic 8 Total**: 16-22 hours (2-3 days, limited parallelization after #41)

---

### Epic 9: Conflict Detection & Validation
**Goal**: Implement OCC validation logic
**Duration**: 2-3 days with 2-3 Claudes
**Stories**: 4-5 stories
**Parallelization**: Stories #48-50 can run in parallel

15. **Story #47**: Implement read-set validation
    - validate_read_set() function
    - Check current versions match read versions
    - Generate ReadConflict info
    - Unit tests for read conflicts
    - **Estimated**: 4-5 hours
    - **BLOCKS**: Story #51

16. **Story #48**: Implement write-set validation
    - validate_write_set() function
    - Check for concurrent writes
    - Generate WriteConflict info
    - Unit tests for write conflicts
    - **Estimated**: 4-5 hours

17. **Story #49**: Implement CAS validation
    - validate_cas_set() function
    - Check expected versions match current
    - Generate CASConflict info
    - Unit tests for CAS conflicts
    - **Estimated**: 3-4 hours

18. **Story #50**: Integrate validation phases
    - validate_transaction() orchestrator
    - Combine all validation checks
    - Return conflict list
    - Unit tests for combined validation
    - **Estimated**: 3-4 hours

19. **Story #51**: Validation integration tests
    - All conflict scenarios
    - Mixed conflict types
    - Validation performance tests
    - **Estimated**: 3-4 hours

**Epic 9 Total**: 17-22 hours (2-3 days with 2-3 Claudes)

---

### Epic 10: Transaction Commit & Abort
**Goal**: Implement transaction finalization
**Duration**: 2 days with 2 Claudes
**Stories**: 4 stories
**Parallelization**: Limited (sequential dependencies)

20. **Story #52**: Implement transaction commit logic
    - Set status to Validating
    - Call validate_transaction()
    - Handle validation success/failure
    - Set status to Committed/Aborted
    - Unit tests for commit flow
    - **Estimated**: 4-5 hours

21. **Story #53**: Implement apply_transaction()
    - Apply writes to storage
    - Apply deletes to storage
    - Apply CAS operations
    - Atomic application
    - Unit tests for application
    - **Estimated**: 4-5 hours

22. **Story #54**: Implement transaction abort
    - Discard buffered operations
    - Set status to Aborted
    - Unit tests for abort scenarios
    - **Estimated**: 2-3 hours

23. **Story #55**: Commit/abort integration tests
    - Full transaction lifecycle tests
    - Retry logic tests
    - Transaction status tests
    - **Estimated**: 3-4 hours

**Epic 10 Total**: 13-17 hours (2 days with 2 Claudes)

---

### Epic 11: WAL Transaction Support
**Goal**: Extend WAL for transaction boundaries
**Duration**: 2-3 days with 2 Claudes
**Stories**: 4-5 stories
**Parallelization**: Stories #58-59 can run in parallel

24. **Story #56**: Extend WALEntry for transactions
    - Add BeginTxn variant
    - Add CommitTxn variant
    - Add AbortTxn variant
    - Update serialization
    - Unit tests for new entries
    - **Estimated**: 3-4 hours

25. **Story #57**: Implement WAL transaction logging
    - Log BeginTxn at transaction start
    - Log Write/Delete during apply
    - Log CommitTxn/AbortTxn at end
    - Unit tests for transaction logging
    - **Estimated**: 4-5 hours

26. **Story #58**: WAL transaction batching
    - Batch writes within transaction
    - Atomic WAL append for transaction
    - Handle fsync for transaction boundaries
    - Unit tests for batching
    - **Estimated**: 4-5 hours

27. **Story #59**: WAL corruption handling for transactions
    - Detect incomplete transactions
    - Handle corruption mid-transaction
    - Unit tests for corruption scenarios
    - **Estimated**: 3-4 hours

28. **Story #60**: WAL transaction integration tests
    - Full transaction WAL tests
    - Crash during transaction tests
    - Large transaction tests
    - **Estimated**: 3-4 hours

**Epic 11 Total**: 17-22 hours (2-3 days with 2 Claudes)

---

### Epic 12: Recovery for Transactions
**Goal**: Replay transactions from WAL
**Duration**: 2-3 days with 2 Claudes
**Stories**: 4-5 stories
**Parallelization**: Stories #63-64 can run in parallel

29. **Story #61**: Implement transaction replay logic
    - Reconstruct TransactionContext from WAL
    - Track BeginTxn → Writes → CommitTxn
    - Discard incomplete transactions
    - Unit tests for replay
    - **Estimated**: 5-6 hours

30. **Story #62**: Handle incomplete transactions in recovery
    - Detect transactions without CommitTxn
    - Discard partial writes
    - Handle AbortTxn entries
    - Unit tests for incomplete transactions
    - **Estimated**: 4-5 hours

31. **Story #63**: Recovery integration with storage
    - Apply recovered transactions to UnifiedStore
    - Maintain version consistency
    - Rebuild indices
    - Integration tests
    - **Estimated**: 4-5 hours

32. **Story #64**: Crash simulation for transactions
    - Property-based crash tests
    - Crash during transaction tests
    - Verify no partial commits
    - **Estimated**: 4-5 hours

33. **Story #65**: Recovery performance tests
    - Large transaction recovery
    - Multiple transaction recovery
    - Performance benchmarks
    - **Estimated**: 3-4 hours

**Epic 12 Total**: 20-25 hours (2-3 days with 2 Claudes)

---

### Epic 13: Database Transaction API
**Goal**: Expose transaction API through Database
**Duration**: 2-3 days with 2-3 Claudes
**Stories**: 5-6 stories
**Parallelization**: Stories #68-69 can run in parallel

34. **Story #66**: Add Database::transaction() method
    - Accept closure with TransactionContext
    - Handle retry logic
    - Error handling
    - Unit tests for transaction API
    - **Estimated**: 4-5 hours

35. **Story #67**: Transaction coordinator in engine
    - Allocate transaction IDs
    - Manage snapshot creation
    - Track active transactions
    - Unit tests for coordinator
    - **Estimated**: 4-5 hours

36. **Story #68**: Backwards compatibility with M1 API
    - Wrap db.put() in implicit transaction
    - Wrap db.get() in read snapshot
    - Wrap db.delete() in implicit transaction
    - Unit tests for M1 API still working
    - **Estimated**: 3-4 hours

37. **Story #69**: Multi-primitive transaction support
    - Enable KV + other primitives in one transaction
    - Cross-primitive atomicity
    - Integration tests
    - **Estimated**: 4-5 hours

38. **Story #70**: Engine transaction integration tests
    - End-to-end transaction tests
    - Multi-run transaction tests
    - Error handling tests
    - **Estimated**: 4-5 hours

39. **Story #71**: Database transaction documentation
    - API documentation
    - Usage examples
    - Migration guide from M1
    - **Estimated**: 2-3 hours

**Epic 13 Total**: 21-27 hours (2-3 days with 2-3 Claudes)

---

### Epic 14: OCC Testing & Benchmarking
**Goal**: Comprehensive validation and performance testing
**Duration**: 2-3 days with 3 Claudes
**Stories**: 5 stories
**Parallelization**: All stories can run in parallel

40. **Story #72**: Multi-threaded conflict tests
    - Intentional conflict scenarios
    - High-contention workloads
    - Conflict rate measurement
    - **Estimated**: 4-5 hours

41. **Story #73**: Property-based transaction tests
    - Use proptest for random operations
    - Verify serializability
    - Check invariants
    - **Estimated**: 5-6 hours

42. **Story #74**: Performance benchmarks
    - Single-threaded throughput
    - Multi-threaded throughput
    - Conflict rate vs parallelism
    - Snapshot overhead
    - **Estimated**: 4-5 hours

43. **Story #75**: Edge case and stress tests
    - Long-running transactions
    - Large write sets
    - Timeout handling
    - Concurrent snapshot creation
    - **Estimated**: 4-5 hours

44. **Story #76**: M2 integration and regression tests
    - Full M2 end-to-end scenarios
    - Agent coordination patterns
    - Backwards compatibility validation
    - **Estimated**: 4-5 hours

**Epic 14 Total**: 21-26 hours (2-3 days with 3 Claudes, all parallel)

---

## Revised Summary

### Epic Count: 9 epics (was 4)
### Story Count: 44 stories (was 24)
### Total Estimated Time: ~140-170 hours sequential (~45-55 hours parallel with 3 Claudes)

### Epic Breakdown

| Epic | Stories | Estimated Time | Parallelization |
|------|---------|----------------|-----------------|
| **6: Concurrency Infrastructure** | 3 | 5-7 hrs | Sequential (foundation) |
| **7: Snapshot Management** | 5 | 14-19 hrs | 3 Claudes after #36 |
| **8: Transaction Operations** | 6 | 16-22 hrs | Limited after #41 |
| **9: Conflict Detection** | 5 | 17-22 hrs | 2-3 Claudes |
| **10: Commit & Abort** | 4 | 13-17 hrs | 2 Claudes |
| **11: WAL Transaction Support** | 5 | 17-22 hrs | 2 Claudes |
| **12: Recovery for Transactions** | 5 | 20-25 hrs | 2 Claudes |
| **13: Database Transaction API** | 6 | 21-27 hrs | 2-3 Claudes |
| **14: OCC Testing & Benchmarking** | 5 | 21-26 hrs | 3 Claudes (all parallel) |

### Critical Path

```
Epic 6 (foundation, 1 day)
  ↓
Epic 7 (snapshots, 2-3 days)
  ↓
Epic 8 (operations, 2-3 days)
  ↓
Epic 9 (validation, 2-3 days)
  ↓
Epic 10 (commit/abort, 2 days)
  ↓
[Epic 11 (WAL) + Epic 12 (Recovery) can partially overlap]
  ↓
Epic 13 (Engine API, 2-3 days)
  ↓
Epic 14 (Testing, 2-3 days)
```

**Total Duration**: ~18-22 days sequential, **~7-9 days parallel** (with 3 Claudes)

---

## Why This Structure Is Better

### 1. Matches M1 Granularity
- **Story size**: 2-6 hours (M1 pattern)
- **Epic focus**: Single component/concern (M1 pattern)
- **Testing**: Integrated into stories (M1 learning)

### 2. Better Parallelization
- **Epic 7**: 3 Claudes after foundation story
- **Epic 9**: 2-3 Claudes for validation types
- **Epic 14**: 3 Claudes all parallel

### 3. Clear Dependencies
- Each epic has 1-2 foundation stories that block parallelization
- Within-epic dependencies explicit
- Between-epic dependencies clear

### 4. Incremental Testing
- Unit tests in each story
- Integration tests at epic boundaries
- Final validation epic (Epic 14)

### 5. Lower Risk
- Smaller stories = easier to debug
- Focused epics = clearer scope
- Tests throughout = earlier bug detection

---

## Recommended Next Steps

1. **Review this analysis** with stakeholders
2. **Validate epic boundaries** against M2 architecture
3. **Create GitHub issues** following revised structure
4. **Update M2_PROJECT_STATUS.md** with new epic breakdown
5. **Begin implementation** with Epic 6

---

## Conclusion

The original 4-epic, 24-story breakdown was **too coarse-grained** for M2's complexity. The revised **9-epic, 44-story structure** matches M1's proven granularity and will enable:

- **2.5-3x speedup** with parallelization (same as M1)
- **Clear progress tracking** (44 milestones vs 24)
- **Better testing** (integrated, not deferred)
- **Lower risk** (smaller, focused changes)

M2 is more complex than M1 (OCC > basic storage), so it deserves **more epics** (9 vs 5), not fewer.
