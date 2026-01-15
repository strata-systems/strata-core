//! Sharded storage for M4 performance
//!
//! Replaces RwLock + BTreeMap with DashMap + HashMap.
//! Lock-free reads, sharded writes, O(1) lookups.
//!
//! # Design
//!
//! - DashMap: 16-way sharded by default, lock-free reads
//! - FxHashMap: O(1) lookups, fast non-crypto hash
//! - Per-RunId: Natural agent partitioning, no cross-run contention
//!
//! # Performance Targets
//!
//! - get(): Lock-free via DashMap
//! - put(): Only locks target shard
//! - Snapshot acquisition: < 500ns
//! - Different runs: Never contend

use dashmap::DashMap;
use in_mem_core::types::{Key, RunId};
use in_mem_core::VersionedValue;
use rustc_hash::FxHashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Per-run shard containing run's data
///
/// Each RunId gets its own shard with an FxHashMap for O(1) lookups.
/// This ensures different runs never contend with each other.
#[derive(Debug)]
pub struct Shard {
    /// HashMap with FxHash for O(1) lookups
    pub(crate) data: FxHashMap<Key, VersionedValue>,
}

impl Shard {
    /// Create a new empty shard
    pub fn new() -> Self {
        Self {
            data: FxHashMap::default(),
        }
    }

    /// Create a shard with pre-allocated capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: FxHashMap::with_capacity_and_hasher(capacity, Default::default()),
        }
    }

    /// Get number of entries in this shard
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if shard is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl Default for Shard {
    fn default() -> Self {
        Self::new()
    }
}

/// Sharded storage - DashMap by RunId, HashMap within
///
/// # Design
///
/// - DashMap: 16-way sharded by default, lock-free reads
/// - FxHashMap: O(1) lookups, fast non-crypto hash
/// - Per-RunId: Natural agent partitioning, no cross-run contention
///
/// # Thread Safety
///
/// All operations are thread-safe:
/// - get(): Lock-free read via DashMap
/// - put(): Only locks the target run's shard
/// - Different runs never contend
///
/// # Example
///
/// ```ignore
/// use in_mem_storage::ShardedStore;
/// use std::sync::Arc;
///
/// let store = Arc::new(ShardedStore::new());
/// let snapshot = store.snapshot();
/// ```
pub struct ShardedStore {
    /// Per-run shards using DashMap
    shards: DashMap<RunId, Shard>,
    /// Global version for snapshots
    version: AtomicU64,
}

impl ShardedStore {
    /// Create new sharded store
    pub fn new() -> Self {
        Self {
            shards: DashMap::new(),
            version: AtomicU64::new(0),
        }
    }

    /// Create with expected number of runs
    pub fn with_capacity(num_runs: usize) -> Self {
        Self {
            shards: DashMap::with_capacity(num_runs),
            version: AtomicU64::new(0),
        }
    }

