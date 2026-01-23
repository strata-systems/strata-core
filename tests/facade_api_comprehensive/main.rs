//! Facade API Comprehensive Test Suite
//!
//! This test suite verifies that the Facade API correctly desugars to Substrate
//! API calls. The facade provides Redis-like simplicity while substrate provides
//! full Strata power.
//!
//! ## Key Verification Points
//!
//! 1. Facade only accesses Substrate (never primitives/storage directly)
//! 2. Default run targeting works correctly
//! 3. Auto-commit semantics are correct
//! 4. Return types are appropriately simplified
//!
//! ## Running Tests
//!
//! ```bash
//! # Run all facade tests
//! cargo test --test facade_api_comprehensive
//!
//! # Run KV facade tests only
//! cargo test --test facade_api_comprehensive kv::
//! ```

use std::sync::Arc;

use strata_api::facade::{FacadeImpl, KVFacade, KVFacadeBatch};
use strata_api::substrate::{ApiRunId, KVStore, KVStoreBatch, SubstrateImpl};
use strata_core::Value;
use strata_engine::Database;

// Test modules
pub mod kv;

// =============================================================================
// SHARED TEST UTILITIES
// =============================================================================

/// Create an in-memory test database
pub fn create_inmemory_db() -> Arc<Database> {
    Arc::new(
        Database::builder()
            .in_memory()
            .open_temp()
            .expect("Failed to create in-memory database"),
    )
}

/// Create a facade for testing
pub fn create_facade() -> FacadeImpl {
    let db = create_inmemory_db();
    let substrate = Arc::new(SubstrateImpl::new(db));
    FacadeImpl::new(substrate)
}

/// Create both facade and substrate for comparison testing
pub fn create_facade_and_substrate() -> (FacadeImpl, Arc<SubstrateImpl>) {
    let db = create_inmemory_db();
    let substrate = Arc::new(SubstrateImpl::new(db));
    let facade = FacadeImpl::new(Arc::clone(&substrate));
    (facade, substrate)
}

/// Get the default run ID (used by facade)
pub fn default_run() -> ApiRunId {
    ApiRunId::default()
}

/// Standard test values covering common types
pub fn standard_test_values() -> Vec<(&'static str, Value)> {
    vec![
        ("null", Value::Null),
        ("bool_true", Value::Bool(true)),
        ("bool_false", Value::Bool(false)),
        ("int_pos", Value::Int(42)),
        ("int_neg", Value::Int(-42)),
        ("int_zero", Value::Int(0)),
        ("float_pos", Value::Float(3.14159)),
        ("float_neg", Value::Float(-2.71828)),
        ("string", Value::String("hello world".into())),
        ("string_unicode", Value::String("日本語 🌍".into())),
        ("string_empty", Value::String("".into())),
        ("bytes", Value::Bytes(vec![0x00, 0x01, 0xFF, 0xFE])),
        ("bytes_empty", Value::Bytes(vec![])),
        (
            "array",
            Value::Array(vec![Value::Int(1), Value::String("two".into())]),
        ),
        ("object", {
            let mut m = std::collections::HashMap::new();
            m.insert("nested".to_string(), Value::Int(123));
            Value::Object(m)
        }),
    ]
}
