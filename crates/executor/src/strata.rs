//! High-level typed wrapper for the Executor.
//!
//! The [`Strata`] struct provides a convenient Rust API that wraps the
//! [`Executor`] and [`Command`]/[`Output`] enums with typed method calls.
//!
//! # Example
//!
//! ```ignore
//! use strata_executor::Strata;
//! use strata_core::Value;
//!
//! let db = Strata::new(substrate);
//!
//! // Type-safe KV operations
//! db.kv_put("default", "key", Value::String("hello".into()))?;
//! let value = db.kv_get("default", "key")?;
//! ```

use std::sync::Arc;

use strata_api::substrate::SubstrateImpl;
use strata_core::Value;

use crate::types::*;
use crate::{Command, Error, Executor, Output, Result};

/// High-level typed wrapper for database operations.
///
/// `Strata` provides a convenient Rust API that wraps the executor's
/// command-based interface with typed method calls. Each method:
///
/// 1. Creates the appropriate [`Command`]
/// 2. Executes it via the [`Executor`]
/// 3. Extracts and returns the typed result
///
/// This provides a more ergonomic API for Rust users while maintaining
/// the same semantics as the command-based interface.
pub struct Strata {
    executor: Executor,
}

impl Strata {
    /// Create a new Strata instance wrapping the given substrate.
    pub fn new(substrate: Arc<SubstrateImpl>) -> Self {
        Self {
            executor: Executor::new(substrate),
        }
    }

    /// Get the underlying executor.
    pub fn executor(&self) -> &Executor {
        &self.executor
    }

    // =========================================================================
    // Database Operations
    // =========================================================================

    /// Ping the database.
    pub fn ping(&self) -> Result<String> {
        match self.executor.execute(Command::Ping)? {
            Output::Pong { version } => Ok(version),
            _ => Err(Error::Internal {
                reason: "Unexpected output for Ping".into(),
            }),
        }
    }

    /// Get database info.
    pub fn info(&self) -> Result<DatabaseInfo> {
        match self.executor.execute(Command::Info)? {
            Output::DatabaseInfo(info) => Ok(info),
            _ => Err(Error::Internal {
                reason: "Unexpected output for Info".into(),
            }),
        }
    }

    /// Flush the database to disk.
    pub fn flush(&self) -> Result<()> {
        match self.executor.execute(Command::Flush)? {
            Output::Unit => Ok(()),
            _ => Err(Error::Internal {
                reason: "Unexpected output for Flush".into(),
            }),
        }
    }

    /// Compact the database.
    pub fn compact(&self) -> Result<()> {
        match self.executor.execute(Command::Compact)? {
            Output::Unit => Ok(()),
            _ => Err(Error::Internal {
                reason: "Unexpected output for Compact".into(),
            }),
        }
    }

    // =========================================================================
    // KV Operations
    // =========================================================================

    /// Put a value in the KV store.
    pub fn kv_put(&self, run: &str, key: &str, value: Value) -> Result<u64> {
        match self.executor.execute(Command::KvPut {
            run: RunId::from(run),
            key: key.to_string(),
            value,
        })? {
            Output::Version(v) => Ok(v),
            _ => Err(Error::Internal {
                reason: "Unexpected output for KvPut".into(),
            }),
        }
    }

    /// Get a value from the KV store.
    pub fn kv_get(&self, run: &str, key: &str) -> Result<Option<VersionedValue>> {
        match self.executor.execute(Command::KvGet {
            run: RunId::from(run),
            key: key.to_string(),
        })? {
            Output::MaybeVersioned(v) => Ok(v),
            _ => Err(Error::Internal {
                reason: "Unexpected output for KvGet".into(),
            }),
        }
    }

    /// Delete a key from the KV store.
    pub fn kv_delete(&self, run: &str, key: &str) -> Result<bool> {
        match self.executor.execute(Command::KvDelete {
            run: RunId::from(run),
            key: key.to_string(),
        })? {
            Output::Bool(deleted) => Ok(deleted),
            _ => Err(Error::Internal {
                reason: "Unexpected output for KvDelete".into(),
            }),
        }
    }