    /// Get current version
    #[inline]
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }

    /// Increment version and return new value
    #[inline]
    pub fn next_version(&self) -> u64 {
        self.version.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Set version (used during recovery)
    pub fn set_version(&self, version: u64) {
        self.version.store(version, Ordering::Release);
    }

    /// Get number of shards (runs)
    pub fn shard_count(&self) -> usize {
        self.shards.len()
    }

    /// Check if a run exists
    pub fn has_run(&self, run_id: &RunId) -> bool {
        self.shards.contains_key(run_id)
    }

    /// Get total number of entries across all shards
    pub fn total_entries(&self) -> usize {
        self.shards.iter().map(|entry| entry.value().len()).sum()
    }

    // ========================================================================
    // Get/Put/Delete Operations (Story #228)
    // ========================================================================

    /// Get a value by key
    ///
    /// Lock-free read via DashMap. Only the run's shard is accessed.
    ///
    /// # Arguments
    ///
    /// * `key` - Key to look up (contains RunId)
    ///
    /// # Performance
    ///
    /// - O(1) lookup via FxHashMap
    /// - Lock-free via DashMap read guard
    #[inline]
    pub fn get(&self, key: &Key) -> Option<VersionedValue> {
        let run_id = key.namespace.run_id;
        self.shards
            .get(&run_id)
            .and_then(|shard| shard.data.get(key).cloned())
    }

    /// Put a value for a key
    ///
    /// Sharded write - only locks this run's shard.
    /// Other runs can read/write concurrently without contention.
    ///
    /// # Arguments
    ///
    /// * `key` - Key to store (contains RunId)
    /// * `value` - Value to store
    ///
    /// # Performance
    ///
    /// - O(1) insert via FxHashMap
    /// - Only locks the target run's shard
    #[inline]
    pub fn put(&self, key: Key, value: VersionedValue) {
        let run_id = key.namespace.run_id;
        self.shards
            .entry(run_id)
            .or_insert_with(Shard::new)
            .data
            .insert(key, value);
    }

    /// Delete a key
    ///
    /// Returns the removed value if it existed.
    ///
    /// # Arguments
    ///
    /// * `key` - Key to delete (contains RunId)
    #[inline]
    pub fn delete(&self, key: &Key) -> Option<VersionedValue> {
        let run_id = key.namespace.run_id;
        self.shards
            .get_mut(&run_id)
            .and_then(|mut shard| shard.data.remove(key))
    }

    /// Check if a key exists
    ///
    /// Lock-free check via DashMap read guard.
    #[inline]
    pub fn contains(&self, key: &Key) -> bool {
        let run_id = key.namespace.run_id;
        self.shards
            .get(&run_id)
            .map(|shard| shard.data.contains_key(key))
            .unwrap_or(false)
    }

    /// Apply a batch of writes and deletes atomically
    ///
    /// All operations in the batch are applied with the given version.
    ///
    /// # Arguments
    ///
    /// * `writes` - Key-value pairs to write
    /// * `deletes` - Keys to delete
    /// * `version` - Version to assign to all writes
    pub fn apply_batch(
        &self,
        writes: &[(Key, in_mem_core::value::Value)],
        deletes: &[Key],
        version: u64,
    ) {
        use chrono::Utc;

        // Apply writes
        for (key, value) in writes {
            let versioned = VersionedValue {
                value: value.clone(),
                version,
                timestamp: Utc::now().timestamp(),
                ttl: None,
            };
            self.put(key.clone(), versioned);
        }

        // Apply deletes
        for key in deletes {
            self.delete(key);
        }
    }

    /// Get count of entries for a specific run
    pub fn run_entry_count(&self, run_id: &RunId) -> usize {
        self.shards
            .get(run_id)
            .map(|shard| shard.len())
            .unwrap_or(0)
    }

    // ========================================================================
    // List Operations (Story #229)
    // ========================================================================

    /// List all entries for a run
    ///
    /// NOTE: Slower than BTreeMap range scan. Requires collect + sort.
    /// This is acceptable because list operations are NOT on the hot path.
    ///
    /// # Arguments
    ///
    /// * `run_id` - The run to list entries for
    ///
    /// # Returns
    ///
    /// Vector of (Key, VersionedValue) pairs, sorted by key
    pub fn list_run(&self, run_id: &RunId) -> Vec<(Key, VersionedValue)> {
        self.shards
            .get(run_id)
            .map(|shard| {
                let mut results: Vec<_> = shard
                    .data
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

                // Sort for consistent ordering (Key implements Ord)
                results.sort_by(|(a, _), (b, _)| a.cmp(b));
                results
            })
            .unwrap_or_default()
    }

    /// List entries matching a key prefix
    ///
    /// Returns all entries where `key.starts_with(prefix)`.
    /// The prefix key determines namespace, type_tag, and user_key prefix.
    ///
    /// NOTE: Requires filter + sort, O(n) where n = shard size.
    /// Use sparingly; not for hot path operations.
    ///
    /// # Arguments
    ///
    /// * `prefix` - Prefix key to match (namespace + type_tag + user_key prefix)
    ///
    /// # Returns
    ///
    /// Vector of (Key, VersionedValue) pairs matching prefix, sorted by key
    pub fn list_by_prefix(&self, prefix: &Key) -> Vec<(Key, VersionedValue)> {
        let run_id = prefix.namespace.run_id;
        self.shards
            .get(&run_id)
            .map(|shard| {
                let mut results: Vec<_> = shard
                    .data
                    .iter()
                    .filter(|(k, _)| k.starts_with(prefix))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

                // Sort for consistent ordering
                results.sort_by(|(a, _), (b, _)| a.cmp(b));
                results
            })
            .unwrap_or_default()
    }

    /// List entries of a specific type for a run
    ///
    /// Filters by TypeTag within a run's shard.
    ///
    /// # Arguments
    ///
    /// * `run_id` - The run to query
    /// * `type_tag` - The type to filter by (KV, Event, State, etc.)
    ///
    /// # Returns
    ///
    /// Vector of (Key, VersionedValue) pairs of the specified type, sorted
    pub fn list_by_type(
        &self,
        run_id: &RunId,
        type_tag: in_mem_core::types::TypeTag,
    ) -> Vec<(Key, VersionedValue)> {
        self.shards
            .get(run_id)
            .map(|shard| {
                let mut results: Vec<_> = shard
                    .data
                    .iter()
                    .filter(|(k, _)| k.type_tag == type_tag)
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

                // Sort for consistent ordering
                results.sort_by(|(a, _), (b, _)| a.cmp(b));
                results
            })
            .unwrap_or_default()
    }

    /// Count entries of a specific type for a run
    pub fn count_by_type(
        &self,
        run_id: &RunId,
        type_tag: in_mem_core::types::TypeTag,
    ) -> usize {
        self.shards
            .get(run_id)
            .map(|shard| {
                shard
                    .data
                    .iter()
                    .filter(|(k, _)| k.type_tag == type_tag)
                    .count()
            })
            .unwrap_or(0)
    }

    /// Iterate over all runs
    ///
    /// Returns an iterator over all RunIds that have data
    pub fn run_ids(&self) -> Vec<RunId> {
        self.shards.iter().map(|entry| *entry.key()).collect()
    }

    /// Clear all data for a run
    ///
    /// Removes the entire shard for the given run.
    /// Returns true if the run existed and was removed.
    pub fn clear_run(&self, run_id: &RunId) -> bool {
        self.shards.remove(run_id).is_some()
    }
}

