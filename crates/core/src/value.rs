//! Value types for Strata
//!
//! This module defines the canonical Value type for all Strata operations.
//! The Value enum has exactly 8 variants and is FROZEN after M11.
//!
//! ## M11 Contract
//!
//! After M11, this enum cannot change without a major version bump.
//! - No implicit type coercions
//! - IEEE-754 float equality semantics
//! - Bytes and String are distinct types
//!
//! ## Migration Note
//!
//! M11 renamed variants for consistency with contract:
//! - `I64` -> `Int`
//! - `F64` -> `Float`
//! - `Map` -> `Object`

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Canonical Strata Value type
///
/// This is the ONLY public value model. All API surfaces use this type.
/// After M11, this enum is FROZEN and cannot change without major version bump.
///
/// ## The Eight Types
///
/// 1. `Null` - JSON null / absence of value
/// 2. `Bool` - Boolean true or false
/// 3. `Int` - 64-bit signed integer
/// 4. `Float` - 64-bit IEEE-754 floating point
/// 5. `String` - UTF-8 encoded string
/// 6. `Bytes` - Arbitrary binary data (distinct from String)
/// 7. `Array` - Ordered sequence of values
/// 8. `Object` - String-keyed map of values
///
/// ## Equality Rules
///
/// - Different types are NEVER equal (no type coercion)
/// - `Int(1)` != `Float(1.0)`
/// - `String("abc")` != `Bytes([97, 98, 99])`
/// - Float uses IEEE-754 equality: `NaN != NaN`, `-0.0 == 0.0`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    /// JSON null / absence of value
    Null,

    /// Boolean true or false
    Bool(bool),

    /// 64-bit signed integer
    /// Range: -9,223,372,036,854,775,808 to 9,223,372,036,854,775,807
    Int(i64),

    /// 64-bit IEEE-754 floating point
    /// Supports: NaN, +Inf, -Inf, -0.0, subnormals
    Float(f64),

    /// UTF-8 encoded string
    String(String),

    /// Arbitrary binary data
    /// NOT equivalent to String - distinct type
    Bytes(Vec<u8>),

    /// Ordered sequence of values
    Array(Vec<Value>),

    /// String-keyed map of values
    Object(HashMap<String, Value>),
}

