//! JsonStore Comprehensive Test Suite
//!
//! Tests organized by functionality:
//! - basic_ops: CRUD operations (set, get, delete, exists)
//! - path_ops: Path navigation and nested operations
//! - merge_ops: JSON merge patch (RFC 7396)
//! - durability: Persistence across restarts
//! - concurrency: Thread safety
//! - edge_cases: Validation and boundary conditions

mod basic_ops;
mod path_ops;
mod merge_ops;
mod durability;
mod concurrency;
mod edge_cases;
