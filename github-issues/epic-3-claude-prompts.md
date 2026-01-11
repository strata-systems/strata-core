# Epic 3: WAL Implementation - Claude Prompts

This file contains detailed prompts for each user story in Epic 3. Each prompt is designed to be copied and pasted to a separate Claude instance working on that specific story.

---

## Story #17: Define WAL entry types

### Prompt for Claude Instance 1

```
You are working on Story #17 of the in-mem database project (M1 Epic 3: WAL Implementation).

## Getting Started

1. If you haven't cloned the repository yet:
   git clone https://github.com/anibjoshi/in-mem.git
   cd in-mem

2. Start the story branch:
   ./scripts/start-story.sh 3 17 wal-entry-types

3. Read the full story context:
   /opt/homebrew/bin/gh issue view 17

## Your Task

Define WAL entry types with run_id in all entries to enable run-scoped replay.

**CRITICAL**: All WAL entries (except Checkpoint) MUST include run_id field. This is fundamental for:
- Run-scoped replay (filter WAL by run_id)
- Run diffing (compare WAL entries for two runs)
- Audit trails (track all operations per run)

## Key Requirements

1. **WALEntry enum with 6 variants**:
   - BeginTxn { txn_id, run_id, timestamp }
   - Write { run_id, key, value, version }
   - Delete { run_id, key, version }
   - CommitTxn { txn_id, run_id }
   - AbortTxn { txn_id, run_id }
   - Checkpoint { snapshot_id, version, active_runs: Vec<RunId> }

2. **All types implement**: Serialize, Deserialize, Debug, Clone, PartialEq

3. **Helper methods**:
   - run_id() -> Option<RunId>
   - txn_id() -> Option<u64>
   - version() -> Option<u64>
   - is_txn_boundary() -> bool
   - is_checkpoint() -> bool

## Files to Create

1. **crates/durability/Cargo.toml** (new crate):
   - Dependencies: core, serde, bincode, crc32fast, uuid

2. **crates/durability/src/lib.rs**:
   - Module structure: wal, encoding (stub), snapshot (stub), recovery (stub)

3. **crates/durability/src/wal.rs**:
   - WALEntry enum with all 6 variants
   - Helper methods
   - Unit tests for all variants
   - Serialization roundtrip tests

## Testing Requirements

- [ ] All 6 WALEntry variants can be created
- [ ] All entries serialize/deserialize correctly with bincode
- [ ] Helper methods return correct values
- [ ] is_txn_boundary and is_checkpoint work correctly
- [ ] Serialization roundtrip preserves all data
- [ ] cargo test -p durability passes

## Implementation Guidance

**See issue #17 for complete implementation code**. The issue contains:
- Full WALEntry enum definition
- All helper methods
- Complete test suite (7 tests)

**TDD Approach**:
1. Create durability crate in workspace
2. Add dependencies to Cargo.toml
3. Define WALEntry enum with serde derives
4. Implement helper methods
5. Write tests FIRST (see issue for test list)
6. Run tests, ensure all pass

## Quality Checks

Before completing:
1. Run tests: cargo test -p durability
2. Run clippy: cargo clippy -p durability -- -D warnings
3. Run format: cargo fmt -p durability
4. Verify all acceptance criteria met

## When Complete

Run the completion script:
./scripts/complete-story.sh 17

This will:
- Run all quality checks (build, test, clippy, format)
- Create PR to epic-3-wal-implementation
- Link PR to issue #17

## Questions?

- Read M1_ARCHITECTURE.md section on WAL design
- Read TDD_METHODOLOGY.md for testing approach
- Check EPIC_3_COORDINATION.md for dependencies
- Ask in issue #17 comments if blocked

**Estimated effort**: 3-4 hours
```

---

## Story #18: Implement entry encoding/decoding

### Prompt for Claude Instance 2

