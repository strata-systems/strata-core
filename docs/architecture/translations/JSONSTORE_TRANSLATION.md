# JsonStore: Substrate → Primitive Translation

## Contract Promises (M11_CONTRACT.md)

### Facade API (Section 10.2)

| Operation | Signature | Return | Notes |
|-----------|-----------|--------|-------|
| `json_set` | `(key, path, value)` | `()` | Root must be Object |
| `json_get` | `(key, path)` | `Option<Value>` | Returns value at path |
| `json_getv` | `(key, path)` | `Option<Versioned<Value>>` | Returns document-level version |
| `json_del` | `(key, path)` | `u64` | Count of elements removed |
| `json_merge` | `(key, path, value)` | `()` | RFC 7396 JSON Merge Patch semantics |

**Path syntax:**
- Root: `$` (entire document)
- Object field: `$.a.b`
- Array index: `$.items[0]`
- Array append: `$.items[-]` (for `json_set` only)
- Negative indices `[-1]` NOT supported → `InvalidPath`

### Substrate API (Section 11.4)

```rust
json_set(run, key, path, value) -> Version
json_get(run, key, path) -> Option<Versioned<Value>>
json_delete(run, key, path) -> u64
json_merge(run, key, path, value) -> Version
json_history(run, key, limit?, before?) -> Vec<Versioned<Value>>
```

---

## Substrate API Promises

| Method | Signature | Purpose |
|--------|-----------|---------|
| `json_set` | `(run, key, path, value) → Version` | Set value at path (creates doc if needed) |
| `json_get` | `(run, key, path) → Option<Versioned<Value>>` | Get value at path |
| `json_delete` | `(run, key, path) → u64` | Delete at path, return count |
| `json_merge` | `(run, key, path, patch) → Version` | RFC 7396 merge patch |
| `json_history` | `(run, key, limit, before) → Vec<Versioned<Value>>` | Document version history |

## Primitive Provides

| Method | Signature | Version Exposed? |
|--------|-----------|------------------|
| `create(run_id, doc_id, value)` | `→ Result<Version>` | ✅ Yes |
| `get(run_id, doc_id, path)` | `→ Result<Option<Versioned<JsonValue>>>` | ✅ Yes |
| `get_doc(run_id, doc_id)` | `→ Result<Option<Versioned<JsonDoc>>>` | ✅ Yes |
| `get_version(run_id, doc_id)` | `→ Result<Option<u64>>` | ✅ Yes |
| `exists(run_id, doc_id)` | `→ Result<bool>` | ❌ No |
| `set(run_id, doc_id, path, value)` | `→ Result<Version>` | ✅ Yes |
| `delete_at_path(run_id, doc_id, path)` | `→ Result<Version>` | ✅ Yes (returns version, not count) |
| `destroy(run_id, doc_id)` | `→ Result<bool>` | ❌ No |

## Type Differences

| Substrate | Primitive | Conversion Needed |
|-----------|-----------|-------------------|
| `&str` (key) | `JsonDocId` | Hash string → deterministic UUID |
| `&str` (path) | `JsonPath` | Parse with `path.parse()` |
| `Value` | `JsonValue` | `serde_json::to_value` / `from_value` |
| `ApiRunId` | `RunId` | `run.to_run_id()` |

## Gap Analysis

### `json_set` - SEMANTIC DIFFERENCE

**Substrate**: Creates document if it doesn't exist, then sets at path.

**Primitive**: `set()` **fails** if document doesn't exist.

**Translation**:
```rust
fn json_set(&self, run: &ApiRunId, key: &str, path: &str, value: Value) -> StrataResult<Version> {
    let run_id = run.to_run_id();
    let doc_id = parse_doc_id(key)?;
    let json_path = parse_path(path)?;
    let json_value = value_to_json(value)?;

    // Check if document exists
    if !self.json.exists(&run_id, &doc_id)? {
        if json_path.is_root() {
            // Create new document at root
            return self.json.create(&run_id, &doc_id, json_value);
        } else {
            // Create with empty object, then set path
            self.json.create(&run_id, &doc_id, JsonValue::object())?;
        }
    }

    self.json.set(&run_id, &doc_id, &json_path, json_value)
}
```

---

### `json_get` - DIRECT with type conversion

**Substrate**: Returns `Versioned<Value>` (strata_core::Value)

**Primitive**: Returns `Versioned<JsonValue>` (serde_json wrapper)

**Translation**:
```rust
fn json_get(&self, run: &ApiRunId, key: &str, path: &str)
    -> StrataResult<Option<Versioned<Value>>>
{
    let run_id = run.to_run_id();
    let doc_id = parse_doc_id(key)?;
    let json_path = parse_path(path)?;

    match self.json.get(&run_id, &doc_id, &json_path)? {
        Some(versioned) => {
            let value = json_to_value(versioned.value)?;  // JsonValue → Value
            Ok(Some(Versioned {
                value,
                version: versioned.version,
                timestamp: versioned.timestamp,
            }))
        }
        None => Ok(None),
    }
}
```

---

### `json_delete` - RETURN TYPE DIFFERENCE

**Substrate**: Returns `u64` (count of elements deleted)

**Primitive**:
- `delete_at_path()` returns `Version` (not count)
- `destroy()` returns `bool` (existed or not)

