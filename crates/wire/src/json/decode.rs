//! JSON decoding for Strata values
//!
//! Implements decoding of JSON strings to Value, handling special wrappers:
//! - `$bytes` for binary data (base64)
//! - `$f64` for special floats (NaN, ±Inf, -0.0)
//! - `$absent` for CAS expected-missing

use base64::Engine;
use strata_core::Value;
use std::collections::HashMap;
use thiserror::Error;

/// Decode error types
#[derive(Debug, Error, PartialEq)]
pub enum DecodeError {
    /// Invalid JSON syntax
    #[error("Invalid JSON: {0}")]
    InvalidJson(String),

    /// Invalid number format
    #[error("Invalid number: {0}")]
    InvalidNumber(String),

    /// Invalid base64 in $bytes wrapper
    #[error("Invalid base64: {0}")]
    InvalidBase64(String),

    /// Invalid value in $f64 wrapper
    #[error("Invalid $f64 value: {0}")]
    InvalidF64Wrapper(String),

    /// Invalid version type
    #[error("Invalid version type: {0}")]
    InvalidVersionType(String),

    /// Unexpected end of input
    #[error("Unexpected end of input")]
    UnexpectedEnd,

    /// Unexpected character
    #[error("Unexpected character: {0}")]
    UnexpectedChar(char),
}

/// Decode a JSON string to Value
pub fn decode_json(json: &str) -> Result<Value, DecodeError> {
    let trimmed = json.trim();
    if trimmed.is_empty() {
        return Err(DecodeError::UnexpectedEnd);
    }

    let mut parser = JsonParser::new(trimmed);
    parser.parse_value()
}