```
You are working on Story #18 of the in-mem database project (M1 Epic 3: WAL Implementation).

## Getting Started

1. If you haven't cloned the repository yet:
   git clone https://github.com/anibjoshi/in-mem.git
   cd in-mem

2. Start the story branch:
   ./scripts/start-story.sh 3 18 encoding-decoding

3. Read the full story context:
   /opt/homebrew/bin/gh issue view 18

## Your Task

Implement WAL entry encoding/decoding with CRC checksums for corruption detection.

**Entry Format**: [Length(4)][Type(1)][Payload(N)][CRC(4)]

**Why this format**:
- Length enables reading variable-sized entries
- Type tag enables forward compatibility (skip unknown types)
- CRC32 detects corruption (bit flips, partial writes)
- bincode serialization: fast, deterministic, compact

## Key Requirements

1. **encode_entry()**: Serialize WALEntry to bytes
   - Format: [length: u32][type: u8][payload: bytes][crc32: u32]
   - Returns Vec<u8> ready for file I/O

2. **decode_entry()**: Deserialize from bytes with CRC validation
   - Returns (WALEntry, bytes_consumed)
   - Returns CorruptionError on CRC mismatch
   - Includes offset in error for debugging

3. **Type tags**:
   - BeginTxn=1, Write=2, Delete=3, CommitTxn=4, AbortTxn=5, Checkpoint=6

## Files to Create/Update

1. **crates/durability/src/encoding.rs** (new file):
   - encode_entry() function
   - decode_entry() function
   - Type tag constants
   - Complete test suite

2. **Update crates/durability/src/lib.rs**:
   - Add: pub mod encoding;
   - Export: pub use encoding::{encode_entry, decode_entry};

## Testing Requirements

- [ ] Encoding/decoding roundtrip works for all entry types
- [ ] CRC32 detects corrupted payloads (bit flips)
- [ ] Truncated entries return CorruptionError
- [ ] Type tag verification catches mismatches
- [ ] Entry format matches specification
- [ ] cargo test -p durability passes

## Implementation Guidance

**See issue #18 for complete implementation code**. The issue contains:
- Full encode_entry() implementation
- Full decode_entry() implementation
- Type tag constants
- Complete test suite (6 tests)

**TDD Approach**:
1. Write test_encode_decode_roundtrip FIRST
2. Implement encode_entry() to make it pass
3. Implement decode_entry() to make it pass
4. Write test_crc_detects_corruption
5. Enhance CRC validation
6. Write remaining tests

**Critical**:
- CRC calculated over [type][payload], NOT including length
- Use crc32fast crate (already in dependencies)
- Errors must include offset for debugging

## Quality Checks

Before completing:
1. Run tests: cargo test -p durability
2. Test corruption detection specifically:
   cargo test -p durability test_crc_detects_corruption --nocapture
3. Run clippy: cargo clippy -p durability -- -D warnings
4. Run format: cargo fmt -p durability

## When Complete

Run the completion script:
./scripts/complete-story.sh 18

This will:
- Run all quality checks
- Create PR to epic-3-wal-implementation
- Link PR to issue #18

## Questions?

- Read M1_ARCHITECTURE.md section on WAL encoding
- Read TDD_METHODOLOGY.md for corruption testing approach
- Check issue #17 for WALEntry definition
- Ask in issue #18 comments if blocked

**Estimated effort**: 4-5 hours
```

---

## Story #19: Implement WAL file operations

### Prompt for Claude Instance 3

