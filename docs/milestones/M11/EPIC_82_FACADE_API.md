# Epic 82: Facade API Implementation

**Goal**: Implement the Redis-like facade API targeting default run

**Dependencies**: Epic 80, Epic 81

---

## Scope

- KV operations (set, get, getv, mget, mset, delete, exists, exists_many, incr)
- JSON operations (json_set, json_get, json_getv, json_del, json_merge)
- Event operations (xadd, xrange, xlen)
- Vector operations (vset, vget, vdel)
- State/CAS operations (cas_set, cas_get)
- History operations (history, get_at, latest_version)
- Run operations (runs, use_run)
- Capability discovery
- Auto-commit semantics

---

## User Stories

| Story | Description | Priority |
|-------|-------------|----------|
| #566 | KV Operations | CRITICAL |
| #567 | JSON Operations | CRITICAL |
| #568 | Event Operations | CRITICAL |
| #569 | Vector Operations | CRITICAL |
| #570 | State Operations (CAS) | CRITICAL |
| #571 | History Operations | HIGH |
| #572 | Run Operations | HIGH |
| #573 | Capability Discovery | HIGH |
| #574 | Facade Auto-Commit Semantics | CRITICAL |

---

## Story #566: KV Operations

**File**: `crates/api/src/facade/kv.rs` (NEW)

**Deliverable**: All KV facade operations

### Implementation

```rust
use crate::error::StrataError;
use crate::value::Value;
use crate::contract::{Versioned, RunId, DEFAULT_RUN};

/// Facade API for KV operations
///
/// All operations target the default run and auto-commit.
pub trait KvFacade {
    /// Set a key-value pair
    ///
    /// Overwrites existing value if present.
    fn set(&self, key: &str, value: Value) -> Result<(), StrataError>;

    /// Get a value by key
    ///
    /// Returns None if key doesn't exist.
    fn get(&self, key: &str) -> Result<Option<Value>, StrataError>;

    /// Get a versioned value by key
    ///
    /// Escape hatch to access version information.
    fn getv(&self, key: &str) -> Result<Option<Versioned<Value>>, StrataError>;

    /// Get multiple values by keys
    ///
    /// Returns values in same order as keys. None for missing keys.
    fn mget(&self, keys: &[&str]) -> Result<Vec<Option<Value>>, StrataError>;

    /// Set multiple key-value pairs atomically
    ///
    /// All-or-nothing: if any entry fails validation, none are applied.
    fn mset(&self, entries: &[(&str, Value)]) -> Result<(), StrataError>;

    /// Delete one or more keys
    ///
    /// Returns count of keys that **existed** (were actually deleted).
    fn delete(&self, keys: &[&str]) -> Result<u64, StrataError>;

    /// Check if a key exists
    fn exists(&self, key: &str) -> Result<bool, StrataError>;

    /// Count how many keys exist
    fn exists_many(&self, keys: &[&str]) -> Result<u64, StrataError>;

    /// Atomically increment an integer value
    ///
    /// - If key doesn't exist, treats as 0 then increments
    /// - If key exists but is not Int, returns WrongType
    /// - If increment would overflow/underflow, returns Overflow
    fn incr(&self, key: &str, delta: i64) -> Result<i64, StrataError>;
}

/// Default implementation using substrate
pub struct KvFacadeImpl<S: Substrate> {
    substrate: S,
    run_id: RunId,
}

impl<S: Substrate> KvFacadeImpl<S> {
    pub fn new(substrate: S) -> Self {
        KvFacadeImpl {
            substrate,
            run_id: RunId::default_run(),
        }
    }

    pub fn with_run(substrate: S, run_id: RunId) -> Self {
        KvFacadeImpl { substrate, run_id }
    }
}

impl<S: Substrate> KvFacade for KvFacadeImpl<S> {
    fn set(&self, key: &str, value: Value) -> Result<(), StrataError> {
        // Desugar: begin(); kv_put(default, key, value); commit()
        let txn = self.substrate.begin(&self.run_id)?;
        self.substrate.kv_put(&txn, key, value)?;
        self.substrate.commit(txn)?;
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<Value>, StrataError> {
        // Desugar: kv_get(default, key).map(|v| v.value)
        Ok(self.substrate.kv_get(&self.run_id, key)?
            .map(|v| v.value))
    }

    fn getv(&self, key: &str) -> Result<Option<Versioned<Value>>, StrataError> {
        // Desugar: kv_get(default, key)
        self.substrate.kv_get(&self.run_id, key)
    }

    fn mget(&self, keys: &[&str]) -> Result<Vec<Option<Value>>, StrataError> {
        // Desugar: batch { kv_get(default, k) for k in keys }
        keys.iter()
            .map(|k| self.get(k))
            .collect()
    }

    fn mset(&self, entries: &[(&str, Value)]) -> Result<(), StrataError> {
        // Desugar: begin(); for (k,v): kv_put(default, k, v); commit()
        let txn = self.substrate.begin(&self.run_id)?;
        for (key, value) in entries {
            self.substrate.kv_put(&txn, key, value.clone())?;
        }
        self.substrate.commit(txn)?;
        Ok(())
    }

    fn delete(&self, keys: &[&str]) -> Result<u64, StrataError> {
        // Desugar: begin(); for k: kv_delete(default, k); commit() - returns count existed
        let txn = self.substrate.begin(&self.run_id)?;
        let mut count = 0u64;
        for key in keys {
            if self.substrate.kv_delete(&txn, key)? {
                count += 1;
            }
        }
        self.substrate.commit(txn)?;
        Ok(count)
    }

    fn exists(&self, key: &str) -> Result<bool, StrataError> {
        // Desugar: kv_get(default, key).is_some()
        Ok(self.substrate.kv_get(&self.run_id, key)?.is_some())
    }

    fn exists_many(&self, keys: &[&str]) -> Result<u64, StrataError> {
        // Desugar: keys.filter(|k| kv_get(default, k).is_some()).count()
        let mut count = 0u64;
        for key in keys {
            if self.exists(key)? {
                count += 1;
            }
        }
        Ok(count)
    }

    fn incr(&self, key: &str, delta: i64) -> Result<i64, StrataError> {
        // Desugar: kv_incr(default, key, delta) - atomic engine operation
        self.substrate.kv_incr(&self.run_id, key, delta)
    }
}
```

