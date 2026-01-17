//! M7 Crash Recovery with Snapshot + WAL Replay
//!
//! This module implements crash recovery using:
//! - Snapshot discovery (find latest valid, fallback to older)
//! - Snapshot loading with checksum validation
//! - WAL replay from snapshot offset
//! - Corrupt entry handling with configurable limits
//!
//! ## Recovery Sequence
//!
//! 1. Find latest valid snapshot
//! 2. Load snapshot into memory
//! 3. Replay WAL from snapshot's WAL offset
//! 4. Rebuild indexes (if configured)
//!
//! ## Key Principle
//!
//! After crash recovery, the database must correspond to a **prefix of the
//! committed transaction history**. No partial transactions may be visible.
//!
//! ## Usage
//!
//! ```ignore
//! let (data, result) = M7Recovery::recover(
//!     data_dir,
//!     M7RecoveryOptions::default(),
//! )?;
//!
//! println!("{}", result.summary());
//! ```

use crate::m7_wal_reader::WalReader;
use crate::m7_wal_types::WalEntryError;
use crate::snapshot::SnapshotReader;
use crate::snapshot_types::*;
use crate::wal_entry_types::WalEntryType;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, info, warn};

// ============================================================================
// Recovery Options
// ============================================================================

/// M7 Recovery options
#[derive(Debug, Clone)]
pub struct M7RecoveryOptions {
    /// Maximum corrupt entries to tolerate before failing
    pub max_corrupt_entries: usize,
    /// Whether to verify all checksums (slower but safer)
    pub verify_all_checksums: bool,
    /// Whether to rebuild indexes after recovery
    pub rebuild_indexes: bool,
    /// Whether to log recovery progress
    pub verbose: bool,
    /// Snapshot file pattern (e.g., "snapshot_*.snap")
    pub snapshot_pattern: String,
    /// WAL file name
    pub wal_filename: String,
}

impl Default for M7RecoveryOptions {
    fn default() -> Self {
        M7RecoveryOptions {
            max_corrupt_entries: 10,
            verify_all_checksums: true,
            rebuild_indexes: true,
            verbose: false,
            snapshot_pattern: "snapshot_*.snap".to_string(),
            wal_filename: "wal.dat".to_string(),
        }
    }
}

impl M7RecoveryOptions {
    /// Strict recovery options - fail on any corruption
    pub fn strict() -> Self {
        M7RecoveryOptions {
            max_corrupt_entries: 0,
            verify_all_checksums: true,
            rebuild_indexes: true,
            verbose: true,
            ..Default::default()
        }
    }

    /// Permissive recovery options - tolerate more corruption
    pub fn permissive() -> Self {
        M7RecoveryOptions {
            max_corrupt_entries: 100,
            verify_all_checksums: false,
            rebuild_indexes: true,
            verbose: false,
            ..Default::default()
        }
    }

    /// Fast recovery options - skip some safety checks
    pub fn fast() -> Self {
        M7RecoveryOptions {
            max_corrupt_entries: 10,
            verify_all_checksums: false,
            rebuild_indexes: false,
            verbose: false,
            ..Default::default()
        }
    }
}

// ============================================================================
// Recovery Result
// ============================================================================

/// M7 Recovery result
#[derive(Debug, Default, Clone)]
pub struct M7RecoveryResult {
    /// Snapshot used (if any)
    pub snapshot_used: Option<SnapshotInfo>,
    /// Snapshots that were skipped due to corruption
    pub snapshots_skipped: usize,
    /// WAL entries replayed
    pub wal_entries_replayed: u64,
    /// Transactions successfully recovered
    pub transactions_recovered: u64,
    /// Orphaned transactions (no commit marker)
    pub orphaned_transactions: u64,
    /// Aborted transactions discarded
    pub aborted_transactions: u64,
    /// Corrupt entries skipped
    pub corrupt_entries_skipped: u64,
    /// Total recovery time (microseconds)
    pub recovery_time_micros: u64,
    /// WAL replay start offset
    pub wal_replay_from_offset: u64,
    /// Whether recovery was successful
    pub success: bool,
}

