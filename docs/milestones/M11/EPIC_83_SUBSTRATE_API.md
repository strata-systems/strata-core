# Epic 83: Substrate API Implementation

**Goal**: Implement the explicit substrate API with run/version/txn access

**Dependencies**: Epic 80, Epic 81

---

## Scope

- KVStore substrate operations
- JsonStore substrate operations
- EventLog substrate operations
- StateCell substrate operations
- VectorStore substrate operations
- TraceStore substrate operations
- RunIndex substrate operations
- Transaction control (begin, commit, rollback)
- Retention operations
- Core types (RunId, RunInfo, RunState, RetentionPolicy)

---

## User Stories

| Story | Description | Priority |
|-------|-------------|----------|
| #575 | KVStore Substrate Operations | CRITICAL |
| #576 | JsonStore Substrate Operations | CRITICAL |
| #577 | EventLog Substrate Operations | CRITICAL |
| #578 | StateCell Substrate Operations | CRITICAL |
| #579 | VectorStore Substrate Operations | CRITICAL |
| #580 | TraceStore Substrate Operations | HIGH |
| #581 | RunIndex Substrate Operations | CRITICAL |
| #582 | Transaction Control | CRITICAL |
| #583 | Retention Operations | HIGH |
| #584 | Core Types | FOUNDATION |

---

## Story #575: KVStore Substrate Operations

**File**: `crates/api/src/substrate/kv.rs` (NEW)

**Deliverable**: All KVStore substrate operations with explicit run parameter

### Implementation

```rust
use crate::error::StrataError;
use crate::value::Value;
use crate::contract::{Versioned, Version, RunId};

/// Substrate API for KV operations
///
/// All operations require explicit run_id.
pub trait KvSubstrate {
    /// Put a value
    fn kv_put(&self, txn: &Txn, key: &str, value: Value) -> Result<Version, StrataError>;

    /// Get a value (returns Versioned)
    fn kv_get(&self, run: &RunId, key: &str) -> Result<Option<Versioned<Value>>, StrataError>;

    /// Get a value at a specific version
    fn kv_get_at(&self, run: &RunId, key: &str, version: Version) -> Result<Versioned<Value>, StrataError>;

    /// Delete a key
    fn kv_delete(&self, txn: &Txn, key: &str) -> Result<bool, StrataError>;

    /// Check if key exists
    fn kv_exists(&self, run: &RunId, key: &str) -> Result<bool, StrataError>;

    /// Get version history
    fn kv_history(
        &self,
        run: &RunId,
        key: &str,
        limit: Option<u64>,
        before: Option<Version>,
    ) -> Result<Vec<Versioned<Value>>, StrataError>;

    /// Atomic increment
    fn kv_incr(&self, run: &RunId, key: &str, delta: i64) -> Result<i64, StrataError>;

    /// CAS by version
    fn kv_cas_version(
        &self,
        run: &RunId,
        key: &str,
        expected_version: Version,
        new_value: Value,
    ) -> Result<bool, StrataError>;

    /// CAS by value
    fn kv_cas_value(
        &self,
        run: &RunId,
        key: &str,
        expected_value: Option<Value>,
        new_value: Value,
    ) -> Result<bool, StrataError>;
}
```

### Acceptance Criteria

- [ ] All operations require explicit `run_id`
- [ ] `kv_put` returns Version
- [ ] `kv_get` returns `Option<Versioned<Value>>`
- [ ] `kv_get_at` returns Versioned or HistoryTrimmed error
- [ ] `kv_delete` returns bool (whether key existed)
- [ ] `kv_history` returns newest first
- [ ] `kv_incr` is atomic engine operation
- [ ] `kv_cas_version` and `kv_cas_value` for CAS patterns

---

## Story #576: JsonStore Substrate Operations

**File**: `crates/api/src/substrate/json.rs` (NEW)

**Deliverable**: All JsonStore substrate operations

### Implementation

```rust
use crate::error::StrataError;
use crate::value::Value;
use crate::contract::{Versioned, Version, RunId};

/// Substrate API for JSON operations
pub trait JsonSubstrate {
    /// Set a value at path
    fn json_set(&self, txn: &Txn, key: &str, path: &str, value: Value) -> Result<Version, StrataError>;

    /// Get a value at path (returns document-level version)
    fn json_get(&self, run: &RunId, key: &str, path: &str) -> Result<Option<Versioned<Value>>, StrataError>;

    /// Delete at path
    fn json_delete(&self, txn: &Txn, key: &str, path: &str) -> Result<u64, StrataError>;

    /// Merge at path (RFC 7396)
    fn json_merge(&self, txn: &Txn, key: &str, path: &str, value: Value) -> Result<Version, StrataError>;

    /// Get document version history
    fn json_history(
        &self,
        run: &RunId,
        key: &str,
        limit: Option<u64>,
        before: Option<Version>,
    ) -> Result<Vec<Versioned<Value>>, StrataError>;
}
```

