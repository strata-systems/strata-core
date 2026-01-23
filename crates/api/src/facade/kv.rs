//! KV Facade - Redis-like key-value operations
//!
//! This module provides simplified key-value operations that mirror
//! Redis commands while using Strata's substrate under the hood.
//!
//! ## Desugaring
//!
//! | Facade | Substrate |
//! |--------|-----------|
//! | `get(key)` | `kv_get(default_run, key).map(\|v\| v.value)` |
//! | `getv(key)` | `kv_get(default_run, key)` |
//! | `set(key, val)` | `kv_put(default_run, key, val)` |
//! | `del(key)` | `kv_delete(default_run, key)` |
//! | `exists(key)` | `kv_exists(default_run, key)` |
//! | `incr(key)` | `kv_incr(default_run, key, 1)` |
//! | `setnx(key, val)` | `kv_cas_version(default_run, key, None, val)` |

use super::types::{SetOptions, GetOptions, IncrOptions};
use strata_core::{StrataResult, Value};

/// A versioned value from the KV store
///
/// This type provides access to version information when needed.
/// Use `getv` instead of `get` when you need version info.
#[derive(Debug, Clone)]
pub struct Versioned<T> {
    /// The value
    pub value: T,
    /// The version number (transaction ID)
    pub version: u64,
    /// Timestamp when this version was created (microseconds)
    pub timestamp: u64,
}

/// KV Facade - simplified key-value operations
///
/// This trait provides Redis-familiar operations that desugar to
/// substrate calls against the default run with auto-commit.
///
/// ## Implicit Behaviors
///
/// - Operations target the default run
/// - Each operation auto-commits immediately
/// - Versions are stripped from return values by default
///
/// ## Example
///
/// ```ignore
/// // Simple get/set
/// facade.set("user:1", Value::String("Alice".to_string()))?;
/// let name = facade.get("user:1")?;
///
/// // Increment counter
/// let count = facade.incr("visits")?;
///
/// // Conditional set
/// let was_new = facade.setnx("lock", Value::Bool(true))?;
/// ```
pub trait KVFacade {
    /// Get a value by key
    ///
    /// Returns `None` if key doesn't exist.
    ///
    /// ## Desugars to
    /// ```text
    /// kv_get(default_run, key).map(|v| v.value)
    /// ```
    fn get(&self, key: &str) -> StrataResult<Option<Value>>;

    /// Get a versioned value by key
    ///
    /// Escape hatch to access version information.
    /// Use this when you need the version number or timestamp.
    ///
    /// ## Desugars to
    /// ```text
    /// kv_get(default_run, key)
    /// ```
    fn getv(&self, key: &str) -> StrataResult<Option<Versioned<Value>>>;

    /// Get a value with options
    ///
    /// Allows requesting version info or historical values.
    fn get_with_options(&self, key: &str, options: GetOptions)
        -> StrataResult<Option<(Value, Option<u64>)>>;

    /// Set a value
    ///
    /// Creates or replaces the value.
    ///
    /// ## Desugars to
    /// ```text
    /// kv_put(default_run, key, value)
    /// ```
    fn set(&self, key: &str, value: Value) -> StrataResult<()>;

    /// Set a value with options
    ///
    /// Supports NX (not exists), XX (exists), and GET (return old).
    fn set_with_options(
        &self,
        key: &str,
        value: Value,
        options: SetOptions,
    ) -> StrataResult<Option<Value>>;

    /// Delete a key
    ///
    /// Returns `true` if the key existed.
    ///
    /// ## Desugars to
    /// ```text
    /// kv_delete(default_run, key)
    /// ```
    fn del(&self, key: &str) -> StrataResult<bool>;

    /// Check if a key exists
    ///
    /// ## Desugars to
    /// ```text
    /// kv_exists(default_run, key)
    /// ```
    fn exists(&self, key: &str) -> StrataResult<bool>;

    /// Increment by 1
    ///
    /// Creates key with value 1 if it doesn't exist.
    ///
    /// ## Desugars to
    /// ```text
    /// kv_incr(default_run, key, 1)
    /// ```
    fn incr(&self, key: &str) -> StrataResult<i64>;

