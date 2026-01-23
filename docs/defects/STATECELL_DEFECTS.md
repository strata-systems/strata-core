# StateCell Defects and Gaps

> Consolidated from architecture review, primitive vs substrate analysis, and coordination primitive best practices.
> Source: `crates/api/src/substrate/state.rs` and `crates/primitives/src/state_cell.rs`

## Summary

| Category | Count | Priority |
|----------|-------|----------|
| Critical Missing Features | 2 | P0 |
| Stubbed/Unimplemented | 1 | P0 |
| Missing Table Stakes APIs | 3 | P0 |
| Missing Important APIs | 2 | P1 |
| API Design Issues | 2 | P1 |
| **Total Issues** | **10** | |

---

## What is StateCell?

StateCell is a **coordination primitive** for single-value state machines, NOT a general key-value store.

**Purpose:** Atomic state transitions with compare-and-swap semantics
- Locks and mutexes
- Leader election
- Distributed barriers
- State machine coordination
- Configuration with atomic updates

**vs KVStore:**
| Aspect | StateCell | KVStore |
|--------|-----------|---------|
| Model | Single named cell | Multiple key-value pairs |
| Versioning | Counter (1, 2, 3...) | Transaction version |
| Core Pattern | CAS + transitions | Read/write/delete |
| Use Case | Coordination | Storage |

---

## Current Substrate API (6 methods)

```rust
// What exists today
fn state_set(run, cell, value) -> Version;
fn state_get(run, cell) -> Option<Versioned<Value>>;
fn state_cas(run, cell, expected_counter, value) -> Option<Version>;
fn state_delete(run, cell) -> bool;
fn state_exists(run, cell) -> bool;
fn state_history(run, cell, limit, before) -> Vec<Versioned<Value>>;  // STUBBED
```

---

## Part 1: Critical Missing Features (P0)

### Gap 1: `state_transition` - Atomic State Machine Transitions

**Priority:** P0 - This IS the core feature of StateCell

**What Primitive Has:**
```rust
// In crates/primitives/src/state_cell.rs
fn transition<F, T>(&self, run_id: &RunId, name: &str, f: F) -> Result<(T, Versioned<u64>)>
where
    F: Fn(&State) -> (T, Value);

fn transition_or_init<F, T>(&self, run_id: &RunId, name: &str, initial: Value, f: F)
    -> Result<(T, Versioned<u64>)>
where
    F: Fn(&State) -> (T, Value);
```

**What Substrate Exposes:** Nothing

**Why Critical:**
- Transition closures are THE defining feature of StateCell
- Provides atomic read-modify-write with automatic OCC retry (200 retries)
- Without this, users must manually implement retry loops
- Without this, StateCell is just a worse KVStore

**Example Use Case - Distributed Counter:**
```rust
// What users SHOULD be able to do:
let (old_count, version) = substrate.state_transition(&run, "counter", |state| {
    let count = state.value.as_i64().unwrap_or(0);
    (count, Value::Int(count + 1))
}).unwrap();

// What users MUST do today (manual retry loop):
loop {
    let current = substrate.state_get(&run, "counter")?;
    let (old_val, expected_ver) = match current {
        Some(v) => (v.value.as_i64().unwrap_or(0), Some(v.version.as_counter())),
        None => (0, None),
    };
    match substrate.state_cas(&run, "counter", expected_ver, Value::Int(old_val + 1))? {
        Some(_) => break,
        None => continue,  // Retry on conflict
    }
}
```

**Proposed Substrate API:**
```rust
fn state_transition<F, T>(&self, run: &ApiRunId, cell: &str, f: F)
    -> StrataResult<(T, Version)>
where
    F: Fn(Option<&Value>) -> (T, Value) + Send + Sync;

fn state_transition_or_init<F, T>(&self, run: &ApiRunId, cell: &str, initial: Value, f: F)
    -> StrataResult<(T, Version)>
where
    F: Fn(&Value) -> (T, Value) + Send + Sync;
```

**Current Workaround:** Manual CAS retry loop (error-prone, verbose)

---

### Gap 2: `state_list` - List All Cells

**Priority:** P0 - Required for administration

**What Primitive Has:**
```rust
fn list(&self, run_id: &RunId) -> Result<Vec<String>>;
```

**What Substrate Exposes:** Nothing

**Why Critical:**
- Cannot discover what cells exist in a run
- Required for:
  - Admin/debugging tools
  - Cleanup operations
  - State enumeration
  - Monitoring dashboards

**Proposed Substrate API:**
```rust
fn state_list(&self, run: &ApiRunId) -> StrataResult<Vec<String>>;
```

**Current Workaround:** None - must know cell names in advance

---

## Part 2: Stubbed/Unimplemented (P0)

### Gap 3: `state_history` - Version History (Stubbed)

