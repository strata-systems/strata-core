//! Access control and configuration for Strata database.
//!
//! This crate provides the [`AccessMode`] and [`OpenOptions`] types used to
//! control how a database is opened and what operations are permitted.

use serde::{Deserialize, Serialize};

/// Controls whether the database allows writes or is read-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessMode {
    ReadWrite,
    ReadOnly,
}

impl Default for AccessMode {
    fn default() -> Self {
        AccessMode::ReadWrite
    }
}

/// Options for opening a database.
///
/// Use the builder pattern to configure options:
///
/// ```ignore
/// use strata_security::{OpenOptions, AccessMode};
///
/// let opts = OpenOptions::new().access_mode(AccessMode::ReadOnly);
/// ```
#[derive(Debug, Clone)]
pub struct OpenOptions {
    pub access_mode: AccessMode,
}

impl OpenOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn access_mode(mut self, mode: AccessMode) -> Self {
        self.access_mode = mode;
        self
    }
}

impl Default for OpenOptions {
    fn default() -> Self {
        Self {
            access_mode: AccessMode::ReadWrite,
        }
    }
}
