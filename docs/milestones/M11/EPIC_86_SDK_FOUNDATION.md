# Epic 86: SDK Foundation

**Goal**: Define SDK mappings and implement Rust SDK

**Dependencies**: M11a complete

---

## Scope

- SDK value mapping specification
- Rust SDK implementation
- Python SDK mapping definition
- JavaScript SDK mapping definition
- SDK conformance test harness

---

## User Stories

| Story | Description | Priority |
|-------|-------------|----------|
| #597 | SDK Value Mapping Specification | FOUNDATION |
| #598 | Rust SDK Implementation | CRITICAL |
| #599 | Python SDK Mapping Definition | HIGH |
| #600 | JavaScript SDK Mapping Definition | HIGH |
| #601 | SDK Conformance Test Harness | CRITICAL |

---

## Story #597: SDK Value Mapping Specification

**File**: `docs/architecture/SDK_MAPPING.md` (NEW)

**Deliverable**: Complete SDK value mapping specification

### Specification

All SDKs must preserve these properties:
1. **Numeric widths**: i64 and f64 must not be truncated
2. **Bytes vs String**: Must remain distinct types
3. **None vs Null**: Missing value differs from Value::Null
4. **Versioned shape**: Must preserve value, version, timestamp
5. **Structured errors**: Must include code, message, details

### Value Mapping Table

| Strata Type | Rust | Python | JavaScript |
|-------------|------|--------|------------|
| `Null` | `Value::Null` | `None` | `null` |
| `Bool` | `Value::Bool(bool)` | `bool` | `boolean` |
| `Int(i64)` | `Value::Int(i64)` | `int` | `number \| BigInt` |
| `Float(f64)` | `Value::Float(f64)` | `float` | `number` |
| `String` | `Value::String(String)` | `str` | `string` |
| `Bytes` | `Value::Bytes(Vec<u8>)` | `bytes` | `Uint8Array` |
| `Array` | `Value::Array(Vec<Value>)` | `list` | `Array<any>` |
| `Object` | `Value::Object(HashMap)` | `dict[str, Any]` | `Record<string, any>` |

### JavaScript Integer Handling

JavaScript cannot safely represent all i64 values. The SDK must:
- Use `number` for values within safe range (`Number.MIN_SAFE_INTEGER` to `Number.MAX_SAFE_INTEGER`)
- Use `BigInt` for values outside safe range
- Detect and handle this at serialization/deserialization boundaries

### Acceptance Criteria

- [ ] All mappings documented
- [ ] Numeric width requirements specified
- [ ] Bytes vs String distinction specified
- [ ] None vs Null distinction specified
- [ ] JavaScript BigInt handling specified

---

## Story #598: Rust SDK Implementation

**File**: `crates/sdk/src/lib.rs` (NEW)

**Deliverable**: Complete Rust SDK using native types

### Implementation

