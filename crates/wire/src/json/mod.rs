//! JSON wire encoding for Strata values
//!
//! This module implements JSON encoding and decoding for Strata's Value type.
//! Special wrappers are used for non-JSON-native values:
//!
//! - `{"$bytes": "<base64>"}` for binary data
//! - `{"$f64": "NaN|+Inf|-Inf|-0.0"}` for special floats
//! - `{"$absent": true}` for CAS expected-missing

mod decode;
mod encode;
mod envelope;
mod version;

pub use decode::{decode_json, parse_json_object, DecodeError};
pub use encode::{encode_json, encode_string};
pub use envelope::{
    decode_request, decode_response, encode_request, encode_response, ApiError, Request,
    RequestParams, Response,
};
pub use version::{decode_version, decode_versioned, encode_version, encode_versioned, Version, Versioned};

/// Encode the absent marker for CAS operations
///
/// Used to indicate "expected key does not exist" in CAS operations.
/// This is distinct from `null` which is a valid value.
pub fn encode_absent() -> String {
    r#"{"$absent":true}"#.to_string()
}

/// Check if a decoded value represents the absent marker
///
/// Returns true if the value is `{"$absent": true}`.
pub fn is_absent(value: &strata_core::Value) -> bool {
    match value {
        strata_core::Value::Object(map) if map.len() == 1 => {
            matches!(map.get("$absent"), Some(strata_core::Value::Bool(true)))
        }
        _ => false,
    }
}

#[cfg(test)]
mod absent_tests {
    use super::*;
    use strata_core::Value;
    use std::collections::HashMap;

