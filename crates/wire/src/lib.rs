//! Wire encoding for Strata
//!
//! This crate implements the wire encoding contract for Strata values.
//! It provides JSON encoding with special wrappers for non-JSON-native values:
//!
//! - `$bytes`: Base64 encoding for `Value::Bytes`
//! - `$f64`: Special float wrapper for NaN, ±Inf, -0.0
//! - `$absent`: CAS expected-missing marker
//!
//! ## Wire Encoding Rules
//!
//! | Value Type | JSON Encoding |
//! |------------|--------------|
//! | Null | `null` |
//! | Bool | `true`/`false` |
//! | Int | number |
//! | Float (normal) | number |
//! | Float (special) | `{"$f64": "..."}` |
//! | String | `"..."` |
//! | Bytes | `{"$bytes": "..."}` |
//! | Array | `[...]` |
//! | Object | `{...}` |
//!
//! ## Examples
//!
//! ```
//! use strata_wire::{encode_json, decode_json};
//! use strata_core::Value;
//!
//! // Encode a value
//! let value = Value::Int(42);
//! let json = encode_json(&value);
//! assert_eq!(json, "42");
//!
//! // Decode a value
//! let decoded = decode_json("42").unwrap();
//! assert_eq!(decoded, Value::Int(42));
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod json;

// Re-export main types
pub use json::{
    decode_json, decode_request, decode_response, decode_version, decode_versioned, encode_absent,
    encode_json, encode_request, encode_response, encode_string, encode_version, encode_versioned,
    is_absent, parse_json_object, ApiError, DecodeError, Request, RequestParams, Response, Version,
    Versioned,
};
