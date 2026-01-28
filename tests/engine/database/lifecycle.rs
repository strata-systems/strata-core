//! Database Lifecycle Tests
//!
//! Tests for database creation, opening, closing, and reopening.

use crate::common::*;

// ============================================================================
// Ephemeral Database
// ============================================================================

#[test]
fn ephemeral_database_is_functional() {
    let db = Database::builder()
        .no_durability()
        .open_temp()
        .expect("ephemeral database");

    let run_id = RunId::new();
    let kv = KVStore::new(std::sync::Arc::new(db));

    kv.put(&run_id, "key", Value::Int(42)).unwrap();
    let result = kv.get(&run_id, "key").unwrap();

    assert!(result.is_some());
    assert_eq!(result.unwrap().value, Value::Int(42));
}

#[test]
fn ephemeral_database_data_is_lost_on_drop() {
    let run_id = RunId::new();
    let key = unique_key();

    // Write data
    {
        let db = Database::builder()
            .no_durability()
            .open_temp()
            .expect("ephemeral database");
        let kv = KVStore::new(std::sync::Arc::new(db));
        kv.put(&run_id, &key, Value::Int(42)).unwrap();
    }

    // New ephemeral database has no data
    let db = Database::builder()
        .no_durability()
        .open_temp()
        .expect("ephemeral database");
    let kv = KVStore::new(std::sync::Arc::new(db));
    let result = kv.get(&run_id, &key).unwrap();

    assert!(result.is_none());
}

// ============================================================================
// Persistent Database
// ============================================================================

#[test]
fn persistent_database_creates_directory() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_db");

    assert!(!db_path.exists());

    let _db = Database::builder()
        .path(&db_path)
        .buffered()
        .open()
        .expect("create database");

    assert!(db_path.exists());
}

#[test]
fn persistent_database_survives_reopen() {
    let mut test_db = TestDb::new();
    let run_id = test_db.run_id;
    let key = unique_key();

    // Write data
    {
        let kv = test_db.kv();
        kv.put(&run_id, &key, Value::Int(42)).unwrap();
    }

    // Force sync and reopen
    test_db.db.shutdown().unwrap();
    test_db.reopen();

    // Verify data persisted
    let kv = test_db.kv();
    let result = kv.get(&run_id, &key).unwrap();

    assert!(result.is_some());
    assert_eq!(result.unwrap().value, Value::Int(42));
}

#[test]
fn persistent_database_multiple_reopens() {
    let mut test_db = TestDb::new();
    let run_id = test_db.run_id;

    // Write and reopen multiple times
    for i in 0..5 {
        let kv = test_db.kv();
        kv.put(&run_id, &format!("key_{}", i), Value::Int(i)).unwrap();
        test_db.db.shutdown().unwrap();
        test_db.reopen();
    }

    // Verify all data present
    let kv = test_db.kv();
    for i in 0..5 {
        let result = kv.get(&run_id, &format!("key_{}", i)).unwrap();
        assert!(result.is_some(), "key_{} should exist", i);
        assert_eq!(result.unwrap().value, Value::Int(i));
    }
}

// ============================================================================
// Builder API
// ============================================================================

#[test]
fn builder_no_durability_open_temp_uses_temp_files() {
    // no_durability().open_temp() still creates temp files on disk
    // It's NOT truly ephemeral
    let db = Database::builder()
        .no_durability()
        .open_temp()
        .unwrap();

    assert!(!db.is_ephemeral()); // Uses temp files, not purely in-memory
}

#[test]
fn database_ephemeral_is_truly_ephemeral() {
    // Database::ephemeral() creates a purely in-memory database
    let db = Database::ephemeral().expect("ephemeral database");
    assert!(db.is_ephemeral());
}

#[test]
fn builder_creates_persistent_with_path() {
    let temp_dir = tempfile::tempdir().unwrap();

    let db = Database::builder()
        .path(temp_dir.path())
        .buffered()
        .open()
        .unwrap();

    assert!(!db.is_ephemeral());
}

#[test]
fn builder_strict_mode() {
    let temp_dir = tempfile::tempdir().unwrap();

    let db = Database::builder()
        .path(temp_dir.path())
        .strict()
        .open()
        .unwrap();

    // Verify it works
    let run_id = RunId::new();
    let kv = KVStore::new(std::sync::Arc::new(db));
    kv.put(&run_id, "key", Value::Int(1)).unwrap();
    assert!(kv.get(&run_id, "key").unwrap().is_some());
}

#[test]
fn builder_buffered_mode() {
    let temp_dir = tempfile::tempdir().unwrap();

    let db = Database::builder()
        .path(temp_dir.path())
        .buffered()
        .open()
        .unwrap();

    // Verify it works
    let run_id = RunId::new();
    let kv = KVStore::new(std::sync::Arc::new(db));
    kv.put(&run_id, "key", Value::Int(1)).unwrap();
    assert!(kv.get(&run_id, "key").unwrap().is_some());
}

// ============================================================================
// Shutdown
// ============================================================================

#[test]
fn shutdown_is_idempotent() {
    let test_db = TestDb::new();

    // Multiple shutdowns should not panic
    test_db.db.shutdown().unwrap();
    // Second shutdown - should be safe
    let _ = test_db.db.shutdown();
}

#[test]
fn is_open_reflects_state() {
    let temp_dir = tempfile::tempdir().unwrap();

    let db = Database::builder()
        .path(temp_dir.path())
        .buffered()
        .open()
        .unwrap();

    assert!(db.is_open());

    db.shutdown().unwrap();

    assert!(!db.is_open());
}
