# RunIndex: Substrate → Primitive Translation

## Contract Promises (M11_CONTRACT.md)

### Facade API (Section 10.7)

| Operation | Signature | Return | Notes |
|-----------|-----------|--------|-------|
| `runs` | `()` | `Vec<RunInfo>` | List all runs |
| `use_run` | `(run_id)` | `ScopedFacade` | Scope operations to run |

**Run lifecycle** (`create_run`, `close_run`) is **substrate-only**. Facade hides run lifecycle.

**use_run behavior:** If `run_id` does not exist, returns `NotFound`. No lazy creation.

### Substrate API (Section 11.9)

```rust
run_create(metadata: Value) -> RunId
run_get(run: RunId) -> Option<RunInfo>
run_list() -> Vec<RunInfo>
run_close(run: RunId) -> ()
```

**No run deletion in M11.** Garbage collection of runs is deferred.

### Retention API (Section 11.10)

```rust
retention_get(run_id) -> Option<Versioned<RetentionPolicy>>
retention_set(run_id, policy) -> Version

enum RetentionPolicy {
    KeepAll,              // Default
    KeepLast(u64),        // Keep N most recent versions
    KeepFor(Duration),    // Keep versions within time window
    Composite(Vec<RetentionPolicy>)  // Union of policies
}
```

### Run Semantics (Section 16)

**Default Run:**
- Named `"default"` (literal string, not a UUID)
- Always exists implicitly
- Cannot be closed
- Internally maps to UUID::nil (`00000000-0000-0000-0000-000000000000`)

**RunInfo structure:**
```rust
struct RunInfo {
    run_id: RunId,
    created_at: u64,  // microseconds
    metadata: Value,
    state: RunState   // "active" | "closed"
}
```

---

## Substrate API Promises

| Method | Signature | Purpose |
|--------|-----------|---------|
| `run_create` | `(run_id?, metadata?) → (RunInfo, Version)` | Create run |
| `run_get` | `(run) → Option<Versioned<RunInfo>>` | Get run info |
| `run_list` | `(state?, limit?, offset?) → Vec<Versioned<RunInfo>>` | List runs |
| `run_close` | `(run) → Version` | Close run (make read-only) |
| `run_update_metadata` | `(run, metadata) → Version` | Update metadata |
| `run_exists` | `(run) → bool` | Check existence |
| `run_set_retention` | `(run, policy) → Version` | Set retention policy |
| `run_get_retention` | `(run) → RetentionPolicy` | Get retention policy |

## Primitive Provides

| Method | Signature | Version Exposed? |
|--------|-----------|------------------|
| `create_run(run_id)` | `→ Result<Versioned<RunMetadata>>` | ✅ Yes (Counter) |
| `create_run_with_options(run_id, parent, tags, meta)` | `→ Result<Versioned<RunMetadata>>` | ✅ Yes |
| `get_run(run_id)` | `→ Result<Option<Versioned<RunMetadata>>>` | ✅ Yes |
| `exists(run_id)` | `→ Result<bool>` | ❌ No |
| `list_runs()` | `→ Result<Vec<String>>` | ❌ No |
| `update_status(run_id, status)` | `→ Result<Versioned<RunMetadata>>` | ✅ Yes |
| `complete_run(run_id)` | `→ Result<Versioned<RunMetadata>>` | ✅ Yes |
| `archive_run(run_id)` | `→ Result<Versioned<RunMetadata>>` | ✅ Yes |
| `delete_run(run_id)` | `→ Result<()>` | ❌ No |
| `add_tags(run_id, tags)` | `→ Result<Versioned<RunMetadata>>` | ✅ Yes |
| `remove_tags(run_id, tags)` | `→ Result<Versioned<RunMetadata>>` | ✅ Yes |
| `update_metadata(run_id, metadata)` | `→ Result<Versioned<RunMetadata>>` | ✅ Yes |
| `query_by_status(status)` | `→ Result<Vec<RunMetadata>>` | ❌ No |

## Type Differences

| Substrate | Primitive | Conversion Needed |
|-----------|-----------|-------------------|
| `ApiRunId` | `&str` (run name) | `run.as_str()` |
| `substrate::RunState` | `primitives::RunStatus` | See mapping below |
| `substrate::RunInfo` | `primitives::RunMetadata` | Field mapping |
| `substrate::RetentionPolicy` | ❌ None | Not implemented |

### RunState vs RunStatus Mapping

**Substrate RunState** (simple):
```rust
enum RunState {
    Active,
    Closed,
}
```

