//! Unified error types for Strata.
//!
//! This module provides a clean error type that wraps internal errors
//! and presents a consistent interface to users.

use thiserror::Error;

/// All Strata errors.
///
/// This is the canonical error type for all Strata operations.
/// It provides a clean, stable interface that hides internal error details.
#[derive(Debug, Error)]
pub enum Error {
    /// Entity not found (key, document, run, etc.)
    #[error("not found: {0}")]
    NotFound(String),

    /// Wrong type for operation
    #[error("wrong type: expected {expected}, got {actual}")]
    WrongType {
        /// Expected type
        expected: String,
        /// Actual type found
        actual: String,
    },

    /// Invalid key format
    #[error("invalid key: {0}")]
    InvalidKey(String),

    /// Invalid JSON path
    #[error("invalid path: {0}")]
    InvalidPath(String),

    /// Version conflict (CAS failure, write conflict)
    #[error("conflict: {0}")]
    Conflict(String),

    /// Constraint violation (invalid input, limits exceeded)
    #[error("constraint violation: {0}")]
    ConstraintViolation(String),

    /// Run is closed or not found
    #[error("run error: {0}")]
    RunError(String),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Storage error
    #[error("storage error: {0}")]
    Storage(String),

    /// Internal error (bug or invariant violation)
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type for Strata operations.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Check if this error is retryable.
    ///
    /// Retryable errors (conflicts) may succeed on retry with fresh data.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Error::Conflict(_))
    }

    /// Check if this is a not-found error.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Error::NotFound(_))
    }

    /// Check if this is a conflict error.
    pub fn is_conflict(&self) -> bool {
        matches!(self, Error::Conflict(_))
    }

    /// Check if this is a serious/unrecoverable error.
    pub fn is_serious(&self) -> bool {
        matches!(self, Error::Internal(_))
    }
}

// Convert from internal core errors
impl From<strata_core::error::Error> for Error {
    fn from(e: strata_core::error::Error) -> Self {
        use strata_core::error::Error as CoreError;
        match e {
            CoreError::IoError(io_err) => Error::Io(io_err),
            CoreError::SerializationError(msg) => Error::Serialization(msg),
            CoreError::KeyNotFound(key) => Error::NotFound(format!("{:?}", key)),
            CoreError::VersionMismatch { expected, actual } => {
                Error::Conflict(format!("version mismatch: expected {}, got {}", expected, actual))
            }
            CoreError::Corruption(msg) => Error::Storage(format!("corruption: {}", msg)),
            CoreError::InvalidOperation(msg) => Error::ConstraintViolation(msg),
            CoreError::TransactionAborted(run_id) => {
                Error::Conflict(format!("transaction aborted for run {}", run_id))
            }
            CoreError::StorageError(msg) => Error::Storage(msg),
            CoreError::InvalidState(msg) => Error::ConstraintViolation(msg),
            CoreError::TransactionConflict(msg) => Error::Conflict(msg),
            CoreError::TransactionTimeout(msg) => Error::Conflict(format!("timeout: {}", msg)),
            CoreError::IncompleteEntry { offset, have, needed } => {
                Error::Storage(format!(
                    "incomplete entry at {}: need {} bytes, have {}",
                    offset, needed, have
                ))
            }
            CoreError::ValidationError(msg) => Error::ConstraintViolation(msg),
        }
    }
}

// Convert from internal StrataError
impl From<strata_core::error::StrataError> for Error {
    fn from(e: strata_core::error::StrataError) -> Self {
        use strata_core::error::StrataError as SE;
        match e {
            SE::NotFound { entity_ref } => Error::NotFound(entity_ref.to_string()),
            SE::RunNotFound { run_id } => Error::NotFound(format!("run {}", run_id)),
            SE::WrongType { expected, actual } => Error::WrongType { expected, actual },
            SE::Conflict { reason, .. } => Error::Conflict(reason),
            SE::VersionConflict { entity_ref, expected, actual } => {
                Error::Conflict(format!(
                    "version conflict on {}: expected {}, got {}",
                    entity_ref, expected, actual
                ))
            }
            SE::WriteConflict { entity_ref } => {
                Error::Conflict(format!("write conflict on {}", entity_ref))
            }
            SE::TransactionAborted { reason } => Error::Conflict(format!("aborted: {}", reason)),
            SE::TransactionTimeout { duration_ms } => {
                Error::Conflict(format!("transaction timeout after {}ms", duration_ms))
            }
            SE::TransactionNotActive { state } => {
                Error::ConstraintViolation(format!("transaction not active ({})", state))
            }
            SE::InvalidOperation { entity_ref, reason } => {
                Error::ConstraintViolation(format!("{}: {}", entity_ref, reason))
            }
            SE::InvalidInput { message } => Error::ConstraintViolation(message),
            SE::DimensionMismatch { expected, got } => {
                Error::ConstraintViolation(format!(
                    "dimension mismatch: expected {}, got {}",
                    expected, got
                ))
            }
            SE::PathNotFound { entity_ref, path } => {
                Error::InvalidPath(format!("{} in {}", path, entity_ref))
            }
            SE::HistoryTrimmed { entity_ref, requested, earliest_retained } => {
                Error::NotFound(format!(
                    "version {} of {} (earliest: {})",
                    requested, entity_ref, earliest_retained
                ))
            }
            SE::Storage { message, .. } => Error::Storage(message),
            SE::Serialization { message } => Error::Serialization(message),
            SE::Corruption { message } => Error::Storage(format!("corruption: {}", message)),
            SE::CapacityExceeded { resource, limit, requested } => {
                Error::ConstraintViolation(format!(
                    "{} capacity exceeded: {} > {}",
                    resource, requested, limit
                ))
            }
            SE::BudgetExceeded { operation } => {
                Error::ConstraintViolation(format!("budget exceeded: {}", operation))
            }
            SE::Internal { message } => Error::Internal(message),
        }
    }
}

// Convert from serde_json errors
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Serialization(e.to_string())
    }
}
