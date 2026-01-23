//! History Facade - Simplified history operations
//!
//! This module provides history access operations for the facade layer.
//!
//! ## Desugaring
//!
//! | Facade | Substrate |
//! |--------|-----------|
//! | `history(key, limit?, before?)` | `kv_history(default_run, key, limit, before)` |
//! | `get_at(key, version)` | `kv_get_at(default_run, key, version)` |
//! | `latest_version(key)` | `kv_get(default_run, key).map(\|v\| v.version)` |
//!
//! ## Note
//!
//! Facade history is KV-only. Other primitives access history through
//! their specific substrate APIs.

use strata_core::{StrataResult, Value};

/// Versioned value returned from history operations
#[derive(Debug, Clone)]
pub struct VersionedValue {
    /// The value at this version
    pub value: Value,
    /// The version number (transaction ID)
    pub version: u64,
    /// Timestamp of when this version was created (microseconds)
    pub timestamp: u64,
}

/// History Facade - simplified history access operations
///
/// Provides access to version history for key-value pairs.
///
/// ## Design
///
/// - History operations are read-only (no auto-commit needed)
/// - Returns version and timestamp info along with values
/// - Results are ordered newest-first by default
pub trait HistoryFacade {
    /// Get version history for a key
    ///
    /// Returns versions in newest-first order (descending by version).
    ///
    /// ## Parameters
    ///
    /// - `key`: The key to get history for
    /// - `limit`: Maximum number of versions to return (None = all)
    /// - `before`: Only return versions before this version number
    ///
    /// ## Desugars to
    ///
    /// ```text
    /// kv_history(default_run, key, limit, before)
    /// ```
    ///
    /// ## Example
    ///
    /// ```ignore
    /// // Get last 10 versions
    /// let versions = facade.history("counter", Some(10), None)?;
    ///
    /// // Paginate: get next 10 versions before version 500
    /// let older = facade.history("counter", Some(10), Some(500))?;
    /// ```
    fn history(
        &self,
        key: &str,
        limit: Option<u64>,
        before: Option<u64>,
    ) -> StrataResult<Vec<VersionedValue>>;

    /// Get value at a specific version
    ///
    /// ## Errors
    ///
    /// - Returns `HistoryTrimmed` if the version has been garbage collected
    /// - Returns `NotFound` if the key never existed at that version
    ///
    /// ## Desugars to
    ///
    /// ```text
    /// kv_get_at(default_run, key, version)
    /// ```
    fn get_at(&self, key: &str, version: u64) -> StrataResult<Value>;

    /// Get the latest version number for a key
    ///
    /// Returns `None` if the key doesn't exist.
    ///
    /// ## Desugars to
    ///
    /// ```text
    /// kv_get(default_run, key).map(|v| v.version)
    /// ```
    fn latest_version(&self, key: &str) -> StrataResult<Option<u64>>;
}

// =============================================================================
// Implementation
// =============================================================================

use strata_core::Version;
use super::impl_::{FacadeImpl, version_to_u64};
use crate::substrate::KVStore as SubstrateKVStore;

impl HistoryFacade for FacadeImpl {
    fn history(&self, key: &str, limit: Option<u64>, before: Option<u64>) -> StrataResult<Vec<VersionedValue>> {
        let before_version = before.map(Version::Txn);
        let results = self.substrate().kv_history(self.default_run(), key, limit, before_version)?;
        Ok(results.into_iter().map(|v| VersionedValue {
            value: v.value,
            version: version_to_u64(&v.version),
            timestamp: v.timestamp.as_micros(),
        }).collect())
    }

    fn get_at(&self, key: &str, version: u64) -> StrataResult<Value> {
        let result = self.substrate().kv_get_at(self.default_run(), key, Version::Txn(version))?;
        Ok(result.value)
    }

    fn latest_version(&self, key: &str) -> StrataResult<Option<u64>> {
        let result = self.substrate().kv_get(self.default_run(), key)?;
        Ok(result.map(|v| version_to_u64(&v.version)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trait_is_object_safe() {
        fn _assert_object_safe(_: &dyn HistoryFacade) {}
    }
}
