//! Durability mode for WAL operations.
//!
//! Defines the durability guarantees for WAL writes.

/// Durability mode for WAL writes.
///
/// Controls when data is fsynced to disk and the trade-off between
/// performance and durability.
///
/// # Mode Comparison
///
/// | Mode | Latency Target | Use Case |
/// |------|----------------|----------|
/// | None | <3µs | Tests, caches, ephemeral data |
/// | Batched | <30µs | Production (balanced) |
/// | Strict | ~2ms | Checkpoints, audit logs |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurabilityMode {
    /// No durability - all data lost on crash.
    ///
    /// Bypasses WAL entirely. No fsync, no file I/O.
    /// Target latency: <3µs for engine/put_direct.
    /// Use case: Tests, caches, ephemeral data, development.
    None,

    /// fsync after every commit (slow, maximum durability).
    ///
    /// Use when data loss is unacceptable, even for a single write.
    /// Expect 10ms+ latency per write.
    Strict,

    /// fsync every N commits OR every T milliseconds.
    ///
    /// Good balance of speed and safety. May lose up to batch_size
    /// writes or interval_ms of data on crash.
    /// Target latency: <30µs.
    Batched {
        /// Maximum time between fsyncs in milliseconds
        interval_ms: u64,
        /// Maximum writes between fsyncs
        batch_size: usize,
    },
}

impl DurabilityMode {
    /// Check if this mode requires WAL persistence.
    ///
    /// Returns false for None mode, true for all others.
    pub fn requires_wal(&self) -> bool {
        !matches!(self, DurabilityMode::None)
    }

    /// Check if this mode requires immediate fsync on every commit.
    ///
    /// Returns true only for Strict mode.
    pub fn requires_immediate_fsync(&self) -> bool {
        matches!(self, DurabilityMode::Strict)
    }

    /// Human-readable description of the mode.
    pub fn description(&self) -> &'static str {
        match self {
            DurabilityMode::None => "No durability (fastest, all data lost on crash)",
            DurabilityMode::Strict => "Sync fsync (safest, slowest)",
            DurabilityMode::Batched { .. } => "Batched fsync (balanced speed/safety)",
        }
    }

    /// Create a buffered mode with recommended defaults.
    ///
    /// Returns `Batched { interval_ms: 100, batch_size: 1000 }`.
    ///
    /// # Default Values
    ///
    /// - **interval_ms**: 100 - Maximum 100ms between fsyncs
    /// - **batch_size**: 1000 - Maximum 1000 writes before fsync
    pub fn buffered_default() -> Self {
        DurabilityMode::Batched {
            interval_ms: 100,
            batch_size: 1000,
        }
    }
}

impl Default for DurabilityMode {
    fn default() -> Self {
        // Default: batched with 100ms interval or 1000 commits
        DurabilityMode::Batched {
            interval_ms: 100,
            batch_size: 1000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_none_mode() {
        let mode = DurabilityMode::None;
        assert!(!mode.requires_wal());
        assert!(!mode.requires_immediate_fsync());
    }

    #[test]
    fn test_strict_mode() {
        let mode = DurabilityMode::Strict;
        assert!(mode.requires_wal());
        assert!(mode.requires_immediate_fsync());
    }

    #[test]
    fn test_batched_mode() {
        let mode = DurabilityMode::Batched {
            interval_ms: 100,
            batch_size: 1000,
        };
        assert!(mode.requires_wal());
        assert!(!mode.requires_immediate_fsync());
    }

    #[test]
    fn test_default_is_batched() {
        let mode = DurabilityMode::default();
        assert!(matches!(mode, DurabilityMode::Batched { .. }));
    }

    #[test]
    fn test_buffered_default() {
        let mode = DurabilityMode::buffered_default();
        match mode {
            DurabilityMode::Batched {
                interval_ms,
                batch_size,
            } => {
                assert_eq!(interval_ms, 100);
                assert_eq!(batch_size, 1000);
            }
            _ => panic!("Expected Batched mode"),
        }
    }
}