### Acceptance Criteria

- [ ] `set(key, value) -> ()` overwrites, creates new version internally
- [ ] `get(key) -> Option<Value>` returns latest value or None
- [ ] `getv(key) -> Option<Versioned<Value>>` escape hatch for version info
- [ ] `mget(keys) -> Vec<Option<Value>>` order preserved, None for missing
- [ ] `mset(entries) -> ()` atomic, all-or-nothing on validation failure
- [ ] `delete(keys) -> u64` count of keys that **existed**
- [ ] `exists(key) -> bool` human-friendly boolean
- [ ] `exists_many(keys) -> u64` count of keys that exist
- [ ] `incr(key, delta=1) -> i64` atomic increment, missing = 0

---

## Story #567: JSON Operations

**File**: `crates/api/src/facade/json.rs` (NEW)

**Deliverable**: All JSON facade operations

### Implementation

```rust
use crate::error::StrataError;
use crate::value::Value;
use crate::contract::{Versioned, RunId};

/// Facade API for JSON operations
pub trait JsonFacade {
    /// Set a value at a JSON path
    ///
    /// Creates document if key doesn't exist.
    /// Root must be Object.
    fn json_set(&self, key: &str, path: &str, value: Value) -> Result<(), StrataError>;

    /// Get a value at a JSON path
    fn json_get(&self, key: &str, path: &str) -> Result<Option<Value>, StrataError>;

    /// Get a versioned value at a JSON path
    ///
    /// Returns **document-level** version, not subpath version.
    fn json_getv(&self, key: &str, path: &str) -> Result<Option<Versioned<Value>>, StrataError>;

    /// Delete a value at a JSON path
    ///
    /// Returns count of elements removed.
    fn json_del(&self, key: &str, path: &str) -> Result<u64, StrataError>;

    /// Merge a value at a JSON path (RFC 7396)
    ///
    /// - null deletes a field
    /// - Objects merge recursively
    /// - Arrays replace (not merge)
    /// - Scalars replace
    fn json_merge(&self, key: &str, path: &str, value: Value) -> Result<(), StrataError>;
}

/// Path syntax:
/// - `$` = root (entire document)
/// - `$.a.b` = object field access
/// - `$.items[0]` = array index
/// - `$.items[-]` = array append (json_set only)
/// - Negative indices `[-1]` NOT supported -> InvalidPath

impl<S: Substrate> JsonFacade for FacadeImpl<S> {
    fn json_set(&self, key: &str, path: &str, value: Value) -> Result<(), StrataError> {
        // Desugar: begin(); json_set(default, key, path, value); commit()
        let txn = self.substrate.begin(&self.run_id)?;
        self.substrate.json_set(&txn, key, path, value)?;
        self.substrate.commit(txn)?;
        Ok(())
    }

    fn json_get(&self, key: &str, path: &str) -> Result<Option<Value>, StrataError> {
        // Desugar: json_get(default, key, path).map(|v| v.value)
        Ok(self.substrate.json_get(&self.run_id, key, path)?
            .map(|v| v.value))
    }

    fn json_getv(&self, key: &str, path: &str) -> Result<Option<Versioned<Value>>, StrataError> {
        // Desugar: json_get(default, key, path) - document-level version
        self.substrate.json_get(&self.run_id, key, path)
    }

    fn json_del(&self, key: &str, path: &str) -> Result<u64, StrataError> {
        // Desugar: begin(); json_delete(default, key, path); commit()
        let txn = self.substrate.begin(&self.run_id)?;
        let count = self.substrate.json_delete(&txn, key, path)?;
        self.substrate.commit(txn)?;
        Ok(count)
    }

    fn json_merge(&self, key: &str, path: &str, value: Value) -> Result<(), StrataError> {
        // Desugar: begin(); json_merge(default, key, path, value); commit()
        let txn = self.substrate.begin(&self.run_id)?;
        self.substrate.json_merge(&txn, key, path, value)?;
        self.substrate.commit(txn)?;
        Ok(())
    }
}
```

