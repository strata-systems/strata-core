//! Capabilities Facade - System capability discovery
//!
//! This module provides capability discovery for the facade layer.
//!
//! ## Desugaring
//!
//! | Facade | Substrate |
//! |--------|-----------|
//! | `capabilities()` | Returns system capabilities object |
//!
//! ## Purpose
//!
//! Clients can query capabilities to:
//! - Discover available operations
//! - Check configured limits
//! - Verify feature availability
//! - Adapt behavior based on server configuration

use serde::{Deserialize, Serialize};

/// System capabilities
///
/// Describes the capabilities and limits of the Strata instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    /// API version string
    pub version: String,

    /// Available operations
    pub operations: Vec<String>,

    /// Configured limits
    pub limits: CapabilityLimits,

    /// Supported encodings
    pub encodings: Vec<String>,

    /// Enabled features
    pub features: Vec<String>,
}

/// Configured limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityLimits {
    /// Maximum key size in bytes
    pub max_key_bytes: usize,

    /// Maximum string value size in bytes
    pub max_string_bytes: usize,

    /// Maximum bytes value size
    pub max_bytes_len: usize,

    /// Maximum encoded value size in bytes
    pub max_value_bytes_encoded: usize,

    /// Maximum array length
    pub max_array_len: usize,

    /// Maximum object entries
    pub max_object_entries: usize,

    /// Maximum nesting depth for values
    pub max_nesting_depth: usize,

    /// Maximum vector dimension
    pub max_vector_dim: usize,
}

impl Default for Capabilities {
    fn default() -> Self {
        Capabilities {
            version: "1.0.0".into(),
            operations: vec![
                // KV operations
                "kv.set".into(),
                "kv.get".into(),
                "kv.getv".into(),
                "kv.mget".into(),
                "kv.mset".into(),
                "kv.delete".into(),
                "kv.exists".into(),
                "kv.exists_many".into(),
                "kv.incr".into(),
                // JSON operations
                "json.set".into(),
                "json.get".into(),
                "json.getv".into(),
                "json.del".into(),
                "json.merge".into(),
                // Event operations
                "event.add".into(),
                "event.range".into(),
                "event.len".into(),
                // Vector operations
                "vector.set".into(),
                "vector.get".into(),
                "vector.del".into(),
                // State/CAS operations
                "state.cas_set".into(),
                "state.get".into(),
                // History operations
                "history.list".into(),
                "history.get_at".into(),
                "history.latest_version".into(),
                // Run operations
                "run.list".into(),
                "run.use".into(),
                // System operations
                "system.capabilities".into(),
            ],
            limits: CapabilityLimits::default(),
            encodings: vec!["json".into()],
            features: vec![
                "history".into(),
                "retention".into(),
                "cas".into(),
            ],
        }
    }
}

impl Default for CapabilityLimits {
    fn default() -> Self {
        CapabilityLimits {
            max_key_bytes: 256,
            max_string_bytes: 1024 * 1024,    // 1MB
            max_bytes_len: 16 * 1024 * 1024,  // 16MB
            max_value_bytes_encoded: 64 * 1024 * 1024, // 64MB
            max_array_len: 65536,
            max_object_entries: 65536,
            max_nesting_depth: 64,
            max_vector_dim: 8192,
        }
    }
}

/// System Facade - system operations
///
/// Provides access to system-level operations and configuration.
pub trait SystemFacade {
    /// Get system capabilities
    ///
    /// Returns information about the system's capabilities, limits,
    /// and available features.
    ///
    /// ## Example
    ///
    /// ```ignore
    /// let caps = facade.capabilities();
    /// println!("API version: {}", caps.version);
    /// println!("Max vector dim: {}", caps.limits.max_vector_dim);
    ///
    /// // Check feature availability
    /// if caps.features.contains(&"cas".to_string()) {
    ///     // Use CAS operations
    /// }
    /// ```
    fn capabilities(&self) -> Capabilities;
}

// =============================================================================
// Implementation
// =============================================================================

use super::impl_::FacadeImpl;

impl SystemFacade for FacadeImpl {
    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trait_is_object_safe() {
        fn _assert_object_safe(_: &dyn SystemFacade) {}
    }

    #[test]
    fn test_capabilities_default() {
        let caps = Capabilities::default();
        assert_eq!(caps.version, "1.0.0");
        assert!(!caps.operations.is_empty());
        assert!(caps.features.contains(&"cas".to_string()));
    }

    #[test]
    fn test_limits_default() {
        let limits = CapabilityLimits::default();
        assert_eq!(limits.max_key_bytes, 256);
        assert_eq!(limits.max_vector_dim, 8192);
    }
}
