//! RunIndex Substrate Operations
//!
//! The RunIndex manages run lifecycle and metadata.
//! It provides operations for creating, listing, and closing runs.
//!
//! ## Run Model
//!
//! - Every entity belongs to exactly one run (Invariant 5)
//! - The "default" run always exists and cannot be closed
//! - Custom runs are created with UUIDs
//! - Closed runs are read-only
//!
//! ## Run Lifecycle
//!
//! ```text
//! [create] --> Active --> [close] --> Closed
//! ```
//!
//! ## Versioning
//!
//! Run info uses transaction-based versioning (`Version::Txn`).

use super::types::{ApiRunId, RetentionPolicy, RunInfo, RunState};
use strata_core::{StrataResult, Value, Version, Versioned};

/// RunIndex substrate operations
///
/// This trait defines the canonical run management operations.
///
/// ## Contract
///
/// - "default" run always exists
/// - "default" run cannot be closed
/// - Run IDs are unique (UUID or "default")
/// - Closed runs are read-only for data primitives
///
/// ## Error Handling
///
/// | Condition | Error |
/// |-----------|-------|
/// | Invalid run ID format | `InvalidKey` |
/// | Run already exists | `ConstraintViolation` |
/// | Run not found | `NotFound` |
/// | Cannot close default run | `ConstraintViolation` |
/// | Run already closed | `ConstraintViolation` |
pub trait RunIndex {
    /// Create a new run
    ///
    /// Creates a new run with optional metadata.
    /// Returns the run info and version.
    ///
    /// ## Parameters
    ///
    /// - `run_id`: Optional specific ID (if None, generates UUID)
    /// - `metadata`: Optional metadata (must be Object or Null)
    ///
    /// ## Return Value
    ///
    /// Returns `(run_info, version)`.
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Run ID format is invalid
    /// - `ConstraintViolation`: Run already exists, or metadata not Object/Null
    fn run_create(
        &self,
        run_id: Option<&ApiRunId>,
        metadata: Option<Value>,
    ) -> StrataResult<(RunInfo, Version)>;

    /// Get run info
    ///
    /// Returns information about a run.
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Run ID format is invalid
    /// - `NotFound`: Run does not exist
    fn run_get(&self, run: &ApiRunId) -> StrataResult<Option<Versioned<RunInfo>>>;

    /// List all runs
    ///
    /// Returns all runs matching the filters.
    ///
    /// ## Parameters
    ///
    /// - `state`: Filter by state (Active/Closed)
    /// - `limit`: Maximum runs to return
    /// - `offset`: Skip first N runs
    ///
    /// ## Return Value
    ///
    /// Vector of run info, ordered by creation time (newest first).
    fn run_list(
        &self,
        state: Option<RunState>,
        limit: Option<u64>,
        offset: Option<u64>,
    ) -> StrataResult<Vec<Versioned<RunInfo>>>;

    /// Close a run
    ///
    /// Marks a run as closed. Closed runs are read-only.
    /// Returns the new version.
    ///
    /// ## Semantics
    ///
    /// - Cannot close "default" run
    /// - Cannot close already-closed run
    /// - After closing, all write operations fail with `ConstraintViolation`
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Run ID format is invalid
    /// - `NotFound`: Run does not exist
    /// - `ConstraintViolation`: Cannot close default run, or already closed
    fn run_close(&self, run: &ApiRunId) -> StrataResult<Version>;

    /// Update run metadata
    ///
    /// Updates the metadata for a run.
    /// Returns the new version.
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Run ID format is invalid
    /// - `NotFound`: Run does not exist
    /// - `ConstraintViolation`: Run is closed, or metadata not Object/Null
    fn run_update_metadata(&self, run: &ApiRunId, metadata: Value) -> StrataResult<Version>;

    /// Check if a run exists
    ///
    /// Returns `true` if the run exists (regardless of state).
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Run ID format is invalid
    fn run_exists(&self, run: &ApiRunId) -> StrataResult<bool>;

    /// Pause a run
    ///
    /// Transitions the run to Paused state. Can be resumed later.
    ///
    /// ## Errors
    ///
    /// - `NotFound`: Run does not exist
    /// - `ConstraintViolation`: Run is not in Active state
    fn run_pause(&self, run: &ApiRunId) -> StrataResult<Version>;

    /// Resume a paused run
    ///
    /// Transitions the run from Paused back to Active state.
    ///
    /// ## Errors
    ///
    /// - `NotFound`: Run does not exist
    /// - `ConstraintViolation`: Run is not in Paused state
    fn run_resume(&self, run: &ApiRunId) -> StrataResult<Version>;

