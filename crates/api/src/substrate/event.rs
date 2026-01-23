//! EventLog Substrate Operations
//!
//! The EventLog provides append-only event streams for logging and messaging.
//! Events are immutable once appended and use sequence-based versioning.
//!
//! ## Stream Model
//!
//! - Events are organized into named streams
//! - Each stream has independent sequence numbers
//! - Events are immutable (append-only, no updates or deletes)
//!
//! ## Versioning
//!
//! Events use sequence-based versioning (`Version::Sequence`).
//! Each event gets a unique, monotonically increasing sequence number within its stream.
//!
//! ## Payload
//!
//! Event payloads must be `Value::Object`. Empty objects `{}` are allowed.
//! Bytes values are allowed within the payload (encoded via `$bytes` wrapper on wire).

use super::types::ApiRunId;
use strata_core::{StrataResult, Value, Version, Versioned};

/// EventLog substrate operations
///
/// This trait defines the canonical event log operations.
/// All operations require explicit run_id and return versioned results.
///
/// ## Contract
///
/// - Events are append-only (no updates, no deletes)
/// - Payloads must be `Value::Object`
/// - Sequence numbers are unique and monotonically increasing within a stream
///
/// ## Error Handling
///
/// | Condition | Error |
/// |-----------|-------|
/// | Invalid stream name | `InvalidKey` |
/// | Payload not Object | `ConstraintViolation` |
/// | Run not found | `NotFound` |
/// | Run is closed | `ConstraintViolation` |
pub trait EventLog {
    /// Append an event to a stream
    ///
    /// Appends a new event and returns its sequence number.
    ///
    /// ## Semantics
    ///
    /// - Creates stream if it doesn't exist
    /// - Assigns next sequence number in the stream
    /// - Event is immutable once appended
    ///
    /// ## Return Value
    ///
    /// Returns `Version::Sequence(n)` where `n` is the event's sequence number.
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Stream name is invalid
    /// - `ConstraintViolation`: Payload is not Object, or run is closed
    /// - `NotFound`: Run does not exist
    fn event_append(
        &self,
        run: &ApiRunId,
        stream: &str,
        payload: Value,
    ) -> StrataResult<Version>;

    /// Read events from a stream
    ///
    /// Returns events within the specified range, in sequence order.
    ///
    /// ## Parameters
    ///
    /// - `start`: Start sequence (inclusive), `None` = from beginning
    /// - `end`: End sequence (inclusive), `None` = to end
    /// - `limit`: Maximum events to return, `None` = no limit
    ///
    /// ## Return Value
    ///
    /// Vector of `Versioned<Value>` in ascending sequence order (oldest first).
    ///
    /// ## Pagination
    ///
    /// Use `start` and `limit` for pagination:
    /// 1. First page: `range(run, stream, None, None, Some(100))`
    /// 2. Next page: `range(run, stream, Some(last_seq + 1), None, Some(100))`
    ///
    /// ## Performance Note
    ///
    /// Without bounds, this can be expensive for large streams.
    /// Always use `limit` in production.
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Stream name is invalid
    /// - `NotFound`: Run does not exist
    fn event_range(
        &self,
        run: &ApiRunId,
        stream: &str,
        start: Option<u64>,
        end: Option<u64>,
        limit: Option<u64>,
    ) -> StrataResult<Vec<Versioned<Value>>>;

    /// Get a specific event by sequence number
    ///
    /// Returns the event at the specified sequence number.
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Stream name is invalid
    /// - `NotFound`: Run or event does not exist
    /// - `HistoryTrimmed`: Event has been garbage collected
    fn event_get(
        &self,
        run: &ApiRunId,
        stream: &str,
        sequence: u64,
    ) -> StrataResult<Option<Versioned<Value>>>;

    /// Get the count of events in a stream
    ///
    /// Returns the total number of events in the stream.
    ///
    /// ## Return Value
    ///
    /// - `0` if stream doesn't exist or is empty
    /// - Count of events otherwise
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Stream name is invalid
    /// - `NotFound`: Run does not exist
    fn event_len(&self, run: &ApiRunId, stream: &str) -> StrataResult<u64>;