```rust
//! Strata Rust SDK
//!
//! Direct binding to the Strata engine using native Rust types.

use strata_core::{Value, Versioned, Version, StrataError};
use strata_api::{Facade, Substrate};

/// Strata database client
pub struct Strata {
    facade: Box<dyn Facade>,
    substrate: Box<dyn Substrate>,
}

impl Strata {
    /// Open a Strata database
    pub fn open(path: &str) -> Result<Self, StrataError> {
        let engine = strata_engine::Database::open(path)?;
        Ok(Strata {
            facade: Box::new(engine.facade()),
            substrate: Box::new(engine.substrate()),
        })
    }

    /// Open with custom configuration
    pub fn open_with_config(config: Config) -> Result<Self, StrataError> {
        let engine = strata_engine::Database::open_with_config(config)?;
        Ok(Strata {
            facade: Box::new(engine.facade()),
            substrate: Box::new(engine.substrate()),
        })
    }

    // ===== KV Operations =====

    /// Set a key-value pair
    pub fn set(&self, key: &str, value: Value) -> Result<(), StrataError> {
        self.facade.set(key, value)
    }

    /// Get a value by key
    pub fn get(&self, key: &str) -> Result<Option<Value>, StrataError> {
        self.facade.get(key)
    }

    /// Get a versioned value by key
    pub fn getv(&self, key: &str) -> Result<Option<Versioned<Value>>, StrataError> {
        self.facade.getv(key)
    }

    /// Get multiple values
    pub fn mget(&self, keys: &[&str]) -> Result<Vec<Option<Value>>, StrataError> {
        self.facade.mget(keys)
    }

    /// Set multiple key-value pairs atomically
    pub fn mset(&self, entries: &[(&str, Value)]) -> Result<(), StrataError> {
        self.facade.mset(entries)
    }

    /// Delete keys
    pub fn delete(&self, keys: &[&str]) -> Result<u64, StrataError> {
        self.facade.delete(keys)
    }

    /// Check if key exists
    pub fn exists(&self, key: &str) -> Result<bool, StrataError> {
        self.facade.exists(key)
    }

    /// Count existing keys
    pub fn exists_many(&self, keys: &[&str]) -> Result<u64, StrataError> {
        self.facade.exists_many(keys)
    }

    /// Atomic increment
    pub fn incr(&self, key: &str, delta: i64) -> Result<i64, StrataError> {
        self.facade.incr(key, delta)
    }

    // ===== JSON Operations =====

    /// Set JSON at path
    pub fn json_set(&self, key: &str, path: &str, value: Value) -> Result<(), StrataError> {
        self.facade.json_set(key, path, value)
    }

    /// Get JSON at path
    pub fn json_get(&self, key: &str, path: &str) -> Result<Option<Value>, StrataError> {
        self.facade.json_get(key, path)
    }

    /// Get versioned JSON at path
    pub fn json_getv(&self, key: &str, path: &str) -> Result<Option<Versioned<Value>>, StrataError> {
        self.facade.json_getv(key, path)
    }

    /// Delete JSON at path
    pub fn json_del(&self, key: &str, path: &str) -> Result<u64, StrataError> {
        self.facade.json_del(key, path)
    }

    /// Merge JSON at path
    pub fn json_merge(&self, key: &str, path: &str, value: Value) -> Result<(), StrataError> {
        self.facade.json_merge(key, path, value)
    }

    // ===== Event Operations =====

    /// Add event to stream
    pub fn xadd(&self, stream: &str, payload: Value) -> Result<Version, StrataError> {
        self.facade.xadd(stream, payload)
    }

    /// Read events from stream
    pub fn xrange(
        &self,
        stream: &str,
        start: Option<Version>,
        end: Option<Version>,
        limit: Option<u64>,
    ) -> Result<Vec<Versioned<Value>>, StrataError> {
        self.facade.xrange(stream, start, end, limit)
    }

    /// Count events in stream
    pub fn xlen(&self, stream: &str) -> Result<u64, StrataError> {
        self.facade.xlen(stream)
    }

    // ===== Vector Operations =====

    /// Set vector with metadata
    pub fn vset(&self, key: &str, vector: Vec<f32>, metadata: Value) -> Result<(), StrataError> {
        self.facade.vset(key, vector, metadata)
    }

    /// Get vector with metadata
    pub fn vget(&self, key: &str) -> Result<Option<Versioned<VectorEntry>>, StrataError> {
        self.facade.vget(key)
    }

    /// Delete vector
    pub fn vdel(&self, key: &str) -> Result<bool, StrataError> {
        self.facade.vdel(key)
    }

    // ===== State/CAS Operations =====

    /// Compare-and-swap
    pub fn cas_set(&self, key: &str, expected: Option<Value>, new: Value) -> Result<bool, StrataError> {
        self.facade.cas_set(key, expected, new)
    }

    /// Get from state store
    pub fn cas_get(&self, key: &str) -> Result<Option<Value>, StrataError> {
        self.facade.cas_get(key)
    }

    // ===== History Operations =====

    /// Get version history
    pub fn history(
        &self,
        key: &str,
        limit: Option<u64>,
        before: Option<Version>,
    ) -> Result<Vec<Versioned<Value>>, StrataError> {
        self.facade.history(key, limit, before)
    }

    /// Get value at specific version
    pub fn get_at(&self, key: &str, version: Version) -> Result<Value, StrataError> {
        self.facade.get_at(key, version)
    }

    /// Get latest version
    pub fn latest_version(&self, key: &str) -> Result<Option<Version>, StrataError> {
        self.facade.latest_version(key)
    }

    // ===== Run Operations =====

    /// List all runs
    pub fn runs(&self) -> Result<Vec<RunInfo>, StrataError> {
        self.facade.runs()
    }

    /// Scope to specific run
    pub fn use_run(&self, run_id: &str) -> Result<ScopedStrata, StrataError> {
        let scoped = self.facade.use_run(run_id)?;
        Ok(ScopedStrata { facade: scoped })
    }

    /// Get capabilities
    pub fn capabilities(&self) -> Capabilities {
        self.facade.capabilities()
    }

    // ===== Substrate Access =====

    /// Get substrate API for advanced operations
    pub fn substrate(&self) -> &dyn Substrate {
        self.substrate.as_ref()
    }
}

/// Strata scoped to a specific run
pub struct ScopedStrata {
    facade: Box<dyn Facade>,
}

// ScopedStrata has same methods as Strata, operating on the scoped run
```

