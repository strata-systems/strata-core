//! Audit test for issue #928: Storage error source chain discarded in convert.rs
//! Verdict: PARTIALLY FIXED
//!
//! The `From<StrataError> for Error` conversion now concatenates the source error's
//! Display message into the reason string (e.g., "failed to write: access denied"),
//! preserving the diagnostic information. However, the actual `std::error::Error`
//! source chain is still lost because `Error::Io { reason: String }` has no
//! `#[source]` field — this is an architectural choice since `Error` must be
//! `Clone + Serialize + Deserialize`, and `std::io::Error` is neither.

/// Documents the architectural constraint: Error::Io has no source chain,
/// but the error message string preserves the diagnostic information.
#[test]
fn issue_928_error_io_variant_has_no_source_chain() {
    let err = strata_executor::Error::Io {
        reason: "disk full".into(),
    };

    // The error implements std::error::Error
    let std_err: &dyn std::error::Error = &err;

    // source() returns None because there is no chained error object
    assert!(
        std_err.source().is_none(),
        "Error::Io has no source chain — the original error object is not preserved"
    );

    // The only information preserved is the message string
    assert_eq!(err.to_string(), "I/O error: disk full");
}

/// Confirms that StrataError::Storage conversion now includes the source message.
#[test]
fn issue_928_strata_storage_error_source_message_preserved() {
    use strata_core::StrataError;

    // Create a StrataError::Storage with both message and source
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
    let strata_err = StrataError::Storage {
        message: "failed to write".into(),
        source: Some(Box::new(io_err)),
    };

    // Convert to executor Error
    let exec_err: strata_executor::Error = strata_err.into();

    // The source error's message is now concatenated into the reason string
    match exec_err {
        strata_executor::Error::Io { reason } => {
            assert_eq!(reason, "failed to write: access denied");
        }
        other => panic!("Expected Io variant, got: {:?}", other),
    }
}

/// Confirms that conversion without a source still works.
#[test]
fn issue_928_strata_storage_error_without_source() {
    use strata_core::StrataError;

    let strata_err = StrataError::Storage {
        message: "disk full".into(),
        source: None,
    };

    let exec_err: strata_executor::Error = strata_err.into();

    match exec_err {
        strata_executor::Error::Io { reason } => {
            assert_eq!(reason, "disk full");
        }
        other => panic!("Expected Io variant, got: {:?}", other),
    }
}
