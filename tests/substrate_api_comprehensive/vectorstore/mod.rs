//! VectorStore Comprehensive Test Suite
//!
//! Tests organized by functionality:
//! - basic_ops: CRUD operations (upsert, get, delete)
//! - collections: Collection management (create, drop, info, list)
//! - search: Similarity search and filtering
//! - durability: Persistence across restarts
//! - concurrency: Thread safety
//! - edge_cases: Validation and boundary conditions

mod basic_ops;
mod collections;
mod search;
mod durability;
mod concurrency;
mod edge_cases;
