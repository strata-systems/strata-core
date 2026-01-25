//! Main database entry point for Strata.
//!
//! This module provides the `Strata` struct, the primary entry point for
//! all database operations.

use crate::error::{Error, Result};
use crate::primitives::{Events, Json, Runs, State, Vectors, KV};
use std::path::Path;
use std::sync::Arc;

/// The Strata database.
///
/// This is the main entry point for all database operations. Create a database
/// using [`Strata::open`] or [`Strata::builder`].
///
/// # Example
///
/// ```ignore
/// use strata::prelude::*;
///
/// // Open with default settings
/// let db = Strata::open("./my-db")?;
///
/// // Access primitives
/// db.kv.set("key", "value")?;
/// db.json.set("doc", json!({"name": "Alice"}))?;
/// db.events.append("stream", json!({"action": "login"}))?;
///
/// // Graceful shutdown
/// db.close()?;
/// ```
pub struct Strata {
    /// The underlying engine database
    pub(crate) inner: Arc<strata_engine::Database>,

    /// Key-value operations
    pub kv: KV,

    /// JSON document operations
    pub json: Json,

    /// Event stream operations
    pub events: Events,

    /// State cell operations
    pub state: State,

    /// Vector similarity search
    pub vectors: Vectors,

    /// Run lifecycle management
    pub runs: Runs,
}

impl Strata {
    /// Open a database at the given path.
    ///
    /// Uses default settings (buffered durability mode).
    ///
    /// # Arguments
    ///
    /// * `path` - Directory path for the database files
    ///
    /// # Example
    ///
    /// ```ignore
    /// let db = Strata::open("./my-db")?;
    /// ```
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::builder().path(path).open()
    }

    /// Create an ephemeral database with no disk I/O.
    ///
    /// This creates a truly in-memory database that:
    /// - Creates no files or directories
    /// - Has no WAL (write-ahead log)
    /// - Cannot recover after crash
    /// - Loses all data when dropped
    ///
    /// Use this for:
    /// - Unit tests that need maximum isolation and speed
    /// - Caching scenarios
    /// - Temporary computations
    ///
    /// # Example
    ///
    /// ```ignore
    /// use stratadb::prelude::*;
    ///
    /// let db = Strata::ephemeral()?;
    ///
    /// // All operations work normally
    /// db.kv.set("key", "value")?;
    /// let value = db.kv.get("key")?;
    ///
    /// // But data is gone when db is dropped
    /// drop(db);
    /// ```
    ///
    /// # Comparison
    ///
    /// | Method | Disk Files | Recovery |
    /// |--------|------------|----------|
    /// | `Strata::ephemeral()` | None | No |
    /// | `Strata::open_temp()` | Temp dir | Yes |
    /// | `Strata::open(path)` | User dir | Yes |
    pub fn ephemeral() -> Result<Self> {
        let db = Arc::new(strata_engine::Database::ephemeral().map_err(Error::from)?);
        Ok(Self::from_engine(db))
    }

    /// Create a builder for database configuration.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let db = Strata::builder()
    ///     .path("./my-db")
    ///     .no_durability()
    ///     .open()?;
    /// ```
    pub fn builder() -> StrataBuilder {
        StrataBuilder::new()
    }

    /// Force flush all pending writes to disk.
    ///
    /// In buffered mode, writes are batched for performance.
    /// Call `flush()` to ensure all data is persisted.
    pub fn flush(&self) -> Result<()> {
        self.inner.flush().map_err(Into::into)
    }

    /// Gracefully close the database.
    ///
    /// Flushes pending writes, closes files, and releases resources.
    /// After calling `close()`, the database should not be used.
    pub fn close(&self) -> Result<()> {
        self.inner.shutdown().map_err(Into::into)
    }

    /// Get the database directory path.
    pub fn path(&self) -> &Path {
        self.inner.data_dir()
    }

    /// Get the current durability mode.
    pub fn durability_mode(&self) -> strata_engine::DurabilityMode {
        self.inner.durability_mode()
    }

    /// Check if this is an ephemeral (no-disk) database.
    ///
    /// Returns `true` if created with [`Strata::ephemeral()`].
    pub fn is_ephemeral(&self) -> bool {
        self.inner.is_ephemeral()
    }

    /// Get database metrics.
    pub fn metrics(&self) -> DatabaseMetrics {
        let txn_metrics = self.inner.coordinator().metrics();
        DatabaseMetrics {
            transactions_committed: txn_metrics.total_committed,
            transactions_aborted: txn_metrics.total_aborted,
            transactions_active: txn_metrics.active_count,
            commit_rate: txn_metrics.commit_rate,
            operations: txn_metrics.total_committed + txn_metrics.total_aborted,
        }
    }
}

