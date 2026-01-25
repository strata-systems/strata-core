//! Run lifecycle management.
//!
//! The Runs primitive provides run creation, listing, and lifecycle operations.
//! Runs are isolated namespaces for organizing data.

use crate::error::Result;
use crate::types::{run_id_to_api, RetentionPolicy, RunId, RunInfo, RunState, Value, Version, Versioned};
use std::sync::Arc;

use strata_api::substrate::{ApiRunId, RunIndex};

/// Run lifecycle operations.
///
/// Access via `db.runs`.
pub struct Runs {
    #[allow(dead_code)]
    db: Arc<strata_engine::Database>,
    substrate: strata_api::substrate::SubstrateImpl,
}

impl Runs {
    pub(crate) fn new(db: Arc<strata_engine::Database>) -> Self {
        let substrate = strata_api::substrate::SubstrateImpl::new(db.clone());
        Self { db, substrate }
    }

    // =========================================================================
    // Run lifecycle
    // =========================================================================

    /// Create a new run.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let run = db.runs.create(None)?;
    /// ```
    pub fn create(&self, metadata: Option<Value>) -> Result<RunId> {
        let (info, _version) = self.substrate.run_create(None, metadata)?;
        Ok(info.run_id.to_run_id())
    }

    /// Create a run with a specific ID.
    pub fn create_with_id(&self, run_id: &ApiRunId, metadata: Option<Value>) -> Result<RunId> {
        let (info, _version) = self.substrate.run_create(Some(run_id), metadata)?;
        Ok(info.run_id.to_run_id())
    }

    /// Get information about a run.
    pub fn get(&self, run: &RunId) -> Result<Option<Versioned<RunInfo>>> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.run_get(&api_run)?)
    }

    /// List all runs.
    ///
    /// # Arguments
    ///
    /// * `state` - Filter by state (None for all)
    /// * `limit` - Maximum runs to return
    ///
    /// # Example
    ///
    /// ```ignore
    /// // List active runs
    /// let active = db.runs.list(Some(RunState::Active), Some(100))?;
    ///
    /// // List all runs
    /// let all = db.runs.list(None, None)?;
    /// ```
    pub fn list(&self, state: Option<RunState>, limit: Option<u64>) -> Result<Vec<Versioned<RunInfo>>> {
        Ok(self.substrate.run_list(state, limit, None)?)
    }

    /// Close a run (mark as completed).
    ///
    /// A closed run cannot receive new writes but remains readable.
    ///
    /// # Example
    ///
    /// ```ignore
    /// db.runs.close(&run)?;
    /// ```
    pub fn close(&self, run: &RunId) -> Result<Version> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.run_close(&api_run)?)
    }

    /// Pause a run.
    ///
    /// A paused run can be resumed later.
    pub fn pause(&self, run: &RunId) -> Result<Version> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.run_pause(&api_run)?)
    }

    /// Resume a paused run.
    pub fn resume(&self, run: &RunId) -> Result<Version> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.run_resume(&api_run)?)
    }

    /// Fail a run with an error message.
    pub fn fail(&self, run: &RunId, error: &str) -> Result<Version> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.run_fail(&api_run, error)?)
    }

    /// Cancel a run.
    pub fn cancel(&self, run: &RunId) -> Result<Version> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.run_cancel(&api_run)?)
    }

    /// Archive a run (terminal state).
    pub fn archive(&self, run: &RunId) -> Result<Version> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.run_archive(&api_run)?)
    }

    // =========================================================================
    // Metadata
    // =========================================================================

    /// Update run metadata.
    pub fn update_metadata(&self, run: &RunId, metadata: Value) -> Result<Version> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.run_update_metadata(&api_run, metadata)?)
    }

    // =========================================================================
    // Retention policy
    // =========================================================================

    /// Set retention policy for a run.
    ///
    /// Controls how long historical versions are kept.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Keep only last 10 versions
    /// db.runs.set_retention(&run, RetentionPolicy::KeepLast(10))?;
    /// ```
    pub fn set_retention(&self, run: &RunId, policy: RetentionPolicy) -> Result<Version> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.run_set_retention(&api_run, policy)?)
    }

    /// Get retention policy for a run.
    pub fn get_retention(&self, run: &RunId) -> Result<RetentionPolicy> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.run_get_retention(&api_run)?)
    }

    // =========================================================================
    // Utility
    // =========================================================================

    /// Check if a run exists.
    pub fn exists(&self, run: &RunId) -> Result<bool> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.run_exists(&api_run)?)
    }

    /// Check if a run exists and is active.
    pub fn is_active(&self, run: &RunId) -> Result<bool> {
        match self.get(run)? {
            Some(info) => Ok(info.value.state.is_active()),
            None => Ok(false),
        }
    }

    /// Get the default run ID.
    ///
    /// The default run is automatically created if it doesn't exist.
    pub fn default_run(&self) -> RunId {
        ApiRunId::default().to_run_id()
    }
}