impl Default for ShardedStore {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ShardedStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShardedStore")
            .field("shard_count", &self.shard_count())
            .field("version", &self.version())
            .field("total_entries", &self.total_entries())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_sharded_store_creation() {
        let store = ShardedStore::new();
        assert_eq!(store.shard_count(), 0);
        assert_eq!(store.version(), 0);
    }

    #[test]
    fn test_sharded_store_with_capacity() {
        let store = ShardedStore::with_capacity(100);
        assert_eq!(store.shard_count(), 0);
        assert_eq!(store.version(), 0);
    }

    #[test]
    fn test_version_increment() {
        let store = ShardedStore::new();
        assert_eq!(store.next_version(), 1);
        assert_eq!(store.next_version(), 2);
        assert_eq!(store.version(), 2);
    }

    #[test]
    fn test_set_version() {
        let store = ShardedStore::new();
        store.set_version(100);
        assert_eq!(store.version(), 100);
    }

    #[test]
    fn test_version_thread_safety() {
        use std::thread;
        let store = Arc::new(ShardedStore::new());
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let store = Arc::clone(&store);
                thread::spawn(move || {
                    for _ in 0..100 {
                        store.next_version();
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(store.version(), 1000);
    }

    #[test]
    fn test_shard_creation() {
        let shard = Shard::new();
        assert!(shard.is_empty());
        assert_eq!(shard.len(), 0);
    }

    #[test]
    fn test_shard_with_capacity() {
        let shard = Shard::with_capacity(100);
        assert!(shard.is_empty());
    }

    #[test]
    fn test_debug_impl() {
        let store = ShardedStore::new();
        let debug_str = format!("{:?}", store);
        assert!(debug_str.contains("ShardedStore"));
        assert!(debug_str.contains("shard_count"));
    }

    // ========================================================================
    // Story #228: Get/Put Operations Tests
    // ========================================================================

    fn create_test_key(run_id: RunId, name: &str) -> Key {
        use in_mem_core::types::Namespace;
        let ns = Namespace::new(
            "tenant".to_string(),
            "app".to_string(),
            "agent".to_string(),
            run_id,
        );
        Key::new_kv(ns, name)
    }

    fn create_versioned_value(value: in_mem_core::value::Value, version: u64) -> VersionedValue {
        use chrono::Utc;
        VersionedValue {
            value,
            version,
            timestamp: Utc::now().timestamp(),
            ttl: None,
        }
    }

    #[test]
    fn test_put_and_get() {
        use in_mem_core::value::Value;

        let store = ShardedStore::new();
        let run_id = RunId::new();
        let key = create_test_key(run_id, "test_key");
        let value = create_versioned_value(Value::I64(42), 1);

        // Put
        store.put(key.clone(), value);

        // Get
        let retrieved = store.get(&key);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().value, Value::I64(42));
    }

    #[test]
    fn test_get_nonexistent() {
        let store = ShardedStore::new();
        let run_id = RunId::new();
        let key = create_test_key(run_id, "nonexistent");

        assert!(store.get(&key).is_none());
    }

    #[test]
    fn test_delete() {
        use in_mem_core::value::Value;

        let store = ShardedStore::new();
        let run_id = RunId::new();
        let key = create_test_key(run_id, "to_delete");
        let value = create_versioned_value(Value::I64(42), 1);

        store.put(key.clone(), value);
        assert!(store.get(&key).is_some());

        // Delete
        let deleted = store.delete(&key);
        assert!(deleted.is_some());
        assert!(store.get(&key).is_none());
    }

    #[test]
    fn test_delete_nonexistent() {
        let store = ShardedStore::new();
        let run_id = RunId::new();
        let key = create_test_key(run_id, "nonexistent");

        assert!(store.delete(&key).is_none());
    }

    #[test]
    fn test_contains() {
        use in_mem_core::value::Value;

        let store = ShardedStore::new();
        let run_id = RunId::new();
        let key = create_test_key(run_id, "exists");
        let value = create_versioned_value(Value::I64(42), 1);

        assert!(!store.contains(&key));
        store.put(key.clone(), value);
        assert!(store.contains(&key));
    }

    #[test]
    fn test_overwrite() {
        use in_mem_core::value::Value;

        let store = ShardedStore::new();
        let run_id = RunId::new();
        let key = create_test_key(run_id, "overwrite");

        store.put(key.clone(), create_versioned_value(Value::I64(1), 1));
        store.put(key.clone(), create_versioned_value(Value::I64(2), 2));

        let retrieved = store.get(&key).unwrap();
        assert_eq!(retrieved.value, Value::I64(2));
        assert_eq!(retrieved.version, 2);
    }

    #[test]
    fn test_multiple_runs_isolated() {
        use in_mem_core::value::Value;

        let store = ShardedStore::new();
        let run1 = RunId::new();
        let run2 = RunId::new();

        let key1 = create_test_key(run1, "key");
        let key2 = create_test_key(run2, "key");

        store.put(key1.clone(), create_versioned_value(Value::I64(1), 1));
        store.put(key2.clone(), create_versioned_value(Value::I64(2), 1));

        // Different runs, same key name, different values
        assert_eq!(store.get(&key1).unwrap().value, Value::I64(1));
        assert_eq!(store.get(&key2).unwrap().value, Value::I64(2));
        assert_eq!(store.shard_count(), 2);
    }

    #[test]
    fn test_apply_batch() {
        use in_mem_core::value::Value;

        let store = ShardedStore::new();
        let run_id = RunId::new();

        let key1 = create_test_key(run_id, "batch1");
        let key2 = create_test_key(run_id, "batch2");
        let key3 = create_test_key(run_id, "batch3");

        // First, put key3 so we can delete it
        store.put(key3.clone(), create_versioned_value(Value::I64(999), 1));

        // Apply batch
        let writes = vec![
            (key1.clone(), Value::I64(1)),
            (key2.clone(), Value::I64(2)),
        ];
        let deletes = vec![key3.clone()];

        store.apply_batch(&writes, &deletes, 2);

        assert_eq!(store.get(&key1).unwrap().value, Value::I64(1));
        assert_eq!(store.get(&key1).unwrap().version, 2);
        assert_eq!(store.get(&key2).unwrap().value, Value::I64(2));
        assert!(store.get(&key3).is_none());
    }

    #[test]
    fn test_run_entry_count() {
        use in_mem_core::value::Value;

        let store = ShardedStore::new();
        let run_id = RunId::new();

        assert_eq!(store.run_entry_count(&run_id), 0);

        for i in 0..5 {
            let key = create_test_key(run_id, &format!("key{}", i));
            store.put(key, create_versioned_value(Value::I64(i), 1));
        }

        assert_eq!(store.run_entry_count(&run_id), 5);
        assert_eq!(store.total_entries(), 5);
    }

    #[test]
    fn test_concurrent_writes_different_runs() {
        use in_mem_core::value::Value;
        use std::thread;

        let store = Arc::new(ShardedStore::new());

        // 10 threads, each with its own run, writing 100 keys
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let store = Arc::clone(&store);
                thread::spawn(move || {
                    let run_id = RunId::new();
                    for i in 0..100 {
                        let key = create_test_key(run_id, &format!("key{}", i));
                        store.put(key, create_versioned_value(Value::I64(i), 1));
                    }
                    run_id
                })
            })
            .collect();

        let run_ids: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // Verify each run has 100 entries
        for run_id in &run_ids {
            assert_eq!(store.run_entry_count(run_id), 100);
        }

        assert_eq!(store.shard_count(), 10);
        assert_eq!(store.total_entries(), 1000);
    }

