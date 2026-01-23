# Epic 80: Value Model & Wire Encoding

**Goal**: Freeze the canonical value model with all eight types, version types, Versioned<T> structure, and wire encoding

**Dependencies**: M10 complete

---

## Scope

- Value enum with 8 canonical types
- Value equality semantics (structural, IEEE-754 floats)
- Value size limits and validation
- Version tagged union (Txn/Sequence/Counter)
- Versioned<T> structure finalization
- RunId format (UUID + "default" literal)
- JSON wire encoding with special wrappers ($bytes, $f64, $absent)

---

## User Stories

| Story | Description | Priority |
|-------|-------------|----------|
| #550 | Value Enum Implementation (8 types) | FOUNDATION |
| #551 | Value Equality Semantics (structural, IEEE-754 floats) | FOUNDATION |
| #552 | Value Size Limits and Validation | CRITICAL |
| #553 | Version Tagged Union (Txn/Sequence/Counter) | CRITICAL |
| #554 | Versioned<T> Structure Finalization | CRITICAL |
| #555 | RunId Format (UUID + "default" literal) | CRITICAL |
| #556 | JSON Wire Encoding (basic types) | CRITICAL |
| #557 | $bytes Wrapper (base64 encoding) | CRITICAL |
| #558 | $f64 Wrapper (NaN, ±Inf, -0.0) | CRITICAL |
| #559 | $absent Wrapper (for CAS) | HIGH |

---

## Story #550: Value Enum Implementation

**File**: `crates/core/src/value/types.rs` (NEW)

**Deliverable**: Canonical Value enum with exactly 8 variants

### Design

The Value enum is the single public value model for Strata. All data flowing through the API uses this type.

```rust
/// Canonical Strata value type
///
/// This is the ONLY public value model. All surfaces (wire, CLI, SDK) use this.
#[derive(Debug, Clone)]
pub enum Value {
    /// JSON null
    Null,

    /// Boolean value
    Bool(bool),

    /// 64-bit signed integer
    Int(i64),

    /// IEEE-754 double-precision float
    /// Preserves NaN, +Inf, -Inf, -0.0
    Float(f64),

    /// UTF-8 string
    String(String),

    /// Binary data (distinct from String)
    Bytes(Vec<u8>),

    /// Ordered array of values
    Array(Vec<Value>),

    /// String-keyed object (key order not preserved)
    Object(HashMap<String, Value>),
}
```

### Implementation

```rust
use std::collections::HashMap;

impl Value {
    /// Check if value is null
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Get as bool if Bool variant
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Get as i64 if Int variant
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// Get as f64 if Float variant
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Get as &str if String variant
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// Get as &[u8] if Bytes variant
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Bytes(b) => Some(b),
            _ => None,
        }
    }

    /// Get as &[Value] if Array variant
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Get as &HashMap if Object variant
    pub fn as_object(&self) -> Option<&HashMap<String, Value>> {
        match self {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Get type name for error messages
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
}

// Convenience From implementations
impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Value::Int(v as i64)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(v)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::String(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::String(v.to_string())
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Value::Bytes(v)
    }
}

impl From<Vec<Value>> for Value {
    fn from(v: Vec<Value>) -> Self {
        Value::Array(v)
    }
}

impl From<HashMap<String, Value>> for Value {
    fn from(v: HashMap<String, Value>) -> Self {
        Value::Object(v)
    }
}
```

### Acceptance Criteria

- [ ] `Value` enum with exactly 8 variants: Null, Bool, Int(i64), Float(f64), String, Bytes, Array, Object
- [ ] Type accessor methods (as_bool, as_int, as_float, as_str, as_bytes, as_array, as_object)
- [ ] `type_name()` for error messages
- [ ] From implementations for common types
- [ ] No implicit type coercion between variants

---

## Story #551: Value Equality Semantics

**File**: `crates/core/src/value/equality.rs` (NEW)

**Deliverable**: Structural equality with IEEE-754 float semantics

### Design

