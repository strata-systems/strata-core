//! Storage layer for in-mem
//!
//! This crate implements the unified storage backend with:
//! - UnifiedStore: BTreeMap-based storage with RwLock
//! - Secondary indices (run_index, type_index)
//! - TTL index for expiration
//! - Version management with AtomicU64
//! - ClonedSnapshotView implementation

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod index;
pub mod unified;
// pub mod ttl;        // Story #14
// pub mod snapshot;   // Story #15

pub use index::{RunIndex, TypeIndex};
pub use unified::UnifiedStore;
