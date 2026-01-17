//! VectorStore: Vector storage and search primitive
//!
//! ## Design
//!
//! VectorStore is a stateless facade over the Database engine for collection
//! management. It holds:
//! - `Arc<Database>` for storage operations
//! - `RwLock<BTreeMap<CollectionId, Box<dyn VectorIndexBackend>>>` for in-memory index
//!
//! ## Run Isolation
//!
//! All operations are scoped to a `RunId`. Different runs cannot see
//! each other's collections or vectors.
//!
//! ## Thread Safety
//!
//! VectorStore is `Send + Sync` and can be safely shared across threads.

use crate::vector::collection::validate_collection_name;
use crate::vector::{
    CollectionId, CollectionInfo, CollectionRecord, IndexBackendFactory, VectorConfig, VectorError,
    VectorIndexBackend, VectorResult,
};
use in_mem_core::types::{Key, Namespace, RunId};
use in_mem_core::value::Value;
use in_mem_engine::Database;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

/// Vector storage and search primitive
///
/// Manages collections of vectors with similarity search capabilities.
/// Uses BTreeMap for deterministic iteration order (Invariant R3).
///
/// # Example
///
/// ```ignore
/// use in_mem_primitives::VectorStore;
/// use in_mem_engine::Database;
/// use in_mem_core::types::RunId;
///
/// let db = Arc::new(Database::open("/path/to/data")?);
/// let store = VectorStore::new(db);
/// let run_id = RunId::new();
///
/// // Create collection
/// let config = VectorConfig::for_minilm();
/// store.create_collection(run_id, "embeddings", config)?;
///
/// // List collections
/// let collections = store.list_collections(run_id)?;
/// ```
#[derive(Clone)]
pub struct VectorStore {
    db: Arc<Database>,
    /// In-memory index backends per collection
    /// CRITICAL: BTreeMap for deterministic iteration (Invariant R3)
    backends: Arc<RwLock<BTreeMap<CollectionId, Box<dyn VectorIndexBackend>>>>,
    /// Factory for creating index backends
    backend_factory: IndexBackendFactory,
}