### Acceptance Criteria

- [ ] All facade operations exposed
- [ ] Native `Value` enum used
- [ ] `Versioned<T>` preserved
- [ ] `substrate()` escape hatch available
- [ ] Error handling with `StrataError`

---

## Story #599: Python SDK Mapping Definition

**File**: `docs/architecture/SDK_MAPPING.md`

**Deliverable**: Python SDK mapping specification

### Mapping

```python
# Value mapping
Null    -> None
Bool    -> bool
Int     -> int  # Python int has arbitrary precision
Float   -> float
String  -> str
Bytes   -> bytes
Array   -> list
Object  -> dict[str, Any]

# Versioned wrapper
class Versioned(Generic[T]):
    value: T
    version: Version
    timestamp: int  # microseconds

# Version types
class Version:
    type: Literal["txn", "sequence", "counter"]
    value: int

# Error class
class StrataError(Exception):
    code: str       # e.g., "NotFound", "WrongType"
    message: str    # Human-readable message
    details: dict | None  # Structured details

# Usage example
db = Strata.open("./data")
db.set("x", 123)
value = db.get("x")           # -> 123
versioned = db.getv("x")      # -> Versioned(value=123, version=..., timestamp=...)
exists = db.exists("x")       # -> True
count = db.delete(["x"])      # -> 1

# JSON
db.json_set("doc", "$.a.b", 5)
value = db.json_get("doc", "$.a.b")  # -> 5

# Advanced
runs = db.runs()
scoped = db.use_run("my-run-id")
scoped.set("x", 456)
```

### Acceptance Criteria

- [ ] All value types mapped
- [ ] `Versioned` wrapper defined
- [ ] `Version` class defined
- [ ] `StrataError` exception defined
- [ ] Usage examples documented

---

## Story #600: JavaScript SDK Mapping Definition

**File**: `docs/architecture/SDK_MAPPING.md`

**Deliverable**: JavaScript SDK mapping specification

### Mapping