```
You are working on Story #19 of the in-mem database project (M1 Epic 3: WAL Implementation).

## Getting Started

1. If you haven't cloned the repository yet:
   git clone https://github.com/anibjoshi/in-mem.git
   cd in-mem

2. Start the story branch:
   ./scripts/start-story.sh 3 19 file-operations

3. Read the full story context:
   /opt/homebrew/bin/gh issue view 19

## Your Task

Implement WAL file operations: open, append, read, close.

**WAL is append-only**: Always write at end of file, sequential reads only.

## Key Requirements

1. **WAL struct**:
   - File handle (BufWriter for appends)
   - Path
   - Current offset (for error reporting)

2. **Operations**:
   - open() - Opens existing WAL or creates new one
   - append() - Writes encoded entry to file (no fsync yet)
   - read_entries(offset) - Scans from offset, yields decoded entries
   - read_all() - Scans from beginning
   - flush() - Flushes buffered writes
   - size() - Returns current file size

## Files to Create/Update

1. **Update crates/durability/src/wal.rs**:
   - Add WAL struct with file operations
   - Implement all methods
   - Complete test suite

2. **Add dev-dependency to crates/durability/Cargo.toml**:
   - tempfile = "3.8" (for tests)

## Testing Requirements

- [ ] WAL::open creates new file or opens existing
- [ ] append() writes entries to file
- [ ] read_all() reads back all entries correctly
- [ ] read_entries(offset) reads from specific offset
- [ ] Reopen WAL preserves previously written entries
- [ ] Multiple entries can be appended and read back
- [ ] cargo test -p durability passes

## Implementation Guidance

**See issue #19 for complete implementation code**. The issue contains:
- Complete WAL struct definition
- All file operation methods
- Complete test suite (6 tests)

**TDD Approach**:
1. Write test_open_new_wal FIRST
2. Implement WAL::open() to make it pass
3. Write test_append_and_read
4. Implement append() and read_all()
5. Write remaining tests (offset reads, reopen, multiple entries)
6. Ensure all tests pass

**Critical**:
- Use BufWriter for buffered appends (performance)
- Use BufReader for reads (performance)
- Track current_offset for error reporting
- Handle EOF gracefully in read_entries()
- Incomplete entries at EOF are expected (partial writes)

**File I/O Notes**:
- Create parent directories if needed
- Use OpenOptions::create(true).append(true).read(true)
- Separate file handles for read/write (don't interfere with buffering)

## Quality Checks

Before completing:
1. Run tests: cargo test -p durability
2. Test file operations specifically:
   cargo test -p durability test_append_and_read --nocapture
   cargo test -p durability test_reopen_wal --nocapture
3. Run clippy: cargo clippy -p durability -- -D warnings
4. Run format: cargo fmt -p durability

## When Complete

Run the completion script:
./scripts/complete-story.sh 19

This will:
- Run all quality checks
- Create PR to epic-3-wal-implementation
- Link PR to issue #19

## Dependencies

**Depends on**:
- Story #17 (WALEntry types)
- Story #18 (encoding/decoding)

Make sure those stories are merged to epic-3-wal-implementation before starting this one.

## Questions?

- Read M1_ARCHITECTURE.md section on WAL file format
- Read TDD_METHODOLOGY.md for file I/O testing
- Check issues #17 and #18 for encoding implementation
- Ask in issue #19 comments if blocked

**Estimated effort**: 5-6 hours
```

---

## Story #20: Add fsync with durability modes

### Prompt for Claude Instance 4

