# Concurrency Crate Audit

## Status: All findings resolved

## Date: 2026-01-29

## Context

After consolidating the commit protocol (see `COMMIT_PROTOCOL_CONSOLIDATION.md`),
the concurrency crate was re-audited. The crate is well-structured overall (83 tests,
clean architecture, no circular dependencies), but the consolidation exposed dead
code both within the concurrency crate and in the engine's durability module.

All findings have been resolved.

---

## ~~HIGH PRIORITY: Engine Durability Module is Dead Code~~ RESOLVED

**Resolved**: Removed the entire `crates/engine/src/durability/` module (~1,416 lines),
its re-exports from `lib.rs`, and the redundant `fsync()` call in `commit_internal()`.
`DurabilityMode` is now re-exported directly from `strata_durability::wal::DurabilityMode`.
The WAL's `append()` method handles all fsync behavior internally based on its
`DurabilityMode`.

---

## ~~MEDIUM PRIORITY: Dead Public Exports from Concurrency Crate~~ RESOLVED

**Resolved**: Trimmed `pub use` in `crates/concurrency/src/lib.rs` to only
re-export items with external callers. Made `conflict` and `validation` modules
`pub(crate)`. Removed `abort()` and `commit_or_rollback()` from
`TransactionManager`. Demoted `RecoveryCoordinator::with_snapshot_path()` and
`ClonedSnapshotView::from_arc()` to `pub(crate)`.

Remaining exports: `TransactionManager`, `TransactionContext`, `CommitError`,
`JsonStoreExt`, `TransactionStatus`, `RecoveryCoordinator`, `RecoveryResult`,
`RecoveryStats`, `ClonedSnapshotView`, `TransactionWALWriter`, `SnapshotView`.

---

## ~~MEDIUM PRIORITY: Dead WAL Writer Methods~~ RESOLVED

**Resolved**: Removed `write_abort()` and all 4 vector WAL methods
(`write_vector_collection_create`, `write_vector_collection_delete`,
`write_vector_upsert`, `write_vector_delete`) from `TransactionWALWriter`,
along with the `write_abort` test.

---

## ~~LOW PRIORITY: No-Op Validation Function~~ RESOLVED

**Resolved**: Removed `validate_write_set()` and its call in
`validate_transaction()`. The design rationale (blind writes don't conflict,
per spec Section 3.2) is documented in `validate_transaction()`'s doc comment.

---

## ~~LOW PRIORITY: Field Access Inconsistency~~ RESOLVED

**Resolved**: The only location that accessed `TransactionContext` fields
directly from the engine was `crates/engine/src/durability/traits.rs`, which
was removed as part of the HIGH PRIORITY fix. No other engine code accesses
these fields directly (verified via grep).

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