**Priority:** P0 - API exists but doesn't work

**Current State:**
```rust
// In substrate trait - method EXISTS
fn state_history(&self, run: &ApiRunId, cell: &str,
                 limit: Option<u64>, before: Option<Version>)
    -> StrataResult<Vec<Versioned<Value>>>;

// In implementation - RETURNS EMPTY
fn state_history(...) -> StrataResult<Vec<Versioned<Value>>> {
    Ok(vec![])  // Not implemented
}
```

**Why Critical:**
- API is defined but always returns empty vector
- Users expect it to work based on trait signature
- Important for:
  - Debugging state transitions
  - Audit trails
  - Rollback scenarios

**Fix:** Implement actual history retrieval from storage layer

---

## Part 3: Missing Table Stakes APIs (P0)

### Gap 4: `state_init` - Conditional Create (Init If Absent)

**Priority:** P0 - Common coordination pattern

**What Primitive Has:**
```rust
fn init(&self, run_id: &RunId, name: &str, value: Value) -> Result<Versioned<u64>>;
```

**What Substrate Exposes:** Nothing (must use CAS workaround)

**Why Critical:**
- "Create if not exists" is fundamental for:
  - Lock acquisition
  - Leader election
  - One-time initialization
- CAS with `expected_counter: None` is non-obvious workaround

**Proposed Substrate API:**
```rust
fn state_init(&self, run: &ApiRunId, cell: &str, value: Value)
    -> StrataResult<Option<Version>>;  // None if already exists
```

---

### Gap 5: `state_get_or_init` - Get or Initialize

**Priority:** P0 - Extremely common pattern

**What Primitive Has:** Can be composed from `init` + `read`

**What Substrate Exposes:** Nothing

**Why Critical:**
- "Get existing or create with default" is ubiquitous
- Without it, requires two calls + race condition handling

**Proposed Substrate API:**
```rust
fn state_get_or_init(&self, run: &ApiRunId, cell: &str, default: Value)
    -> StrataResult<Versioned<Value>>;
```

---

### Gap 6: `state_info` - Cell Metadata (O(1))

**Priority:** P0 - Performance and monitoring

**What Primitive Has:** Can be derived from `read()`

**What Substrate Exposes:** Only `state_exists()` (boolean)

**Why Critical:**
- Need to know version/timestamp without reading full value
- Useful for:
  - Staleness checks
  - Version comparisons
  - Monitoring cell activity

**Proposed Substrate API:**
```rust
struct CellInfo {
    version: u64,
    updated_at: i64,
    exists: bool,
}

fn state_info(&self, run: &ApiRunId, cell: &str) -> StrataResult<Option<CellInfo>>;
```

---

## Part 4: Missing Important APIs (P1)

### Gap 7: `state_watch` - Watch for Changes

**Priority:** P1 - Standard coordination feature

**Proposed API:**
```rust
fn state_watch(&self, run: &ApiRunId, cell: &str,
               from_version: Option<u64>, timeout_ms: u64)
    -> StrataResult<Option<Versioned<Value>>>;
```

**Why Important:**
- Coordination primitives need change notification
- Without this, must poll for changes
- Industry standard:
  - ZooKeeper: Watches
  - etcd: Watch API
  - Consul: Blocking queries

**Current Workaround:** Polling with sleep (inefficient)

---

### Gap 8: `state_ttl` / `state_set_with_ttl` - Time-To-Live

**Priority:** P1 - Important for ephemeral coordination

**Proposed API:**
```rust
fn state_set_with_ttl(&self, run: &ApiRunId, cell: &str, value: Value, ttl_ms: u64)
    -> StrataResult<Version>;

fn state_refresh_ttl(&self, run: &ApiRunId, cell: &str, ttl_ms: u64)
    -> StrataResult<bool>;
```

**Why Important:**
- Ephemeral locks that auto-release on failure
- Leader election with automatic failover
- Session management
- Industry standard:
  - ZooKeeper: Ephemeral nodes
  - etcd: Leases
  - Redis: EXPIRE

**Current Workaround:** Manual cleanup (unreliable if process crashes)

---

## Part 5: API Design Issues (P1)

### Design Issue 1: CAS Returns Option Instead of Result

**Current:**
```rust
fn state_cas(...) -> StrataResult<Option<Version>>;
// None = version mismatch (not an error)
```

**Problem:**
- Conflates "operation failed" with "version mismatch"
- Users can't distinguish network error from CAS failure
- Inconsistent with KVStore CAS which returns error

**Should Be:**
```rust
fn state_cas(...) -> StrataResult<Version>;
// Err(VersionMismatch) on conflict
```

---

### Design Issue 2: Timestamp Not Exposed Consistently

**Current Return Type:**
```rust
Versioned<Value>  // Has version + value, timestamp buried
```