Equality follows these rules:
- Structural equality only (no total ordering)
- No implicit type coercions (`Int(1) != Float(1.0)`)
- IEEE-754 float equality: `NaN != NaN`, `-0.0 == 0.0`
- Bytes are not strings
- Object key order is irrelevant

### Implementation

```rust
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => {
                // IEEE-754 equality: NaN != NaN, -0.0 == 0.0
                a == b
            }
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Bytes(a), Value::Bytes(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x == y)
            }
            (Value::Object(a), Value::Object(b)) => {
                // Same keys and recursive value equality
                a.len() == b.len()
                    && a.iter().all(|(k, v)| b.get(k).map_or(false, |bv| v == bv))
            }
            // Different types are never equal
            _ => false,
        }
    }
}

// Note: We intentionally do NOT implement Eq because Float(f64) is not reflexive (NaN != NaN)

impl Value {
    /// Check if two values are structurally equal for CAS operations
    ///
    /// This is the same as PartialEq but explicit for documentation.
    pub fn cas_equals(&self, other: &Value) -> bool {
        self == other
    }

    /// Check if value contains NaN (for debugging/testing)
    pub fn contains_nan(&self) -> bool {
        match self {
            Value::Float(f) => f.is_nan(),
            Value::Array(arr) => arr.iter().any(|v| v.contains_nan()),
            Value::Object(obj) => obj.values().any(|v| v.contains_nan()),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_int_float_not_equal() {
        // Critical invariant: Int(1) != Float(1.0)
        assert_ne!(Value::Int(1), Value::Float(1.0));
    }

    #[test]
    fn test_nan_not_equal_to_itself() {
        let nan1 = Value::Float(f64::NAN);
        let nan2 = Value::Float(f64::NAN);
        assert_ne!(nan1, nan2);
    }

    #[test]
    fn test_negative_zero_equals_positive_zero() {
        let neg_zero = Value::Float(-0.0);
        let pos_zero = Value::Float(0.0);
        assert_eq!(neg_zero, pos_zero);
    }

    #[test]
    fn test_bytes_not_equal_to_string() {
        let bytes = Value::Bytes(vec![72, 101, 108, 108, 111]); // "Hello"
        let string = Value::String("Hello".to_string());
        assert_ne!(bytes, string);
    }

    #[test]
    fn test_object_key_order_irrelevant() {
        let mut obj1 = HashMap::new();
        obj1.insert("a".to_string(), Value::Int(1));
        obj1.insert("b".to_string(), Value::Int(2));

        let mut obj2 = HashMap::new();
        obj2.insert("b".to_string(), Value::Int(2));
        obj2.insert("a".to_string(), Value::Int(1));

        assert_eq!(Value::Object(obj1), Value::Object(obj2));
    }
}
```

### Acceptance Criteria

- [ ] `PartialEq` implementation for Value
- [ ] `Int(1) != Float(1.0)` - no numeric widening
- [ ] `NaN != NaN` - IEEE-754 equality
- [ ] `-0.0 == 0.0` - IEEE-754 equality
- [ ] `Bytes` and `String` are never equal
- [ ] Object key order is irrelevant for equality
- [ ] Recursive equality for Array and Object
- [ ] `cas_equals()` method for explicit CAS comparison
- [ ] No `Eq` trait (floats prevent reflexivity)

---

## Story #552: Value Size Limits and Validation

**File**: `crates/core/src/value/limits.rs` (NEW)

**Deliverable**: Configurable size limits with validation

### Implementation

