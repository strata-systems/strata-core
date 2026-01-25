//! Key-value store primitive.
//!
//! The KV primitive provides simple key-value storage with versioning,
//! history access, and atomic operations.
//!
//! # Example
//!
//! ```ignore
//! use strata::prelude::*;
//!
//! let db = Strata::open("./my-db")?;
//!
//! // Simple operations (default run)
//! db.kv.set("name", "Alice")?;
//! let name = db.kv.get("name")?;
//!
//! // Run-scoped operations
//! let run = db.runs.create("my-run")?;
//! db.kv.set_in(&run, "counter", 0)?;
//!
//! // Full control (returns version)
//! let version = db.kv.put(&run, "counter", 1)?;
//! ```

use crate::error::Result;
use crate::types::{run_id_to_api, RunId, Value, Version, Versioned};
use std::sync::Arc;

use strata_api::substrate::{ApiRunId, KVStore, KVStoreBatch};

/// Key-value store operations.
///
/// Access via `db.kv`.
pub struct KV {
    #[allow(dead_code)]
    db: Arc<strata_engine::Database>,
    substrate: strata_api::substrate::SubstrateImpl,
}

impl KV {
    pub(crate) fn new(db: Arc<strata_engine::Database>) -> Self {
        let substrate = strata_api::substrate::SubstrateImpl::new(db.clone());
        Self { db, substrate }
    }

    // =========================================================================
    // Simple API (default run)
    // =========================================================================

    /// Set a value.
    ///
    /// Uses the default run. For run-scoped operations, use `set_in`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// db.kv.set("name", "Alice")?;
    /// db.kv.set("age", 30)?;
    /// ```
    pub fn set(&self, key: &str, value: impl Into<Value>) -> Result<()> {
        let run = ApiRunId::default();
        self.substrate.kv_put(&run, key, value.into())?;
        Ok(())
    }

    /// Get a value.
    ///
    /// Returns `None` if the key doesn't exist.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(versioned) = db.kv.get("name")? {
    ///     println!("Name: {:?}", versioned.value);
    /// }
    /// ```
    pub fn get(&self, key: &str) -> Result<Option<Versioned<Value>>> {
        let run = ApiRunId::default();
        Ok(self.substrate.kv_get(&run, key)?)
    }

    /// Delete a key.
    ///
    /// Returns `true` if the key existed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let existed = db.kv.delete("name")?;
    /// ```
    pub fn delete(&self, key: &str) -> Result<bool> {
        let run = ApiRunId::default();
        Ok(self.substrate.kv_delete(&run, key)?)
    }

    /// Check if a key exists.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if db.kv.exists("name")? {
    ///     println!("Key exists");
    /// }
    /// ```
    pub fn exists(&self, key: &str) -> Result<bool> {
        let run = ApiRunId::default();
        Ok(self.substrate.kv_exists(&run, key)?)
    }

    // =========================================================================
    // Run-scoped API
    // =========================================================================

    /// Set a value in a specific run.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let run = db.runs.create("my-run")?;
    /// db.kv.set_in(&run, "key", "value")?;
    /// ```
    pub fn set_in(&self, run: &RunId, key: &str, value: impl Into<Value>) -> Result<()> {
        let api_run = run_id_to_api(run);
        self.substrate.kv_put(&api_run, key, value.into())?;
        Ok(())
    }

