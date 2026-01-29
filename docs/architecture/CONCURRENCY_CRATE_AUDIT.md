# Concurrency Crate Audit

## Status: Findings documented, action pending

## Date: 2026-01-29

## Context

After consolidating the commit protocol (see `COMMIT_PROTOCOL_CONSOLIDATION.md`),
the concurrency crate was re-audited. The crate is well-structured overall (83 tests,
clean architecture, no circular dependencies), but the consolidation exposed dead
code both within the concurrency crate and in the engine's durability module.

---

## ~~HIGH PRIORITY: Engine Durability Module is Dead Code~~ RESOLVED

**Resolved**: Removed the entire `crates/engine/src/durability/` module (~1,416 lines),
its re-exports from `lib.rs`, and the redundant `fsync()` call in `commit_internal()`.
`DurabilityMode` is now re-exported directly from `strata_durability::wal::DurabilityMode`.
The WAL's `append()` method handles all fsync behavior internally based on its
`DurabilityMode`.

---

## MEDIUM PRIORITY: Dead Public Exports from Concurrency Crate

These items are exported from `crates/concurrency/src/lib.rs` but have zero
external callers. They should be removed from `pub use` (made `pub(crate)` or removed).

### Manager methods

| Item | File | Notes |
|------|------|-------|
| `commit_or_rollback()` | manager.rs:281 | Never called. Engine uses `TransactionCoordinator::commit()` which handles errors itself. |
| `TransactionManager::abort()` | manager.rs:272 | Only called in a test. Engine calls `txn.mark_aborted()` directly. |

### Transaction types

| Item | File | Notes |
|------|------|-------|
| `ApplyResult` | transaction.rs:77 | Return type of `apply_writes()` but always discarded by `TransactionManager::commit()`. |
| `PendingOperations` | transaction.rs:97 | Debugging struct returned by `pending_operations()`. Zero external callers. |
| `CASOperation` | transaction.rs | Only used internally by validation. |
| `JsonPathRead`, `JsonPatchEntry` | transaction.rs | Only used internally by validation/conflict. |

### Validation functions (all 6)

| Item | File | Notes |
|------|------|-------|
| `validate_transaction` | validation.rs | Only called internally by `TransactionContext::commit()`. |
| `validate_read_set` | validation.rs | Only called internally by `validate_transaction()`. |
| `validate_write_set` | validation.rs | No-op function (always returns ok). |
| `validate_cas_set` | validation.rs | Only called internally. |
| `validate_json_set` | validation.rs | Only called internally. |
| `validate_json_paths` | validation.rs | Only called internally. |
| `ConflictType` | validation.rs | Only used internally. |
| `ValidationResult` | validation.rs | Only used internally. Engine converts via `StrataError::from(CommitError)`. |

### Conflict module (all 9 items)

| Item | File | Notes |
|------|------|-------|
| `check_all_conflicts` | conflict.rs | Never called. |
| `check_read_write_conflicts` | conflict.rs | Never called. |
| `check_version_conflicts` | conflict.rs | Never called. |
| `check_write_write_conflicts` | conflict.rs | Only called internally by `validate_json_paths`. |
| `find_first_read_write_conflict` | conflict.rs | Only called by `check_all_conflicts` (which is itself never called). |
| `find_first_version_conflict` | conflict.rs | Only called by `check_all_conflicts`. |
| `find_first_write_write_conflict` | conflict.rs | Only called by `check_all_conflicts`. |
| `ConflictResult` | conflict.rs | Only used internally. |
| `JsonConflictError` | conflict.rs | Only used internally. |

### Recovery and snapshot methods

| Item | File | Notes |
|------|------|-------|
| `RecoveryCoordinator::with_snapshot_path()` | recovery.rs:58 | Only called in a test. Documented as "M3+ feature". |
| `ClonedSnapshotView::from_arc()` | snapshot.rs:96 | Only called in tests. |

**Action**: Remove all items above from `pub use` in `lib.rs`. Make them
`pub(crate)` where they're still used internally, or remove entirely if unused.

---

## MEDIUM PRIORITY: Dead WAL Writer Methods

**Location**: `crates/concurrency/src/wal_writer.rs`

| Method | Lines | Callers |
|--------|-------|---------|
| `write_abort()` | 128-135 | Only in wal_writer tests. Spec says aborted transactions don't need WAL entries. |
| `write_vector_collection_create()` | 158-178 | Zero callers anywhere. |
| `write_vector_collection_delete()` | 181-201 | Zero callers anywhere. |
| `write_vector_upsert()` | 205-233 | Zero callers anywhere. |
| `write_vector_delete()` | 236-252 | Zero callers anywhere. |

The 4 vector WAL methods were built for future vector WAL integration but were
never connected. The vector store in the engine does not use WAL for its operations.

**Action**: Remove all 5 methods and their tests.

---

## LOW PRIORITY: No-Op Validation Function

**Location**: `crates/concurrency/src/validation.rs:195`

`validate_write_set()` always returns `ValidationResult::ok()`. It's called on
every commit via `validate_transaction()` but does nothing. A comment explains
that blind writes don't conflict (by design), so write-set validation is a no-op.

**Action**: Remove the function and its call in `validate_transaction()`. The
design rationale (blind writes don't conflict) is already documented elsewhere.

---

## LOW PRIORITY: Field Access Inconsistency

The engine accesses `TransactionContext` internal fields directly
(`txn.write_set`, `txn.delete_set`, `txn.cas_set`) rather than using the
accessor methods (`write_count()`, `delete_count()`, `cas_count()`). This
makes the accessor methods dead code externally.

**Location**: `crates/engine/src/durability/traits.rs:170-173` (this file is
itself dead code per the high-priority finding above)

**Action**: Once the durability module is removed, verify no other engine code
accesses these fields directly. If the fields are only accessed internally by
the concurrency crate, consider making them `pub(crate)` instead of `pub`.

---

## Items That ARE Properly Used Externally

For reference, these concurrency crate exports are actively used:

| Item | Used By |
|------|---------|
| `TransactionManager` | engine coordinator.rs |
| `TransactionContext` | engine (database, transaction, primitives), executor session |
| `CommitError` | engine coordinator.rs (From impl) |
| `RecoveryCoordinator` | engine database/mod.rs |
| `RecoveryResult` | engine coordinator.rs |
| `RecoveryStats` | engine coordinator.rs, vector recovery |
| `ClonedSnapshotView` | engine transaction context/pool |
| `TransactionWALWriter` | engine vector/store.rs |
| `JsonStoreExt` | engine transaction context, json primitive, extensions, run handle |
| `TransactionStatus` | concurrency internal (via `txn.status` public field) |
| `SnapshotView` (re-export) | engine, storage |

---

## Estimated Cleanup Impact

| Action | Lines removed | Files affected |
|--------|--------------|----------------|
| Remove engine durability module | ~500 | 5 (4 files + mod.rs) |
| Trim concurrency public exports | ~0 (just `pub use` changes) | 1 (lib.rs) |
| Remove dead WAL writer methods | ~130 | 1 (wal_writer.rs) |
| Remove `validate_write_set` no-op | ~20 | 1 (validation.rs) |
| Remove `commit_or_rollback` | ~15 | 1 (manager.rs) |
| **Total** | **~665 lines** | **9 files** |
