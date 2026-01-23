//! StateCell Test Suite
//!
//! Comprehensive tests for the StateCell substrate API.
//!
//! ## Modules
//!
//! - `basic_ops`: Basic operations (get, set, delete, exists)
//! - `cas_ops`: Compare-and-swap operations
//! - `durability`: Durability modes and crash recovery
//! - `concurrency`: Multi-threaded safety and contention
//! - `edge_cases`: Validation, constraints, cell names

pub mod basic_ops;
pub mod cas_ops;
pub mod concurrency;
pub mod durability;
pub mod edge_cases;