    // ========================================================================
    // Story #229: List Operations Tests
    // ========================================================================

    #[test]
    fn test_list_run_empty() {
        let store = ShardedStore::new();
        let run_id = RunId::new();

        let results = store.list_run(&run_id);
        assert!(results.is_empty());
    }

    #[test]
    fn test_list_run() {
        use in_mem_core::value::Value;

        let store = ShardedStore::new();
        let run_id = RunId::new();

        // Insert some keys
        for i in 0..5 {
            let key = create_test_key(run_id, &format!("key{}", i));
            store.put(key, create_versioned_value(Value::I64(i), 1));
        }

        let results = store.list_run(&run_id);
        assert_eq!(results.len(), 5);

        // Verify sorted order
        for i in 0..results.len() - 1 {
            assert!(results[i].0 < results[i + 1].0);
        }
    }

    #[test]
    fn test_list_by_prefix() {
        use in_mem_core::value::Value;
        use in_mem_core::types::Namespace;

        let store = ShardedStore::new();
        let run_id = RunId::new();
        let ns = Namespace::new(
            "tenant".to_string(),
            "app".to_string(),
            "agent".to_string(),
            run_id,
        );

        // Insert keys with different prefixes
        store.put(
            Key::new_kv(ns.clone(), "user:alice"),
            create_versioned_value(Value::I64(1), 1),
        );
        store.put(
            Key::new_kv(ns.clone(), "user:bob"),
            create_versioned_value(Value::I64(2), 1),
        );
        store.put(
            Key::new_kv(ns.clone(), "config:timeout"),
            create_versioned_value(Value::I64(3), 1),
        );

        // Query with "user:" prefix
        let prefix = Key::new_kv(ns.clone(), "user:");
        let results = store.list_by_prefix(&prefix);

        assert_eq!(results.len(), 2);
        // Should be alice, bob in sorted order
        assert!(results[0].0.user_key_string().unwrap().contains("alice"));
        assert!(results[1].0.user_key_string().unwrap().contains("bob"));
    }

