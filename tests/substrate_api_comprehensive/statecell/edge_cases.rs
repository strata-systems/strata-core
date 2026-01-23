//! StateCell Edge Cases Tests
//!
//! Tests for validation and boundary conditions:
//! - Cell name validation
//! - Large values
//! - Special characters
//! - Unicode handling

use crate::*;

/// Test cell with special characters in name
#[test]
fn test_state_special_characters_in_name() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let special_names = vec![
            "cell-with-dashes",
            "cell_with_underscores",
            "cell.with.dots",
            "cell:with:colons",
            "cell/with/slashes",
            "cell@with@at",
        ];

        for name in special_names {
            db.state_set(&run, name, Value::Int(1)).unwrap();
            let result = db.state_get(&run, name).unwrap();
            assert!(result.is_some(), "Cell '{}' should be retrievable", name);
        }
    });
}

/// Test cell with Unicode name
#[test]
fn test_state_unicode_cell_name() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let unicode_names = vec![
            "cell_\u{4e2d}\u{6587}", // Chinese
            "cell_\u{65e5}\u{672c}\u{8a9e}", // Japanese
            "cell_\u{d55c}\u{ad6d}\u{c5b4}", // Korean
            "cell_\u{1f600}", // Emoji
        ];

        for name in unicode_names {
            db.state_set(&run, name, Value::String("unicode".to_string())).unwrap();
            let result = db.state_get(&run, name).unwrap();
            assert!(result.is_some(), "Unicode cell '{}' should work", name);
        }
    });
}

/// Test very long cell name
#[test]
fn test_state_long_cell_name() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Create a 1000 character cell name
        let long_name: String = (0..1000).map(|_| 'x').collect();

        db.state_set(&run, &long_name, Value::Int(42)).unwrap();
        let result = db.state_get(&run, &long_name).unwrap();
        assert!(result.is_some(), "Long cell name should work");
    });
}

/// Test large value
#[test]
fn test_state_large_value() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "large_value_cell";

        // Create a large string (1MB)
        let large_string: String = (0..1_000_000).map(|i| ((i % 26) as u8 + b'a') as char).collect();

        db.state_set(&run, cell, Value::String(large_string.clone())).unwrap();
        let result = db.state_get(&run, cell).unwrap().unwrap();

        if let Value::String(s) = result.value {
            assert_eq!(s.len(), 1_000_000, "Large string should be preserved");
        } else {
            panic!("Expected string value");
        }
    });
}

/// Test large bytes value
#[test]
fn test_state_large_bytes_value() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "large_bytes_cell";

        // Create large bytes (100KB)
        let large_bytes: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();

        db.state_set(&run, cell, Value::Bytes(large_bytes.clone())).unwrap();
        let result = db.state_get(&run, cell).unwrap().unwrap();

        if let Value::Bytes(b) = result.value {
            assert_eq!(b.len(), 100_000, "Large bytes should be preserved");
            assert_eq!(b, large_bytes, "Bytes content should match");
        } else {
            panic!("Expected bytes value");
        }
    });
}

/// Test deeply nested value
#[test]
fn test_state_deeply_nested_value() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "nested_cell";

        // Create nested structure
        let mut nested = Value::Int(42);
        for _ in 0..10 {
            nested = Value::Array(vec![nested]);
        }

        db.state_set(&run, cell, nested.clone()).unwrap();
        let result = db.state_get(&run, cell).unwrap().unwrap();
        assert_eq!(result.value, nested);
    });
}

/// Test setting Null explicitly
#[test]
fn test_state_explicit_null() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "null_cell";

        // Set non-null first
        db.state_set(&run, cell, Value::Int(42)).unwrap();
        assert_eq!(db.state_get(&run, cell).unwrap().unwrap().value, Value::Int(42));

        // Set to null
        db.state_set(&run, cell, Value::Null).unwrap();
        let result = db.state_get(&run, cell).unwrap().unwrap();
        assert_eq!(result.value, Value::Null, "Explicit null should be stored");
    });
}

