//! Transaction manager for coordinating commit operations
//!
//! Provides atomic commit by orchestrating:
//! 1. Validation (first-committer-wins)
//! 2. WAL writing (durability)
//! 3. Storage application (visibility)
//!
//! Per spec Core Invariants:
//! - All-or-nothing commit: transaction writes either ALL succeed or ALL fail
//! - WAL before storage: durability requires WAL to be written first
//! - CommitTxn = durable: transaction is only durable when CommitTxn is in WAL
//!
//! ## Commit Sequence
//!
//! ```text
//! 1. begin_validation() - Change state to Validating
//! 2. validate_transaction() - Check for conflicts
//! 3. IF conflicts: abort() and return error
//! 4. mark_committed() - Change state to Committed
//! 5. Allocate commit_version (increment global version)
//! 6. write_begin() to WAL - BeginTxn entry
//! 7. write_to_wal() - Write/Delete entries with commit_version
//! 8. write_commit() to WAL - CommitTxn entry (DURABILITY POINT)
//! 9. apply_writes() to storage - Apply to in-memory storage
//! 10. Return Ok(commit_version)
//! ```
//!
//! If crash occurs before step 8: Transaction is not durable, discarded on recovery.
//! If crash occurs after step 8: Transaction is durable, replayed on recovery.

use crate::wal_writer::TransactionWALWriter;
use crate::{CommitError, TransactionContext, TransactionStatus};
use parking_lot::Mutex;
use strata_core::error::Result;
use strata_core::traits::Storage;
use strata_durability::wal::WAL;
use std::sync::atomic::{AtomicU64, Ordering};

/// Manages transaction lifecycle and atomic commits
///
/// TransactionManager coordinates the commit protocol:
/// - Validation against current storage state
/// - WAL writing for durability
/// - Storage application for visibility
///
/// Per spec Section 6.1: Global version counter is incremented once per transaction.
/// All keys in a transaction get the same commit version.
///
/// # Thread Safety
///
/// The commit operation is serialized via an internal lock to prevent TOCTOU
/// (time-of-check-to-time-of-use) races between validation and storage application.
/// This ensures that no other transaction can modify storage between the time
/// we validate and the time we apply our writes.
pub struct TransactionManager {
    /// Global version counter
    ///
    /// Monotonically increasing. Each committed transaction increments by 1.
    version: AtomicU64,

    /// Next transaction ID
    ///
    /// Unique identifier for transactions. Used in WAL entries.
    next_txn_id: AtomicU64,

    /// Commit serialization lock
    ///
    /// Prevents TOCTOU race between validation and apply. Without this lock,
    /// the following race can occur:
    /// 1. T1 validates (succeeds, storage at v1)
    /// 2. T2 validates (succeeds, storage still at v1)
    /// 3. T1 applies (storage now at v2)
    /// 4. T2 applies (uses stale validation from step 2)
    ///
    /// The lock ensures validation → WAL → apply is atomic.
    commit_lock: Mutex<()>,
}

impl TransactionManager {
    /// Create a new transaction manager
    ///
    /// # Arguments
    /// * `initial_version` - Starting version (typically from recovery's final_version)
    pub fn new(initial_version: u64) -> Self {
        Self::with_txn_id(initial_version, 0)
    }

    /// Create a new transaction manager with specific starting txn_id
    ///
    /// This is used during recovery to ensure new transactions get unique IDs
    /// that don't conflict with transactions already in the WAL.
    ///
    /// # Arguments
    /// * `initial_version` - Starting version (from recovery's final_version)
    /// * `max_txn_id` - Maximum txn_id seen in WAL (new transactions start at max_txn_id + 1)
    pub fn with_txn_id(initial_version: u64, max_txn_id: u64) -> Self {
        TransactionManager {
            version: AtomicU64::new(initial_version),
            // Start next_txn_id at max_txn_id + 1 to avoid conflicts
            next_txn_id: AtomicU64::new(max_txn_id + 1),
            commit_lock: Mutex::new(()),
        }
    }

    /// Get current global version
    pub fn current_version(&self) -> u64 {
        self.version.load(Ordering::SeqCst)
    }