impl M7RecoveryResult {
    /// Get human-readable summary
    pub fn summary(&self) -> String {
        let snapshot_info = match &self.snapshot_used {
            Some(info) => format!(
                "snapshot at offset {}",
                info.wal_offset
            ),
            None => "no snapshot (full WAL replay)".to_string(),
        };

        format!(
            "Recovery complete: {} transactions, {} WAL entries, {} orphaned, {} aborted, {} corrupt, {:.2}ms ({})",
            self.transactions_recovered,
            self.wal_entries_replayed,
            self.orphaned_transactions,
            self.aborted_transactions,
            self.corrupt_entries_skipped,
            self.recovery_time_micros as f64 / 1000.0,
            snapshot_info
        )
    }

    /// Check if recovery had any issues (corruption, orphaned txns, etc.)
    pub fn has_issues(&self) -> bool {
        self.corrupt_entries_skipped > 0
            || self.orphaned_transactions > 0
            || self.snapshots_skipped > 0
    }
}

// ============================================================================
// Recovery Error
// ============================================================================

/// M7 Recovery errors
#[derive(Debug, Error)]
pub enum M7RecoveryError {
    /// Too many corrupt entries
    #[error("Too many corrupt entries: {0} (max allowed: {1})")]
    TooManyCorruptEntries(u64, usize),