    /// Get a value from a specific run.
    pub fn get_in(&self, run: &RunId, key: &str) -> Result<Option<Versioned<Value>>> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.kv_get(&api_run, key)?)
    }

    /// Delete a key from a specific run.
    pub fn delete_in(&self, run: &RunId, key: &str) -> Result<bool> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.kv_delete(&api_run, key)?)
    }

    /// Check if a key exists in a specific run.
    pub fn exists_in(&self, run: &RunId, key: &str) -> Result<bool> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.kv_exists(&api_run, key)?)
    }

    // =========================================================================
    // Full control API (returns version)
    // =========================================================================

    /// Put a value and return the version.
    ///
    /// This is the full-control method that returns version information.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let version = db.kv.put(&run, "key", "value")?;
    /// println!("Written at version: {:?}", version);
    /// ```
    pub fn put(&self, run: &RunId, key: &str, value: impl Into<Value>) -> Result<Version> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.kv_put(&api_run, key, value.into())?)
    }

    /// Get a value at a specific version.
    ///
    /// Returns the value as it existed at that version.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let old_value = db.kv.get_at(&run, "key", version)?;
    /// ```
    pub fn get_at(&self, run: &RunId, key: &str, version: Version) -> Result<Versioned<Value>> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.kv_get_at(&api_run, key, version)?)
    }

    /// Get version history for a key.
    ///
    /// Returns historical versions, newest first.
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum versions to return
    /// * `before` - Return versions older than this (for pagination)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let history = db.kv.history(&run, "counter", Some(10), None)?;
    /// for entry in history {
    ///     println!("{:?} at version {:?}", entry.value, entry.version);
    /// }
    /// ```
    pub fn history(
        &self,
        run: &RunId,
        key: &str,
        limit: Option<u64>,
        before: Option<Version>,
    ) -> Result<Vec<Versioned<Value>>> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.kv_history(&api_run, key, limit, before)?)
    }

    // =========================================================================
    // Atomic operations
    // =========================================================================

    /// Atomic increment.
    ///
    /// Increments the value by `delta`. Creates the key with value `delta` if
    /// it doesn't exist.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let new_value = db.kv.incr(&run, "counter", 1)?;
    /// ```
    pub fn incr(&self, run: &RunId, key: &str, delta: i64) -> Result<i64> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.kv_incr(&api_run, key, delta)?)
    }

    /// Compare-and-swap by version.
    ///
    /// Sets the value only if the current version matches `expected`.
    /// Pass `None` as expected to succeed only if the key doesn't exist.
    ///
    /// Returns `true` if the swap succeeded.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Get current version
    /// let current = db.kv.get_in(&run, "key")?;
    /// let version = current.map(|v| v.version);
    ///
    /// // Try to update
    /// if db.kv.cas(&run, "key", version, "new-value")? {
    ///     println!("Updated successfully");
    /// } else {
    ///     println!("Concurrent modification detected");
    /// }
    /// ```
    pub fn cas(
        &self,
        run: &RunId,
        key: &str,
        expected: Option<Version>,
        value: impl Into<Value>,
    ) -> Result<bool> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.kv_cas_version(&api_run, key, expected, value.into())?)
    }

    // =========================================================================
    // Batch operations
    // =========================================================================

    /// Get multiple values.
    ///
    /// Returns values in the same order as keys. Missing keys return `None`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let values = db.kv.mget(&run, &["key1", "key2", "key3"])?;
    /// ```
    pub fn mget(
        &self,
        run: &RunId,
        keys: &[&str],
    ) -> Result<Vec<Option<Versioned<Value>>>> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.kv_mget(&api_run, keys)?)
    }

    /// Set multiple values atomically.
    ///
    /// All values are written in the same transaction.
    ///
    /// # Example
    ///
    /// ```ignore
    /// db.kv.mset(&run, &[
    ///     ("key1", "value1".into()),
    ///     ("key2", "value2".into()),
    /// ])?;
    /// ```
    pub fn mset(&self, run: &RunId, entries: &[(&str, Value)]) -> Result<Version> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.kv_mput(&api_run, entries)?)
    }

    /// Delete multiple keys atomically.
    ///
    /// Returns the count of keys that existed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let deleted = db.kv.mdelete(&run, &["key1", "key2"])?;
    /// ```
    pub fn mdelete(&self, run: &RunId, keys: &[&str]) -> Result<u64> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.kv_mdelete(&api_run, keys)?)
    }

    // =========================================================================
    // Key listing
    // =========================================================================

    /// List keys with optional prefix filter.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // List all keys
    /// let all_keys = db.kv.keys(&run, "", None)?;
    ///
    /// // List keys with prefix
    /// let user_keys = db.kv.keys(&run, "user:", Some(100))?;
    /// ```
    pub fn keys(&self, run: &RunId, prefix: &str, limit: Option<usize>) -> Result<Vec<String>> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.kv_keys(&api_run, prefix, limit)?)
    }
}