```
You are working on Story #20 of the in-mem database project (M1 Epic 3: WAL Implementation).

## Getting Started

1. If you haven't cloned the repository yet:
   git clone https://github.com/anibjoshi/in-mem.git
   cd in-mem

2. Start the story branch:
   ./scripts/start-story.sh 3 20 durability-modes

3. Read the full story context:
   /opt/homebrew/bin/gh issue view 20

## Your Task

Add configurable durability modes (strict, batched, async) with fsync.

**Why configurable**:
- Strict: Maximum durability (10ms+ latency per write)
- Batched (DEFAULT): Good balance (100ms or 1000 commits)
- Async: Maximum speed (risk losing recent commits)

**Agents prefer speed**: Losing 100ms of writes is acceptable; blocking 10ms per write is NOT.

## Key Requirements

1. **DurabilityMode enum**:
   - Strict - fsync after every commit
   - Batched { interval_ms, batch_size } - fsync every N commits OR T ms
   - Async { interval_ms } - background thread fsyncs periodically

2. **Default**: Batched { interval_ms: 100, batch_size: 1000 }

3. **WAL::open() takes DurabilityMode parameter**

4. **Strict mode**: flush() + fsync after every append

5. **Batched mode**: flush() + fsync when:
   - writes_since_fsync >= batch_size OR
   - time_since_fsync >= interval_ms

6. **Async mode**: Background thread fsyncs every interval_ms

## Files to Update

1. **crates/durability/src/wal.rs**:
   - Add DurabilityMode enum
   - Update WAL struct with mode tracking
   - Update append() to handle each mode
   - Add fsync() method
   - Add async background thread for Async mode
   - Update tests

## Testing Requirements

- [ ] Strict mode: fsync after every append
- [ ] Batched mode: fsync after batch_size commits
- [ ] Batched mode: fsync after interval_ms elapsed
- [ ] Async mode: background thread fsyncs periodically
- [ ] Drop handler calls final fsync
- [ ] All modes preserve durability guarantees
- [ ] cargo test -p durability passes

## Implementation Guidance

**See issue #20 for complete implementation code**. The issue contains:
- DurabilityMode enum with Default impl
- Updated WAL struct with Arc<Mutex<>> for shared access
- Complete append() implementation for all modes
- Background fsync thread for async mode
- Drop impl for cleanup
- Complete test suite (3 tests)

**TDD Approach**:
1. Write test_strict_mode FIRST
2. Implement Strict mode
3. Write test_batched_mode
4. Implement Batched mode (tracking, fsync logic)
5. Write test_async_mode
6. Implement Async mode (background thread)
7. Ensure all tests pass

**Critical**:
- Use Arc<Mutex<BufWriter<File>>> for shared writer access
- Use Arc<AtomicU64> for current_offset (thread-safe)
- Background thread must shutdown gracefully (Arc<AtomicBool>)
- Drop handler must call final fsync
- Batched mode tracks both time AND count

## Quality Checks

Before completing:
1. Run tests: cargo test -p durability
2. Test each mode specifically:
   cargo test -p durability test_strict_mode --nocapture
   cargo test -p durability test_batched_mode --nocapture
   cargo test -p durability test_async_mode --nocapture
3. Run clippy: cargo clippy -p durability -- -D warnings
4. Run format: cargo fmt -p durability

## When Complete

Run the completion script:
./scripts/complete-story.sh 20

This will:
- Run all quality checks
- Create PR to epic-3-wal-implementation
- Link PR to issue #20

## Dependencies

**Depends on**:
- Story #19 (file operations)

Make sure story #19 is merged to epic-3-wal-implementation before starting this one.

## Questions?

- Read M1_ARCHITECTURE.md section on durability strategy
- Read TDD_METHODOLOGY.md for async testing
- Check issue #19 for WAL file operations
- Ask in issue #20 comments if blocked

**Estimated effort**: 5-6 hours
```

---

## Story #21: Add CRC/checksums for corruption detection

### Prompt for Claude Instance 5

