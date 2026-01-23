//! JsonStore Path Operations Tests
//!
//! Tests for path-based navigation:
//! - Object field access ($.field)
//! - Array index access ($.array[0])
//! - Nested path access
//! - Array append ($.array[-])

use crate::*;

/// Test simple object field access
#[test]
fn test_json_path_object_field() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "path_test";

        let document = obj([
            ("a", Value::Int(1)),
            ("b", Value::Int(2)),
        ]);
        db.json_set(&run, key, "$", document).unwrap();

        // Access individual fields
        let a = db.json_get(&run, key, "a").unwrap().unwrap();
        let b = db.json_get(&run, key, "b").unwrap().unwrap();
        assert_eq!(a.value, Value::Int(1));
        assert_eq!(b.value, Value::Int(2));
    });
}

/// Test array index access
#[test]
fn test_json_path_array_index() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "array_doc";

        let document = obj([
            ("items", Value::Array(vec![
                Value::String("first".to_string()),
                Value::String("second".to_string()),
                Value::String("third".to_string()),
            ])),
        ]);
        db.json_set(&run, key, "$", document).unwrap();

        // Access array elements
        let item0 = db.json_get(&run, key, "items[0]").unwrap().unwrap();
        let item1 = db.json_get(&run, key, "items[1]").unwrap().unwrap();
        let item2 = db.json_get(&run, key, "items[2]").unwrap().unwrap();

        assert_eq!(item0.value, Value::String("first".to_string()));
        assert_eq!(item1.value, Value::String("second".to_string()));
        assert_eq!(item2.value, Value::String("third".to_string()));
    });
}

/// Test deeply nested path access
#[test]
fn test_json_path_deep_nesting() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "deep_doc";

        let document = obj([
            ("level1", obj([
                ("level2", obj([
                    ("level3", obj([
                        ("value", Value::String("deep".to_string())),
                    ])),
                ])),
            ])),
        ]);
        db.json_set(&run, key, "$", document).unwrap();

        // Access deeply nested value
        let deep = db.json_get(&run, key, "level1.level2.level3.value").unwrap().unwrap();
        assert_eq!(deep.value, Value::String("deep".to_string()));
    });
}

/// Test setting value at nested path (creates intermediates)
#[test]
fn test_json_path_set_nested() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "set_nested_doc";

        // Create empty document
        let document = obj([]);
        db.json_set(&run, key, "$", document).unwrap();

        // Set a nested path (should create intermediates)
        db.json_set(&run, key, "user.profile.name", Value::String("Charlie".to_string())).unwrap();

        // Verify the nested structure was created
        let name = db.json_get(&run, key, "user.profile.name").unwrap().unwrap();
        assert_eq!(name.value, Value::String("Charlie".to_string()));
    });
}

/// Test array element modification
#[test]
fn test_json_path_modify_array_element() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "modify_array_doc";

        let document = obj([
            ("items", Value::Array(vec![
                Value::Int(1),
                Value::Int(2),
                Value::Int(3),
            ])),
        ]);
        db.json_set(&run, key, "$", document).unwrap();

        // Modify middle element
        db.json_set(&run, key, "items[1]", Value::Int(200)).unwrap();

        // Verify modification
        let items = db.json_get(&run, key, "items").unwrap().unwrap();
        assert_eq!(items.value, Value::Array(vec![
            Value::Int(1),
            Value::Int(200),
            Value::Int(3),
        ]));
    });
}

/// Test array append with [-] syntax
///
/// Note: The `[-]` array append syntax is documented in the API but not yet
/// implemented in the path parser. This test is ignored until that feature is added.
#[test]
#[ignore = "array append syntax [-] not yet implemented in path parser"]
fn test_json_path_array_append() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "append_array_doc";

        let document = obj([
            ("items", Value::Array(vec![
                Value::Int(1),
                Value::Int(2),
            ])),
        ]);
        db.json_set(&run, key, "$", document).unwrap();

        // Append to array
        db.json_set(&run, key, "items[-]", Value::Int(3)).unwrap();

        // Verify append
        let items = db.json_get(&run, key, "items").unwrap().unwrap();
        assert_eq!(items.value, Value::Array(vec![
            Value::Int(1),
            Value::Int(2),
            Value::Int(3),
        ]));
    });
}

/// Test mixed object and array path
#[test]
fn test_json_path_mixed() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "mixed_doc";

        let document = obj([
            ("users", Value::Array(vec![
                obj([("name", Value::String("User1".to_string()))]),
                obj([("name", Value::String("User2".to_string()))]),
            ])),
        ]);
        db.json_set(&run, key, "$", document).unwrap();

        // Access object within array
        let name1 = db.json_get(&run, key, "users[0].name").unwrap().unwrap();
        let name2 = db.json_get(&run, key, "users[1].name").unwrap().unwrap();

        assert_eq!(name1.value, Value::String("User1".to_string()));
        assert_eq!(name2.value, Value::String("User2".to_string()));
    });
}

/// Test deleting array element (shifts remaining)
#[test]
fn test_json_path_delete_array_element() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "delete_array_doc";

        let document = obj([
            ("items", Value::Array(vec![
                Value::String("a".to_string()),
                Value::String("b".to_string()),
                Value::String("c".to_string()),
            ])),
        ]);
        db.json_set(&run, key, "$", document).unwrap();

        // Delete middle element
        db.json_delete(&run, key, "items[1]").unwrap();

        // Verify remaining elements shifted
        let items = db.json_get(&run, key, "items").unwrap().unwrap();
        assert_eq!(items.value, Value::Array(vec![
            Value::String("a".to_string()),
            Value::String("c".to_string()),
        ]));
    });
}

/// Test deleting nested object field
#[test]
fn test_json_path_delete_nested_field() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "delete_nested_doc";

        let document = obj([
            ("user", obj([
                ("name", Value::String("Dave".to_string())),
                ("email", Value::String("dave@example.com".to_string())),
            ])),
        ]);
        db.json_set(&run, key, "$", document).unwrap();

        // Delete nested field
        db.json_delete(&run, key, "user.email").unwrap();

        // Verify field deleted but parent remains
        assert!(db.json_get(&run, key, "user.email").unwrap().is_none());
        assert!(db.json_get(&run, key, "user.name").unwrap().is_some());
    });
}

/// Test out-of-bounds array access
#[test]
fn test_json_path_array_out_of_bounds() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "oob_doc";

        let document = obj([
            ("items", Value::Array(vec![
                Value::Int(1),
                Value::Int(2),
            ])),
        ]);
        db.json_set(&run, key, "$", document).unwrap();

        // Access out of bounds - should return None
        let result = db.json_get(&run, key, "items[99]").unwrap();
        assert!(result.is_none(), "Out of bounds access should return None");
    });
}

/// Test replacing entire array
#[test]
fn test_json_path_replace_array() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let key = "replace_array_doc";

        let document = obj([
            ("items", Value::Array(vec![
                Value::Int(1),
                Value::Int(2),
            ])),
        ]);
        db.json_set(&run, key, "$", document).unwrap();

        // Replace entire array
        let new_array = Value::Array(vec![
            Value::String("new".to_string()),
        ]);
        db.json_set(&run, key, "items", new_array.clone()).unwrap();

        let result = db.json_get(&run, key, "items").unwrap().unwrap();
        assert_eq!(result.value, new_array);
    });
}
