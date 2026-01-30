//! RunBundle archive reader
//!
//! Reads .runbundle.tar.zst archives and validates their contents.

use crate::run_bundle::error::{RunBundleError, RunBundleResult};
use crate::run_bundle::types::{
    paths, xxh3_hex, BundleManifest, BundleRunInfo, BundleVerifyInfo, RUNBUNDLE_FORMAT_VERSION,
};
use crate::run_bundle::wal_log::WalLogReader;
use crate::wal::WALEntry;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use tar::Archive;

/// Reader for RunBundle archives
///
/// Reads and validates .runbundle.tar.zst files.
pub struct RunBundleReader;

impl RunBundleReader {
    /// Validate a bundle's integrity without fully parsing WAL entries
    ///
    /// Checks:
    /// - Archive can be decompressed
    /// - Required files exist (MANIFEST.json, RUN.json, WAL.runlog)
    /// - Checksums match manifest
    /// - WAL.runlog header is valid
    pub fn validate(path: &Path) -> RunBundleResult<BundleVerifyInfo> {
        let files = Self::extract_all_files(path)?;

        // Check required files
        let manifest_data = files
            .get("MANIFEST.json")
            .ok_or_else(|| RunBundleError::missing_file("MANIFEST.json"))?;
        let run_data = files
            .get("RUN.json")
            .ok_or_else(|| RunBundleError::missing_file("RUN.json"))?;
        let wal_data = files
            .get("WAL.runlog")
            .ok_or_else(|| RunBundleError::missing_file("WAL.runlog"))?;

        // Parse manifest
        let manifest: BundleManifest = serde_json::from_slice(manifest_data)?;

        // Validate format version
        if manifest.format_version != RUNBUNDLE_FORMAT_VERSION {
            return Err(RunBundleError::UnsupportedVersion {
                version: manifest.format_version,
            });
        }

        // Validate checksums
        let mut checksums_valid = true;

        if let Some(expected) = manifest.checksums.get("RUN.json") {
            let actual = xxh3_hex(run_data);
            if expected != &actual {
                checksums_valid = false;
            }
        }

        if let Some(expected) = manifest.checksums.get("WAL.runlog") {
            let actual = xxh3_hex(wal_data);
            if expected != &actual {
                checksums_valid = false;
            }
        }

        // Validate WAL header (without parsing entries)
        WalLogReader::validate(std::io::Cursor::new(wal_data))?;

        // Parse run info for run_id
        let run_info: BundleRunInfo = serde_json::from_slice(run_data)?;

        Ok(BundleVerifyInfo {
            run_id: run_info.run_id,
            format_version: manifest.format_version,
            wal_entry_count: manifest.contents.wal_entry_count,
            checksums_valid,
        })
    }

    /// Read and parse the manifest
    pub fn read_manifest(path: &Path) -> RunBundleResult<BundleManifest> {
        let data = Self::extract_file(path, "MANIFEST.json")?;
        let manifest: BundleManifest = serde_json::from_slice(&data)?;

        if manifest.format_version != RUNBUNDLE_FORMAT_VERSION {
            return Err(RunBundleError::UnsupportedVersion {
                version: manifest.format_version,
            });
        }

        Ok(manifest)
    }

    /// Read and parse the run info
    pub fn read_run_info(path: &Path) -> RunBundleResult<BundleRunInfo> {
        let data = Self::extract_file(path, "RUN.json")?;
        let run_info: BundleRunInfo = serde_json::from_slice(&data)?;
        Ok(run_info)
    }

    /// Read and parse WAL entries
    pub fn read_wal_entries(path: &Path) -> RunBundleResult<Vec<WALEntry>> {
        let data = Self::extract_file(path, "WAL.runlog")?;
        WalLogReader::read_from_slice(&data)
    }

    /// Read and parse WAL entries with checksum validation
    pub fn read_wal_entries_validated(path: &Path) -> RunBundleResult<Vec<WALEntry>> {
        let files = Self::extract_all_files(path)?;

        let manifest_data = files
            .get("MANIFEST.json")
            .ok_or_else(|| RunBundleError::missing_file("MANIFEST.json"))?;
        let wal_data = files
            .get("WAL.runlog")
            .ok_or_else(|| RunBundleError::missing_file("WAL.runlog"))?;

        let manifest: BundleManifest = serde_json::from_slice(manifest_data)?;

        // Validate WAL checksum
        if let Some(expected) = manifest.checksums.get("WAL.runlog") {
            let actual = xxh3_hex(wal_data);
            if expected != &actual {
                return Err(RunBundleError::ChecksumMismatch {
                    file: "WAL.runlog".to_string(),
                    expected: expected.clone(),
                    actual,
                });
            }
        }

        WalLogReader::read_from_slice(wal_data)
    }