```rust
/// Value size limits (configurable at DB open)
#[derive(Debug, Clone)]
pub struct ValueLimits {
    /// Maximum key length in UTF-8 bytes (default: 1024)
    pub max_key_bytes: usize,

    /// Maximum string value size (default: 16 MiB)
    pub max_string_bytes: usize,

    /// Maximum bytes value size (default: 16 MiB)
    pub max_bytes_len: usize,

    /// Maximum encoded value size (default: 32 MiB)
    pub max_value_bytes_encoded: usize,

    /// Maximum array elements (default: 1,000,000)
    pub max_array_len: usize,

    /// Maximum object entries (default: 1,000,000)
    pub max_object_entries: usize,

    /// Maximum nesting depth (default: 128)
    pub max_nesting_depth: usize,

    /// Maximum vector dimensions (default: 8192)
    pub max_vector_dim: usize,
}

impl Default for ValueLimits {
    fn default() -> Self {
        ValueLimits {
            max_key_bytes: 1024,
            max_string_bytes: 16 * 1024 * 1024,      // 16 MiB
            max_bytes_len: 16 * 1024 * 1024,         // 16 MiB
            max_value_bytes_encoded: 32 * 1024 * 1024, // 32 MiB
            max_array_len: 1_000_000,
            max_object_entries: 1_000_000,
            max_nesting_depth: 128,
            max_vector_dim: 8192,
        }
    }
}

/// Constraint violation reason codes
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

impl ValueLimits {
    /// Validate a value against limits
    pub fn validate_value(&self, value: &Value) -> Result<(), ConstraintReason> {
        self.validate_value_depth(value, 0)
    }

    fn validate_value_depth(&self, value: &Value, depth: usize) -> Result<(), ConstraintReason> {
        if depth > self.max_nesting_depth {
            return Err(ConstraintReason::NestingTooDeep);
        }

        match value {
            Value::String(s) => {
                if s.len() > self.max_string_bytes {
                    return Err(ConstraintReason::ValueTooLarge);
                }
            }
            Value::Bytes(b) => {
                if b.len() > self.max_bytes_len {
                    return Err(ConstraintReason::ValueTooLarge);
                }
            }
            Value::Array(arr) => {
                if arr.len() > self.max_array_len {
                    return Err(ConstraintReason::ValueTooLarge);
                }
                for item in arr {
                    self.validate_value_depth(item, depth + 1)?;
                }
            }
            Value::Object(obj) => {
                if obj.len() > self.max_object_entries {
                    return Err(ConstraintReason::ValueTooLarge);
                }
                for (key, val) in obj {
                    if key.len() > self.max_key_bytes {
                        return Err(ConstraintReason::KeyTooLong);
                    }
                    self.validate_value_depth(val, depth + 1)?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Validate a key
    pub fn validate_key(&self, key: &str) -> Result<(), KeyError> {
        if key.is_empty() {
            return Err(KeyError::EmptyKey);
        }
        if key.len() > self.max_key_bytes {
            return Err(KeyError::TooLong);
        }
        if key.contains('\0') {
            return Err(KeyError::ContainsNul);
        }
        if key.starts_with("_strata/") {
            return Err(KeyError::ReservedPrefix);
        }
        Ok(())
    }
}

/// Key validation errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyError {
    EmptyKey,
    TooLong,
    ContainsNul,
    ReservedPrefix,
    InvalidUtf8,
}
```

### Acceptance Criteria

- [ ] `ValueLimits` with configurable defaults
- [ ] `validate_value()` checks all limits recursively
- [ ] `validate_key()` checks key constraints
- [ ] `ConstraintReason` enum with all reason codes
- [ ] Size limits: 1024 bytes keys, 16 MiB strings/bytes, 32 MiB encoded
- [ ] Container limits: 1M array elements, 1M object entries
- [ ] Nesting depth limit: 128
- [ ] Vector dimension limit: 8192
- [ ] Reserved prefix `_strata/` blocked

---

## Story #553: Version Tagged Union

**File**: `crates/core/src/contract/version.rs`

**Deliverable**: Version enum with Txn/Sequence/Counter variants

### Implementation