    /// Get the latest sequence number in a stream
    ///
    /// Returns the highest sequence number in the stream, or `None` if empty.
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Stream name is invalid
    /// - `NotFound`: Run does not exist
    fn event_latest_sequence(&self, run: &ApiRunId, stream: &str) -> StrataResult<Option<u64>>;

    /// Read events from a stream in reverse order (newest first)
    ///
    /// Returns events within the specified range, in reverse sequence order.
    ///
    /// ## Parameters
    ///
    /// - `start`: Start sequence (inclusive), `None` = from end
    /// - `end`: End sequence (inclusive), `None` = to beginning
    /// - `limit`: Maximum events to return, `None` = no limit
    ///
    /// ## Return Value
    ///
    /// Vector of `Versioned<Value>` in descending sequence order (newest first).
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Stream name is invalid
    /// - `NotFound`: Run does not exist
    fn event_rev_range(
        &self,
        run: &ApiRunId,
        stream: &str,
        start: Option<u64>,
        end: Option<u64>,
        limit: Option<u64>,
    ) -> StrataResult<Vec<Versioned<Value>>>;

    /// List all streams (event types) in a run
    ///
    /// Returns all distinct stream names that have events.
    ///
    /// ## Errors
    ///
    /// - `NotFound`: Run does not exist
    fn event_streams(&self, run: &ApiRunId) -> StrataResult<Vec<String>>;

    /// Get the latest event (head) of a stream
    ///
    /// Returns the most recent event in the stream, or `None` if empty.
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Stream name is invalid
    /// - `NotFound`: Run does not exist
    fn event_head(&self, run: &ApiRunId, stream: &str) -> StrataResult<Option<Versioned<Value>>>;

    /// Verify the hash chain integrity of the event log
    ///
    /// Validates that all events exist and the hash chain is unbroken.
    ///
    /// ## Return Value
    ///
    /// `ChainVerification` containing:
    /// - `is_valid`: Whether the chain is valid
    /// - `length`: Total number of events
    /// - `first_invalid`: Sequence of first invalid event (if any)
    /// - `error`: Description of the error (if any)
    ///
    /// ## Errors
    ///
    /// - `NotFound`: Run does not exist
    fn event_verify_chain(&self, run: &ApiRunId) -> StrataResult<ChainVerification>;
}

/// Chain verification result
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChainVerification {
    /// Whether the chain is valid
    pub is_valid: bool,
    /// Total number of events in the chain
    pub length: u64,
    /// Sequence of first invalid event (if any)
    pub first_invalid: Option<u64>,
    /// Description of the error (if any)
    pub error: Option<String>,
}

// =============================================================================
// Implementation
// =============================================================================
//
// Note: The primitive EventLog is per-run (not per-stream). We map
// `stream` parameter to `event_type` in the primitive. Sequence numbers
// are global per-run, not per-stream.

use super::impl_::{SubstrateImpl, convert_error, validate_stream_name, validate_event_payload};

impl EventLog for SubstrateImpl {
    fn event_append(
        &self,
        run: &ApiRunId,
        stream: &str,
        payload: Value,
    ) -> StrataResult<Version> {
        validate_stream_name(stream)?;
        validate_event_payload(&payload)?;
        let run_id = run.to_run_id();
        // Use stream as event_type
        self.event().append(&run_id, stream, payload).map_err(convert_error)
    }

    fn event_range(
        &self,
        run: &ApiRunId,
        stream: &str,
        start: Option<u64>,
        end: Option<u64>,
        limit: Option<u64>,
    ) -> StrataResult<Vec<Versioned<Value>>> {
        validate_stream_name(stream)?;
        let run_id = run.to_run_id();

        // Read events filtered by type (stream)
        let events = self.event().read_by_type(&run_id, stream).map_err(convert_error)?;

        // Apply start/end range and limit
        let filtered: Vec<_> = events
            .into_iter()
            .filter(|e| {
                let seq = match e.version {
                    Version::Sequence(s) => s,
                    _ => return false,
                };
                start.map_or(true, |s| seq >= s) && end.map_or(true, |e| seq <= e)
            })
            .take(limit.unwrap_or(u64::MAX) as usize)
            .map(|e| Versioned {
                value: e.value.payload.clone(),
                version: e.version,
                timestamp: strata_core::Timestamp::from_millis(e.value.timestamp as u64),
            })
            .collect();

        Ok(filtered)
    }