### Acceptance Criteria

- [ ] Path syntax: `$`, `$.a.b`, `$.items[0]`, `$.items[-]`
- [ ] Negative indices `[-1]` return `InvalidPath`
- [ ] Deleting root `$` is forbidden
- [ ] `json_getv` returns document-level version
- [ ] `json_merge` follows RFC 7396 semantics

---

## Story #568: Event Operations

**File**: `crates/api/src/facade/event.rs` (NEW)

**Deliverable**: All Event facade operations

### Implementation

```rust
use crate::error::StrataError;
use crate::value::Value;
use crate::contract::{Versioned, Version, RunId};

/// Facade API for Event operations
pub trait EventFacade {
    /// Append an event to a stream
    ///
    /// Returns the event's sequence version.
    fn xadd(&self, stream: &str, payload: Value) -> Result<Version, StrataError>;

    /// Read events from a stream
    fn xrange(
        &self,
        stream: &str,
        start: Option<Version>,
        end: Option<Version>,
        limit: Option<u64>,
    ) -> Result<Vec<Versioned<Value>>, StrataError>;

    /// Count events in a stream
    fn xlen(&self, stream: &str) -> Result<u64, StrataError>;
}

impl<S: Substrate> EventFacade for FacadeImpl<S> {
    fn xadd(&self, stream: &str, payload: Value) -> Result<Version, StrataError> {
        // Desugar: event_append(default, stream, payload)
        // Payload must be Object
        if !matches!(payload, Value::Object(_)) {
            // Actually, empty object {} is allowed, and bytes in payloads via $bytes
            // The constraint is on the top level being an Object
        }
        self.substrate.event_append(&self.run_id, stream, payload)
    }

    fn xrange(
        &self,
        stream: &str,
        start: Option<Version>,
        end: Option<Version>,
        limit: Option<u64>,
    ) -> Result<Vec<Versioned<Value>>, StrataError> {
        // Desugar: event_range(default, stream, start, end, limit)
        self.substrate.event_range(&self.run_id, stream, start, end, limit)
    }

    fn xlen(&self, stream: &str) -> Result<u64, StrataError> {
        // Desugar: event_range(default, stream, None, None, None).len()
        let events = self.xrange(stream, None, None, None)?;
        Ok(events.len() as u64)
    }
}
```

