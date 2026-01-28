//! Search types — re-exported from strata_engine::search_types
//!
//! These types have been moved to the engine crate where they belong
//! as derived operations. This module re-exports them for backward compatibility.

// Re-export contract types that were previously re-exported here
pub use crate::contract::EntityRef;
pub use crate::contract::PrimitiveType;

// Note: SearchBudget, SearchMode, SearchRequest, SearchHit, SearchStats, SearchResponse
// are now defined in strata_engine::search_types. Downstream code should import from
// strata_engine instead.
//
// The core crate re-exports at lib.rs level are removed. Code that was using
// strata_core::SearchRequest should now use strata_engine::SearchRequest.
