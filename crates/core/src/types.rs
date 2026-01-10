//! Core types for the in-memory database
//!
//! This module defines the fundamental types used throughout the system:
//! - [`RunId`]: Unique identifier for agent runs
//! - [`Namespace`]: Hierarchical namespace for multi-tenant isolation

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a run (agent execution)
///
/// RunId is used throughout the system to identify individual agent runs.
/// It's used in:
/// - WAL entries for replay
/// - Storage keys for data isolation
/// - Transaction contexts
/// - Lineage tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RunId(Uuid);

impl RunId {
    /// Create a new random RunId using UUID v4
    ///
    /// # Examples
    ///
    /// ```
    /// use in_mem_core::types::RunId;
    ///
    /// let id1 = RunId::new();
    /// let id2 = RunId::new();
    /// assert_ne!(id1, id2); // Each RunId is unique
    /// ```
    pub fn new() -> Self {
        RunId(Uuid::new_v4())
    }

    /// Create RunId from raw bytes
    ///
    /// # Examples
    ///
    /// ```
    /// use in_mem_core::types::RunId;
    ///
    /// let bytes = [0u8; 16];
    /// let id = RunId::from_bytes(bytes);
    /// ```
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        RunId(Uuid::from_bytes(bytes))
    }

    /// Get raw bytes representation
    ///
    /// # Examples
    ///
    /// ```
    /// use in_mem_core::types::RunId;
    ///
    /// let id = RunId::new();
    /// let bytes = id.as_bytes();
    /// let id2 = RunId::from_bytes(*bytes);
    /// assert_eq!(id, id2);
    /// ```
    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl Default for RunId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for RunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Namespace for hierarchical data isolation
///
/// Provides multi-tenant isolation with four levels:
/// - tenant: Top-level organization
/// - app: Application within tenant
/// - agent: Agent within application
/// - run_id: Specific execution of the agent
///
/// Namespaces are ordered lexicographically: tenant → app → agent → run_id
///
/// # Examples
///
/// ```
/// use in_mem_core::types::{Namespace, RunId};
///
/// let run_id = RunId::new();
/// let ns = Namespace::new("acme", "chatbot", "agent-42", run_id);
/// assert_eq!(ns.tenant, "acme");
/// assert_eq!(ns.app, "chatbot");
/// assert_eq!(ns.agent, "agent-42");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Namespace {
    /// Tenant identifier (top-level organization)
    pub tenant: String,
    /// Application identifier within tenant
    pub app: String,
    /// Agent identifier within application
    pub agent: String,
    /// Run identifier for this specific execution
    pub run_id: RunId,
}

impl Namespace {
    /// Create a new namespace
    ///
    /// # Examples
    ///
    /// ```
    /// use in_mem_core::types::{Namespace, RunId};
    ///
    /// let run_id = RunId::new();
    /// let ns = Namespace::new("acme", "myapp", "agent-1", run_id);
    /// ```
    pub fn new(
        tenant: impl Into<String>,
        app: impl Into<String>,
        agent: impl Into<String>,
        run_id: RunId,
    ) -> Self {
        Self {
            tenant: tenant.into(),
            app: app.into(),
            agent: agent.into(),
            run_id,
        }
    }
}

impl std::fmt::Display for Namespace {
    /// Display namespace in the format: tenant/app/agent/run_id
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}/{}/{}", self.tenant, self.app, self.agent, self.run_id)
    }
}

// Ord implementation for BTreeMap key ordering
// Orders by: tenant → app → agent → run_id
impl Ord for Namespace {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.tenant
            .cmp(&other.tenant)
            .then(self.app.cmp(&other.app))
            .then(self.agent.cmp(&other.agent))
            .then(self.run_id.0.cmp(&other.run_id.0))
    }
}

impl PartialOrd for Namespace {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== RunId Tests =====

    #[test]
    fn test_run_id_creation() {
        let id1 = RunId::new();
        let id2 = RunId::new();
        assert_ne!(id1, id2, "Each RunId should be unique");
    }

    #[test]
    fn test_run_id_serialization() {
        let id = RunId::new();
        let bytes = *id.as_bytes();
        let restored = RunId::from_bytes(bytes);
        assert_eq!(id, restored, "RunId should roundtrip through bytes");
    }

