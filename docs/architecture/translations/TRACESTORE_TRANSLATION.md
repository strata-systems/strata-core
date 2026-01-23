# TraceStore: Substrate → Primitive Translation

## Contract Promises (M11_CONTRACT.md)

### Facade API

**No facade trace operations.** Contract explicitly states:
> "Trace operations are **substrate-only** for M11."

### Substrate API (Section 11.8)

```rust
trace_record(run, trace_type: String, payload: Value) -> Version
trace_get(run, id) -> Option<Versioned<Value>>
trace_range(run, start?, end?, limit?) -> Vec<Versioned<Value>>
```

**Key points:**
- Trace is substrate-only (not exposed in facade)
- Simple API: record, get, range
- No parent/child relationships in contract
- No tags in contract

---

## Substrate API Promises

| Method | Signature | Purpose |
|--------|-----------|---------|
| `trace_create` | `(run, type, parent, content, tags) → (String, Version)` | Create trace |
| `trace_create_with_id` | `(run, id, type, parent, content, tags) → Version` | Create with explicit ID |
| `trace_get` | `(run, id) → Option<Versioned<TraceEntry>>` | Get trace by ID |
| `trace_list` | `(run, type?, parent?, tag?, limit?, before?) → Vec<Versioned<TraceEntry>>` | List with filters |
| `trace_children` | `(run, parent_id) → Vec<Versioned<TraceEntry>>` | Get child traces |
| `trace_tree` | `(run, root_id) → Vec<Versioned<TraceEntry>>` | Get trace subtree |
| `trace_update_tags` | `(run, id, add, remove) → Version` | Update tags |

## Primitive Provides

| Method | Signature | Version Exposed? |
|--------|-----------|------------------|
| `record(run_id, trace_type, tags, metadata)` | `→ Result<Versioned<String>>` | ✅ Yes (Txn) |
| `record_child(run_id, parent_id, trace_type, tags, metadata)` | `→ Result<Versioned<String>>` | ✅ Yes (Txn) |
| `get(run_id, trace_id)` | `→ Result<Option<Versioned<Trace>>>` | ✅ Yes |
| `exists(run_id, trace_id)` | `→ Result<bool>` | ❌ No |
| `query_by_type(run_id, type_name)` | `→ Result<Vec<Trace>>` | ❌ No (returns Trace not Versioned) |
| `query_by_tag(run_id, tag)` | `→ Result<Vec<Trace>>` | ❌ No |
| `get_children(run_id, parent_id)` | `→ Result<Vec<Trace>>` | ❌ No |
| `get_tree(run_id, root_id)` | `→ Result<Option<TraceTree>>` | ❌ No |
| `get_roots(run_id)` | `→ Result<Vec<Trace>>` | ❌ No |
| `list(run_id)` | `→ Result<Vec<Trace>>` | ❌ No |

## Type Differences

| Substrate | Primitive | Conversion Needed |
|-----------|-----------|-------------------|
| `ApiRunId` | `RunId` | `run.to_run_id()` |
| `substrate::TraceType` | `primitives::TraceType` | Different enums - mapping needed |
| `substrate::TraceEntry` | `primitives::Trace` | Different field names |
| `content: Value` | `metadata: Value` | Field rename |
| `Vec<Versioned<TraceEntry>>` | `Vec<Trace>` | Wrap in Versioned |

### TraceType Mapping

**Substrate TraceType** (simple):
```rust
enum TraceType {
    Thought, Action, Observation, Tool, Message, Custom(String)
}
```

**Primitive TraceType** (rich):
```rust
enum TraceType {
    ToolCall { tool_name, arguments, result, duration_ms },
    Decision { question, options, chosen, reasoning },
    Query { query_type, query, results_count },
    Thought { content, confidence },
    Error { error_type, message, recoverable },
    Custom { name, data },
}
```

**Mapping**:
- `Thought` → `TraceType::Thought { content, confidence: None }`
- `Action` → `TraceType::Custom { name: "Action", data }`
- `Observation` → `TraceType::Custom { name: "Observation", data }`
- `Tool` → `TraceType::ToolCall { ... }`
- `Message` → `TraceType::Custom { name: "Message", data }`
- `Custom(name)` → `TraceType::Custom { name, data }`