    /// Read all components from the bundle
    pub fn read_all(path: &Path) -> RunBundleResult<BundleContents> {
        let files = Self::extract_all_files(path)?;

        let manifest_data = files
            .get("MANIFEST.json")
            .ok_or_else(|| RunBundleError::missing_file("MANIFEST.json"))?;
        let run_data = files
            .get("RUN.json")
            .ok_or_else(|| RunBundleError::missing_file("RUN.json"))?;
        let wal_data = files
            .get("WAL.runlog")
            .ok_or_else(|| RunBundleError::missing_file("WAL.runlog"))?;

        let manifest: BundleManifest = serde_json::from_slice(manifest_data)?;
        let run_info: BundleRunInfo = serde_json::from_slice(run_data)?;
        let wal_entries = WalLogReader::read_from_slice(wal_data)?;

        Ok(BundleContents {
            manifest,
            run_info,
            wal_entries,
        })
    }

    /// Extract a single file from the archive
    fn extract_file(path: &Path, file_name: &str) -> RunBundleResult<Vec<u8>> {
        let file = File::open(path)?;
        let buf_reader = BufReader::new(file);
        let decoder = zstd::Decoder::new(buf_reader)
            .map_err(|e| RunBundleError::compression(format!("zstd decode: {}", e)))?;

        let mut archive = Archive::new(decoder);
        let target_path = format!("{}/{}", paths::ROOT, file_name);

        for entry in archive.entries().map_err(|e| RunBundleError::archive(e.to_string()))? {
            let mut entry = entry.map_err(|e| RunBundleError::archive(e.to_string()))?;
            let entry_path = entry
                .path()
                .map_err(|e| RunBundleError::archive(e.to_string()))?
                .to_string_lossy()
                .to_string();

            if entry_path == target_path {
                let mut data = Vec::new();
                entry
                    .read_to_end(&mut data)
                    .map_err(|e| RunBundleError::archive(format!("read {}: {}", file_name, e)))?;
                return Ok(data);
            }
        }

        Err(RunBundleError::missing_file(file_name))
    }

    /// Extract all files from the archive into a HashMap
    fn extract_all_files(path: &Path) -> RunBundleResult<HashMap<String, Vec<u8>>> {
        let file = File::open(path)?;
        let buf_reader = BufReader::new(file);
        let decoder = zstd::Decoder::new(buf_reader)
            .map_err(|e| RunBundleError::compression(format!("zstd decode: {}", e)))?;

        let mut archive = Archive::new(decoder);
        let mut files = HashMap::new();
        let prefix = format!("{}/", paths::ROOT);

        for entry in archive.entries().map_err(|e| RunBundleError::archive(e.to_string()))? {
            let mut entry = entry.map_err(|e| RunBundleError::archive(e.to_string()))?;
            let entry_path = entry
                .path()
                .map_err(|e| RunBundleError::archive(e.to_string()))?
                .to_string_lossy()
                .to_string();

            // Strip prefix to get relative file name
            if let Some(name) = entry_path.strip_prefix(&prefix) {
                if !name.is_empty() {
                    let mut data = Vec::new();
                    entry
                        .read_to_end(&mut data)
                        .map_err(|e| RunBundleError::archive(format!("read {}: {}", name, e)))?;
                    files.insert(name.to_string(), data);
                }
            }
        }

        Ok(files)
    }

    /// Read from a byte slice (for testing)
    pub fn read_manifest_from_bytes(data: &[u8]) -> RunBundleResult<BundleManifest> {
        let decoder = zstd::Decoder::new(data)
            .map_err(|e| RunBundleError::compression(format!("zstd decode: {}", e)))?;

        let mut archive = Archive::new(decoder);
        let target_path = paths::MANIFEST;

        for entry in archive.entries().map_err(|e| RunBundleError::archive(e.to_string()))? {
            let mut entry = entry.map_err(|e| RunBundleError::archive(e.to_string()))?;
            let entry_path = entry
                .path()
                .map_err(|e| RunBundleError::archive(e.to_string()))?
                .to_string_lossy()
                .to_string();

            if entry_path == target_path {
                let mut data = Vec::new();
                entry.read_to_end(&mut data)?;
                let manifest: BundleManifest = serde_json::from_slice(&data)?;
                return Ok(manifest);
            }
        }

        Err(RunBundleError::missing_file("MANIFEST.json"))
    }