/// Simple JSON parser
struct JsonParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(input: &'a str) -> Self {
        JsonParser { input, pos: 0 }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn advance(&mut self) {
        if let Some(c) = self.peek() {
            self.pos += c.len_utf8();
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn parse_value(&mut self) -> Result<Value, DecodeError> {
        self.skip_whitespace();

        match self.peek() {
            None => Err(DecodeError::UnexpectedEnd),
            Some('n') => self.parse_null(),
            Some('t') => self.parse_true(),
            Some('f') => self.parse_false(),
            Some('"') => self.parse_string().map(Value::String),
            Some('[') => self.parse_array(),
            Some('{') => self.parse_object_or_wrapper(),
            Some(c) if c == '-' || c.is_ascii_digit() => self.parse_number(),
            Some(c) => Err(DecodeError::UnexpectedChar(c)),
        }
    }

    fn parse_null(&mut self) -> Result<Value, DecodeError> {
        if self.input[self.pos..].starts_with("null") {
            self.pos += 4;
            Ok(Value::Null)
        } else {
            Err(DecodeError::InvalidJson("Expected 'null'".to_string()))
        }
    }

    fn parse_true(&mut self) -> Result<Value, DecodeError> {
        if self.input[self.pos..].starts_with("true") {
            self.pos += 4;
            Ok(Value::Bool(true))
        } else {
            Err(DecodeError::InvalidJson("Expected 'true'".to_string()))
        }
    }

    fn parse_false(&mut self) -> Result<Value, DecodeError> {
        if self.input[self.pos..].starts_with("false") {
            self.pos += 5;
            Ok(Value::Bool(false))
        } else {
            Err(DecodeError::InvalidJson("Expected 'false'".to_string()))
        }
    }

    fn parse_string(&mut self) -> Result<String, DecodeError> {
        self.advance(); // consume opening quote
        let mut result = String::new();

        loop {
            match self.peek() {
                None => return Err(DecodeError::UnexpectedEnd),
                Some('"') => {
                    self.advance();
                    return Ok(result);
                }
                Some('\\') => {
                    self.advance();
                    match self.peek() {
                        Some('"') => {
                            result.push('"');
                            self.advance();
                        }
                        Some('\\') => {
                            result.push('\\');
                            self.advance();
                        }
                        Some('/') => {
                            result.push('/');
                            self.advance();
                        }
                        Some('n') => {
                            result.push('\n');
                            self.advance();
                        }
                        Some('r') => {
                            result.push('\r');
                            self.advance();
                        }
                        Some('t') => {
                            result.push('\t');
                            self.advance();
                        }
                        Some('b') => {
                            result.push('\x08');
                            self.advance();
                        }
                        Some('f') => {
                            result.push('\x0c');
                            self.advance();
                        }
                        Some('u') => {
                            self.advance();
                            let hex: String = (0..4)
                                .filter_map(|_| {
                                    let c = self.peek()?;
                                    self.advance();
                                    Some(c)
                                })
                                .collect();
                            if hex.len() != 4 {
                                return Err(DecodeError::InvalidJson(
                                    "Invalid unicode escape".to_string(),
                                ));
                            }
                            let code = u32::from_str_radix(&hex, 16).map_err(|_| {
                                DecodeError::InvalidJson("Invalid unicode escape".to_string())
                            })?;
                            if let Some(c) = char::from_u32(code) {
                                result.push(c);
                            } else {
                                return Err(DecodeError::InvalidJson(
                                    "Invalid unicode codepoint".to_string(),
                                ));
                            }
                        }
                        Some(c) => {
                            return Err(DecodeError::InvalidJson(format!(
                                "Invalid escape: \\{}",
                                c
                            )))
                        }
                        None => return Err(DecodeError::UnexpectedEnd),
                    }
                }
                Some(c) => {
                    result.push(c);
                    self.advance();
                }
            }
        }
    }

    fn parse_number(&mut self) -> Result<Value, DecodeError> {
        let start = self.pos;

        // Handle negative sign
        if self.peek() == Some('-') {
            self.advance();
        }

        // Parse integer part
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.advance();
            } else {
                break;
            }
        }

        let mut is_float = false;

        // Parse decimal part
        if self.peek() == Some('.') {
            is_float = true;
            self.advance();
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        // Parse exponent
        if let Some('e' | 'E') = self.peek() {
            is_float = true;
            self.advance();
            if let Some('+' | '-') = self.peek() {
                self.advance();
            }
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        let num_str = &self.input[start..self.pos];

        if is_float {
            num_str
                .parse::<f64>()
                .map(Value::Float)
                .map_err(|_| DecodeError::InvalidNumber(num_str.to_string()))
        } else {
            // Try parsing as i64 first
            if let Ok(i) = num_str.parse::<i64>() {
                Ok(Value::Int(i))
            } else {
                // Fall back to f64 for large numbers
                num_str
                    .parse::<f64>()
                    .map(Value::Float)
                    .map_err(|_| DecodeError::InvalidNumber(num_str.to_string()))
            }
        }
    }

    fn parse_array(&mut self) -> Result<Value, DecodeError> {
        self.advance(); // consume '['
        self.skip_whitespace();

        let mut arr = Vec::new();

        if self.peek() == Some(']') {
            self.advance();
            return Ok(Value::Array(arr));
        }

        loop {
            arr.push(self.parse_value()?);
            self.skip_whitespace();

            match self.peek() {
                Some(',') => {
                    self.advance();
                    self.skip_whitespace();
                }
                Some(']') => {
                    self.advance();
                    return Ok(Value::Array(arr));
                }
                Some(c) => return Err(DecodeError::UnexpectedChar(c)),
                None => return Err(DecodeError::UnexpectedEnd),
            }
        }
    }

    fn parse_object_or_wrapper(&mut self) -> Result<Value, DecodeError> {
        let obj = self.parse_object_raw()?;

        // Check for special wrappers (single-key objects with $ prefix)
        if obj.len() == 1 {
            if let Some(Value::String(b64)) = obj.get("$bytes") {
                return decode_bytes_wrapper(b64);
            }
            if let Some(Value::String(f64_str)) = obj.get("$f64") {
                return decode_f64_wrapper(f64_str);
            }
            // $absent: true stays as an object - caller uses is_absent() to check
        }

        Ok(Value::Object(obj))
    }

    fn parse_object_raw(&mut self) -> Result<HashMap<String, Value>, DecodeError> {
        self.advance(); // consume '{'
        self.skip_whitespace();

        let mut map = HashMap::new();

        if self.peek() == Some('}') {
            self.advance();
            return Ok(map);
        }

        loop {
            self.skip_whitespace();

            // Parse key
            if self.peek() != Some('"') {
                return Err(DecodeError::InvalidJson("Expected string key".to_string()));
            }
            let key = self.parse_string()?;

            self.skip_whitespace();

            // Expect colon
            if self.peek() != Some(':') {
                return Err(DecodeError::InvalidJson("Expected ':'".to_string()));
            }
            self.advance();

            // Parse value
            let value = self.parse_value()?;
            map.insert(key, value);

            self.skip_whitespace();

            match self.peek() {
                Some(',') => {
                    self.advance();
                }
                Some('}') => {
                    self.advance();
                    return Ok(map);
                }
                Some(c) => return Err(DecodeError::UnexpectedChar(c)),
                None => return Err(DecodeError::UnexpectedEnd),
            }
        }
    }
}

/// Decode $bytes wrapper (base64)
fn decode_bytes_wrapper(b64: &str) -> Result<Value, DecodeError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| DecodeError::InvalidBase64(e.to_string()))?;
    Ok(Value::Bytes(bytes))
}

