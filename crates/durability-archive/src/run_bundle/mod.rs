//! RunBundle - Portable Execution Artifacts
//!
//! This module implements export and import of Strata runs as portable archives.
//!
//! A RunBundle is an immutable artifact representing a single completed run.
//! It can be exported from one Strata instance and imported into another for
//! replay, inspection, or debugging.
//!
//! ## Archive Format
//!
//! RunBundles use the `.runbundle.tar.zst` format - a zstd-compressed tar archive:
//!
//! ```text
//! <run_id>.runbundle.tar.zst
//! └── runbundle/
//!     ├── MANIFEST.json    # Format version, checksums
//!     ├── RUN.json         # Run metadata (id, state, tags, error)
//!     └── WAL.runlog       # Run-scoped WAL entries
//! ```
//!
//! ## Usage
//!
//! Export a completed run:
//! ```ignore
//! let info = db.export_run(&run_id, Path::new("./my-run.runbundle.tar.zst"))?;
//! ```
//!
//! Verify a bundle:
//! ```ignore
//! let info = db.verify_bundle(Path::new("./my-run.runbundle.tar.zst"))?;
//! ```
//!
//! Import into an empty database:
//! ```ignore
//! let info = db.import_run(Path::new("./my-run.runbundle.tar.zst"))?;
//! ```
//!
//! ## Design Principles
//!
//! - **Explicit**: All operations are explicit, no background behavior
//! - **Immutable**: Only terminal runs (Completed, Failed, Cancelled, Archived) can be exported
//! - **Portable**: Archives can be moved between machines, stored in VCS
//! - **Inspectable**: Standard tools (tar, jq) can inspect contents
//! - **Deterministic**: Same run exported twice produces identical bundles

mod error;
mod reader;
mod types;
mod wal_log;
mod writer;

// Re-export public types
pub use error::{RunBundleError, RunBundleResult};
pub use reader::{BundleContents as ReadBundleContents, RunBundleReader};
pub use types::{
    paths, xxh3_hex, BundleContents, BundleManifest, BundleRunInfo, BundleVerifyInfo,
    ExportOptions, ImportedRunInfo, RunExportInfo, RUNBUNDLE_EXTENSION, RUNBUNDLE_FORMAT_VERSION,
    WAL_RUNLOG_MAGIC, WAL_RUNLOG_VERSION,
};
pub use wal_log::{filter_wal_for_run, WalLogInfo, WalLogIterator, WalLogReader, WalLogWriter};
pub use writer::RunBundleWriter;
