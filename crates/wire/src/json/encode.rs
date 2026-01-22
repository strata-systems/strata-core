//! JSON encoding for Strata values
//!
//! Implements encoding of Value to JSON strings with special wrappers:
//! - `$bytes` for binary data (base64)
//! - `$f64` for special floats (NaN, ±Inf, -0.0)

use base64::Engine;
use strata_core::Value;
use std::collections::HashMap;

/// Encode a Value to JSON string
pub fn encode_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => encode_float(*f),
        Value::String(s) => encode_string(s),
        Value::Bytes(b) => encode_bytes(b),
        Value::Array(arr) => encode_array(arr),
        Value::Object(obj) => encode_object(obj),
    }
}

/// Encode a float, using $f64 wrapper for special values
fn encode_float(f: f64) -> String {
    if f.is_nan() {
        r#"{"$f64":"NaN"}"#.to_string()
    } else if f == f64::INFINITY {
        r#"{"$f64":"+Inf"}"#.to_string()
    } else if f == f64::NEG_INFINITY {
        r#"{"$f64":"-Inf"}"#.to_string()
    } else if f.to_bits() == (-0.0_f64).to_bits() {
        r#"{"$f64":"-0.0"}"#.to_string()
    } else {
        // Normal float - ensure decimal point for whole numbers
        format_normal_float(f)
    }
}

/// Format a normal float, ensuring it has a decimal point
fn format_normal_float(f: f64) -> String {
    let s = f.to_string();
    if s.contains('.') || s.contains('e') || s.contains('E') {
        s
    } else {
        format!("{}.0", s)
    }
}

/// Encode a string with proper JSON escaping
pub fn encode_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 2);
    result.push('"');
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => result.push(c),
        }
    }
    result.push('"');
    result
}

/// Encode bytes as $bytes wrapper with base64
fn encode_bytes(bytes: &[u8]) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    format!(r#"{{"$bytes":"{}"}}"#, b64)
}

/// Encode an array
fn encode_array(arr: &[Value]) -> String {
    let elements: Vec<String> = arr.iter().map(encode_json).collect();
    format!("[{}]", elements.join(","))
}