```rust
/// Version tagged union
///
/// Different primitives use different version semantics:
/// - KV, JSON, Vector, Run: Transaction-based (Txn)
/// - Events: Append-only sequence (Sequence)
/// - StateCell: Per-entity counter (Counter)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Version {
    /// Transaction-based version (KV, JSON, Vector, Run)
    Txn(u64),

    /// Sequence number (Events - append-only)
    Sequence(u64),

    /// Per-entity counter (StateCell)
    Counter(u64),
}

impl Version {
    /// Get the numeric value regardless of variant
    pub fn value(&self) -> u64 {
        match self {
            Version::Txn(v) | Version::Sequence(v) | Version::Counter(v) => *v,
        }
    }

    /// Get the type name for wire encoding
    pub fn type_name(&self) -> &'static str {
        match self {
            Version::Txn(_) => "txn",
            Version::Sequence(_) => "sequence",
            Version::Counter(_) => "counter",
        }
    }

    /// Check if two versions are comparable (same variant)
    pub fn is_comparable_with(&self, other: &Version) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    /// Compare versions (only valid for same variant)
    ///
    /// Returns None if versions are different types
    pub fn compare(&self, other: &Version) -> Option<std::cmp::Ordering> {
        if !self.is_comparable_with(other) {
            return None;
        }
        Some(self.value().cmp(&other.value()))
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Version::Txn(v) => write!(f, "txn:{}", v),
            Version::Sequence(v) => write!(f, "seq:{}", v),
            Version::Counter(v) => write!(f, "ctr:{}", v),
        }
    }
}
```

### Acceptance Criteria

- [ ] `Version` enum with Txn(u64), Sequence(u64), Counter(u64) variants
- [ ] `value()` extracts numeric value
- [ ] `type_name()` returns wire encoding type
- [ ] `is_comparable_with()` checks same variant
- [ ] `compare()` returns None for different variants
- [ ] No comparison across version types (WrongType error at API layer)

---

## Story #554: Versioned<T> Structure Finalization

**File**: `crates/core/src/contract/versioned.rs`

**Deliverable**: Frozen Versioned<T> structure with microsecond timestamps

### Implementation

```rust
/// Versioned wrapper for values
///
/// This structure is FROZEN and cannot change without major version bump.
#[derive(Debug, Clone)]
pub struct Versioned<T> {
    /// The value
    pub value: T,

    /// Version (tagged union)
    pub version: Version,

    /// Timestamp in microseconds since Unix epoch
    pub timestamp: u64,
}

impl<T> Versioned<T> {
    /// Create a new versioned value
    pub fn new(value: T, version: Version, timestamp: u64) -> Self {
        Versioned {
            value,
            version,
            timestamp,
        }
    }

    /// Map the inner value
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Versioned<U> {
        Versioned {
            value: f(self.value),
            version: self.version,
            timestamp: self.timestamp,
        }
    }

    /// Get reference to inner value
    pub fn as_ref(&self) -> Versioned<&T> {
        Versioned {
            value: &self.value,
            version: self.version,
            timestamp: self.timestamp,
        }
    }
}

impl<T: Clone> Versioned<&T> {
    /// Clone the inner value
    pub fn cloned(self) -> Versioned<T> {
        Versioned {
            value: self.value.clone(),
            version: self.version,
            timestamp: self.timestamp,
        }
    }
}

/// Get current timestamp in microseconds since Unix epoch
pub fn now_micros() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_micros() as u64
}
```

### Acceptance Criteria

- [ ] `Versioned<T>` with value, version, timestamp fields
- [ ] Timestamp is microseconds since Unix epoch (u64)
- [ ] `map()` transforms inner value preserving version/timestamp
- [ ] `as_ref()` provides reference without cloning
- [ ] `now_micros()` utility for timestamp generation
- [ ] Structure is frozen (documented, no changes without major version)

---

## Story #555: RunId Format

**File**: `crates/core/src/contract/run_name.rs`

**Deliverable**: RunId type with UUID format and "default" literal

### Implementation