/// Decode $f64 wrapper (special floats)
fn decode_f64_wrapper(value: &str) -> Result<Value, DecodeError> {
    let f = match value {
        "NaN" => f64::NAN,
        "+Inf" => f64::INFINITY,
        "-Inf" => f64::NEG_INFINITY,
        "-0.0" => -0.0_f64,
        _ => return Err(DecodeError::InvalidF64Wrapper(value.to_string())),
    };
    Ok(Value::Float(f))
}

/// Parse a JSON object (used by version decoder)
pub fn parse_json_object(json: &str) -> Result<HashMap<String, Value>, DecodeError> {
    let mut parser = JsonParser::new(json.trim());
    parser.skip_whitespace();
    if parser.peek() != Some('{') {
        return Err(DecodeError::InvalidJson("Expected object".to_string()));
    }
    parser.parse_object_raw()
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Null ===

    #[test]
    fn test_decode_null() {
        let value = decode_json("null").unwrap();
        assert!(matches!(value, Value::Null));
    }

    // === Bool ===

    #[test]
    fn test_decode_bool_true() {
        let value = decode_json("true").unwrap();
        assert!(matches!(value, Value::Bool(true)));
    }

    #[test]
    fn test_decode_bool_false() {
        let value = decode_json("false").unwrap();
        assert!(matches!(value, Value::Bool(false)));
    }

    // === Int ===

    #[test]
    fn test_decode_int() {
        let value = decode_json("42").unwrap();
        assert!(matches!(value, Value::Int(42)));
    }

    #[test]
    fn test_decode_int_negative() {
        let value = decode_json("-123").unwrap();
        assert!(matches!(value, Value::Int(-123)));
    }

    #[test]
    fn test_decode_int_max() {
        let value = decode_json("9223372036854775807").unwrap();
        assert!(matches!(value, Value::Int(i64::MAX)));
    }

    #[test]
    fn test_decode_int_min() {
        let value = decode_json("-9223372036854775808").unwrap();
        assert!(matches!(value, Value::Int(i64::MIN)));
    }

    // === Float ===

    #[test]
    fn test_decode_float() {
        let value = decode_json("3.14").unwrap();
        match value {
            Value::Float(f) => assert!((f - 3.14).abs() < f64::EPSILON),
            _ => panic!("Expected Float"),
        }
    }

    #[test]
    fn test_decode_float_negative() {
        let value = decode_json("-2.5").unwrap();
        match value {
            Value::Float(f) => assert!((f - (-2.5)).abs() < f64::EPSILON),
            _ => panic!("Expected Float"),
        }
    }

    #[test]
    fn test_decode_float_exponent() {
        let value = decode_json("1.5e10").unwrap();
        match value {
            Value::Float(f) => assert!((f - 1.5e10).abs() < 1.0),
            _ => panic!("Expected Float"),
        }
    }

    // === Float ($f64 wrapper) ===

    #[test]
    fn test_decode_nan_wrapper() {
        let value = decode_json(r#"{"$f64":"NaN"}"#).unwrap();
        match value {
            Value::Float(f) => assert!(f.is_nan()),
            _ => panic!("Expected Float"),
        }
    }

    #[test]
    fn test_decode_positive_inf_wrapper() {
        let value = decode_json(r#"{"$f64":"+Inf"}"#).unwrap();
        assert!(matches!(value, Value::Float(f) if f == f64::INFINITY));
    }

    #[test]
    fn test_decode_negative_inf_wrapper() {
        let value = decode_json(r#"{"$f64":"-Inf"}"#).unwrap();
        assert!(matches!(value, Value::Float(f) if f == f64::NEG_INFINITY));
    }

    #[test]
    fn test_decode_negative_zero_wrapper() {
        let value = decode_json(r#"{"$f64":"-0.0"}"#).unwrap();
        match value {
            Value::Float(f) => {
                assert_eq!(f, 0.0);
                assert!(f.is_sign_negative());
            }
            _ => panic!("Expected Float"),
        }
    }

    #[test]
    fn test_f64_wrapper_invalid_value() {
        let result = decode_json(r#"{"$f64":"invalid"}"#);
        assert!(matches!(result, Err(DecodeError::InvalidF64Wrapper(_))));
    }

    // === String ===

    #[test]
    fn test_decode_string() {
        let value = decode_json(r#""hello""#).unwrap();
        assert!(matches!(value, Value::String(s) if s == "hello"));
    }

    #[test]
    fn test_decode_string_empty() {
        let value = decode_json(r#""""#).unwrap();
        assert!(matches!(value, Value::String(s) if s.is_empty()));
    }

    #[test]
    fn test_decode_string_unicode() {
        let value = decode_json(r#""日本語""#).unwrap();
        assert!(matches!(value, Value::String(s) if s == "日本語"));
    }

    #[test]
    fn test_decode_string_escapes() {
        let value = decode_json(r#""a\n\t\"b""#).unwrap();
        assert!(matches!(value, Value::String(s) if s == "a\n\t\"b"));
    }

    // === Bytes ($bytes wrapper) ===

    #[test]
    fn test_decode_bytes() {
        let value = decode_json(r#"{"$bytes":"SGVsbG8="}"#).unwrap();
        assert!(matches!(value, Value::Bytes(b) if b == vec![72, 101, 108, 108, 111]));
    }

    #[test]
    fn test_decode_bytes_empty() {
        let value = decode_json(r#"{"$bytes":""}"#).unwrap();
        assert!(matches!(value, Value::Bytes(b) if b.is_empty()));
    }

    #[test]
    fn test_decode_bytes_invalid_base64() {
        let result = decode_json(r#"{"$bytes":"!!invalid!!"}"#);
        assert!(matches!(result, Err(DecodeError::InvalidBase64(_))));
    }

    // === Array ===

    #[test]
    fn test_decode_array() {
        let value = decode_json("[1,2,3]").unwrap();
        match value {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 3);
                assert!(matches!(arr[0], Value::Int(1)));
                assert!(matches!(arr[1], Value::Int(2)));
                assert!(matches!(arr[2], Value::Int(3)));
            }
            _ => panic!("Expected Array"),
        }
    }

    #[test]
    fn test_decode_array_empty() {
        let value = decode_json("[]").unwrap();
        assert!(matches!(value, Value::Array(arr) if arr.is_empty()));
    }

    #[test]
    fn test_decode_array_nested() {
        let value = decode_json("[[1]]").unwrap();
        match value {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 1);
                assert!(matches!(&arr[0], Value::Array(inner) if inner.len() == 1));
            }
            _ => panic!("Expected Array"),
        }
    }

    // === Object ===

    #[test]
    fn test_decode_object() {
        let value = decode_json(r#"{"key":"value"}"#).unwrap();
        match value {
            Value::Object(map) => {
                assert_eq!(map.get("key"), Some(&Value::String("value".to_string())));
            }
            _ => panic!("Expected Object"),
        }
    }

    #[test]
    fn test_decode_object_empty() {
        let value = decode_json("{}").unwrap();
        assert!(matches!(value, Value::Object(m) if m.is_empty()));
    }

    #[test]
    fn test_decode_object_nested() {
        let value = decode_json(r#"{"a":{"b":1}}"#).unwrap();
        match value {
            Value::Object(outer) => {
                match outer.get("a") {
                    Some(Value::Object(inner)) => {
                        assert_eq!(inner.get("b"), Some(&Value::Int(1)));
                    }
                    _ => panic!("Expected nested object"),
                }
            }
            _ => panic!("Expected Object"),
        }
    }

    // === $absent wrapper ===

    #[test]
    fn test_decode_absent_wrapper() {
        let value = decode_json(r#"{"$absent":true}"#).unwrap();
        // $absent stays as object, caller uses is_absent() to check
        match value {
            Value::Object(map) => {
                assert_eq!(map.get("$absent"), Some(&Value::Bool(true)));
            }
            _ => panic!("Expected Object"),
        }
    }

    // === Error cases ===

    #[test]
    fn test_decode_empty_input() {
        let result = decode_json("");
        assert!(matches!(result, Err(DecodeError::UnexpectedEnd)));
    }

    #[test]
    fn test_decode_invalid_json() {
        let result = decode_json("invalid");
        assert!(result.is_err());
    }
}
