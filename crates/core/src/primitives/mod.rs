//! Primitive types for in-mem
//!
//! This module defines the canonical data structures for all primitives.
//! These types are shared between the `engine` and `primitives` crates.
//!
//! ## Design Principle
//!
//! - **in-mem-core** defines canonical semantic types (this module)
//! - **in-mem-primitives** provides stateless facades and implementation logic
//! - **in-mem-engine** orchestrates transactions and recovery
//!
//! All crates share the same type definitions from core.

pub mod event;
pub mod state;
pub mod trace;
pub mod vector;

// Re-export all types at module level
pub use event::{ChainVerification, Event};
pub use state::State;
pub use trace::{Trace, TraceTree, TraceType};
pub use vector::{
    CollectionId, CollectionInfo, DistanceMetric, JsonScalar, MetadataFilter, StorageDtype,
    VectorConfig, VectorEntry, VectorId, VectorMatch,
};