### Acceptance Criteria

- [ ] `xadd(stream, payload: Object) -> Version` returns sequence type
- [ ] `xrange(stream, start?, end?, limit?) -> Vec<Versioned<Value>>`
- [ ] `xlen(stream) -> u64` count of events
- [ ] Empty object `{}` is allowed as payload
- [ ] Bytes allowed in payloads (via `$bytes` wrapper)

---

## Story #569: Vector Operations

**File**: `crates/api/src/facade/vector.rs` (NEW)

**Deliverable**: All Vector facade operations

### Implementation

```rust
use crate::error::StrataError;
use crate::value::Value;
use crate::contract::{Versioned, RunId};

/// Vector entry structure
#[derive(Debug, Clone)]
pub struct VectorEntry {
    pub vector: Vec<f32>,
    pub metadata: Value,
}

/// Facade API for Vector operations
pub trait VectorFacade {
    /// Set a vector with metadata
    fn vset(&self, key: &str, vector: Vec<f32>, metadata: Value) -> Result<(), StrataError>;

    /// Get a vector with metadata
    fn vget(&self, key: &str) -> Result<Option<Versioned<VectorEntry>>, StrataError>;

    /// Delete a vector
    fn vdel(&self, key: &str) -> Result<bool, StrataError>;
}

impl<S: Substrate> VectorFacade for FacadeImpl<S> {
    fn vset(&self, key: &str, vector: Vec<f32>, metadata: Value) -> Result<(), StrataError> {
        // Desugar: begin(); vector_set(default, key, vector, metadata); commit()
        let txn = self.substrate.begin(&self.run_id)?;
        self.substrate.vector_set(&txn, key, vector, metadata)?;
        self.substrate.commit(txn)?;
        Ok(())
    }

    fn vget(&self, key: &str) -> Result<Option<Versioned<VectorEntry>>, StrataError> {
        // Desugar: vector_get(default, key)
        self.substrate.vector_get(&self.run_id, key)
    }

    fn vdel(&self, key: &str) -> Result<bool, StrataError> {
        // Desugar: begin(); vector_delete(default, key); commit()
        let txn = self.substrate.begin(&self.run_id)?;
        let deleted = self.substrate.vector_delete(&txn, key)?;
        self.substrate.commit(txn)?;
        Ok(deleted)
    }
}
```

### Acceptance Criteria

- [ ] `vset(key, vector, metadata) -> ()` stores vector with metadata
- [ ] `vget(key) -> Option<Versioned<VectorEntry>>` returns Versioned
- [ ] `vdel(key) -> bool` returns true if deleted
- [ ] Dimension rules: 1 to max_vector_dim (8192)
- [ ] Dimension mismatch returns `ConstraintViolation`

---

## Story #570: State Operations (CAS)

**File**: `crates/api/src/facade/state.rs` (NEW)

**Deliverable**: CAS facade operations

### Implementation