impl VectorStore {
    /// Create a new VectorStore
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            backends: Arc::new(RwLock::new(BTreeMap::new())),
            backend_factory: IndexBackendFactory::default(),
        }
    }

    /// Create a new VectorStore with custom backend factory
    pub fn with_backend_factory(db: Arc<Database>, factory: IndexBackendFactory) -> Self {
        Self {
            db,
            backends: Arc::new(RwLock::new(BTreeMap::new())),
            backend_factory: factory,
        }
    }

    /// Get the underlying database reference
    pub fn database(&self) -> &Arc<Database> {
        &self.db
    }

    // ========================================================================
    // Collection Management (Epic 53)
    // ========================================================================

    /// Create a new collection
    ///
    /// Creates a collection with the specified configuration.
    /// The configuration (dimension, metric, dtype) is immutable after creation.
    ///
    /// # Errors
    /// - `CollectionAlreadyExists` if a collection with this name exists
    /// - `InvalidCollectionName` if name is invalid
    /// - `InvalidDimension` if dimension is 0
    pub fn create_collection(
        &self,
        run_id: RunId,
        name: &str,
        config: VectorConfig,
    ) -> VectorResult<CollectionInfo> {
        // Validate name
        validate_collection_name(name)?;

        // Validate config (dimension must be > 0)
        if config.dimension == 0 {
            return Err(VectorError::InvalidDimension {
                dimension: config.dimension,
            });
        }

        let collection_id = CollectionId::new(run_id, name);

        // Check if collection already exists
        if self.collection_exists(run_id, name)? {
            return Err(VectorError::CollectionAlreadyExists {
                name: name.to_string(),
            });
        }

        let now = now_micros();

        // Create collection record
        let record = CollectionRecord::new(&config);

        // Store config in KV
        let config_key = Key::new_vector_config(Namespace::for_run(run_id), name);
        let config_bytes = record.to_bytes()?;

        // Use transaction for atomic storage
        self.db
            .transaction(run_id, |txn| {
                txn.put(config_key.clone(), Value::Bytes(config_bytes.clone()))
            })
            .map_err(|e| VectorError::Storage(e.to_string()))?;

        // Initialize in-memory backend
        self.init_backend(&collection_id, &config);

        Ok(CollectionInfo {
            name: name.to_string(),
            config,
            count: 0,
            created_at: now,
        })
    }

    /// Delete a collection and all its vectors
    ///
    /// This is a destructive operation that:
    /// 1. Deletes all vectors in the collection
    /// 2. Deletes the collection configuration
    /// 3. Removes the in-memory backend
    ///
    /// # Errors
    /// - `CollectionNotFound` if collection doesn't exist
    pub fn delete_collection(&self, run_id: RunId, name: &str) -> VectorResult<()> {
        let collection_id = CollectionId::new(run_id, name);

        // Check if collection exists
        if !self.collection_exists(run_id, name)? {
            return Err(VectorError::CollectionNotFound {
                name: name.to_string(),
            });
        }

        // Delete all vectors in the collection
        self.delete_all_vectors(run_id, name)?;

        // Delete config from KV
        let config_key = Key::new_vector_config(Namespace::for_run(run_id), name);
        self.db
            .transaction(run_id, |txn| txn.delete(config_key.clone()))
            .map_err(|e| VectorError::Storage(e.to_string()))?;

        // Remove in-memory backend
        self.backends.write().unwrap().remove(&collection_id);

        Ok(())
    }

    /// List all collections for a run
    ///
    /// Returns CollectionInfo for each collection, including current vector count.
    /// Results are sorted by name for determinism (Invariant R4).
    pub fn list_collections(&self, run_id: RunId) -> VectorResult<Vec<CollectionInfo>> {
        use in_mem_core::traits::SnapshotView;

        let namespace = Namespace::for_run(run_id);
        let prefix = Key::new_vector_config_prefix(namespace);

        // Read from snapshot for consistency
        let snapshot = self.db.storage().create_snapshot();
        let entries = snapshot
            .scan_prefix(&prefix)
            .map_err(|e| VectorError::Storage(e.to_string()))?;

        let mut collections = Vec::new();

        for (key, versioned_value) in entries {
            // Extract collection name from key
            let name = String::from_utf8(key.user_key.clone())
                .map_err(|e| VectorError::Serialization(e.to_string()))?;

            // Deserialize the record from the stored bytes
            let bytes = match &versioned_value.value {
                Value::Bytes(b) => b.clone(),
                _ => {
                    return Err(VectorError::Serialization(
                        "Expected Bytes value for collection record".to_string(),
                    ))
                }
            };
            let record = CollectionRecord::from_bytes(&bytes)?;
            let config = VectorConfig::try_from(record.config)?;

            // Get current count from backend
            let collection_id = CollectionId::new(run_id, &name);
            let count = self.get_collection_count(&collection_id, run_id, &name)?;

            collections.push(CollectionInfo {
                name,
                config,
                count,
                created_at: record.created_at,
            });
        }

        // Sort by name for determinism
        collections.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(collections)
    }

    /// Get a single collection's info
    ///
    /// Returns None if collection doesn't exist.
    pub fn get_collection(
        &self,
        run_id: RunId,
        name: &str,
    ) -> VectorResult<Option<CollectionInfo>> {
        let config_key = Key::new_vector_config(Namespace::for_run(run_id), name);

        // Read from snapshot
        use in_mem_core::traits::SnapshotView;
        let snapshot = self.db.storage().create_snapshot();

        let Some(versioned_value) = snapshot
            .get(&config_key)
            .map_err(|e| VectorError::Storage(e.to_string()))?
        else {
            return Ok(None);
        };

        // Deserialize the record
        let bytes = match &versioned_value.value {
            Value::Bytes(b) => b.clone(),
            _ => {
                return Err(VectorError::Serialization(
                    "Expected Bytes value for collection record".to_string(),
                ))
            }
        };
        let record = CollectionRecord::from_bytes(&bytes)?;
        let config = VectorConfig::try_from(record.config)?;

        let collection_id = CollectionId::new(run_id, name);
        let count = self.get_collection_count(&collection_id, run_id, name)?;

        Ok(Some(CollectionInfo {
            name: name.to_string(),
            config,
            count,
            created_at: record.created_at,
        }))
    }

    /// Check if a collection exists
    pub fn collection_exists(&self, run_id: RunId, name: &str) -> VectorResult<bool> {
        use in_mem_core::traits::SnapshotView;

        let config_key = Key::new_vector_config(Namespace::for_run(run_id), name);
        let snapshot = self.db.storage().create_snapshot();

        Ok(snapshot
            .get(&config_key)
            .map_err(|e| VectorError::Storage(e.to_string()))?
            .is_some())
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    /// Initialize the index backend for a collection
    fn init_backend(&self, id: &CollectionId, config: &VectorConfig) {
        let backend = self.backend_factory.create(config);
        self.backends.write().unwrap().insert(id.clone(), backend);
    }

    /// Get the current vector count for a collection
    fn get_collection_count(
        &self,
        id: &CollectionId,
        run_id: RunId,
        name: &str,
    ) -> VectorResult<usize> {
        // Check in-memory backend first
        let backends = self.backends.read().unwrap();
        if let Some(backend) = backends.get(id) {
            return Ok(backend.len());
        }
        drop(backends);

        // Backend not loaded - count from KV
        use in_mem_core::traits::SnapshotView;
        let namespace = Namespace::for_run(run_id);
        let prefix = Key::vector_collection_prefix(namespace, name);

        let snapshot = self.db.storage().create_snapshot();
        let entries = snapshot
            .scan_prefix(&prefix)
            .map_err(|e| VectorError::Storage(e.to_string()))?;

        Ok(entries.len())
    }

    /// Delete all vectors in a collection
    fn delete_all_vectors(&self, run_id: RunId, name: &str) -> VectorResult<()> {
        use in_mem_core::traits::SnapshotView;

        let namespace = Namespace::for_run(run_id);
        let prefix = Key::vector_collection_prefix(namespace, name);

        // Scan all vector keys in this collection
        let snapshot = self.db.storage().create_snapshot();
        let entries = snapshot
            .scan_prefix(&prefix)
            .map_err(|e| VectorError::Storage(e.to_string()))?;

        let keys: Vec<Key> = entries.into_iter().map(|(key, _)| key).collect();

        // Delete each vector in a transaction
        if !keys.is_empty() {
            self.db
                .transaction(run_id, |txn| {
                    for key in &keys {
                        let k: Key = key.clone();
                        txn.delete(k)?;
                    }
                    Ok(())
                })
                .map_err(|e| VectorError::Storage(e.to_string()))?;
        }

        Ok(())
    }

    /// Load collection config from KV
    fn load_collection_config(
        &self,
        run_id: RunId,
        name: &str,
    ) -> VectorResult<Option<VectorConfig>> {
        use in_mem_core::traits::SnapshotView;

        let config_key = Key::new_vector_config(Namespace::for_run(run_id), name);
        let snapshot = self.db.storage().create_snapshot();

        let Some(versioned_value) = snapshot
            .get(&config_key)
            .map_err(|e| VectorError::Storage(e.to_string()))?
        else {
            return Ok(None);
        };

        let bytes = match &versioned_value.value {
            Value::Bytes(b) => b.clone(),
            _ => {
                return Err(VectorError::Serialization(
                    "Expected Bytes value for collection record".to_string(),
                ))
            }
        };

        let record = CollectionRecord::from_bytes(&bytes)?;
        let config = VectorConfig::try_from(record.config)?;
        Ok(Some(config))
    }

    /// Ensure collection is loaded into memory
    ///
    /// If the collection exists in KV but not in memory (after recovery),
    /// this loads it and initializes the backend.
    pub fn ensure_collection_loaded(&self, run_id: RunId, name: &str) -> VectorResult<()> {
        let collection_id = CollectionId::new(run_id, name);

        // Already loaded?
        if self.backends.read().unwrap().contains_key(&collection_id) {
            return Ok(());
        }

        // Load from KV
        let config = self.load_collection_config(run_id, name)?.ok_or_else(|| {
            VectorError::CollectionNotFound {
                name: name.to_string(),
            }
        })?;

        // Initialize backend
        self.init_backend(&collection_id, &config);

        // Note: Loading vectors into backend happens in Epic 55 (recovery)

        Ok(())
    }
}

