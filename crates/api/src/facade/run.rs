//! Run Facade - Simplified run operations
//!
//! This module provides run listing and scoping operations for the facade layer.
//!
//! ## Desugaring
//!
//! | Facade | Substrate |
//! |--------|-----------|
//! | `runs()` | `run_list()` |
//! | `use_run(run_id)` | Returns facade with `default = run_id` (client-side binding) |
//!
//! ## Note
//!
//! Run lifecycle (create/close) is substrate-only.
//! The facade only provides listing and scoping.

use strata_core::StrataResult;

/// Summary information about a run
#[derive(Debug, Clone)]
pub struct RunSummary {
    /// The run identifier
    pub run_id: String,
    /// When the run was created (microseconds since epoch)
    pub created_at: u64,
    /// Whether the run is active or closed
    pub is_active: bool,
}

/// Run Facade - simplified run operations
///
/// Provides access to run listing and scoping operations.
///
/// ## Design
///
/// - The facade exposes only read operations for runs
/// - Run creation and closure are substrate-only operations
/// - `use_run` creates a scoped facade that targets a specific run
pub trait RunFacade {
    /// List all runs
    ///
    /// Returns summary information for all runs in the system.
    ///
    /// ## Desugars to
    ///
    /// ```text
    /// run_list()
    /// ```
    ///
    /// ## Example
    ///
    /// ```ignore
    /// let runs = facade.runs()?;
    /// for run in runs {
    ///     println!("Run: {} (active: {})", run.run_id, run.is_active);
    /// }
    /// ```
    fn runs(&self) -> StrataResult<Vec<RunSummary>>;

    /// Scope operations to a specific run
    ///
    /// Returns a scoped facade that targets the specified run instead
    /// of the default run.
    ///
    /// ## Errors
    ///
    /// Returns `NotFound` if the run doesn't exist. The facade does not
    /// lazily create runs - they must be created via substrate first.
    ///
    /// ## Desugars to
    ///
    /// ```text
    /// // Client-side binding: returns facade with default = run_id
    /// ```
    ///
    /// ## Example
    ///
    /// ```ignore
    /// // Create run via substrate
    /// let run_id = substrate.run_create(Value::Null)?;
    ///
    /// // Scope facade to that run
    /// let scoped = facade.use_run(&run_id.to_string())?;
    ///
    /// // Operations now target the scoped run
    /// scoped.set("key", Value::Int(42))?;
    /// ```
    fn use_run(&self, run_id: &str) -> StrataResult<Box<dyn ScopedFacade>>;
}

/// A facade scoped to a specific run
///
/// This trait combines all facade operations but targets
/// a specific run instead of the default run.
pub trait ScopedFacade: Send + Sync {
    /// The run this facade is scoped to
    fn run_id(&self) -> &str;
}

// =============================================================================
// Implementation
// =============================================================================

use std::sync::Arc;
use strata_core::StrataError;
use super::impl_::FacadeImpl;
use crate::substrate::{SubstrateImpl, RunIndex as SubstrateRunIndex, ApiRunId, RunState};

impl RunFacade for FacadeImpl {
    fn runs(&self) -> StrataResult<Vec<RunSummary>> {
        let results = self.substrate().run_list(None, None, None)?;
        Ok(results.into_iter().map(|v| RunSummary {
            run_id: v.value.run_id.as_str().to_string(),
            created_at: v.value.created_at,
            is_active: matches!(v.value.state, RunState::Active),
        }).collect())
    }

    fn use_run(&self, run_id: &str) -> StrataResult<Box<dyn ScopedFacade>> {
        let api_run_id = ApiRunId::parse(run_id).ok_or_else(|| {
            StrataError::invalid_input("Invalid run ID format")
        })?;
        Ok(Box::new(ScopedFacadeImpl {
            substrate: Arc::clone(&self.substrate_arc()),
            run_id: api_run_id,
        }))
    }
}

/// Scoped facade implementation
struct ScopedFacadeImpl {
    #[allow(dead_code)]
    substrate: Arc<SubstrateImpl>,
    run_id: ApiRunId,
}

impl ScopedFacade for ScopedFacadeImpl {
    fn run_id(&self) -> &str {
        self.run_id.as_str()
    }
}

// Make ScopedFacadeImpl Send + Sync
unsafe impl Send for ScopedFacadeImpl {}
unsafe impl Sync for ScopedFacadeImpl {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trait_is_object_safe() {
        fn _assert_run_facade_object_safe(_: &dyn RunFacade) {}
        fn _assert_scoped_facade_object_safe(_: &dyn ScopedFacade) {}
    }
}
