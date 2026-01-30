# Storage Crate: Persistence Module Analysis

## Context

The storage crate contains 6 modules (22 files, 205 tests) that form a cohesive
on-disk persistence system. None of these modules are currently wired into the
higher layers (engine, executor). This document analyzes what they provide and
how they relate to the existing durability crate.

---

## The Two Persistence Systems

Strata currently has **two** persistence implementations at different maturity levels:

| Aspect | Durability Crate (active) | Storage Modules (unwired) |
|--------|--------------------------|--------------------------|
| WAL format | Single JSON file, `serde_json` | Segmented binary files, CRC32 per-record |
| Snapshot format | Variable-length header, no codec | Fixed 64-byte header, codec abstraction, database UUID |
| Crash safety | Basic write pattern | write-fsync-rename-fsync-parent (standard practice) |
| Integrity | Corruption detection on read | CRC32 per-record (WAL) and per-section (snapshots) |
| WAL lifecycle | Single file, grows forever | Segmented with compaction (remove segments covered by snapshots) |
| Version retention | None | Policy-based (KeepAll, KeepLast(n), KeepFor(duration), Composite) |
| Durability modes | None / Strict / Batched | Always-sync (via segment sync) |
| Test count | ~40 | 205 |

The durability crate is the simpler system that works today. The storage modules
are a more production-grade system designed to replace it.

---

## Module 1: `format/` — Binary On-Disk Formats (69 tests)

**What it provides:** The byte-level serialization formats for everything that
touches disk. This is the foundation layer — all other persistence modules build
on these formats.

### Sub-components

**`snapshot.rs`** — Snapshot file format (`.chk` files)
- Fixed 64-byte `SnapshotHeader`: magic bytes, format version, snapshot ID,
  watermark txn, creation timestamp, database UUID, codec ID
- `SectionHeader`: primitive type tag + data length
- Path utilities: `snapshot_path()`, `find_latest_snapshot()`, `list_snapshots()`,
  `parse_snapshot_id()`
- 10 tests covering header roundtrip, magic/version validation, path operations

**`wal_record.rs`** — Segmented WAL format (`wal_NNNNNNNN.seg` files)
- Fixed 32-byte `SegmentHeader`: magic bytes, format version, segment number,
  creation timestamp, database UUID
- `WalRecord`: txn_id + run_id + timestamp + CRC32-protected writeset bytes
- `WalSegment`: lifecycle management (create, open, append, sync, close)
- 10 tests covering header roundtrip, segment lifecycle, record integrity,
  checksum failure detection

**`writeset.rs`** — Mutation batches
- `Mutation` enum: Put(key, value), Delete(key), Append(key, value)
- `EntityRef` enum: target routing per primitive type (Kv, Event, State, Run, Json, Vector)
- `Writeset`: ordered list of (EntityRef, Mutation) pairs with binary serialization
- 12 tests covering all mutation types, all entity ref variants, edge cases

**`manifest.rs`** — Database metadata (single `MANIFEST` file)
- `Manifest`: database UUID, active WAL segment, snapshot watermark, codec ID
- `ManifestManager`: atomic write-fsync-rename persistence; in-memory + on-disk state
- 14 tests covering roundtrip, atomic persistence, corruption detection

**`primitives.rs`** — Per-primitive snapshot entries
- Typed entry structs: `KvSnapshotEntry`, `EventSnapshotEntry`, `StateSnapshotEntry`,
  `RunSnapshotEntry`, `JsonSnapshotEntry`, `VectorSnapshotEntry`,
  `VectorCollectionSnapshotEntry`
- `SnapshotSerializer`: wraps a `StorageCodec` to encode/decode typed entries
- 10 tests covering roundtrip for all primitive types including 384-dim vectors

**`watermark.rs`** — Snapshot coverage tracking
- `SnapshotWatermark`: tracks which transactions are covered by snapshots
- `CheckpointInfo`: snapshot metadata (id, txn coverage, timestamp)
- Key methods: `is_covered(txn_id)`, `needs_replay(txn_id)`
- Used by compaction to decide which WAL segments are safe to remove
- 13 tests covering coverage logic, replay decisions, serialization

### Relationship to durability crate
The durability crate uses `serde_json` for WAL entries and a simpler snapshot
format without CRC32 integrity or codec support. The format module is designed
to replace this with production-grade binary formats.

---

## Module 2: `disk_snapshot/` — Crash-Safe Snapshot I/O (23 tests)

**What it provides:** The read and write path for snapshot files, plus a
checkpoint coordinator that orchestrates the full checkpoint flow.

### Sub-components

**`writer.rs`** — `SnapshotWriter`
- Crash-safe write pattern: write to `.tmp` file, fsync, atomic rename to `.chk`,
  fsync parent directory