```typescript
// Value mapping
Null    -> null
Bool    -> boolean
Int     -> number | bigint  // BigInt for values outside safe range
Float   -> number
String  -> string
Bytes   -> Uint8Array
Array   -> Array<any>
Object  -> Record<string, any>

// Versioned wrapper
interface Versioned<T> {
    value: T;
    version: Version;
    timestamp: number;  // microseconds as number (safe for ~285,000 years)
}

// Version types
interface Version {
    type: "txn" | "sequence" | "counter";
    value: number | bigint;
}

// Error class
class StrataError extends Error {
    code: string;
    message: string;
    details?: Record<string, any>;
}

// Usage example
const db = await Strata.open("./data");
await db.set("x", 123);
const value = await db.get("x");           // -> 123
const versioned = await db.getv("x");      // -> { value: 123, version: {...}, timestamp: ... }
const exists = await db.exists("x");       // -> true
const count = await db.delete(["x"]);      // -> 1

// JSON
await db.jsonSet("doc", "$.a.b", 5);
const v = await db.jsonGet("doc", "$.a.b");  // -> 5

// BigInt for large integers
await db.set("big", 9007199254740993n);  // Uses BigInt
const big = await db.get("big");          // -> 9007199254740993n
```

### BigInt Handling

```typescript
// Internal conversion logic
function toJsValue(strataValue: StrataValue): any {
    if (strataValue.type === "Int") {
        const n = strataValue.value;
        if (n >= Number.MIN_SAFE_INTEGER && n <= Number.MAX_SAFE_INTEGER) {
            return Number(n);
        }
        return BigInt(n);
    }
    // ... other types
}
```

### Acceptance Criteria

- [ ] All value types mapped
- [ ] BigInt handling specified
- [ ] TypeScript types defined
- [ ] Async API (promises)
- [ ] Usage examples documented

---

## Story #601: SDK Conformance Test Harness

**File**: `crates/sdk/tests/conformance.rs` (NEW)

**Deliverable**: Conformance test harness for SDK validation

### Implementation

