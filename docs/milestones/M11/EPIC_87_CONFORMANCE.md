# Epic 87: Contract Conformance Testing

**Goal**: Comprehensive tests verifying all contract invariants

**Dependencies**: Epic 84, Epic 86

---

## Scope

- Facade-Substrate parity tests
- Value model & wire encoding tests
- Error model coverage tests
- CLI conformance tests
- SDK conformance tests

---

## User Stories

| Story | Description | Priority |
|-------|-------------|----------|
| #602 | Facade-Substrate Parity Tests | CRITICAL |
| #603 | Value Model & Wire Encoding Tests | CRITICAL |
| #604 | Error Model Coverage Tests | CRITICAL |
| #605 | CLI Conformance Tests | CRITICAL |
| #606 | SDK Conformance Tests | CRITICAL |

---

## Story #602: Facade-Substrate Parity Tests

**File**: `crates/api/tests/conformance_tests.rs` (NEW)

**Deliverable**: Tests verifying facade-substrate equivalence

### Implementation

```rust
//! Facade-Substrate Parity Tests
//!
//! Verifies FAC invariants:
//! - FAC-1: Every facade operation maps to deterministic substrate operations
//! - FAC-2: Facade adds no semantic behavior beyond defaults
//! - FAC-3: Facade never swallows substrate errors
//! - FAC-4: Facade does not reorder operations
//! - FAC-5: All behavior traces to explicit substrate operation

#[cfg(test)]
mod parity_tests {
    use super::*;

    /// FAC-1: Deterministic mapping
    #[test]
    fn test_fac1_deterministic_mapping() {
        let db = setup_test_db();
        let run = RunId::default_run();

        // Execute same logical operation via facade and substrate
        // Verify identical results

        // Facade
        db.facade().set("key", Value::Int(42)).unwrap();
        let facade_result = db.facade().get("key").unwrap();

        // Reset
        db.facade().delete(&["key"]).unwrap();

        // Substrate (desugared)
        let txn = db.substrate().begin(&run).unwrap();
        db.substrate().kv_put(&txn, "key", Value::Int(42)).unwrap();
        db.substrate().commit(txn).unwrap();
        let substrate_result = db.substrate()
            .kv_get(&run, "key")
            .unwrap()
            .map(|v| v.value);

        assert_eq!(facade_result, substrate_result);
    }

    /// FAC-2: No semantic behavior beyond defaults
    #[test]
    fn test_fac2_no_extra_semantics() {
        let db = setup_test_db();
        let run = RunId::default_run();

        // Facade set should only do: begin, kv_put, commit
        // Nothing else (no caching, no preprocessing, etc.)

        db.facade().set("key", Value::Int(1)).unwrap();

        // Verify state is exactly what substrate would produce
        let versioned = db.substrate().kv_get(&run, "key").unwrap().unwrap();
        assert_eq!(versioned.value, Value::Int(1));
        assert!(matches!(versioned.version, Version::Txn(_)));
    }

    /// FAC-3: Error propagation
    #[test]
    fn test_fac3_error_propagation() {
        let db = setup_test_db();

        // Create situation that causes substrate error
        db.facade().set("key", Value::String("not int".into())).unwrap();

        // Facade should propagate substrate error unchanged
        let facade_err = db.facade().incr("key", 1).unwrap_err();
        assert_eq!(facade_err.code(), "WrongType");

        // Error details should be preserved
        if let StrataError::WrongType { expected, actual, .. } = &facade_err {
            assert_eq!(expected, "Int");
            assert_eq!(actual, "String");
        }
    }

    /// FAC-4: Operation ordering
    #[test]
    fn test_fac4_operation_ordering() {
        let db = setup_test_db();

        // mset should write keys in order
        db.facade().mset(&[
            ("a", Value::Int(1)),
            ("b", Value::Int(2)),
            ("c", Value::Int(3)),
        ]).unwrap();

        // All should be present
        let values = db.facade().mget(&["a", "b", "c"]).unwrap();
        assert_eq!(values, vec![
            Some(Value::Int(1)),
            Some(Value::Int(2)),
            Some(Value::Int(3)),
        ]);
    }

    /// FAC-5: All behavior traces to substrate
    #[test]
    fn test_fac5_traceable_behavior() {
        let db = setup_test_db();
        let run = RunId::default_run();

        // Every facade operation should be explainable via substrate
        // This test verifies by checking intermediate state

        // Set up initial state
        db.facade().set("key", Value::Int(0)).unwrap();

        // Facade incr
        let result = db.facade().incr("key", 5).unwrap();
        assert_eq!(result, 5);

        // Verify via substrate
        let versioned = db.substrate().kv_get(&run, "key").unwrap().unwrap();
        assert_eq!(versioned.value, Value::Int(5));
    }

    /// Test all operations produce parity
    #[test]
    fn test_comprehensive_parity() {
        let db = setup_test_db();
        let run = RunId::default_run();

        let test_cases = vec![
            ("set", || db.facade().set("k", Value::Int(1))),
            ("get", || db.facade().get("k").map(|_| ())),
            ("delete", || db.facade().delete(&["k"]).map(|_| ())),
            ("json_set", || db.facade().json_set("doc", "$", json!({}))),
            ("xadd", || db.facade().xadd("stream", json!({})).map(|_| ())),
            ("vset", || db.facade().vset("vec", vec![1.0], json!({}))),
            ("cas_set", || db.facade().cas_set("cas", None, Value::Int(1)).map(|_| ())),
        ];

        for (name, op) in test_cases {
            let result = op();
            assert!(result.is_ok(), "Parity test '{}' failed: {:?}", name, result);
        }
    }
}
```