```rust
use crate::error::StrataError;
use crate::value::Value;
use crate::contract::RunId;

/// Facade API for State (CAS) operations
pub trait StateFacade {
    /// Compare-and-swap set
    ///
    /// - expected = None: only set if key is missing (create-if-not-exists)
    /// - expected = Some(Value::Null): only set if current value is null
    /// - Type matters: Int(1) != Float(1.0)
    fn cas_set(&self, key: &str, expected: Option<Value>, new: Value) -> Result<bool, StrataError>;

    /// Get value from state store
    fn cas_get(&self, key: &str) -> Result<Option<Value>, StrataError>;
}

impl<S: Substrate> StateFacade for FacadeImpl<S> {
    fn cas_set(&self, key: &str, expected: Option<Value>, new: Value) -> Result<bool, StrataError> {
        // Desugar: state_cas(default, key, expected, new)
        self.substrate.state_cas(&self.run_id, key, expected, new)
    }

    fn cas_get(&self, key: &str) -> Result<Option<Value>, StrataError> {
        // Desugar: state_get(default, key).map(|v| v.value)
        Ok(self.substrate.state_get(&self.run_id, key)?
            .map(|v| v.value))
    }
}
```

### Acceptance Criteria

- [ ] `cas_set(key, expected, new) -> bool` returns true if swap succeeded
- [ ] `cas_get(key) -> Option<Value>` returns current value
- [ ] `expected = None` = create-if-not-exists
- [ ] `expected = Some(Value::Null)` = only set if current value is null
- [ ] Type matters: `Int(1) != Float(1.0)` in comparison

---

## Story #571: History Operations

**File**: `crates/api/src/facade/history.rs` (NEW)

**Deliverable**: History facade operations

### Implementation

```rust
use crate::error::StrataError;
use crate::value::Value;
use crate::contract::{Versioned, Version, RunId};

/// Facade API for History operations
///
/// Note: Facade history is KV-only. Other primitives use specific APIs.
pub trait HistoryFacade {
    /// Get version history for a key
    ///
    /// Returns newest first (descending by version).
    fn history(
        &self,
        key: &str,
        limit: Option<u64>,
        before: Option<Version>,
    ) -> Result<Vec<Versioned<Value>>, StrataError>;

    /// Get value at a specific version
    fn get_at(&self, key: &str, version: Version) -> Result<Value, StrataError>;

    /// Get latest version for a key
    fn latest_version(&self, key: &str) -> Result<Option<Version>, StrataError>;
}

impl<S: Substrate> HistoryFacade for FacadeImpl<S> {
    fn history(
        &self,
        key: &str,
        limit: Option<u64>,
        before: Option<Version>,
    ) -> Result<Vec<Versioned<Value>>, StrataError> {
        // Desugar: kv_history(default, key, limit, before)
        self.substrate.kv_history(&self.run_id, key, limit, before)
    }

    fn get_at(&self, key: &str, version: Version) -> Result<Value, StrataError> {
        // Desugar: kv_get_at(default, key, version)
        let versioned = self.substrate.kv_get_at(&self.run_id, key, version)?;
        Ok(versioned.value)
    }

    fn latest_version(&self, key: &str) -> Result<Option<Version>, StrataError> {
        // Desugar: kv_get(default, key).map(|v| v.version)
        Ok(self.substrate.kv_get(&self.run_id, key)?
            .map(|v| v.version))
    }
}
```

### Acceptance Criteria

- [ ] `history(key, limit?, before?) -> Vec<Versioned<Value>>` newest first
- [ ] `get_at(key, version) -> Value | HistoryTrimmed`
- [ ] `latest_version(key) -> Option<Version>`
- [ ] Facade history is KV-only

---

## Story #572: Run Operations

**File**: `crates/api/src/facade/run.rs` (NEW)

**Deliverable**: Run facade operations

### Implementation

