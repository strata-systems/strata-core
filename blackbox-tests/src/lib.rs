//! Black Box Tests for StrataDB
//!
//! These tests only use the public API - no internal crate access.
//! This simulates what an end user would experience.

#[cfg(test)]
mod tests {
    use stratadb::prelude::*;
    use tempfile::TempDir;

    // ========================================================================
    // Database Lifecycle
    // ========================================================================

    #[test]
    fn user_can_open_database() {
        let dir = TempDir::new().unwrap();
        let db = Strata::open(dir.path().join("mydb")).unwrap();
        assert!(dir.path().join("mydb").exists());
        drop(db);
    }

    #[test]
    fn user_can_use_builder() {
        let db = StrataBuilder::new().no_durability().open_temp().unwrap();
        drop(db);
    }

    #[test]
    fn user_can_create_ephemeral_database() {
        // Ephemeral databases have NO disk files at all
        let db = Strata::ephemeral().unwrap();
        assert!(db.is_ephemeral());

        // All operations work normally
        db.kv.set("key", "value").unwrap();
        let result = db.kv.get("key").unwrap();
        assert_eq!(result.unwrap().as_str(), Some("value"));

        // Data is lost when db is dropped (no way to verify, but we can demonstrate it works)
        drop(db);
    }

    #[test]
    fn ephemeral_vs_temp_comparison() {
        // Ephemeral: no disk files
        let ephemeral = Strata::ephemeral().unwrap();
        assert!(ephemeral.is_ephemeral());

        // Temp with no_durability: disk files in temp dir, but no fsync
        let temp = StrataBuilder::new().no_durability().open_temp().unwrap();
        assert!(!temp.is_ephemeral());

        // Both support all operations
        ephemeral.kv.set("key", "ephemeral").unwrap();
        temp.kv.set("key", "temp").unwrap();

        assert_eq!(ephemeral.kv.get("key").unwrap().unwrap().as_str(), Some("ephemeral"));
        assert_eq!(temp.kv.get("key").unwrap().unwrap().as_str(), Some("temp"));
    }

    // ========================================================================
    // Key-Value Operations
    // ========================================================================

    #[test]
    fn user_can_set_and_get_string() {
        let db = Strata::ephemeral().unwrap();

        db.kv.set("greeting", "Hello, World!").unwrap();

        let result = db.kv.get("greeting").unwrap();
        assert!(result.is_some());
        // Convenience: .as_str() delegates to inner value
        assert_eq!(result.unwrap().as_str(), Some("Hello, World!"));
    }

