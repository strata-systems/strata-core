## Proposed M2 Epic Structure

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