    /// Check if a key exists in the KV store.
    pub fn kv_exists(&self, run: &str, key: &str) -> Result<bool> {
        match self.executor.execute(Command::KvExists {
            run: RunId::from(run),
            key: key.to_string(),
        })? {
            Output::Bool(exists) => Ok(exists),
            _ => Err(Error::Internal {
                reason: "Unexpected output for KvExists".into(),
            }),
        }
    }

    /// Increment a counter in the KV store.
    pub fn kv_incr(&self, run: &str, key: &str, delta: i64) -> Result<i64> {
        match self.executor.execute(Command::KvIncr {
            run: RunId::from(run),
            key: key.to_string(),
            delta,
        })? {
            Output::Int(val) => Ok(val),
            _ => Err(Error::Internal {
                reason: "Unexpected output for KvIncr".into(),
            }),
        }
    }

    /// Get multiple values from the KV store.
    pub fn kv_mget(&self, run: &str, keys: Vec<String>) -> Result<Vec<Option<VersionedValue>>> {
        match self.executor.execute(Command::KvMget {
            run: RunId::from(run),
            keys,
        })? {
            Output::Values(vals) => Ok(vals),
            _ => Err(Error::Internal {
                reason: "Unexpected output for KvMget".into(),
            }),
        }
    }

    /// Put multiple values in the KV store.
    pub fn kv_mput(&self, run: &str, entries: Vec<(String, Value)>) -> Result<u64> {
        match self.executor.execute(Command::KvMput {
            run: RunId::from(run),
            entries,
        })? {
            Output::Version(v) => Ok(v),
            _ => Err(Error::Internal {
                reason: "Unexpected output for KvMput".into(),
            }),
        }
    }

    // =========================================================================
    // JSON Operations
    // =========================================================================

    /// Set a JSON value at a path.
    pub fn json_set(&self, run: &str, key: &str, path: &str, value: Value) -> Result<u64> {
        match self.executor.execute(Command::JsonSet {
            run: RunId::from(run),
            key: key.to_string(),
            path: path.to_string(),
            value,
        })? {
            Output::Version(v) => Ok(v),
            _ => Err(Error::Internal {
                reason: "Unexpected output for JsonSet".into(),
            }),
        }
    }

    /// Get a JSON value at a path.
    pub fn json_get(&self, run: &str, key: &str, path: &str) -> Result<Option<VersionedValue>> {
        match self.executor.execute(Command::JsonGet {
            run: RunId::from(run),
            key: key.to_string(),
            path: path.to_string(),
        })? {
            Output::MaybeVersioned(v) => Ok(v),
            _ => Err(Error::Internal {
                reason: "Unexpected output for JsonGet".into(),
            }),
        }
    }

    /// Check if a JSON document exists.
    pub fn json_exists(&self, run: &str, key: &str) -> Result<bool> {
        match self.executor.execute(Command::JsonExists {
            run: RunId::from(run),
            key: key.to_string(),
        })? {
            Output::Bool(exists) => Ok(exists),
            _ => Err(Error::Internal {
                reason: "Unexpected output for JsonExists".into(),
            }),
        }
    }

    // =========================================================================
    // Event Operations
    // =========================================================================

    /// Append an event to a stream.
    pub fn event_append(&self, run: &str, stream: &str, payload: Value) -> Result<u64> {
        match self.executor.execute(Command::EventAppend {
            run: RunId::from(run),
            stream: stream.to_string(),
            payload,
        })? {
            Output::Version(v) => Ok(v),
            _ => Err(Error::Internal {
                reason: "Unexpected output for EventAppend".into(),
            }),
        }
    }

