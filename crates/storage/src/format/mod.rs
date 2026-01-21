//! On-disk byte formats for WAL, snapshots, and MANIFEST.
//!
//! This module centralizes all serialization logic for persistent storage.
//! Keeping serialization separate from operational logic (how WAL/snapshots
//! are managed) makes format evolution easier to manage.
//!
//! # Module Structure
//!
//! - `wal_record`: WAL segment header and record format
//! - `writeset`: Transaction writeset serialization
//! - `manifest`: MANIFEST file format (added in Epic 72)
//! - `snapshot`: Snapshot file format (added in Epic 71)

pub mod wal_record;
pub mod writeset;

pub use wal_record::{
    SegmentHeader, WalRecord, WalRecordError, WalSegment, SEGMENT_FORMAT_VERSION,
    SEGMENT_HEADER_SIZE, SEGMENT_MAGIC, WAL_RECORD_FORMAT_VERSION,
};
pub use writeset::{Mutation, Writeset, WritesetError};