**Primitive RunStatus** (rich):
```rust
enum RunStatus {
    Active,
    Completed,
    Failed,
    Cancelled,
    Paused,
    Archived,
}
```

**Mapping**:
- Substrate `Active` → Primitive `Active`
- Substrate `Closed` → Primitive `Archived` OR any of `Completed/Failed/Cancelled/Archived`
- For query: Substrate `Closed` = Primitive `!Active`

### RunInfo vs RunMetadata Mapping

**Substrate RunInfo**:
```rust
struct RunInfo {
    run_id: String,
    state: RunState,
    created_at: u64,
    metadata: Value,
    retention: RetentionPolicy,
}
```

**Primitive RunMetadata**:
```rust
struct RunMetadata {
    name: String,
    run_id: String,       // UUID
    parent_run: Option<String>,
    status: RunStatus,
    created_at: i64,
    updated_at: i64,
    completed_at: Option<i64>,
    tags: Vec<String>,
    metadata: Value,
    error: Option<String>,
    version: u64,
}
```

---

## Method Translations

### `run_create` - TYPE MAPPING

**Substrate**: Creates with optional ID and metadata, returns `(RunInfo, Version)`.

**Primitive**: Creates with required ID, optional parent/tags/metadata.

**Translation**:
```rust
fn run_create(
    &self,
    run_id: Option<&ApiRunId>,
    metadata: Option<Value>,
) -> StrataResult<(RunInfo, Version)> {
    let name = match run_id {
        Some(id) => id.as_str().to_string(),
        None => uuid::Uuid::new_v4().to_string(),
    };

    let versioned = self.run_index.create_run_with_options(
        &name,
        None,           // parent
        vec![],         // tags
        metadata.unwrap_or(Value::Null),
    )?;

    let info = metadata_to_info(&versioned.value);
    Ok((info, versioned.version))
}

fn metadata_to_info(meta: &RunMetadata) -> RunInfo {
    RunInfo {
        run_id: meta.name.clone(),
        state: match meta.status {
            RunStatus::Active | RunStatus::Paused => RunState::Active,
            _ => RunState::Closed,
        },
        created_at: meta.created_at as u64,
        metadata: meta.metadata.clone(),
        retention: RetentionPolicy::default(), // Not stored in primitive
    }
}
```

---

### `run_get` - TYPE CONVERSION

**Substrate**: Returns `Versioned<RunInfo>`.

**Primitive**: Returns `Versioned<RunMetadata>`.

**Translation**:
```rust
fn run_get(&self, run: &ApiRunId) -> StrataResult<Option<Versioned<RunInfo>>> {
    match self.run_index.get_run(run.as_str())? {
        Some(versioned) => {
            let info = metadata_to_info(&versioned.value);
            Ok(Some(Versioned {
                value: info,
                version: versioned.version,
                timestamp: versioned.timestamp,
            }))
        }
        None => Ok(None),
    }
}
```

---

### `run_list` - FILTERING + PAGINATION

**Substrate**: Filters by state, supports limit/offset.

**Primitive**: Has `list_runs()` returning names and `query_by_status(status)`.

**Translation**:
```rust
fn run_list(
    &self,
    state: Option<RunState>,
    limit: Option<u64>,
    offset: Option<u64>,
) -> StrataResult<Vec<Versioned<RunInfo>>> {
    // Get all runs (or filter by status)
    let run_names = self.run_index.list_runs()?;

    let mut results = Vec::new();
    for name in run_names {
        if let Some(versioned) = self.run_index.get_run(&name)? {
            // Filter by state
            let matches_state = match state {
                Some(RunState::Active) => matches!(
                    versioned.value.status,
                    RunStatus::Active | RunStatus::Paused
                ),
                Some(RunState::Closed) => !matches!(
                    versioned.value.status,
                    RunStatus::Active | RunStatus::Paused
                ),
                None => true,
            };

            if matches_state {
                results.push(Versioned {
                    value: metadata_to_info(&versioned.value),
                    version: versioned.version,
                    timestamp: versioned.timestamp,
                });
            }
        }
    }

    // Sort by created_at (newest first)
    results.sort_by(|a, b| b.value.created_at.cmp(&a.value.created_at));

    // Apply offset and limit
    let offset = offset.unwrap_or(0) as usize;
    let results: Vec<_> = results.into_iter().skip(offset).collect();

    let results = match limit {
        Some(n) => results.into_iter().take(n as usize).collect(),
        None => results,
    };

    Ok(results)
}
```