```
You are working on Story #21 of the in-mem database project (M1 Epic 3: WAL Implementation).

## Getting Started

1. If you haven't cloned the repository yet:
   git clone https://github.com/anibjoshi/in-mem.git
   cd in-mem

2. Start the story branch:
   ./scripts/start-story.sh 3 21 crc-checksums

3. Read the full story context:
   /opt/homebrew/bin/gh issue view 21

## Your Task

Add corruption detection via CRC checks and simulation tests.

**NOTE**: CRC32 is already implemented in encoding layer (story #18). This story focuses on TESTING corruption detection thoroughly.

## Key Requirements

1. **Corruption detection** (already exists):
   - decode_entry() detects CRC mismatches
   - Returns CorruptionError with offset

2. **Simulation tests** (NEW):
   - Flip random bit in WAL → verify detection
   - Truncate WAL mid-entry → verify graceful stop
   - BeginTxn without CommitTxn → verify incomplete transaction handling
   - Recovery handles corruption by stopping at first bad entry

## Files to Create

1. **crates/durability/tests/corruption_test.rs** (new file):
   - test_crc_detects_bit_flip
   - test_truncated_entry_handling
   - test_incomplete_transaction_discarded
   - test_multiple_corruption_points
   - test_valid_wal_after_crash_simulation
   - test_crc_on_all_entry_types

## Testing Requirements

- [ ] CRC detects bit flips in WAL entries
- [ ] Truncated entries are handled gracefully (stop at corruption, don't crash)
- [ ] Incomplete transactions (no CommitTxn) are detected
- [ ] Multiple corruption points cause recovery to stop at first error
- [ ] All entry types have CRC protection
- [ ] Crash simulation tests verify recovery behavior
- [ ] cargo test -p durability passes

## Implementation Guidance

**See issue #21 for complete implementation code**. The issue contains:
- Complete corruption_test.rs file
- 6 comprehensive simulation tests
- File corruption techniques (flip bits, truncate)

**TDD Approach**:
1. Write test_crc_detects_bit_flip FIRST
2. Verify CRC detection works (should already pass from story #18)
3. Write test_truncated_entry_handling
4. Verify graceful EOF handling
5. Write remaining tests
6. Ensure all corruption scenarios are caught

**Critical Testing Patterns**:
- Use tempfile::TempDir for test isolation
- Write valid WAL, then corrupt it
- Use OpenOptions to modify WAL files directly
- Verify error messages include offsets
- Test all corruption types: header, payload, CRC, truncation

**Corruption Techniques**:
- Bit flip: `file.seek(offset); buf[0] ^= 0xFF; file.write(buf);`
- Truncate: `file.set_len(new_size);`
- Remove bytes: truncate to smaller size

## Quality Checks

Before completing:
1. Run tests: cargo test -p durability
2. Run corruption tests specifically:
   /Users/aniruddhajoshi/.cargo/bin/cargo test -p durability --test corruption_test --nocapture
3. Verify all 6 corruption scenarios tested
4. Run clippy: /Users/aniruddhajoshi/.cargo/bin/cargo clippy -p durability -- -D warnings
5. Run format: /Users/aniruddhajoshi/.cargo/bin/cargo fmt -p durability

## When Complete

Run the completion script:
./scripts/complete-story.sh 21

This will:
- Run all quality checks
- Create PR to epic-3-wal-implementation
- Link PR to issue #21

## Dependencies

**Depends on**:
- Story #18 (encoding with CRC)
- Story #19 (file operations)
- Story #20 (durability modes)

Make sure those stories are merged to epic-3-wal-implementation before starting this one.

## Questions?

- Read M1_ARCHITECTURE.md section on corruption handling
- Read TDD_METHODOLOGY.md: "WAL: Corruption tests EARLY"
- Check issue #18 for CRC implementation
- Ask in issue #21 comments if blocked

**Estimated effort**: 4-5 hours
```

---

## Story #22: Write corruption simulation tests

### Prompt for Claude Instance 6

