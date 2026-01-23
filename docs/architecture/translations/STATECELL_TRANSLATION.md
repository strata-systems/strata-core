# StateCell: Substrate → Primitive Translation

## Contract Promises (M11_CONTRACT.md)

### Facade API (Section 10.5)

| Operation | Signature | Return | Notes |
|-----------|-----------|--------|-------|
| `cas_set` | `(key, expected: Option<Value>, new: Value)` | `bool` | Compare-and-swap |
| `cas_get` | `(key)` | `Option<Value>` | Get current value |

**CAS semantics:**
- `expected = None` means "only set if key is missing" (create-if-not-exists)
- `expected = Some(Value::Null)` means "only set if current value is null"
- Comparison uses structural equality

**Value equality for CAS:**
- `Null`, `Bool`, `Int`, `String`, `Bytes`: exact equality
- `Float`: IEEE-754 equality (`NaN != NaN`, `-0.0 == 0.0`)
- `Array`: element-wise recursive equality
- `Object`: key-set equality + recursive value equality (order irrelevant)

### Substrate API (Section 11.6)

```rust
state_get(run, key) -> Option<Versioned<Value>>
state_set(run, key, value) -> Version
state_cas(run, key, expected, new) -> bool
```

Note: Contract also mentions `state_history` in Section 18.1 as optional.

---

## Substrate API Promises

| Method | Signature | Purpose |
|--------|-----------|---------|
| `state_set` | `(run, cell, value) → Version` | Set value unconditionally |
| `state_get` | `(run, cell) → Option<Versioned<Value>>` | Get current value with version |
| `state_cas` | `(run, cell, expected, value) → Option<Version>` | Compare-and-swap |
| `state_delete` | `(run, cell) → bool` | Delete cell |
| `state_exists` | `(run, cell) → bool` | Check existence |
| `state_history` | `(run, cell, limit, before) → Vec<Versioned<Value>>` | Version history |

## Primitive Provides

| Method | Signature | Version Exposed? |
|--------|-----------|------------------|
| `init(run_id, name, value)` | `→ Result<Versioned<u64>>` | ✅ Yes (Counter) |
| `read(run_id, name)` | `→ Result<Option<Versioned<State>>>` | ✅ Yes (Counter) |
| `set(run_id, name, value)` | `→ Result<Versioned<u64>>` | ✅ Yes (Counter) |
| `cas(run_id, name, expected, new)` | `→ Result<Versioned<u64>>` | ✅ Yes (Counter) |
| `delete(run_id, name)` | `→ Result<bool>` | ❌ No |
| `exists(run_id, name)` | `→ Result<bool>` | ❌ No |
| `list(run_id)` | `→ Result<Vec<String>>` | ❌ No |
| `transition(run_id, name, f)` | `→ Result<(T, Versioned<u64>)>` | ✅ Yes |

## Type Differences

| Substrate | Primitive | Conversion Needed |
|-----------|-----------|-------------------|
| `ApiRunId` | `RunId` | `run.to_run_id()` |
| `&str` (cell) | `&str` (name) | Same |
| `Value` | `Value` | Same |
| `Option<u64>` (expected) | `u64` (expected) | Handle None case |
| `Versioned<Value>` | `Versioned<State>` | Extract `state.value` |
| `Version` | `Versioned<u64>` | Create `Version::Counter` |

---

## Method Translations

### `state_set` - DIRECT MAPPING

**Substrate**: Unconditionally sets value, creates cell if needed, returns Version.

**Primitive**: `set()` does exactly this.

**Translation**:
```rust
fn state_set(&self, run: &ApiRunId, cell: &str, value: Value) -> StrataResult<Version> {
    let run_id = run.to_run_id();

    let versioned = self.state.set(&run_id, cell, value)?;
    Ok(Version::Counter(versioned.value))
}
```

**No gap** - direct mapping.

---

### `state_get` - TYPE CONVERSION

**Substrate**: Returns `Versioned<Value>`.

**Primitive**: Returns `Versioned<State>` where `State { value, version, updated_at }`.

**Translation**:
```rust
fn state_get(&self, run: &ApiRunId, cell: &str) -> StrataResult<Option<Versioned<Value>>> {
    let run_id = run.to_run_id();

    match self.state.read(&run_id, cell)? {
        Some(versioned_state) => {
            // Extract Value from State
            let state = versioned_state.value;
            Ok(Some(Versioned {
                value: state.value,
                version: Version::Counter(state.version),
                timestamp: versioned_state.timestamp,
            }))
        }
        None => Ok(None),
    }
}
```

**No gap** - just type conversion.

---

### `state_cas` - SEMANTIC DIFFERENCE

**Substrate**:
- `expected_counter = None` → only succeed if cell doesn't exist
- `expected_counter = Some(n)` → succeed only if counter == n
- Returns `Option<Version>`: Some on success, None on mismatch (no error)

**Primitive**:
- `cas()` requires cell to exist, returns `Error::VersionMismatch` on mismatch
- `init()` creates only if not exists, returns `Error::InvalidOperation` if exists

