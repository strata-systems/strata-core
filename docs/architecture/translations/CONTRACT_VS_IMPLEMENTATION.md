# M11 Contract vs Actual Facade Implementation

This document compares what the M11 contract specifies versus what the facade trait files actually define.

## Summary

The facade trait files have **expanded beyond** what the M11 contract specifies. This needs resolution:
- Either the contract is the "frozen minimum" and extras are allowed
- Or the facade files have unauthorized additions
- Or the contract needs updating

---

## KV Facade

### M11 Contract Says (Section 10.1)

| Operation | Signature | Notes |
|-----------|-----------|-------|
| `set` | `(key, value) → ()` | Overwrites |
| `get` | `(key) → Option<Value>` | Returns latest |
| `getv` | `(key) → Option<Versioned<Value>>` | Escape hatch |
| `mget` | `(keys[]) → Vec<Option<Value>>` | Order preserved |
| `mset` | `(entries) → ()` | Atomic |
| `delete` | `(keys[]) → u64` | Count existed |
| `exists` | `(key) → bool` | |
| `exists_many` | `(keys[]) → u64` | Count |
| `incr` | `(key, delta=1) → i64` | Atomic |

### Actual Facade Trait Has

| Method | In Contract? | Notes |
|--------|--------------|-------|
| `get` | ✅ | |
| `getv` | ✅ | |
| `get_with_options` | ❌ NO | Extra |
| `set` | ✅ | |
| `set_with_options` | ❌ NO | Extra (NX/XX/GET flags) |
| `del` | ✅ | Named `del` not `delete` |
| `exists` | ✅ | |
| `incr` | ✅ | |
| `incrby` | ❌ NO | Extra |
| `incr_with_options` | ❌ NO | Extra |
| `decr` | ❌ NO | Extra (default impl) |
| `decrby` | ❌ NO | Extra (default impl) |
| `setnx` | ❌ NO | Extra |
| `getset` | ❌ NO | Extra |
| `mget` | ✅ | In KVFacadeBatch |
| `mset` | ✅ | In KVFacadeBatch |
| `mdel` | ❌ NO | Extra (contract has `delete`) |
| `mexists` | ✅ | Named `mexists` not `exists_many` |

**Issues:**
- Contract says `delete(keys[])`, impl has `del(key)` + `mdel(keys[])`
- Contract says `exists_many`, impl has `mexists`
- 8 extra methods not in contract

---

## JSON Facade

### M11 Contract Says (Section 10.2)

| Operation | Signature | Notes |
|-----------|-----------|-------|
| `json_set` | `(key, path, value) → ()` | |
| `json_get` | `(key, path) → Option<Value>` | |
| `json_getv` | `(key, path) → Option<Versioned<Value>>` | Document-level version |
| `json_del` | `(key, path) → u64` | Count removed |
| `json_merge` | `(key, path, value) → ()` | RFC 7396 |

### Actual Facade Trait Has

| Method | In Contract? | Notes |
|--------|--------------|-------|
| `json_get` | ✅ | |
| `json_getv` | ✅ | |
| `json_set` | ✅ | |
| `json_del` | ✅ | |
| `json_merge` | ✅ | |
| `json_type` | ❌ NO | Extra - get type name |
| `json_numincrby` | ❌ NO | Extra - increment number |
| `json_strappend` | ❌ NO | Extra - append string |
| `json_arrappend` | ❌ NO | Extra - append to array |
| `json_arrlen` | ❌ NO | Extra - array length |
| `json_objkeys` | ❌ NO | Extra - object keys |
| `json_objlen` | ❌ NO | Extra - object key count |

**Issues:**
- 7 extra methods not in contract (RedisJSON-style operations)

---

## Event Facade

### M11 Contract Says (Section 10.3)

| Operation | Signature | Notes |
|-----------|-----------|-------|
| `xadd` | `(stream, payload: Object) → Version` | |
| `xrange` | `(stream, start?, end?, limit?) → Vec<Versioned<Value>>` | |
| `xlen` | `(stream) → u64` | |

### Actual Facade Trait Has

| Method | In Contract? | Notes |
|--------|--------------|-------|
| `xadd` | ✅ | |
| `xrange` | ✅ | |
| `xrange_count` | ❌ NO | Extra - with limit |
| `xrevrange` | ❌ NO | Extra - reverse order |
| `xlen` | ✅ | |
| `xlast` | ❌ NO | Extra - latest sequence |
| `xget` | ❌ NO | Extra - get by sequence |

**Issues:**
- 4 extra methods not in contract

---

## Vector Facade

### M11 Contract Says (Section 10.4)

| Operation | Signature | Notes |
|-----------|-----------|-------|
| `vset` | `(key, vector, metadata) → ()` | |
| `vget` | `(key) → Option<Versioned<VectorEntry>>` | Returns Versioned |
| `vdel` | `(key) → bool` | |