    /// Get events from a stream in a range.
    pub fn event_range(
        &self,
        run: &str,
        stream: &str,
        start: Option<u64>,
        end: Option<u64>,
        limit: Option<u64>,
    ) -> Result<Vec<VersionedValue>> {
        match self.executor.execute(Command::EventRange {
            run: RunId::from(run),
            stream: stream.to_string(),
            start,
            end,
            limit,
        })? {
            Output::VersionedValues(events) => Ok(events),
            _ => Err(Error::Internal {
                reason: "Unexpected output for EventRange".into(),
            }),
        }
    }

    /// List all event streams in a run.
    pub fn event_streams(&self, run: &str) -> Result<Vec<String>> {
        match self.executor.execute(Command::EventStreams {
            run: RunId::from(run),
        })? {
            Output::Strings(streams) => Ok(streams),
            _ => Err(Error::Internal {
                reason: "Unexpected output for EventStreams".into(),
            }),
        }
    }

    // =========================================================================
    // State Operations
    // =========================================================================

    /// Set a state cell value.
    pub fn state_set(&self, run: &str, cell: &str, value: Value) -> Result<u64> {
        match self.executor.execute(Command::StateSet {
            run: RunId::from(run),
            cell: cell.to_string(),
            value,
        })? {
            Output::Version(v) => Ok(v),
            _ => Err(Error::Internal {
                reason: "Unexpected output for StateSet".into(),
            }),
        }
    }

    /// Get a state cell value.
    pub fn state_get(&self, run: &str, cell: &str) -> Result<Option<VersionedValue>> {
        match self.executor.execute(Command::StateGet {
            run: RunId::from(run),
            cell: cell.to_string(),
        })? {
            Output::MaybeVersioned(v) => Ok(v),
            _ => Err(Error::Internal {
                reason: "Unexpected output for StateGet".into(),
            }),
        }
    }

    // =========================================================================
    // Vector Operations
    // =========================================================================

    /// Create a vector collection.
    pub fn vector_create_collection(
        &self,
        run: &str,
        collection: &str,
        dimension: u64,
        metric: DistanceMetric,
    ) -> Result<u64> {
        match self.executor.execute(Command::VectorCreateCollection {
            run: RunId::from(run),
            collection: collection.to_string(),
            dimension,
            metric,
        })? {
            Output::Version(v) => Ok(v),
            _ => Err(Error::Internal {
                reason: "Unexpected output for VectorCreateCollection".into(),
            }),
        }
    }

    /// Upsert a vector.
    pub fn vector_upsert(
        &self,
        run: &str,
        collection: &str,
        key: &str,
        vector: Vec<f32>,
        metadata: Option<Value>,
    ) -> Result<u64> {
        match self.executor.execute(Command::VectorUpsert {
            run: RunId::from(run),
            collection: collection.to_string(),
            key: key.to_string(),
            vector,
            metadata,
        })? {
            Output::Version(v) => Ok(v),
            _ => Err(Error::Internal {
                reason: "Unexpected output for VectorUpsert".into(),
            }),
        }
    }

    /// Search for similar vectors.
    pub fn vector_search(
        &self,
        run: &str,
        collection: &str,
        query: Vec<f32>,
        k: u64,
    ) -> Result<Vec<VectorMatch>> {
        match self.executor.execute(Command::VectorSearch {
            run: RunId::from(run),
            collection: collection.to_string(),
            query,
            k,
            filter: None,
            metric: None,
        })? {
            Output::VectorMatches(matches) => Ok(matches),
            _ => Err(Error::Internal {
                reason: "Unexpected output for VectorSearch".into(),
            }),
        }
    }

    // =========================================================================
    // Run Operations
    // =========================================================================

    /// Create a new run.
    pub fn run_create(
        &self,
        run_id: Option<String>,
        metadata: Option<Value>,
    ) -> Result<(RunInfo, u64)> {
        match self.executor.execute(Command::RunCreate { run_id, metadata })? {
            Output::RunWithVersion { info, version } => Ok((info, version)),
            _ => Err(Error::Internal {
                reason: "Unexpected output for RunCreate".into(),
            }),
        }
    }

