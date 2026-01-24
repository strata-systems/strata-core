//! EventLog Substrate Operations
//!
//! The EventLog provides append-only event streams for logging and messaging.
//! Events are immutable once appended and use sequence-based versioning.
//!
//! ## Role
//!
//! EventLog is the **determinism boundary recorder**. It captures all non-deterministic
//! inputs (API calls, timestamps, randomness, external state) at the point of entry,
//! enabling replay-based recovery and debugging.
//!
//! ## Stream Model
//!
//! - Events are organized into named streams (event types)
//! - **Sequences are GLOBAL** - all streams share a single sequence counter
//! - Streams are FILTERS over the global sequence, not partitions
//! - Events are immutable (append-only, no updates or deletes)
//!
//! ## Versioning
//!
//! Events use sequence-based versioning (`Version::Sequence`).
//! Each event gets a unique, monotonically increasing sequence number globally.
//!
//! ## Payload
//!
//! Event payloads must be `Value::Object`. Empty objects `{}` are allowed.
//! Bytes values are allowed within the payload (encoded via `$bytes` wrapper on wire).
//! NaN and Infinity float values are rejected for JSON serialization safety.

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
/// - Payloads must be `Value::Object` (no primitives, no arrays at top level)
/// - Payloads cannot contain NaN or Infinity float values
/// - Sequence numbers are GLOBAL (shared across all streams in a run)
/// - Streams are filters, not partitions - they filter the global sequence
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
    /// - Assigns next GLOBAL sequence number (shared across all streams)
    /// - Event is immutable once appended
    /// - Hash chain is extended with SHA-256 for deterministic verification
    ///
    /// ## Return Value
    ///
    /// Returns `Version::Sequence(n)` where `n` is the event's global sequence number.
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
    /// ## Performance
    ///
    /// O(1) - uses cached stream metadata.
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
    /// ## Performance
    ///
    /// O(1) - uses cached stream metadata.
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Stream name is invalid
    /// - `NotFound`: Run does not exist
    fn event_latest_sequence(&self, run: &ApiRunId, stream: &str) -> StrataResult<Option<u64>>;

    /// Get stream metadata
    ///
    /// Returns detailed metadata about a stream including count, sequence bounds,
    /// and timestamp bounds. All data is O(1) access.
    ///
    /// ## Return Value
    ///
    /// `StreamInfo` containing:
    /// - `count`: Number of events in the stream
    /// - `first_sequence`/`last_sequence`: Sequence bounds
    /// - `first_timestamp`/`last_timestamp`: Time bounds
    ///
    /// Returns a StreamInfo with count=0 if the stream doesn't exist.
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Stream name is invalid
    /// - `NotFound`: Run does not exist
    fn event_stream_info(&self, run: &ApiRunId, stream: &str) -> StrataResult<StreamInfo>;

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
    /// ## Performance
    ///
    /// O(1) - uses cached stream metadata to find the last sequence.
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

/// Stream metadata (O(1) access)
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StreamInfo {
    /// Number of events in this stream
    pub count: u64,
    /// First (oldest) sequence number in this stream
    pub first_sequence: Option<u64>,
    /// Last (newest) sequence number in this stream
    pub last_sequence: Option<u64>,
    /// Timestamp of first event (microseconds since epoch)
    pub first_timestamp: Option<i64>,
    /// Timestamp of last event (microseconds since epoch)
    pub last_timestamp: Option<i64>,
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
        // O(1) using stream metadata
        self.event().len_by_type(&run_id, stream).map_err(convert_error)
    }

    fn event_latest_sequence(&self, run: &ApiRunId, stream: &str) -> StrataResult<Option<u64>> {
        validate_stream_name(stream)?;
        let run_id = run.to_run_id();
        // O(1) using stream metadata
        self.event().latest_sequence_by_type(&run_id, stream).map_err(convert_error)
    }

    fn event_stream_info(&self, run: &ApiRunId, stream: &str) -> StrataResult<StreamInfo> {
        validate_stream_name(stream)?;
        let run_id = run.to_run_id();
        // O(1) using stream metadata
        match self.event().stream_info(&run_id, stream).map_err(convert_error)? {
            Some(meta) => Ok(StreamInfo {
                count: meta.count,
                first_sequence: Some(meta.first_sequence),
                last_sequence: Some(meta.last_sequence),
                first_timestamp: Some(meta.first_timestamp),
                last_timestamp: Some(meta.last_timestamp),
            }),
            None => Ok(StreamInfo {
                count: 0,
                first_sequence: None,
                last_sequence: None,
                first_timestamp: None,
                last_timestamp: None,
            }),
        }
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
        // O(1) using stream metadata keys
        self.event().stream_names(&run_id).map_err(convert_error)
    }

    fn event_head(&self, run: &ApiRunId, stream: &str) -> StrataResult<Option<Versioned<Value>>> {
        validate_stream_name(stream)?;
        let run_id = run.to_run_id();
        // O(1) using stream metadata to find last sequence
        match self.event().head_by_type(&run_id, stream).map_err(convert_error)? {
            Some(e) => Ok(Some(Versioned {
                value: e.value.payload.clone(),
                version: e.version,
                timestamp: strata_core::Timestamp::from_millis(e.value.timestamp as u64),
            })),
            None => Ok(None),
        }
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