    /// Increment by delta
    ///
    /// Creates key with `delta` if it doesn't exist.
    ///
    /// ## Desugars to
    /// ```text
    /// kv_incr(default_run, key, delta)
    /// ```
    fn incrby(&self, key: &str, delta: i64) -> StrataResult<i64>;

    /// Increment with options
    fn incr_with_options(&self, key: &str, delta: i64, options: IncrOptions) -> StrataResult<i64>;

    /// Decrement by 1
    ///
    /// Equivalent to `incrby(key, -1)`.
    fn decr(&self, key: &str) -> StrataResult<i64> {
        self.incrby(key, -1)
    }

    /// Decrement by delta
    ///
    /// Equivalent to `incrby(key, -delta)`.
    fn decrby(&self, key: &str, delta: i64) -> StrataResult<i64> {
        self.incrby(key, -delta)
    }

    /// Set if not exists (NX)
    ///
    /// Returns `true` if the key was set (didn't exist).
    ///
    /// ## Desugars to
    /// ```text
    /// kv_cas_version(default_run, key, None, value)
    /// ```
    fn setnx(&self, key: &str, value: Value) -> StrataResult<bool>;

    /// Get and set atomically
    ///
    /// Sets the new value and returns the old value.
    ///
    /// ## Desugars to
    /// ```text
    /// let old = kv_get(default_run, key);
    /// kv_put(default_run, key, value);
    /// old.map(|v| v.value)
    /// ```
    fn getset(&self, key: &str, value: Value) -> StrataResult<Option<Value>>;
}

/// Batch operations for KV Facade
pub trait KVFacadeBatch: KVFacade {
    /// Get multiple keys
    ///
    /// Returns values in same order as keys, with `None` for missing.
    ///
    /// ## Desugars to
    /// ```text
    /// kv_mget(default_run, keys).map(|vs| vs.map(|v| v.map(|x| x.value)))
    /// ```
    fn mget(&self, keys: &[&str]) -> StrataResult<Vec<Option<Value>>>;

    /// Set multiple key-value pairs
    ///
    /// Atomic: all set or none set.
    ///
    /// ## Desugars to
    /// ```text
    /// kv_mput(default_run, entries)
    /// ```
    fn mset(&self, entries: &[(&str, Value)]) -> StrataResult<()>;

    /// Delete multiple keys
    ///
    /// Returns count of keys that existed.
    ///
    /// ## Desugars to
    /// ```text
    /// kv_mdelete(default_run, keys)
    /// ```
    fn mdel(&self, keys: &[&str]) -> StrataResult<u64>;

    /// Count existing keys
    ///
    /// ## Desugars to
    /// ```text
    /// kv_mexists(default_run, keys)
    /// ```
    fn mexists(&self, keys: &[&str]) -> StrataResult<u64>;
}

// =============================================================================
// Implementation
// =============================================================================

use strata_core::Version;
use super::impl_::{FacadeImpl, version_to_u64};
use crate::substrate::KVStore as SubstrateKVStore;
use crate::substrate::KVStoreBatch as SubstrateKVStoreBatch;

impl KVFacade for FacadeImpl {
    fn get(&self, key: &str) -> StrataResult<Option<Value>> {
        let result = self.substrate().kv_get(self.default_run(), key)?;
        Ok(result.map(|v| v.value))
    }

    fn getv(&self, key: &str) -> StrataResult<Option<Versioned<Value>>> {
        let result = self.substrate().kv_get(self.default_run(), key)?;
        Ok(result.map(|v| Versioned {
            value: v.value,
            version: version_to_u64(&v.version),
            timestamp: v.timestamp.as_micros(),
        }))
    }

