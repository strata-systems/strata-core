//! Run Index primitive implementation
//!
//! First-class run lifecycle management with metadata, status tracking,
//! and run relationships.
//!
//! ## Design
//!
//! RunIndex is a stateless facade over the Database engine. It provides:
//! - Run lifecycle: create, get, update_status, complete, fail
//! - Status transition validation (no resurrection, archived is terminal)
//! - Query operations with filters (by status, tag, time, parent)
//! - Cascading delete and soft archive
//!
//! ## Status Transitions
//!
//! Valid transitions are enforced:
//! - Active → Completed, Failed, Cancelled, Paused, Archived
//! - Paused → Active, Cancelled, Archived
//! - Completed → Archived
//! - Failed → Archived
//! - Cancelled → Archived
//!
//! Invalid transitions (will error):
//! - Completed → Active (no resurrection)
//! - Failed → Active (no resurrection)
//! - Archived → * (terminal state)
//!
//! ## Implementation Status
//!
//! TODO: Implement in Epic 18 (Stories #191-#196)

// Placeholder - implementation coming in Epic 18
