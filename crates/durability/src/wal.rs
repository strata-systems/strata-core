//! WAL (Write-Ahead Log) entry types
//!
//! This module defines all WAL entry types for the durability layer:
//! - BeginTxn: Start of a transaction
//! - Write: Put or update operation
//! - Delete: Delete operation
//! - CommitTxn: Successful transaction completion
//! - AbortTxn: Transaction rollback
//! - Checkpoint: Snapshot boundary marker
//!
//! CRITICAL: All entries include run_id (except Checkpoint which tracks active runs)
//! This enables:
//! - Run-scoped replay (filter WAL by run_id)
//! - Run diffing (compare WAL entries for two runs)
//! - Audit trails (track all operations per run)

use in_mem_core::{
    types::{Key, RunId},
    value::{Timestamp, Value},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// WAL entry types
///
/// Each entry represents a state-changing operation that must be persisted
/// before it can be considered durable. All entries (except Checkpoint)
/// include run_id to enable run-scoped operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WALEntry {
    /// Begin transaction
    ///
    /// Marks the start of a transaction. All writes/deletes between
    /// BeginTxn and CommitTxn/AbortTxn belong to this transaction.
    BeginTxn {
        /// Transaction identifier (unique within a run)
        txn_id: u64,
        /// Run this transaction belongs to
        run_id: RunId,
        /// Timestamp when transaction started
        timestamp: Timestamp,
    },

    /// Write operation (put or update)
    ///
    /// Records a key-value write operation with its version.
    Write {
        /// Run this write belongs to
        run_id: RunId,
        /// Key being written
        key: Key,
        /// Value being written
        value: Value,
        /// Version number for this write
        version: u64,
    },

    /// Delete operation
    ///
    /// Records a key deletion with its version.
    Delete {
        /// Run this delete belongs to
        run_id: RunId,
        /// Key being deleted
        key: Key,
        /// Version number for this delete
        version: u64,
    },

    /// Commit transaction
    ///
    /// Marks successful completion of a transaction.
    /// All operations in this transaction are now durable.
    CommitTxn {
        /// Transaction identifier
        txn_id: u64,
        /// Run this transaction belongs to
        run_id: RunId,
    },

    /// Abort transaction
    ///
    /// Marks that a transaction was rolled back.
    /// All operations in this transaction should be discarded.
    AbortTxn {
        /// Transaction identifier
        txn_id: u64,
        /// Run this transaction belongs to
        run_id: RunId,
    },

    /// Checkpoint marker (snapshot boundary)
    ///
    /// Marks a point where a consistent snapshot was taken.
    /// WAL entries before this checkpoint can be truncated after
    /// the snapshot is safely persisted.
    Checkpoint {
        /// Unique identifier for this snapshot
        snapshot_id: Uuid,
        /// Version at checkpoint time
        version: u64,
        /// Runs that were active at checkpoint time
        active_runs: Vec<RunId>,
    },
}

impl WALEntry {
    /// Get run_id from entry (if applicable)
    ///
    /// Returns the run_id for all entry types except Checkpoint,
    /// which tracks multiple runs instead of belonging to a single run.
    pub fn run_id(&self) -> Option<RunId> {
        match self {
            WALEntry::BeginTxn { run_id, .. } => Some(*run_id),
            WALEntry::Write { run_id, .. } => Some(*run_id),
            WALEntry::Delete { run_id, .. } => Some(*run_id),
            WALEntry::CommitTxn { run_id, .. } => Some(*run_id),
            WALEntry::AbortTxn { run_id, .. } => Some(*run_id),
            WALEntry::Checkpoint { .. } => None, // Checkpoint tracks multiple runs
        }
    }

    /// Get transaction ID (if applicable)
    ///
    /// Returns the transaction ID for transaction-related entries:
    /// BeginTxn, CommitTxn, AbortTxn.
    pub fn txn_id(&self) -> Option<u64> {
        match self {
            WALEntry::BeginTxn { txn_id, .. } => Some(*txn_id),
            WALEntry::CommitTxn { txn_id, .. } => Some(*txn_id),
            WALEntry::AbortTxn { txn_id, .. } => Some(*txn_id),
            _ => None,
        }
    }

    /// Get version (if applicable)
    ///
    /// Returns the version for entries that track versions:
    /// Write, Delete, Checkpoint.
    pub fn version(&self) -> Option<u64> {
        match self {
            WALEntry::Write { version, .. } => Some(*version),
            WALEntry::Delete { version, .. } => Some(*version),
            WALEntry::Checkpoint { version, .. } => Some(*version),
            _ => None,
        }
    }

    /// Check if entry is a transaction boundary
    ///
    /// Transaction boundaries are BeginTxn, CommitTxn, and AbortTxn.
    /// These mark the start and end of transactions.
    pub fn is_txn_boundary(&self) -> bool {
        matches!(
            self,
            WALEntry::BeginTxn { .. } | WALEntry::CommitTxn { .. } | WALEntry::AbortTxn { .. }
        )
    }

