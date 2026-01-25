//! Primitive wrappers for the unified API.
//!
//! Each primitive provides a clean interface following the progressive disclosure pattern:
//!
//! 1. **Simple** - Default run, no version info: `db.kv.set("key", value)`
//! 2. **Run-scoped** - Explicit run: `db.kv.set_in(&run, "key", value)`
//! 3. **Full control** - Returns version: `db.kv.put(&run, "key", value)`

mod events;
mod json;
mod kv;
mod runs;
mod state;
mod vectors;

pub use events::Events;
pub use json::Json;
pub use kv::KV;
pub use runs::Runs;
pub use state::State;
pub use vectors::Vectors;