- Writes `SnapshotHeader` followed by `SectionHeader` + data for each primitive
- Temp file cleanup on both success and failure paths
- 7 tests covering file format, crash safety, multi-snapshot writes

**`reader.rs`** — `SnapshotReader`
- Validates magic bytes, format version, codec ID, CRC32 integrity
- Returns `LoadedSnapshot` containing `LoadedSection`s (primitive tag + raw bytes)
- 9 tests covering load, CRC validation, corruption detection, all section types

**`checkpoint.rs`** — `CheckpointCoordinator`
- Orchestrates: serialize primitives -> write snapshot -> update watermark
- Maintains `SnapshotWatermark` across checkpoints
- `CheckpointData` builder for optional per-primitive-type data
- 7 tests covering empty/full checkpoints, watermark tracking, temp cleanup

### Relationship to durability crate
The durability crate has `SnapshotWriter`/`SnapshotReader` in `snapshot.rs` with
a simpler design (no codec, no database UUID, no fixed header, variable-length
format). The disk_snapshot module is the next-generation replacement with
stronger crash safety guarantees and extensibility.

---

## Module 3: `compaction/` — WAL Segment Cleanup (35 tests)

**What it provides:** Lifecycle management for WAL segments after snapshots make
them redundant, plus tombstone tracking for deleted entries.

### Sub-components

**`wal_only.rs`** — `WalOnlyCompactor`
- Reads WAL segments, finds max txn_id per segment
- Removes segments where `max_txn_id <= watermark` (covered by snapshot)
- Never removes the active segment (safety invariant)
- Reports `CompactInfo`: bytes reclaimed, segments removed, time elapsed
- 9 tests covering segment removal logic, safety invariants, edge cases

**`tombstone.rs`** — `TombstoneIndex`
- Tracks deleted entries with metadata: key, version, reason, timestamp, run_id
- Reasons: UserDelete, RetentionPolicy, Compaction
- Query methods: `is_tombstoned()`, `get_by_run()`, `get_by_reason()`,
  `cleanup_before()`
- Binary serialization for persistence
- 15 tests covering CRUD, queries, cleanup, serialization

**`mod.rs`** — `CompactMode`
- `WALOnly`: remove covered segments (fast, no data loss)
- `Full`: also apply retention policies (prunes old versions)
- 11 tests covering mode behavior, metrics, error handling

### Why this matters
Without compaction, WAL files grow without bound. This module is essential for
any deployment that runs longer than a single session. The segmented WAL design
in `format/wal_record.rs` is specifically designed to enable per-segment removal.

---

## Module 4: `retention/` — Version Retention Policies (16 tests)

**What it provides:** Configurable policies for how many historical versions to
keep per key, enabling bounded storage growth.

### Sub-components

**`policy.rs`** — `RetentionPolicy`
- `KeepAll`: never discard (default)
- `KeepLast(n)`: keep the n most recent versions
- `KeepFor(duration)`: keep versions newer than a time threshold
- `Composite`: per-primitive-type overrides with a default fallback
- `CompositeBuilder`: fluent API for building composite policies
- Binary serialization for persistence
- `should_retain(age, version_count)` evaluation method
- 12 tests covering all policy types, edge cases, serialization

**`mod.rs`** — System namespace utilities
- `_system/retention/{hex_run_id}` key format for storing policies in the database
- 4 tests covering key format, run_id extraction

### Why this matters
The versioning API design (`docs/design/versioning-api.md`) describes a system
where "data is never overwritten - every write creates a new version." Without
retention policies, version history grows without bound. This module provides
the mechanism to control that growth while honoring configurable preservation
rules per primitive type.

---

## Module 5: `codec/` — Storage Codec Abstraction (16 tests)

**What it provides:** An abstraction layer for data encoding/decoding, providing
a seam for future encryption or compression.

### Sub-components

**`traits.rs`** — `StorageCodec` trait
- `encode(&[u8]) -> Vec<u8>`, `decode(&[u8]) -> Vec<u8>`, `codec_id() -> &str`
- Requires `Send + Sync` for thread safety
- Object-safe (can be used as `Box<dyn StorageCodec>`)
- 7 tests covering object safety, roundtrip, error handling

**`identity.rs`** — `IdentityCodec`
- Pass-through (no transformation), `codec_id() = "identity"`
- 7 tests covering encode/decode/roundtrip

**`mod.rs`** — `get_codec()` factory
- Maps codec ID string to `Box<dyn StorageCodec>`
- Currently only returns `IdentityCodec`; returns error for unknown IDs
- 2 tests

### Why this matters
The codec abstraction is woven into the snapshot format header, the manifest, and
the snapshot reader's validation path. It's the extension point for adding
encryption-at-rest or compression without changing any other module. The identity
codec is the baseline; swapping in a real codec requires only implementing the
trait and registering it in `get_codec()`.

---

## Module 6: `testing/` — Test Infrastructure (19 tests)

