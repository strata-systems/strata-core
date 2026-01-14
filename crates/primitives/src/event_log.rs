//! Event Log primitive implementation
//!
//! Immutable, append-only event stream for capturing agent actions,
//! observations, and state changes with causal hash chaining.
//!
//! ## Design
//!
//! EventLog is a stateless facade over the Database engine. It provides:
//! - Append-only semantics (no update or delete)
//! - Automatic sequence number assignment
//! - Causal hash chaining for tamper-evidence
//! - Single-writer-ordered per run (CAS on metadata key)
//!
//! ## Important Design Decisions
//!
//! 1. **Single-writer-ordered**: All appends are serialized through CAS on
//!    the metadata key. Parallel append is NOT supported by design.
//!
//! 2. **Causal hash chaining**: The hash chain provides tamper-evidence
//!    within the process boundary but is NOT cryptographically secure.
//!
//! ## Implementation Status
//!
//! TODO: Implement in Epic 15 (Stories #174-#179)

// Placeholder - implementation coming in Epic 15