**Translation**:
```rust
fn json_delete(&self, run: &ApiRunId, key: &str, path: &str) -> StrataResult<u64> {
    let run_id = run.to_run_id();
    let doc_id = parse_doc_id(key)?;
    let json_path = parse_path(path)?;

    if json_path.is_root() {
        // Delete entire document
        let existed = self.json.destroy(&run_id, &doc_id)?;
        Ok(if existed { 1 } else { 0 })
    } else {
        // Delete at path - assume 1 if successful
        // Note: Primitive doesn't tell us if path existed
        self.json.delete_at_path(&run_id, &doc_id, &json_path)?;
        Ok(1)
    }
}
```

**Gap**: Primitive doesn't return count. We return 1 on success.

---

### `json_merge` - NOT IN PRIMITIVE

**Substrate**: RFC 7396 JSON Merge Patch

**Primitive**: No merge operation.

**Translation** (substrate must implement):
```rust
fn json_merge(&self, run: &ApiRunId, key: &str, path: &str, patch: Value)
    -> StrataResult<Version>
{
    let run_id = run.to_run_id();
    let doc_id = parse_doc_id(key)?;
    let json_path = parse_path(path)?;
    let patch_json = value_to_json(patch)?;

    // Get current value at path
    let current = self.json.get(&run_id, &doc_id, &json_path)?;

    // Apply RFC 7396 merge patch
    let merged = match current {
        Some(versioned) => {
            let mut base: serde_json::Value = versioned.value.into();
            let patch_val: serde_json::Value = patch_json.into();
            json_merge_patch(&mut base, &patch_val);  // Implement RFC 7396
            JsonValue::from(base)
        }
        None => patch_json,
    };

    self.json.set(&run_id, &doc_id, &json_path, merged)
}

// RFC 7396 implementation
fn json_merge_patch(target: &mut serde_json::Value, patch: &serde_json::Value) {
    if let serde_json::Value::Object(patch_obj) = patch {
        if !target.is_object() {
            *target = serde_json::Value::Object(Default::default());
        }
        if let serde_json::Value::Object(target_obj) = target {
            for (key, value) in patch_obj {
                if value.is_null() {
                    target_obj.remove(key);
                } else {
                    json_merge_patch(target_obj.entry(key).or_insert(serde_json::Value::Null), value);
                }
            }
        }
    } else {
        *target = patch.clone();
    }
}
```

**Gap**: Must implement merge logic in substrate layer.

---

### `json_history` - NOT IN PRIMITIVE

**Substrate**: Get document version history.

**Primitive**: No history support.

**Same gap as KVStore**: Storage layer has `VersionChain` but primitive doesn't expose it.

**Current stub**:
```rust
fn json_history(&self, _run: &ApiRunId, _key: &str, _limit: Option<u64>, _before: Option<Version>)
    -> StrataResult<Vec<Versioned<Value>>>
{
    // History not yet implemented in primitive
    Ok(vec![])
}
```

---

## Summary Table

| Substrate Method | Primitive Method | Gap |
|-----------------|------------------|-----|
| `json_set` | `exists` + `create` + `set` | Semantic: create-if-not-exists |
| `json_get` | `get` | Type conversion only |
| `json_delete` | `destroy` / `delete_at_path` | Returns Version not count |
| `json_merge` | ❌ None | Must implement RFC 7396 in substrate |
| `json_history` | ❌ None | Same as KVStore - needs primitive support |

## Gaps Requiring Primitive Enhancement

| Method | What's Needed |
|--------|---------------|
| `json_history` | Expose `VersionChain` iteration for documents |

## Gaps Handled in Substrate

| Method | How Handled |
|--------|-------------|
| `json_set` | Check exists, create if needed, then set |
| `json_delete` | Use destroy for root, delete_at_path for paths |
| `json_merge` | Implement RFC 7396 merge patch algorithm |

---

## Contract Gap Summary

### Facade → Substrate: FULLY COVERED

| Facade | Substrate | Status |
|--------|-----------|--------|
| `json_set(key, path, value)` | `begin(); json_set(default, ...); commit()` | ✅ |
| `json_get(key, path)` | `json_get(default, key, path).map(\|v\| v.value)` | ✅ |
| `json_getv(key, path)` | `json_get(default, key, path)` | ✅ |
| `json_del(key, path)` | `begin(); json_delete(default, ...); commit()` | ✅ |
| `json_merge(key, path, value)` | `begin(); json_merge(default, ...); commit()` | ✅ |

### Substrate → Primitive: GAPS EXIST

| Substrate Method | Primitive Support | Gap |
|------------------|-------------------|-----|
| `json_set` | `create()` + `set()` ✅ | Semantic: create-if-not-exists |
| `json_get` | `get()` ✅ | Type conversion only |
| `json_delete` | `destroy()` / `delete_at_path()` ✅ | Returns Version not count |
| `json_merge` | ❌ **MISSING** | **Must implement RFC 7396 in substrate** |
| `json_history` | ❌ **MISSING** | **P0: Primitive must expose VersionChain** |

### Critical Gaps

1. **`json_merge`** (P1): Contract requires RFC 7396 merge patch. Primitive has no merge. Substrate must implement.
2. **`json_history`** (P0): Contract promises document history. Same gap as KVStore - primitive doesn't expose VersionChain.

### Substrate Implementation Required

```rust
// RFC 7396 merge patch - substrate must implement
fn json_merge_patch(target: &mut Value, patch: &Value) {
    if let Value::Object(patch_obj) = patch {
        if !target.is_object() {
            *target = Value::Object(Default::default());
        }
        if let Value::Object(target_obj) = target {
            for (key, value) in patch_obj {
                if value.is_null() {
                    target_obj.remove(key);
                } else {
                    json_merge_patch(
                        target_obj.entry(key).or_insert(Value::Null),
                        value
                    );
                }
            }
        }
    } else {
        *target = patch.clone();
    }
}
```
