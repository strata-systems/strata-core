//! # Strata
//!
//! Production-grade embedded database for AI agents.
//!
//! Strata provides a unified API for storing and querying agent state across
//! multiple primitives: key-value, JSON documents, event streams, state cells,
//! and vector embeddings.
//!
//! ## Quick Start
//!
//! ```ignore
//! use strata::prelude::*;
//!
//! // Open a database
//! let db = Strata::open("./my-db")?;
//!
//! // Key-value operations
//! db.kv.set("user:1", "Alice")?;
//! let name = db.kv.get("user:1")?;
//!
//! // JSON documents
//! db.json.set("profile", serde_json::json!({"name": "Alice"}))?;
//!
//! // Event streams
//! db.events.append("activity", serde_json::json!({"action": "login"}))?;
//!
//! // Graceful shutdown
//! db.close()?;
//! ```
//!
//! ## Progressive Disclosure
//!
//! Strata's API follows a progressive disclosure pattern:
//!
//! 1. **Simple** - Default run, no version info: `db.kv.set("key", value)`
//! 2. **Run-scoped** - Explicit run: `db.kv.set_in(&run, "key", value)`
//! 3. **Full control** - Returns version: `db.kv.put(&run, "key", value)`
//!
//! ## Primitives
//!
//! - [`KV`] - Key-value store for simple data
//! - [`Json`] - JSON documents with path-level operations
//! - [`Events`] - Append-only event streams
//! - [`State`] - State cells with CAS operations
//! - [`Vectors`] - Vector embeddings with similarity search
//! - [`Runs`] - Run lifecycle management

#![warn(missing_docs)]

mod database;
mod error;
mod primitives;
mod types;

pub mod prelude;

// Re-export main entry points
pub use database::{Strata, StrataBuilder};
pub use error::{Error, Result};

// Re-export primitives
pub use primitives::{Events, Json, Runs, State, Vectors, KV};

// Re-export types
pub use types::*;
