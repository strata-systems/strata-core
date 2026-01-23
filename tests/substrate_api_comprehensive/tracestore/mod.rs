//! TraceStore Comprehensive Test Suite
//!
//! Tests for structured reasoning trace storage:
//! - Basic CRUD operations
//! - Hierarchical parent-child relationships
//! - Query operations (by type, tag, time)
//! - Search operations
//! - Durability and persistence
//! - Concurrency
//! - Edge cases and validation

mod basic_ops;
mod hierarchy;
mod queries;
mod durability;
mod concurrency;
mod edge_cases;