```rust
use uuid::Uuid;

/// Run identifier
///
/// Can be either:
/// - A UUID in lowercase hyphenated format (e.g., f47ac10b-58cc-4372-a567-0e02b2c3d479)
/// - The literal string "default" for the default run
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RunId(String);

/// The canonical default run name
pub const DEFAULT_RUN: &str = "default";

impl RunId {
    /// Create the default run ID
    pub fn default_run() -> Self {
        RunId(DEFAULT_RUN.to_string())
    }

    /// Generate a new UUID-based run ID
    pub fn generate() -> Self {
        RunId(Uuid::new_v4().to_string())
    }

    /// Parse a run ID from a string
    ///
    /// Accepts:
    /// - "default" (literal)
    /// - Valid UUID in lowercase hyphenated format
    pub fn parse(s: &str) -> Result<Self, RunIdError> {
        if s == DEFAULT_RUN {
            return Ok(RunId::default_run());
        }

        // Validate UUID format
        Uuid::parse_str(s).map_err(|_| RunIdError::InvalidFormat)?;

        // Ensure lowercase hyphenated
        let normalized = s.to_lowercase();
        if normalized != s {
            return Err(RunIdError::NotLowercase);
        }

        Ok(RunId(s.to_string()))
    }

    /// Check if this is the default run
    pub fn is_default(&self) -> bool {
        self.0 == DEFAULT_RUN
    }

    /// Get the string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<RunId> for String {
    fn from(id: RunId) -> String {
        id.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunIdError {
    InvalidFormat,
    NotLowercase,
}

impl std::fmt::Display for RunIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunIdError::InvalidFormat => write!(f, "Invalid run ID format"),
            RunIdError::NotLowercase => write!(f, "Run ID must be lowercase"),
        }
    }
}

impl std::error::Error for RunIdError {}
```

### Acceptance Criteria

- [ ] `RunId` type encapsulating run identifier
- [ ] `default_run()` returns literal "default"
- [ ] `generate()` creates new UUID
- [ ] `parse()` validates format (UUID or "default")
- [ ] `is_default()` checks for default run
- [ ] UUID must be lowercase hyphenated
- [ ] `DEFAULT_RUN` constant exported

---

## Story #556: JSON Wire Encoding (Basic Types)

**File**: `crates/wire/src/json/value.rs` (NEW)

**Deliverable**: JSON encoding for all 8 value types

### Implementation

```rust
use crate::value::Value;
use serde_json::{json, Value as JsonValue};

/// Encode a Value to JSON
pub fn encode_value(value: &Value) -> JsonValue {
    match value {
        Value::Null => JsonValue::Null,
        Value::Bool(b) => JsonValue::Bool(*b),
        Value::Int(i) => json!(*i),
        Value::Float(f) => encode_float(*f),
        Value::String(s) => JsonValue::String(s.clone()),
        Value::Bytes(b) => encode_bytes(b),
        Value::Array(arr) => {
            JsonValue::Array(arr.iter().map(encode_value).collect())
        }
        Value::Object(obj) => {
            let map: serde_json::Map<String, JsonValue> = obj
                .iter()
                .map(|(k, v)| (k.clone(), encode_value(v)))
                .collect();
            JsonValue::Object(map)
        }
    }
}

/// Decode JSON to a Value
pub fn decode_value(json: &JsonValue) -> Result<Value, DecodeError> {
    match json {
        JsonValue::Null => Ok(Value::Null),
        JsonValue::Bool(b) => Ok(Value::Bool(*b)),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Int(i))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Float(f))
            } else {
                Err(DecodeError::InvalidNumber)
            }
        }
        JsonValue::String(s) => Ok(Value::String(s.clone())),
        JsonValue::Array(arr) => {
            let values: Result<Vec<_>, _> = arr.iter().map(decode_value).collect();
            Ok(Value::Array(values?))
        }
        JsonValue::Object(obj) => {
            // Check for special wrappers
            if let Some(result) = try_decode_wrapper(obj)? {
                return Ok(result);
            }

            // Regular object
            let mut map = HashMap::new();
            for (k, v) in obj {
                map.insert(k.clone(), decode_value(v)?);
            }
            Ok(Value::Object(map))
        }
    }
}

/// Try to decode a special wrapper ($bytes, $f64, $absent)
fn try_decode_wrapper(obj: &serde_json::Map<String, JsonValue>) -> Result<Option<Value>, DecodeError> {
    // Check for $bytes wrapper
    if obj.len() == 1 {
        if let Some(JsonValue::String(b64)) = obj.get("$bytes") {
            let bytes = base64::decode(b64).map_err(|_| DecodeError::InvalidBase64)?;
            return Ok(Some(Value::Bytes(bytes)));
        }

        if let Some(JsonValue::String(s)) = obj.get("$f64") {
            let f = match s.as_str() {
                "NaN" => f64::NAN,
                "+Inf" => f64::INFINITY,
                "-Inf" => f64::NEG_INFINITY,
                "-0.0" => -0.0_f64,
                _ => return Err(DecodeError::InvalidF64Wrapper),
            };
            return Ok(Some(Value::Float(f)));
        }

        if let Some(JsonValue::Bool(true)) = obj.get("$absent") {
            return Err(DecodeError::AbsentNotAllowedHere);
        }
    }

    Ok(None)
}

#[derive(Debug, Clone)]
pub enum DecodeError {
    InvalidNumber,
    InvalidBase64,
    InvalidF64Wrapper,
    AbsentNotAllowedHere,
}
```