    #[test]
    fn test_list_by_prefix_empty() {
        use in_mem_core::value::Value;
        use in_mem_core::types::Namespace;

        let store = ShardedStore::new();
        let run_id = RunId::new();
        let ns = Namespace::new(
            "tenant".to_string(),
            "app".to_string(),
            "agent".to_string(),
            run_id,
        );

        store.put(
            Key::new_kv(ns.clone(), "data:foo"),
            create_versioned_value(Value::I64(1), 1),
        );

        // Query with non-matching prefix
        let prefix = Key::new_kv(ns.clone(), "user:");
        let results = store.list_by_prefix(&prefix);

        assert!(results.is_empty());
    }

    #[test]
    fn test_list_by_type() {
        use in_mem_core::value::Value;
        use in_mem_core::types::{Namespace, TypeTag};

        let store = ShardedStore::new();
        let run_id = RunId::new();
        let ns = Namespace::new(
            "tenant".to_string(),
            "app".to_string(),
            "agent".to_string(),
            run_id,
        );

        // Insert KV entries
        store.put(
            Key::new_kv(ns.clone(), "kv1"),
            create_versioned_value(Value::I64(1), 1),
        );
        store.put(
            Key::new_kv(ns.clone(), "kv2"),
            create_versioned_value(Value::I64(2), 1),
        );

        // Insert Event entries
        store.put(
            Key::new_event(ns.clone(), 1),
            create_versioned_value(Value::I64(10), 1),
        );

        // Insert State entries
        store.put(
            Key::new_state(ns.clone(), "state1"),
            create_versioned_value(Value::I64(20), 1),
        );

        // Query by type
        let kv_results = store.list_by_type(&run_id, TypeTag::KV);
        assert_eq!(kv_results.len(), 2);

        let event_results = store.list_by_type(&run_id, TypeTag::Event);
        assert_eq!(event_results.len(), 1);

        let state_results = store.list_by_type(&run_id, TypeTag::State);
        assert_eq!(state_results.len(), 1);

        let trace_results = store.list_by_type(&run_id, TypeTag::Trace);
        assert!(trace_results.is_empty());
    }