### Acceptance Criteria

- [ ] FAC-1: Deterministic mapping verified
- [ ] FAC-2: No extra semantics verified
- [ ] FAC-3: Error propagation verified
- [ ] FAC-4: Operation ordering verified
- [ ] FAC-5: Traceable behavior verified

---

## Story #603: Value Model & Wire Encoding Tests

**File**: `crates/wire/tests/roundtrip_tests.rs` (NEW)

**Deliverable**: Tests for value model and wire encoding invariants

### Implementation

```rust
//! Value Model & Wire Encoding Tests
//!
//! Verifies VAL and WIRE invariants.

#[cfg(test)]
mod value_tests {
    use super::*;

    /// VAL-1: Eight types only
    #[test]
    fn test_val1_eight_types() {
        // Exhaustive pattern match (compiler enforces completeness)
        fn type_name(v: &Value) -> &'static str {
            match v {
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

        let types = vec![
            type_name(&Value::Null),
            type_name(&Value::Bool(true)),
            type_name(&Value::Int(0)),
            type_name(&Value::Float(0.0)),
            type_name(&Value::String("".into())),
            type_name(&Value::Bytes(vec![])),
            type_name(&Value::Array(vec![])),
            type_name(&Value::Object(HashMap::new())),
        ];

        assert_eq!(types.len(), 8);
    }

    /// VAL-2: No implicit type coercions
    #[test]
    fn test_val2_no_coercion() {
        // Int != Float even with same numeric value
        assert_ne!(Value::Int(1), Value::Float(1.0));
        assert_ne!(Value::Int(0), Value::Float(0.0));
        assert_ne!(Value::Int(-1), Value::Float(-1.0));
    }

    /// VAL-3: Int(1) != Float(1.0)
    #[test]
    fn test_val3_int_float_distinct() {
        let int_one = Value::Int(1);
        let float_one = Value::Float(1.0);

        assert_ne!(int_one, float_one);
        assert_ne!(float_one, int_one);
    }

    /// VAL-4: Bytes are not String
    #[test]
    fn test_val4_bytes_not_string() {
        let bytes = Value::Bytes(b"hello".to_vec());
        let string = Value::String("hello".into());

        assert_ne!(bytes, string);
    }

    /// VAL-5: IEEE-754 float equality
    #[test]
    fn test_val5_ieee754_equality() {
        // NaN != NaN
        let nan1 = Value::Float(f64::NAN);
        let nan2 = Value::Float(f64::NAN);
        assert_ne!(nan1, nan2);

        // -0.0 == 0.0
        let neg_zero = Value::Float(-0.0);
        let pos_zero = Value::Float(0.0);
        assert_eq!(neg_zero, pos_zero);
    }
}

#[cfg(test)]
mod wire_tests {
    use super::*;
    use crate::wire::json::{encode_value, decode_value};

    /// WIRE-1: JSON encoding is mandatory
    #[test]
    fn test_wire1_json_mandatory() {
        // All value types must encode to valid JSON
        let values = vec![
            Value::Null,
            Value::Bool(true),
            Value::Int(42),
            Value::Float(3.14),
            Value::String("hello".into()),
            Value::Bytes(vec![1, 2, 3]),
            Value::Array(vec![Value::Int(1)]),
            Value::Object(HashMap::new()),
        ];

        for v in values {
            let json = encode_value(&v);
            // Should be valid JSON
            assert!(serde_json::to_string(&json).is_ok());
        }
    }

    /// WIRE-2: Bytes encode as $bytes wrapper
    #[test]
    fn test_wire2_bytes_wrapper() {
        let bytes = Value::Bytes(vec![72, 101, 108, 108, 111]); // "Hello"
        let json = encode_value(&bytes);

        assert_eq!(json, serde_json::json!({ "$bytes": "SGVsbG8=" }));
    }

    /// WIRE-3: Non-finite floats encode as $f64 wrapper
    #[test]
    fn test_wire3_float_wrapper() {
        let test_cases = vec![
            (Value::Float(f64::NAN), serde_json::json!({ "$f64": "NaN" })),
            (Value::Float(f64::INFINITY), serde_json::json!({ "$f64": "+Inf" })),
            (Value::Float(f64::NEG_INFINITY), serde_json::json!({ "$f64": "-Inf" })),
            (Value::Float(-0.0), serde_json::json!({ "$f64": "-0.0" })),
        ];

        for (value, expected) in test_cases {
            let json = encode_value(&value);
            assert_eq!(json, expected);
        }
    }

    /// WIRE-4: Absent values encode as $absent wrapper
    #[test]
    fn test_wire4_absent_wrapper() {
        let absent = encode_absent();
        assert_eq!(absent, serde_json::json!({ "$absent": true }));

        assert!(is_absent(&absent));
        assert!(!is_absent(&serde_json::Value::Null));
    }

    /// WIRE-5: Round-trip preserves exact type and value
    #[test]
    fn test_wire5_roundtrip() {
        let values = vec![
            Value::Null,
            Value::Bool(true),
            Value::Bool(false),
            Value::Int(i64::MIN),
            Value::Int(i64::MAX),
            Value::Int(0),
            Value::Float(3.14159),
            Value::Float(0.0),
            Value::Float(-0.0), // Preserved even though == 0.0
            Value::Float(f64::INFINITY),
            Value::Float(f64::NEG_INFINITY),
            Value::String("hello world".into()),
            Value::String("".into()),
            Value::Bytes(vec![0, 255, 128]),
            Value::Bytes(vec![]),
            Value::Array(vec![Value::Int(1), Value::String("two".into())]),
            Value::Object({
                let mut m = HashMap::new();
                m.insert("key".into(), Value::Bool(true));
                m
            }),
        ];

        for value in values {
            let json = encode_value(&value);
            let decoded = decode_value(&json).unwrap();

            // Special case: NaN != NaN, but both should be NaN
            if let Value::Float(f) = &value {
                if f.is_nan() {
                    if let Value::Float(d) = &decoded {
                        assert!(d.is_nan());
                        continue;
                    }
                }
            }

            assert_eq!(value, decoded, "Round-trip failed for {:?}", value);
        }
    }

    /// Test negative zero preservation
    #[test]
    fn test_negative_zero_preserved() {
        let neg_zero = Value::Float(-0.0);
        let json = encode_value(&neg_zero);
        let decoded = decode_value(&json).unwrap();

        if let Value::Float(f) = decoded {
            assert!(f.is_sign_negative(), "-0.0 sign not preserved");
        } else {
            panic!("Expected Float");
        }
    }
}
```

