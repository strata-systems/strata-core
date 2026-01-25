//! RunIndex Comprehensive Test Suite
//!
//! Tests organized by functionality:
//! - basic_ops: CRUD operations (create, get, exists, update_metadata)
//! - lifecycle: State transitions (close, pause, resume, fail, cancel, archive)
//! - queries: List, query, search, count
//! - tags: Tag management (add, remove, get)
//! - hierarchy: Parent-child relationships
//! - retention: Retention policy
//! - delete: Cascading delete
//! - edge_cases: Validation and boundaries
//! - concurrency: Thread safety
//! - invariants: Contract invariants that must always hold

mod basic_ops;
mod lifecycle;
mod queries;
mod tags;
mod hierarchy;
mod retention;
mod delete;
mod edge_cases;
mod concurrency;
mod invariants;
