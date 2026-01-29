//! High-level typed wrapper for the Executor.
//!
//! The [`Strata`] struct provides a convenient Rust API that wraps the
//! [`Executor`] and [`Command`]/[`Output`] enums with typed method calls.
//!
//! ## Run Context
//!
//! Strata maintains a "current run" context, similar to how git maintains
//! a current branch. All data operations operate on the current run.
//!
//! - Use `checkout_run(name)` to switch to a different run (creates if needed)
//! - Use `current_run()` to get the current run name
//! - Use `list_runs()` to see all available runs
//!
//! By default, Strata starts on the "default" run.
//!
//! # Example
//!
//! ```ignore
//! use strata_executor::Strata;
//! use strata_core::Value;
//!
//! let db = Strata::new(substrate);
//!
//! // Work on the default run
//! db.kv_put("key", Value::String("hello".into()))?;
//!
//! // Switch to a different run
//! db.checkout_run("experiment-1")?;
//! db.kv_put("key", Value::String("different".into()))?;
//!
//! // Switch back
//! db.checkout_run("default")?;
//! assert_eq!(db.kv_get("key")?, Some(Value::String("hello".into())));
//! ```

mod db;
mod event;
mod json;
mod kv;
mod run;
mod state;
mod vector;

use std::sync::Arc;

use strata_engine::Database;

use crate::types::RunId;
use crate::{Command, Error, Executor, Output, Result, Session};

/// High-level typed wrapper for database operations.
///
/// `Strata` provides a convenient Rust API that wraps the executor's
/// command-based interface with typed method calls. It maintains a
/// "current run" context that all data operations use.
///
/// ## Run Context (git-like mental model)
///
/// - **Database** = repository (the whole storage)
/// - **Strata** = working directory (stateful view into the repo)
/// - **Run** = branch (isolated namespace for data)
///
/// Use `checkout_run()` to switch between runs, just like `git checkout`.
pub struct Strata {
    executor: Executor,
    current_run: RunId,
}