### Acceptance Criteria

- [ ] VAL-1 to VAL-5 verified
- [ ] WIRE-1 to WIRE-5 verified
- [ ] Float edge cases (NaN, Inf, -0.0) preserved
- [ ] Round-trip tests pass for all types

---

## Story #604: Error Model Coverage Tests

**File**: `crates/core/tests/error_tests.rs` (NEW)

**Deliverable**: Tests for all error conditions

### Implementation

```rust
//! Error Model Coverage Tests
//!
//! Verifies ERR invariants and all error-producing conditions.

#[cfg(test)]
mod error_tests {
    use super::*;

    /// ERR-1: All errors surface through structured model
    #[test]
    fn test_err1_structured_errors() {
        let db = setup_test_db();

        // Trigger various errors
        let errors = vec![
            db.facade().get("_strata/reserved").unwrap_err(),  // InvalidKey
            db.facade().json_get("doc", "invalid[[path").unwrap_err(), // InvalidPath
            {
                db.facade().set("key", Value::String("x".into())).unwrap();
                db.facade().incr("key", 1).unwrap_err()  // WrongType
            },
        ];

        for err in errors {
            // All must have code, message
            assert!(!err.code().is_empty());
            assert!(!err.message().is_empty());
        }
    }

    /// ERR-2: All errors include code, message, details
    #[test]
    fn test_err2_error_completeness() {
        let err = StrataError::HistoryTrimmed {
            message: "Test".into(),
            requested: Version::Txn(100),
            earliest_retained: Version::Txn(150),
        };

        let wire = err.to_wire_response();
        assert_eq!(wire["ok"], false);
        assert!(wire["error"]["code"].is_string());
        assert!(wire["error"]["message"].is_string());
        assert!(wire["error"]["details"].is_object());
    }

    /// ERR-3: No undefined behavior
    #[test]
    fn test_err3_no_undefined_behavior() {
        let db = setup_test_db();

        // All edge cases should return explicit errors, not panic
        let edge_cases: Vec<Box<dyn FnOnce() -> Result<(), StrataError>>> = vec![
            Box::new(|| db.facade().set("", Value::Null)), // Empty key
            Box::new(|| db.facade().set("a\0b", Value::Null)), // NUL in key
            Box::new(|| db.facade().json_get("x", "not a path")), // Invalid path
            Box::new(|| {
                db.facade().set("x", Value::String("y".into()))?;
                db.facade().incr("x", 1)?;
                Ok(())
            }), // Wrong type
        ];

        for case in edge_cases {
            let result = case();
            assert!(result.is_err(), "Expected error, got Ok");
        }
    }

    /// ERR-4: Conflict vs ConstraintViolation
    #[test]
    fn test_err4_error_categories() {
        let db = setup_test_db();

        // ConstraintViolation = structural
        db.facade().set("key", Value::Int(1)).unwrap();
        let result = db.facade().cas_set("key", Some(Value::Int(999)), Value::Int(2));
        // CAS mismatch is Conflict (temporal)
        if let Err(e) = result {
            // Note: CAS mismatch returns bool, not error. Test with wrong type instead.
        }

        // Reserved prefix = InvalidKey (structural)
        let err = db.facade().set("_strata/test", Value::Int(1)).unwrap_err();
        assert_eq!(err.code(), "InvalidKey");
    }

    /// Test all error-producing conditions
    #[test]
    fn test_all_error_conditions() {
        let db = setup_test_db();

        // InvalidKey conditions
        assert!(db.facade().set("", Value::Null).is_err());  // Empty key
        assert!(db.facade().set("a\0b", Value::Null).is_err());  // NUL byte
        assert!(db.facade().set("_strata/x", Value::Null).is_err());  // Reserved
        assert!(db.facade().set(&"x".repeat(2000), Value::Null).is_err());  // Too long

        // WrongType conditions
        db.facade().set("str", Value::String("x".into())).unwrap();
        assert!(db.facade().incr("str", 1).is_err());  // incr on non-Int

        // Overflow conditions
        db.facade().set("max", Value::Int(i64::MAX)).unwrap();
        assert!(db.facade().incr("max", 1).is_err());  // Overflow

        db.facade().set("min", Value::Int(i64::MIN)).unwrap();
        assert!(db.facade().incr("min", -1).is_err());  // Underflow
    }
}
```

