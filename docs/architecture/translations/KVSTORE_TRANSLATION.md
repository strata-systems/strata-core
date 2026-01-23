# KVStore: Substrate → Primitive Translation

## Contract Promises (M11_CONTRACT.md)

### Facade API (Section 10.1)

| Operation | Signature | Return | Notes |
|-----------|-----------|--------|-------|
| `set` | `(key, value)` | `()` | Overwrites, creates new version internally |
| `get` | `(key)` | `Option<Value>` | Returns latest value or None |
| `getv` | `(key)` | `Option<Versioned<Value>>` | Escape hatch for version info |
| `mget` | `(keys[])` | `Vec<Option<Value>>` | Order preserved, None for missing |
| `mset` | `(entries)` | `()` | Atomic multi-set |
| `delete` | `(keys[])` | `u64` | Count of keys that **existed** |
| `exists` | `(key)` | `bool` | Human-friendly boolean |
| `exists_many` | `(keys[])` | `u64` | Count of keys that exist |
| `incr` | `(key, delta=1)` | `i64` | **Atomic** increment, missing = 0 |

### Substrate API (Section 11.3)

```rust
kv_put(run, key, value) -> Version
kv_get(run, key) -> Option<Versioned<Value>>
kv_get_at(run, key, version) -> Versioned<Value> | HistoryTrimmed
kv_delete(run, key) -> bool
kv_exists(run, key) -> bool
kv_history(run, key, limit?, before?) -> Vec<Versioned<Value>>
kv_incr(run, key, delta) -> i64  // atomic
kv_cas_version(run, key, expected_version, new_value) -> bool
kv_cas_value(run, key, expected_value, new_value) -> bool
```

### History Facade (Section 10.6) - KV Only

| Operation | Signature | Return | Notes |
|-----------|-----------|--------|-------|
| `history` | `(key, limit?, before?)` | `Vec<Versioned<Value>>` | All retained versions, newest first |
| `get_at` | `(key, version)` | `Value \| HistoryTrimmed` | Point-in-time read |
| `latest_version` | `(key)` | `Option<Version>` | Get version without value |

---

## The Problem

Strata has a **two-layer API architecture**:

```
┌─────────────────────────────────────────────────────────────┐
│  Facade API (Redis-like)                                    │
│  - Implicit default run                                     │
│  - Auto-commit                                              │
│  - Strips version info (returns just Value)                 │
│  - Simple: set(key, value), get(key)                        │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ desugars to
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  Substrate API (Power User)                                 │
│  - Explicit run_id                                          │
│  - Explicit transactions                                    │
│  - Full version info (returns Versioned<Value>)             │
│  - History access, CAS, point-in-time reads                 │
│  - This is Strata's VALUE PROPOSITION                       │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ delegates to
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  Primitives Layer                                           │
│  - KVStore, JsonStore, EventLog, StateCell, etc.            │
│  - Stateless facades over Database/Storage                  │
│  - Should expose full versioning capabilities               │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ uses
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  Storage Layer (MVCC)                                       │
│  - VersionChain: stores ALL versions of each key            │
│  - get_at_version(max_version): read historical version     │
│  - Full MVCC with snapshot isolation                        │
└─────────────────────────────────────────────────────────────┘
```

## The Gap

The **Substrate API** promises these KV operations:

| Method | Purpose | Strata's Value |
|--------|---------|----------------|
| `kv_get` | Get latest value with version | ✓ Version info |
| `kv_get_at` | Get value at specific version | ✓ Time travel |
| `kv_history` | List all versions of a key | ✓ Full audit trail |
| `kv_put` | Write with version returned | ✓ Version tracking |
| `kv_incr` | Atomic increment | ✓ Transactions |
| `kv_cas_version` | Compare-and-swap by version | ✓ Optimistic concurrency |
| `kv_cas_value` | Compare-and-swap by value | ✓ Optimistic concurrency |

The **KVStore Primitive** currently provides:

| Method | Returns | Version Exposed? |
|--------|---------|------------------|
| `get(run_id, key)` | `Option<Versioned<Value>>` | ✅ Yes |
| `put(run_id, key, value)` | `Version` | ✅ Yes |
| `delete(run_id, key)` | `bool` | ❌ No version |
| `exists(run_id, key)` | `bool` | ❌ No version |
| `get_many(run_id, keys)` | `Vec<Option<Versioned<Value>>>` | ✅ Yes |

**Missing from Primitive:**

| Method | Why It's Needed |
|--------|-----------------|
| `get_at(run_id, key, version)` | Read historical version - **time travel** |
| `history(run_id, key, limit, before)` | List version history - **audit trail** |

The **Storage Layer** HAS the capability:

```rust
// In VersionChain (storage/src/sharded.rs)
pub fn get_at_version(&self, max_version: u64) -> Option<&StoredValue>

// In Storage trait (core/src/traits.rs)
fn get_versioned(&self, key: &Key, max_version: u64) -> Result<Option<VersionedValue>>
```

But the **Primitive doesn't expose it**.

## What Needs to Happen

### Option A: Add methods to KVStore Primitive