impl Value {
    /// Returns the type name as a string (for error messages)
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "Null",
            Value::Bool(_) => "Bool",
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::String(_) => "String",
            Value::Bytes(_) => "Bytes",
            Value::Array(_) => "Array",
            Value::Object(_) => "Object",
        }
    }

    /// Check if this value is null
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Try to get as bool
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to get as i64
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// Try to get as f64
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Try to get as string slice
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// Try to get as bytes slice
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Bytes(b) => Some(b),
            _ => None,
        }
    }

    /// Try to get as array slice
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Try to get as object reference
    pub fn as_object(&self) -> Option<&HashMap<String, Value>> {
        match self {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Check if this is a special float value requiring wire encoding wrapper
    ///
    /// Special floats: NaN, +Inf, -Inf, -0.0
    /// These require `{"$f64": "..."}` wrapper in JSON wire encoding.
    pub fn is_special_float(&self) -> bool {
        match self {
            Value::Float(f) => f.is_nan() || f.is_infinite() || (*f == 0.0 && f.is_sign_negative()),
            _ => false,
        }
    }

    /// Get the special float kind if this is a special float
    pub fn special_float_kind(&self) -> Option<SpecialFloatKind> {
        match self {
            Value::Float(f) => {
                if f.is_nan() {
                    Some(SpecialFloatKind::NaN)
                } else if *f == f64::INFINITY {
                    Some(SpecialFloatKind::PositiveInfinity)
                } else if *f == f64::NEG_INFINITY {
                    Some(SpecialFloatKind::NegativeInfinity)
                } else if *f == 0.0 && f.is_sign_negative() {
                    Some(SpecialFloatKind::NegativeZero)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// Kinds of special float values
///
/// These values require special encoding in JSON wire format using the `$f64` wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialFloatKind {
    /// IEEE-754 Not-a-Number
    NaN,
    /// IEEE-754 positive infinity (+Inf)
    PositiveInfinity,
    /// IEEE-754 negative infinity (-Inf)
    NegativeInfinity,
    /// IEEE-754 negative zero (-0.0)
    NegativeZero,
}

impl SpecialFloatKind {
    /// Convert to wire encoding string
    pub fn to_wire_string(&self) -> &'static str {
        match self {
            SpecialFloatKind::NaN => "NaN",
            SpecialFloatKind::PositiveInfinity => "+Inf",
            SpecialFloatKind::NegativeInfinity => "-Inf",
            SpecialFloatKind::NegativeZero => "-0.0",
        }
    }

    /// Parse from wire encoding string
    pub fn from_wire_string(s: &str) -> Option<Self> {
        match s {
            "NaN" => Some(SpecialFloatKind::NaN),
            "+Inf" => Some(SpecialFloatKind::PositiveInfinity),
            "-Inf" => Some(SpecialFloatKind::NegativeInfinity),
            "-0.0" => Some(SpecialFloatKind::NegativeZero),
            _ => None,
        }
    }

    /// Convert to f64 value
    pub fn to_f64(&self) -> f64 {
        match self {
            SpecialFloatKind::NaN => f64::NAN,
            SpecialFloatKind::PositiveInfinity => f64::INFINITY,
            SpecialFloatKind::NegativeInfinity => f64::NEG_INFINITY,
            SpecialFloatKind::NegativeZero => -0.0,
        }
    }
}

// ============================================================================
// Custom PartialEq Implementation (IEEE-754 semantics, no type coercion)
// ============================================================================

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // Same types
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => {
                // IEEE-754 equality: NaN != NaN, but -0.0 == 0.0
                a == b
            }
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Bytes(a), Value::Bytes(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Object(a), Value::Object(b)) => a == b,

            // Different types: NEVER equal (NO TYPE COERCION)
            _ => false,
        }
    }
}

// Note: We intentionally implement Eq even though Float doesn't satisfy reflexivity.
// This is because our Value type follows IEEE-754 semantics where NaN != NaN.
// Users comparing Values with NaN should be aware of this behavior.
impl Eq for Value {}

impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Discriminant first for type distinction
        std::mem::discriminant(self).hash(state);

        match self {
            Value::Null => {}
            Value::Bool(b) => b.hash(state),
            Value::Int(i) => i.hash(state),
            Value::Float(f) => {
                // Hash the bits for consistency
                // Note: -0.0 and 0.0 have different bits but equal values
                // We normalize to 0.0 bits for hashing to maintain hash consistency with equality
                if *f == 0.0 {
                    0u64.hash(state);
                } else {
                    f.to_bits().hash(state);
                }
            }
            Value::String(s) => s.hash(state),
            Value::Bytes(b) => b.hash(state),
            Value::Array(a) => {
                a.len().hash(state);
                for v in a {
                    v.hash(state);
                }
            }
            Value::Object(o) => {
                // Hash entries in sorted order for determinism
                let mut entries: Vec<_> = o.iter().collect();
                entries.sort_by_key(|(k, _)| *k);
                entries.len().hash(state);
                for (k, v) in entries {
                    k.hash(state);
                    v.hash(state);
                }
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Story #560: Value Construction Tests
    // ========================================================================

    mod value_construction_tests {
        use super::*;

        // === Null Tests ===

        #[test]
        fn test_null_construction() {
            let v = Value::Null;
            assert!(matches!(v, Value::Null));
        }

        // === Bool Tests ===

        #[test]
        fn test_bool_true_construction() {
            let v = Value::Bool(true);
            assert!(matches!(v, Value::Bool(true)));
        }

        #[test]
        fn test_bool_false_construction() {
            let v = Value::Bool(false);
            assert!(matches!(v, Value::Bool(false)));
        }

        // === Int Tests ===

        #[test]
        fn test_int_positive_construction() {
            let v = Value::Int(123);
            assert!(matches!(v, Value::Int(123)));
        }

        #[test]
        fn test_int_negative_construction() {
            let v = Value::Int(-456);
            assert!(matches!(v, Value::Int(-456)));
        }

        #[test]
        fn test_int_zero_construction() {
            let v = Value::Int(0);
            assert!(matches!(v, Value::Int(0)));
        }

        #[test]
        fn test_int_max_construction() {
            let v = Value::Int(i64::MAX);
            assert!(matches!(v, Value::Int(i64::MAX)));
        }

        #[test]
        fn test_int_min_construction() {
            let v = Value::Int(i64::MIN);
            assert!(matches!(v, Value::Int(i64::MIN)));
        }

        // === Float Tests ===

        #[test]
        fn test_float_positive_construction() {
            let v = Value::Float(1.23);
            match v {
                Value::Float(f) => assert!((f - 1.23).abs() < f64::EPSILON),
                _ => panic!("Expected Float"),
            }
        }

        #[test]
        fn test_float_negative_construction() {
            let v = Value::Float(-4.56);
            match v {
                Value::Float(f) => assert!((f - (-4.56)).abs() < f64::EPSILON),
                _ => panic!("Expected Float"),
            }
        }

        #[test]
        fn test_float_zero_construction() {
            let v = Value::Float(0.0);
            match v {
                Value::Float(f) => assert_eq!(f, 0.0),
                _ => panic!("Expected Float"),
            }
        }

        // === String Tests ===

        #[test]
        fn test_string_empty_construction() {
            let v = Value::String(String::new());
            assert!(matches!(v, Value::String(ref s) if s.is_empty()));
        }

        #[test]
        fn test_string_ascii_construction() {
            let v = Value::String("hello".to_string());
            assert!(matches!(v, Value::String(ref s) if s == "hello"));
        }

        #[test]
        fn test_string_unicode_construction() {
            let v = Value::String("こんにちは".to_string());
            assert!(matches!(v, Value::String(ref s) if s == "こんにちは"));
        }

        #[test]
        fn test_string_emoji_construction() {
            let v = Value::String("🚀🎉".to_string());
            assert!(matches!(v, Value::String(ref s) if s == "🚀🎉"));
        }

        // === Bytes Tests ===

        #[test]
        fn test_bytes_empty_construction() {
            let v = Value::Bytes(vec![]);
            assert!(matches!(v, Value::Bytes(ref b) if b.is_empty()));
        }

        #[test]
        fn test_bytes_binary_construction() {
            let v = Value::Bytes(vec![0, 255, 128]);
            assert!(matches!(v, Value::Bytes(ref b) if b == &[0, 255, 128]));
        }

        #[test]
        fn test_bytes_all_values_construction() {
            let all_bytes: Vec<u8> = (0..=255).collect();
            let v = Value::Bytes(all_bytes.clone());
            assert!(matches!(v, Value::Bytes(ref b) if b == &all_bytes));
        }

        // === Array Tests ===

        #[test]
        fn test_array_empty_construction() {
            let v = Value::Array(vec![]);
            assert!(matches!(v, Value::Array(ref a) if a.is_empty()));
        }

        #[test]
        fn test_array_single_element_construction() {
            let v = Value::Array(vec![Value::Int(1)]);
            assert!(matches!(v, Value::Array(ref a) if a.len() == 1));
        }

        #[test]
        fn test_array_mixed_types_construction() {
            let v = Value::Array(vec![
                Value::Int(1),
                Value::String("hello".to_string()),
                Value::Bool(true),
            ]);
            assert!(matches!(v, Value::Array(ref a) if a.len() == 3));
        }

        #[test]
        fn test_array_nested_construction() {
            let v = Value::Array(vec![Value::Array(vec![Value::Int(1)])]);
            match &v {
                Value::Array(outer) => {
                    assert_eq!(outer.len(), 1);
                    assert!(matches!(&outer[0], Value::Array(_)));
                }
                _ => panic!("Expected Array"),
            }
        }

        // === Object Tests ===

        #[test]
        fn test_object_empty_construction() {
            let v = Value::Object(HashMap::new());
            assert!(matches!(v, Value::Object(ref o) if o.is_empty()));
        }

        #[test]
        fn test_object_single_entry_construction() {
            let mut map = HashMap::new();
            map.insert("key".to_string(), Value::Int(42));
            let v = Value::Object(map);
            assert!(matches!(v, Value::Object(ref o) if o.len() == 1));
        }

        #[test]
        fn test_object_nested_construction() {
            let mut inner = HashMap::new();
            inner.insert("inner_key".to_string(), Value::Int(1));

            let mut outer = HashMap::new();
            outer.insert("outer_key".to_string(), Value::Object(inner));

            let v = Value::Object(outer);
            match &v {
                Value::Object(o) => {
                    assert!(matches!(o.get("outer_key"), Some(Value::Object(_))));
                }
                _ => panic!("Expected Object"),
            }
        }
    }

    // ========================================================================
    // Story #560: Type Name Tests
    // ========================================================================

    mod type_name_tests {
        use super::*;

        #[test]
        fn test_type_name_null() {
            assert_eq!(Value::Null.type_name(), "Null");
        }

        #[test]
        fn test_type_name_bool() {
            assert_eq!(Value::Bool(true).type_name(), "Bool");
        }

        #[test]
        fn test_type_name_int() {
            assert_eq!(Value::Int(42).type_name(), "Int");
        }

        #[test]
        fn test_type_name_float() {
            assert_eq!(Value::Float(3.14).type_name(), "Float");
        }

        #[test]
        fn test_type_name_string() {
            assert_eq!(Value::String("test".to_string()).type_name(), "String");
        }

        #[test]
        fn test_type_name_bytes() {
            assert_eq!(Value::Bytes(vec![1, 2, 3]).type_name(), "Bytes");
        }

        #[test]
        fn test_type_name_array() {
            assert_eq!(Value::Array(vec![]).type_name(), "Array");
        }

        #[test]
        fn test_type_name_object() {
            assert_eq!(Value::Object(HashMap::new()).type_name(), "Object");
        }

        #[test]
        fn test_all_type_names_unique() {
            let values = vec![
                Value::Null,
                Value::Bool(true),
                Value::Int(0),
                Value::Float(0.0),
                Value::String(String::new()),
                Value::Bytes(vec![]),
                Value::Array(vec![]),
                Value::Object(HashMap::new()),
            ];

            let type_names: std::collections::HashSet<_> =
                values.iter().map(|v| v.type_name()).collect();
            assert_eq!(type_names.len(), 8, "All 8 type names must be unique");
        }
    }

    // ========================================================================
    // Story #560: Accessor Tests
    // ========================================================================

    mod accessor_tests {
        use super::*;

        #[test]
        fn test_is_null() {
            assert!(Value::Null.is_null());
            assert!(!Value::Bool(false).is_null());
            assert!(!Value::Int(0).is_null());
        }

        #[test]
        fn test_as_bool() {
            assert_eq!(Value::Bool(true).as_bool(), Some(true));
            assert_eq!(Value::Bool(false).as_bool(), Some(false));
            assert_eq!(Value::Int(1).as_bool(), None);
        }

        #[test]
        fn test_as_int() {
            assert_eq!(Value::Int(42).as_int(), Some(42));
            assert_eq!(Value::Float(42.0).as_int(), None);
        }

        #[test]
        fn test_as_float() {
            assert_eq!(Value::Float(3.14).as_float(), Some(3.14));
            assert_eq!(Value::Int(3).as_float(), None);
        }

        #[test]
        fn test_as_str() {
            assert_eq!(Value::String("hello".to_string()).as_str(), Some("hello"));
            assert_eq!(Value::Bytes(b"hello".to_vec()).as_str(), None);
        }

        #[test]
        fn test_as_bytes() {
            assert_eq!(Value::Bytes(vec![1, 2, 3]).as_bytes(), Some(&[1, 2, 3][..]));
            assert_eq!(Value::String("test".to_string()).as_bytes(), None);
        }

        #[test]
        fn test_as_array() {
            let arr = vec![Value::Int(1), Value::Int(2)];
            let v = Value::Array(arr.clone());
            assert_eq!(v.as_array(), Some(&arr[..]));
            assert_eq!(Value::Object(HashMap::new()).as_array(), None);
        }

        #[test]
        fn test_as_object() {
            let mut map = HashMap::new();
            map.insert("a".to_string(), Value::Int(1));
            let v = Value::Object(map.clone());
            assert_eq!(v.as_object(), Some(&map));
            assert_eq!(Value::Array(vec![]).as_object(), None);
        }
    }

    // ========================================================================
    // Story #561: Float Edge Case Tests
    // ========================================================================

    mod float_edge_case_tests {
        use super::*;

        // === NaN Tests ===

        #[test]
        fn test_nan_construction() {
            let v = Value::Float(f64::NAN);
            match v {
                Value::Float(f) => assert!(f.is_nan()),
                _ => panic!("Expected Float"),
            }
        }

        #[test]
        fn test_nan_is_nan() {
            let f = f64::NAN;
            assert!(f.is_nan());
        }

        // === Infinity Tests ===

        #[test]
        fn test_positive_infinity_construction() {
            let v = Value::Float(f64::INFINITY);
            match v {
                Value::Float(f) => {
                    assert!(f.is_infinite());
                    assert!(f.is_sign_positive());
                }
                _ => panic!("Expected Float"),
            }
        }

        #[test]
        fn test_negative_infinity_construction() {
            let v = Value::Float(f64::NEG_INFINITY);
            match v {
                Value::Float(f) => {
                    assert!(f.is_infinite());
                    assert!(f.is_sign_negative());
                }
                _ => panic!("Expected Float"),
            }
        }

        // === Negative Zero Tests ===

        #[test]
        fn test_negative_zero_construction() {
            let v = Value::Float(-0.0);
            match v {
                Value::Float(f) => {
                    assert_eq!(f, 0.0); // -0.0 == 0.0 per IEEE-754
                    assert!(f.is_sign_negative()); // But sign is preserved
                }
                _ => panic!("Expected Float"),
            }
        }

        #[test]
        fn test_negative_zero_sign_preserved() {
            let v = Value::Float(-0.0);
            match v {
                Value::Float(f) => {
                    // Bit pattern must be preserved
                    assert_eq!(f.to_bits(), (-0.0_f64).to_bits());
                }
                _ => panic!("Expected Float"),
            }
        }

        // === Extreme Values ===

        #[test]
        fn test_float_max_construction() {
            let v = Value::Float(f64::MAX);
            match v {
                Value::Float(f) => assert_eq!(f, f64::MAX),
                _ => panic!("Expected Float"),
            }
        }

        #[test]
        fn test_float_min_positive_construction() {
            let v = Value::Float(f64::MIN_POSITIVE);
            match v {
                Value::Float(f) => assert_eq!(f, f64::MIN_POSITIVE),
                _ => panic!("Expected Float"),
            }
        }

        #[test]
        fn test_float_subnormal_construction() {
            // Smallest subnormal
            let subnormal = f64::from_bits(1);
            assert!(subnormal.is_subnormal());

            let v = Value::Float(subnormal);
            match v {
                Value::Float(f) => {
                    assert!(f.is_subnormal());
                    assert_eq!(f.to_bits(), 1);
                }
                _ => panic!("Expected Float"),
            }
        }

        #[test]
        fn test_float_precision_preserved() {
            // This value cannot be exactly represented in f32
            let precise = 1.0000000000000002_f64;
            let v = Value::Float(precise);
            match v {
                Value::Float(f) => {
                    assert_eq!(f.to_bits(), precise.to_bits());
                }
                _ => panic!("Expected Float"),
            }
        }

        // === Float Helper Methods ===

        #[test]
        fn test_is_special_float_nan() {
            let v = Value::Float(f64::NAN);
            assert!(v.is_special_float());
        }

        #[test]
        fn test_is_special_float_infinity() {
            let v = Value::Float(f64::INFINITY);
            assert!(v.is_special_float());
        }

        #[test]
        fn test_is_special_float_neg_infinity() {
            let v = Value::Float(f64::NEG_INFINITY);
            assert!(v.is_special_float());
        }

        #[test]
        fn test_is_special_float_neg_zero() {
            let v = Value::Float(-0.0);
            assert!(v.is_special_float());
        }

        #[test]
        fn test_is_special_float_normal() {
            let v = Value::Float(1.5);
            assert!(!v.is_special_float());
        }

        #[test]
        fn test_is_special_float_positive_zero() {
            let v = Value::Float(0.0);
            assert!(!v.is_special_float());
        }

        // === SpecialFloatKind Tests ===

        #[test]
        fn test_special_float_kind_nan() {
            let v = Value::Float(f64::NAN);
            assert_eq!(v.special_float_kind(), Some(SpecialFloatKind::NaN));
        }

        #[test]
        fn test_special_float_kind_pos_inf() {
            let v = Value::Float(f64::INFINITY);
            assert_eq!(
                v.special_float_kind(),
                Some(SpecialFloatKind::PositiveInfinity)
            );
        }

        #[test]
        fn test_special_float_kind_neg_inf() {
            let v = Value::Float(f64::NEG_INFINITY);
            assert_eq!(
                v.special_float_kind(),
                Some(SpecialFloatKind::NegativeInfinity)
            );
        }

        #[test]
        fn test_special_float_kind_neg_zero() {
            let v = Value::Float(-0.0);
            assert_eq!(v.special_float_kind(), Some(SpecialFloatKind::NegativeZero));
        }

        #[test]
        fn test_special_float_kind_normal() {
            let v = Value::Float(1.5);
            assert_eq!(v.special_float_kind(), None);
        }

        // === Wire String Conversion ===

        #[test]
        fn test_wire_string_conversion() {
            assert_eq!(SpecialFloatKind::NaN.to_wire_string(), "NaN");
            assert_eq!(SpecialFloatKind::PositiveInfinity.to_wire_string(), "+Inf");
            assert_eq!(SpecialFloatKind::NegativeInfinity.to_wire_string(), "-Inf");
            assert_eq!(SpecialFloatKind::NegativeZero.to_wire_string(), "-0.0");
        }

        #[test]
        fn test_wire_string_parsing() {
            assert_eq!(
                SpecialFloatKind::from_wire_string("NaN"),
                Some(SpecialFloatKind::NaN)
            );
            assert_eq!(
                SpecialFloatKind::from_wire_string("+Inf"),
                Some(SpecialFloatKind::PositiveInfinity)
            );
            assert_eq!(
                SpecialFloatKind::from_wire_string("-Inf"),
                Some(SpecialFloatKind::NegativeInfinity)
            );
            assert_eq!(
                SpecialFloatKind::from_wire_string("-0.0"),
                Some(SpecialFloatKind::NegativeZero)
            );
            assert_eq!(SpecialFloatKind::from_wire_string("invalid"), None);
        }

        #[test]
        fn test_special_float_to_f64() {
            assert!(SpecialFloatKind::NaN.to_f64().is_nan());
            assert_eq!(SpecialFloatKind::PositiveInfinity.to_f64(), f64::INFINITY);
            assert_eq!(
                SpecialFloatKind::NegativeInfinity.to_f64(),
                f64::NEG_INFINITY
            );
            assert_eq!(
                SpecialFloatKind::NegativeZero.to_f64().to_bits(),
                (-0.0_f64).to_bits()
            );
        }
    }

    // ========================================================================
    // Story #562: Value Equality Tests
    // ========================================================================

    mod equality_tests {
        use super::*;

        // === Same Type Equality ===

        #[test]
        fn test_null_equals_null() {
            assert_eq!(Value::Null, Value::Null);
        }

        #[test]
        fn test_bool_true_equals_true() {
            assert_eq!(Value::Bool(true), Value::Bool(true));
        }

        #[test]
        fn test_bool_false_equals_false() {
            assert_eq!(Value::Bool(false), Value::Bool(false));
        }

        #[test]
        fn test_bool_true_not_equals_false() {
            assert_ne!(Value::Bool(true), Value::Bool(false));
        }

        #[test]
        fn test_int_equals_same_int() {
            assert_eq!(Value::Int(42), Value::Int(42));
        }

        #[test]
        fn test_int_not_equals_different_int() {
            assert_ne!(Value::Int(42), Value::Int(43));
        }

        #[test]
        fn test_float_equals_same_float() {
            assert_eq!(Value::Float(3.14), Value::Float(3.14));
        }

        #[test]
        fn test_string_equals_same_string() {
            assert_eq!(
                Value::String("hello".to_string()),
                Value::String("hello".to_string())
            );
        }

        #[test]
        fn test_string_not_equals_different_string() {
            assert_ne!(
                Value::String("hello".to_string()),
                Value::String("world".to_string())
            );
        }

        #[test]
        fn test_bytes_equals_same_bytes() {
            assert_eq!(Value::Bytes(vec![1, 2, 3]), Value::Bytes(vec![1, 2, 3]));
        }

        #[test]
        fn test_array_equals_same_elements() {
            assert_eq!(
                Value::Array(vec![Value::Int(1), Value::Int(2)]),
                Value::Array(vec![Value::Int(1), Value::Int(2)])
            );
        }

        #[test]
        fn test_array_not_equals_different_order() {
            assert_ne!(
                Value::Array(vec![Value::Int(1), Value::Int(2)]),
                Value::Array(vec![Value::Int(2), Value::Int(1)])
            );
        }

        #[test]
        fn test_object_equals_same_entries() {
            let mut map1 = HashMap::new();
            map1.insert("a".to_string(), Value::Int(1));

            let mut map2 = HashMap::new();
            map2.insert("a".to_string(), Value::Int(1));

            assert_eq!(Value::Object(map1), Value::Object(map2));
        }

        #[test]
        fn test_object_equals_regardless_of_insertion_order() {
            let mut map1 = HashMap::new();
            map1.insert("a".to_string(), Value::Int(1));
            map1.insert("b".to_string(), Value::Int(2));

            let mut map2 = HashMap::new();
            map2.insert("b".to_string(), Value::Int(2));
            map2.insert("a".to_string(), Value::Int(1));

            assert_eq!(Value::Object(map1), Value::Object(map2));
        }

        // === IEEE-754 Float Equality ===

        #[test]
        fn test_nan_not_equals_nan() {
            // CRITICAL: NaN != NaN per IEEE-754
            assert_ne!(Value::Float(f64::NAN), Value::Float(f64::NAN));
        }

        #[test]
        fn test_different_nan_payloads_not_equal() {
            let nan1 = f64::from_bits(0x7ff8000000000001);
            let nan2 = f64::from_bits(0x7ff8000000000002);
            assert!(nan1.is_nan() && nan2.is_nan());
            assert_ne!(Value::Float(nan1), Value::Float(nan2));
        }

        #[test]
        fn test_positive_infinity_equals_positive_infinity() {
            assert_eq!(Value::Float(f64::INFINITY), Value::Float(f64::INFINITY));
        }

        #[test]
        fn test_negative_infinity_equals_negative_infinity() {
            assert_eq!(
                Value::Float(f64::NEG_INFINITY),
                Value::Float(f64::NEG_INFINITY)
            );
        }

        #[test]
        fn test_positive_infinity_not_equals_negative_infinity() {
            assert_ne!(Value::Float(f64::INFINITY), Value::Float(f64::NEG_INFINITY));
        }

        #[test]
        fn test_negative_zero_equals_positive_zero() {
            // CRITICAL: -0.0 == 0.0 per IEEE-754
            assert_eq!(Value::Float(-0.0), Value::Float(0.0));
        }

        // === Cross-Type Inequality (NO COERCION) ===

        #[test]
        fn test_null_not_equals_bool() {
            assert_ne!(Value::Null, Value::Bool(false));
        }

        #[test]
        fn test_null_not_equals_int_zero() {
            assert_ne!(Value::Null, Value::Int(0));
        }

        #[test]
        fn test_null_not_equals_empty_string() {
            assert_ne!(Value::Null, Value::String(String::new()));
        }

        #[test]
        fn test_int_one_not_equals_float_one() {
            // CRITICAL: No type coercion - Int(1) != Float(1.0)
            assert_ne!(Value::Int(1), Value::Float(1.0));
        }

        #[test]
        fn test_int_zero_not_equals_float_zero() {
            // CRITICAL: No type coercion
            assert_ne!(Value::Int(0), Value::Float(0.0));
        }

        #[test]
        fn test_bool_true_not_equals_int_one() {
            // CRITICAL: No type coercion
            assert_ne!(Value::Bool(true), Value::Int(1));
        }

        #[test]
        fn test_bool_false_not_equals_int_zero() {
            // CRITICAL: No type coercion
            assert_ne!(Value::Bool(false), Value::Int(0));
        }

        #[test]
        fn test_string_not_equals_bytes() {
            // CRITICAL: String("abc") != Bytes([97, 98, 99])
            assert_ne!(
                Value::String("abc".to_string()),
                Value::Bytes(vec![97, 98, 99])
            );
        }

        #[test]
        fn test_empty_array_not_equals_null() {
            assert_ne!(Value::Array(vec![]), Value::Null);
        }

        #[test]
        fn test_empty_object_not_equals_null() {
            assert_ne!(Value::Object(HashMap::new()), Value::Null);
        }

        #[test]
        fn test_string_number_not_equals_int() {
            // "123" != Int(123)
            assert_ne!(Value::String("123".to_string()), Value::Int(123));
        }
    }

    // ========================================================================
    // Story #563: No Type Coercion Tests (Contract Tests)
    // ========================================================================

    mod no_coercion_tests {
        use super::*;

        /// These tests verify the NO TYPE COERCION rule.
        /// If any test fails, the implementation is WRONG.
        /// DO NOT modify these tests - fix the implementation.

        #[test]
        fn nc_001_int_one_not_float_one() {
            assert_ne!(Value::Int(1), Value::Float(1.0));
        }

        #[test]
        fn nc_002_int_zero_not_float_zero() {
            assert_ne!(Value::Int(0), Value::Float(0.0));
        }

        #[test]
        fn nc_003_int_max_not_float() {
            assert_ne!(Value::Int(i64::MAX), Value::Float(i64::MAX as f64));
        }

        #[test]
        fn nc_004_string_not_bytes() {
            // Even when the bytes are the UTF-8 encoding of the string
            let s = "abc";
            let b = s.as_bytes().to_vec();
            assert_ne!(Value::String(s.to_string()), Value::Bytes(b));
        }

        #[test]
        fn nc_005_null_not_empty_string() {
            assert_ne!(Value::Null, Value::String(String::new()));
        }

        #[test]
        fn nc_006_null_not_zero() {
            assert_ne!(Value::Null, Value::Int(0));
        }

        #[test]
        fn nc_007_null_not_false() {
            assert_ne!(Value::Null, Value::Bool(false));
        }

        #[test]
        fn nc_008_empty_array_not_null() {
            assert_ne!(Value::Array(vec![]), Value::Null);
        }

        #[test]
        fn nc_009_empty_object_not_null() {
            assert_ne!(Value::Object(HashMap::new()), Value::Null);
        }

        #[test]
        fn nc_010_bool_true_not_int_one() {
            assert_ne!(Value::Bool(true), Value::Int(1));
        }

        #[test]
        fn nc_011_bool_false_not_int_zero() {
            assert_ne!(Value::Bool(false), Value::Int(0));
        }

        #[test]
        fn nc_012_string_number_not_int() {
            assert_ne!(Value::String("123".to_string()), Value::Int(123));
        }

        #[test]
        fn nc_013_no_implicit_string_to_bytes() {
            // Cannot compare String to Bytes - they are different types
            let s = Value::String("test".to_string());
            let b = Value::Bytes(b"test".to_vec());
            assert_ne!(s, b);
        }

        #[test]
        fn nc_014_no_implicit_int_promotion() {
            // Int should never be promoted to Float
            let i = Value::Int(42);
            let f = Value::Float(42.0);
            assert_ne!(i, f);

            // Type is preserved
            assert!(matches!(i, Value::Int(42)));
            assert!(matches!(f, Value::Float(_)));
        }
    }

    // ========================================================================
    // Hash Tests
    // ========================================================================

    mod hash_tests {
        use super::*;
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        fn hash_value(v: &Value) -> u64 {
            let mut hasher = DefaultHasher::new();
            v.hash(&mut hasher);
            hasher.finish()
        }

        #[test]
        fn test_equal_values_have_same_hash() {
            let v1 = Value::Int(42);
            let v2 = Value::Int(42);
            assert_eq!(v1, v2);
            assert_eq!(hash_value(&v1), hash_value(&v2));
        }

        #[test]
        fn test_different_values_usually_different_hash() {
            let v1 = Value::Int(42);
            let v2 = Value::Int(43);
            assert_ne!(v1, v2);
            // Note: Hashes can collide, but these should differ
            assert_ne!(hash_value(&v1), hash_value(&v2));
        }

        #[test]
        fn test_different_types_different_hash() {
            let v1 = Value::Int(1);
            let v2 = Value::Float(1.0);
            assert_ne!(v1, v2);
            assert_ne!(hash_value(&v1), hash_value(&v2));
        }

        #[test]
        fn test_negative_zero_positive_zero_same_hash() {
            // Since -0.0 == 0.0 per IEEE-754, they must have the same hash
            let v1 = Value::Float(-0.0);
            let v2 = Value::Float(0.0);
            assert_eq!(v1, v2);
            assert_eq!(hash_value(&v1), hash_value(&v2));
        }

        #[test]
        fn test_object_hash_order_independent() {
            let mut map1 = HashMap::new();
            map1.insert("a".to_string(), Value::Int(1));
            map1.insert("b".to_string(), Value::Int(2));

            let mut map2 = HashMap::new();
            map2.insert("b".to_string(), Value::Int(2));
            map2.insert("a".to_string(), Value::Int(1));

            let v1 = Value::Object(map1);
            let v2 = Value::Object(map2);
            assert_eq!(v1, v2);
            assert_eq!(hash_value(&v1), hash_value(&v2));
        }
    }

    // ========================================================================
    // Serialization Tests
    // ========================================================================

    mod serialization_tests {
        use super::*;

        #[test]
        fn test_value_serialization_all_variants() {
            let test_values = vec![
                Value::Null,
                Value::Bool(true),
                Value::Int(42),
                Value::Float(3.14),
                Value::String("test".to_string()),
                Value::Bytes(vec![1, 2, 3]),
                Value::Array(vec![Value::Int(1), Value::String("a".to_string())]),
            ];

            for value in test_values {
                let serialized = serde_json::to_string(&value).unwrap();
                let deserialized: Value = serde_json::from_str(&serialized).unwrap();
                assert_eq!(value, deserialized);
            }
        }

        #[test]
        fn test_object_serialization() {
            let mut map = HashMap::new();
            map.insert("test".to_string(), Value::Int(123));
            let value = Value::Object(map);

            let serialized = serde_json::to_string(&value).unwrap();
            let deserialized: Value = serde_json::from_str(&serialized).unwrap();
            assert_eq!(value, deserialized);
        }
    }

    // ========================================================================
    // Integration Tests
    // ========================================================================

    mod integration_tests {
        use super::*;

        #[test]
        fn test_value_type_completeness() {
            // Ensure exactly 8 variants
            let values = vec![
                Value::Null,
                Value::Bool(true),
                Value::Int(0),
                Value::Float(0.0),
                Value::String(String::new()),
                Value::Bytes(vec![]),
                Value::Array(vec![]),
                Value::Object(HashMap::new()),
            ];

            // Each variant has a distinct type_name
            let type_names: std::collections::HashSet<_> =
                values.iter().map(|v| v.type_name()).collect();
            assert_eq!(type_names.len(), 8);
        }

        #[test]
        fn test_complex_nested_value() {
            let v = Value::Object({
                let mut m = HashMap::new();
                m.insert(
                    "array".to_string(),
                    Value::Array(vec![
                        Value::Int(1),
                        Value::Float(2.5),
                        Value::String("three".to_string()),
                    ]),
                );
                m.insert(
                    "nested".to_string(),
                    Value::Object({
                        let mut inner = HashMap::new();
                        inner.insert("key".to_string(), Value::Bytes(vec![1, 2, 3]));
                        inner
                    }),
                );
                m
            });

            let v2 = v.clone();
            assert_eq!(v, v2);
        }
    }
}
