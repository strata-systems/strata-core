//! API-level error types for wire encoding
//!
//! This module defines the `ApiError` enum for public API surfaces (Facade, Substrate)
//! and the `WireError` struct for JSON wire encoding.
//!
//! ## Wire Format
//!
//! All errors encode to JSON as:
//! ```json
//! {
//!   "code": "NotFound",
//!   "message": "Key not found: mykey",
//!   "details": {"key": "mykey"}
//! }
//! ```
//!
//! ## Error Codes (Canonical)
//!
//! These codes are frozen and must not change:
//!
//! | Code | Description |
//! |------|-------------|
//! | NotFound | Entity or key not found |
//! | WrongType | Wrong primitive or value type |
//! | InvalidKey | Key syntax invalid |
//! | InvalidPath | JSON path invalid |
//! | HistoryTrimmed | Requested version is unavailable |
//! | ConstraintViolation | API-level invariant violation |
//! | Conflict | Version mismatch or concurrent modification |
//! | Overflow | Numeric overflow |
//! | RunNotFound | Run does not exist |
//! | RunClosed | Run is closed |
//! | RunExists | Run already exists |
//! | Internal | Bug or invariant violation |

use crate::contract::Version;
use crate::Value;
use std::collections::HashMap;

/// Wire error representation for JSON encoding
///
/// This is the canonical wire format for all errors:
/// ```json
/// {
///   "code": "NotFound",
///   "message": "Key not found: mykey",
///   "details": {"key": "mykey"}
/// }
/// ```
#[derive(Debug, Clone)]
pub struct WireError {
    /// The canonical error code (e.g., "NotFound", "WrongType")
    pub code: String,
    /// Human-readable error message
    pub message: String,
    /// Optional structured details as a Value::Object
    pub details: Option<Value>,
}

impl WireError {
    /// Create a new wire error
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    /// Create a wire error with details
    pub fn with_details(
        code: impl Into<String>,
        message: impl Into<String>,
        details: Value,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: Some(details),
        }
    }
}

/// API-level error type for Facade and Substrate APIs
///
/// This enum represents all error conditions that can occur at the API level.
/// Each variant maps to one of the canonical error codes.
#[derive(Debug, Clone)]
pub enum ApiError {
    /// Key or entity not found
    NotFound {
        /// The key that was not found
        key: String,
    },

    /// Wrong value type
    WrongType {
        /// Expected type
        expected: &'static str,
        /// Actual type found
        actual: &'static str,
    },

    /// Invalid key syntax
    InvalidKey {
        /// The invalid key
        key: String,
        /// Reason the key is invalid
        reason: String,
    },

    /// Invalid JSON path
    InvalidPath {
        /// The invalid path
        path: String,
        /// Reason the path is invalid
        reason: String,
    },

    /// History has been trimmed
    HistoryTrimmed {
        /// The requested version
        requested: Version,
        /// The earliest version still retained
        earliest_retained: Version,
    },

    /// Constraint violation (structural failure)
    ConstraintViolation {
        /// Reason for the violation
        reason: String,
        /// Optional extra details
        details: Option<Value>,
    },

    /// Conflict (temporal failure)
    Conflict {
        /// Expected value
        expected: Value,
        /// Actual value found
        actual: Value,
    },

    /// Numeric overflow
    Overflow,

    /// Run not found
    RunNotFound {
        /// The run ID that was not found
        run_id: String,
    },

    /// Run is closed
    RunClosed {
        /// The run ID that is closed
        run_id: String,
    },

    /// Run already exists
    RunExists {
        /// The run ID that already exists
        run_id: String,
    },

    /// Serialization error
    SerializationError {
        /// Error message
        message: String,
    },

    /// Storage error
    StorageError {
        /// Error message
        message: String,
    },

    /// Internal error (bug or invariant violation)
    Internal {
        /// Error message
        message: String,
    },
}