    /// List runs.
    pub fn run_list(
        &self,
        state: Option<RunStatus>,
        limit: Option<u64>,
        offset: Option<u64>,
    ) -> Result<Vec<VersionedRunInfo>> {
        match self.executor.execute(Command::RunList {
            state,
            limit,
            offset,
        })? {
            Output::RunInfoList(runs) => Ok(runs),
            _ => Err(Error::Internal {
                reason: "Unexpected output for RunList".into(),
            }),
        }
    }

    /// Get run info.
    pub fn run_get(&self, run: &str) -> Result<Option<VersionedRunInfo>> {
        match self.executor.execute(Command::RunGet {
            run: RunId::from(run),
        })? {
            Output::RunInfoVersioned(info) => Ok(Some(info)),
            Output::Maybe(None) => Ok(None),
            _ => Err(Error::Internal {
                reason: "Unexpected output for RunGet".into(),
            }),
        }
    }

    /// Close a run.
    pub fn run_close(&self, run: &str) -> Result<u64> {
        match self.executor.execute(Command::RunClose {
            run: RunId::from(run),
        })? {
            Output::Version(v) => Ok(v),
            _ => Err(Error::Internal {
                reason: "Unexpected output for RunClose".into(),
            }),
        }
    }

    /// Delete a run.
    pub fn run_delete(&self, run: &str) -> Result<()> {
        match self.executor.execute(Command::RunDelete {
            run: RunId::from(run),
        })? {
            Output::Unit => Ok(()),
            _ => Err(Error::Internal {
                reason: "Unexpected output for RunDelete".into(),
            }),
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use strata_engine::Database;

    fn create_strata() -> Strata {
        let db = Arc::new(Database::builder().no_durability().open_temp().unwrap());
        let substrate = Arc::new(SubstrateImpl::new(db));
        Strata::new(substrate)
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

        let version = db.kv_put("default", "key1", Value::String("hello".into())).unwrap();
        assert!(version > 0);

        let value = db.kv_get("default", "key1").unwrap();
        assert!(value.is_some());
        assert_eq!(value.unwrap().value, Value::String("hello".into()));
    }

    #[test]
    fn test_kv_exists_delete() {
        let db = create_strata();

        db.kv_put("default", "key1", Value::Int(42)).unwrap();
        assert!(db.kv_exists("default", "key1").unwrap());

        db.kv_delete("default", "key1").unwrap();
        assert!(!db.kv_exists("default", "key1").unwrap());
    }

    #[test]
    fn test_kv_incr() {
        let db = create_strata();

        db.kv_put("default", "counter", Value::Int(10)).unwrap();
        let val = db.kv_incr("default", "counter", 5).unwrap();
        assert_eq!(val, 15);
    }

    #[test]
    fn test_state_set_get() {
        let db = create_strata();

        db.state_set("default", "cell", Value::String("state".into())).unwrap();
        let value = db.state_get("default", "cell").unwrap();
        assert!(value.is_some());
        assert_eq!(value.unwrap().value, Value::String("state".into()));
    }

    #[test]
    fn test_event_append_range() {
        let db = create_strata();

        // Event payloads must be Objects
        db.event_append("default", "stream", Value::Object(
            [("value".to_string(), Value::Int(1))].into_iter().collect()
        )).unwrap();
        db.event_append("default", "stream", Value::Object(
            [("value".to_string(), Value::Int(2))].into_iter().collect()
        )).unwrap();

        let events = db.event_range("default", "stream", None, None, None).unwrap();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_vector_operations() {
        let db = create_strata();

        db.vector_create_collection("default", "vecs", 4u64, DistanceMetric::Cosine).unwrap();
        db.vector_upsert("default", "vecs", "v1", vec![1.0, 0.0, 0.0, 0.0], None).unwrap();
        db.vector_upsert("default", "vecs", "v2", vec![0.0, 1.0, 0.0, 0.0], None).unwrap();

        let matches = db.vector_search("default", "vecs", vec![1.0, 0.0, 0.0, 0.0], 10u64).unwrap();
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
}