```rust
//! SDK Conformance Test Harness
//!
//! These tests verify that SDK behavior matches the contract.
//! Run against any SDK implementation.

use std::collections::HashMap;

/// SDK conformance test trait
pub trait ConformanceTarget {
    fn set(&self, key: &str, value: TestValue) -> Result<(), TestError>;
    fn get(&self, key: &str) -> Result<Option<TestValue>, TestError>;
    fn getv(&self, key: &str) -> Result<Option<TestVersioned>, TestError>;
    // ... all operations
}

/// Test value (JSON-serializable for cross-language testing)
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum TestValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    Array(Vec<TestValue>),
    Object(HashMap<String, TestValue>),
}

/// Test versioned value
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TestVersioned {
    pub value: TestValue,
    pub version: TestVersion,
    pub timestamp: u64,
}

/// Test version
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TestVersion {
    #[serde(rename = "type")]
    pub version_type: String,
    pub value: u64,
}

/// Conformance tests
pub mod conformance {
    use super::*;

    /// Test: Value types are preserved
    pub fn test_value_preservation<T: ConformanceTarget>(sdk: &T) {
        let test_values = vec![
            ("null", TestValue::Null),
            ("bool_true", TestValue::Bool(true)),
            ("bool_false", TestValue::Bool(false)),
            ("int_pos", TestValue::Int(42)),
            ("int_neg", TestValue::Int(-42)),
            ("int_max", TestValue::Int(i64::MAX)),
            ("int_min", TestValue::Int(i64::MIN)),
            ("float_pos", TestValue::Float(3.14)),
            ("float_neg", TestValue::Float(-3.14)),
            ("string", TestValue::String("hello".into())),
            ("string_empty", TestValue::String("".into())),
            ("bytes", TestValue::Bytes(vec![1, 2, 3])),
            ("bytes_empty", TestValue::Bytes(vec![])),
            ("array", TestValue::Array(vec![
                TestValue::Int(1),
                TestValue::String("two".into()),
            ])),
            ("object", TestValue::Object({
                let mut m = HashMap::new();
                m.insert("a".into(), TestValue::Int(1));
                m
            })),
        ];

        for (key, value) in test_values {
            sdk.set(key, value.clone()).unwrap();
            let retrieved = sdk.get(key).unwrap().unwrap();
            assert_eq!(retrieved, value, "Value mismatch for key '{}'", key);
        }
    }

    /// Test: Int and Float are distinct
    pub fn test_int_float_distinct<T: ConformanceTarget>(sdk: &T) {
        sdk.set("int_one", TestValue::Int(1)).unwrap();
        sdk.set("float_one", TestValue::Float(1.0)).unwrap();

        let int_val = sdk.get("int_one").unwrap().unwrap();
        let float_val = sdk.get("float_one").unwrap().unwrap();

        assert!(matches!(int_val, TestValue::Int(_)));
        assert!(matches!(float_val, TestValue::Float(_)));
        assert_ne!(int_val, float_val);
    }

    /// Test: Bytes and String are distinct
    pub fn test_bytes_string_distinct<T: ConformanceTarget>(sdk: &T) {
        sdk.set("bytes", TestValue::Bytes(b"hello".to_vec())).unwrap();
        sdk.set("string", TestValue::String("hello".into())).unwrap();

        let bytes_val = sdk.get("bytes").unwrap().unwrap();
        let string_val = sdk.get("string").unwrap().unwrap();

        assert!(matches!(bytes_val, TestValue::Bytes(_)));
        assert!(matches!(string_val, TestValue::String(_)));
        assert_ne!(bytes_val, string_val);
    }

    /// Test: Versioned shape preserved
    pub fn test_versioned_shape<T: ConformanceTarget>(sdk: &T) {
        sdk.set("key", TestValue::Int(42)).unwrap();
        let versioned = sdk.getv("key").unwrap().unwrap();

        assert_eq!(versioned.value, TestValue::Int(42));
        assert!(!versioned.version.version_type.is_empty());
        assert!(versioned.timestamp > 0);
    }

    /// Test: Error structure
    pub fn test_error_structure<T: ConformanceTarget>(sdk: &T) {
        // Trigger WrongType error
        sdk.set("string_key", TestValue::String("not a number".into())).unwrap();

        match sdk.incr("string_key", 1) {
            Err(e) => {
                assert_eq!(e.code, "WrongType");
                assert!(!e.message.is_empty());
            }
            Ok(_) => panic!("Expected WrongType error"),
        }
    }

    /// Run all conformance tests
    pub fn run_all<T: ConformanceTarget>(sdk: &T) {
        test_value_preservation(sdk);
        test_int_float_distinct(sdk);
        test_bytes_string_distinct(sdk);
        test_versioned_shape(sdk);
        test_error_structure(sdk);
    }
}
```

### Acceptance Criteria

- [ ] Value preservation tests
- [ ] Int/Float distinction tests
- [ ] Bytes/String distinction tests
- [ ] Versioned shape tests
- [ ] Error structure tests
- [ ] Cross-language test format (JSON-serializable)

---

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_sdk_conformance() {
        let db = Strata::open(":memory:").unwrap();
        conformance::run_all(&RustSdkAdapter { db });
    }

    struct RustSdkAdapter {
        db: Strata,
    }

    impl ConformanceTarget for RustSdkAdapter {
        fn set(&self, key: &str, value: TestValue) -> Result<(), TestError> {
            self.db.set(key, test_to_value(value))?;
            Ok(())
        }

        fn get(&self, key: &str) -> Result<Option<TestValue>, TestError> {
            Ok(self.db.get(key)?.map(value_to_test))
        }

        // ... implement all methods
    }
}
```

---

## Files Modified/Created

| File | Action |
|------|--------|
| `docs/architecture/SDK_MAPPING.md` | CREATE - SDK mapping specification |
| `crates/sdk/src/lib.rs` | CREATE - Rust SDK |
| `crates/sdk/src/error.rs` | CREATE - SDK error handling |
| `crates/sdk/tests/conformance.rs` | CREATE - Conformance harness |
| `Cargo.toml` | MODIFY - Add sdk crate |
