//! Durability layer for in-mem
//!
//! This crate implements write-ahead logging and snapshots:
//! - WAL: Append-only write-ahead log
//! - WALEntry types: BeginTxn, Write, Delete, CommitTxn, etc.
//! - Entry encoding/decoding with CRC32 checksums
//! - Durability modes: Strict, Batched (default), Async
//! - Snapshot creation and loading
//! - Recovery: Replay WAL from last snapshot

#![warn(missing_docs)]
#![warn(clippy::all)]

// Module declarations
pub mod encoding; // Story #18: Entry encoding/decoding with CRC
pub mod wal; // Story #17: WALEntry types, Story #19: File operations

// Stubs for future stories
// pub mod snapshot;   // M4
// pub mod recovery;   // Story #23-25

// Re-export commonly used types
pub use encoding::{decode_entry, encode_entry};
pub use wal::{WALEntry, WAL};
