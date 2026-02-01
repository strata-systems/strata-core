//! Audit test for issue #878: Storage errors silently treated as version 0 during transaction validation
//! Verdict: FIXED (in PR #915, commit 17e7148)
//!
//! The validate_transaction and sub-validators now return StrataResult, propagating
//! storage errors instead of silently treating them as version 0. This prevents
//! CAS with expected_version=0 from bypassing existence checks when I/O errors occur.

use std::sync::Arc;
use std::time::Duration;

use strata_concurrency::TransactionContext;
use strata_core::traits::Storage;
use strata_core::types::{BranchId, Key, Namespace};
use strata_core::value::Value;
use strata_core::{StrataError, StrataResult, VersionedValue};
use strata_storage::ShardedStore;

fn create_namespace(branch_id: BranchId) -> Namespace {
    Namespace::new(
        "tenant".to_string(),
        "app".to_string(),
        "agent".to_string(),
        branch_id,
    )
}

fn create_key(ns: &Namespace, name: &str) -> Key {
    Key::new_kv(ns.clone(), name)
}

/// A mock storage that returns an error for specific keys.
/// This simulates an I/O failure during validation.
struct FailingStore {
    inner: Arc<ShardedStore>,
    fail_keys: Vec<Key>,
}

impl Storage for FailingStore {
    fn get(&self, key: &Key) -> StrataResult<Option<VersionedValue>> {
        if self.fail_keys.contains(key) {
            Err(StrataError::Storage {
                message: "Simulated I/O error".to_string(),
                source: None,
            })
        } else {
            self.inner.get(key)
        }
    }

    fn get_versioned(&self, key: &Key, max_version: u64) -> StrataResult<Option<VersionedValue>> {
        self.inner.get_versioned(key, max_version)
    }

    fn get_history(
        &self,
        key: &Key,
        limit: Option<usize>,
        before_version: Option<u64>,
    ) -> StrataResult<Vec<VersionedValue>> {
        self.inner.get_history(key, limit, before_version)
    }

    fn put(&self, key: Key, value: Value, ttl: Option<Duration>) -> StrataResult<u64> {
        Storage::put(self.inner.as_ref(), key, value, ttl)
    }

    fn delete(&self, key: &Key) -> StrataResult<Option<VersionedValue>> {
        Storage::delete(self.inner.as_ref(), key)
    }

    fn scan_prefix(
        &self,
        prefix: &Key,
        max_version: u64,
    ) -> StrataResult<Vec<(Key, VersionedValue)>> {
        self.inner.scan_prefix(prefix, max_version)
    }

    fn scan_by_branch(
        &self,
        branch_id: BranchId,
        max_version: u64,
    ) -> StrataResult<Vec<(Key, VersionedValue)>> {
        self.inner.scan_by_branch(branch_id, max_version)
    }

    fn current_version(&self) -> u64 {
        self.inner.current_version()
    }

    fn put_with_version(
        &self,
        key: Key,
        value: Value,
        version: u64,
        ttl: Option<Duration>,
    ) -> StrataResult<()> {
        self.inner.put_with_version(key, value, version, ttl)
    }

    fn delete_with_version(
        &self,
        key: &Key,
        version: u64,
    ) -> StrataResult<Option<VersionedValue>> {
        self.inner.delete_with_version(key, version)
    }
}

/// FIXED: CAS with expected_version=0 now fails when storage returns an I/O error,
/// instead of silently treating the error as version 0 and allowing the CAS to pass.
#[test]
fn issue_878_cas_fails_on_storage_error() {
    let branch_id = BranchId::new();
    let ns = create_namespace(branch_id);
    let key = create_key(&ns, "important_key");

    let inner = Arc::new(ShardedStore::new());

    // Pre-populate the key at version 5 (it EXISTS)
    inner
        .put_with_version(key.clone(), Value::Int(42), 5, None)
        .unwrap();

    // Create a failing store that returns errors for this key
    let failing_store = FailingStore {
        inner: Arc::clone(&inner),
        fail_keys: vec![key.clone()],
    };

    // Create a transaction with CAS expected_version=0 (expects key to NOT exist)
    let snapshot = inner.snapshot();
    let mut txn = TransactionContext::with_snapshot(1, branch_id, Box::new(snapshot));
    txn.cas(key.clone(), 0, Value::Int(999)).unwrap();

    // Commit against the failing store
    let result = txn.commit(&failing_store);

    // FIXED: Storage error is now propagated instead of being treated as version 0
    assert!(
        result.is_err(),
        "CAS should fail when storage returns I/O error during validation"
    );
}

/// Read-set validation with storage error produces an error (not a silent conflict).
#[test]
fn issue_878_read_set_error_propagated() {
    let branch_id = BranchId::new();
    let ns = create_namespace(branch_id);
    let key = create_key(&ns, "read_key");

    let inner = Arc::new(ShardedStore::new());

    // Pre-populate the key at version 3
    inner
        .put_with_version(key.clone(), Value::Int(100), 3, None)
        .unwrap();

    // Create a failing store
    let failing_store = FailingStore {
        inner: Arc::clone(&inner),
        fail_keys: vec![key.clone()],
    };

    // Create a transaction that reads the key (recording version 3 in read_set)
    let snapshot = inner.snapshot();
    let mut txn = TransactionContext::with_snapshot(1, branch_id, Box::new(snapshot));

    // Read the key from snapshot (records version 3 in read_set)
    let val = txn.get(&key).unwrap();
    assert!(val.is_some());

    // Write something so txn is not read-only
    let other_key = create_key(&ns, "other");
    txn.put(other_key, Value::Int(1)).unwrap();

    // Commit against failing store — storage error during read-set validation
    let result = txn.commit(&failing_store);

    // Validation fails because the storage error is propagated
    assert!(
        result.is_err(),
        "Read-set validation should fail when storage returns an error"
    );
}

/// FIXED: CAS create-if-not-exists no longer bypasses existence check on I/O error.
#[test]
fn issue_878_cas_create_if_not_exists_blocked_on_error() {
    let branch_id = BranchId::new();
    let ns = create_namespace(branch_id);
    let unique_key = create_key(&ns, "unique_constraint_key");

    let inner = Arc::new(ShardedStore::new());

    // The key exists at version 10
    inner
        .put_with_version(
            unique_key.clone(),
            Value::String("existing".to_string()),
            10,
            None,
        )
        .unwrap();

    // Failing store can't read this key
    let failing_store = FailingStore {
        inner: Arc::clone(&inner),
        fail_keys: vec![unique_key.clone()],
    };

    // Transaction tries to create the key only if it doesn't exist (CAS v0)
    let snapshot = inner.snapshot();
    let mut txn = TransactionContext::with_snapshot(1, branch_id, Box::new(snapshot));
    txn.cas(
        unique_key.clone(),
        0, // Expected: key must not exist
        Value::String("new_value".to_string()),
    )
    .unwrap();

    // FIXED: CAS now fails because I/O error is propagated instead of
    // being silently treated as version 0
    let result = txn.commit(&failing_store);
    assert!(
        result.is_err(),
        "CAS create-if-not-exists should fail when storage error prevents existence check"
    );
}