    /// Read run info from a byte slice (for testing)
    pub fn read_run_info_from_bytes(data: &[u8]) -> RunBundleResult<BundleRunInfo> {
        let decoder = zstd::Decoder::new(data)
            .map_err(|e| RunBundleError::compression(format!("zstd decode: {}", e)))?;

        let mut archive = Archive::new(decoder);
        let target_path = paths::RUN;

        for entry in archive.entries().map_err(|e| RunBundleError::archive(e.to_string()))? {
            let mut entry = entry.map_err(|e| RunBundleError::archive(e.to_string()))?;
            let entry_path = entry
                .path()
                .map_err(|e| RunBundleError::archive(e.to_string()))?
                .to_string_lossy()
                .to_string();

            if entry_path == target_path {
                let mut data = Vec::new();
                entry.read_to_end(&mut data)?;
                let run_info: BundleRunInfo = serde_json::from_slice(&data)?;
                return Ok(run_info);
            }
        }

        Err(RunBundleError::missing_file("RUN.json"))
    }

    /// Read WAL entries from a byte slice (for testing)
    pub fn read_wal_entries_from_bytes(data: &[u8]) -> RunBundleResult<Vec<WALEntry>> {
        let decoder = zstd::Decoder::new(data)
            .map_err(|e| RunBundleError::compression(format!("zstd decode: {}", e)))?;

        let mut archive = Archive::new(decoder);
        let target_path = paths::WAL;

        for entry in archive.entries().map_err(|e| RunBundleError::archive(e.to_string()))? {
            let mut entry = entry.map_err(|e| RunBundleError::archive(e.to_string()))?;
            let entry_path = entry
                .path()
                .map_err(|e| RunBundleError::archive(e.to_string()))?
                .to_string_lossy()
                .to_string();

            if entry_path == target_path {
                let mut wal_data = Vec::new();
                entry.read_to_end(&mut wal_data)?;
                return WalLogReader::read_from_slice(&wal_data);
            }
        }

        Err(RunBundleError::missing_file("WAL.runlog"))
    }
}

/// Complete bundle contents after reading
#[derive(Debug)]
pub struct BundleContents {
    /// Bundle manifest
    pub manifest: BundleManifest,
    /// Run metadata
    pub run_info: BundleRunInfo,
    /// WAL entries
    pub wal_entries: Vec<WALEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_bundle::writer::RunBundleWriter;
    use crate::run_bundle::types::ExportOptions;
    use crate::wal::WALEntry;
    use strata_core::types::{Key, Namespace, RunId, TypeTag};
    use strata_core::value::Value;
    use strata_core::Timestamp;
    use tempfile::tempdir;

    fn make_test_run_info() -> BundleRunInfo {
        BundleRunInfo {
            run_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            name: "test-run".to_string(),
            state: "completed".to_string(),
            created_at: "2025-01-24T10:00:00Z".to_string(),
            closed_at: "2025-01-24T11:00:00Z".to_string(),
            parent_run_id: None,
            tags: vec!["test".to_string()],
            metadata: serde_json::json!({"key": "value"}),
            error: None,
        }
    }

    fn make_test_entries() -> Vec<WALEntry> {
        let run_id = RunId::new();
        let ns = Namespace::for_run(run_id);
        vec![
            WALEntry::BeginTxn {
                txn_id: 1,
                run_id,
                timestamp: Timestamp::now(),
            },
            WALEntry::Write {
                run_id,
                key: Key::new(ns.clone(), TypeTag::KV, b"key1".to_vec()),
                value: Value::String("value1".to_string()),
                version: 1,
            },
            WALEntry::CommitTxn { txn_id: 1, run_id },
        ]
    }

    fn create_test_bundle() -> (Vec<u8>, BundleRunInfo, Vec<WALEntry>) {
        let writer = RunBundleWriter::new(&ExportOptions::default());
        let run_info = make_test_run_info();
        let entries = make_test_entries();
        let (data, _) = writer.write_to_vec(&run_info, &entries).unwrap();
        (data, run_info, entries)
    }

    #[test]
    fn test_read_manifest_from_bytes() {
        let (data, _, _) = create_test_bundle();

        let manifest = RunBundleReader::read_manifest_from_bytes(&data).unwrap();

        assert_eq!(manifest.format_version, RUNBUNDLE_FORMAT_VERSION);
        assert!(manifest.checksums.contains_key("RUN.json"));
        assert!(manifest.checksums.contains_key("WAL.runlog"));
    }

    #[test]
    fn test_read_run_info_from_bytes() {
        let (data, expected_run_info, _) = create_test_bundle();

        let run_info = RunBundleReader::read_run_info_from_bytes(&data).unwrap();

        assert_eq!(run_info.run_id, expected_run_info.run_id);
        assert_eq!(run_info.name, expected_run_info.name);
        assert_eq!(run_info.state, expected_run_info.state);
    }

    #[test]
    fn test_read_wal_entries_from_bytes() {
        let (data, _, expected_entries) = create_test_bundle();

        let entries = RunBundleReader::read_wal_entries_from_bytes(&data).unwrap();

        assert_eq!(entries.len(), expected_entries.len());
    }