impl Strata {
    /// Create a new Strata instance wrapping the given database.
    ///
    /// Starts with the current run set to "default".
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            executor: Executor::new(db),
            current_run: RunId::default(),
        }
    }

    /// Get the underlying executor.
    pub fn executor(&self) -> &Executor {
        &self.executor
    }

    /// Create a new [`Session`] for interactive transaction support.
    ///
    /// The returned session wraps a fresh executor and can manage an
    /// optional open transaction across multiple `execute()` calls.
    pub fn session(db: Arc<Database>) -> Session {
        Session::new(db)
    }

    // =========================================================================
    // Run Context (git-like interface)
    // =========================================================================

    /// Get the current run name.
    ///
    /// Returns the name of the run that all data operations will use.
    pub fn current_run(&self) -> &str {
        self.current_run.as_str()
    }

    /// Switch to a different run.
    ///
    /// If the run doesn't exist, it will be created. This is like
    /// `git checkout -b` - you can switch to any run name and it will
    /// be created if needed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// db.checkout_run("experiment-1")?; // Creates if doesn't exist
    /// db.kv_put("key", Value::Int(42))?; // Data goes to experiment-1
    ///
    /// db.checkout_run("default")?; // Switch back
    /// ```
    pub fn checkout_run(&mut self, run_name: &str) -> Result<()> {
        // Check if run exists
        let exists = match self.executor.execute(Command::RunExists {
            run: RunId::from(run_name),
        })? {
            Output::Bool(b) => b,
            _ => {
                return Err(Error::Internal {
                    reason: "Unexpected output for RunExists".into(),
                })
            }
        };

        // Create if doesn't exist
        if !exists {
            self.executor.execute(Command::RunCreate {
                run_id: Some(run_name.to_string()),
                metadata: None,
            })?;
        }

        self.current_run = RunId::from(run_name);
        Ok(())
    }

    /// Alias for `checkout_run`.
    ///
    /// Switch to a different run (creates if needed).
    pub fn set_run(&mut self, run_name: &str) -> Result<()> {
        self.checkout_run(run_name)
    }

    /// List all available runs.
    ///
    /// Returns a list of run names.
    pub fn list_runs(&self) -> Result<Vec<String>> {
        match self.executor.execute(Command::RunList {
            state: None,
            limit: None,
            offset: None,
        })? {
            Output::RunInfoList(runs) => Ok(runs.into_iter().map(|r| r.info.id.0).collect()),
            _ => Err(Error::Internal {
                reason: "Unexpected output for RunList".into(),
            }),
        }
    }

    /// Delete a run and all its data.
    ///
    /// **WARNING**: This is irreversible! All data in the run will be deleted.
    ///
    /// # Errors
    ///
    /// - Returns an error if trying to delete the current run
    /// - Returns an error if trying to delete the "default" run
    pub fn delete_run(&self, run_name: &str) -> Result<()> {
        // Cannot delete current run
        if run_name == self.current_run.as_str() {
            return Err(Error::ConstraintViolation {
                reason: "Cannot delete the current run. Switch to a different run first.".into(),
            });
        }

        // Cannot delete default run
        if run_name == "default" {
            return Err(Error::ConstraintViolation {
                reason: "Cannot delete the default run".into(),
            });
        }

        match self.executor.execute(Command::RunDelete {
            run: RunId::from(run_name),
        })? {
            Output::Unit => Ok(()),
            _ => Err(Error::Internal {
                reason: "Unexpected output for RunDelete".into(),
            }),
        }
    }

    /// Get the RunId for use in commands.
    ///
    /// This is used internally by the data operation methods.
    pub(crate) fn run_id(&self) -> Option<RunId> {
        Some(self.current_run.clone())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use strata_core::Value;
    use strata_engine::Database;
    use crate::types::*;

    fn create_strata() -> Strata {
        let db = Database::builder().no_durability().open_temp().unwrap();
        Strata::new(db)
    }

    #[test]
    fn test_ping() {
        let db = create_strata();
        let version = db.ping().unwrap();
        assert!(!version.is_empty());
    }

    #[test]
    fn test_info() {
        let db = create_strata();
        let info = db.info().unwrap();
        assert!(!info.version.is_empty());
    }

    #[test]
    fn test_kv_put_get() {
        let db = create_strata();

        let version = db.kv_put("key1", Value::String("hello".into())).unwrap();
        assert!(version > 0);

        let value = db.kv_get("key1").unwrap();
        assert!(value.is_some());
        assert_eq!(value.unwrap(), Value::String("hello".into()));
    }

    #[test]
    fn test_kv_delete() {
        let db = create_strata();

        db.kv_put("key1", Value::Int(42)).unwrap();
        assert!(db.kv_get("key1").unwrap().is_some());

        let existed = db.kv_delete("key1").unwrap();
        assert!(existed);
        assert!(db.kv_get("key1").unwrap().is_none());
    }

    #[test]
    fn test_kv_list() {
        let db = create_strata();

        db.kv_put("user:1", Value::Int(1)).unwrap();
        db.kv_put("user:2", Value::Int(2)).unwrap();
        db.kv_put("task:1", Value::Int(3)).unwrap();

        let user_keys = db.kv_list(Some("user:")).unwrap();
        assert_eq!(user_keys.len(), 2);

        let all_keys = db.kv_list(None).unwrap();
        assert_eq!(all_keys.len(), 3);
    }

    #[test]
    fn test_state_set_get() {
        let db = create_strata();

        db.state_set("cell", Value::String("state".into())).unwrap();
        let value = db.state_read("cell").unwrap();
        assert!(value.is_some());
        assert_eq!(value.unwrap().value, Value::String("state".into()));
    }

    #[test]
    fn test_event_append_range() {
        let db = create_strata();

        // Event payloads must be Objects
        db.event_append("stream", Value::Object(
            [("value".to_string(), Value::Int(1))].into_iter().collect()
        )).unwrap();
        db.event_append("stream", Value::Object(
            [("value".to_string(), Value::Int(2))].into_iter().collect()
        )).unwrap();

        let events = db.event_range("stream", None, None, None).unwrap();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_vector_operations() {
        let db = create_strata();

        db.vector_create_collection("vecs", 4u64, DistanceMetric::Cosine).unwrap();
        db.vector_upsert("vecs", "v1", vec![1.0, 0.0, 0.0, 0.0], None).unwrap();
        db.vector_upsert("vecs", "v2", vec![0.0, 1.0, 0.0, 0.0], None).unwrap();

        let matches = db.vector_search("vecs", vec![1.0, 0.0, 0.0, 0.0], 10u64).unwrap();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].key, "v1");
    }

    #[test]
    fn test_run_create_list() {
        let db = create_strata();

        let (info, _version) = db.run_create(
            Some("550e8400-e29b-41d4-a716-446655440099".to_string()),
            None,
        ).unwrap();
        assert_eq!(info.id.as_str(), "550e8400-e29b-41d4-a716-446655440099");

        let runs = db.run_list(None, None, None).unwrap();
        assert!(!runs.is_empty());
    }

    // =========================================================================
    // Run Context Tests
    // =========================================================================

    #[test]
    fn test_current_run_default() {
        let db = create_strata();
        assert_eq!(db.current_run(), "default");
    }

    #[test]
    fn test_checkout_run_creates_if_not_exists() {
        let mut db = create_strata();

        // Switch to a new run (should be created)
        db.checkout_run("experiment-1").unwrap();
        assert_eq!(db.current_run(), "experiment-1");

        // Verify the run exists
        assert!(db.run_exists("experiment-1").unwrap());
    }

    #[test]
    fn test_checkout_run_switches_to_existing() {
        let mut db = create_strata();

        // Create a run first
        db.run_create(Some("my-run".to_string()), None).unwrap();

        // Switch to it
        db.checkout_run("my-run").unwrap();
        assert_eq!(db.current_run(), "my-run");
    }

    #[test]
    fn test_set_run_alias() {
        let mut db = create_strata();

        db.set_run("another-run").unwrap();
        assert_eq!(db.current_run(), "another-run");
    }

    #[test]
    fn test_list_runs() {
        let mut db = create_strata();

        // Create a few runs
        db.checkout_run("run-a").unwrap();
        db.checkout_run("run-b").unwrap();
        db.checkout_run("run-c").unwrap();

        let runs = db.list_runs().unwrap();
        assert!(runs.contains(&"run-a".to_string()));
        assert!(runs.contains(&"run-b".to_string()));
        assert!(runs.contains(&"run-c".to_string()));
    }

    #[test]
    fn test_delete_run() {
        let mut db = create_strata();

        // Create and switch to a run
        db.checkout_run("to-delete").unwrap();

        // Switch away before deleting
        db.checkout_run("default").unwrap();

        // Delete the run
        db.delete_run("to-delete").unwrap();

        // Verify it's gone
        assert!(!db.run_exists("to-delete").unwrap());
    }

    #[test]
    fn test_delete_current_run_fails() {
        let mut db = create_strata();

        db.checkout_run("current-run").unwrap();

        // Trying to delete the current run should fail
        let result = db.delete_run("current-run");
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_default_run_fails() {
        let db = create_strata();

        // Trying to delete the default run should fail
        let result = db.delete_run("default");
        assert!(result.is_err());
    }

    #[test]
    fn test_run_context_data_isolation() {
        let mut db = create_strata();

        // Put data in default run
        db.kv_put("key", Value::String("default-value".into())).unwrap();

        // Switch to another run
        db.checkout_run("experiment").unwrap();

        // The key should not exist in this run
        assert!(db.kv_get("key").unwrap().is_none());

        // Put different data
        db.kv_put("key", Value::String("experiment-value".into())).unwrap();

        // Switch back to default
        db.checkout_run("default").unwrap();

        // Original value should still be there
        let value = db.kv_get("key").unwrap();
        assert_eq!(value, Some(Value::String("default-value".into())));
    }

    #[test]
    fn test_run_context_isolation_all_primitives() {
        let mut db = create_strata();

        // Put data in default run
        db.kv_put("kv-key", Value::Int(1)).unwrap();
        db.state_set("state-cell", Value::Int(10)).unwrap();
        db.event_append("stream", Value::Object(
            [("x".to_string(), Value::Int(100))].into_iter().collect()
        )).unwrap();

        // Switch to another run
        db.checkout_run("isolated").unwrap();

        // None of the data should exist in this run
        assert!(db.kv_get("kv-key").unwrap().is_none());
        assert!(db.state_read("state-cell").unwrap().is_none());
        assert_eq!(db.event_len("stream").unwrap(), 0);
    }
}