**Translation**:
```rust
fn state_cas(
    &self,
    run: &ApiRunId,
    cell: &str,
    expected_counter: Option<u64>,
    value: Value,
) -> StrataResult<Option<Version>> {
    let run_id = run.to_run_id();

    match expected_counter {
        None => {
            // Create only if doesn't exist
            match self.state.init(&run_id, cell, value) {
                Ok(versioned) => Ok(Some(Version::Counter(versioned.value))),
                Err(strata_core::error::Error::InvalidOperation(_)) => Ok(None), // Already exists
                Err(e) => Err(e.into()),
            }
        }
        Some(expected) => {
            // CAS with expected version
            match self.state.cas(&run_id, cell, expected, value) {
                Ok(versioned) => Ok(Some(Version::Counter(versioned.value))),
                Err(strata_core::error::Error::VersionMismatch { .. }) => Ok(None),
                Err(strata_core::error::Error::InvalidOperation(_)) => Ok(None), // Not found
                Err(e) => Err(e.into()),
            }
        }
    }
}
```

**Gap handled**: Convert errors to None for CAS failure semantics.

---

### `state_delete` - DIRECT MAPPING

**Substrate**: Returns `true` if cell existed.

**Primitive**: Same semantics.

**Translation**:
```rust
fn state_delete(&self, run: &ApiRunId, cell: &str) -> StrataResult<bool> {
    let run_id = run.to_run_id();
    Ok(self.state.delete(&run_id, cell)?)
}
```

**No gap** - direct mapping.

---

### `state_exists` - DIRECT MAPPING

**Substrate**: Returns `true` if cell exists.

**Primitive**: Same semantics.

**Translation**:
```rust
fn state_exists(&self, run: &ApiRunId, cell: &str) -> StrataResult<bool> {
    let run_id = run.to_run_id();
    Ok(self.state.exists(&run_id, cell)?)
}
```

**No gap** - direct mapping.

---

### `state_history` - NOT IN PRIMITIVE

**Substrate**: Returns historical versions, newest first.

**Primitive**: No history support.

**Same gap as KVStore**: Storage layer has `VersionChain` but primitive doesn't expose it.

**Current stub**:
```rust
fn state_history(
    &self,
    _run: &ApiRunId,
    _cell: &str,
    _limit: Option<u64>,
    _before: Option<Version>,
) -> StrataResult<Vec<Versioned<Value>>> {
    // History not yet implemented in primitive
    Ok(vec![])
}
```

---

## Summary Table

| Substrate Method | Primitive Method | Gap |
|-----------------|------------------|-----|
| `state_set` | `set` | None - direct |
| `state_get` | `read` | Type conversion only |
| `state_cas` | `init` + `cas` | Error→None conversion for CAS semantics |
| `state_delete` | `delete` | None - direct |
| `state_exists` | `exists` | None - direct |
| `state_history` | ❌ None | Same as KVStore - needs primitive support |

## Gaps Requiring Primitive Enhancement

| Method | What's Needed |
|--------|---------------|
| `state_history` | Expose `VersionChain` iteration for cells |

## Gaps Handled in Substrate

| Method | How Handled |
|--------|-------------|
| `state_cas` (expected=None) | Use `init()`, convert AlreadyExists → None |
| `state_cas` (expected=Some) | Use `cas()`, convert VersionMismatch → None |

## Additional Primitive Capabilities

The primitive has features NOT exposed in substrate:

| Primitive Feature | Description | Substrate Equivalent? |
|-------------------|-------------|----------------------|
| `list()` | List all cell names | ❌ Not exposed |
| `transition()` | CAS with closure + retry | ❌ Not exposed |
| `transition_or_init()` | Init + transition combo | ❌ Not exposed |

Consider exposing these in substrate for full value:

```rust
// Potential substrate additions
fn state_list(&self, run: &ApiRunId) -> StrataResult<Vec<String>>;
fn state_transition<F, T>(&self, run: &ApiRunId, cell: &str, f: F) -> StrataResult<T>;
```

## Version Model

StateCell uses `Version::Counter` semantics:
- Counter starts at 1 when cell is created
- Every write increments counter by 1
- Counter is a simple monotonic u64

Both layers agree on this model.

---

## Contract Gap Summary

### Facade → Substrate: FULLY COVERED

| Facade | Substrate | Status |
|--------|-----------|--------|
| `cas_set(key, expected, new)` | `state_cas(default, key, expected, new)` | ✅ |
| `cas_get(key)` | `state_get(default, key).map(\|v\| v.value)` | ✅ |

### Substrate → Primitive: MOSTLY COVERED

| Substrate Method | Primitive Support | Gap |
|------------------|-------------------|-----|
| `state_get` | `read()` ✅ | Type conversion only |
| `state_set` | `set()` ✅ | None - direct |
| `state_cas` | `init()` + `cas()` ✅ | Error→None conversion |
| `state_delete` | `delete()` ✅ | None - direct |
| `state_exists` | `exists()` ✅ | None - direct |
| `state_history` | ❌ **MISSING** | P2: Optional per contract |

### CAS Semantics Translation

Contract CAS has special handling for `expected = None`:

```rust
fn state_cas(run, key, expected: Option<Value>, new: Value) -> bool {
    match expected {
        None => {
            // Create only if doesn't exist
            match primitive.init(run_id, key, new) {
                Ok(_) => true,
                Err(AlreadyExists) => false,
                Err(e) => return Err(e),
            }
        }
        Some(expected_val) => {
            // True CAS with value comparison
            match primitive.cas(run_id, key, expected_val, new) {
                Ok(_) => true,
                Err(VersionMismatch) => false,
                Err(NotFound) => false,
                Err(e) => return Err(e),
            }
        }
    }
}
```

### Note on CAS Value Comparison

Contract requires **value comparison** but primitive uses **version comparison**. Translation:

1. Get current value with version
2. Compare value structurally (following equality rules)
3. If match, use version for CAS operation
4. This maintains atomicity through version-based CAS
