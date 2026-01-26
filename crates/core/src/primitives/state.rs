//! State types for the StateCell primitive
//!
//! These types define the structure of versioned state cells.

use crate::contract::Version;
use crate::value::Value;
use serde::{Deserialize, Serialize};

/// Current state of a cell
///
/// Each state cell has:
/// - A value (arbitrary data)
/// - A version (Counter-based, monotonically increasing)
/// - A timestamp of last update
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct State {
    /// Current value
    pub value: Value,
    /// Version (Counter-based for CAS operations)
    pub version: Version,
    /// Last update timestamp (microseconds since epoch)
    pub updated_at: u64,
}

impl State {
    /// Create a new state with version 1
    pub fn new(value: Value) -> Self {
        Self {
            value,
            version: Version::counter(1),
            updated_at: Self::now(),
        }
    }

    /// Create a new state with explicit version
    pub fn with_version(value: Value, version: Version) -> Self {
        Self {
            value,
            version,
            updated_at: Self::now(),
        }
    }

    /// Get current timestamp in microseconds
    pub fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64
    }
}