    /// Fail a run with an error message
    ///
    /// Transitions the run to Failed state with an error message.
    ///
    /// ## Errors
    ///
    /// - `NotFound`: Run does not exist
    /// - `ConstraintViolation`: Run is in terminal state
    fn run_fail(&self, run: &ApiRunId, error: &str) -> StrataResult<Version>;

    /// Delete a run and all its data
    ///
    /// This operation is destructive and cascades to all run data.
    /// The default run cannot be deleted.
    ///
    /// ## Errors
    ///
    /// - `NotFound`: Run does not exist
    /// - `ConstraintViolation`: Cannot delete the default run
    fn run_delete(&self, run: &ApiRunId) -> StrataResult<()>;

    /// Query runs by status
    ///
    /// Returns all runs that are in the specified state.
    ///
    /// ## Errors
    ///
    /// - Storage errors
    fn run_query_by_status(&self, state: RunState) -> StrataResult<Vec<Versioned<RunInfo>>>;

    /// Set retention policy for a run
    ///
    /// Configures the history retention policy for a run.
    /// Returns the new version.
    ///
    /// ## Semantics
    ///
    /// - Policy applies to all primitives in the run
    /// - Existing history beyond policy may be garbage collected
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Run ID format is invalid
    /// - `NotFound`: Run does not exist
    fn run_set_retention(&self, run: &ApiRunId, policy: RetentionPolicy) -> StrataResult<Version>;

    /// Get retention policy for a run
    ///
    /// Returns the current retention policy.
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Run ID format is invalid
    /// - `NotFound`: Run does not exist
    fn run_get_retention(&self, run: &ApiRunId) -> StrataResult<RetentionPolicy>;
}

// =============================================================================
// Implementation
// =============================================================================

use strata_core::StrataError;
use super::impl_::{SubstrateImpl, convert_error, api_run_id_to_string};

impl RunIndex for SubstrateImpl {
    fn run_create(
        &self,
        run_id: Option<&ApiRunId>,
        metadata: Option<Value>,
    ) -> StrataResult<(RunInfo, Version)> {
        let run_str = run_id.map(api_run_id_to_string).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let versioned = if let Some(meta) = metadata {
            self.run().create_run_with_options(&run_str, None, vec![], meta).map_err(convert_error)?
        } else {
            self.run().create_run(&run_str).map_err(convert_error)?
        };

        let api_run_id = run_id.cloned().unwrap_or_else(|| {
            ApiRunId::parse(&run_str).unwrap_or_else(|| ApiRunId::new())
        });

        let info = RunInfo {
            run_id: api_run_id,
            created_at: (versioned.value.created_at.max(0) as u64).saturating_mul(1000),
            metadata: versioned.value.metadata,
            state: convert_run_status(&versioned.value.status),
            error: versioned.value.error,
        };

        Ok((info, versioned.version))
    }

    fn run_get(&self, run: &ApiRunId) -> StrataResult<Option<Versioned<RunInfo>>> {
        let run_str = api_run_id_to_string(run);
        let meta = self.run().get_run(&run_str).map_err(convert_error)?;

        Ok(meta.map(|m| {
            let info = RunInfo {
                run_id: run.clone(),
                // Primitive stores created_at as i64 millis, convert to u64 micros
                created_at: (m.value.created_at.max(0) as u64).saturating_mul(1000),
                metadata: m.value.metadata,
                state: convert_run_status(&m.value.status),
                error: m.value.error,
            };
            Versioned {
                value: info,
                version: Version::Txn(0),
                // Convert i64 millis to Timestamp
                timestamp: strata_core::Timestamp::from_millis(m.value.created_at.max(0) as u64),
            }
        }))
    }

    fn run_list(
        &self,
        state: Option<RunState>,
        limit: Option<u64>,
        _offset: Option<u64>,
    ) -> StrataResult<Vec<Versioned<RunInfo>>> {
        let run_ids = if let Some(s) = state {
            let primitive_status = convert_run_state_to_status(s);
            self.run().query_by_status(primitive_status).map_err(convert_error)?
        } else {
            // Get all runs
            let ids = self.run().list_runs().map_err(convert_error)?;
            let mut runs = Vec::new();
            for id in ids {
                if let Some(versioned) = self.run().get_run(&id).map_err(convert_error)? {
                    runs.push(versioned.value);
                }
            }
            runs
        };

        let limited = match limit {
            Some(l) => run_ids.into_iter().take(l as usize).collect(),
            None => run_ids,
        };

        Ok(limited
            .into_iter()
            .map(|m| {
                let api_run_id = ApiRunId::parse(&m.run_id).unwrap_or_else(|| ApiRunId::new());
                let info = RunInfo {
                    run_id: api_run_id,
                    // Primitive stores created_at as i64 millis, convert to u64 micros
                    created_at: (m.created_at.max(0) as u64).saturating_mul(1000),
                    metadata: m.metadata,
                    state: convert_run_status(&m.status),
                    error: m.error,
                };
                Versioned {
                    value: info,
                    version: Version::Txn(0),
                    // Convert i64 millis to Timestamp
                    timestamp: strata_core::Timestamp::from_millis(m.created_at.max(0) as u64),
                }
            })
            .collect())
    }

