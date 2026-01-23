# Epic 81: Error Model Standardization

**Goal**: Freeze all error codes and structured payloads

**Dependencies**: Epic 80

---

## Scope

- StrataError enum with all 11 error codes
- Wire error shape (code, message, details)
- ConstraintViolation reason codes
- Complete error-producing conditions
- Overflow error for numeric operations
- Error documentation

---

## User Stories

| Story | Description | Priority |
|-------|-------------|----------|
| #560 | StrataError Enum with All Codes | FOUNDATION |
| #561 | Error Wire Shape (code, message, details) | CRITICAL |
| #562 | ConstraintViolation Reason Codes | CRITICAL |
| #563 | Error-Producing Conditions Coverage | HIGH |
| #564 | Overflow Error for Numeric Operations | HIGH |
| #565 | Error Documentation | HIGH |

---

## Story #560: StrataError Enum with All Codes

**File**: `crates/core/src/error/codes.rs` (NEW)

**Deliverable**: Canonical error enum with all 11 error codes

### Design

All errors flow through this single enum. Each code has a specific meaning and use case.

### Implementation

```rust
use std::collections::HashMap;
use crate::contract::Version;

/// Canonical Strata error type
///
/// All errors in the system are represented by this enum.
/// Error codes are FROZEN and cannot change without major version bump.
#[derive(Debug, Clone)]
pub enum StrataError {
    /// Entity or key not found
    NotFound {
        message: String,
        key: Option<String>,
    },

    /// Wrong primitive or value type
    WrongType {
        message: String,
        expected: String,
        actual: String,
    },

    /// Key syntax invalid
    InvalidKey {
        message: String,
        key: String,
        reason: KeyErrorReason,
    },

    /// JSON path invalid
    InvalidPath {
        message: String,
        path: String,
    },

    /// Requested version no longer retained
    HistoryTrimmed {
        message: String,
        requested: Version,
        earliest_retained: Version,
    },

    /// Schema/shape/invariant violation
    ConstraintViolation {
        message: String,
        reason: ConstraintReason,
        details: Option<HashMap<String, serde_json::Value>>,
    },

    /// CAS failure, transaction conflict, version mismatch
    Conflict {
        message: String,
        details: Option<HashMap<String, serde_json::Value>>,
    },

    /// Value encode/decode failure
    SerializationError {
        message: String,
        details: Option<String>,
    },

    /// Disk, WAL, or IO failure
    StorageError {
        message: String,
        source: Option<String>,
    },

    /// Bug or invariant violation
    InternalError {
        message: String,
        details: Option<String>,
    },

    /// Numeric overflow/underflow (for incr)
    Overflow {
        message: String,
        operation: String,
    },
}

/// Key error reasons
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyErrorReason {
    Empty,
    TooLong,
    ContainsNul,
    ReservedPrefix,
    InvalidUtf8,
}

/// Constraint violation reasons
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstraintReason {
    ValueTooLarge,
    NestingTooDeep,
    KeyTooLong,
    VectorDimExceeded,
    VectorDimMismatch,
    RootNotObject,
    ReservedPrefix,
    RunClosed,
}

impl StrataError {
    /// Get the error code as a string
    pub fn code(&self) -> &'static str {
        match self {
            StrataError::NotFound { .. } => "NotFound",
            StrataError::WrongType { .. } => "WrongType",
            StrataError::InvalidKey { .. } => "InvalidKey",
            StrataError::InvalidPath { .. } => "InvalidPath",
            StrataError::HistoryTrimmed { .. } => "HistoryTrimmed",
            StrataError::ConstraintViolation { .. } => "ConstraintViolation",
            StrataError::Conflict { .. } => "Conflict",
            StrataError::SerializationError { .. } => "SerializationError",
            StrataError::StorageError { .. } => "StorageError",
            StrataError::InternalError { .. } => "InternalError",
            StrataError::Overflow { .. } => "Overflow",
        }
    }

    /// Get the human-readable message
    pub fn message(&self) -> &str {
        match self {
            StrataError::NotFound { message, .. } => message,
            StrataError::WrongType { message, .. } => message,
            StrataError::InvalidKey { message, .. } => message,
            StrataError::InvalidPath { message, .. } => message,
            StrataError::HistoryTrimmed { message, .. } => message,
            StrataError::ConstraintViolation { message, .. } => message,
            StrataError::Conflict { message, .. } => message,
            StrataError::SerializationError { message, .. } => message,
            StrataError::StorageError { message, .. } => message,
            StrataError::InternalError { message, .. } => message,
            StrataError::Overflow { message, .. } => message,
        }
    }

    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(self, StrataError::Conflict { .. })
    }

    /// Check if this error indicates a bug
    pub fn is_bug(&self) -> bool {
        matches!(self, StrataError::InternalError { .. })
    }
}

impl std::fmt::Display for StrataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code(), self.message())
    }
}

impl std::error::Error for StrataError {}

impl std::fmt::Display for KeyErrorReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyErrorReason::Empty => write!(f, "empty"),
            KeyErrorReason::TooLong => write!(f, "too_long"),
            KeyErrorReason::ContainsNul => write!(f, "contains_nul"),
            KeyErrorReason::ReservedPrefix => write!(f, "reserved_prefix"),
            KeyErrorReason::InvalidUtf8 => write!(f, "invalid_utf8"),
        }
    }
}

impl std::fmt::Display for ConstraintReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConstraintReason::ValueTooLarge => write!(f, "value_too_large"),
            ConstraintReason::NestingTooDeep => write!(f, "nesting_too_deep"),
            ConstraintReason::KeyTooLong => write!(f, "key_too_long"),
            ConstraintReason::VectorDimExceeded => write!(f, "vector_dim_exceeded"),
            ConstraintReason::VectorDimMismatch => write!(f, "vector_dim_mismatch"),
            ConstraintReason::RootNotObject => write!(f, "root_not_object"),
            ConstraintReason::ReservedPrefix => write!(f, "reserved_prefix"),
            ConstraintReason::RunClosed => write!(f, "run_closed"),
        }
    }
}
```

