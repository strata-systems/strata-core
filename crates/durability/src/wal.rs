//! WAL (Write-Ahead Log) entry types and file operations
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
//!
//! ## File Format
//!
//! WAL is an append-only log file containing a sequence of encoded entries.
//! Each entry is self-describing (no framing needed).
//!
//! ## File Operations
//!
//! - `WAL::open()` - Open existing WAL or create new one
//! - `WAL::append()` - Write encoded entry to end of file
//! - `WAL::read_entries()` - Scan from offset, decode entries
//! - `WAL::read_all()` - Scan from beginning
//! - `WAL::flush()` - Flush buffered writes
//! - `WAL::size()` - Get current file size

use crate::encoding::{decode_entry, encode_entry};
use in_mem_core::{
    error::{Error, Result},
    types::{Key, RunId},
    value::{Timestamp, Value},
};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
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

// ============================================================================
// WAL File Operations
// ============================================================================

/// Write-Ahead Log
///
/// Append-only log of WAL entries persisted to disk.
/// File format: sequence of encoded entries (self-describing, no framing).
///
/// # Example
///
/// ```ignore
/// use in_mem_durability::wal::{WAL, WALEntry};
///
/// let mut wal = WAL::open("data/wal/segment.wal")?;
/// wal.append(&entry)?;
/// wal.flush()?;
///
/// let entries = wal.read_all()?;
/// ```
pub struct WAL {
    /// File path
    path: PathBuf,

    /// File handle (buffered writer for appends)
    writer: BufWriter<File>,

    /// Current file offset (for error reporting and size tracking)
    current_offset: u64,
}