/// Get current time in microseconds since Unix epoch
fn now_micros() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_micros() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector::{DistanceMetric, VectorConfig};
    use tempfile::TempDir;

    fn setup() -> (TempDir, Arc<Database>, VectorStore) {
        let temp_dir = TempDir::new().unwrap();
        let db = Arc::new(Database::open(temp_dir.path()).unwrap());
        let store = VectorStore::new(db.clone());
        (temp_dir, db, store)
    }

    // ========================================
    // Collection Lifecycle Tests (#347, #348)
    // ========================================

    #[test]
    fn test_create_collection() {
        let (_temp, _db, store) = setup();
        let run_id = RunId::new();

        let config = VectorConfig::for_minilm();
        let info = store
            .create_collection(run_id, "test", config.clone())
            .unwrap();

        assert_eq!(info.name, "test");
        assert_eq!(info.count, 0);
        assert_eq!(info.config.dimension, 384);
        assert_eq!(info.config.metric, DistanceMetric::Cosine);
    }

    #[test]
    fn test_collection_already_exists() {
        let (_temp, _db, store) = setup();
        let run_id = RunId::new();

        let config = VectorConfig::for_minilm();
        store
            .create_collection(run_id, "test", config.clone())
            .unwrap();

        // Second create should fail
        let result = store.create_collection(run_id, "test", config);
        assert!(matches!(
            result,
            Err(VectorError::CollectionAlreadyExists { .. })
        ));
    }

    #[test]
    fn test_delete_collection() {
        let (_temp, _db, store) = setup();
        let run_id = RunId::new();

        let config = VectorConfig::for_minilm();
        store
            .create_collection(run_id, "test", config.clone())
            .unwrap();

        // Delete should succeed
        store.delete_collection(run_id, "test").unwrap();

        // Collection should no longer exist
        assert!(!store.collection_exists(run_id, "test").unwrap());
    }

    #[test]
    fn test_delete_collection_not_found() {
        let (_temp, _db, store) = setup();
        let run_id = RunId::new();

        let result = store.delete_collection(run_id, "nonexistent");
        assert!(matches!(
            result,
            Err(VectorError::CollectionNotFound { .. })
        ));
    }

    // ========================================
    // Collection Discovery Tests (#349)
    // ========================================

    #[test]
    fn test_list_collections() {
        let (_temp, _db, store) = setup();
        let run_id = RunId::new();

        // Create multiple collections
        store
            .create_collection(run_id, "zeta", VectorConfig::for_minilm())
            .unwrap();
        store
            .create_collection(run_id, "alpha", VectorConfig::for_mpnet())
            .unwrap();
        store
            .create_collection(run_id, "beta", VectorConfig::for_openai_ada())
            .unwrap();

        let collections = store.list_collections(run_id).unwrap();

        // Should be sorted by name
        assert_eq!(collections.len(), 3);
        assert_eq!(collections[0].name, "alpha");
        assert_eq!(collections[1].name, "beta");
        assert_eq!(collections[2].name, "zeta");
    }

    #[test]
    fn test_list_collections_empty() {
        let (_temp, _db, store) = setup();
        let run_id = RunId::new();

        let collections = store.list_collections(run_id).unwrap();
        assert!(collections.is_empty());
    }

    #[test]
    fn test_get_collection() {
        let (_temp, _db, store) = setup();
        let run_id = RunId::new();

        let config = VectorConfig::new(768, DistanceMetric::Euclidean).unwrap();
        store
            .create_collection(run_id, "embeddings", config)
            .unwrap();

        let info = store.get_collection(run_id, "embeddings").unwrap().unwrap();
        assert_eq!(info.name, "embeddings");
        assert_eq!(info.config.dimension, 768);
        assert_eq!(info.config.metric, DistanceMetric::Euclidean);
    }

    #[test]
    fn test_get_collection_not_found() {
        let (_temp, _db, store) = setup();
        let run_id = RunId::new();

        let info = store.get_collection(run_id, "nonexistent").unwrap();
        assert!(info.is_none());
    }

    // ========================================
    // Run Isolation Tests (Rule #2)
    // ========================================

    #[test]
    fn test_run_isolation() {
        let (_temp, _db, store) = setup();
        let run1 = RunId::new();
        let run2 = RunId::new();

        let config = VectorConfig::for_minilm();

        // Create same-named collection in different runs
        store
            .create_collection(run1, "shared_name", config.clone())
            .unwrap();
        store
            .create_collection(run2, "shared_name", config)
            .unwrap();

        // Each run sees only its own collection
        let list1 = store.list_collections(run1).unwrap();
        let list2 = store.list_collections(run2).unwrap();

        assert_eq!(list1.len(), 1);
        assert_eq!(list2.len(), 1);

        // Deleting from one run doesn't affect the other
        store.delete_collection(run1, "shared_name").unwrap();
        assert!(store.get_collection(run2, "shared_name").unwrap().is_some());
    }

    // ========================================
    // Config Persistence Tests (#350)
    // ========================================

    #[test]
    fn test_collection_config_roundtrip() {
        let (_temp, _db, store) = setup();
        let run_id = RunId::new();

        let config = VectorConfig::new(768, DistanceMetric::Euclidean).unwrap();
        store
            .create_collection(run_id, "test", config.clone())
            .unwrap();

        // Get collection and verify config
        let info = store.get_collection(run_id, "test").unwrap().unwrap();
        assert_eq!(info.config.dimension, config.dimension);
        assert_eq!(info.config.metric, config.metric);
    }

    #[test]
    fn test_collection_survives_reload() {
        let temp_dir = TempDir::new().unwrap();
        let run_id = RunId::new();

        // Create collection
        {
            let db = Arc::new(Database::open(temp_dir.path()).unwrap());
            let store = VectorStore::new(db);

            let config = VectorConfig::new(512, DistanceMetric::DotProduct).unwrap();
            store
                .create_collection(run_id, "persistent", config)
                .unwrap();
        }

        // Reopen database and verify collection exists
        {
            let db = Arc::new(Database::open(temp_dir.path()).unwrap());
            let store = VectorStore::new(db);

            let info = store.get_collection(run_id, "persistent").unwrap().unwrap();
            assert_eq!(info.config.dimension, 512);
            assert_eq!(info.config.metric, DistanceMetric::DotProduct);
        }
    }

    // ========================================
    // Validation Tests
    // ========================================

    #[test]
    fn test_invalid_collection_name() {
        let (_temp, _db, store) = setup();
        let run_id = RunId::new();

        let config = VectorConfig::for_minilm();

        // Empty name
        let result = store.create_collection(run_id, "", config.clone());
        assert!(matches!(
            result,
            Err(VectorError::InvalidCollectionName { .. })
        ));

        // Reserved name
        let result = store.create_collection(run_id, "_reserved", config.clone());
        assert!(matches!(
            result,
            Err(VectorError::InvalidCollectionName { .. })
        ));

        // Contains slash
        let result = store.create_collection(run_id, "has/slash", config);
        assert!(matches!(
            result,
            Err(VectorError::InvalidCollectionName { .. })
        ));
    }

    #[test]
    fn test_invalid_dimension() {
        let (_temp, _db, store) = setup();
        let run_id = RunId::new();

        // Dimension 0 should fail
        let config = VectorConfig {
            dimension: 0,
            metric: DistanceMetric::Cosine,
            storage_dtype: crate::vector::StorageDtype::F32,
        };

        let result = store.create_collection(run_id, "test", config);
        assert!(matches!(
            result,
            Err(VectorError::InvalidDimension { dimension: 0 })
        ));
    }

    // ========================================
    // Thread Safety Tests
    // ========================================

    #[test]
    fn test_vector_store_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<VectorStore>();
    }

    #[test]
    fn test_vector_store_clone() {
        let (_temp, _db, store1) = setup();
        let store2 = store1.clone();

        // Both point to same database
        assert!(Arc::ptr_eq(store1.database(), store2.database()));
    }
}