---

## Method Translations

### `trace_create` - TYPE MAPPING + RETURN FORMAT

**Substrate**: Returns `(trace_id: String, version: Version)`.

**Primitive**: `record()` returns `Versioned<String>`.

**Translation**:
```rust
fn trace_create(
    &self,
    run: &ApiRunId,
    trace_type: substrate::TraceType,
    parent_id: Option<&str>,
    content: Value,
    tags: Vec<String>,
) -> StrataResult<(String, Version)> {
    let run_id = run.to_run_id();

    // Map substrate TraceType → primitive TraceType
    let prim_type = map_trace_type(trace_type, content.clone());

    let versioned = match parent_id {
        Some(pid) => self.trace.record_child(&run_id, pid, prim_type, tags, content)?,
        None => self.trace.record(&run_id, prim_type, tags, content)?,
    };

    Ok((versioned.value, versioned.version))
}

fn map_trace_type(t: substrate::TraceType, content: Value) -> primitives::TraceType {
    match t {
        substrate::TraceType::Thought => primitives::TraceType::Thought {
            content: content.to_string(),
            confidence: None,
        },
        substrate::TraceType::Tool => primitives::TraceType::ToolCall {
            tool_name: extract_tool_name(&content),
            arguments: content.clone(),
            result: None,
            duration_ms: None,
        },
        substrate::TraceType::Action => primitives::TraceType::Custom {
            name: "Action".to_string(),
            data: content,
        },
        substrate::TraceType::Observation => primitives::TraceType::Custom {
            name: "Observation".to_string(),
            data: content,
        },
        substrate::TraceType::Message => primitives::TraceType::Custom {
            name: "Message".to_string(),
            data: content,
        },
        substrate::TraceType::Custom(name) => primitives::TraceType::Custom {
            name,
            data: content,
        },
    }
}
```

---

### `trace_create_with_id` - NOT IN PRIMITIVE

**Substrate**: Creates trace with caller-provided ID.

**Primitive**: Always generates UUID.

**Gap**: Must implement in substrate by bypassing `record()`:
```rust
fn trace_create_with_id(
    &self,
    run: &ApiRunId,
    id: &str,
    trace_type: substrate::TraceType,
    parent_id: Option<&str>,
    content: Value,
    tags: Vec<String>,
) -> StrataResult<Version> {
    // Must implement directly using database transaction
    // Primitive doesn't support explicit IDs
    todo!("Requires database-level implementation")
}
```

---

### `trace_get` - TYPE CONVERSION

**Substrate**: Returns `Versioned<TraceEntry>`.

**Primitive**: Returns `Versioned<Trace>`.

**Translation**:
```rust
fn trace_get(&self, run: &ApiRunId, id: &str) -> StrataResult<Option<Versioned<TraceEntry>>> {
    let run_id = run.to_run_id();

    match self.trace.get(&run_id, id)? {
        Some(versioned) => {
            let trace = versioned.value;
            let entry = TraceEntry {
                id: trace.id,
                trace_type: map_trace_type_reverse(&trace.trace_type),
                parent_id: trace.parent_id,
                content: trace.metadata,
                tags: trace.tags,
                created_at: trace.timestamp as u64,
            };
            Ok(Some(Versioned {
                value: entry,
                version: versioned.version,
                timestamp: versioned.timestamp,
            }))
        }
        None => Ok(None),
    }
}
```

---

### `trace_list` - COMPOSITE QUERY

**Substrate**: Complex filter with type, parent, tag, limit, before.

**Primitive**: Separate query_by_* methods, no combined query.