### Acceptance Criteria

- [ ] `json_set` returns Version
- [ ] `json_get` returns document-level version
- [ ] `json_delete` returns count of elements removed
- [ ] `json_merge` follows RFC 7396
- [ ] `json_history` available at substrate level

---

## Story #577: EventLog Substrate Operations

**File**: `crates/api/src/substrate/event.rs` (NEW)

**Deliverable**: All EventLog substrate operations

### Implementation

```rust
use crate::error::StrataError;
use crate::value::Value;
use crate::contract::{Versioned, Version, RunId};

/// Substrate API for Event operations
pub trait EventSubstrate {
    /// Append event to stream
    fn event_append(&self, run: &RunId, stream: &str, payload: Value) -> Result<Version, StrataError>;

    /// Read events from stream
    fn event_range(
        &self,
        run: &RunId,
        stream: &str,
        start: Option<Version>,
        end: Option<Version>,
        limit: Option<u64>,
    ) -> Result<Vec<Versioned<Value>>, StrataError>;
}
```

### Acceptance Criteria

- [ ] `event_append` returns Sequence version
- [ ] `event_range` supports start/end/limit parameters
- [ ] Events are append-only

---

## Story #578: StateCell Substrate Operations

**File**: `crates/api/src/substrate/state.rs` (NEW)

**Deliverable**: All StateCell substrate operations

### Implementation

```rust
use crate::error::StrataError;
use crate::value::Value;
use crate::contract::{Versioned, Version, RunId};

/// Substrate API for StateCell operations
pub trait StateSubstrate {
    /// Get state value
    fn state_get(&self, run: &RunId, key: &str) -> Result<Option<Versioned<Value>>, StrataError>;

    /// Set state value
    fn state_set(&self, run: &RunId, key: &str, value: Value) -> Result<Version, StrataError>;

    /// CAS on state
    fn state_cas(&self, run: &RunId, key: &str, expected: Option<Value>, new: Value) -> Result<bool, StrataError>;

    /// State history
    fn state_history(
        &self,
        run: &RunId,
        key: &str,
        limit: Option<u64>,
        before: Option<Version>,
    ) -> Result<Vec<Versioned<Value>>, StrataError>;
}
```

### Acceptance Criteria

- [ ] `state_get` returns Versioned with Counter version type
- [ ] `state_set` returns Counter version
- [ ] `state_cas` supports None for create-if-not-exists
- [ ] `state_history` available at substrate level

---

## Story #579: VectorStore Substrate Operations

**File**: `crates/api/src/substrate/vector.rs` (NEW)

**Deliverable**: All VectorStore substrate operations

### Implementation

```rust
use crate::error::StrataError;
use crate::value::Value;
use crate::contract::{Versioned, Version, RunId};
use crate::facade::VectorEntry;

/// Substrate API for Vector operations
pub trait VectorSubstrate {
    /// Set vector with metadata
    fn vector_set(
        &self,
        txn: &Txn,
        key: &str,
        vector: Vec<f32>,
        metadata: Value,
    ) -> Result<Version, StrataError>;

    /// Get vector with metadata
    fn vector_get(&self, run: &RunId, key: &str) -> Result<Option<Versioned<VectorEntry>>, StrataError>;

    /// Delete vector
    fn vector_delete(&self, txn: &Txn, key: &str) -> Result<bool, StrataError>;

    /// Vector history
    fn vector_history(
        &self,
        run: &RunId,
        key: &str,
        limit: Option<u64>,
        before: Option<Version>,
    ) -> Result<Vec<Versioned<Value>>, StrataError>;
}
```

### Acceptance Criteria

- [ ] `vector_set` returns Version
- [ ] `vector_get` returns Versioned<VectorEntry>
- [ ] `vector_delete` returns bool
- [ ] `vector_history` available at substrate level
- [ ] Dimension validation enforced

---

## Story #580: TraceStore Substrate Operations

**File**: `crates/api/src/substrate/trace.rs` (NEW)

**Deliverable**: TraceStore substrate operations (substrate-only)

### Implementation

