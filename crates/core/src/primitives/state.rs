//! State types for the StateCell primitive
//!
//! These types define the structure of versioned state cells.

use crate::value::Value;
use serde::{Deserialize, Serialize};

/// Current state of a cell
///
/// Each state cell has:
/// - A value (arbitrary data)
/// - A version number (monotonically increasing)
/// - A timestamp of last update
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct State {
    /// Current value
    pub value: Value,
    /// Version number (monotonically increasing)
    pub version: u64,
    /// Last update timestamp (milliseconds since epoch)
    pub updated_at: i64,
}

impl State {
    /// Create a new state with version 1
    pub fn new(value: Value) -> Self {
        Self {
            value,
            version: 1,
            updated_at: Self::now(),
        }
    }

    /// Create a new state with explicit version
    pub fn with_version(value: Value, version: u64) -> Self {
        Self {
            value,
            version,
            updated_at: Self::now(),
        }
    }

    /// Get current timestamp in milliseconds
    pub fn now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }
}
