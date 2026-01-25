//! VectorStore Comprehensive Test Suite
//!
//! Tests organized by functionality:
//! - basic_ops: CRUD operations (upsert, get, delete)
//! - batch: Batch operations (upsert_batch, get_batch, delete_batch)
//! - collections: Collection management (create, drop, info, list)
//! - search: Similarity search and filtering
//! - durability: Persistence across restarts
//! - concurrency: Thread safety
//! - edge_cases: Validation and boundary conditions
//! - history: Version history and point-in-time retrieval

mod basic_ops;
mod batch;
mod collections;
mod search;
mod durability;
mod concurrency;
mod edge_cases;
mod history;