### Acceptance Criteria

- [ ] `encode_value()` converts Value to JSON
- [ ] `decode_value()` converts JSON to Value
- [ ] Null → `null`
- [ ] Bool → `true`/`false`
- [ ] Int → JSON number
- [ ] Float (finite) → JSON number
- [ ] String → JSON string
- [ ] Array → JSON array
- [ ] Object → JSON object
- [ ] Wrapper detection for $bytes, $f64

---

## Story #557: $bytes Wrapper

**File**: `crates/wire/src/json/wrappers.rs` (NEW)

**Deliverable**: Base64 encoding for Bytes type

### Implementation

```rust
use base64::{Engine as _, engine::general_purpose::STANDARD};

/// Encode bytes as $bytes wrapper
pub fn encode_bytes(bytes: &[u8]) -> serde_json::Value {
    let b64 = STANDARD.encode(bytes);
    serde_json::json!({ "$bytes": b64 })
}

/// Decode $bytes wrapper
pub fn decode_bytes(b64: &str) -> Result<Vec<u8>, base64::DecodeError> {
    STANDARD.decode(b64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_roundtrip() {
        let original = b"Hello, World!";
        let encoded = encode_bytes(original);

        assert_eq!(
            encoded,
            serde_json::json!({ "$bytes": "SGVsbG8sIFdvcmxkIQ==" })
        );

        let decoded = decode_bytes("SGVsbG8sIFdvcmxkIQ==").unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_empty_bytes() {
        let encoded = encode_bytes(&[]);
        assert_eq!(encoded, serde_json::json!({ "$bytes": "" }));

        let decoded = decode_bytes("").unwrap();
        assert!(decoded.is_empty());
    }
}
```

### Acceptance Criteria

- [ ] `encode_bytes()` produces `{"$bytes": "<base64>"}`
- [ ] `decode_bytes()` parses base64 string
- [ ] Standard base64 encoding (RFC 4648)
- [ ] Round-trip preserves exact bytes
- [ ] Empty bytes encode to empty string

---

## Story #558: $f64 Wrapper

**File**: `crates/wire/src/json/wrappers.rs`

**Deliverable**: Special float value encoding (NaN, ±Inf, -0.0)

### Implementation