**What it provides:** Reusable test infrastructure for crash testing and state
verification during persistence operations.

### Sub-components

**`crash_harness.rs`** — `CrashConfig`
- 8 crash injection points throughout the persistence lifecycle:
  `BeforeWalWrite`, `DuringWalWrite`, `AfterWalWrite`, `BeforeSnapshot`,
  `DuringSnapshot`, `AfterSnapshot`, `BeforeManifestUpdate`, `DuringCompaction`
- Each point has an expected `DataState`: Committed, Uncommitted,
  PartiallyWritten, Unknown
- `CrashConfig` builder: crash type (Kill, Panic, IoError), probability, target point
- 7 tests covering configuration, crash point expectations

**`reference_model.rs`** — `ReferenceModel`
- In-memory oracle tracking expected state across KV, events, state values
- `Operation` enum: KvPut, KvDelete, EventAppend, StateSet, Checkpoint
- Comparison methods: `compare_kv()`, `compare_events()`, `compare_state()`
  produce `Vec<StateMismatch>` diffs
- 12 tests covering operations, comparisons, reset

### Why this matters
Crash testing is critical for persistence correctness. The harness defines the
contract: "if a crash happens at point X, the data state should be Y." The
reference model enables property-based testing: apply operations to both the
real system and the model, then verify they agree after recovery.

---

## Architecture: How the Modules Fit Together

```
                    ┌─────────────────────┐
                    │  CheckpointCoordinator  │ (disk_snapshot/checkpoint)
                    │  - orchestrates full    │
                    │    checkpoint flow      │
                    └────────┬────────────────┘
                             │ uses
                ┌────────────┼──────────────┐
                │            │              │
    ┌───────────▼──┐  ┌──────▼──────┐  ┌───▼──────────────┐
    │ SnapshotWriter│  │ Snapshot    │  │ SnapshotWatermark│
    │ SnapshotReader│  │ Serializer  │  │ (format/watermark)│
    │ (disk_snapshot)│  │ (format/    │  └───┬──────────────┘
    └───────┬───────┘  │  primitives)│      │
            │          └──────┬──────┘      │ used by
            │ uses            │ uses        │
    ┌───────▼───────┐  ┌─────▼──────┐  ┌───▼──────────────┐
    │ SnapshotHeader│  │ StorageCodec│  │ WalOnlyCompactor │
    │ SectionHeader │  │ (codec/)    │  │ (compaction/)    │
    │ (format/      │  └────────────┘  └───┬──────────────┘
    │  snapshot)    │                      │ reads
    └───────────────┘               ┌──────▼──────────────┐
                                    │ WalSegment, WalRecord│
    ┌───────────────┐               │ (format/wal_record)  │
    │ ManifestManager│◄─────────────┤                      │
    │ (format/       │  referenced   └──────────────────────┘
    │  manifest)     │  by compactor
    └───────────────┘
                                    ┌──────────────────────┐
    ┌───────────────┐               │ RetentionPolicy      │
    │ TombstoneIndex│◄──────────────│ (retention/)         │
    │ (compaction/  │  used by Full │                      │
    │  tombstone)   │  compaction   └──────────────────────┘
    └───────────────┘
                                    ┌──────────────────────┐
                                    │ CrashConfig          │
                                    │ ReferenceModel       │
                                    │ (testing/)           │
                                    └──────────────────────┘
```

The modules form a layered system:
1. **format/** defines the byte-level contracts (bottom layer)
2. **codec/** provides the encoding extension point
3. **disk_snapshot/** and **compaction/** implement I/O operations on top of the formats
4. **retention/** provides the policy framework for version lifecycle
5. **testing/** provides verification infrastructure for the entire stack

---

## Relationship to Versioning API

The versioning API design (`docs/design/versioning-api.md`) describes a system
where every write creates a new version and version history is queryable. Two
of these modules are directly relevant:

- **retention/**: Without retention policies, version history grows without
  bound. `KeepLast(n)` and `KeepFor(duration)` provide the controls.
- **compaction/**: Without compaction, WAL files grow without bound.
  The `WalOnlyCompactor` reclaims space after snapshots make segments redundant.

The versioning API operates on in-memory `VersionChain`s in `ShardedStore`.
These modules handle the complementary problem: persisting those version chains
to disk and managing the on-disk lifecycle.

---

## Current State

These modules are fully implemented and tested (205 tests, 0 failures) but are
**not wired into the engine or executor layers**. The active persistence path
uses the durability crate's simpler WAL and snapshot system.

The durability crate's system works for the current milestone but lacks:
- CRC32 integrity checking per record
- Segmented WAL with compaction
- Codec abstraction for encryption/compression
- Retention policies for version lifecycle
- Crash-safe snapshot write pattern (fsync + atomic rename)
- Database UUID tracking
- Watermark-based WAL segment reclamation