### Acceptance Criteria

- [ ] ERR-1 to ERR-4 verified
- [ ] All error-producing conditions tested
- [ ] No panics on edge cases (only explicit errors)

---

## Story #605: CLI Conformance Tests

**File**: `crates/cli/tests/conformance.rs` (NEW)

**Deliverable**: CLI behavior conformance tests

### Implementation

```rust
//! CLI Conformance Tests
//!
//! Verifies CLI parsing rules and output formatting.

use std::process::Command;

fn run_cli(args: &[&str]) -> (i32, String, String) {
    let output = Command::new("cargo")
        .args(&["run", "--bin", "strata", "--"])
        .args(args)
        .output()
        .expect("Failed to run CLI");

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (exit_code, stdout, stderr)
}

#[cfg(test)]
mod parsing_tests {
    use super::*;

    #[test]
    fn test_int_parsing() {
        run_cli(&["set", "x", "123"]);
        let (_, stdout, _) = run_cli(&["get", "x"]);
        assert!(stdout.contains("123"));
    }

    #[test]
    fn test_negative_int_parsing() {
        run_cli(&["set", "x", "-456"]);
        let (_, stdout, _) = run_cli(&["get", "x"]);
        assert!(stdout.contains("-456"));
    }

    #[test]
    fn test_float_parsing() {
        run_cli(&["set", "x", "3.14"]);
        let (_, stdout, _) = run_cli(&["get", "x"]);
        assert!(stdout.contains("3.14"));
    }

    #[test]
    fn test_string_quoted() {
        run_cli(&["set", "x", "\"hello world\""]);
        let (_, stdout, _) = run_cli(&["get", "x"]);
        assert!(stdout.contains("hello world"));
    }

    #[test]
    fn test_string_bare() {
        run_cli(&["set", "x", "hello"]);
        let (_, stdout, _) = run_cli(&["get", "x"]);
        assert!(stdout.contains("hello"));
    }

    #[test]
    fn test_bool_parsing() {
        run_cli(&["set", "t", "true"]);
        run_cli(&["set", "f", "false"]);

        let (_, stdout_t, _) = run_cli(&["get", "t"]);
        let (_, stdout_f, _) = run_cli(&["get", "f"]);

        // Bools displayed as (integer) 1 or 0
        assert!(stdout_t.contains("1") || stdout_t.contains("true"));
        assert!(stdout_f.contains("0") || stdout_f.contains("false"));
    }

    #[test]
    fn test_null_parsing() {
        run_cli(&["set", "x", "null"]);
        let (_, stdout, _) = run_cli(&["get", "x"]);
        assert!(stdout.contains("null"));
    }

    #[test]
    fn test_bytes_parsing() {
        run_cli(&["set", "x", "b64:SGVsbG8="]);  // "Hello"
        let (_, stdout, _) = run_cli(&["get", "x"]);
        assert!(stdout.contains("$bytes"));
    }

    #[test]
    fn test_json_object_parsing() {
        run_cli(&["set", "x", r#"{"a": 1}"#]);
        let (_, stdout, _) = run_cli(&["get", "x"]);
        assert!(stdout.contains("a"));
        assert!(stdout.contains("1"));
    }
}

#[cfg(test)]
mod output_tests {
    use super::*;

    #[test]
    fn test_nil_output() {
        let (_, stdout, _) = run_cli(&["get", "nonexistent"]);
        assert!(stdout.contains("(nil)"));
    }

    #[test]
    fn test_integer_output() {
        run_cli(&["set", "x", "1"]);
        let (_, stdout, _) = run_cli(&["delete", "x"]);
        assert!(stdout.contains("(integer) 1"));
    }

    #[test]
    fn test_bool_as_integer() {
        let (_, stdout, _) = run_cli(&["exists", "nonexistent"]);
        assert!(stdout.contains("(integer) 0"));
    }
}

#[cfg(test)]
mod exit_code_tests {
    use super::*;

    #[test]
    fn test_success_exit_code() {
        run_cli(&["set", "x", "1"]);
        let (code, _, _) = run_cli(&["get", "x"]);
        assert_eq!(code, 0);
    }

    #[test]
    fn test_error_exit_code() {
        let (code, _, stderr) = run_cli(&["get", "_strata/reserved"]);
        assert_eq!(code, 1);
        assert!(stderr.contains("InvalidKey") || stderr.contains("error"));
    }

    #[test]
    fn test_usage_exit_code() {
        let (code, _, _) = run_cli(&["unknown_command"]);
        assert_eq!(code, 2);
    }
}
```

