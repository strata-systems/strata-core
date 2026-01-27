//! Storage layer for Strata
//!
//! This crate provides in-memory storage:
//! - ShardedStore: DashMap + HashMap with MVCC version chains
//! - Lock-free reads via DashMap
//! - Per-RunId sharding (no cross-run contention)
//! - FxHashMap for O(1) lookups
//!
//! # Note on Persistence
//!
//! Persistence and durability are handled by the `strata-durability` crate.
//! This crate focuses solely on in-memory data structures.

#![warn(missing_docs)]
#![warn(clippy::all)]

// In-memory storage
pub mod index;
pub mod primitive_ext;
pub mod registry;
pub mod sharded;
pub mod stored_value;
pub mod ttl;

// Disk storage (formats and utilities - persistence handled by strata-durability)
pub mod codec;
pub mod compaction;
pub mod disk_snapshot;
pub mod format;
pub mod retention;
pub mod testing;

// In-memory storage re-exports
pub use index::{RunIndex, TypeIndex};
pub use primitive_ext::{
    is_future_wal_type, is_vector_wal_type, primitive_for_wal_type, primitive_type_ids, wal_ranges,
    PrimitiveExtError, PrimitiveStorageExt,
};
pub use registry::PrimitiveRegistry;
pub use sharded::{Shard, ShardedSnapshot, ShardedStore};
pub use ttl::TTLIndex;

// Disk storage re-exports
pub use codec::{get_codec, CodecError, IdentityCodec, StorageCodec};
pub use disk_snapshot::{
    CheckpointCoordinator, CheckpointData, CheckpointError, LoadedSection, LoadedSnapshot,
    SnapshotInfo, SnapshotReadError, SnapshotReader, SnapshotSection, SnapshotWriter,
};
pub use format::{
    // Snapshot format
    find_latest_snapshot,
    list_snapshots,
    parse_snapshot_id,
    primitive_tags,
    snapshot_path,
    // Watermark tracking
    CheckpointInfo,
    // Primitive serialization
    EventSnapshotEntry,
    JsonSnapshotEntry,
    KvSnapshotEntry,
    // MANIFEST format
    Manifest,
    ManifestError,
    ManifestManager,
    // WAL format
    Mutation,
    PrimitiveSerializeError,
    RunSnapshotEntry,
    SectionHeader,
    SegmentHeader,
    SnapshotHeader,
    SnapshotHeaderError,
    SnapshotSerializer,
    SnapshotWatermark,
    StateSnapshotEntry,
    VectorCollectionSnapshotEntry,
    VectorSnapshotEntry,
    WalRecord,
    WalRecordError,
    WalSegment,
    WatermarkError,
    Writeset,
    WritesetError,
    MANIFEST_FORMAT_VERSION,
    MANIFEST_MAGIC,
    SEGMENT_FORMAT_VERSION,
    SEGMENT_HEADER_SIZE,
    SEGMENT_MAGIC,
    SNAPSHOT_FORMAT_VERSION,
    SNAPSHOT_HEADER_SIZE,
    SNAPSHOT_MAGIC,
    WAL_RECORD_FORMAT_VERSION,
};
pub use retention::{CompositeBuilder, RetentionPolicy, RetentionPolicyError};
pub use compaction::{
    CompactInfo, CompactMode, CompactionError, Tombstone, TombstoneError, TombstoneIndex,
    TombstoneReason, WalOnlyCompactor,
};
pub use testing::{
    CrashConfig, CrashPoint, CrashTestError, CrashTestResult, CrashType, DataState, Operation,
    ReferenceModel, StateMismatch, VerificationResult,
};
