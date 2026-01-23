//! KVStore Test Suite
//!
//! Comprehensive tests for the KVStore substrate API.
//!
//! ## Modules
//!
//! - `basic_ops`: CRUD operations (put, get, delete, exists)
//! - `value_types`: All 8 value types with edge cases
//! - `batch_ops`: Batch operations (mget, mput, mdelete)
//! - `atomic_ops`: Atomic operations (incr, cas_value, cas_version)
//! - `durability`: Durability modes and crash recovery
//! - `concurrency`: Multi-threaded isolation and safety
//! - `transactions`: Transaction semantics and conflict detection
//! - `scan_ops`: Key enumeration and scanning (NOT YET IMPLEMENTED)
//! - `edge_cases`: Key validation, value limits, edge cases
//! - `recovery_invariants`: Recovery guarantees (R1-R6)

pub mod atomic_ops;
pub mod basic_ops;
pub mod batch_ops;
pub mod concurrency;
pub mod durability;
pub mod edge_cases;
pub mod recovery_invariants;
pub mod scan_ops;
pub mod transactions;
pub mod value_types;