/// Database metrics.
#[derive(Debug, Clone)]
pub struct DatabaseMetrics {
    /// Total committed transactions
    pub transactions_committed: u64,
    /// Total aborted transactions
    pub transactions_aborted: u64,
    /// Currently active transactions
    pub transactions_active: u64,
    /// Commit success rate (0.0 - 1.0)
    pub commit_rate: f64,
    /// Total operations (commits + aborts)
    pub operations: u64,
}

/// Builder for database configuration.
///
/// # Example
///
/// ```ignore
/// // Production: disk-backed with durability
/// let db = Strata::builder()
///     .path("./my-db")
///     .buffered()  // Default, good for production
///     .open()?;
///
/// // Integration testing: temp directory, no durability
/// let db = Strata::builder()
///     .no_durability()
///     .open_temp()?;
///
/// // Unit testing: truly ephemeral (no disk at all)
/// let db = Strata::ephemeral()?;
/// ```
pub struct StrataBuilder {
    inner: strata_engine::DatabaseBuilder,
}

impl StrataBuilder {
    /// Create a new builder with default settings.
    pub fn new() -> Self {
        Self {
            inner: strata_engine::DatabaseBuilder::new(),
        }
    }

    /// Set the database directory path.
    pub fn path(mut self, path: impl AsRef<Path>) -> Self {
        self.inner = self.inner.path(path.as_ref());
        self
    }

    /// Use no-durability mode (no WAL sync, files still created).
    ///
    /// This sets the WAL to skip fsync, providing fast writes.
    /// **Note**: Disk files are still created. For truly file-free operation,
    /// use [`Strata::ephemeral()`] instead.
    ///
    /// All data is lost on shutdown or crash.
    /// Use for integration testing where you want file isolation but not durability.
    pub fn no_durability(mut self) -> Self {
        self.inner = self.inner.no_durability();
        self
    }

    /// Deprecated: Use `no_durability()` instead.
    ///
    /// This method name was confusing because it only affects WAL sync behavior,
    /// not storage location. Files are still created on disk.
    ///
    /// For truly in-memory operation with no disk files, use [`Strata::ephemeral()`].
    #[deprecated(
        since = "0.14.0",
        note = "Use .no_durability() instead - this sets WAL mode, not storage location. For no disk files, use Strata::ephemeral()"
    )]
    pub fn in_memory(mut self) -> Self {
        #[allow(deprecated)]
        {
            self.inner = self.inner.in_memory();
        }
        self
    }

    /// Use buffered mode (default, recommended for production).
    ///
    /// Batches writes for performance while providing good durability.
    /// Default flush interval: 100ms or 1000 writes.
    pub fn buffered(mut self) -> Self {
        self.inner = self.inner.buffered();
        self
    }

    /// Use buffered mode with custom parameters.
    ///
    /// # Arguments
    ///
    /// * `flush_interval_ms` - Maximum time between fsyncs
    /// * `max_pending_writes` - Maximum writes before forced fsync
    pub fn buffered_with(mut self, flush_interval_ms: u64, max_pending_writes: usize) -> Self {
        self.inner = self.inner.buffered_with(flush_interval_ms, max_pending_writes);
        self
    }

    /// Use strict mode (safest, slowest).
    ///
    /// Syncs to disk on every commit. Zero data loss on crash.
    /// Use for critical data like audit logs or financial transactions.
    pub fn strict(mut self) -> Self {
        self.inner = self.inner.strict();
        self
    }

    /// Open the database.
    ///
    /// Uses the configured path, or a temp directory if none set.
    pub fn open(self) -> Result<Strata> {
        let db = Arc::new(self.inner.open().map_err(Error::from)?);
        Ok(Strata::from_engine(db))
    }

    /// Open a temporary database.
    ///
    /// Creates a unique temporary directory. Useful for testing.
    pub fn open_temp(self) -> Result<Strata> {
        let db = Arc::new(self.inner.open_temp().map_err(Error::from)?);
        Ok(Strata::from_engine(db))
    }
}

impl Default for StrataBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl Strata {
    /// Create Strata from an engine Database.
    fn from_engine(db: Arc<strata_engine::Database>) -> Self {
        Self {
            kv: KV::new(db.clone()),
            json: Json::new(db.clone()),
            events: Events::new(db.clone()),
            state: State::new(db.clone()),
            vectors: Vectors::new(db.clone()),
            runs: Runs::new(db.clone()),
            inner: db,
        }
    }
}