```rust
use crate::error::StrataError;
use crate::value::Value;
use crate::contract::{Versioned, Version, RunId};

/// Substrate API for Trace operations (substrate-only)
pub trait TraceSubstrate {
    /// Record a trace entry
    fn trace_record(
        &self,
        run: &RunId,
        trace_type: &str,
        payload: Value,
    ) -> Result<Version, StrataError>;

    /// Get trace entry by ID
    fn trace_get(&self, run: &RunId, id: Version) -> Result<Option<Versioned<Value>>, StrataError>;

    /// Read trace entries
    fn trace_range(
        &self,
        run: &RunId,
        start: Option<Version>,
        end: Option<Version>,
        limit: Option<u64>,
    ) -> Result<Vec<Versioned<Value>>, StrataError>;
}
```

### Acceptance Criteria

- [ ] `trace_record` returns Version
- [ ] `trace_get` retrieves by ID
- [ ] `trace_range` supports pagination
- [ ] TraceStore is substrate-only (no facade)

---

## Story #581: RunIndex Substrate Operations

**File**: `crates/api/src/substrate/run.rs` (NEW)

**Deliverable**: Run lifecycle operations

### Implementation

```rust
use crate::error::StrataError;
use crate::value::Value;
use crate::contract::{RunId, RunInfo, RunState};

/// Run information
#[derive(Debug, Clone)]
pub struct RunInfo {
    pub run_id: RunId,
    pub created_at: u64,  // microseconds
    pub metadata: Value,
    pub state: RunState,
}

/// Run state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    Active,
    Closed,
}

/// Substrate API for Run operations
pub trait RunSubstrate {
    /// Create a new run
    fn run_create(&self, metadata: Value) -> Result<RunId, StrataError>;

    /// Get run info
    fn run_get(&self, run: &RunId) -> Result<Option<RunInfo>, StrataError>;

    /// List all runs
    fn run_list(&self) -> Result<Vec<RunInfo>, StrataError>;

    /// Close a run (marks as closed, no deletion)
    fn run_close(&self, run: &RunId) -> Result<(), StrataError>;
}
```

### Acceptance Criteria

- [ ] `run_create(metadata) -> RunId` returns UUID format
- [ ] `run_get(run_id) -> Option<RunInfo>`
- [ ] `run_list() -> Vec<RunInfo>`
- [ ] `run_close(run_id) -> ()` marks run as closed
- [ ] Default run (`"default"`) cannot be closed
- [ ] Default run created lazily on first write or on DB open
- [ ] No run deletion in M11

---

## Story #582: Transaction Control

**File**: `crates/api/src/substrate/transaction.rs` (NEW)

**Deliverable**: Transaction control operations

### Implementation

```rust
use crate::error::StrataError;
use crate::contract::RunId;

/// Transaction handle
pub struct Txn {
    id: u64,
    run_id: RunId,
    state: TxnState,
}

enum TxnState {
    Active,
    Committed,
    RolledBack,
}

/// Substrate API for transactions
pub trait TransactionSubstrate {
    /// Begin a new transaction
    ///
    /// Transactions are scoped to a single run.
    fn begin(&self, run: &RunId) -> Result<Txn, StrataError>;

    /// Commit a transaction
    ///
    /// Uses OCC validation at commit time.
    fn commit(&self, txn: Txn) -> Result<(), StrataError>;

    /// Rollback a transaction
    fn rollback(&self, txn: Txn) -> Result<(), StrataError>;
}

impl Txn {
    /// Check if transaction is still active
    pub fn is_active(&self) -> bool {
        matches!(self.state, TxnState::Active)
    }

    /// Get the run this transaction is scoped to
    pub fn run_id(&self) -> &RunId {
        &self.run_id
    }
}
```

### Acceptance Criteria

- [ ] `begin(run_id) -> Txn`
- [ ] `commit(txn) -> ()`
- [ ] `rollback(txn) -> ()`
- [ ] Transactions scoped to single run
- [ ] Snapshot isolation (OCC validation at commit)
- [ ] Using stale/committed transaction handle returns Conflict

---

## Story #583: Retention Operations

**File**: `crates/api/src/substrate/retention.rs` (NEW)

**Deliverable**: Retention policy operations

### Implementation

```rust
use std::time::Duration;
use crate::error::StrataError;
use crate::contract::{Versioned, Version, RunId};

/// Retention policy
#[derive(Debug, Clone)]
pub enum RetentionPolicy {
    /// Keep all versions (default)
    KeepAll,

    /// Keep N most recent versions
    KeepLast(u64),

    /// Keep versions within time window
    KeepFor(Duration),

    /// Union of multiple policies
    Composite(Vec<RetentionPolicy>),
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        RetentionPolicy::KeepAll
    }
}

/// Substrate API for retention
pub trait RetentionSubstrate {
    /// Get retention policy for a run
    fn retention_get(&self, run: &RunId) -> Result<Option<Versioned<RetentionPolicy>>, StrataError>;

    /// Set retention policy for a run
    fn retention_set(&self, run: &RunId, policy: RetentionPolicy) -> Result<Version, StrataError>;
}
```

