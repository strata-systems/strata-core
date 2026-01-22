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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trait_is_object_safe() {
        fn _assert_run_facade_object_safe(_: &dyn RunFacade) {}
        fn _assert_scoped_facade_object_safe(_: &dyn ScopedFacade) {}
    }
}
