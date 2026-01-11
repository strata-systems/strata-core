//! Storage layer for in-mem
//!
//! This crate implements the unified storage backend with:
//! - UnifiedStore: BTreeMap-based storage with RwLock
//! - Secondary indices (run_index, type_index)
//! - TTL index for expiration
//! - TTL cleaner background task
//! - Version management with AtomicU64
//! - ClonedSnapshotView implementation

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod cleaner;
pub mod index;
pub mod snapshot;
pub mod ttl;
pub mod unified;

pub use cleaner::TTLCleaner;
pub use index::{RunIndex, TypeIndex};
pub use snapshot::ClonedSnapshotView;
pub use ttl::TTLIndex;
pub use unified::UnifiedStore;