### Acceptance Criteria

- [ ] `retention_get(run_id) -> Option<Versioned<RetentionPolicy>>`
- [ ] `retention_set(run_id, policy) -> Version`
- [ ] Default policy is `KeepAll`
- [ ] Per-key retention NOT supported in M11
- [ ] Retention is per-run

---

## Story #584: Core Types

**File**: `crates/core/src/contract/types.rs`

**Deliverable**: Core type definitions

### Implementation

(Included across Stories #553, #554, #555, and #581)

Core types:
- `RunId` - Run identifier (UUID or "default")
- `RunInfo` - Run metadata
- `RunState` - Active or Closed
- `Version` - Tagged union (Txn/Sequence/Counter)
- `Versioned<T>` - Value with version and timestamp
- `RetentionPolicy` - Retention configuration

### Acceptance Criteria

- [ ] All core types implemented
- [ ] Types are used consistently across substrate
- [ ] Wire encoding defined for all types

---

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substrate_explicit_run() {
        let db = setup_test_db();
        let run = RunId::default_run();

        let txn = db.begin(&run).unwrap();
        db.kv_put(&txn, "key", Value::Int(42)).unwrap();
        db.commit(txn).unwrap();

        let versioned = db.kv_get(&run, "key").unwrap().unwrap();
        assert_eq!(versioned.value, Value::Int(42));
        assert!(matches!(versioned.version, Version::Txn(_)));
    }

    #[test]
    fn test_transaction_scoping() {
        let db = setup_test_db();

        let run1 = db.run_create(Value::Null).unwrap();
        let run2 = db.run_create(Value::Null).unwrap();

        let txn1 = db.begin(&run1).unwrap();
        db.kv_put(&txn1, "key", Value::Int(1)).unwrap();
        db.commit(txn1).unwrap();

        // Different run doesn't see the key
        assert!(db.kv_get(&run2, "key").unwrap().is_none());
    }

    #[test]
    fn test_stale_transaction() {
        let db = setup_test_db();
        let run = RunId::default_run();

        let txn = db.begin(&run).unwrap();
        db.commit(txn.clone()).unwrap();

        // Using committed transaction should fail
        let result = db.kv_put(&txn, "key", Value::Int(1));
        assert!(matches!(result, Err(StrataError::Conflict { .. })));
    }

    #[test]
    fn test_default_run_cannot_close() {
        let db = setup_test_db();
        let result = db.run_close(&RunId::default_run());
        assert!(result.is_err()); // Cannot close default run
    }

    #[test]
    fn test_version_types_by_primitive() {
        let db = setup_test_db();
        let run = RunId::default_run();

        // KV uses Txn version
        let txn = db.begin(&run).unwrap();
        db.kv_put(&txn, "key", Value::Int(1)).unwrap();
        db.commit(txn).unwrap();

        let kv_versioned = db.kv_get(&run, "key").unwrap().unwrap();
        assert!(matches!(kv_versioned.version, Version::Txn(_)));

        // Events use Sequence version
        let event_version = db.event_append(&run, "stream", json!({})).unwrap();
        assert!(matches!(event_version, Version::Sequence(_)));

        // State uses Counter version
        db.state_set(&run, "state_key", Value::Int(1)).unwrap();
        let state_versioned = db.state_get(&run, "state_key").unwrap().unwrap();
        assert!(matches!(state_versioned.version, Version::Counter(_)));
    }
}
```

---

## Files Modified/Created

| File | Action |
|------|--------|
| `crates/api/src/substrate/mod.rs` | CREATE - Substrate module |
| `crates/api/src/substrate/kv.rs` | CREATE - KVStore substrate |
| `crates/api/src/substrate/json.rs` | CREATE - JsonStore substrate |
| `crates/api/src/substrate/event.rs` | CREATE - EventLog substrate |
| `crates/api/src/substrate/state.rs` | CREATE - StateCell substrate |
| `crates/api/src/substrate/vector.rs` | CREATE - VectorStore substrate |
| `crates/api/src/substrate/trace.rs` | CREATE - TraceStore substrate |
| `crates/api/src/substrate/run.rs` | CREATE - RunIndex substrate |
| `crates/api/src/substrate/transaction.rs` | CREATE - Transaction control |
| `crates/api/src/substrate/retention.rs` | CREATE - Retention substrate |