### Acceptance Criteria

- [ ] All 11 error codes implemented
- [ ] `code()` returns stable string code
- [ ] `message()` returns human-readable message
- [ ] `is_retryable()` identifies Conflict as retryable
- [ ] `is_bug()` identifies InternalError as bug
- [ ] Display implementation for logging

---

## Story #561: Error Wire Shape

**File**: `crates/core/src/error/wire.rs` (NEW)

**Deliverable**: JSON wire shape for errors

### Implementation

```rust
use serde::{Serialize, Deserialize};
use crate::error::StrataError;

/// Wire format for errors
///
/// Shape: {"ok": false, "error": {"code": "...", "message": "...", "details": {...}}}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl From<&StrataError> for WireError {
    fn from(err: &StrataError) -> Self {
        WireError {
            code: err.code().to_string(),
            message: err.message().to_string(),
            details: err.to_details(),
        }
    }
}

impl StrataError {
    /// Convert error to wire details
    pub fn to_details(&self) -> Option<serde_json::Value> {
        match self {
            StrataError::NotFound { key, .. } => {
                key.as_ref().map(|k| serde_json::json!({ "key": k }))
            }
            StrataError::WrongType { expected, actual, .. } => {
                Some(serde_json::json!({
                    "expected": expected,
                    "actual": actual
                }))
            }
            StrataError::InvalidKey { key, reason, .. } => {
                Some(serde_json::json!({
                    "key": key,
                    "reason": reason.to_string()
                }))
            }
            StrataError::InvalidPath { path, .. } => {
                Some(serde_json::json!({ "path": path }))
            }
            StrataError::HistoryTrimmed { requested, earliest_retained, .. } => {
                Some(serde_json::json!({
                    "requested": {
                        "type": requested.type_name(),
                        "value": requested.value()
                    },
                    "earliest_retained": {
                        "type": earliest_retained.type_name(),
                        "value": earliest_retained.value()
                    }
                }))
            }
            StrataError::ConstraintViolation { reason, details, .. } => {
                let mut obj = serde_json::json!({ "reason": reason.to_string() });
                if let Some(d) = details {
                    if let serde_json::Value::Object(ref mut map) = obj {
                        for (k, v) in d {
                            map.insert(k.clone(), v.clone());
                        }
                    }
                }
                Some(obj)
            }
            StrataError::Conflict { details, .. } => details.clone().map(|d| serde_json::json!(d)),
            StrataError::SerializationError { details, .. } => {
                details.as_ref().map(|d| serde_json::json!({ "details": d }))
            }
            StrataError::StorageError { source, .. } => {
                source.as_ref().map(|s| serde_json::json!({ "source": s }))
            }
            StrataError::InternalError { details, .. } => {
                details.as_ref().map(|d| serde_json::json!({ "details": d }))
            }
            StrataError::Overflow { operation, .. } => {
                Some(serde_json::json!({ "operation": operation }))
            }
        }
    }

    /// Convert to wire response envelope
    pub fn to_wire_response(&self) -> serde_json::Value {
        serde_json::json!({
            "ok": false,
            "error": WireError::from(self)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::Version;

    #[test]
    fn test_history_trimmed_wire_shape() {
        let err = StrataError::HistoryTrimmed {
            message: "Requested version no longer retained".into(),
            requested: Version::Txn(100),
            earliest_retained: Version::Txn(150),
        };

        let wire = err.to_wire_response();
        let expected = serde_json::json!({
            "ok": false,
            "error": {
                "code": "HistoryTrimmed",
                "message": "Requested version no longer retained",
                "details": {
                    "requested": { "type": "txn", "value": 100 },
                    "earliest_retained": { "type": "txn", "value": 150 }
                }
            }
        });

        assert_eq!(wire, expected);
    }
}
```