**State Struct Has:**
```rust
struct State {
    value: Value,
    version: u64,
    updated_at: i64,  // This exists!
}
```

**Problem:** `updated_at` timestamp is in the primitive `State` struct but not consistently exposed through `Versioned<Value>` at substrate level.

---

## Part 6: Known Limitations (Not Bugs)

### Limitation 1: Counter Versioning (Not Transaction Versioning)

StateCell uses counter versioning (1, 2, 3...) not transaction versioning.

**Implication:** Cannot correlate StateCell versions with KVStore versions in cross-primitive transactions.

**Status:** By design - different versioning semantics

---

### Limitation 2: Transition Closure Purity Requirement

Transition closures MUST be pure functions (no I/O, no side effects) because they may be retried multiple times.

**Status:** By design - documented requirement

---

## Priority Matrix

| ID | Issue | Priority | Effort | Category |
|----|-------|----------|--------|----------|
| Gap 1 | Transition closures | P0 | Low | Missing Core Feature |
| Gap 2 | List cells | P0 | Low | Missing API |
| Gap 3 | History stubbed | P0 | Medium | Unimplemented |
| Gap 4 | Init (create if absent) | P0 | Low | Missing API |
| Gap 5 | Get or init | P0 | Low | Missing API |
| Gap 6 | Cell info/metadata | P0 | Low | Missing API |
| Gap 7 | Watch/subscribe | P1 | High | Missing API |
| Gap 8 | TTL/lease | P1 | High | Missing API |
| Design 1 | CAS return type | P1 | Low | Design |
| Design 2 | Timestamp exposure | P1 | Low | Design |

---

## Recommended Fix Order

### Phase 1: Expose Existing Primitives (Low Effort)
1. Expose `state_transition` / `state_transition_or_init` (Gap 1) - **CRITICAL**
2. Expose `state_list` (Gap 2) - primitive has it
3. Expose `state_init` (Gap 4) - primitive has it
4. Add `state_get_or_init` (Gap 5) - compose from existing
5. Add `state_info` (Gap 6) - derive from read

### Phase 2: Implement Missing Features (Medium Effort)
6. Implement `state_history` (Gap 3) - storage layer may have capability
7. Fix CAS return type (Design 1)
8. Expose timestamp consistently (Design 2)

### Phase 3: Advanced Coordination (High Effort)
9. Implement `state_watch` (Gap 7) - requires notification infrastructure
10. Implement `state_ttl` (Gap 8) - requires background expiration

---

## Test Coverage Needed

| API | Test Cases |
|-----|------------|
| `state_transition` | Basic transition, retry on conflict, concurrent transitions |
| `state_transition_or_init` | Init path, existing path, concurrent init |
| `state_list` | Empty run, multiple cells, after delete |
| `state_init` | Create new, reject existing, concurrent init |
| `state_get_or_init` | Get existing, create default, concurrent |
| `state_info` | Exists, not exists, after updates |
| `state_history` | Single version, multiple versions, limit, before cursor |
| `state_watch` | Immediate change, timeout, version filtering |
| `state_ttl` | Expiration, refresh, delete before expire |

---

## Comparison with Industry Standards

| Feature | Strata StateCell | ZooKeeper | etcd | Consul KV |
|---------|------------------|-----------|------|-----------|
| Get/Set | ✅ | ✅ | ✅ | ✅ |
| CAS | ✅ | ✅ (version) | ✅ (mod_revision) | ✅ (ModifyIndex) |
| Delete | ✅ | ✅ | ✅ | ✅ |
| Exists | ✅ | ✅ | ✅ | ✅ |
| **Transitions** | ❌ (primitive only) | ❌ | ❌ | ❌ |
| **List** | ❌ (primitive only) | ✅ (children) | ✅ (prefix) | ✅ (prefix) |
| History | ❌ (stubbed) | ❌ | ✅ (revisions) | ❌ |
| Watch | ❌ | ✅ | ✅ | ✅ (blocking) |
| TTL/Lease | ❌ | ✅ (ephemeral) | ✅ (lease) | ✅ (session) |
| Init if absent | ❌ (primitive only) | ✅ (create) | ❌ | ❌ |

**Strata's Unique Strength:** Transition closures with automatic retry (but hidden!)

**Strata's Gaps:** Watch, TTL, list cells, history

---

## Critical Finding

**The most important feature of StateCell (transition closures) is completely hidden at the Substrate level.**

The primitive layer has:
- `transition()` - atomic read-modify-write with 200-retry OCC
- `transition_or_init()` - same with initialization

But Substrate users must manually implement retry loops using `state_get` + `state_cas`, which is:
1. Error-prone (easy to get retry logic wrong)
2. Verbose (10+ lines vs 3 lines)
3. Missing the point (StateCell IS transitions)

**Recommendation:** Exposing `state_transition` should be the #1 priority for StateCell.