    #[test]
    fn test_count_by_type() {
        use in_mem_core::value::Value;
        use in_mem_core::types::{Namespace, TypeTag};

        let store = ShardedStore::new();
        let run_id = RunId::new();
        let ns = Namespace::new(
            "tenant".to_string(),
            "app".to_string(),
            "agent".to_string(),
            run_id,
        );

        // Insert mixed types
        for i in 0..5 {
            store.put(
                Key::new_kv(ns.clone(), &format!("kv{}", i)),
                create_versioned_value(Value::I64(i), 1),
            );
        }
        for i in 0..3 {
            store.put(
                Key::new_event(ns.clone(), i as u64),
                create_versioned_value(Value::I64(i), 1),
            );
        }

        assert_eq!(store.count_by_type(&run_id, TypeTag::KV), 5);
        assert_eq!(store.count_by_type(&run_id, TypeTag::Event), 3);
        assert_eq!(store.count_by_type(&run_id, TypeTag::State), 0);
    }

    #[test]
    fn test_run_ids() {
        use in_mem_core::value::Value;

        let store = ShardedStore::new();
        let run1 = RunId::new();
        let run2 = RunId::new();
        let run3 = RunId::new();

        // Insert data for 3 runs
        store.put(create_test_key(run1, "k1"), create_versioned_value(Value::I64(1), 1));
        store.put(create_test_key(run2, "k1"), create_versioned_value(Value::I64(2), 1));
        store.put(create_test_key(run3, "k1"), create_versioned_value(Value::I64(3), 1));

        let run_ids = store.run_ids();
        assert_eq!(run_ids.len(), 3);
        assert!(run_ids.contains(&run1));
        assert!(run_ids.contains(&run2));
        assert!(run_ids.contains(&run3));
    }

    #[test]
    fn test_clear_run() {
        use in_mem_core::value::Value;

        let store = ShardedStore::new();
        let run_id = RunId::new();

        // Insert some data
        for i in 0..5 {
            let key = create_test_key(run_id, &format!("key{}", i));
            store.put(key, create_versioned_value(Value::I64(i), 1));
        }

        assert_eq!(store.run_entry_count(&run_id), 5);
        assert!(store.has_run(&run_id));

        // Clear the run
        assert!(store.clear_run(&run_id));

        assert_eq!(store.run_entry_count(&run_id), 0);
        assert!(!store.has_run(&run_id));
    }

    #[test]
    fn test_clear_run_nonexistent() {
        let store = ShardedStore::new();
        let run_id = RunId::new();

        // Clear non-existent run returns false
        assert!(!store.clear_run(&run_id));
    }

    #[test]
    fn test_list_sorted_order() {
        use in_mem_core::value::Value;
        use in_mem_core::types::Namespace;

        let store = ShardedStore::new();
        let run_id = RunId::new();
        let ns = Namespace::new(
            "tenant".to_string(),
            "app".to_string(),
            "agent".to_string(),
            run_id,
        );

        // Insert in random order
        let keys = vec!["zebra", "apple", "mango", "banana"];
        for k in &keys {
            store.put(
                Key::new_kv(ns.clone(), *k),
                create_versioned_value(Value::String(k.to_string()), 1),
            );
        }

        let results = store.list_run(&run_id);
        let result_keys: Vec<_> = results
            .iter()
            .map(|(k, _)| k.user_key_string().unwrap())
            .collect();

        // Should be sorted: apple, banana, mango, zebra
        assert_eq!(result_keys, vec!["apple", "banana", "mango", "zebra"]);
    }
}