```
You are working on Story #22 of the in-mem database project (M1 Epic 3: WAL Implementation).

## Getting Started

1. If you haven't cloned the repository yet:
   git clone https://github.com/anibjoshi/in-mem.git
   cd in-mem

2. Start the story branch:
   ./scripts/start-story.sh 3 22 corruption-simulation

3. Read the full story context:
   /opt/homebrew/bin/gh issue view 22

## Your Task

Write comprehensive corruption simulation tests covering all failure scenarios.

**This story goes beyond story #21** by testing:
- Power loss (partial writes)
- Disk errors (bit flips in multiple locations)
- Filesystem bugs (truncation, garbage data)
- Multi-failure scenarios

## Key Requirements

1. **Test scenarios**:
   - Corrupt entry header (length field)
   - Corrupt entry payload (multiple locations)
   - Missing CRC bytes (truncated)
   - Multiple entries, first corrupt
   - Multiple entries, middle corrupt
   - Valid entries after corruption (should NOT be read)
   - Interleaved valid/corrupt entries
   - Error messages include file offset
   - Power loss simulation (partial writes)
   - Filesystem bug simulation (garbage data)

2. **All tests must**:
   - Use real file I/O (catch platform-specific issues)
   - Verify error messages provide debugging info
   - Pass consistently (no flaky failures)
   - Demonstrate fail-safe recovery (stop at error, don't propagate)

## Files to Create

1. **crates/durability/tests/corruption_simulation_test.rs** (new file):
   - Helper functions for corruption
   - 10 comprehensive simulation tests
   - Covers all failure modes

## Testing Requirements

- [ ] Corrupt entry headers cause clear errors with offsets
- [ ] Corrupt payloads detected by CRC
- [ ] Truncated entries handled gracefully
- [ ] Multiple corruptions: stops at first
- [ ] Valid entries after corruption NOT read (conservative)
- [ ] Interleaved corruption handled correctly
- [ ] Error messages include file offsets for debugging
- [ ] Power loss simulation (partial writes) handled
- [ ] Filesystem bugs (garbage data) handled
- [ ] All tests pass consistently
- [ ] cargo test -p durability passes

## Implementation Guidance

**See issue #22 for complete implementation code**. The issue contains:
- Complete corruption_simulation_test.rs file
- Helper functions: write_entries, corrupt_at_offset, truncate_file
- 10 comprehensive simulation tests

**TDD Approach**:
1. Write helper functions FIRST (write_entries, corrupt_at_offset, etc.)
2. Write test_corrupt_entry_length_field
3. Write test_corrupt_entry_payload
4. Write remaining tests systematically
5. Ensure all corruption types are covered
6. Verify tests are deterministic (run multiple times)

**Critical Testing Patterns**:
- Helper: write_entries(wal, run_id, count) - write N valid entries
- Helper: corrupt_at_offset(path, offset, bytes) - corrupt specific location
- Helper: truncate_file(path, size) - truncate to size
- All tests use tempfile::TempDir for isolation
- Verify errors with: assert!(result.is_err())
- Check error messages: err_msg.contains("CRC") or err_msg.contains("offset")

**Corruption Scenarios**:
- **Header corruption**: Set length to 0xFFFFFFFF
- **Payload corruption**: Flip bits at various offsets
- **CRC corruption**: Truncate last 2 bytes of entry
- **Power loss**: Write partial entry (header only, no payload/CRC)
- **Filesystem bug**: Append zeros to valid WAL

## Quality Checks

Before completing:
1. Run tests: /Users/aniruddhajoshi/.cargo/bin/cargo test -p durability
2. Run simulation tests specifically:
   /Users/aniruddhajoshi/.cargo/bin/cargo test -p durability --test corruption_simulation_test --nocapture
3. Run tests multiple times to verify determinism:
   for i in {1..5}; do /Users/aniruddhajoshi/.cargo/bin/cargo test -p durability --test corruption_simulation_test; done
4. Run clippy: /Users/aniruddhajoshi/.cargo/bin/cargo clippy -p durability -- -D warnings
5. Run format: /Users/aniruddhajoshi/.cargo/bin/cargo fmt -p durability

## When Complete

Run the completion script:
./scripts/complete-story.sh 22

This will:
- Run all quality checks
- Create PR to epic-3-wal-implementation
- Link PR to issue #22

## Dependencies

**Depends on**:
- Story #21 (CRC checksums and basic corruption tests)

Make sure story #21 is merged to epic-3-wal-implementation before starting this one.

## Questions?

- Read M1_ARCHITECTURE.md section on recovery protocol
- Read TDD_METHODOLOGY.md: "WAL: Corruption tests EARLY"
- Check issue #21 for basic corruption tests
- Ask in issue #22 comments if blocked

**Estimated effort**: 5-6 hours
```

---

## Notes for Coordination

### Story Dependencies

**Sequential dependencies**:
- Story #17 (WAL entry types) → MUST complete first (blocks #18, #19)
- Story #18 (Encoding/decoding) → Can start after #17
- Story #19 (File operations) → Depends on #17, #18
- Story #20 (Durability modes) → Depends on #19
- Story #21 (CRC/checksums) → Depends on #18, #19, #20
- Story #22 (Corruption simulation) → Depends on #21

**Parallelization opportunities**:
- After #17 merges: #18 and #19 can run in parallel (NO dependencies between them)
- After #18, #19 merge: #20 can start
- After #20 merges: #21 can start
- After #21 merges: #22 can start

### Fully Qualified Paths

All prompts use:
- `/opt/homebrew/bin/gh` for GitHub CLI
- `/Users/aniruddhajoshi/.cargo/bin/cargo` for Rust commands (in quality checks)
- `./scripts/start-story.sh` and `./scripts/complete-story.sh` for workflow

### Testing Standards

All stories must pass:
- `cargo test -p durability`
- `cargo clippy -p durability -- -D warnings`
- `cargo fmt -p durability --check`

Epic 3 completion requires:
- All 6 stories merged to epic-3-wal-implementation
- 95%+ test coverage for durability crate
- All corruption scenarios tested
- No clippy warnings
- No failing tests
