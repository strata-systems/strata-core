//! Transaction management for M4 performance and M9 TransactionOps
//!
//! This module provides:
//! - Thread-local transaction pooling (zero allocations after warmup)
//! - Pool management utilities
//! - Transaction wrapper implementing TransactionOps (M9)
//!
//! # Architecture
//!
//! The pool uses thread-local storage to avoid synchronization overhead:
//! - Each thread has its own pool of up to 8 TransactionContext objects
//! - Contexts are reset (not reallocated) when reused
//! - HashMap/HashSet capacity is preserved across reuse
//!
//! # M9 TransactionOps
//!
//! The Transaction type wraps TransactionContext and implements the
//! TransactionOps trait for unified primitive access within transactions.

pub mod context;
pub mod pool;

pub use context::Transaction;
pub use pool::{TransactionPool, MAX_POOL_SIZE};