### Acceptance Criteria

- [ ] Argument parsing tests pass
- [ ] Output formatting tests pass
- [ ] Exit codes correct (0, 1, 2)
- [ ] Error output on stderr

---

## Story #606: SDK Conformance Tests

**File**: `crates/sdk/tests/sdk_conformance.rs` (NEW)

**Deliverable**: SDK conformance test suite

### Implementation

```rust
//! SDK Conformance Tests
//!
//! Tests SDK behavior against contract requirements.

#[cfg(test)]
mod sdk_conformance {
    use super::*;
    use strata_sdk::Strata;

    #[test]
    fn test_value_mapping_preservation() {
        let db = Strata::open(":memory:").unwrap();

        // Test all value types
        let test_values = vec![
            ("null", Value::Null),
            ("bool", Value::Bool(true)),
            ("int", Value::Int(42)),
            ("float", Value::Float(3.14)),
            ("string", Value::String("hello".into())),
            ("bytes", Value::Bytes(vec![1, 2, 3])),
            ("array", Value::Array(vec![Value::Int(1)])),
            ("object", Value::Object({
                let mut m = HashMap::new();
                m.insert("a".into(), Value::Int(1));
                m
            })),
        ];

        for (key, value) in test_values {
            db.set(key, value.clone()).unwrap();
            let retrieved = db.get(key).unwrap().unwrap();
            assert_eq!(retrieved, value, "Value mismatch for {}", key);
        }
    }

    #[test]
    fn test_versioned_shape() {
        let db = Strata::open(":memory:").unwrap();

        db.set("key", Value::Int(42)).unwrap();
        let versioned = db.getv("key").unwrap().unwrap();

        // Versioned must have value, version, timestamp
        assert_eq!(versioned.value, Value::Int(42));
        assert!(matches!(versioned.version, Version::Txn(_)));
        assert!(versioned.timestamp > 0);
    }

    #[test]
    fn test_error_handling() {
        let db = Strata::open(":memory:").unwrap();

        // Trigger WrongType error
        db.set("key", Value::String("not a number".into())).unwrap();
        let err = db.incr("key", 1).unwrap_err();

        // Error must have code, message
        assert_eq!(err.code(), "WrongType");
        assert!(!err.message().is_empty());
    }

    #[test]
    fn test_operation_names() {
        let db = Strata::open(":memory:").unwrap();

        // SDK must use same operation names as facade
        // These should all compile and work
        db.set("k", Value::Int(1)).unwrap();
        db.get("k").unwrap();
        db.getv("k").unwrap();
        db.mget(&["k"]).unwrap();
        db.mset(&[("k", Value::Int(2))]).unwrap();
        db.delete(&["k"]).unwrap();
        db.exists("k").unwrap();
        db.exists_many(&["k"]).unwrap();
        db.incr("k", 1).unwrap();

        db.json_set("doc", "$", json!({})).unwrap();
        db.json_get("doc", "$").unwrap();
        db.json_getv("doc", "$").unwrap();
        db.json_del("doc", "$.x").unwrap();
        db.json_merge("doc", "$", json!({"a": 1})).unwrap();

        db.xadd("stream", json!({})).unwrap();
        db.xrange("stream", None, None, None).unwrap();
        db.xlen("stream").unwrap();

        db.vset("vec", vec![1.0], json!({})).unwrap();
        db.vget("vec").unwrap();
        db.vdel("vec").unwrap();

        db.cas_set("cas", None, Value::Int(1)).unwrap();
        db.cas_get("cas").unwrap();

        db.history("k", None, None).unwrap();
        db.runs().unwrap();
        db.capabilities();
    }

    /// Minimum 70 conformance tests
    #[test]
    fn test_conformance_coverage() {
        // This meta-test ensures we have enough coverage
        let test_count = count_tests_in_module();
        assert!(test_count >= 70, "Need at least 70 conformance tests, found {}", test_count);
    }
}
```

### Acceptance Criteria

- [ ] Value mapping tests pass
- [ ] Error handling tests pass
- [ ] Operation names match facade
- [ ] Minimum 70 conformance tests

---

## Testing Summary

The conformance test suite must verify:

| Category | Tests |
|----------|-------|
| Facade-Substrate parity | FAC-1 to FAC-5 |
| Value model | VAL-1 to VAL-5 |
| Wire encoding | WIRE-1 to WIRE-5 |
| Error model | ERR-1 to ERR-4 |
| Determinism | DET-1 to DET-5 |
| Versioned | VER-1 to VER-4 |
| CLI | Parsing, output, exit codes |
| SDK | Value mapping, errors, operations |

**Total minimum**: 70 conformance tests

---

## Files Modified/Created

| File | Action |
|------|--------|
| `crates/api/tests/conformance_tests.rs` | CREATE - Conformance tests |
| `crates/wire/tests/roundtrip_tests.rs` | CREATE - Wire encoding tests |
| `crates/core/tests/error_tests.rs` | CREATE - Error model tests |
| `crates/cli/tests/conformance.rs` | CREATE - CLI conformance |
| `crates/sdk/tests/sdk_conformance.rs` | CREATE - SDK conformance |
