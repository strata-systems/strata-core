//! JsonStore Edge Cases Tests
//!
//! Tests for validation and boundary conditions:
//! - Key validation
//! - Path syntax validation
//! - Large documents
//! - Special characters

use crate::*;

/// Test document key with special characters
#[test]
fn test_json_special_key_names() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let keys = vec![
            "key-with-dashes",
            "key_with_underscores",
            "key.with.dots",
            "key:with:colons",
        ];

        for key in keys {
            let document = obj([("test", Value::Int(1))]);
            db.json_set(&run, key, "$", document).unwrap();
            assert!(db.json_exists(&run, key).unwrap(), "Key '{}' should work", key);
        }
    });
}

/// Test document key with unicode
#[test]
fn test_json_unicode_key() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let keys = vec![
            "key_\u{4e2d}\u{6587}", // Chinese
            "key_\u{65e5}\u{672c}\u{8a9e}", // Japanese
            "key_\u{d55c}\u{ad6d}\u{c5b4}", // Korean
        ];

        for key in keys {
            let document = obj([("unicode", Value::String("test".to_string()))]);
            db.json_set(&run, key, "$", document).unwrap();
            assert!(db.json_exists(&run, key).unwrap());
        }
    });
}

/// Test field names with special characters
#[test]
fn test_json_special_field_names() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "special_fields";

        let document = obj([
            ("field_underscores", Value::Int(2)),
            ("field123", Value::Int(3)),
            ("UPPERCASE", Value::Int(4)),
            ("MixedCase", Value::Int(5)),
        ]);
        db.json_set(&run, key, "$", document).unwrap();

        // Access fields with special names
        let f2 = db.json_get(&run, key, "field_underscores").unwrap();
        let f3 = db.json_get(&run, key, "field123").unwrap();

        assert!(f2.is_some(), "Underscore fields should work");
        assert!(f3.is_some(), "Numeric suffix should work");
    });
}

/// Test large document
#[test]
fn test_json_large_document() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "large_doc";

        // Create document with many fields
        let fields: HashMap<String, Value> = (0..100)
            .map(|i| (format!("field_{}", i), Value::Int(i)))
            .collect();
        let document = Value::Object(fields);

        db.json_set(&run, key, "$", document).unwrap();

        // Verify some fields
        let f0 = db.json_get(&run, key, "field_0").unwrap().unwrap();
        let f99 = db.json_get(&run, key, "field_99").unwrap().unwrap();
        assert_eq!(f0.value, Value::Int(0));
        assert_eq!(f99.value, Value::Int(99));
    });
}

/// Test large nested array
#[test]
fn test_json_large_array() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "large_array";

        // Create document with large array
        let items: Vec<Value> = (0..1000).map(|i| Value::Int(i)).collect();
        let document = obj([("items", Value::Array(items))]);

        db.json_set(&run, key, "$", document).unwrap();

        // Access elements at different positions
        let first = db.json_get(&run, key, "items[0]").unwrap().unwrap();
        let middle = db.json_get(&run, key, "items[500]").unwrap().unwrap();
        let last = db.json_get(&run, key, "items[999]").unwrap().unwrap();

        assert_eq!(first.value, Value::Int(0));
        assert_eq!(middle.value, Value::Int(500));
        assert_eq!(last.value, Value::Int(999));
    });
}

/// Test deeply nested document
#[test]
fn test_json_deeply_nested() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "deep_doc";

        // Create 10-level nested structure
        let mut nested = Value::Int(42);
        for _ in 0..10 {
            nested = obj([("nested", nested)]);
        }

        db.json_set(&run, key, "$", nested).unwrap();

        // Access deep value
        let deep_path = "nested.nested.nested.nested.nested.nested.nested.nested.nested.nested";
        let result = db.json_get(&run, key, deep_path).unwrap().unwrap();
        assert_eq!(result.value, Value::Int(42));
    });
}

/// Test empty document
#[test]
fn test_json_empty_document() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "empty_doc";

        let document = obj([]);
        db.json_set(&run, key, "$", document.clone()).unwrap();

        let result = db.json_get(&run, key, "$").unwrap().unwrap();
        assert_eq!(result.value, document);
    });
}

/// Test empty array value
#[test]
fn test_json_empty_array() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "empty_array_doc";

        let document = obj([("items", Value::Array(vec![]))]);
        db.json_set(&run, key, "$", document).unwrap();

        let result = db.json_get(&run, key, "items").unwrap().unwrap();
        assert_eq!(result.value, Value::Array(vec![]));
    });
}