    #[test]
    fn user_can_set_and_get_integer() {
        let db = Strata::ephemeral().unwrap();

        db.kv.set("count", 42i64).unwrap();

        let result = db.kv.get("count").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().as_int(), Some(42));
    }

    #[test]
    fn user_can_check_key_exists() {
        let db = Strata::ephemeral().unwrap();

        assert!(!db.kv.exists("missing").unwrap());

        db.kv.set("present", "here").unwrap();
        assert!(db.kv.exists("present").unwrap());
    }

    #[test]
    fn user_can_delete_key() {
        let db = Strata::ephemeral().unwrap();

        db.kv.set("to_delete", "goodbye").unwrap();
        assert!(db.kv.exists("to_delete").unwrap());

        db.kv.delete("to_delete").unwrap();
        assert!(!db.kv.exists("to_delete").unwrap());
    }

    #[test]
    fn get_missing_key_returns_none() {
        let db = Strata::ephemeral().unwrap();

        let result = db.kv.get("does_not_exist").unwrap();
        assert!(result.is_none());
    }

    // ========================================================================
    // JSON Document Operations
    // ========================================================================

    #[test]
    fn user_can_store_json_document() {
        let db = Strata::ephemeral().unwrap();

        let mut doc = std::collections::HashMap::new();
        doc.insert("name".to_string(), Value::from("Alice"));
        doc.insert("age".to_string(), Value::from(30i64));

        db.json.set("user:1", doc).unwrap();

        let result = db.json.get("user:1").unwrap();
        assert!(result.is_some());

        let value = result.unwrap().value;
        let obj = value.as_object().unwrap();
        assert_eq!(obj.get("name").unwrap().as_str(), Some("Alice"));
        assert_eq!(obj.get("age").unwrap().as_int(), Some(30));
    }

    #[test]
    fn user_can_use_json_macro_for_ergonomic_syntax() {
        let db = Strata::ephemeral().unwrap();

        // The idiomatic Rust way - use json! macro
        db.json.set("user:1", json!({
            "name": "Alice",
            "age": 30,
            "active": true,
            "preferences": {
                "theme": "dark",
                "notifications": true
            },
            "tags": ["admin", "user"]
        })).unwrap();

        let result = db.json.get("user:1").unwrap();
        assert!(result.is_some());

        let value = result.unwrap().value;
        let obj = value.as_object().unwrap();
        assert_eq!(obj.get("name").unwrap().as_str(), Some("Alice"));
        assert_eq!(obj.get("age").unwrap().as_int(), Some(30));
        assert_eq!(obj.get("active").unwrap().as_bool(), Some(true));

        // Nested object
        let prefs = obj.get("preferences").unwrap().as_object().unwrap();
        assert_eq!(prefs.get("theme").unwrap().as_str(), Some("dark"));

        // Array
        let tags = obj.get("tags").unwrap().as_array().unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].as_str(), Some("admin"));
    }

    // ========================================================================
    // Event Stream Operations
    // ========================================================================

    #[test]
    fn user_can_append_and_read_events() {
        let db = Strata::ephemeral().unwrap();

        // Use json! macro for clean event creation
        db.events.append("activity", json!({"action": "login"})).unwrap();
        db.events.append("activity", json!({"action": "click"})).unwrap();

        let events = db.events.read("activity", 10).unwrap();
        assert_eq!(events.len(), 2);
    }

    // ========================================================================
    // State Cell Operations
    // ========================================================================

    #[test]
    fn user_can_use_state_cells() {
        let db = Strata::ephemeral().unwrap();

        db.state.set("status", "active").unwrap();

        let result = db.state.get("status").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().as_str(), Some("active"));

        // Update
        db.state.set("status", "inactive").unwrap();
        let result = db.state.get("status").unwrap();
        assert_eq!(result.unwrap().as_str(), Some("inactive"));
    }

    // ========================================================================
    // Vector Operations
    // ========================================================================

    #[test]
    fn user_can_create_vector_collection() {
        let db = Strata::ephemeral().unwrap();
        let run = db.runs.create(None).unwrap();

        db.vectors
            .create_collection(&run, "embeddings", 4, DistanceMetric::Cosine)
            .unwrap();

        let collections = db.vectors.list_collections(&run).unwrap();
        assert!(collections.iter().any(|c| c.name == "embeddings"));
    }

    #[test]
    fn user_can_upsert_and_search_vectors() {
        let db = Strata::ephemeral().unwrap();
        let run = db.runs.create(None).unwrap();

        db.vectors
            .create_collection(&run, "test", 4, DistanceMetric::Cosine)
            .unwrap();

        // Insert vectors
        db.vectors
            .upsert(&run, "test", "vec1", &[1.0, 0.0, 0.0, 0.0], None)
            .unwrap();
        db.vectors
            .upsert(&run, "test", "vec2", &[0.0, 1.0, 0.0, 0.0], None)
            .unwrap();
        db.vectors
            .upsert(&run, "test", "vec3", &[0.7, 0.7, 0.0, 0.0], None)
            .unwrap();

        // Search for similar to vec1
        let results = db
            .vectors
            .search(&run, "test", &[1.0, 0.0, 0.0, 0.0], 3, None)
            .unwrap();

        assert!(!results.is_empty());
        // First result should be exact match
        assert_eq!(results[0].key, "vec1");
    }

    // ========================================================================
    // Run Operations
    // ========================================================================

    #[test]
    fn user_can_create_and_close_runs() {
        let db = Strata::ephemeral().unwrap();

        let run = db.runs.create(None).unwrap();
        assert!(db.runs.exists(&run).unwrap());

        db.runs.close(&run).unwrap();

        let info = db.runs.get(&run).unwrap().unwrap();
        assert!(!info.value.state.is_active());
    }

    #[test]
    fn user_can_isolate_data_in_runs() {
        let db = Strata::ephemeral().unwrap();

        let run1 = db.runs.create(None).unwrap();
        let run2 = db.runs.create(None).unwrap();

        // Set in run1
        db.kv.set_in(&run1, "key", "value1").unwrap();

        // Set in run2
        db.kv.set_in(&run2, "key", "value2").unwrap();

        // Should be isolated
        let v1 = db.kv.get_in(&run1, "key").unwrap().unwrap();
        let v2 = db.kv.get_in(&run2, "key").unwrap().unwrap();

        assert_eq!(v1.as_str(), Some("value1"));
        assert_eq!(v2.as_str(), Some("value2"));
    }

    // ========================================================================
    // Persistence
    // ========================================================================

    #[test]
    fn data_persists_after_reopen() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("persist_test");

        // Write data
        {
            let db = Strata::open(&db_path).unwrap();
            db.kv.set("persistent", "data").unwrap();
            db.flush().unwrap();
        }

        // Reopen and verify
        {
            let db = Strata::open(&db_path).unwrap();
            let result = db.kv.get("persistent").unwrap();
            assert!(result.is_some());
            assert_eq!(result.unwrap().as_str(), Some("data"));
        }
    }

    // ========================================================================
    // Error Cases
    // ========================================================================

    #[test]
    fn search_nonexistent_collection_fails() {
        let db = Strata::ephemeral().unwrap();
        let run = db.runs.create(None).unwrap();

        let result = db.vectors.search(&run, "nonexistent", &[1.0, 0.0], 5, None);
        assert!(result.is_err());
    }

    // ========================================================================
    // Integration: AI Agent Workflow
    // ========================================================================

    #[test]
    fn ai_agent_memory_workflow() {
        let db = Strata::ephemeral().unwrap();

        // Agent creates a conversation run
        let conversation = db.runs.create(None).unwrap();

        // Store conversation context
        db.kv.set_in(&conversation, "user_name", "Alice").unwrap();
        db.kv.set_in(&conversation, "topic", "weather").unwrap();

        // Log agent actions as events - json! macro makes this clean
        db.events.append_in(&conversation, "trace", json!({
            "type": "user_message",
            "content": "What's the weather?"
        })).unwrap();

        db.events.append_in(&conversation, "trace", json!({
            "type": "tool_call",
            "tool": "weather_api"
        })).unwrap();

        // Verify we can read everything back
        let name = db.kv.get_in(&conversation, "user_name").unwrap().unwrap();
        assert_eq!(name.as_str(), Some("Alice"));

        let events = db.events.read_in(&conversation, "trace", 10).unwrap();
        assert_eq!(events.len(), 2);

        // Close the conversation
        db.runs.close(&conversation).unwrap();
    }
}