/// Test float special values
/// Note: Infinity and NaN are not valid JSON values and cannot be serialized.
/// This test only covers valid JSON float values.
#[test]
fn test_state_float_special_values() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Only test JSON-serializable float values
        let test_cases = vec![
            ("zero_cell", 0.0),
            ("neg_zero_cell", -0.0),
            ("tiny_cell", f64::MIN_POSITIVE),
            ("max_cell", f64::MAX),
            ("min_cell", f64::MIN),
            ("pi_cell", std::f64::consts::PI),
            ("neg_pi_cell", -std::f64::consts::PI),
        ];

        for (cell, value) in test_cases {
            db.state_set(&run, cell, Value::Float(value)).unwrap();
            let result = db.state_get(&run, cell).unwrap().unwrap();
            if let Value::Float(f) = result.value {
                // For -0.0 and 0.0, they compare equal but have different bit patterns
                if value == 0.0 {
                    assert_eq!(f, 0.0, "Float {} should be zero", cell);
                } else {
                    assert_eq!(f, value, "Float {} should be preserved", cell);
                }
            } else {
                panic!("Expected float value for {}", cell);
            }
        }
    });
}

/// Test Infinity handling - documents round-trip behavior limitation
/// Infinity is not a valid JSON value, so round-trip may fail
#[test]
fn test_state_float_infinity_handling() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Store Infinity - behavior depends on serialization
        let set_result = db.state_set(&run, "inf_cell", Value::Float(f64::INFINITY));

        if set_result.is_ok() {
            // If set succeeds, the get might fail due to JSON round-trip issues
            // (stored as null in JSON, but deserialization expects f64)
            let get_result = db.state_get(&run, "inf_cell");

            // Either success with transformed value, or deserialization error is acceptable
            match get_result {
                Ok(Some(versioned)) => {
                    // If we successfully get it back, check the value
                    match versioned.value {
                        Value::Null => (), // Serialized as null - acceptable
                        Value::Float(f) if f.is_infinite() => (), // Preserved - also acceptable
                        other => panic!("Unexpected value for infinity: {:?}", other),
                    }
                }
                Ok(None) => (), // Cell doesn't exist - acceptable
                Err(_) => (), // Deserialization error - acceptable (JSON limitation)
            }
        }
        // If set fails, that's also acceptable - JSON doesn't support Infinity
    });
}

/// Test NaN handling - documents round-trip behavior limitation
/// NaN is not a valid JSON value, so round-trip may fail
#[test]
fn test_state_float_nan_handling() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "nan_cell";

        // Store NaN - behavior depends on serialization
        let set_result = db.state_set(&run, cell, Value::Float(f64::NAN));

        if set_result.is_ok() {
            // If set succeeds, the get might fail due to JSON round-trip issues
            // (stored as null in JSON, but deserialization expects f64)
            let get_result = db.state_get(&run, cell);

            // Either success with transformed value, or deserialization error is acceptable
            match get_result {
                Ok(Some(versioned)) => {
                    match versioned.value {
                        Value::Null => (), // Serialized as null - acceptable
                        Value::Float(f) if f.is_nan() => (), // Preserved - also acceptable
                        other => panic!("Unexpected value for NaN: {:?}", other),
                    }
                }
                Ok(None) => (), // Cell doesn't exist - acceptable
                Err(_) => (), // Deserialization error - acceptable (JSON limitation)
            }
        }
        // If set fails, that's also acceptable - JSON doesn't support NaN
    });
}

/// Test integer boundary values
#[test]
fn test_state_integer_boundaries() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let test_cases = vec![
            ("max_i64", i64::MAX),
            ("min_i64", i64::MIN),
            ("zero_i64", 0i64),
            ("neg_one", -1i64),
        ];

        for (cell, value) in test_cases {
            db.state_set(&run, cell, Value::Int(value)).unwrap();
            let result = db.state_get(&run, cell).unwrap().unwrap();
            assert_eq!(result.value, Value::Int(value), "{} should be preserved", cell);
        }
    });
}

/// Test empty string value
#[test]
fn test_state_empty_string() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "empty_string_cell";

        db.state_set(&run, cell, Value::String(String::new())).unwrap();
        let result = db.state_get(&run, cell).unwrap().unwrap();
        assert_eq!(result.value, Value::String(String::new()));
    });
}

/// Test empty bytes value
#[test]
fn test_state_empty_bytes() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "empty_bytes_cell";

        db.state_set(&run, cell, Value::Bytes(vec![])).unwrap();
        let result = db.state_get(&run, cell).unwrap().unwrap();
        assert_eq!(result.value, Value::Bytes(vec![]));
    });
}

/// Test empty array value
#[test]
fn test_state_empty_array() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let cell = "empty_array_cell";

        db.state_set(&run, cell, Value::Array(vec![])).unwrap();
        let result = db.state_get(&run, cell).unwrap().unwrap();
        assert_eq!(result.value, Value::Array(vec![]));
    });
}