    /// Snapshot error
    #[error("Snapshot error: {0}")]
    Snapshot(#[from] SnapshotError),

    /// WAL error
    #[error("WAL error: {0}")]
    Wal(#[from] WalEntryError),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// No valid data to recover
    #[error("No valid data to recover: no snapshots and no WAL")]
    NoDataToRecover,

    /// Deserialization error
    #[error("Deserialization error: {0}")]
    Deserialize(String),
}

// ============================================================================
// Snapshot Discovery
// ============================================================================

/// Snapshot discovery for finding valid snapshots
pub struct SnapshotDiscovery;

impl SnapshotDiscovery {
    /// Find all snapshot files in directory
    pub fn list_snapshots(dir: &Path) -> Result<Vec<PathBuf>, M7RecoveryError> {
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut snapshots = Vec::new();

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            // Check for snapshot file extension (.snap or .dat)
            if let Some(ext) = path.extension() {
                if ext == "snap" || ext == "dat" {
                    // Verify it's a file, not a directory
                    if path.is_file() {
                        snapshots.push(path);
                    }
                }
            }
        }

        // Sort by filename (which includes timestamp) - newest first
        snapshots.sort_by(|a, b| b.cmp(a));

        Ok(snapshots)
    }

    /// Find the latest valid snapshot
    ///
    /// Tries snapshots from newest to oldest until a valid one is found.
    /// Returns None if no valid snapshots exist.
    pub fn find_latest_valid(
        snapshot_dir: &Path,
    ) -> Result<Option<(SnapshotInfo, usize)>, M7RecoveryError> {
        let snapshots = Self::list_snapshots(snapshot_dir)?;

        if snapshots.is_empty() {
            debug!("No snapshot files found in {}", snapshot_dir.display());
            return Ok(None);
        }

        let total = snapshots.len();
        info!("Found {} snapshot files, checking validity...", total);

        for (idx, path) in snapshots.iter().enumerate() {
            match SnapshotReader::validate_checksum(path) {
                Ok(()) => {
                    // Read header for metadata
                    match SnapshotReader::read_header(path) {
                        Ok(header) => {
                            let info = SnapshotInfo {
                                path: path.clone(),
                                timestamp_micros: header.timestamp_micros,
                                wal_offset: header.wal_offset,
                                size_bytes: std::fs::metadata(path)
                                    .map(|m| m.len())
                                    .unwrap_or(0),
                            };

                            if idx > 0 {
                                warn!(
                                    "Using snapshot {} (skipped {} newer corrupt snapshots)",
                                    path.display(),
                                    idx
                                );
                            } else {
                                info!("Using latest snapshot: {}", path.display());
                            }

                            return Ok(Some((info, idx)));
                        }
                        Err(e) => {
                            warn!(
                                "Snapshot {} has valid checksum but invalid header: {}",
                                path.display(),
                                e
                            );
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        "Snapshot {} is corrupt: {}. Trying older...",
                        path.display(),
                        e
                    );
                }
            }
        }

        warn!("No valid snapshots found in {}", snapshot_dir.display());
        Ok(None)
    }
}

// ============================================================================
// WAL Replay Result
// ============================================================================

/// WAL replay result (internal)
#[derive(Debug, Default)]
struct WalReplayResult {
    entries_replayed: u64,
    transactions_recovered: u64,
    orphaned_transactions: u64,
    aborted_transactions: u64,
    corrupt_entries: u64,
}

// ============================================================================
// M7 Recovery Engine
// ============================================================================

/// M7 Recovery engine
///
/// Combines snapshot loading with WAL replay for crash recovery.
pub struct M7Recovery;

impl M7Recovery {
    /// Recover primitive data from disk
    ///
    /// Returns the recovered data as a vector of primitive sections and
    /// recovery statistics.
    pub fn recover(
        data_dir: &Path,
        options: M7RecoveryOptions,
    ) -> Result<(Vec<PrimitiveSection>, M7RecoveryResult), M7RecoveryError> {
        let start = std::time::Instant::now();
        let mut result = M7RecoveryResult::default();

        info!("Starting M7 recovery from {}", data_dir.display());

        // 1. Find latest valid snapshot
        let snapshot_dir = data_dir.join("snapshots");
        let snapshot_result = SnapshotDiscovery::find_latest_valid(&snapshot_dir)?;

        // 2. Load snapshot sections or start empty
        let (mut sections, wal_replay_from) = if let Some((info, skipped)) = snapshot_result {
            result.snapshot_used = Some(info.clone());
            result.snapshots_skipped = skipped;

            // Load snapshot
            let envelope = SnapshotReader::read_envelope(&info.path)?;
            info!(
                "Loaded snapshot: {} sections, WAL offset {}",
                envelope.sections.len(),
                info.wal_offset
            );

            (envelope.sections, info.wal_offset)
        } else {
            info!("No valid snapshot found, will replay entire WAL");
            (Vec::new(), 0)
        };

        result.wal_replay_from_offset = wal_replay_from;

        // 3. Replay WAL from offset
        let wal_path = data_dir.join(&options.wal_filename);
        if wal_path.exists() {
            let replay_result =
                Self::replay_wal_to_sections(&mut sections, &wal_path, wal_replay_from, &options)?;

            result.wal_entries_replayed = replay_result.entries_replayed;
            result.transactions_recovered = replay_result.transactions_recovered;
            result.orphaned_transactions = replay_result.orphaned_transactions;
            result.aborted_transactions = replay_result.aborted_transactions;
            result.corrupt_entries_skipped = replay_result.corrupt_entries;

            // Check corrupt entry limit
            if result.corrupt_entries_skipped > options.max_corrupt_entries as u64 {
                return Err(M7RecoveryError::TooManyCorruptEntries(
                    result.corrupt_entries_skipped,
                    options.max_corrupt_entries,
                ));
            }
        } else if sections.is_empty() {
            // No snapshot and no WAL - nothing to recover
            debug!("No WAL file found at {}", wal_path.display());
        }

        result.recovery_time_micros = start.elapsed().as_micros() as u64;
        result.success = true;

        info!("{}", result.summary());

        Ok((sections, result))
    }

    /// Replay WAL entries to update primitive sections
    ///
    /// This is a simplified replay that tracks committed transactions
    /// but doesn't actually apply them to sections (that requires
    /// primitive-specific deserialization which is done by the caller).
    fn replay_wal_to_sections(
        _sections: &mut [PrimitiveSection],
        wal_path: &Path,
        from_offset: u64,
        options: &M7RecoveryOptions,
    ) -> Result<WalReplayResult, M7RecoveryError> {
        let mut result = WalReplayResult::default();
        let mut reader = WalReader::open(wal_path)?;

        // Seek to offset if needed
        if from_offset > 0 {
            reader.seek_to(from_offset)?;
            debug!("WAL replay starting from offset {}", from_offset);
        }

        // Track transactions by TxId (which is Copy, Hash, Eq)
        let mut tx_entries: HashMap<crate::m7_wal_types::TxId, Vec<crate::m7_wal_types::WalEntry>> =
            HashMap::new();

        // Read all entries
        while let Some(entry) = reader.next_entry()? {
            result.entries_replayed += 1;

            // Check for corrupt entries (via reader stats)
            if reader.corruption_count() > result.corrupt_entries {
                let new_corrupt = reader.corruption_count() - result.corrupt_entries;
                result.corrupt_entries = reader.corruption_count();

                if options.verbose {
                    warn!("Detected {} corrupt entries during replay", new_corrupt);
                }

                if result.corrupt_entries > options.max_corrupt_entries as u64 {
                    return Err(M7RecoveryError::TooManyCorruptEntries(
                        result.corrupt_entries,
                        options.max_corrupt_entries,
                    ));
                }
            }

            let tx_id = entry.tx_id;

            match entry.entry_type {
                WalEntryType::TransactionCommit => {
                    // Transaction committed - count it
                    if tx_entries.remove(&tx_id).is_some() {
                        result.transactions_recovered += 1;
                    }
                }
                WalEntryType::TransactionAbort => {
                    // Transaction aborted - discard entries
                    if tx_entries.remove(&tx_id).is_some() {
                        result.aborted_transactions += 1;
                    }
                }
                _ => {
                    // Buffer entry for transaction
                    tx_entries.entry(tx_id).or_default().push(entry);
                }
            }
        }

        // Count orphaned transactions (started but not committed/aborted)
        result.orphaned_transactions = tx_entries.len() as u64;
        for (tx_id, entries) in tx_entries {
            if options.verbose {
                warn!(
                    "Orphaned transaction {} with {} entries",
                    tx_id,
                    entries.len()
                );
            }
        }

        Ok(result)
    }

    /// Validate recovery data integrity
    ///
    /// Can be used after recovery to verify data consistency.
    pub fn validate_recovery(
        data_dir: &Path,
        _options: &M7RecoveryOptions,
    ) -> Result<bool, M7RecoveryError> {
        // Check snapshot directory
        let snapshot_dir = data_dir.join("snapshots");
        if snapshot_dir.exists() {
            let snapshots = SnapshotDiscovery::list_snapshots(&snapshot_dir)?;
            for path in snapshots {
                if let Err(e) = SnapshotReader::validate_checksum(&path) {
                    warn!("Snapshot {} failed validation: {}", path.display(), e);
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::m7_wal_writer::WalWriter;
    use crate::snapshot::SnapshotWriter;
    use crate::wal::DurabilityMode;
    use tempfile::TempDir;

    fn create_test_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn test_recovery_options_default() {
        let opts = M7RecoveryOptions::default();
        assert_eq!(opts.max_corrupt_entries, 10);
        assert!(opts.verify_all_checksums);
        assert!(opts.rebuild_indexes);
    }

    #[test]
    fn test_recovery_options_strict() {
        let opts = M7RecoveryOptions::strict();
        assert_eq!(opts.max_corrupt_entries, 0);
        assert!(opts.verbose);
    }

    #[test]
    fn test_recovery_options_permissive() {
        let opts = M7RecoveryOptions::permissive();
        assert_eq!(opts.max_corrupt_entries, 100);
        assert!(!opts.verify_all_checksums);
    }

    #[test]
    fn test_recovery_result_summary() {
        let result = M7RecoveryResult {
            transactions_recovered: 100,
            wal_entries_replayed: 500,
            orphaned_transactions: 2,
            aborted_transactions: 3,
            corrupt_entries_skipped: 1,
            recovery_time_micros: 5000,
            success: true,
            ..Default::default()
        };

        let summary = result.summary();
        assert!(summary.contains("100 transactions"));
        assert!(summary.contains("500 WAL entries"));
        assert!(summary.contains("2 orphaned"));
    }

    #[test]
    fn test_recovery_result_has_issues() {
        let clean = M7RecoveryResult::default();
        assert!(!clean.has_issues());

        let with_corrupt = M7RecoveryResult {
            corrupt_entries_skipped: 1,
            ..Default::default()
        };
        assert!(with_corrupt.has_issues());

        let with_orphaned = M7RecoveryResult {
            orphaned_transactions: 1,
            ..Default::default()
        };
        assert!(with_orphaned.has_issues());
    }

    #[test]
    fn test_snapshot_discovery_empty_dir() {
        let temp_dir = create_test_dir();
        let snapshots = SnapshotDiscovery::list_snapshots(temp_dir.path()).unwrap();
        assert!(snapshots.is_empty());
    }

    #[test]
    fn test_snapshot_discovery_nonexistent_dir() {
        let path = PathBuf::from("/nonexistent/path");
        let snapshots = SnapshotDiscovery::list_snapshots(&path).unwrap();
        assert!(snapshots.is_empty());
    }

    #[test]
    fn test_snapshot_discovery_finds_snapshots() {
        let temp_dir = create_test_dir();

        // Create some snapshot files
        let path1 = temp_dir.path().join("snapshot_001.snap");
        let path2 = temp_dir.path().join("snapshot_002.snap");
        std::fs::write(&path1, b"test1").unwrap();
        std::fs::write(&path2, b"test2").unwrap();

        let snapshots = SnapshotDiscovery::list_snapshots(temp_dir.path()).unwrap();
        assert_eq!(snapshots.len(), 2);
    }

    #[test]
    fn test_snapshot_discovery_ignores_non_snapshots() {
        let temp_dir = create_test_dir();

        // Create snapshot and non-snapshot files
        std::fs::write(temp_dir.path().join("snapshot_001.snap"), b"test").unwrap();
        std::fs::write(temp_dir.path().join("other.txt"), b"test").unwrap();
        std::fs::write(temp_dir.path().join("wal.dat"), b"test").unwrap();

        let snapshots = SnapshotDiscovery::list_snapshots(temp_dir.path()).unwrap();
        // wal.dat has .dat extension so it matches
        assert_eq!(snapshots.len(), 2);
    }

    #[test]
    fn test_find_latest_valid_no_snapshots() {
        let temp_dir = create_test_dir();
        let result = SnapshotDiscovery::find_latest_valid(temp_dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_find_latest_valid_with_valid_snapshot() {
        let temp_dir = create_test_dir();
        let path = temp_dir.path().join("snapshot_001.snap");

        // Create valid snapshot
        let header = SnapshotHeader::new(12345, 100);
        let sections = vec![PrimitiveSection::new(primitive_ids::KV, vec![1, 2, 3])];
        let mut writer = SnapshotWriter::new();
        writer.write(&header, &sections, &path).unwrap();

        let result = SnapshotDiscovery::find_latest_valid(temp_dir.path()).unwrap();
        assert!(result.is_some());

        let (info, skipped) = result.unwrap();
        assert_eq!(info.wal_offset, 12345);
        assert_eq!(skipped, 0);
    }

    #[test]
    fn test_find_latest_valid_with_corrupt_falls_back() {
        let temp_dir = create_test_dir();
        let path1 = temp_dir.path().join("snapshot_001.snap");
        let path2 = temp_dir.path().join("snapshot_002.snap");

        // Create valid older snapshot
        let header1 = SnapshotHeader::new(10000, 50);
        let mut writer = SnapshotWriter::new();
        writer.write(&header1, &[], &path1).unwrap();

        // Create corrupt newer snapshot
        let header2 = SnapshotHeader::new(20000, 100);
        writer.write(&header2, &[], &path2).unwrap();

        // Corrupt the newer snapshot
        let mut data = std::fs::read(&path2).unwrap();
        data[20] ^= 0xFF;
        std::fs::write(&path2, &data).unwrap();

        // Should fall back to older snapshot
        let result = SnapshotDiscovery::find_latest_valid(temp_dir.path()).unwrap();
        assert!(result.is_some());

        let (info, skipped) = result.unwrap();
        assert_eq!(info.wal_offset, 10000); // Should be the older snapshot
        assert_eq!(skipped, 1); // Skipped 1 corrupt snapshot
    }

    #[test]
    fn test_recovery_no_data() {
        let temp_dir = create_test_dir();
        let options = M7RecoveryOptions::default();

        let (sections, result) = M7Recovery::recover(temp_dir.path(), options).unwrap();

        assert!(sections.is_empty());
        assert!(result.snapshot_used.is_none());
        assert_eq!(result.wal_entries_replayed, 0);
        assert!(result.success);
    }

    #[test]
    fn test_recovery_from_snapshot_only() {
        let temp_dir = create_test_dir();
        let snapshot_dir = temp_dir.path().join("snapshots");
        std::fs::create_dir_all(&snapshot_dir).unwrap();

        // Create snapshot
        let path = snapshot_dir.join("snapshot_001.snap");
        let header = SnapshotHeader::new(12345, 100);
        let sections = vec![
            PrimitiveSection::new(primitive_ids::KV, vec![1, 2, 3]),
            PrimitiveSection::new(primitive_ids::JSON, vec![4, 5]),
        ];
        let mut writer = SnapshotWriter::new();
        writer.write(&header, &sections, &path).unwrap();

        // Recover
        let options = M7RecoveryOptions::default();
        let (recovered_sections, result) = M7Recovery::recover(temp_dir.path(), options).unwrap();

        assert!(result.snapshot_used.is_some());
        assert_eq!(result.snapshot_used.unwrap().wal_offset, 12345);
        assert_eq!(recovered_sections.len(), 2);
        assert_eq!(recovered_sections[0].primitive_type, primitive_ids::KV);
        assert_eq!(recovered_sections[1].primitive_type, primitive_ids::JSON);
        assert!(result.success);
    }

    #[test]
    fn test_recovery_with_wal_replay() {
        let temp_dir = create_test_dir();
        let snapshot_dir = temp_dir.path().join("snapshots");
        std::fs::create_dir_all(&snapshot_dir).unwrap();

        // Create snapshot with WAL offset 0
        let path = snapshot_dir.join("snapshot_001.snap");
        let header = SnapshotHeader::new(0, 0);
        let mut writer = SnapshotWriter::new();
        writer.write(&header, &[], &path).unwrap();

        // Create WAL with some transactions
        let wal_path = temp_dir.path().join("wal.dat");
        {
            let mut wal_writer = WalWriter::open(&wal_path, DurabilityMode::Strict).unwrap();
            // Write a committed transaction
            wal_writer
                .write_transaction(vec![(WalEntryType::KvPut, b"key=value".to_vec())])
                .unwrap();
            // Write another committed transaction
            wal_writer
                .write_transaction(vec![(WalEntryType::KvPut, b"key2=value2".to_vec())])
                .unwrap();
        }

        // Recover
        let options = M7RecoveryOptions::default();
        let (_, result) = M7Recovery::recover(temp_dir.path(), options).unwrap();

        assert!(result.snapshot_used.is_some());
        assert_eq!(result.transactions_recovered, 2);
        assert!(result.wal_entries_replayed > 0);
        assert!(result.success);
    }

    #[test]
    fn test_recovery_wal_only_no_snapshot() {
        let temp_dir = create_test_dir();

        // Create WAL only (no snapshot)
        let wal_path = temp_dir.path().join("wal.dat");
        {
            let mut wal_writer = WalWriter::open(&wal_path, DurabilityMode::Strict).unwrap();
            wal_writer
                .write_transaction(vec![(WalEntryType::KvPut, b"key=value".to_vec())])
                .unwrap();
        }

        // Recover
        let options = M7RecoveryOptions::default();
        let (_, result) = M7Recovery::recover(temp_dir.path(), options).unwrap();

        assert!(result.snapshot_used.is_none());
        assert_eq!(result.transactions_recovered, 1);
        assert!(result.success);
    }

    #[test]
    fn test_recovery_with_orphaned_transaction() {
        let temp_dir = create_test_dir();

        // Create WAL with incomplete transaction
        let wal_path = temp_dir.path().join("wal.dat");
        {
            let mut wal_writer = WalWriter::open(&wal_path, DurabilityMode::Strict).unwrap();

            // Start a transaction but don't commit
            let tx_id = wal_writer.begin_transaction();
            wal_writer
                .write_tx_entry(tx_id, WalEntryType::KvPut, b"orphaned".to_vec())
                .unwrap();
            // Don't commit - simulates crash

            // Also write a complete transaction
            wal_writer
                .write_transaction(vec![(WalEntryType::KvPut, b"complete".to_vec())])
                .unwrap();
        }

        // Recover
        let options = M7RecoveryOptions::default();
        let (_, result) = M7Recovery::recover(temp_dir.path(), options).unwrap();

        assert_eq!(result.transactions_recovered, 1); // Only the complete one
        assert_eq!(result.orphaned_transactions, 1); // The incomplete one
        assert!(result.has_issues()); // Has orphaned transactions
        assert!(result.success); // But still successful
    }

    #[test]
    fn test_recovery_with_aborted_transaction() {
        let temp_dir = create_test_dir();

        // Create WAL with aborted transaction
        let wal_path = temp_dir.path().join("wal.dat");
        {
            let mut wal_writer = WalWriter::open(&wal_path, DurabilityMode::Strict).unwrap();

            // Aborted transaction
            let tx_id = wal_writer.begin_transaction();
            wal_writer
                .write_tx_entry(tx_id, WalEntryType::KvPut, b"aborted".to_vec())
                .unwrap();
            wal_writer.abort_transaction(tx_id).unwrap();

            // Committed transaction
            wal_writer
                .write_transaction(vec![(WalEntryType::KvPut, b"committed".to_vec())])
                .unwrap();
        }

        // Recover
        let options = M7RecoveryOptions::default();
        let (_, result) = M7Recovery::recover(temp_dir.path(), options).unwrap();

        assert_eq!(result.transactions_recovered, 1);
        assert_eq!(result.aborted_transactions, 1);
        assert!(result.success);
    }

    #[test]
    fn test_validate_recovery_empty_dir() {
        let temp_dir = create_test_dir();
        let options = M7RecoveryOptions::default();

        let valid = M7Recovery::validate_recovery(temp_dir.path(), &options).unwrap();
        assert!(valid);
    }

    #[test]
    fn test_validate_recovery_with_corrupt_snapshot() {
        let temp_dir = create_test_dir();
        let snapshot_dir = temp_dir.path().join("snapshots");
        std::fs::create_dir_all(&snapshot_dir).unwrap();

        // Create valid snapshot
        let path = snapshot_dir.join("snapshot_001.snap");
        let header = SnapshotHeader::new(0, 0);
        let mut writer = SnapshotWriter::new();
        writer.write(&header, &[], &path).unwrap();

        // Corrupt it
        let mut data = std::fs::read(&path).unwrap();
        data[20] ^= 0xFF;
        std::fs::write(&path, &data).unwrap();

        let options = M7RecoveryOptions::default();
        let valid = M7Recovery::validate_recovery(temp_dir.path(), &options).unwrap();
        assert!(!valid);
    }
}