**Gap**: Primitive returns `Vec<String>` (names), not metadata. Must re-fetch each.

---

### `run_close` - STATUS TRANSITION

**Substrate**: Marks run as closed (read-only).

**Primitive**: Uses status transitions. Map to `archive_run` or a "closed" concept.

**Translation**:
```rust
fn run_close(&self, run: &ApiRunId) -> StrataResult<Version> {
    let name = run.as_str();

    // Check it's not the default run
    if name == "default" {
        return Err(StrataError::ConstraintViolation(
            "Cannot close default run".into()
        ));
    }

    // Archive the run (makes it "closed")
    let versioned = self.run_index.archive_run(name)?;
    Ok(versioned.version)
}
```

**Gap**: Substrate uses simple Active/Closed; Primitive has richer lifecycle.

---

### `run_update_metadata` - DIRECT MAPPING

**Substrate**: Updates metadata.

**Primitive**: Same.

**Translation**:
```rust
fn run_update_metadata(&self, run: &ApiRunId, metadata: Value) -> StrataResult<Version> {
    let versioned = self.run_index.update_metadata(run.as_str(), metadata)?;
    Ok(versioned.version)
}
```

**No gap** - direct mapping.

---

### `run_exists` - DIRECT MAPPING

**Substrate**: Returns `bool`.

**Primitive**: Same.

**Translation**:
```rust
fn run_exists(&self, run: &ApiRunId) -> StrataResult<bool> {
    Ok(self.run_index.exists(run.as_str())?)
}
```

**No gap** - direct mapping.

---

### `run_set_retention` - NOT IN PRIMITIVE

**Substrate**: Configures history retention.

**Primitive**: No retention policy support.

**Gap**: Must store retention policy separately or stub:
```rust
fn run_set_retention(&self, run: &ApiRunId, policy: RetentionPolicy) -> StrataResult<Version> {
    // Option 1: Store in metadata
    let mut meta_obj = /* get current metadata */;
    meta_obj.insert("_retention", policy.to_json());
    self.run_index.update_metadata(run.as_str(), meta_obj)?;

    // Option 2: Stub
    todo!("Retention policy not implemented in primitive")
}
```

---

### `run_get_retention` - NOT IN PRIMITIVE

**Substrate**: Gets retention policy.

**Primitive**: No retention policy support.

**Gap**: Return default or extract from metadata:
```rust
fn run_get_retention(&self, run: &ApiRunId) -> StrataResult<RetentionPolicy> {
    // Return default policy - retention not implemented
    Ok(RetentionPolicy::default())
}
```

---

## Summary Table

| Substrate Method | Primitive Method | Gap |
|-----------------|------------------|-----|
| `run_create` | `create_run_with_options` | Generate ID if not provided |
| `run_get` | `get_run` | Convert RunMetadata → RunInfo |
| `run_list` | `list_runs` + `get_run` | Re-fetch each, filter in substrate |
| `run_close` | `archive_run` | Validate not default |
| `run_update_metadata` | `update_metadata` | None - direct |
| `run_exists` | `exists` | None - direct |
| `run_set_retention` | ❌ None | Not implemented |
| `run_get_retention` | ❌ None | Not implemented |

## Gaps Requiring Primitive Enhancement

| Method | What's Needed |
|--------|---------------|
| `run_set_retention` | Add retention policy to RunMetadata |
| `run_get_retention` | Add retention policy to RunMetadata |
| `run_list` | Return `Vec<Versioned<RunMetadata>>` not `Vec<String>` |

## Gaps Handled in Substrate

| Method | How Handled |
|--------|-------------|
| `run_create` | Generate UUID if no ID provided |
| `run_close` | Map to `archive_run`, validate not default |
| `run_list` | Re-fetch each run, filter/paginate in substrate |
| RunState mapping | Map Active/Paused → Active, others → Closed |

## Design Decision Required

**Default Run Semantics**:
- Substrate says "default" run always exists and cannot be closed
- Primitive doesn't mention default run

**Options**:
1. **Auto-create default**: Substrate ensures "default" exists on first access
2. **Primitive creates default**: RunIndex creates "default" on construction
3. **Document as required**: User must create "default" run explicitly

**Recommendation**: Option 1 - substrate auto-creates on first access.

## Additional Primitive Capabilities

The primitive has features NOT exposed in substrate:

