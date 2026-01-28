//! Tokenizer — re-exported from strata_engine::primitives::tokenizer
//!
//! The tokenizer has been moved to the engine crate so that primitives
//! can use it on write paths. This module re-exports for backward compatibility.

pub use strata_engine::primitives::tokenizer::{tokenize, tokenize_unique};
