# Facade → Substrate → Primitive Gap Analysis

This document identifies facade methods that are missing from either substrate or primitives.

## Summary

| Facade | Total Methods | Covered by Substrate | Missing from Substrate | Missing from Primitive |
|--------|---------------|---------------------|----------------------|----------------------|
| KVFacade | 14 | 11 | 3 | 2 |
| JsonFacade | 12 | 5 | 7 | 2 |
| EventFacade | 7 | 6 | 1 | Stream semantics |
| StateFacade | 5 | 5 | 0 | 1 |
| HistoryFacade | 3 | 3 | 0 | 2 |
| RunFacade | 2 | 2 | 0 | 0 |
| VectorFacade | 7 | 7 | 0 | 0 |
| TraceFacade | 9 | 9 | 0 | 3 |

---

## KVFacade Gaps

### Methods with Full Coverage
| Facade Method | Substrate Method | Primitive Method |
|---------------|------------------|------------------|
| `get(key)` | `kv_get` | `get` ✅ |
| `getv(key)` | `kv_get` | `get` ✅ |
| `set(key, value)` | `kv_put` | `put` ✅ |
| `del(key)` | `kv_delete` | `delete` ✅ |
| `exists(key)` | `kv_exists` | `exists` ✅ |
| `incr(key)` | `kv_incr` | Transaction ✅ |
| `incrby(key, delta)` | `kv_incr` | Transaction ✅ |
| `setnx(key, value)` | `kv_cas_version` | Transaction ✅ |
| `mget(keys)` | `kv_mget` | `get_many` ✅ |
| `mset(entries)` | `kv_mput` | Transaction ✅ |
| `mdel(keys)` | `kv_mdelete` | Transaction ✅ |

### Missing from Substrate

| Facade Method | Description | Resolution |
|---------------|-------------|------------|
| `get_with_options(key, options)` | Can request historical values | Uses `kv_get_at` if version specified |
| `set_with_options(key, value, options)` | NX/XX/GET flags | NX via `kv_cas_version`, XX/GET need substrate support |
| `getset(key, value)` | Atomic get-and-set | Needs `kv_getset` in substrate OR transaction |

### Missing from Primitive (affects substrate)

| Substrate Method | Why Missing |
|------------------|-------------|
| `kv_get_at` | Primitive doesn't expose `VersionChain` |
| `kv_history` | Primitive doesn't expose `VersionChain` |

---

## JsonFacade Gaps

### Methods with Full Coverage
| Facade Method | Substrate Method | Primitive Method |
|---------------|------------------|------------------|
| `json_get(key, path)` | `json_get` | `get` ✅ |
| `json_getv(key, path)` | `json_get` | `get` ✅ |
| `json_set(key, path, value)` | `json_set` | `create` + `set` ✅ |
| `json_del(key, path)` | `json_delete` | `delete_at_path` / `destroy` ✅ |
| `json_merge(key, path, patch)` | `json_merge` | ❌ (Substrate implements RFC 7396) |

### Missing from Substrate (7 methods!)

| Facade Method | Description | Resolution |
|---------------|-------------|------------|
| `json_type(key, path)` | Get value type at path | Add `json_type` to substrate |
| `json_numincrby(key, path, delta)` | Increment number at path | Add `json_numincrby` to substrate |
| `json_strappend(key, path, suffix)` | Append to string | Add `json_strappend` to substrate |
| `json_arrappend(key, path, values)` | Append to array | Add `json_arrappend` to substrate |
| `json_arrlen(key, path)` | Get array length | Add `json_arrlen` to substrate |
| `json_objkeys(key, path)` | Get object keys | Add `json_objkeys` to substrate |
| `json_objlen(key, path)` | Get object key count | Add `json_objlen` to substrate |

**Note**: These are RedisJSON-style convenience methods. They can all be implemented in substrate using `json_get` + `json_set`:

```rust
// json_type: Get value at path, return type string
fn json_type(&self, run: &ApiRunId, key: &str, path: &str) -> StrataResult<Option<String>> {
    let val = self.json_get(run, key, path)?;
    Ok(val.map(|v| v.value.type_name()))
}

// json_numincrby: Get, add delta, set back
fn json_numincrby(&self, run: &ApiRunId, key: &str, path: &str, delta: f64) -> StrataResult<f64> {
    // Transaction: get -> check is number -> add delta -> set
}
```

### Missing from Primitive

| Substrate Method | Why Missing |
|------------------|-------------|
| `json_merge` | Primitive has no merge operation |
| `json_history` | Same as KVStore |

---

## EventFacade Gaps

### Methods with Full Coverage
| Facade Method | Substrate Method | Primitive Method |
|---------------|------------------|------------------|
| `xadd(stream, payload)` | `event_append` | `append` ✅ |
| `xrange(stream, start, end)` | `event_range` | `read_range` + filter ✅ |
| `xrange_count(stream, start, end, count)` | `event_range(limit)` | `read_range` + filter ✅ |
| `xlen(stream)` | `event_len` | `len` + filter ✅ |
| `xlast(stream)` | `event_latest_sequence` | Scan required ✅ |
| `xget(stream, seq)` | `event_get` | `read` ✅ |

### Missing from Substrate

| Facade Method | Description | Resolution |
|---------------|-------------|------------|
| `xrevrange(stream, start, end)` | Reverse order read | Add `event_range_reverse` OR sort in facade |