impl ApiError {
    /// Get the canonical error code
    pub fn error_code(&self) -> &'static str {
        match self {
            ApiError::NotFound { .. } => "NotFound",
            ApiError::WrongType { .. } => "WrongType",
            ApiError::InvalidKey { .. } => "InvalidKey",
            ApiError::InvalidPath { .. } => "InvalidPath",
            ApiError::HistoryTrimmed { .. } => "HistoryTrimmed",
            ApiError::ConstraintViolation { .. } => "ConstraintViolation",
            ApiError::Conflict { .. } => "Conflict",
            ApiError::Overflow => "Overflow",
            ApiError::RunNotFound { .. } => "RunNotFound",
            ApiError::RunClosed { .. } => "RunClosed",
            ApiError::RunExists { .. } => "RunExists",
            ApiError::SerializationError { .. } => "SerializationError",
            ApiError::StorageError { .. } => "StorageError",
            ApiError::Internal { .. } => "Internal",
        }
    }

    /// Get the error message
    pub fn message(&self) -> String {
        match self {
            ApiError::NotFound { key } => format!("Key not found: {}", key),
            ApiError::WrongType { expected, actual } => {
                format!("Wrong type: expected {}, got {}", expected, actual)
            }
            ApiError::InvalidKey { key, reason } => format!("Invalid key '{}': {}", key, reason),
            ApiError::InvalidPath { path, reason } => format!("Invalid path '{}': {}", path, reason),
            ApiError::HistoryTrimmed {
                requested,
                earliest_retained,
            } => format!(
                "History trimmed: requested {}, earliest retained {}",
                requested, earliest_retained
            ),
            ApiError::ConstraintViolation { reason, .. } => {
                format!("Constraint violation: {}", reason)
            }
            ApiError::Conflict { .. } => "Conflict: version mismatch".to_string(),
            ApiError::Overflow => "Numeric overflow".to_string(),
            ApiError::RunNotFound { run_id } => format!("Run not found: {}", run_id),
            ApiError::RunClosed { run_id } => format!("Run is closed: {}", run_id),
            ApiError::RunExists { run_id } => format!("Run already exists: {}", run_id),
            ApiError::SerializationError { message } => format!("Serialization error: {}", message),
            ApiError::StorageError { message } => format!("Storage error: {}", message),
            ApiError::Internal { message } => format!("Internal error: {}", message),
        }
    }

    /// Convert to wire error format
    pub fn to_wire_error(&self) -> WireError {
        let code = self.error_code().to_string();
        let message = self.message();
        let details = self.details();

        WireError {
            code,
            message,
            details,
        }
    }

    /// Get the structured details for this error
    fn details(&self) -> Option<Value> {
        match self {
            ApiError::NotFound { key } => {
                let mut map = HashMap::new();
                map.insert("key".to_string(), Value::String(key.clone()));
                Some(Value::Object(map))
            }
            ApiError::WrongType { expected, actual } => {
                let mut map = HashMap::new();
                map.insert("expected".to_string(), Value::String(expected.to_string()));
                map.insert("actual".to_string(), Value::String(actual.to_string()));
                Some(Value::Object(map))
            }
            ApiError::InvalidKey { key, reason } => {
                let mut map = HashMap::new();
                map.insert("key".to_string(), Value::String(key.clone()));
                map.insert("reason".to_string(), Value::String(reason.clone()));
                Some(Value::Object(map))
            }
            ApiError::InvalidPath { path, reason } => {
                let mut map = HashMap::new();
                map.insert("path".to_string(), Value::String(path.clone()));
                map.insert("reason".to_string(), Value::String(reason.clone()));
                Some(Value::Object(map))
            }
            ApiError::HistoryTrimmed {
                requested,
                earliest_retained,
            } => {
                let mut map = HashMap::new();
                map.insert(
                    "requested".to_string(),
                    Value::String(requested.to_string()),
                );
                map.insert(
                    "earliest_retained".to_string(),
                    Value::String(earliest_retained.to_string()),
                );
                Some(Value::Object(map))
            }
            ApiError::ConstraintViolation { reason, details } => {
                let mut map = HashMap::new();
                map.insert("reason".to_string(), Value::String(reason.clone()));
                if let Some(d) = details {
                    map.insert("extra".to_string(), d.clone());
                }
                Some(Value::Object(map))
            }
            ApiError::Conflict { expected, actual } => {
                let mut map = HashMap::new();
                map.insert("expected".to_string(), expected.clone());
                map.insert("actual".to_string(), actual.clone());
                Some(Value::Object(map))
            }
            ApiError::Overflow => None,
            ApiError::RunNotFound { run_id } => {
                let mut map = HashMap::new();
                map.insert("run_id".to_string(), Value::String(run_id.clone()));
                Some(Value::Object(map))
            }
            ApiError::RunClosed { run_id } => {
                let mut map = HashMap::new();
                map.insert("run_id".to_string(), Value::String(run_id.clone()));
                Some(Value::Object(map))
            }
            ApiError::RunExists { run_id } => {
                let mut map = HashMap::new();
                map.insert("run_id".to_string(), Value::String(run_id.clone()));
                Some(Value::Object(map))
            }
            ApiError::SerializationError { message } => {
                let mut map = HashMap::new();
                map.insert("message".to_string(), Value::String(message.clone()));
                Some(Value::Object(map))
            }
            ApiError::StorageError { message } => {
                let mut map = HashMap::new();
                map.insert("message".to_string(), Value::String(message.clone()));
                Some(Value::Object(map))
            }
            ApiError::Internal { message } => {
                let mut map = HashMap::new();
                map.insert("message".to_string(), Value::String(message.clone()));
                Some(Value::Object(map))
            }
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for ApiError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_found_error_code() {
        let err = ApiError::NotFound {
            key: "test".to_string(),
        };
        assert_eq!(err.error_code(), "NotFound");
    }

    #[test]
    fn test_wrong_type_error_code() {
        let err = ApiError::WrongType {
            expected: "Int",
            actual: "String",
        };
        assert_eq!(err.error_code(), "WrongType");
    }

    #[test]
    fn test_to_wire_error() {
        let err = ApiError::NotFound {
            key: "mykey".to_string(),
        };
        let wire = err.to_wire_error();

        assert_eq!(wire.code, "NotFound");
        assert!(wire.message.contains("mykey"));
        assert!(wire.details.is_some());
    }

    #[test]
    fn test_wire_error_details() {
        let err = ApiError::WrongType {
            expected: "Int",
            actual: "Float",
        };
        let wire = err.to_wire_error();

        match wire.details {
            Some(Value::Object(map)) => {
                assert!(map.contains_key("expected"));
                assert!(map.contains_key("actual"));
            }
            _ => panic!("Expected Object details"),
        }
    }

    #[test]
    fn test_overflow_no_details() {
        let err = ApiError::Overflow;
        let wire = err.to_wire_error();

        assert_eq!(wire.code, "Overflow");
        assert!(wire.details.is_none());
    }

    #[test]
    fn test_constraint_violation_with_extra_details() {
        let mut extra = HashMap::new();
        extra.insert("limit".to_string(), Value::Int(1000));

        let err = ApiError::ConstraintViolation {
            reason: "too_large".to_string(),
            details: Some(Value::Object(extra)),
        };
        let wire = err.to_wire_error();

        assert_eq!(wire.code, "ConstraintViolation");
        match wire.details {
            Some(Value::Object(map)) => {
                assert!(map.contains_key("reason"));
                assert!(map.contains_key("extra"));
            }
            _ => panic!("Expected Object details"),
        }
    }
}