```rust
// In primitives/src/kv.rs

impl KVStore {
    /// Get value at specific version (time travel)
    pub fn get_at(&self, run_id: &RunId, key: &str, version: Version)
        -> Result<Option<Versioned<Value>>>
    {
        use strata_core::traits::Storage;
        let storage_key = self.key_for(run_id, key);
        let version_num = version.as_u64();
        self.db.storage().get_versioned(&storage_key, version_num)
    }

    /// Get version history for a key
    pub fn history(&self, run_id: &RunId, key: &str, limit: Option<u64>, before: Option<Version>)
        -> Result<Vec<Versioned<Value>>>
    {
        // Needs VersionChain to expose iteration
        // Or scan with decreasing version bounds
        todo!("Requires VersionChain.iter() or similar")
    }
}
```

### Option B: Substrate accesses Storage directly

Not recommended - breaks the abstraction layers.

## Current Translation Table

### Methods with Direct Primitive Support

| Substrate | Primitive | Translation |
|-----------|-----------|-------------|
| `kv_put(run, key, value)` | `kv.put(run_id, key, value)` | `run.to_run_id()` + direct call |
| `kv_get(run, key)` | `kv.get(run_id, key)` | `run.to_run_id()` + direct call |
| `kv_delete(run, key)` | `kv.delete(run_id, key)` | `run.to_run_id()` + direct call |
| `kv_exists(run, key)` | `kv.exists(run_id, key)` | `run.to_run_id()` + direct call |
| `kv_mget(run, keys)` | `kv.get_many(run_id, keys)` | `run.to_run_id()` + direct call |

### Methods Needing Transaction

| Substrate | Implementation |
|-----------|----------------|
| `kv_incr(run, key, delta)` | Transaction: get → check type → add → put |
| `kv_cas_version(run, key, expected, new)` | Transaction: get → check version → put |
| `kv_cas_value(run, key, expected, new)` | Transaction: get → check value → put |
| `kv_mput(run, entries)` | Transaction: put each entry |
| `kv_mdelete(run, keys)` | Transaction: delete each, count |

### Methods Needing Primitive Enhancement

| Substrate | Primitive Needed | Storage Has It? |
|-----------|------------------|-----------------|
| `kv_get_at(run, key, version)` | `kv.get_at(run_id, key, version)` | ✅ `Storage::get_versioned()` |
| `kv_history(run, key, limit, before)` | `kv.history(run_id, key, ...)` | ⚠️ `VersionChain` stores it, needs exposure |

## Type Conversions

| Substrate Type | Primitive Type | Conversion |
|----------------|----------------|------------|
| `ApiRunId` | `RunId` | `run.to_run_id()` |
| `StrataResult<T>` | `Result<T>` | `result.map_err(StrataError::from)` |
| `Value` | `Value` | Same type |
| `Version` | `Version` | Same type |
| `Versioned<T>` | `Versioned<T>` | Same type |

## Error Handling

Lightweight - just convert:

```rust
fn convert_error(err: strata_core::error::Error) -> StrataError {
    StrataError::from(err)
}
```

## Summary

1. **5 methods** have direct primitive support (put, get, delete, exists, mget)
2. **5 methods** need transaction-based implementation (incr, cas_version, cas_value, mput, mdelete)
3. **2 methods** need primitive enhancement (get_at, history) - storage has capability, primitive doesn't expose it
4. **Versions ARE exposed** via `Versioned<T>` - this is working
5. **History/time-travel NOT exposed** - this is the gap

---

## Contract Gap Summary

### Facade → Substrate: FULLY COVERED

All facade KV operations desugar to substrate operations per Section 12.1:

| Facade | Substrate | Status |
|--------|-----------|--------|
| `set(key, value)` | `begin(); kv_put(default, key, value); commit()` | ✅ |
| `get(key)` | `kv_get(default, key).map(\|v\| v.value)` | ✅ |
| `getv(key)` | `kv_get(default, key)` | ✅ |
| `mget(keys)` | `batch { kv_get(default, k) }` | ✅ |
| `mset(entries)` | `begin(); for (k,v): kv_put(...); commit()` | ✅ |
| `delete(keys)` | `begin(); for k: kv_delete(...); commit()` | ✅ |
| `exists(key)` | `kv_get(default, key).is_some()` | ✅ |
| `exists_many(keys)` | `keys.filter(\|k\| exists(k)).count()` | ✅ |
| `incr(key, delta)` | `kv_incr(default, key, delta)` | ✅ |
| `history(key, ...)` | `kv_history(default, key, ...)` | ✅ |
| `get_at(key, version)` | `kv_get_at(default, key, version)` | ✅ |
| `latest_version(key)` | `kv_get(default, key).map(\|v\| v.version)` | ✅ |

### Substrate → Primitive: GAPS EXIST

| Substrate Method | Primitive Support | Gap |
|------------------|-------------------|-----|
| `kv_put` | `put()` ✅ | None |
| `kv_get` | `get()` ✅ | None |
| `kv_delete` | `delete()` ✅ | None |
| `kv_exists` | `exists()` ✅ | None |
| `kv_incr` | Transaction ✅ | Must implement in substrate |
| `kv_cas_version` | Transaction ✅ | Must implement in substrate |
| `kv_cas_value` | Transaction ✅ | Must implement in substrate |
| `kv_get_at` | ❌ **MISSING** | **P0: Primitive must expose get_at_version** |
| `kv_history` | ❌ **MISSING** | **P0: Primitive must expose VersionChain iteration** |

### Critical Gaps (P0)

1. **`kv_get_at`**: Contract promises point-in-time reads. Storage has `get_at_version()`. Primitive doesn't expose it.
2. **`kv_history`**: Contract promises version history. Storage has `VersionChain`. Primitive doesn't expose iteration.

**These are Strata's core value proposition and MUST be implemented.**