```rust
use crate::error::StrataError;
use crate::contract::{RunId, RunInfo};

/// Facade API for Run operations
pub trait RunFacade {
    /// List all runs
    fn runs(&self) -> Result<Vec<RunInfo>, StrataError>;

    /// Scope operations to a specific run
    ///
    /// Returns NotFound if run doesn't exist (no lazy creation).
    fn use_run(&self, run_id: &str) -> Result<ScopedFacade, StrataError>;
}

/// A facade scoped to a specific run
pub struct ScopedFacade<S: Substrate> {
    substrate: S,
    run_id: RunId,
}

impl<S: Substrate> ScopedFacade<S> {
    // All KV, JSON, Event, Vector, State, History operations
    // with the scoped run_id instead of default
}

impl<S: Substrate> RunFacade for FacadeImpl<S> {
    fn runs(&self) -> Result<Vec<RunInfo>, StrataError> {
        // Desugar: run_list()
        self.substrate.run_list()
    }

    fn use_run(&self, run_id: &str) -> Result<ScopedFacade<S>, StrataError> {
        // Desugar: Returns facade with default = run_id (client-side binding)
        let parsed = RunId::parse(run_id)
            .map_err(|_| ErrorFactory::run_not_found(run_id))?;

        // Verify run exists
        if self.substrate.run_get(&parsed)?.is_none() {
            return Err(ErrorFactory::run_not_found(run_id));
        }

        Ok(ScopedFacade {
            substrate: self.substrate.clone(),
            run_id: parsed,
        })
    }
}
```

### Acceptance Criteria

- [ ] `runs() -> Vec<RunInfo>` lists all runs
- [ ] `use_run(run_id) -> ScopedFacade` scopes operations to run
- [ ] `use_run` returns `NotFound` if run doesn't exist
- [ ] No lazy creation of runs
- [ ] Run lifecycle (create/close) is substrate-only

---

## Story #573: Capability Discovery

**File**: `crates/api/src/facade/capabilities.rs` (NEW)

**Deliverable**: Capability discovery operation

### Implementation

```rust
use serde::{Serialize, Deserialize};
use crate::value::ValueLimits;

/// System capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    /// API version
    pub version: String,

    /// Available operations
    pub operations: Vec<String>,

    /// Configured limits
    pub limits: CapabilityLimits,

    /// Supported encodings
    pub encodings: Vec<String>,

    /// Enabled features
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityLimits {
    pub max_key_bytes: usize,
    pub max_string_bytes: usize,
    pub max_bytes_len: usize,
    pub max_value_bytes_encoded: usize,
    pub max_array_len: usize,
    pub max_object_entries: usize,
    pub max_nesting_depth: usize,
    pub max_vector_dim: usize,
}

impl From<&ValueLimits> for CapabilityLimits {
    fn from(limits: &ValueLimits) -> Self {
        CapabilityLimits {
            max_key_bytes: limits.max_key_bytes,
            max_string_bytes: limits.max_string_bytes,
            max_bytes_len: limits.max_bytes_len,
            max_value_bytes_encoded: limits.max_value_bytes_encoded,
            max_array_len: limits.max_array_len,
            max_object_entries: limits.max_object_entries,
            max_nesting_depth: limits.max_nesting_depth,
            max_vector_dim: limits.max_vector_dim,
        }
    }
}

impl Capabilities {
    pub fn new(limits: &ValueLimits) -> Self {
        Capabilities {
            version: "1.0.0".into(),
            operations: vec![
                "kv.set", "kv.get", "kv.getv", "kv.mget", "kv.mset",
                "kv.delete", "kv.exists", "kv.exists_many", "kv.incr",
                "json.set", "json.get", "json.getv", "json.del", "json.merge",
                "event.add", "event.range", "event.len",
                "vector.set", "vector.get", "vector.del",
                "state.cas_set", "state.get",
                "history.list", "history.get_at", "history.latest_version",
                "run.list", "run.use",
                "system.capabilities",
            ].into_iter().map(String::from).collect(),
            limits: CapabilityLimits::from(limits),
            encodings: vec!["json".into()],
            features: vec!["history".into(), "retention".into(), "cas".into()],
        }
    }
}

/// Facade API for system operations
pub trait SystemFacade {
    /// Get system capabilities
    fn capabilities(&self) -> Capabilities;
}
```