    #[test]
    fn test_validate_from_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.runbundle.tar.zst");

        let writer = RunBundleWriter::new(&ExportOptions::default());
        let run_info = make_test_run_info();
        let entries = make_test_entries();
        writer.write(&run_info, &entries, &path).unwrap();

        let verify_info = RunBundleReader::validate(&path).unwrap();

        assert_eq!(verify_info.run_id, run_info.run_id);
        assert_eq!(verify_info.format_version, RUNBUNDLE_FORMAT_VERSION);
        assert_eq!(verify_info.wal_entry_count, 3);
        assert!(verify_info.checksums_valid);
    }

    #[test]
    fn test_read_manifest_from_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.runbundle.tar.zst");

        let writer = RunBundleWriter::new(&ExportOptions::default());
        let run_info = make_test_run_info();
        let entries = make_test_entries();
        writer.write(&run_info, &entries, &path).unwrap();

        let manifest = RunBundleReader::read_manifest(&path).unwrap();

        assert_eq!(manifest.format_version, RUNBUNDLE_FORMAT_VERSION);
        assert_eq!(manifest.contents.wal_entry_count, 3);
    }

    #[test]
    fn test_read_run_info_from_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.runbundle.tar.zst");

        let writer = RunBundleWriter::new(&ExportOptions::default());
        let expected_run_info = make_test_run_info();
        let entries = make_test_entries();
        writer.write(&expected_run_info, &entries, &path).unwrap();

        let run_info = RunBundleReader::read_run_info(&path).unwrap();

        assert_eq!(run_info.run_id, expected_run_info.run_id);
        assert_eq!(run_info.name, expected_run_info.name);
    }

    #[test]
    fn test_read_wal_entries_from_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.runbundle.tar.zst");

        let writer = RunBundleWriter::new(&ExportOptions::default());
        let run_info = make_test_run_info();
        let entries = make_test_entries();
        writer.write(&run_info, &entries, &path).unwrap();

        let read_entries = RunBundleReader::read_wal_entries(&path).unwrap();

        assert_eq!(read_entries.len(), entries.len());
    }

    #[test]
    fn test_read_wal_entries_validated() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.runbundle.tar.zst");

        let writer = RunBundleWriter::new(&ExportOptions::default());
        let run_info = make_test_run_info();
        let entries = make_test_entries();
        writer.write(&run_info, &entries, &path).unwrap();

        let read_entries = RunBundleReader::read_wal_entries_validated(&path).unwrap();

        assert_eq!(read_entries.len(), entries.len());
    }

    #[test]
    fn test_read_all() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.runbundle.tar.zst");

        let writer = RunBundleWriter::new(&ExportOptions::default());
        let run_info = make_test_run_info();
        let entries = make_test_entries();
        writer.write(&run_info, &entries, &path).unwrap();

        let contents = RunBundleReader::read_all(&path).unwrap();

        assert_eq!(contents.run_info.run_id, run_info.run_id);
        assert_eq!(contents.wal_entries.len(), entries.len());
        assert_eq!(contents.manifest.contents.wal_entry_count, 3);
    }

    #[test]
    fn test_missing_file_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.runbundle.tar.zst");

        let result = RunBundleReader::validate(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_corrupted_archive() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("corrupted.runbundle.tar.zst");

        // Write garbage
        std::fs::write(&path, b"not a valid archive").unwrap();

        let result = RunBundleReader::validate(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_bundle() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("empty.runbundle.tar.zst");

        let writer = RunBundleWriter::new(&ExportOptions::default());
        let run_info = make_test_run_info();
        let entries: Vec<WALEntry> = vec![];
        writer.write(&run_info, &entries, &path).unwrap();

        let contents = RunBundleReader::read_all(&path).unwrap();

        assert!(contents.wal_entries.is_empty());
        assert_eq!(contents.manifest.contents.wal_entry_count, 0);
    }

    #[test]
    fn test_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("roundtrip.runbundle.tar.zst");

        // Write
        let writer = RunBundleWriter::new(&ExportOptions::default());
        let original_run_info = make_test_run_info();
        let original_entries = make_test_entries();
        writer
            .write(&original_run_info, &original_entries, &path)
            .unwrap();

        // Read back
        let contents = RunBundleReader::read_all(&path).unwrap();

        // Verify
        assert_eq!(contents.run_info.run_id, original_run_info.run_id);
        assert_eq!(contents.run_info.name, original_run_info.name);
        assert_eq!(contents.run_info.state, original_run_info.state);
        assert_eq!(contents.run_info.tags, original_run_info.tags);
        assert_eq!(contents.wal_entries.len(), original_entries.len());

        // Verify WAL entries match
        for (original, read) in original_entries.iter().zip(contents.wal_entries.iter()) {
            assert_eq!(original, read);
        }
    }
}