    #[test]
    fn test_absent_wrapper_structure() {
        let json = encode_absent();
        assert_eq!(json, r#"{"$absent":true}"#);
    }

    #[test]
    fn test_absent_wrapper_value_is_bool_true() {
        let json = encode_absent();

        // Value must be boolean true, not 1, not "true"
        assert!(json.contains("true"));
        assert!(!json.contains("\"true\""));
        assert!(!json.contains(":1"));
    }

    #[test]
    fn test_decode_absent_wrapper() {
        let result = decode_json(r#"{"$absent":true}"#).unwrap();

        // $absent decodes to special Absent marker
        assert!(is_absent(&result));
    }

    #[test]
    fn test_absent_different_from_null() {
        let absent = encode_absent();
        let null = encode_json(&Value::Null);

        assert_ne!(absent, null);
        assert_eq!(absent, r#"{"$absent":true}"#);
        assert_eq!(null, "null");
    }

    #[test]
    fn test_absent_wrapper_collision_string() {
        // Object with $absent key but wrong value type
        let json = r#"{"$absent":"not_bool"}"#;
        let result = decode_json(json).unwrap();

        // Should decode as regular object, not absent marker
        assert!(matches!(result, Value::Object(_)));
        assert!(!is_absent(&result));
    }

    #[test]
    fn test_absent_wrapper_collision_false() {
        // Object with $absent: false - not the marker
        let json = r#"{"$absent":false}"#;
        let result = decode_json(json).unwrap();

        // false is not the absent marker
        assert!(matches!(result, Value::Object(_)));
        assert!(!is_absent(&result));
    }

    #[test]
    fn test_absent_wrapper_collision_multi_key() {
        // Object with $absent and other keys
        let mut map = HashMap::new();
        map.insert("$absent".to_string(), Value::Bool(true));
        map.insert("extra".to_string(), Value::Int(1));
        let value = Value::Object(map);

        // Not absent marker (has multiple keys)
        assert!(!is_absent(&value));
    }
}

#[cfg(test)]
mod roundtrip_tests {
    use super::*;
    use strata_core::Value;
    use std::collections::HashMap;

    #[test]
    fn test_round_trip_null() {
        let original = Value::Null;
        let json = encode_json(&original);
        let decoded = decode_json(&json).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_round_trip_bool() {
        for b in [true, false] {
            let original = Value::Bool(b);
            let json = encode_json(&original);
            let decoded = decode_json(&json).unwrap();
            assert_eq!(original, decoded);
        }
    }

    #[test]
    fn test_round_trip_int() {
        for i in [0, 1, -1, i64::MAX, i64::MIN, 42, -999] {
            let original = Value::Int(i);
            let json = encode_json(&original);
            let decoded = decode_json(&json).unwrap();
            assert_eq!(original, decoded);
        }
    }

    #[test]
    fn test_round_trip_float_normal() {
        for f in [0.0, 1.5, -2.5, 3.14159, 1e10, 1e-10] {
            let original = Value::Float(f);
            let json = encode_json(&original);
            let decoded = decode_json(&json).unwrap();
            match decoded {
                Value::Float(d) => assert!((f - d).abs() < f64::EPSILON * 10.0),
                _ => panic!("Expected Float"),
            }
        }
    }

    #[test]
    fn test_round_trip_float_nan() {
        let original = Value::Float(f64::NAN);
        let json = encode_json(&original);
        let decoded = decode_json(&json).unwrap();
        match decoded {
            Value::Float(f) => assert!(f.is_nan()),
            _ => panic!("Expected Float"),
        }
    }

    #[test]
    fn test_round_trip_float_infinity() {
        for f in [f64::INFINITY, f64::NEG_INFINITY] {
            let original = Value::Float(f);
            let json = encode_json(&original);
            let decoded = decode_json(&json).unwrap();
            assert_eq!(original, decoded);
        }
    }

    #[test]
    fn test_round_trip_float_negative_zero() {
        let original = Value::Float(-0.0);
        let json = encode_json(&original);
        let decoded = decode_json(&json).unwrap();
        match decoded {
            Value::Float(f) => {
                assert_eq!(f, 0.0);
                assert!(f.is_sign_negative());
            }
            _ => panic!("Expected Float"),
        }
    }

    #[test]
    fn test_round_trip_string() {
        for s in ["", "hello", "日本語", "a\n\t\"b", "with spaces"] {
            let original = Value::String(s.to_string());
            let json = encode_json(&original);
            let decoded = decode_json(&json).unwrap();
            assert_eq!(original, decoded);
        }
    }

    #[test]
    fn test_round_trip_bytes() {
        let test_cases: Vec<Vec<u8>> = vec![
            vec![],
            vec![0],
            vec![255],
            vec![0, 127, 255, 1, 254],
            (0..=255).collect(),
        ];

        for bytes in test_cases {
            let original = Value::Bytes(bytes);
            let json = encode_json(&original);
            let decoded = decode_json(&json).unwrap();
            assert_eq!(original, decoded);
        }
    }

    #[test]
    fn test_round_trip_array() {
        let original = Value::Array(vec![
            Value::Int(1),
            Value::String("two".to_string()),
            Value::Bool(true),
            Value::Null,
        ]);
        let json = encode_json(&original);
        let decoded = decode_json(&json).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_round_trip_object() {
        let mut map = HashMap::new();
        map.insert("int".to_string(), Value::Int(42));
        map.insert("str".to_string(), Value::String("hello".to_string()));
        map.insert("bool".to_string(), Value::Bool(true));
        map.insert("null".to_string(), Value::Null);

        let original = Value::Object(map);
        let json = encode_json(&original);
        let decoded = decode_json(&json).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_round_trip_nested() {
        let mut inner = HashMap::new();
        inner.insert("arr".to_string(), Value::Array(vec![Value::Int(1), Value::Int(2)]));

        let mut outer = HashMap::new();
        outer.insert("inner".to_string(), Value::Object(inner));
        outer.insert("bytes".to_string(), Value::Bytes(vec![1, 2, 3]));

        let original = Value::Object(outer);
        let json = encode_json(&original);
        let decoded = decode_json(&json).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_bytes_not_confused_with_array() {
        let bytes = Value::Bytes(vec![1, 2, 3]);
        let array = Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);

        let bytes_json = encode_json(&bytes);
        let array_json = encode_json(&array);

        // They should produce different JSON
        assert_ne!(bytes_json, array_json);

        // And decode back to correct types
        assert!(matches!(decode_json(&bytes_json).unwrap(), Value::Bytes(_)));
        assert!(matches!(decode_json(&array_json).unwrap(), Value::Array(_)));
    }
}