### Acceptance Criteria

- [ ] `capabilities() -> Capabilities` returns system info
- [ ] Includes version, operations, limits, encodings, features
- [ ] All operations listed
- [ ] Current limits reflected

---

## Story #574: Facade Auto-Commit Semantics

**File**: `crates/api/src/facade/mod.rs`

**Deliverable**: Auto-commit behavior for all facade operations

### Design

Every facade operation auto-commits. This means:
1. Each call is atomic
2. No explicit transaction management needed
3. Failures are immediate

### Implementation

All implementations above follow the pattern:
```rust
fn operation(&self, ...) -> Result<_, StrataError> {
    let txn = self.substrate.begin(&self.run_id)?;
    // ... perform operation ...
    self.substrate.commit(txn)?;
    Ok(result)
}
```

### Acceptance Criteria

- [ ] All write operations auto-commit
- [ ] Each call is atomic
- [ ] Failures roll back automatically
- [ ] No transaction handle exposed

---

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_get_roundtrip() {
        let db = setup_test_db();
        db.set("key", Value::Int(42)).unwrap();
        let value = db.get("key").unwrap();
        assert_eq!(value, Some(Value::Int(42)));
    }

    #[test]
    fn test_mset_atomicity() {
        let db = setup_test_db();

        // Should fail atomically if any key is invalid
        let result = db.mset(&[
            ("valid", Value::Int(1)),
            ("_strata/invalid", Value::Int(2)), // Reserved prefix
        ]);

        assert!(result.is_err());
        // Neither key should exist
        assert_eq!(db.get("valid").unwrap(), None);
    }

    #[test]
    fn test_incr_missing_key() {
        let db = setup_test_db();
        let result = db.incr("counter", 1).unwrap();
        assert_eq!(result, 1); // 0 + 1 = 1
    }

    #[test]
    fn test_incr_wrong_type() {
        let db = setup_test_db();
        db.set("key", Value::String("not a number".into())).unwrap();

        let result = db.incr("key", 1);
        assert!(matches!(result, Err(StrataError::WrongType { .. })));
    }

    #[test]
    fn test_use_run_not_found() {
        let db = setup_test_db();
        let result = db.use_run("nonexistent-run-id");
        assert!(matches!(result, Err(StrataError::NotFound { .. })));
    }

    #[test]
    fn test_json_getv_document_version() {
        let db = setup_test_db();
        db.json_set("doc", "$", json!({"a": 1})).unwrap();
        db.json_set("doc", "$.b", json!(2)).unwrap();

        let v1 = db.json_getv("doc", "$.a").unwrap().unwrap();
        let v2 = db.json_getv("doc", "$.b").unwrap().unwrap();

        // Both return document-level version
        assert_eq!(v1.version, v2.version);
    }
}
```

---

## Files Modified/Created

| File | Action |
|------|--------|
| `crates/api/src/lib.rs` | CREATE - API crate entry |
| `crates/api/src/facade/mod.rs` | CREATE - Facade module |
| `crates/api/src/facade/kv.rs` | CREATE - KV facade |
| `crates/api/src/facade/json.rs` | CREATE - JSON facade |
| `crates/api/src/facade/event.rs` | CREATE - Event facade |
| `crates/api/src/facade/vector.rs` | CREATE - Vector facade |
| `crates/api/src/facade/state.rs` | CREATE - State facade |
| `crates/api/src/facade/history.rs` | CREATE - History facade |
| `crates/api/src/facade/run.rs` | CREATE - Run facade |
| `crates/api/src/facade/capabilities.rs` | CREATE - Capabilities |
| `Cargo.toml` | MODIFY - Add api crate |
