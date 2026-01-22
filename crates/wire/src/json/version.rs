//! Version and Versioned<T> wire encoding
//!
//! Implements encoding for:
//! - Version tagged union: `{type: "txn"|"sequence"|"counter", value: N}`
//! - Versioned<T>: `{value: T, version: Version, timestamp: u64}`

use super::decode::{parse_json_object, DecodeError};
use super::encode::encode_json;
use strata_core::Value;

/// Version type (tagged union)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    /// Transaction version (for KV, JSON)
    Txn(u64),
    /// Sequence version (for Events)
    Sequence(u64),
    /// Counter version (for State/CAS)
    Counter(u64),
}

impl Version {
    /// Get the numeric value
    pub fn value(&self) -> u64 {
        match self {
            Version::Txn(v) | Version::Sequence(v) | Version::Counter(v) => *v,
        }
    }

    /// Get the type name
    pub fn type_name(&self) -> &'static str {
        match self {
            Version::Txn(_) => "txn",
            Version::Sequence(_) => "sequence",
            Version::Counter(_) => "counter",
        }
    }
}

/// Versioned value with metadata
#[derive(Debug, Clone, PartialEq)]
pub struct Versioned<T> {
    /// The value
    pub value: T,
    /// Version information
    pub version: Version,
    /// Timestamp in microseconds since Unix epoch
    pub timestamp: u64,
}

/// Encode a Version to JSON
pub fn encode_version(version: &Version) -> String {
    format!(
        r#"{{"type":"{}","value":{}}}"#,
        version.type_name(),
        version.value(),
    )
}

/// Decode a Version from JSON
pub fn decode_version(json: &str) -> Result<Version, DecodeError> {
    let obj = parse_json_object(json)?;

    let type_str = match obj.get("type") {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(DecodeError::InvalidVersionType(
                "missing type".to_string(),
            ))
        }
    };

    let value = match obj.get("value") {
        Some(Value::Int(v)) => *v as u64,
        // Handle large u64 values that exceed i64::MAX (decoded as Float)
        Some(Value::Float(f)) if f.fract() == 0.0 && *f >= 0.0 => *f as u64,
        _ => {
            return Err(DecodeError::InvalidVersionType(
                "missing value".to_string(),
            ))
        }
    };

    match type_str {
        "txn" => Ok(Version::Txn(value)),
        "sequence" => Ok(Version::Sequence(value)),
        "counter" => Ok(Version::Counter(value)),
        _ => Err(DecodeError::InvalidVersionType(type_str.to_string())),
    }
}

/// Encode a Versioned<Value> to JSON
pub fn encode_versioned(versioned: &Versioned<Value>) -> String {
    format!(
        r#"{{"value":{},"version":{},"timestamp":{}}}"#,
        encode_json(&versioned.value),
        encode_version(&versioned.version),
        versioned.timestamp,
    )
}