### Acceptance Criteria

- [ ] Wire shape: `{"ok": false, "error": {"code": "...", "message": "...", "details": {...}}}`
- [ ] `WireError` struct with Serialize/Deserialize
- [ ] `to_details()` produces appropriate details for each error type
- [ ] `to_wire_response()` produces complete envelope
- [ ] HistoryTrimmed includes requested and earliest_retained versions

---

## Story #562: ConstraintViolation Reason Codes

**File**: `crates/core/src/error/constraint.rs` (NEW)

**Deliverable**: All ConstraintViolation reason codes

### Implementation

(Included in Story #560)

### Acceptance Criteria

- [ ] `value_too_large` - Value exceeds size limits
- [ ] `nesting_too_deep` - Nesting exceeds max depth
- [ ] `key_too_long` - Key exceeds max length
- [ ] `vector_dim_exceeded` - Vector dimension exceeds max
- [ ] `vector_dim_mismatch` - Vector dimension doesn't match existing
- [ ] `root_not_object` - JSON root must be Object
- [ ] `reserved_prefix` - Key uses `_strata/` prefix
- [ ] `run_closed` - Operating on closed run

---

## Story #563: Error-Producing Conditions Coverage

**File**: `crates/core/src/error/conditions.rs` (NEW)

**Deliverable**: Factory functions for all error conditions

### Implementation

```rust
use crate::error::{StrataError, KeyErrorReason, ConstraintReason};
use crate::contract::Version;

/// Error factory for common conditions
pub struct ErrorFactory;

impl ErrorFactory {
    // Key errors

    pub fn empty_key() -> StrataError {
        StrataError::InvalidKey {
            message: "Key cannot be empty".into(),
            key: String::new(),
            reason: KeyErrorReason::Empty,
        }
    }

    pub fn key_too_long(key: &str, max: usize) -> StrataError {
        StrataError::InvalidKey {
            message: format!("Key exceeds maximum length of {} bytes", max),
            key: key.to_string(),
            reason: KeyErrorReason::TooLong,
        }
    }

    pub fn key_contains_nul(key: &str) -> StrataError {
        StrataError::InvalidKey {
            message: "Key contains NUL byte".into(),
            key: key.to_string(),
            reason: KeyErrorReason::ContainsNul,
        }
    }

    pub fn key_reserved_prefix(key: &str) -> StrataError {
        StrataError::InvalidKey {
            message: "Key uses reserved prefix '_strata/'".into(),
            key: key.to_string(),
            reason: KeyErrorReason::ReservedPrefix,
        }
    }

    pub fn key_invalid_utf8() -> StrataError {
        StrataError::InvalidKey {
            message: "Key is not valid UTF-8".into(),
            key: String::new(),
            reason: KeyErrorReason::InvalidUtf8,
        }
    }

    // NotFound errors

    pub fn key_not_found(key: &str) -> StrataError {
        StrataError::NotFound {
            message: "Key not found".into(),
            key: Some(key.to_string()),
        }
    }

    pub fn run_not_found(run_id: &str) -> StrataError {
        StrataError::NotFound {
            message: format!("Run '{}' not found", run_id),
            key: Some(run_id.to_string()),
        }
    }

    // Type errors

    pub fn wrong_type(expected: &str, actual: &str) -> StrataError {
        StrataError::WrongType {
            message: format!("Expected {}, found {}", expected, actual),
            expected: expected.to_string(),
            actual: actual.to_string(),
        }
    }

    pub fn incr_on_non_int(actual: &str) -> StrataError {
        StrataError::WrongType {
            message: "incr requires Int value".into(),
            expected: "Int".into(),
            actual: actual.to_string(),
        }
    }

    pub fn version_type_mismatch(expected: &str, actual: &str) -> StrataError {
        StrataError::WrongType {
            message: "Cannot compare versions of different types".into(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        }
    }

    // Constraint violations

    pub fn value_too_large(size: usize, max: usize) -> StrataError {
        StrataError::ConstraintViolation {
            message: format!("Value size {} exceeds maximum {}", size, max),
            reason: ConstraintReason::ValueTooLarge,
            details: Some([
                ("size".to_string(), serde_json::json!(size)),
                ("max".to_string(), serde_json::json!(max)),
            ].into_iter().collect()),
        }
    }

    pub fn nesting_too_deep(depth: usize, max: usize) -> StrataError {
        StrataError::ConstraintViolation {
            message: format!("Nesting depth {} exceeds maximum {}", depth, max),
            reason: ConstraintReason::NestingTooDeep,
            details: Some([
                ("depth".to_string(), serde_json::json!(depth)),
                ("max".to_string(), serde_json::json!(max)),
            ].into_iter().collect()),
        }
    }

    pub fn vector_dim_exceeded(dim: usize, max: usize) -> StrataError {
        StrataError::ConstraintViolation {
            message: format!("Vector dimension {} exceeds maximum {}", dim, max),
            reason: ConstraintReason::VectorDimExceeded,
            details: Some([
                ("dimension".to_string(), serde_json::json!(dim)),
                ("max".to_string(), serde_json::json!(max)),
            ].into_iter().collect()),
        }
    }

    pub fn vector_dim_mismatch(expected: usize, actual: usize) -> StrataError {
        StrataError::ConstraintViolation {
            message: format!("Vector dimension mismatch: expected {}, got {}", expected, actual),
            reason: ConstraintReason::VectorDimMismatch,
            details: Some([
                ("expected".to_string(), serde_json::json!(expected)),
                ("actual".to_string(), serde_json::json!(actual)),
            ].into_iter().collect()),
        }
    }

    pub fn root_not_object() -> StrataError {
        StrataError::ConstraintViolation {
            message: "JSON root must be an Object".into(),
            reason: ConstraintReason::RootNotObject,
            details: None,
        }
    }

    pub fn run_closed(run_id: &str) -> StrataError {
        StrataError::ConstraintViolation {
            message: format!("Run '{}' is closed", run_id),
            reason: ConstraintReason::RunClosed,
            details: Some([
                ("run_id".to_string(), serde_json::json!(run_id)),
            ].into_iter().collect()),
        }
    }

    // Path errors

    pub fn invalid_path(path: &str, reason: &str) -> StrataError {
        StrataError::InvalidPath {
            message: format!("Invalid path '{}': {}", path, reason),
            path: path.to_string(),
        }
    }

    // History errors

    pub fn history_trimmed(requested: Version, earliest: Version) -> StrataError {
        StrataError::HistoryTrimmed {
            message: "Requested version no longer retained".into(),
            requested,
            earliest_retained: earliest,
        }
    }

    // Conflict errors

    pub fn cas_mismatch() -> StrataError {
        StrataError::Conflict {
            message: "CAS comparison failed".into(),
            details: None,
        }
    }

    pub fn stale_transaction() -> StrataError {
        StrataError::Conflict {
            message: "Transaction handle is stale or already committed".into(),
            details: None,
        }
    }

    pub fn transaction_conflict() -> StrataError {
        StrataError::Conflict {
            message: "Transaction conflict detected".into(),
            details: None,
        }
    }

    // Overflow errors

    pub fn incr_overflow(key: &str) -> StrataError {
        StrataError::Overflow {
            message: format!("Increment would overflow for key '{}'", key),
            operation: "incr".into(),
        }
    }

    pub fn incr_underflow(key: &str) -> StrataError {
        StrataError::Overflow {
            message: format!("Decrement would underflow for key '{}'", key),
            operation: "incr".into(),
        }
    }
}
```

### Acceptance Criteria

- [ ] Factory functions for all error conditions
- [ ] Consistent message formatting
- [ ] Appropriate details included
- [ ] Complete coverage of error-producing conditions table

---

## Story #564: Overflow Error for Numeric Operations

**File**: `crates/core/src/error/codes.rs`

**Deliverable**: Overflow error handling for incr operation

### Implementation

(Included in Story #560 and #563)

### Acceptance Criteria

- [ ] `Overflow` error code for numeric overflow/underflow
- [ ] `incr_overflow()` factory for positive overflow
- [ ] `incr_underflow()` factory for negative overflow
- [ ] Operation field identifies which operation caused overflow

---

## Story #565: Error Documentation

**File**: `docs/architecture/ERROR_CODES.md` (NEW)

**Deliverable**: Complete error code documentation

### Implementation

```markdown
# Strata Error Codes

## Overview

All errors in Strata are represented by the `StrataError` enum. Each error has:
- A stable **code** (e.g., "NotFound")
- A human-readable **message**
- Optional structured **details**

## Error Codes

| Code | Meaning | Category | Retryable |
|------|---------|----------|-----------|
| NotFound | Entity or key not found | - | No |
| WrongType | Wrong primitive or value type | Structural | No |
| InvalidKey | Key syntax invalid | Structural | No |
| InvalidPath | JSON path invalid | Structural | No |
| HistoryTrimmed | Requested version no longer retained | - | No |
| ConstraintViolation | Schema/shape/invariant violation | Structural | No |
| Conflict | CAS failure, transaction conflict | Temporal | Yes |
| SerializationError | Value encode/decode failure | - | No |
| StorageError | Disk, WAL, or IO failure | - | Maybe |
| InternalError | Bug or invariant violation | - | No |
| Overflow | Numeric overflow/underflow | Structural | No |

## Conflict vs ConstraintViolation

- **Conflict**: Temporal failures - the same operation might succeed later
  - CAS comparison failed (value changed)
  - Transaction conflict (concurrent modification)
  - Version mismatch

- **ConstraintViolation**: Structural failures - the operation is invalid
  - Type mismatch
  - Size limits exceeded
  - Invalid value shape

## ConstraintViolation Reason Codes

| Reason | Description |
|--------|-------------|
| value_too_large | Value exceeds size limits |
| nesting_too_deep | Nesting exceeds max depth |
| key_too_long | Key exceeds max length |
| vector_dim_exceeded | Vector dimension exceeds max |
| vector_dim_mismatch | Vector dimension doesn't match existing |
| root_not_object | JSON root must be Object |
| reserved_prefix | Key uses `_strata/` prefix |
| run_closed | Operating on closed run |
```

### Acceptance Criteria

- [ ] All error codes documented
- [ ] Retryable status documented
- [ ] Conflict vs ConstraintViolation distinction explained
- [ ] All reason codes documented

---

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_error_codes() {
        let errors = vec![
            StrataError::NotFound { message: "".into(), key: None },
            StrataError::WrongType { message: "".into(), expected: "".into(), actual: "".into() },
            StrataError::InvalidKey { message: "".into(), key: "".into(), reason: KeyErrorReason::Empty },
            StrataError::InvalidPath { message: "".into(), path: "".into() },
            StrataError::HistoryTrimmed { message: "".into(), requested: Version::Txn(1), earliest_retained: Version::Txn(2) },
            StrataError::ConstraintViolation { message: "".into(), reason: ConstraintReason::ValueTooLarge, details: None },
            StrataError::Conflict { message: "".into(), details: None },
            StrataError::SerializationError { message: "".into(), details: None },
            StrataError::StorageError { message: "".into(), source: None },
            StrataError::InternalError { message: "".into(), details: None },
            StrataError::Overflow { message: "".into(), operation: "".into() },
        ];

        // Ensure we have exactly 11 error codes
        assert_eq!(errors.len(), 11);

        // Ensure all codes are unique
        let codes: Vec<_> = errors.iter().map(|e| e.code()).collect();
        let unique: std::collections::HashSet<_> = codes.iter().collect();
        assert_eq!(unique.len(), 11);
    }

    #[test]
    fn test_conflict_is_retryable() {
        let conflict = StrataError::Conflict { message: "".into(), details: None };
        assert!(conflict.is_retryable());

        let not_found = StrataError::NotFound { message: "".into(), key: None };
        assert!(!not_found.is_retryable());
    }

    #[test]
    fn test_internal_error_is_bug() {
        let internal = StrataError::InternalError { message: "".into(), details: None };
        assert!(internal.is_bug());

        let conflict = StrataError::Conflict { message: "".into(), details: None };
        assert!(!conflict.is_bug());
    }

    #[test]
    fn test_wire_format() {
        let err = ErrorFactory::key_not_found("mykey");
        let wire = err.to_wire_response();

        assert_eq!(wire["ok"], false);
        assert_eq!(wire["error"]["code"], "NotFound");
        assert!(wire["error"]["message"].is_string());
        assert_eq!(wire["error"]["details"]["key"], "mykey");
    }
}
```

---

## Files Modified/Created

| File | Action |
|------|--------|
| `crates/core/src/error/mod.rs` | CREATE - Error module entry |
| `crates/core/src/error/codes.rs` | CREATE - StrataError enum |
| `crates/core/src/error/wire.rs` | CREATE - Wire error shape |
| `crates/core/src/error/constraint.rs` | CREATE - Constraint reasons |
| `crates/core/src/error/conditions.rs` | CREATE - Error factories |
| `docs/architecture/ERROR_CODES.md` | CREATE - Error documentation |