    fn get_with_options(&self, key: &str, options: GetOptions)
        -> StrataResult<Option<(Value, Option<u64>)>>
    {
        if let Some(at_version) = options.at_version {
            // kv_get_at returns Versioned<Value> directly (not Option)
            // On error (history trimmed, not found), it returns Err
            match self.substrate().kv_get_at(self.default_run(), key, Version::Txn(at_version)) {
                Ok(versioned) => {
                    let version_out = if options.with_version { Some(at_version) } else { None };
                    Ok(Some((versioned.value, version_out)))
                }
                Err(_) => Ok(None), // Key doesn't exist at that version or history trimmed
            }
        } else {
            let result = self.substrate().kv_get(self.default_run(), key)?;
            Ok(result.map(|v| {
                let version_out = if options.with_version {
                    Some(version_to_u64(&v.version))
                } else {
                    None
                };
                (v.value, version_out)
            }))
        }
    }

    fn set(&self, key: &str, value: Value) -> StrataResult<()> {
        let _version = self.substrate().kv_put(self.default_run(), key, value)?;
        Ok(())
    }

    fn set_with_options(
        &self,
        key: &str,
        value: Value,
        options: SetOptions,
    ) -> StrataResult<Option<Value>> {
        let old_value = if options.get_old_value {
            self.substrate().kv_get(self.default_run(), key)?.map(|v| v.value)
        } else {
            None
        };

        if options.only_if_not_exists {
            let success = self.substrate().kv_cas_version(self.default_run(), key, None, value)?;
            if success {
                Ok(old_value)
            } else {
                Ok(None)
            }
        } else if options.only_if_exists {
            let exists = self.substrate().kv_exists(self.default_run(), key)?;
            if exists {
                let _version = self.substrate().kv_put(self.default_run(), key, value)?;
                Ok(old_value)
            } else {
                Ok(None)
            }
        } else if let Some(expected) = options.expected_version {
            let success = self.substrate().kv_cas_version(
                self.default_run(),
                key,
                Some(Version::Txn(expected)),
                value,
            )?;
            if success {
                Ok(old_value)
            } else {
                Ok(None)
            }
        } else {
            let _version = self.substrate().kv_put(self.default_run(), key, value)?;
            Ok(old_value)
        }
    }

    fn del(&self, key: &str) -> StrataResult<bool> {
        self.substrate().kv_delete(self.default_run(), key)
    }

    fn exists(&self, key: &str) -> StrataResult<bool> {
        self.substrate().kv_exists(self.default_run(), key)
    }

    fn incr(&self, key: &str) -> StrataResult<i64> {
        self.substrate().kv_incr(self.default_run(), key, 1)
    }

    fn incrby(&self, key: &str, delta: i64) -> StrataResult<i64> {
        self.substrate().kv_incr(self.default_run(), key, delta)
    }

    fn incr_with_options(&self, key: &str, delta: i64, _options: IncrOptions) -> StrataResult<i64> {
        self.substrate().kv_incr(self.default_run(), key, delta)
    }

    fn setnx(&self, key: &str, value: Value) -> StrataResult<bool> {
        self.substrate().kv_cas_version(self.default_run(), key, None, value)
    }

    fn getset(&self, key: &str, value: Value) -> StrataResult<Option<Value>> {
        let old = self.substrate().kv_get(self.default_run(), key)?.map(|v| v.value);
        let _version = self.substrate().kv_put(self.default_run(), key, value)?;
        Ok(old)
    }
}

impl KVFacadeBatch for FacadeImpl {
    fn mget(&self, keys: &[&str]) -> StrataResult<Vec<Option<Value>>> {
        let results = self.substrate().kv_mget(self.default_run(), keys)?;
        Ok(results.into_iter().map(|opt| opt.map(|v| v.value)).collect())
    }

    fn mset(&self, entries: &[(&str, Value)]) -> StrataResult<()> {
        let _version = self.substrate().kv_mput(self.default_run(), entries)?;
        Ok(())
    }

    fn mdel(&self, keys: &[&str]) -> StrataResult<u64> {
        self.substrate().kv_mdelete(self.default_run(), keys)
    }

    fn mexists(&self, keys: &[&str]) -> StrataResult<u64> {
        self.substrate().kv_mexists(self.default_run(), keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trait_is_object_safe() {
        fn _assert_object_safe(_: &dyn KVFacade) {}
        fn _assert_batch_object_safe(_: &dyn KVFacadeBatch) {}
    }
}
