//! Retention Substrate - Retention policy operations
//!
//! This module provides substrate-level operations for managing retention policies.
//!
//! ## Retention Policies
//!
//! Strata supports flexible retention policies that control version history:
//!
//! - `KeepAll`: Keep all versions indefinitely (default)
//! - `KeepLast(n)`: Keep the N most recent versions
//! - `KeepFor(duration)`: Keep versions within the time window
//! - `Composite`: Union of multiple policies (most permissive wins)
//!
//! ## Scope
//!
//! - Retention is configured per-run
//! - Per-key retention is NOT supported in M11
//! - Retention applies to all primitives within a run

use strata_core::StrataResult;
use crate::substrate::types::{ApiRunId, RetentionPolicy};

/// Version information for retention policy
#[derive(Debug, Clone)]
pub struct RetentionVersion {
    /// The retention policy
    pub policy: RetentionPolicy,
    /// Version number when this policy was set
    pub version: u64,
    /// Timestamp when this policy was set (microseconds)
    pub timestamp: u64,
}

/// Retention Substrate - retention policy operations
///
/// All operations require explicit `run_id` parameter.
///
/// ## Design
///
/// - Retention is configured at the run level
/// - Per-key retention is not supported in M11
/// - The default policy is `KeepAll`
/// - Changing retention policy does not immediately trigger garbage collection
pub trait RetentionSubstrate {
    /// Get the retention policy for a run
    ///
    /// Returns `None` if no explicit policy is set (defaults apply).
    ///
    /// ## Parameters
    ///
    /// - `run`: The run to query retention for
    ///
    /// ## Returns
    ///
    /// The current retention policy with version info, or `None` if
    /// using the default policy.
    fn retention_get(&self, run: &ApiRunId) -> StrataResult<Option<RetentionVersion>>;

    /// Set the retention policy for a run
    ///
    /// ## Parameters
    ///
    /// - `run`: The run to set retention for
    /// - `policy`: The retention policy to apply
    ///
    /// ## Returns
    ///
    /// The version number of the policy update.
    ///
    /// ## Example
    ///
    /// ```ignore
    /// // Keep last 100 versions
    /// substrate.retention_set(&run, RetentionPolicy::KeepLast(100))?;
    ///
    /// // Keep versions from the last 7 days
    /// substrate.retention_set(&run, RetentionPolicy::KeepFor(Duration::from_secs(7 * 24 * 3600)))?;
    ///
    /// // Composite: keep last 10 OR anything from last hour
    /// substrate.retention_set(&run, RetentionPolicy::Composite(vec![
    ///     RetentionPolicy::KeepLast(10),
    ///     RetentionPolicy::KeepFor(Duration::from_secs(3600)),
    /// ]))?;
    /// ```
    fn retention_set(&self, run: &ApiRunId, policy: RetentionPolicy) -> StrataResult<u64>;

    /// Clear the retention policy for a run (revert to default)
    ///
    /// After clearing, the run will use the default `KeepAll` policy.
    fn retention_clear(&self, run: &ApiRunId) -> StrataResult<bool>;
}

/// Statistics about retention for a run
#[derive(Debug, Clone, Default)]
pub struct RetentionStats {
    /// Total versions across all keys
    pub total_versions: u64,
    /// Versions eligible for garbage collection
    pub gc_eligible_versions: u64,
    /// Estimated bytes that could be reclaimed
    pub estimated_reclaimable_bytes: u64,
}

/// Extended retention operations (optional)
///
/// These operations are not required for M11 but provide
/// useful diagnostics.
pub trait RetentionSubstrateExt: RetentionSubstrate {
    /// Get retention statistics for a run
    ///
    /// Returns statistics about version retention in the run.
    fn retention_stats(&self, run: &ApiRunId) -> StrataResult<RetentionStats>;

    /// Trigger garbage collection for a run
    ///
    /// Normally, garbage collection happens automatically in the background.
    /// This method triggers an immediate collection cycle.
    fn retention_gc(&self, run: &ApiRunId) -> StrataResult<RetentionStats>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trait_is_object_safe() {
        fn _assert_object_safe(_: &dyn RetentionSubstrate) {}
    }

    #[test]
    fn test_retention_version() {
        let rv = RetentionVersion {
            policy: RetentionPolicy::KeepLast(100),
            version: 42,
            timestamp: 1234567890,
        };
        assert!(matches!(rv.policy, RetentionPolicy::KeepLast(100)));
    }

    #[test]
    fn test_retention_stats_default() {
        let stats = RetentionStats::default();
        assert_eq!(stats.total_versions, 0);
        assert_eq!(stats.gc_eligible_versions, 0);
    }
}