/// Encode an object with deterministic key ordering
fn encode_object(obj: &HashMap<String, Value>) -> String {
    // Sort keys for deterministic output
    let mut entries: Vec<_> = obj.iter().collect();
    entries.sort_by_key(|(k, _)| *k);

    let pairs: Vec<String> = entries
        .iter()
        .map(|(k, v)| format!("{}:{}", encode_string(k), encode_json(v)))
        .collect();

    format!("{{{}}}", pairs.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Null ===

    #[test]
    fn test_encode_null() {
        let value = Value::Null;
        let json = encode_json(&value);
        assert_eq!(json, "null");
    }

    // === Bool ===

    #[test]
    fn test_encode_bool_true() {
        let value = Value::Bool(true);
        let json = encode_json(&value);
        assert_eq!(json, "true");
    }

    #[test]
    fn test_encode_bool_false() {
        let value = Value::Bool(false);
        let json = encode_json(&value);
        assert_eq!(json, "false");
    }

    // === Int ===

    #[test]
    fn test_encode_int_positive() {
        let value = Value::Int(123);
        let json = encode_json(&value);
        assert_eq!(json, "123");
    }

    #[test]
    fn test_encode_int_negative() {
        let value = Value::Int(-456);
        let json = encode_json(&value);
        assert_eq!(json, "-456");
    }

    #[test]
    fn test_encode_int_zero() {
        let value = Value::Int(0);
        let json = encode_json(&value);
        assert_eq!(json, "0");
    }

    #[test]
    fn test_encode_int_max() {
        let value = Value::Int(i64::MAX);
        let json = encode_json(&value);
        assert_eq!(json, "9223372036854775807");
    }

    #[test]
    fn test_encode_int_min() {
        let value = Value::Int(i64::MIN);
        let json = encode_json(&value);
        assert_eq!(json, "-9223372036854775808");
    }

    // === Float (normal) ===

    #[test]
    fn test_encode_float_positive() {
        let value = Value::Float(1.5);
        let json = encode_json(&value);
        assert_eq!(json, "1.5");
    }

    #[test]
    fn test_encode_float_negative() {
        let value = Value::Float(-2.5);
        let json = encode_json(&value);
        assert_eq!(json, "-2.5");
    }

    #[test]
    fn test_encode_float_zero() {
        let value = Value::Float(0.0);
        let json = encode_json(&value);
        // Positive zero is plain JSON
        assert_eq!(json, "0.0");
    }

    // === Float (special - $f64 wrapper) ===

    #[test]
    fn test_encode_float_nan() {
        let value = Value::Float(f64::NAN);
        let json = encode_json(&value);
        assert_eq!(json, r#"{"$f64":"NaN"}"#);
    }

    #[test]
    fn test_encode_float_positive_infinity() {
        let value = Value::Float(f64::INFINITY);
        let json = encode_json(&value);
        assert_eq!(json, r#"{"$f64":"+Inf"}"#);
    }

    #[test]
    fn test_encode_float_negative_infinity() {
        let value = Value::Float(f64::NEG_INFINITY);
        let json = encode_json(&value);
        assert_eq!(json, r#"{"$f64":"-Inf"}"#);
    }

    #[test]
    fn test_encode_float_negative_zero() {
        let value = Value::Float(-0.0);
        let json = encode_json(&value);
        assert_eq!(json, r#"{"$f64":"-0.0"}"#);
    }

    #[test]
    fn test_positive_zero_no_wrapper() {
        let value = Value::Float(0.0);
        let json = encode_json(&value);
        // Positive zero is plain JSON, no wrapper
        assert_eq!(json, "0.0");
        assert!(!json.contains("$f64"));
    }

    #[test]
    fn test_normal_float_no_wrapper() {
        let value = Value::Float(1.5);
        let json = encode_json(&value);
        // Normal floats are plain JSON
        assert_eq!(json, "1.5");
        assert!(!json.contains("$f64"));
    }

    // === String ===

    #[test]
    fn test_encode_string_simple() {
        let value = Value::String("hello".to_string());
        let json = encode_json(&value);
        assert_eq!(json, r#""hello""#);
    }

    #[test]
    fn test_encode_string_empty() {
        let value = Value::String(String::new());
        let json = encode_json(&value);
        assert_eq!(json, r#""""#);
    }

    #[test]
    fn test_encode_string_unicode() {
        let value = Value::String("日本語".to_string());
        let json = encode_json(&value);
        assert_eq!(json, r#""日本語""#);
    }

    #[test]
    fn test_encode_string_escapes() {
        let value = Value::String("a\n\t\"b".to_string());
        let json = encode_json(&value);
        assert_eq!(json, r#""a\n\t\"b""#);
    }

    // === Bytes ($bytes wrapper) ===

    #[test]
    fn test_encode_bytes_hello() {
        let value = Value::Bytes(vec![72, 101, 108, 108, 111]); // "Hello"
        let json = encode_json(&value);
        assert_eq!(json, r#"{"$bytes":"SGVsbG8="}"#);
    }

    #[test]
    fn test_encode_bytes_empty() {
        let value = Value::Bytes(vec![]);
        let json = encode_json(&value);
        assert_eq!(json, r#"{"$bytes":""}"#);
    }

    #[test]
    fn test_encode_bytes_with_padding() {
        // Single byte needs padding
        let value = Value::Bytes(vec![65]); // "A"
        let json = encode_json(&value);
        assert_eq!(json, r#"{"$bytes":"QQ=="}"#);
    }

    #[test]
    fn test_bytes_wrapper_structure() {
        let value = Value::Bytes(vec![72, 101, 108, 108, 111]);
        let json = encode_json(&value);
        // Must be object with single $bytes key
        assert!(json.starts_with(r#"{"$bytes":"#));
        assert!(json.ends_with(r#""}"#));
    }

    // === Array ===

    #[test]
    fn test_encode_array_simple() {
        let value = Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        let json = encode_json(&value);
        assert_eq!(json, "[1,2,3]");
    }

    #[test]
    fn test_encode_array_empty() {
        let value = Value::Array(vec![]);
        let json = encode_json(&value);
        assert_eq!(json, "[]");
    }

    #[test]
    fn test_encode_array_nested() {
        let value = Value::Array(vec![Value::Array(vec![Value::Int(1)])]);
        let json = encode_json(&value);
        assert_eq!(json, "[[1]]");
    }

    #[test]
    fn test_encode_array_mixed_types() {
        let value = Value::Array(vec![
            Value::Int(1),
            Value::String("a".to_string()),
            Value::Bool(true),
        ]);
        let json = encode_json(&value);
        assert_eq!(json, r#"[1,"a",true]"#);
    }

    // === Object ===

    #[test]
    fn test_encode_object_simple() {
        let mut map = HashMap::new();
        map.insert("a".to_string(), Value::Int(1));
        let value = Value::Object(map);
        let json = encode_json(&value);
        assert_eq!(json, r#"{"a":1}"#);
    }

    #[test]
    fn test_encode_object_empty() {
        let value = Value::Object(HashMap::new());
        let json = encode_json(&value);
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_encode_object_nested() {
        let mut inner = HashMap::new();
        inner.insert("b".to_string(), Value::Int(1));
        let mut outer = HashMap::new();
        outer.insert("a".to_string(), Value::Object(inner));
        let value = Value::Object(outer);
        let json = encode_json(&value);
        assert_eq!(json, r#"{"a":{"b":1}}"#);
    }

    #[test]
    fn test_encode_object_deterministic_order() {
        let mut map = HashMap::new();
        map.insert("z".to_string(), Value::Int(1));
        map.insert("a".to_string(), Value::Int(2));
        map.insert("m".to_string(), Value::Int(3));
        let value = Value::Object(map);
        let json = encode_json(&value);
        // Keys should be sorted alphabetically
        assert_eq!(json, r#"{"a":2,"m":3,"z":1}"#);
    }
}