| Primitive Feature | Description | Substrate Equivalent? |
|-------------------|-------------|----------------------|
| `complete_run()` | Mark as completed | Close with "completed" state |
| `fail_run()` | Mark as failed with error | Close with "failed" state |
| `pause_run()` | Pause execution | ❌ Not exposed |
| `resume_run()` | Resume from pause | ❌ Not exposed |
| `cancel_run()` | Cancel execution | Close with "cancelled" state |
| `add_tags()` | Add tags | ❌ Not exposed |
| `remove_tags()` | Remove tags | ❌ Not exposed |
| `query_by_tag()` | Query by tag | ❌ Not exposed |
| `query_by_status()` | Query by status | Through `run_list(state)` |
| `get_child_runs()` | Get forked runs | ❌ Not exposed |
| `delete_run()` | Hard delete | ❌ Not exposed (dangerous) |

Consider exposing these in substrate for full lifecycle management.

## Notes on "default" Run

Per substrate documentation:
- "default" run always exists
- "default" run cannot be closed
- Operations without explicit run_id use "default"

Implementation strategy:
1. On substrate construction: check if "default" exists, create if not
2. On `run_close("default")`: return ConstraintViolation
3. On `run_delete("default")` (if exposed): return ConstraintViolation

---

## Contract Gap Summary

### Facade → Substrate: FULLY COVERED

| Facade | Substrate | Status |
|--------|-----------|--------|
| `runs()` | `run_list()` | ✅ |
| `use_run(run_id)` | Client-side binding | ✅ Returns scoped facade |

### Substrate → Primitive: GAPS EXIST

| Contract Method | Primitive Support | Gap |
|-----------------|-------------------|-----|
| `run_create` | `create_run_with_options()` ✅ | Generate ID if not provided |
| `run_get` | `get_run()` ✅ | Type conversion (RunMetadata → RunInfo) |
| `run_list` | `list_runs()` + `get_run()` ✅ | Re-fetch each run for metadata |
| `run_close` | `archive_run()` ✅ | Validate not default run |
| `retention_get` | ❌ **MISSING** | **Retention not in primitive** |
| `retention_set` | ❌ **MISSING** | **Retention not in primitive** |

### Critical Gaps

1. **Retention Policy**: Contract specifies `retention_get/set`, primitive has no retention support.
   - **Workaround:** Store in run metadata with reserved key `_retention`
   - **Long-term:** Add retention to primitive

2. **RunState Mapping**: Contract has simple `Active/Closed`, primitive has richer lifecycle.
   - `Active` → `RunStatus::Active` or `RunStatus::Paused`
   - `Closed` → Any of `Completed`, `Failed`, `Cancelled`, `Archived`

3. **Default Run Semantics**: Contract says default always exists, cannot be closed.
   - Substrate must auto-create on first access
   - Substrate must reject `run_close("default")`

### Default Run Implementation

```rust
// In substrate initialization
fn ensure_default_run(&self) -> StrataResult<()> {
    let default_id = ApiRunId::default();
    if !self.primitive.exists(default_id.as_str())? {
        self.primitive.create_run_with_options(
            "default",
            None,   // no parent
            vec![], // no tags
            Value::Object(Default::default()),
        )?;
    }
    Ok(())
}

// In run_close
fn run_close(&self, run: &ApiRunId) -> StrataResult<()> {
    if run.is_default() {
        return Err(StrataError::ConstraintViolation {
            reason: "run_closed".into(),
            message: "Cannot close default run".into(),
        });
    }
    self.primitive.archive_run(run.as_str())?;
    Ok(())
}
```

### Retention Workaround

Until primitive supports retention:

```rust
fn retention_set(&self, run: &ApiRunId, policy: RetentionPolicy) -> StrataResult<Version> {
    // Store in metadata with reserved key
    let mut meta = self.run_get(run)?.map(|r| r.metadata).unwrap_or_default();
    if let Value::Object(ref mut obj) = meta {
        obj.insert("_strata_retention".into(), policy.to_value());
    }
    self.primitive.update_metadata(run.as_str(), meta)?;
    // Return version
}

fn retention_get(&self, run: &ApiRunId) -> StrataResult<Option<Versioned<RetentionPolicy>>> {
    let info = self.run_get(run)?;
    match info {
        Some(versioned) => {
            if let Value::Object(obj) = &versioned.value.metadata {
                if let Some(policy_val) = obj.get("_strata_retention") {
                    let policy = RetentionPolicy::from_value(policy_val)?;
                    return Ok(Some(Versioned {
                        value: policy,
                        version: versioned.version,
                        timestamp: versioned.timestamp,
                    }));
                }
            }
            Ok(None) // No retention set, use default
        }
        None => Err(StrataError::NotFound { ... }),
    }
}
```
