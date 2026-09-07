//! #3189: the spellings `KvKey::new` and `KvValue::new` accept.
//!
//! Filed as "the stratadb rustdoc greeting example does not compile", on the
//! reasoning that "`String` implements `Into<Vec<u8>>`; `&str` does not". That
//! is not so — std carries `impl From<&str> for Vec<u8>`, so `&str` converts
//! and the documented example has always compiled. The signature has been
//! `impl Into<Vec<u8>>` since the V1 promotion, so it was never bytes-only.
//!
//! The report was still worth something: two downstream projects independently
//! concluded the API was bytes-only by reading the signature rather than trying
//! it. That is a real discoverability problem even though it is not a bug, and
//! it means a future "tightening" of these constructors — to `AsRef<[u8]>`, or
//! to a narrower bound — would break the documented welcome mat while every
//! existing call site still compiled.
//!
//! So this pins the accepted set. It is a contract test, not a bug regression.

use strata_engine::{KvKey, KvValue};

/// Every spelling a caller might reasonably reach for. If one of these stops
/// compiling, the rustdoc greeting example and the README are wrong again.
#[test]
fn kv_keys_accept_strings_and_bytes_alike() {
    let owned = String::from("owned");
    let bytes: Vec<u8> = b"vec".to_vec();
    let slice: &[u8] = b"slice";

    // The form the published rustdoc and README use.
    KvKey::new("greeting").expect("&str literal — the documented form");

    KvKey::new(owned.clone()).expect("String");
    KvKey::new(owned.as_str()).expect("&str borrowed from a String");
    KvKey::new(format!("generated-{}", 1)).expect("format! output");
    KvKey::new(bytes.clone()).expect("Vec<u8>");
    KvKey::new(slice).expect("&[u8]");
    KvKey::new(b"literal").expect("&[u8; N] byte-string literal");
}

/// The value side takes the same set. The README writes
/// `KvValue::new(b"hello".to_vec())`, which is the verbose spelling of
/// something that accepts `"hello"` directly — worth pinning so the shorter
/// form stays available to the facade work (#3137).
#[test]
fn kv_values_accept_strings_and_bytes_alike() {
    // Same bytes in every spelling, so the equality below is meaningful.
    let owned = String::from("hello");

    // `KvValue::new` is infallible and `#[must_use]`, so bind each result.
    let from_literal = KvValue::new("hello");
    let from_string = KvValue::new(owned.clone());
    let from_str = KvValue::new(owned.as_str());
    let from_vec = KvValue::new(b"hello".to_vec());
    let from_array = KvValue::new(b"hello");

    // All five spell the same bytes.
    for made in [from_string, from_str, from_vec, from_array] {
        assert_eq!(
            made.as_bytes(),
            from_literal.as_bytes(),
            "the accepted spellings must produce identical values"
        );
    }
}

/// The one thing that is genuinely rejected, and must stay rejected: an empty
/// key. #3189 asked for the ergonomic widening to keep this, and there is no
/// widening here — but the pin belongs beside the others so the refusal is not
/// lost if these constructors are ever reworked.
#[test]
fn an_empty_key_is_still_refused() {
    let error = KvKey::new("").expect_err("an empty key must be refused");
    assert_eq!(error.status().code(), "invalid_argument.engine.kv_key");

    // And the refusal is about emptiness, not about the input's type.
    let empty_bytes: &[u8] = b"";
    KvKey::new(empty_bytes).expect_err("an empty byte slice is equally refused");
}