```rust
/// Encode float, using $f64 wrapper for special values
pub fn encode_float(f: f64) -> serde_json::Value {
    if f.is_nan() {
        serde_json::json!({ "$f64": "NaN" })
    } else if f.is_infinite() {
        if f.is_sign_positive() {
            serde_json::json!({ "$f64": "+Inf" })
        } else {
            serde_json::json!({ "$f64": "-Inf" })
        }
    } else if f == 0.0 && f.is_sign_negative() {
        serde_json::json!({ "$f64": "-0.0" })
    } else {
        // Regular finite float
        serde_json::json!(f)
    }
}

/// Decode $f64 wrapper
pub fn decode_f64_wrapper(s: &str) -> Result<f64, F64WrapperError> {
    match s {
        "NaN" => Ok(f64::NAN),
        "+Inf" => Ok(f64::INFINITY),
        "-Inf" => Ok(f64::NEG_INFINITY),
        "-0.0" => Ok(-0.0_f64),
        _ => Err(F64WrapperError::InvalidValue(s.to_string())),
    }
}

#[derive(Debug, Clone)]
pub struct F64WrapperError {
    InvalidValue(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nan_encoding() {
        let encoded = encode_float(f64::NAN);
        assert_eq!(encoded, serde_json::json!({ "$f64": "NaN" }));
    }

    #[test]
    fn test_positive_infinity() {
        let encoded = encode_float(f64::INFINITY);
        assert_eq!(encoded, serde_json::json!({ "$f64": "+Inf" }));
    }

    #[test]
    fn test_negative_infinity() {
        let encoded = encode_float(f64::NEG_INFINITY);
        assert_eq!(encoded, serde_json::json!({ "$f64": "-Inf" }));
    }

    #[test]
    fn test_negative_zero() {
        let encoded = encode_float(-0.0_f64);
        assert_eq!(encoded, serde_json::json!({ "$f64": "-0.0" }));
    }

    #[test]
    fn test_positive_zero_no_wrapper() {
        let encoded = encode_float(0.0_f64);
        assert_eq!(encoded, serde_json::json!(0.0));
    }

    #[test]
    fn test_regular_float_no_wrapper() {
        let encoded = encode_float(3.14159);
        assert_eq!(encoded, serde_json::json!(3.14159));
    }
}
```

### Acceptance Criteria

- [ ] `NaN` → `{"$f64": "NaN"}`
- [ ] `+Infinity` → `{"$f64": "+Inf"}`
- [ ] `-Infinity` → `{"$f64": "-Inf"}`
- [ ] `-0.0` → `{"$f64": "-0.0"}`
- [ ] Regular floats use plain JSON numbers
- [ ] `0.0` (positive zero) uses plain JSON number
- [ ] Decode correctly identifies all special values

---

## Story #559: $absent Wrapper

**File**: `crates/wire/src/json/wrappers.rs`

**Deliverable**: Absent value encoding for CAS operations

### Implementation

```rust
/// Absent marker for CAS operations
///
/// Used to distinguish "key is missing" from "value is null"
pub const ABSENT_MARKER: &str = "$absent";

/// Encode absent marker
pub fn encode_absent() -> serde_json::Value {
    serde_json::json!({ "$absent": true })
}

/// Check if JSON value is absent marker
pub fn is_absent(json: &serde_json::Value) -> bool {
    if let serde_json::Value::Object(obj) = json {
        obj.len() == 1 && obj.get(ABSENT_MARKER) == Some(&serde_json::Value::Bool(true))
    } else {
        false
    }
}

/// CAS expected value (can be absent, null, or a concrete value)
#[derive(Debug, Clone)]
pub enum CasExpected {
    /// Key should not exist
    Absent,
    /// Key should exist with this value (including null)
    Value(Value),
}

impl CasExpected {
    /// Encode to JSON
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            CasExpected::Absent => encode_absent(),
            CasExpected::Value(v) => encode_value(v),
        }
    }

    /// Decode from JSON
    pub fn from_json(json: &serde_json::Value) -> Result<Self, DecodeError> {
        if is_absent(json) {
            Ok(CasExpected::Absent)
        } else {
            Ok(CasExpected::Value(decode_value(json)?))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_absent_encoding() {
        let encoded = encode_absent();
        assert_eq!(encoded, serde_json::json!({ "$absent": true }));
        assert!(is_absent(&encoded));
    }

    #[test]
    fn test_null_is_not_absent() {
        let null = serde_json::Value::Null;
        assert!(!is_absent(&null));
    }

    #[test]
    fn test_cas_expected_absent() {
        let expected = CasExpected::Absent;
        let json = expected.to_json();
        assert!(is_absent(&json));

        let decoded = CasExpected::from_json(&json).unwrap();
        assert!(matches!(decoded, CasExpected::Absent));
    }

    #[test]
    fn test_cas_expected_null() {
        let expected = CasExpected::Value(Value::Null);
        let json = expected.to_json();
        assert!(!is_absent(&json));
        assert_eq!(json, serde_json::Value::Null);

        let decoded = CasExpected::from_json(&json).unwrap();
        assert!(matches!(decoded, CasExpected::Value(Value::Null)));
    }
}
```

