//! Durability modes for performance optimization
//!
//! This module provides the durability abstraction layer that enables
//! trading off latency vs durability guarantees.
//!
//! # Durability Modes
//!
//! | Mode | WAL | fsync | Target Latency | Data Loss Window |
//! |------|-----|-------|----------------|------------------|
//! | InMemory | None | None | <3µs | All (on crash) |
//! | Buffered | Append | Periodic | <30µs | Bounded |
//! | Strict | Append | Every write | ~2ms | Zero |
//!
//! # Usage with Database
//!
//! The Database uses [`DurabilityMode`] to configure WAL behavior. Durability
//! is handled internally by the WAL - no separate handler instantiation is needed.
//!
//! ```ignore
//! use strata_engine::Database;
//!
//! // Ephemeral mode for testing (no disk files)
//! let db = Database::ephemeral()?;
//!
//! // Simple open with buffered durability (default)
//! let db = Database::open("/data/mydb")?;
//!
//! // Strict mode for maximum durability
//! let db = Database::builder()
//!     .path("/data/mydb")
//!     .strict()
//!     .open()?;
//! ```
//!
//! # Architecture
//!
//! ## Database Integration (Primary Path)
//!
//! The Database stores a `DurabilityMode` enum and passes it to the WAL during
//! initialization. The WAL internally handles fsync timing based on the mode:
//!
//! - **InMemory**: WAL is bypassed entirely (`requires_wal() == false`)
//! - **Batched**: WAL appends without fsync; fsync triggers on batch_size or interval
//! - **Async**: WAL spawns a background thread for periodic fsync
//! - **Strict**: WAL performs fsync after every append
//!
//! This design keeps the Database simple - it just stores the mode and delegates
//! durability decisions to the WAL layer.
//!
//! ## Durability Trait (Reference Implementations)
//!
//! This module also provides standalone durability implementations for direct use:
//!
//! - [`InMemoryDurability`]: No persistence
//! - [`BufferedDurability`]: Background thread flush (use [`BufferedDurability::threaded()`])
//! - [`StrictDurability`]: Immediate fsync
//!
//! These implement the [`Durability`] trait and can be used directly if you need
//! fine-grained control over durability behavior outside of the Database context.
//!
//! ```text
//! commit_transaction():
//!   ┌─────────────────┐
//!   │   Validate OCC  │
//!   └────────┬────────┘
//!            │
//!   ┌────────▼────────┐
//!   │ Allocate Version│
//!   └────────┬────────┘
//!            │
//!   ┌────────▼────────┐
//!   │  WAL.append()   │  ← DurabilityMode controls fsync
//!   └────────┬────────┘
//!            │
//!   ┌────────▼────────┐
//!   │  Apply Storage  │
//!   └────────┬────────┘
//!            │
//!   ┌────────▼────────┐
//!   │ Mark Committed  │
//!   └─────────────────┘
//! ```

mod buffered;
mod inmemory;
mod strict;
mod traits;

pub use buffered::BufferedDurability;
pub use inmemory::InMemoryDurability;
pub use strict::StrictDurability;
pub use traits::{CommitData, Durability, DurabilityExt};

// Re-export DurabilityMode from durability crate for convenience
pub use strata_durability::wal::DurabilityMode;
