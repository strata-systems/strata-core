//! Public types for the Strata unified API.
//!
//! This module re-exports types from internal crates with a clean public interface.

// Core value types
pub use strata_core::Value;
pub use strata_core::JsonValue;

// Version and versioned wrapper
pub use strata_core::Version;
pub use strata_core::Versioned;
pub use strata_core::Timestamp;

// Run types
pub use strata_core::RunId;
pub use strata_core::RunName;
pub use strata_core::RunStatus;

// JSON types
pub use strata_core::JsonPath;
pub use strata_core::JsonPatch;

// Vector types
pub use strata_core::DistanceMetric;

// Re-export VectorMatch from substrate (it's defined there, not in core)
pub use strata_api::substrate::VectorMatch;

// API types from strata-api substrate
pub use strata_api::substrate::{
    ApiRunId, RunInfo, RunState, RetentionPolicy,
    SearchFilter, VectorData,
};

// Re-export durability mode from engine
pub use strata_engine::DurabilityMode;

/// Convert a RunId to ApiRunId.
///
/// This is a helper for converting between internal and API types.
pub fn run_id_to_api(run_id: &RunId) -> ApiRunId {
    let uuid = uuid::Uuid::from_bytes(*run_id.as_bytes());
    ApiRunId::from_uuid(uuid)
}