    #[test]
    fn test_run_id_display() {
        let id = RunId::new();
        let s = format!("{}", id);
        assert!(
            s.len() > 0,
            "Display format should produce non-empty string"
        );
        // UUID v4 format: 8-4-4-4-12 characters with hyphens
        assert!(s.contains('-'), "UUID should contain hyphens");
    }

    #[test]
    fn test_run_id_hash_consistency() {
        use std::collections::HashSet;

        let id = RunId::new();
        let mut set = HashSet::new();
        set.insert(id);
        assert!(set.contains(&id), "RunId should be consistently hashable");
    }

    #[test]
    fn test_run_id_default() {
        let id1 = RunId::default();
        let id2 = RunId::default();
        assert_ne!(id1, id2, "Default should create unique RunIds");
    }

    // ===== Namespace Tests =====

    #[test]
    fn test_namespace_construction() {
        let run_id = RunId::new();
        let ns = Namespace::new("tenant1", "app1", "agent1", run_id);

        assert_eq!(ns.tenant, "tenant1");
        assert_eq!(ns.app, "app1");
        assert_eq!(ns.agent, "agent1");
        assert_eq!(ns.run_id, run_id);
    }

    #[test]
    fn test_namespace_display() {
        let run_id = RunId::new();
        let ns = Namespace::new("acme", "chatbot", "agent-42", run_id);

        let display = format!("{}", ns);
        assert!(display.starts_with("acme/chatbot/agent-42/"));
        assert!(display.contains(&run_id.to_string()));
    }

    #[test]
    fn test_namespace_equality() {
        let run_id = RunId::new();
        let ns1 = Namespace::new("tenant1", "app1", "agent1", run_id);
        let ns2 = Namespace::new("tenant1", "app1", "agent1", run_id);
        let ns3 = Namespace::new("tenant2", "app1", "agent1", run_id);

        assert_eq!(ns1, ns2, "Same namespace should be equal");
        assert_ne!(ns1, ns3, "Different tenant should not be equal");
    }

    #[test]
    fn test_namespace_ordering() {
        let run1 = RunId::new();
        let run2 = RunId::new();

        let ns1 = Namespace::new("tenant1", "app1", "agent1", run1);
        let ns2 = Namespace::new("tenant1", "app1", "agent1", run2);
        let ns3 = Namespace::new("tenant2", "app1", "agent1", run1);
        let ns4 = Namespace::new("tenant1", "app2", "agent1", run1);
        let ns5 = Namespace::new("tenant1", "app1", "agent2", run1);

        // Same tenant/app/agent, different run_id - order depends on UUID
        assert_ne!(ns1, ns2);

        // Different tenant should sort differently
        assert!(ns1 < ns3, "tenant1 should be less than tenant2");

        // Different app within same tenant
        assert!(ns1 < ns4, "app1 should be less than app2");

        // Different agent within same tenant/app
        assert!(ns5 > ns1, "agent2 should be greater than agent1");
    }

    #[test]
    fn test_namespace_serialization() {
        let run_id = RunId::new();
        let ns = Namespace::new("acme", "myapp", "agent-42", run_id);

        let json = serde_json::to_string(&ns).unwrap();
        let ns2: Namespace = serde_json::from_str(&json).unwrap();

        assert_eq!(ns, ns2, "Namespace should roundtrip through JSON");
    }

    #[test]
    fn test_namespace_btreemap_ordering() {
        use std::collections::BTreeMap;

        let run1 = RunId::new();
        let run2 = RunId::new();

        let ns1 = Namespace::new("acme", "app1", "agent1", run1);
        let ns2 = Namespace::new("acme", "app1", "agent2", run2);
        let ns3 = Namespace::new("acme", "app2", "agent1", run1);

        let mut map = BTreeMap::new();
        map.insert(ns3.clone(), "value3");
        map.insert(ns1.clone(), "value1");
        map.insert(ns2.clone(), "value2");

        // Collect keys in order
        let keys: Vec<_> = map.keys().cloned().collect();

        // Should be ordered: ns1 (app1/agent1) < ns2 (app1/agent2) < ns3 (app2/agent1)
        assert_eq!(keys[0], ns1);
        assert_eq!(keys[1], ns2);
        assert_eq!(keys[2], ns3);
    }
}
