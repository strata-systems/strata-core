//! Wire error encoding for Strata API errors
//!
//! This module provides JSON encoding for API errors, matching the wire format:
//! ```json
//! {
//!   "code": "NotFound",
//!   "message": "Key not found: mykey",
//!   "details": {"key": "mykey"}
//! }
//! ```

use super::encode::{encode_json, encode_string};
use strata_core::{ApiError, WireError};

/// Encode a WireError to JSON
pub fn encode_wire_error(error: &WireError) -> String {
    let details = match &error.details {
        Some(v) => encode_json(v),
        None => "null".to_string(),
    };

    format!(
        r#"{{"code":{},"message":{},"details":{}}}"#,
        encode_string(&error.code),
        encode_string(&error.message),
        details,
    )
}

/// Encode an ApiError directly to JSON wire format
pub fn encode_api_error(error: &ApiError) -> String {
    encode_wire_error(&error.to_wire_error())
}

#[cfg(test)]
mod tests {
    use super::*;
    use strata_core::Value;

    #[test]
    fn test_encode_wire_error_not_found() {
        let err = ApiError::NotFound {
            key: "test".into(),
        };
        let json = encode_api_error(&err);

        assert!(json.contains(r#""code":"NotFound""#));
        assert!(json.contains(r#""message":"#));
        assert!(json.contains(r#""details":"#));
        assert!(json.contains(r#""key":"test""#));
    }

    #[test]
    fn test_encode_wire_error_wrong_type() {
        let err = ApiError::WrongType {
            expected: "Int",
            actual: "Float",
        };
        let json = encode_api_error(&err);

        assert!(json.contains(r#""code":"WrongType""#));
        assert!(json.contains(r#""expected":"Int""#));
        assert!(json.contains(r#""actual":"Float""#));
    }

    #[test]
    fn test_encode_wire_error_overflow() {
        let err = ApiError::Overflow;
        let json = encode_api_error(&err);

        assert!(json.contains(r#""code":"Overflow""#));
        assert!(json.contains(r#""details":null"#));
    }

    #[test]
    fn test_encode_wire_error_constraint_violation() {
        use std::collections::HashMap;

        let mut extra = HashMap::new();
        extra.insert("actual_bytes".to_string(), Value::Int(20_000_000));
        extra.insert("max_bytes".to_string(), Value::Int(16_777_216));

        let err = ApiError::ConstraintViolation {
            reason: "value_too_large".to_string(),
            details: Some(Value::Object(extra)),
        };
        let json = encode_api_error(&err);

        assert!(json.contains(r#""code":"ConstraintViolation""#));
        assert!(json.contains(r#""reason":"value_too_large""#));
    }

    #[test]
    fn test_encode_wire_error_conflict() {
        let err = ApiError::Conflict {
            expected: Value::Int(1),
            actual: Value::Int(2),
        };
        let json = encode_api_error(&err);

        assert!(json.contains(r#""code":"Conflict""#));
        assert!(json.contains(r#""expected":1"#));
        assert!(json.contains(r#""actual":2"#));
    }

    #[test]
    fn test_encode_wire_error_run_not_found() {
        let err = ApiError::RunNotFound {
            run_id: "my-run".into(),
        };
        let json = encode_api_error(&err);

        assert!(json.contains(r#""code":"RunNotFound""#));
        assert!(json.contains(r#""run_id":"my-run""#));
    }

    #[test]
    fn test_encode_wire_error_internal() {
        let err = ApiError::Internal {
            message: "something broke".into(),
        };
        let json = encode_api_error(&err);

        assert!(json.contains(r#""code":"Internal""#));
        assert!(json.contains(r#""message":"something broke""#));
    }

    #[test]
    fn test_wire_error_code_is_string_not_number() {
        let err = ApiError::Overflow;
        let json = encode_api_error(&err);

        // Code must be a string, not a number
        assert!(json.contains(r#""code":"Overflow""#));
        assert!(!json.contains(r#""code":0"#));
    }

    #[test]
    fn test_wire_error_json_is_valid() {
        let errors = vec![
            ApiError::NotFound { key: "k".into() },
            ApiError::WrongType {
                expected: "Int",
                actual: "Float",
            },
            ApiError::InvalidKey {
                key: "".into(),
                reason: "empty".into(),
            },
            ApiError::InvalidPath {
                path: "$.".into(),
                reason: "syntax".into(),
            },
            ApiError::ConstraintViolation {
                reason: "test".into(),
                details: None,
            },
            ApiError::Conflict {
                expected: Value::Null,
                actual: Value::Null,
            },
            ApiError::RunNotFound { run_id: "r".into() },
            ApiError::RunClosed { run_id: "r".into() },
            ApiError::RunExists { run_id: "r".into() },
            ApiError::Overflow,
            ApiError::Internal {
                message: "bug".into(),
            },
        ];

        for err in errors {
            let json = encode_api_error(&err);

            // Should be valid JSON (starts and ends with braces)
            assert!(json.starts_with('{'));
            assert!(json.ends_with('}'));

            // Should contain expected fields
            assert!(json.contains(&format!(r#""code":"{}""#, err.error_code())));
        }
    }

    #[test]
    fn test_encode_wire_error_history_trimmed() {
        let err = ApiError::HistoryTrimmed {
            requested: strata_core::Version::TxnId(10),
            earliest_retained: strata_core::Version::TxnId(100),
        };
        let json = encode_api_error(&err);

        assert!(json.contains(r#""code":"HistoryTrimmed""#));
        assert!(json.contains(r#""requested":"#));
        assert!(json.contains(r#""earliest_retained":"#));
    }
}