/// Decode a Versioned<Value> from JSON
pub fn decode_versioned(json: &str) -> Result<Versioned<Value>, DecodeError> {
    let obj = parse_json_object(json)?;

    let value = match obj.get("value") {
        Some(v) => v.clone(),
        None => return Err(DecodeError::InvalidJson("missing value".to_string())),
    };

    let version_obj = match obj.get("version") {
        Some(Value::Object(m)) => m,
        _ => return Err(DecodeError::InvalidJson("missing version".to_string())),
    };

    let version_type = match version_obj.get("type") {
        Some(Value::String(s)) => s.as_str(),
        _ => {
            return Err(DecodeError::InvalidVersionType(
                "missing version type".to_string(),
            ))
        }
    };

    let version_value = match version_obj.get("value") {
        Some(Value::Int(v)) => *v as u64,
        // Handle large u64 values that exceed i64::MAX (decoded as Float)
        Some(Value::Float(f)) if f.fract() == 0.0 && *f >= 0.0 => *f as u64,
        _ => {
            return Err(DecodeError::InvalidVersionType(
                "missing version value".to_string(),
            ))
        }
    };

    let version = match version_type {
        "txn" => Version::Txn(version_value),
        "sequence" => Version::Sequence(version_value),
        "counter" => Version::Counter(version_value),
        _ => return Err(DecodeError::InvalidVersionType(version_type.to_string())),
    };

    let timestamp = match obj.get("timestamp") {
        Some(Value::Int(t)) => *t as u64,
        _ => return Err(DecodeError::InvalidJson("missing timestamp".to_string())),
    };

    Ok(Versioned {
        value,
        version,
        timestamp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // === Version Encoding ===

    #[test]
    fn test_encode_txn_version() {
        let version = Version::Txn(123);
        let json = encode_version(&version);
        assert_eq!(json, r#"{"type":"txn","value":123}"#);
    }

    #[test]
    fn test_encode_sequence_version() {
        let version = Version::Sequence(456);
        let json = encode_version(&version);
        assert_eq!(json, r#"{"type":"sequence","value":456}"#);
    }

    #[test]
    fn test_encode_counter_version() {
        let version = Version::Counter(789);
        let json = encode_version(&version);
        assert_eq!(json, r#"{"type":"counter","value":789}"#);
    }

    #[test]
    fn test_encode_version_zero() {
        let version = Version::Txn(0);
        let json = encode_version(&version);
        assert_eq!(json, r#"{"type":"txn","value":0}"#);
    }

    #[test]
    fn test_encode_version_max() {
        let version = Version::Txn(u64::MAX);
        let json = encode_version(&version);
        assert!(json.contains("18446744073709551615"));
    }

    // === Version Decoding ===

    #[test]
    fn test_decode_txn_version() {
        let version = decode_version(r#"{"type":"txn","value":123}"#).unwrap();
        assert!(matches!(version, Version::Txn(123)));
    }

    #[test]
    fn test_decode_sequence_version() {
        let version = decode_version(r#"{"type":"sequence","value":456}"#).unwrap();
        assert!(matches!(version, Version::Sequence(456)));
    }

    #[test]
    fn test_decode_counter_version() {
        let version = decode_version(r#"{"type":"counter","value":789}"#).unwrap();
        assert!(matches!(version, Version::Counter(789)));
    }

    #[test]
    fn test_decode_invalid_version_type() {
        let result = decode_version(r#"{"type":"invalid","value":1}"#);
        assert!(matches!(result, Err(DecodeError::InvalidVersionType(_))));
    }

    #[test]
    fn test_version_round_trip() {
        for version in [
            Version::Txn(0),
            Version::Txn(123),
            Version::Sequence(456),
            Version::Counter(789),
            Version::Txn(u64::MAX),
        ] {
            let json = encode_version(&version);
            let decoded = decode_version(&json).unwrap();
            assert_eq!(version, decoded);
        }
    }

    // === Versioned<T> Encoding ===

    #[test]
    fn test_versioned_structure() {
        let versioned = Versioned {
            value: Value::Int(42),
            version: Version::Txn(100),
            timestamp: 1234567890,
        };

        let json = encode_versioned(&versioned);

        // Must have value, version, timestamp
        assert!(json.contains(r#""value":42"#));
        assert!(json.contains(r#""version":{"type":"txn","value":100}"#));
        assert!(json.contains(r#""timestamp":1234567890"#));
    }

    #[test]
    fn test_versioned_timestamp_is_u64() {
        let versioned = Versioned {
            value: Value::Null,
            version: Version::Txn(1),
            timestamp: 1234567890123456, // Microseconds
        };

        let json = encode_versioned(&versioned);

        // Timestamp must be a number, not a string
        assert!(json.contains(r#""timestamp":1234567890123456"#));
        assert!(!json.contains(r#""timestamp":"1234567890123456""#));
    }

    #[test]
    fn test_versioned_with_complex_value() {
        let mut map = HashMap::new();
        map.insert(
            "nested".to_string(),
            Value::Array(vec![Value::Int(1), Value::Int(2)]),
        );

        let versioned = Versioned {
            value: Value::Object(map),
            version: Version::Txn(50),
            timestamp: 999,
        };

        let json = encode_versioned(&versioned);
        let decoded = decode_versioned(&json).unwrap();

        assert_eq!(decoded.version, versioned.version);
        assert_eq!(decoded.timestamp, versioned.timestamp);
    }

    #[test]
    fn test_versioned_round_trip() {
        let versioned = Versioned {
            value: Value::String("test".to_string()),
            version: Version::Sequence(42),
            timestamp: 1000000,
        };

        let json = encode_versioned(&versioned);
        let decoded = decode_versioned(&json).unwrap();

        assert_eq!(decoded.value, versioned.value);
        assert_eq!(decoded.version, versioned.version);
        assert_eq!(decoded.timestamp, versioned.timestamp);
    }

    #[test]
    fn test_versioned_with_null() {
        let versioned = Versioned {
            value: Value::Null,
            version: Version::Counter(1),
            timestamp: 0,
        };

        let json = encode_versioned(&versioned);
        let decoded = decode_versioned(&json).unwrap();

        assert_eq!(decoded.value, Value::Null);
    }

    #[test]
    fn test_versioned_with_bytes() {
        let versioned = Versioned {
            value: Value::Bytes(vec![1, 2, 3]),
            version: Version::Txn(10),
            timestamp: 12345,
        };

        let json = encode_versioned(&versioned);
        let decoded = decode_versioned(&json).unwrap();

        assert_eq!(decoded.value, versioned.value);
    }
}