    fn run_close(&self, run: &ApiRunId) -> StrataResult<Version> {
        if run.is_default() {
            return Err(StrataError::invalid_operation(
                strata_core::EntityRef::run(run.to_run_id()),
                "Cannot close the default run",
            ));
        }
        let run_str = api_run_id_to_string(run);
        let versioned = self.run().complete_run(&run_str).map_err(convert_error)?;
        Ok(versioned.version)
    }

    fn run_update_metadata(&self, run: &ApiRunId, metadata: Value) -> StrataResult<Version> {
        let run_str = api_run_id_to_string(run);
        let versioned = self.run().update_metadata(&run_str, metadata).map_err(convert_error)?;
        Ok(versioned.version)
    }

    fn run_exists(&self, run: &ApiRunId) -> StrataResult<bool> {
        let run_str = api_run_id_to_string(run);
        self.run().exists(&run_str).map_err(convert_error)
    }

    fn run_set_retention(&self, _run: &ApiRunId, _policy: RetentionPolicy) -> StrataResult<Version> {
        // Retention not yet implemented
        Ok(Version::Txn(0))
    }

    fn run_get_retention(&self, _run: &ApiRunId) -> StrataResult<RetentionPolicy> {
        Ok(RetentionPolicy::KeepAll)
    }

    fn run_pause(&self, run: &ApiRunId) -> StrataResult<Version> {
        let run_str = api_run_id_to_string(run);
        let versioned = self.run().pause_run(&run_str).map_err(convert_error)?;
        Ok(versioned.version)
    }

    fn run_resume(&self, run: &ApiRunId) -> StrataResult<Version> {
        let run_str = api_run_id_to_string(run);
        let versioned = self.run().resume_run(&run_str).map_err(convert_error)?;
        Ok(versioned.version)
    }

    fn run_fail(&self, run: &ApiRunId, error: &str) -> StrataResult<Version> {
        let run_str = api_run_id_to_string(run);
        let versioned = self.run().fail_run(&run_str, error).map_err(convert_error)?;
        Ok(versioned.version)
    }

    fn run_delete(&self, run: &ApiRunId) -> StrataResult<()> {
        if run.is_default() {
            return Err(StrataError::invalid_operation(
                strata_core::EntityRef::run(run.to_run_id()),
                "Cannot delete the default run",
            ));
        }
        let run_str = api_run_id_to_string(run);
        self.run().delete_run(&run_str).map_err(convert_error)
    }

    fn run_query_by_status(&self, state: RunState) -> StrataResult<Vec<Versioned<RunInfo>>> {
        let primitive_status = convert_run_state_to_status(state);
        let runs = self.run().query_by_status(primitive_status).map_err(convert_error)?;

        Ok(runs
            .into_iter()
            .map(|m| {
                let api_run_id = ApiRunId::parse(&m.run_id).unwrap_or_else(|| ApiRunId::new());
                let info = RunInfo {
                    run_id: api_run_id,
                    created_at: (m.created_at.max(0) as u64).saturating_mul(1000),
                    metadata: m.metadata,
                    state: convert_run_status(&m.status),
                    error: m.error,
                };
                Versioned {
                    value: info,
                    version: Version::Txn(0),
                    timestamp: strata_core::Timestamp::from_millis(m.created_at.max(0) as u64),
                }
            })
            .collect())
    }
}

fn convert_run_status(status: &strata_primitives::RunStatus) -> RunState {
    match status {
        strata_primitives::RunStatus::Active => RunState::Active,
        strata_primitives::RunStatus::Completed => RunState::Completed,
        strata_primitives::RunStatus::Failed => RunState::Failed,
        strata_primitives::RunStatus::Cancelled => RunState::Cancelled,
        strata_primitives::RunStatus::Paused => RunState::Paused,
        strata_primitives::RunStatus::Archived => RunState::Archived,
    }
}

fn convert_run_state_to_status(state: RunState) -> strata_primitives::RunStatus {
    match state {
        RunState::Active => strata_primitives::RunStatus::Active,
        RunState::Completed => strata_primitives::RunStatus::Completed,
        RunState::Failed => strata_primitives::RunStatus::Failed,
        RunState::Cancelled => strata_primitives::RunStatus::Cancelled,
        RunState::Paused => strata_primitives::RunStatus::Paused,
        RunState::Archived => strata_primitives::RunStatus::Archived,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trait_is_object_safe() {
        fn _assert_object_safe(_: &dyn RunIndex) {}
    }
}
