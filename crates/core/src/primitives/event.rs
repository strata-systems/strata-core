//! Event types for the EventLog primitive
//!
//! These types define the structure of events in the append-only event log.

use crate::value::Value;
use serde::{Deserialize, Serialize};

/// An event in the log
///
/// Events are immutable records in an append-only log. Each event includes:
/// - A monotonically increasing sequence number
/// - A user-defined event type for categorization
/// - An arbitrary payload
/// - Timestamp and hash chain for integrity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    /// Sequence number (auto-assigned, monotonic per run)
    pub sequence: u64,
    /// Event type (user-defined category)
    pub event_type: String,
    /// Event payload (arbitrary data)
    pub payload: Value,
    /// Timestamp when event was appended (milliseconds since epoch)
    pub timestamp: i64,
    /// Hash of previous event (for chaining)
    pub prev_hash: [u8; 32],
    /// Hash of this event
    pub hash: [u8; 32],
}

/// Chain verification result
///
/// Returned by `verify_chain()` to report the integrity status of an event chain.
#[derive(Debug, Clone)]
pub struct ChainVerification {
    /// Whether the chain is valid
    pub is_valid: bool,
    /// Total length of the chain
    pub length: u64,
    /// First invalid sequence number (if any)
    pub first_invalid: Option<u64>,
    /// Error description (if any)
    pub error: Option<String>,
}

impl ChainVerification {
    /// Create a valid verification result
    pub fn valid(length: u64) -> Self {
        Self {
            is_valid: true,
            length,
            first_invalid: None,
            error: None,
        }
    }

    /// Create an invalid verification result
    pub fn invalid(length: u64, first_invalid: u64, error: impl Into<String>) -> Self {
        Self {
            is_valid: false,
            length,
            first_invalid: Some(first_invalid),
            error: Some(error.into()),
        }
    }
}