    /// Check if entry is a checkpoint
    ///
    /// Checkpoints mark snapshot boundaries for WAL truncation.
    pub fn is_checkpoint(&self) -> bool {
        matches!(self, WALEntry::Checkpoint { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use in_mem_core::types::Namespace;

    /// Helper to get current timestamp
    fn now() -> Timestamp {
        Utc::now().timestamp()
    }

    #[test]
    fn test_begin_txn_entry() {
        let run_id = RunId::new();
        let entry = WALEntry::BeginTxn {
            txn_id: 42,
            run_id,
            timestamp: now(),
        };

        assert_eq!(entry.run_id(), Some(run_id));
        assert_eq!(entry.txn_id(), Some(42));
        assert!(entry.is_txn_boundary());
        assert!(!entry.is_checkpoint());
        assert_eq!(entry.version(), None);
    }

    #[test]
    fn test_write_entry() {
        let run_id = RunId::new();
        let ns = Namespace::new(
            "tenant".to_string(),
            "app".to_string(),
            "agent".to_string(),
            run_id,
        );
        let key = Key::new_kv(ns, "test");
        let value = Value::Bytes(b"data".to_vec());

        let entry = WALEntry::Write {
            run_id,
            key: key.clone(),
            value: value.clone(),
            version: 100,
        };

        assert_eq!(entry.run_id(), Some(run_id));
        assert_eq!(entry.version(), Some(100));
        assert!(!entry.is_txn_boundary());
        assert!(!entry.is_checkpoint());
        assert_eq!(entry.txn_id(), None);

        if let WALEntry::Write {
            key: k, value: v, ..
        } = entry
        {
            assert_eq!(k, key);
            assert_eq!(v, value);
        } else {
            panic!("Expected Write variant");
        }
    }

    #[test]
    fn test_delete_entry() {
        let run_id = RunId::new();
        let ns = Namespace::new(
            "tenant".to_string(),
            "app".to_string(),
            "agent".to_string(),
            run_id,
        );
        let key = Key::new_kv(ns, "test");

        let entry = WALEntry::Delete {
            run_id,
            key: key.clone(),
            version: 101,
        };

        assert_eq!(entry.run_id(), Some(run_id));
        assert_eq!(entry.version(), Some(101));
        assert!(!entry.is_txn_boundary());
        assert!(!entry.is_checkpoint());
        assert_eq!(entry.txn_id(), None);

        if let WALEntry::Delete { key: k, .. } = entry {
            assert_eq!(k, key);
        } else {
            panic!("Expected Delete variant");
        }
    }

    #[test]
    fn test_commit_txn_entry() {
        let run_id = RunId::new();
        let entry = WALEntry::CommitTxn { txn_id: 42, run_id };

        assert_eq!(entry.run_id(), Some(run_id));
        assert_eq!(entry.txn_id(), Some(42));
        assert!(entry.is_txn_boundary());
        assert!(!entry.is_checkpoint());
        assert_eq!(entry.version(), None);
    }

    #[test]
    fn test_abort_txn_entry() {
        let run_id = RunId::new();
        let entry = WALEntry::AbortTxn { txn_id: 99, run_id };

        assert_eq!(entry.run_id(), Some(run_id));
        assert_eq!(entry.txn_id(), Some(99));
        assert!(entry.is_txn_boundary());
        assert!(!entry.is_checkpoint());
        assert_eq!(entry.version(), None);
    }

    #[test]
    fn test_checkpoint_entry() {
        let run1 = RunId::new();
        let run2 = RunId::new();

        let entry = WALEntry::Checkpoint {
            snapshot_id: Uuid::new_v4(),
            version: 1000,
            active_runs: vec![run1, run2],
        };

        assert!(entry.is_checkpoint());
        assert_eq!(entry.version(), Some(1000));
        assert_eq!(entry.run_id(), None); // Checkpoint doesn't have single run_id
        assert!(!entry.is_txn_boundary());
        assert_eq!(entry.txn_id(), None);

        if let WALEntry::Checkpoint { active_runs, .. } = entry {
            assert_eq!(active_runs.len(), 2);
            assert!(active_runs.contains(&run1));
            assert!(active_runs.contains(&run2));
        } else {
            panic!("Expected Checkpoint variant");
        }
    }

    #[test]
    fn test_serialization_roundtrip() {
        let run_id = RunId::new();
        let timestamp = now();
        let entry = WALEntry::BeginTxn {
            txn_id: 42,
            run_id,
            timestamp,
        };

        // Serialize with bincode
        let encoded = bincode::serialize(&entry).expect("serialization failed");

        // Deserialize
        let decoded: WALEntry = bincode::deserialize(&encoded).expect("deserialization failed");

        assert_eq!(entry, decoded);
    }

    #[test]
    fn test_all_entries_serialize() {
        let run_id = RunId::new();
        let ns = Namespace::new(
            "tenant".to_string(),
            "app".to_string(),
            "agent".to_string(),
            run_id,
        );

        let entries = vec![
            WALEntry::BeginTxn {
                txn_id: 1,
                run_id,
                timestamp: now(),
            },
            WALEntry::Write {
                run_id,
                key: Key::new_kv(ns.clone(), "key"),
                value: Value::Bytes(b"value".to_vec()),
                version: 10,
            },
            WALEntry::Delete {
                run_id,
                key: Key::new_kv(ns, "key"),
                version: 11,
            },
            WALEntry::CommitTxn { txn_id: 1, run_id },
            WALEntry::AbortTxn { txn_id: 2, run_id },
            WALEntry::Checkpoint {
                snapshot_id: Uuid::new_v4(),
                version: 100,
                active_runs: vec![run_id],
            },
        ];

        for entry in entries {
            let encoded = bincode::serialize(&entry).expect("serialization failed");
            let decoded: WALEntry = bincode::deserialize(&encoded).expect("deserialization failed");
            assert_eq!(entry, decoded);
        }
    }
}