**Translation**:
```rust
fn trace_list(
    &self,
    run: &ApiRunId,
    trace_type: Option<substrate::TraceType>,
    parent_id: Option<Option<&str>>,  // Some(None) = roots only
    tag: Option<&str>,
    limit: Option<u64>,
    before: Option<Version>,
) -> StrataResult<Vec<Versioned<TraceEntry>>> {
    let run_id = run.to_run_id();

    // Start with all traces or filter by most selective criterion
    let mut traces: Vec<Trace> = if let Some(t) = trace_type {
        let type_name = t.type_name();
        self.trace.query_by_type(&run_id, &type_name)?
    } else if let Some(t) = tag {
        self.trace.query_by_tag(&run_id, t)?
    } else if let Some(maybe_parent) = parent_id {
        match maybe_parent {
            Some(pid) => self.trace.get_children(&run_id, pid)?,
            None => self.trace.get_roots(&run_id)?,
        }
    } else {
        self.trace.list(&run_id)?
    };

    // Apply remaining filters
    if let Some(t) = trace_type {
        traces.retain(|tr| tr.trace_type.type_name() == t.type_name());
    }
    if let Some(t) = tag {
        traces.retain(|tr| tr.tags.contains(&t.to_string()));
    }
    if let Some(maybe_parent) = parent_id {
        traces.retain(|tr| match maybe_parent {
            Some(pid) => tr.parent_id.as_deref() == Some(pid),
            None => tr.parent_id.is_none(),
        });
    }

    // Sort newest first
    traces.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    // Apply limit
    if let Some(n) = limit {
        traces.truncate(n as usize);
    }

    // TODO: Apply 'before' filter (requires version info not in query results)

    // Convert to Versioned<TraceEntry>
    // Note: We lose version info here - primitive query methods don't return Versioned
    let entries: Vec<_> = traces
        .into_iter()
        .map(|t| {
            Versioned {
                value: trace_to_entry(t),
                version: Version::txn(0), // FIXME: Version lost in query
                timestamp: Timestamp::now(),
            }
        })
        .collect();

    Ok(entries)
}
```

**Gap**: Primitive query methods return `Vec<Trace>` not `Vec<Versioned<Trace>>`, losing version info.

---

### `trace_children` - NEEDS VERSIONED RETURN

**Substrate**: Returns `Vec<Versioned<TraceEntry>>`.

**Primitive**: `get_children` returns `Vec<Trace>`.

**Translation**:
```rust
fn trace_children(&self, run: &ApiRunId, parent_id: &str) -> StrataResult<Vec<Versioned<TraceEntry>>> {
    let run_id = run.to_run_id();

    let children = self.trace.get_children(&run_id, parent_id)?;

    // FIXME: Version info lost
    Ok(children
        .into_iter()
        .map(|t| Versioned {
            value: trace_to_entry(t),
            version: Version::txn(0),
            timestamp: Timestamp::now(),
        })
        .collect())
}
```

---

### `trace_tree` - FLATTEN TREE

**Substrate**: Returns flat `Vec<Versioned<TraceEntry>>` in pre-order.

**Primitive**: Returns `TraceTree` (nested structure).

**Translation**:
```rust
fn trace_tree(&self, run: &ApiRunId, root_id: &str) -> StrataResult<Vec<Versioned<TraceEntry>>> {
    let run_id = run.to_run_id();

    let tree = self.trace.get_tree(&run_id, root_id)?
        .ok_or_else(|| StrataError::NotFound)?;

    // Flatten tree in pre-order
    fn flatten(tree: TraceTree, result: &mut Vec<Trace>) {
        result.push(tree.trace);
        for child in tree.children {
            flatten(child, result);
        }
    }

    let mut traces = Vec::new();
    flatten(tree, &mut traces);

    // FIXME: Version info lost
    Ok(traces
        .into_iter()
        .map(|t| Versioned {
            value: trace_to_entry(t),
            version: Version::txn(0),
            timestamp: Timestamp::now(),
        })
        .collect())
}
```

---

### `trace_update_tags` - NOT IN PRIMITIVE

**Substrate**: Add/remove tags from existing trace.

**Primitive**: No update operation (traces are append-only).

**Gap**: Must implement in substrate:
```rust
fn trace_update_tags(
    &self,
    run: &ApiRunId,
    id: &str,
    add_tags: Vec<String>,
    remove_tags: Vec<String>,
) -> StrataResult<Version> {
    // Would need to:
    // 1. Read existing trace
    // 2. Modify tags
    // 3. Rewrite trace and update indices
    // This breaks append-only semantics!
    todo!("Requires primitive modification")
}
```

---

## Summary Table