### Actual Facade Trait Has

| Method | In Contract? | Notes |
|--------|--------------|-------|
| `vadd` | ⚠️ RENAMED | Contract says `vset` |
| `vget` | ✅ | But returns `Option<(Vec<f32>, Value)>` not `Versioned` |
| `vdel` | ✅ | |
| `vsim` | ❌ NO | Extra - similarity search |
| `vsim_with_options` | ❌ NO | Extra - search with filter |
| `vcollection_info` | ❌ NO | Extra |
| `vcollection_drop` | ❌ NO | Extra |

**Issues:**
- `vset` renamed to `vadd`
- `vget` return type differs (no `Versioned` in impl!)
- 4 extra methods for search/collections

---

## State Facade

### M11 Contract Says (Section 10.5)

| Operation | Signature | Notes |
|-----------|-----------|-------|
| `cas_set` | `(key, expected, new) → bool` | CAS |
| `cas_get` | `(key) → Option<Value>` | |

### Actual Facade Trait Has

| Method | In Contract? | Notes |
|--------|--------------|-------|
| `state_get` | ⚠️ RENAMED | Contract says `cas_get` |
| `state_set` | ❌ NO | Extra - unconditional set |
| `state_cas` | ⚠️ RENAMED | Contract says `cas_set` |
| `state_del` | ❌ NO | Extra |
| `state_exists` | ❌ NO | Extra |

**Issues:**
- Naming mismatch: `cas_set`/`cas_get` vs `state_cas`/`state_get`
- 3 extra methods

---

## History Facade

### M11 Contract Says (Section 10.6)

| Operation | Signature | Notes |
|-----------|-----------|-------|
| `history` | `(key, limit?, before?) → Vec<Versioned<Value>>` | |
| `get_at` | `(key, version) → Value \| HistoryTrimmed` | |
| `latest_version` | `(key) → Option<Version>` | |

### Actual Facade Trait Has

| Method | In Contract? | Notes |
|--------|--------------|-------|
| `history` | ✅ | |
| `get_at` | ✅ | |
| `latest_version` | ✅ | |

**No issues** - matches contract.

---

## Run Facade

### M11 Contract Says (Section 10.7)

| Operation | Signature | Notes |
|-----------|-----------|-------|
| `runs` | `() → Vec<RunInfo>` | |
| `use_run` | `(run_id) → ScopedFacade` | |

### Actual Facade Trait Has

| Method | In Contract? | Notes |
|--------|--------------|-------|
| `runs` | ✅ | |
| `use_run` | ✅ | |

**No issues** - matches contract.

---

## Trace Facade

### M11 Contract Says

> "Trace operations are **substrate-only** for M11."

### Actual Implementation

Full TraceFacade trait with 9 methods:
- `trace`, `trace_with_options`, `trace_child`
- `trace_get`, `trace_list`, `trace_roots`, `trace_children`
- `trace_tag`, `trace_untag`

**Issue:** Contract says no trace facade, but one exists.

---

## Capabilities

### M11 Contract Says (Section 10.8)

| Operation | Signature | Notes |
|-----------|-----------|-------|
| `capabilities` | `() → Capabilities` | System info |

### Actual Implementation

Not found as separate trait in facade files read.

**Issue:** Missing from facade traits.

---

## Critical Discrepancies Summary

| Issue | Severity | Resolution Needed |
|-------|----------|-------------------|
| **7 extra JSON methods** | HIGH | Are these in contract scope? |
| **TraceFacade exists** | HIGH | Contract says substrate-only |
| **Naming mismatches** | MEDIUM | `vset`→`vadd`, `cas_*`→`state_*`, `delete`→`del` |
| **vget loses Versioned** | HIGH | Contract says returns `Versioned<VectorEntry>` |
| **8 extra KV methods** | MEDIUM | `setnx`, `getset`, `*_with_options`, etc. |
| **4 extra Event methods** | MEDIUM | `xrevrange`, `xlast`, `xget`, `xrange_count` |
| **Missing capabilities trait** | MEDIUM | Specified in contract |

---

## Recommendations

### Option A: Contract is Minimum, Extras Allowed
- Document that contract specifies minimum required operations
- Extra methods are allowed as convenience
- Update contract to note this policy

### Option B: Align Implementation to Contract
- Remove or deprecate extra methods
- Fix naming to match contract exactly
- Remove TraceFacade (move to substrate-only)

### Option C: Update Contract to Match Implementation
- Add all extra methods to contract
- Update naming in contract
- Add TraceFacade to contract

**Recommended: Option A with selective Option C**
- Keep extra convenience methods (they're useful)
- Fix critical issues: naming, return types
- Update contract to reflect actual API
- Keep TraceFacade but document it as "extended facade"