    /// Allocate next transaction ID
    pub fn next_txn_id(&self) -> u64 {
        self.next_txn_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Allocate next commit version (increment global version)
    ///
    /// Per spec Section 6.1: Version incremented ONCE for the whole transaction.
    ///
    /// # Version Gaps
    ///
    /// Version gaps may occur if a transaction fails after version allocation
    /// but before successful commit (e.g., WAL write failure). Consumers should
    /// not assume version numbers are contiguous. A gap means the version was
    /// allocated but no data was committed with that version.
    ///
    /// This is by design - version allocation is atomic and non-blocking,
    /// while failure handling during commit does not attempt to "return"
    /// the allocated version.
    pub fn allocate_version(&self) -> u64 {
        self.version.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Commit a transaction atomically
    ///
    /// Per spec Core Invariants:
    /// - Validates transaction (first-committer-wins)
    /// - Writes to WAL for durability
    /// - Applies to storage only after WAL is durable
    /// - All-or-nothing: either all writes succeed or transaction aborts
    ///
    /// # Arguments
    /// * `txn` - Transaction to commit (must be in Active state)
    /// * `store` - Storage to validate against and apply writes to
    /// * `wal` - WAL for durability
    ///
    /// # Returns
    /// - Ok(commit_version) on success
    /// - Err(CommitError) if validation fails or WAL write fails
    ///
    /// # Commit Sequence
    ///
    /// 1. Acquire commit lock (prevents TOCTOU race)
    /// 2. Validate and mark committed (in-memory state transition)
    /// 3. Allocate commit version
    /// 4. Write BeginTxn to WAL
    /// 5. Write all operations to WAL
    /// 6. Write CommitTxn to WAL (DURABILITY POINT)
    /// 7. Apply writes to storage
    /// 8. Release commit lock
    /// 9. Return commit version
    ///
    /// # Thread Safety
    ///
    /// The commit lock ensures that validation and apply happen atomically
    /// with respect to other transactions. This prevents the TOCTOU race
    /// where validation passes but storage changes before apply.
    pub fn commit<S: Storage>(
        &self,
        txn: &mut TransactionContext,
        store: &S,
        wal: &mut WAL,
    ) -> std::result::Result<u64, CommitError> {
        // Acquire commit lock to prevent TOCTOU race between validation and apply
        // This ensures no other transaction can modify storage between our
        // validation check and our apply_writes call.
        let _commit_guard = self.commit_lock.lock();

        // Step 1: Validate and mark committed (in-memory)
        // This performs: Active → Validating → Committed
        // Or: Active → Validating → Aborted (if conflicts detected)
        txn.commit(store)?;

        // At this point, transaction is in Committed state
        // but NOT yet durable (not in WAL)

        // Step 2: Allocate commit version
        let commit_version = self.allocate_version();

        // Step 3-5: Write to WAL (durability)
        let txn_id = self.next_txn_id();
        let mut wal_writer = TransactionWALWriter::new(wal, txn_id, txn.run_id);

        // Write BeginTxn
        if let Err(e) = wal_writer.write_begin() {
            // WAL write failed - revert transaction state
            txn.status = TransactionStatus::Aborted {
                reason: format!("WAL write failed: {}", e),
            };
            return Err(CommitError::WALError(e.to_string()));
        }

        // Write all operations
        if let Err(e) = txn.write_to_wal(&mut wal_writer, commit_version) {
            txn.status = TransactionStatus::Aborted {
                reason: format!("WAL write failed: {}", e),
            };
            return Err(CommitError::WALError(e.to_string()));
        }

        // Write CommitTxn - DURABILITY POINT
        if let Err(e) = wal_writer.write_commit() {
            txn.status = TransactionStatus::Aborted {
                reason: format!("WAL commit failed: {}", e),
            };
            return Err(CommitError::WALError(e.to_string()));
        }

        // DURABILITY POINT: Transaction is now durable
        // Even if we crash after this, recovery will replay from WAL

        // Step 6: Apply to storage
        if let Err(e) = txn.apply_writes(store, commit_version) {
            // This is a serious error - WAL says committed but storage failed
            // Log error but return success since WAL is authoritative
            // Recovery will replay the transaction anyway
            tracing::error!(
                txn_id = txn.txn_id,
                commit_version = commit_version,
                error = %e,
                "Storage application failed after WAL commit - will be recovered on restart"
            );
        }

        // Step 7: Return commit version
        Ok(commit_version)
    }

    /// Explicitly abort a transaction
    ///
    /// Per spec Appendix A.3:
    /// - No AbortTxn entry written to WAL in M2
    /// - All buffered operations discarded
    /// - Transaction marked as Aborted
    ///
    /// # Arguments
    /// * `txn` - Transaction to abort
    /// * `reason` - Human-readable reason for abort
    pub fn abort(&self, txn: &mut TransactionContext, reason: String) -> Result<()> {
        txn.mark_aborted(reason)
    }

    /// Commit with automatic rollback on failure
    ///
    /// Ensures transaction is properly cleaned up if commit fails.
    /// This is a convenience method that handles the common pattern
    /// of wanting to abort on any error.
    pub fn commit_or_rollback<S: Storage>(
        &self,
        txn: &mut TransactionContext,
        store: &S,
        wal: &mut WAL,
    ) -> std::result::Result<u64, CommitError> {
        match self.commit(txn, store, wal) {
            Ok(version) => Ok(version),
            Err(e) => {
                // Ensure transaction is in Aborted state
                if txn.can_rollback() {
                    let _ = txn.mark_aborted(format!("Commit failed: {}", e));
                }
                Err(e)
            }
        }
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new(0)
    }
}