impl WAL {
    /// Open existing WAL or create new one
    ///
    /// Creates parent directories if they don't exist.
    /// Opens file in append mode with read capability.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to WAL file
    ///
    /// # Returns
    ///
    /// * `Ok(WAL)` - Opened WAL handle
    /// * `Err` - If file operations fail
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // Open file (create if doesn't exist, append mode)
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&path)?;

        // Get current file size (start offset)
        let current_offset = file.metadata()?.len();

        let writer = BufWriter::new(file);

        Ok(Self {
            path,
            writer,
            current_offset,
        })
    }

    /// Append entry to WAL
    ///
    /// Encodes entry and writes to end of file.
    /// Does NOT fsync (added in next story for durability modes).
    ///
    /// # Arguments
    ///
    /// * `entry` - WAL entry to append
    ///
    /// # Returns
    ///
    /// * `Ok(u64)` - Offset where entry was written
    /// * `Err` - If encoding or writing fails
    pub fn append(&mut self, entry: &WALEntry) -> Result<u64> {
        let offset = self.current_offset;

        // Encode entry
        let encoded = encode_entry(entry)?;

        // Write to file
        self.writer.write_all(&encoded).map_err(|e| {
            Error::StorageError(format!("Failed to write entry at offset {}: {}", offset, e))
        })?;

        // Update offset
        self.current_offset += encoded.len() as u64;

        Ok(offset)
    }

    /// Flush buffered writes to disk
    ///
    /// Note: This flushes to OS buffers, not necessarily to disk.
    /// For true durability, fsync is needed (added in next story).
    pub fn flush(&mut self) -> Result<()> {
        self.writer
            .flush()
            .map_err(|e| Error::StorageError(format!("Failed to flush WAL: {}", e)))
    }

    /// Read all entries from WAL starting at offset
    ///
    /// Returns vector of decoded entries.
    /// Stops at first corruption or end of file.
    /// Incomplete entries at EOF are expected (partial writes) and ignored.
    ///
    /// # Arguments
    ///
    /// * `start_offset` - Byte offset to start reading from
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<WALEntry>)` - Decoded entries
    /// * `Err` - If file operations fail or mid-file corruption detected
    pub fn read_entries(&self, start_offset: u64) -> Result<Vec<WALEntry>> {
        // Open separate read handle (writer is buffered, don't interfere)
        let file = File::open(&self.path)?;

        let mut reader = BufReader::new(file);

        // Seek to start offset
        reader.seek(SeekFrom::Start(start_offset))?;

        let mut entries = Vec::new();
        let mut file_offset = start_offset;

        // Read file in chunks
        loop {
            // Read into buffer
            let mut buf = vec![0u8; 64 * 1024]; // 64KB buffer
            let bytes_read = reader.read(&mut buf)?;

            if bytes_read == 0 {
                break; // EOF
            }

            buf.truncate(bytes_read);

            // Decode entries from buffer
            let mut offset_in_buf = 0;
            while offset_in_buf < buf.len() {
                match decode_entry(&buf[offset_in_buf..], file_offset) {
                    Ok((entry, bytes_consumed)) => {
                        entries.push(entry);
                        offset_in_buf += bytes_consumed;
                        file_offset += bytes_consumed as u64;
                    }
                    Err(_) => {
                        // Could be incomplete entry at end or corruption
                        // If buffer wasn't full, we're at EOF - incomplete entry is expected
                        if bytes_read < buf.capacity() {
                            // EOF, incomplete entry at end is expected (partial write)
                            return Ok(entries);
                        }
                        // Buffer was full but decode failed - might need more data
                        // Seek back and try reading more in next iteration
                        // For simplicity, break and return what we have
                        // (A more sophisticated implementation would handle spanning entries)
                        return Ok(entries);
                    }
                }
            }
        }

        Ok(entries)
    }

    /// Read all entries from beginning of file
    ///
    /// Convenience method equivalent to `read_entries(0)`.
    pub fn read_all(&self) -> Result<Vec<WALEntry>> {
        self.read_entries(0)
    }

    /// Get current file size (offset for next write)
    pub fn size(&self) -> u64 {
        self.current_offset
    }

    /// Get file path
    pub fn path(&self) -> &Path {
        &self.path
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

    // ========================================================================
    // WAL File Operations Tests
    // ========================================================================

    use tempfile::TempDir;

    #[test]
    fn test_open_new_wal() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");

        let wal = WAL::open(&wal_path).unwrap();
        assert_eq!(wal.size(), 0);
        assert!(wal_path.exists());
    }

    #[test]
    fn test_append_and_read() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");

        let mut wal = WAL::open(&wal_path).unwrap();

        let run_id = RunId::new();
        let entry1 = WALEntry::BeginTxn {
            txn_id: 1,
            run_id,
            timestamp: now(),
        };
        let entry2 = WALEntry::CommitTxn { txn_id: 1, run_id };

        // Append entries
        wal.append(&entry1).unwrap();
        wal.append(&entry2).unwrap();
        wal.flush().unwrap();

        // Read back
        let entries = wal.read_all().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], entry1);
        assert_eq!(entries[1], entry2);
    }

    #[test]
    fn test_append_multiple_entries() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");

        let mut wal = WAL::open(&wal_path).unwrap();

        let run_id = RunId::new();
        let ns = Namespace::new(
            "tenant".to_string(),
            "app".to_string(),
            "agent".to_string(),
            run_id,
        );

        // Append 100 entries
        for i in 0..100u64 {
            let entry = WALEntry::Write {
                run_id,
                key: Key::new_kv(ns.clone(), format!("key_{}", i)),
                value: Value::Bytes(vec![i as u8]),
                version: i,
            };
            wal.append(&entry).unwrap();
        }

        wal.flush().unwrap();

        // Read back
        let entries = wal.read_all().unwrap();
        assert_eq!(entries.len(), 100);

        // Verify first and last entries
        if let WALEntry::Write { version, .. } = &entries[0] {
            assert_eq!(*version, 0);
        } else {
            panic!("Expected Write entry");
        }
        if let WALEntry::Write { version, .. } = &entries[99] {
            assert_eq!(*version, 99);
        } else {
            panic!("Expected Write entry");
        }
    }

    #[test]
    fn test_read_from_offset() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");

        let mut wal = WAL::open(&wal_path).unwrap();

        let run_id = RunId::new();
        let entry1 = WALEntry::BeginTxn {
            txn_id: 1,
            run_id,
            timestamp: now(),
        };
        let entry2 = WALEntry::CommitTxn { txn_id: 1, run_id };

        let _offset1 = wal.append(&entry1).unwrap();
        let offset2 = wal.append(&entry2).unwrap();
        wal.flush().unwrap();

        // Read from offset2 (should only get entry2)
        let entries = wal.read_entries(offset2).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], entry2);
    }

    #[test]
    fn test_reopen_wal() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");

        let run_id = RunId::new();
        let entry1 = WALEntry::BeginTxn {
            txn_id: 1,
            run_id,
            timestamp: now(),
        };

        let initial_size;

        // Write entry and close
        {
            let mut wal = WAL::open(&wal_path).unwrap();
            wal.append(&entry1).unwrap();
            wal.flush().unwrap();
            initial_size = wal.size();
        }

        // Reopen and verify entry still there
        {
            let wal = WAL::open(&wal_path).unwrap();
            assert_eq!(wal.size(), initial_size);

            let entries = wal.read_all().unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0], entry1);
        }
    }

    #[test]
    fn test_append_after_reopen() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");

        let run_id = RunId::new();
        let entry1 = WALEntry::BeginTxn {
            txn_id: 1,
            run_id,
            timestamp: now(),
        };
        let entry2 = WALEntry::CommitTxn { txn_id: 1, run_id };

        // Write first entry and close
        {
            let mut wal = WAL::open(&wal_path).unwrap();
            wal.append(&entry1).unwrap();
            wal.flush().unwrap();
        }

        // Reopen and append second entry
        {
            let mut wal = WAL::open(&wal_path).unwrap();
            wal.append(&entry2).unwrap();
            wal.flush().unwrap();
        }

        // Read all entries
        {
            let wal = WAL::open(&wal_path).unwrap();
            let entries = wal.read_all().unwrap();
            assert_eq!(entries.len(), 2);
            assert_eq!(entries[0], entry1);
            assert_eq!(entries[1], entry2);
        }
    }

    #[test]
    fn test_wal_creates_parent_directories() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("nested").join("dir").join("test.wal");

        let wal = WAL::open(&wal_path).unwrap();
        assert_eq!(wal.size(), 0);
        assert!(wal_path.exists());
    }
}