### Semantic Gap: Stream Abstraction

The primitive doesn't have true stream support:
- Substrate: Named streams with independent sequences
- Primitive: Single log with `event_type` filtering

This is documented in `EVENTLOG_TRANSLATION.md`.

---

## StateFacade Gaps

### Methods with Full Coverage
| Facade Method | Substrate Method | Primitive Method |
|---------------|------------------|------------------|
| `state_get(cell)` | `state_get` | `read` ✅ |
| `state_set(cell, value)` | `state_set` | `set` ✅ |
| `state_cas(cell, exp, value)` | `state_cas` | `init` + `cas` ✅ |
| `state_del(cell)` | `state_delete` | `delete` ✅ |
| `state_exists(cell)` | `state_exists` | `exists` ✅ |

### No Missing from Substrate

### Missing from Primitive

| Substrate Method | Why Missing |
|------------------|-------------|
| `state_history` | Primitive doesn't expose version history |

---

## HistoryFacade Gaps

### Methods with Full Coverage
| Facade Method | Substrate Method | Primitive Method |
|---------------|------------------|------------------|
| `history(key, limit, before)` | `kv_history` | ❌ Not exposed |
| `get_at(key, version)` | `kv_get_at` | ❌ Not exposed |
| `latest_version(key)` | `kv_get().version` | ✅ |

### Missing from Primitive (Critical!)

| Substrate Method | Why Missing |
|------------------|-------------|
| `kv_history` | Primitive doesn't expose `VersionChain` iteration |
| `kv_get_at` | Primitive doesn't expose `get_at_version` |

**This is Strata's core value proposition - must be fixed in primitive.**

---

## RunFacade Gaps

### Methods with Full Coverage
| Facade Method | Substrate Method | Primitive Method |
|---------------|------------------|------------------|
| `runs()` | `run_list` | `list_runs` + `get_run` ✅ |
| `use_run(run_id)` | Client-side | N/A (scoping) ✅ |

### No Gaps

Note: Run creation/closure are intentionally substrate-only.

---

## VectorFacade Gaps

### Methods with Full Coverage
| Facade Method | Substrate Method | Primitive Method |
|---------------|------------------|------------------|
| `vadd(coll, key, vec, meta)` | `vector_upsert` | `insert` ✅ |
| `vget(coll, key)` | `vector_get` | `get` ✅ |
| `vdel(coll, key)` | `vector_delete` | `delete` ✅ |
| `vsim(coll, query, k)` | `vector_search` | `search` ✅ |
| `vsim_with_options(...)` | `vector_search(filter)` | `search(filter)` ✅ |
| `vcollection_info(coll)` | `vector_collection_info` | `get_collection` ✅ |
| `vcollection_drop(coll)` | `vector_drop_collection` | `delete_collection` ✅ |

### No Gaps

---

## TraceFacade Gaps

### Methods with Full Coverage
| Facade Method | Substrate Method | Primitive Method |
|---------------|------------------|------------------|
| `trace(kind, content)` | `trace_create` | `record` ✅ |
| `trace_with_options(...)` | `trace_create` / `trace_create_with_id` | `record` (no explicit ID) |
| `trace_child(parent, kind, content)` | `trace_create(parent)` | `record_child` ✅ |
| `trace_get(id)` | `trace_get` | `get` ✅ |
| `trace_list(kind, limit)` | `trace_list` | `query_by_type` ✅ |
| `trace_roots(limit)` | `trace_list(parent=None)` | `get_roots` ✅ |
| `trace_children(parent_id)` | `trace_children` | `get_children` ✅ |
| `trace_tag(id, tags)` | `trace_update_tags` | ❌ Not implemented |
| `trace_untag(id, tags)` | `trace_update_tags` | ❌ Not implemented |

### Missing from Primitive

| Substrate Method | Why Missing |
|------------------|-------------|
| `trace_create_with_id` | Primitive always generates UUID |
| `trace_update_tags` | Primitive is append-only |

---

## Critical Gaps Summary

### Must Fix in Primitive (Core Value Proposition)

1. **KVStore**: Expose `get_at_version()` and `VersionChain` iteration for history
2. **Same for**: JsonStore, StateCell (all need history exposure)

### Must Add to Substrate

1. **JsonStore**: 7 RedisJSON-style methods (`json_type`, `json_numincrby`, etc.)
2. **KVStore**: `set_with_options` (XX flag), `getset`
3. **EventLog**: `event_range_reverse` (or handle in facade)

### Design Decisions Needed

1. **Trace update_tags**: Primitive is append-only. Accept limitation or change primitive?
2. **Trace explicit IDs**: Primitive generates UUIDs. Allow explicit IDs?
3. **Event streams**: Accept shared sequence space or implement true streams?

---

## Implementation Priority

### P0 - Blocking (Core Value)
1. Primitive: Expose `get_at_version` for history/time-travel
2. Primitive: Expose `VersionChain` iteration for `kv_history`

### P1 - High (Feature Completeness)
1. Substrate: Add 7 JsonFacade methods
2. Substrate: Add `kv_getset`, `kv_set_with_options(XX)`

### P2 - Medium (Nice to Have)
1. Primitive: Support explicit trace IDs
2. Substrate: Add `event_range_reverse`

### P3 - Low (Deferred)
1. Primitive: Trace tag updates (may break append-only model)
2. Primitive: True stream support for EventLog