### Acceptance Criteria

- [ ] `{"$absent": true}` represents missing key
- [ ] `null` represents Value::Null (key exists with null value)
- [ ] `is_absent()` detects absent marker
- [ ] `CasExpected` enum distinguishes Absent from Value
- [ ] Round-trip preserves absent vs null distinction
- [ ] CAS operations use CasExpected for expected parameter

---

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_type_exhaustiveness() {
        // Ensure all 8 types exist
        let values = vec![
            Value::Null,
            Value::Bool(true),
            Value::Int(42),
            Value::Float(3.14),
            Value::String("hello".into()),
            Value::Bytes(vec![1, 2, 3]),
            Value::Array(vec![]),
            Value::Object(HashMap::new()),
        ];

        assert_eq!(values.len(), 8);
    }

    #[test]
    fn test_no_type_coercion() {
        assert_ne!(Value::Int(1), Value::Float(1.0));
        assert_ne!(Value::String("1".into()), Value::Int(1));
        assert_ne!(Value::Bytes(vec![49]), Value::String("1".into()));
    }

    #[test]
    fn test_version_types_not_comparable() {
        let txn = Version::Txn(5);
        let seq = Version::Sequence(5);
        let ctr = Version::Counter(5);

        assert!(!txn.is_comparable_with(&seq));
        assert!(!txn.is_comparable_with(&ctr));
        assert!(!seq.is_comparable_with(&ctr));

        assert!(txn.compare(&seq).is_none());
    }

    #[test]
    fn test_full_wire_roundtrip() {
        let values = vec![
            Value::Null,
            Value::Bool(true),
            Value::Bool(false),
            Value::Int(i64::MIN),
            Value::Int(i64::MAX),
            Value::Float(f64::NAN),
            Value::Float(f64::INFINITY),
            Value::Float(f64::NEG_INFINITY),
            Value::Float(-0.0),
            Value::Float(3.14159),
            Value::String("hello".into()),
            Value::Bytes(vec![0, 255, 128]),
            Value::Array(vec![Value::Int(1), Value::Int(2)]),
            Value::Object({
                let mut m = HashMap::new();
                m.insert("key".into(), Value::String("value".into()));
                m
            }),
        ];

        for value in values {
            let json = encode_value(&value);
            let decoded = decode_value(&json).unwrap();

            // NaN != NaN, so special case
            if value.contains_nan() {
                assert!(decoded.contains_nan());
            } else {
                assert_eq!(value, decoded);
            }
        }
    }
}
```

---

## Files Modified/Created

| File | Action |
|------|--------|
| `crates/core/src/value/mod.rs` | CREATE - Value module entry |
| `crates/core/src/value/types.rs` | CREATE - Value enum |
| `crates/core/src/value/equality.rs` | CREATE - Equality implementation |
| `crates/core/src/value/limits.rs` | CREATE - Size limits |
| `crates/core/src/contract/version.rs` | MODIFY - Add tagged union |
| `crates/core/src/contract/versioned.rs` | MODIFY - Finalize structure |
| `crates/core/src/contract/run_name.rs` | MODIFY - Add RunId type |
| `crates/wire/src/lib.rs` | CREATE - Wire crate entry |
| `crates/wire/src/json/mod.rs` | CREATE - JSON module |
| `crates/wire/src/json/value.rs` | CREATE - Value encoding |
| `crates/wire/src/json/wrappers.rs` | CREATE - Special wrappers |
| `Cargo.toml` | MODIFY - Add wire crate |
