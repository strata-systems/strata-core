//! Convenient imports for Strata.
//!
//! This module re-exports the most commonly used types so you can get started
//! with a single import:
//!
//! ```ignore
//! use strata::prelude::*;
//!
//! let db = Strata::open("./my-db")?;
//! db.kv.set("key", "value")?;
//! ```

// Main entry point
pub use crate::database::{Strata, StrataBuilder};

// Error handling
pub use crate::error::{Error, Result};

// Primitives
pub use crate::primitives::{Events, Json, Runs, State, Vectors, KV};

// Core types
pub use crate::types::{RunId, Value, Version, Versioned, Timestamp};

// Vector types
pub use crate::types::DistanceMetric;

// Run types
pub use crate::types::{RunState, RetentionPolicy};

// Re-export serde_json for convenience
pub use serde_json::json;