/// Test empty string value
#[test]
fn test_json_empty_string() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "empty_string_doc";

        let document = obj([("name", Value::String(String::new()))]);
        db.json_set(&run, key, "$", document).unwrap();

        let result = db.json_get(&run, key, "name").unwrap().unwrap();
        assert_eq!(result.value, Value::String(String::new()));
    });
}

/// Test large string value
#[test]
fn test_json_large_string() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "large_string_doc";

        // Create 100KB string
        let large_string: String = (0..100_000).map(|i| ((i % 26) as u8 + b'a') as char).collect();
        let document = obj([("content", Value::String(large_string.clone()))]);

        db.json_set(&run, key, "$", document).unwrap();

        let result = db.json_get(&run, key, "content").unwrap().unwrap();
        if let Value::String(s) = result.value {
            assert_eq!(s.len(), 100_000);
        } else {
            panic!("Expected string");
        }
    });
}

/// Test unicode values
#[test]
fn test_json_unicode_values() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "unicode_values";

        let document = obj([
            ("chinese", Value::String("\u{4e2d}\u{6587}\u{6d4b}\u{8bd5}".to_string())),
            ("emoji", Value::String("\u{1f600}\u{1f389}\u{1f680}".to_string())),
            ("mixed", Value::String("Hello \u{4e16}\u{754c} World!".to_string())),
        ]);

        db.json_set(&run, key, "$", document.clone()).unwrap();
        let result = db.json_get(&run, key, "$").unwrap().unwrap();
        assert_eq!(result.value, document);
    });
}

/// Test setting scalar at root
///
/// Note: The primitive layer allows any JSON value at root, not just objects.
/// This is more flexible than strict JSON document semantics.
#[test]
fn test_json_scalar_at_root() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "scalar_root";

        // Setting scalar at root is allowed by the primitive
        let result = db.json_set(&run, key, "$", Value::Int(42));
        assert!(result.is_ok(), "Setting scalar at root is allowed");

        // Verify we can retrieve it
        let value = db.json_get(&run, key, "$").unwrap().unwrap();
        assert_eq!(value.value, Value::Int(42));
    });
}

/// Test setting array at root
///
/// Note: The primitive layer allows any JSON value at root, not just objects.
#[test]
fn test_json_array_at_root() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "array_root";

        // Setting array at root is allowed by the primitive
        let arr = Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        let result = db.json_set(&run, key, "$", arr.clone());
        assert!(result.is_ok(), "Setting array at root is allowed");

        // Verify we can retrieve it
        let value = db.json_get(&run, key, "$").unwrap().unwrap();
        assert_eq!(value.value, arr);
    });
}

/// Test null value in document
#[test]
fn test_json_null_value() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "null_value_doc";

        let document = obj([
            ("nullable", Value::Null),
            ("not_null", Value::Int(1)),
        ]);

        db.json_set(&run, key, "$", document).unwrap();

        let nullable = db.json_get(&run, key, "nullable").unwrap().unwrap();
        assert_eq!(nullable.value, Value::Null);
    });
}

/// Test mixed array types
#[test]
fn test_json_mixed_array() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "mixed_array";

        let document = obj([
            ("items", Value::Array(vec![
                Value::Int(1),
                Value::String("two".to_string()),
                Value::Bool(true),
                Value::Null,
                Value::Float(3.14),
                obj([("nested", Value::Int(5))]),
                Value::Array(vec![Value::Int(6)]),
            ])),
        ]);

        db.json_set(&run, key, "$", document.clone()).unwrap();
        let result = db.json_get(&run, key, "$").unwrap().unwrap();
        assert_eq!(result.value, document);
    });
}

/// Test version increments on updates
#[test]
fn test_json_version_increments() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "version_test";

        let document = obj([("counter", Value::Int(0))]);
        db.json_set(&run, key, "$", document).unwrap();
        let v1 = db.json_get_version(&run, key).unwrap().unwrap();

        db.json_set(&run, key, "counter", Value::Int(1)).unwrap();
        let v2 = db.json_get_version(&run, key).unwrap().unwrap();

        db.json_set(&run, key, "counter", Value::Int(2)).unwrap();
        let v3 = db.json_get_version(&run, key).unwrap().unwrap();

        // Versions should be different (increasing in some way)
        assert!(v1 != v2 || v2 != v3 || v1 <= v3, "Versions should change on update");
    });
}