    fn event_get(
        &self,
        run: &ApiRunId,
        stream: &str,
        sequence: u64,
    ) -> StrataResult<Option<Versioned<Value>>> {
        validate_stream_name(stream)?;
        let run_id = run.to_run_id();

        // Read the event at this sequence
        let event = self.event().read(&run_id, sequence).map_err(convert_error)?;

        // Check if it matches the requested stream (event_type)
        match event {
            Some(e) if e.value.event_type == stream => {
                Ok(Some(Versioned {
                    value: e.value.payload,
                    version: e.version,
                    timestamp: strata_core::Timestamp::from_millis(e.value.timestamp as u64),
                }))
            }
            _ => Ok(None),
        }
    }

    fn event_len(&self, run: &ApiRunId, stream: &str) -> StrataResult<u64> {
        validate_stream_name(stream)?;
        let run_id = run.to_run_id();
        // Count events with this type
        let events = self.event().read_by_type(&run_id, stream).map_err(convert_error)?;
        Ok(events.len() as u64)
    }

    fn event_latest_sequence(&self, run: &ApiRunId, stream: &str) -> StrataResult<Option<u64>> {
        validate_stream_name(stream)?;
        let run_id = run.to_run_id();
        // Find highest sequence with this type
        let events = self.event().read_by_type(&run_id, stream).map_err(convert_error)?;
        let max_seq = events.iter().filter_map(|e| {
            match e.version {
                Version::Sequence(s) => Some(s),
                _ => None,
            }
        }).max();
        Ok(max_seq)
    }

    fn event_rev_range(
        &self,
        run: &ApiRunId,
        stream: &str,
        start: Option<u64>,
        end: Option<u64>,
        limit: Option<u64>,
    ) -> StrataResult<Vec<Versioned<Value>>> {
        validate_stream_name(stream)?;
        let run_id = run.to_run_id();

        // Read events filtered by type (stream)
        let events = self.event().read_by_type(&run_id, stream).map_err(convert_error)?;

        // Apply start/end range, reverse, and limit
        let mut filtered: Vec<_> = events
            .into_iter()
            .filter(|e| {
                let seq = match e.version {
                    Version::Sequence(s) => s,
                    _ => return false,
                };
                start.map_or(true, |s| seq <= s) && end.map_or(true, |e| seq >= e)
            })
            .map(|e| Versioned {
                value: e.value.payload.clone(),
                version: e.version,
                timestamp: strata_core::Timestamp::from_millis(e.value.timestamp as u64),
            })
            .collect();

        // Reverse to get newest first
        filtered.reverse();

        // Apply limit
        if let Some(n) = limit {
            filtered.truncate(n as usize);
        }

        Ok(filtered)
    }

    fn event_streams(&self, run: &ApiRunId) -> StrataResult<Vec<String>> {
        let run_id = run.to_run_id();
        self.event().event_types(&run_id).map_err(convert_error)
    }

    fn event_head(&self, run: &ApiRunId, stream: &str) -> StrataResult<Option<Versioned<Value>>> {
        validate_stream_name(stream)?;
        let run_id = run.to_run_id();

        // Read events filtered by type (stream) and get the last one
        let events = self.event().read_by_type(&run_id, stream).map_err(convert_error)?;

        Ok(events.into_iter().last().map(|e| Versioned {
            value: e.value.payload.clone(),
            version: e.version,
            timestamp: strata_core::Timestamp::from_millis(e.value.timestamp as u64),
        }))
    }

    fn event_verify_chain(&self, run: &ApiRunId) -> StrataResult<ChainVerification> {
        let run_id = run.to_run_id();
        let verification = self.event().verify_chain(&run_id).map_err(convert_error)?;
        Ok(ChainVerification {
            is_valid: verification.is_valid,
            length: verification.length,
            first_invalid: verification.first_invalid,
            error: verification.error,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trait_is_object_safe() {
        fn _assert_object_safe(_: &dyn EventLog) {}
    }
}