| Substrate Method | Primitive Method | Gap |
|-----------------|------------------|-----|
| `trace_create` | `record` / `record_child` | TraceType mapping |
| `trace_create_with_id` | ❌ None | Must implement in substrate |
| `trace_get` | `get` | Type conversion |
| `trace_list` | `query_by_*` + filter | Combined query not available, version lost |
| `trace_children` | `get_children` | Returns Trace not Versioned |
| `trace_tree` | `get_tree` | Flatten tree, version lost |
| `trace_update_tags` | ❌ None | Append-only - no updates |

## Gaps Requiring Primitive Enhancement

| Method | What's Needed |
|--------|---------------|
| `trace_create_with_id` | Support explicit trace IDs |
| `trace_update_tags` | Support tag modification (or accept append-only) |
| All query methods | Return `Vec<Versioned<Trace>>` not `Vec<Trace>` |

## Gaps Handled in Substrate

| Method | How Handled |
|--------|-------------|
| `trace_create` | Map TraceType, use record/record_child |
| `trace_get` | Convert Trace → TraceEntry |
| `trace_list` | Combine query_by_* methods + post-filter |
| `trace_tree` | Flatten TraceTree to Vec |

## Design Decision Required

**TraceType Mapping**: The substrate has simple types (Thought, Action, etc.) while the primitive has rich structured types (ToolCall with fields, Decision with fields, etc.).

Options:
1. **Substrate adopts rich types**: Change substrate TraceType to match primitive
2. **Primitive supports simple types**: Add simple variants to primitive TraceType
3. **Keep both**: Map at translation layer (current approach, lossy)

**Recommendation**: Option 1 - substrate should expose the rich TraceType from primitive.

## Additional Notes

1. **Append-Only Semantics**: Primitive TraceStore is designed as append-only (like EventLog). `trace_update_tags` breaks this contract.

2. **Version Loss**: Query methods in primitive return unwrapped `Trace` not `Versioned<Trace>`. This loses version info that substrate promises.

3. **Index Types**: Primitive has by-type, by-tag, by-parent, by-time indices. Substrate could leverage these more directly.

---

## Contract Gap Summary

### Facade → Substrate: N/A

**Trace is substrate-only per contract.** No facade operations defined.

### Substrate → Primitive: GAPS EXIST

Contract defines minimal API, but primitive has richer features:

| Contract Method | Primitive Support | Gap |
|-----------------|-------------------|-----|
| `trace_record` | `record()` ✅ | TraceType mapping |
| `trace_get` | `get()` ✅ | Type conversion |
| `trace_range` | `list()` + filter ✅ | No direct range support |

### Contract vs Implementation Scope

**Contract (Section 11.8) specifies only:**
```rust
trace_record(run, trace_type: String, payload: Value) -> Version
trace_get(run, id) -> Option<Versioned<Value>>
trace_range(run, start?, end?, limit?) -> Vec<Versioned<Value>>
```

**Primitive provides much more:**
- Parent/child relationships (`record_child`, `get_children`, `get_tree`)
- Tags (`query_by_tag`)
- Type queries (`query_by_type`)
- Root trace access (`get_roots`)

### Recommendation

For M11, implement **contract minimum**:
1. `trace_record` → maps to `primitive.record()`
2. `trace_get` → maps to `primitive.get()`
3. `trace_range` → maps to `primitive.list()` with filtering

**Extended features** (parent/child, tags) can be:
- Exposed in later milestone
- Available through primitive directly for internal use
- Not part of public substrate API for M11

### TraceType Simplification

Contract uses simple `trace_type: String`. Primitive has rich enum:

```rust
// Contract: simple string
trace_record(run, "thought", payload)

// Primitive: rich enum
TraceType::Thought { content, confidence }
TraceType::ToolCall { tool_name, arguments, result, duration_ms }
```

**Translation:** Treat contract's `trace_type` as category name, store payload as metadata.

```rust
fn trace_record(run, trace_type: &str, payload: Value) -> Version {
    let prim_type = TraceType::Custom {
        name: trace_type.to_string(),
        data: payload,
    };
    self.primitive.record(run_id, prim_type, vec![], Value::Null)
}
```
