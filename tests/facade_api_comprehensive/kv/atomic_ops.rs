//! Atomic KV Facade Operations Tests
//!
//! Tests for incr, incrby, decr, decrby operations.

use crate::*;

// =============================================================================
// INCR TESTS
// =============================================================================

#[test]
fn test_incr_creates_key_with_1() {
    let facade = create_facade();

    let result = facade.incr("counter").unwrap();
    assert_eq!(result, 1);

    let value = facade.get("counter").unwrap();
    assert_eq!(value, Some(Value::Int(1)));
}

#[test]
fn test_incr_increments_existing() {
    let facade = create_facade();

    facade.set("counter", Value::Int(10)).unwrap();
    let result = facade.incr("counter").unwrap();

    assert_eq!(result, 11);
}

#[test]
fn test_incr_multiple_times() {
    let facade = create_facade();

    assert_eq!(facade.incr("counter").unwrap(), 1);
    assert_eq!(facade.incr("counter").unwrap(), 2);
    assert_eq!(facade.incr("counter").unwrap(), 3);
    assert_eq!(facade.incr("counter").unwrap(), 4);
    assert_eq!(facade.incr("counter").unwrap(), 5);
}

#[test]
fn test_incr_wrong_type_fails() {
    let facade = create_facade();

    facade.set("not_int", Value::String("hello".into())).unwrap();
    let result = facade.incr("not_int");

    assert!(result.is_err(), "Should fail on non-integer value");
}

// =============================================================================
// INCRBY TESTS
// =============================================================================

#[test]
fn test_incrby_creates_key() {
    let facade = create_facade();

    let result = facade.incrby("counter", 5).unwrap();
    assert_eq!(result, 5);
}

#[test]
fn test_incrby_positive_delta() {
    let facade = create_facade();

    facade.set("counter", Value::Int(10)).unwrap();
    let result = facade.incrby("counter", 7).unwrap();

    assert_eq!(result, 17);
}

#[test]
fn test_incrby_negative_delta() {
    let facade = create_facade();

    facade.set("counter", Value::Int(10)).unwrap();
    let result = facade.incrby("counter", -3).unwrap();

    assert_eq!(result, 7);
}

#[test]
fn test_incrby_zero_delta() {
    let facade = create_facade();

    facade.set("counter", Value::Int(10)).unwrap();
    let result = facade.incrby("counter", 0).unwrap();

    assert_eq!(result, 10);
}

#[test]
fn test_incrby_large_delta() {
    let facade = create_facade();

    facade.set("counter", Value::Int(0)).unwrap();
    let result = facade.incrby("counter", 1_000_000_000).unwrap();

    assert_eq!(result, 1_000_000_000);
}

// =============================================================================
// DECR TESTS
// =============================================================================

#[test]
fn test_decr_creates_key_with_minus_1() {
    let facade = create_facade();

    let result = facade.decr("counter").unwrap();
    assert_eq!(result, -1);
}

#[test]
fn test_decr_decrements_existing() {
    let facade = create_facade();

    facade.set("counter", Value::Int(10)).unwrap();
    let result = facade.decr("counter").unwrap();

    assert_eq!(result, 9);
}

#[test]
fn test_decr_goes_negative() {
    let facade = create_facade();

    facade.set("counter", Value::Int(0)).unwrap();
    let result = facade.decr("counter").unwrap();

    assert_eq!(result, -1);
}

// =============================================================================
// DECRBY TESTS
// =============================================================================

#[test]
fn test_decrby_positive_delta() {
    let facade = create_facade();

    facade.set("counter", Value::Int(10)).unwrap();
    let result = facade.decrby("counter", 3).unwrap();

    assert_eq!(result, 7);
}

#[test]
fn test_decrby_is_incrby_negated() {
    let facade = create_facade();

    // decrby(key, 5) == incrby(key, -5)
    facade.set("a", Value::Int(10)).unwrap();
    facade.set("b", Value::Int(10)).unwrap();

    let result_a = facade.decrby("a", 3).unwrap();
    let result_b = facade.incrby("b", -3).unwrap();

    assert_eq!(result_a, result_b);
}

// =============================================================================
// OVERFLOW TESTS
// =============================================================================

#[test]
fn test_incr_overflow_returns_error() {
    let facade = create_facade();

    facade.set("max", Value::Int(i64::MAX)).unwrap();
    let result = facade.incr("max");

    assert!(result.is_err(), "Should error on overflow");
}

#[test]
fn test_decr_underflow_returns_error() {
    let facade = create_facade();

    facade.set("min", Value::Int(i64::MIN)).unwrap();
    let result = facade.decr("min");

    assert!(result.is_err(), "Should error on underflow");
}

// =============================================================================
// CONCURRENT INCREMENT TESTS
// =============================================================================

#[test]
fn test_incr_concurrent_isolation() {
    use std::sync::Arc;
    use std::thread;

    let db = create_inmemory_db();
    let substrate = Arc::new(SubstrateImpl::new(db));

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let substrate = Arc::clone(&substrate);
            thread::spawn(move || {
                let facade = FacadeImpl::new(substrate);
                for _ in 0..100 {
                    facade.incr("shared_counter").unwrap();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let facade = FacadeImpl::new(substrate);
    let final_value = facade.get("shared_counter").unwrap().unwrap();

    assert_eq!(final_value, Value::Int(1000), "Should be 10 threads * 100 increments");
}
