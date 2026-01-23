# Facade API Reference

The **Facade API** provides Redis-like convenience for common operations, with simplified return types and automatic run targeting.

**Version**: 0.11.0

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [KVFacade](#kvfacade)
  - [Basic Operations](#basic-operations)
  - [Atomic Operations](#atomic-operations)
  - [Batch Operations](#batch-operations)
- [Configuration](#configuration)
- [Desugaring Reference](#desugaring-reference)
- [Error Handling](#error-handling)

---

## Overview

The Facade API is syntactic sugar over the [Substrate API](substrate-api.md). Every facade call desugars to exactly one substrate call pattern.

### Key Differences from Substrate

| Aspect | Substrate | Facade |
|--------|-----------|--------|
| Run specification | Explicit `run_id` on every call | Implicit default run |
| Return types | `Versioned<Value>` with metadata | Plain `Value` (or `Option<Value>`) |
| Commit behavior | Explicit transactions | Auto-commit each operation |
| Version info | Always returned | Opt-in via `getv()` |

### When to Use Facade vs Substrate

**Use Facade when:**
- You want Redis-like simplicity
- You only need the default run
- You don't need version history
- Auto-commit per operation is fine

**Use Substrate when:**
- You need multiple runs
- You need version history or point-in-time reads
- You need explicit transaction control
- You need full metadata on reads

---

## Quick Start

```rust
use strata_api::facade::{FacadeImpl, KVFacade, KVFacadeBatch};
use strata_api::substrate::SubstrateImpl;
use strata_core::Value;
use strata_engine::Database;
use std::sync::Arc;

// Create facade
let db = Arc::new(Database::open("./my-db")?);
let substrate = Arc::new(SubstrateImpl::new(db));
let facade = FacadeImpl::new(substrate);

// Simple get/set
facade.set("user:1", Value::String("Alice".into()))?;
let name = facade.get("user:1")?; // Returns Option<Value>

// Increment counter
let count = facade.incr("page_views")?; // Returns i64

// Conditional set
let was_new = facade.setnx("lock", Value::Bool(true))?; // Returns bool
```

---

## KVFacade

Redis-familiar key-value operations.

### Basic Operations

#### get

Get a value by key.

```rust
fn get(&self, key: &str) -> StrataResult<Option<Value>>;
```

**Returns**: `None` if key doesn't exist.

**Desugars to**: `kv_get(default_run, key).map(|v| v.value)`

```rust
let value = facade.get("user:1")?;
match value {
    Some(v) => println!("Found: {:?}", v),
    None => println!("Not found"),
}
```

#### getv

Get a value with version information.

```rust
fn getv(&self, key: &str) -> StrataResult<Option<Versioned<Value>>>;

pub struct Versioned<T> {
    pub value: T,
    pub version: u64,
    pub timestamp: u64,
}
```

**Desugars to**: `kv_get(default_run, key)`

Use this when you need the version number for optimistic locking:

```rust
let versioned = facade.getv("config")?.unwrap();
println!("Value: {:?}, Version: {}", versioned.value, versioned.version);
```

#### set

Set a value.

```rust
fn set(&self, key: &str, value: Value) -> StrataResult<()>;
```

**Desugars to**: `kv_put(default_run, key, value)`

```rust
facade.set("user:1", Value::String("Alice".into()))?;
facade.set("count", Value::Int(42))?;
facade.set("active", Value::Bool(true))?;
```

#### del

Delete a key.

```rust
fn del(&self, key: &str) -> StrataResult<bool>;
```

**Returns**: `true` if the key existed, `false` otherwise.

**Desugars to**: `kv_delete(default_run, key)`

```rust
let existed = facade.del("user:1")?;
```

#### exists

Check if a key exists.

```rust
fn exists(&self, key: &str) -> StrataResult<bool>;
```

**Desugars to**: `kv_exists(default_run, key)`

```rust
if facade.exists("user:1")? {
    println!("User exists");
}
```

#### setnx

Set if not exists (NX).

```rust
fn setnx(&self, key: &str, value: Value) -> StrataResult<bool>;
```

**Returns**: `true` if the key was set (didn't exist), `false` if it already existed.

**Desugars to**: `kv_cas_version(default_run, key, None, value)`

```rust
// Acquire a lock
if facade.setnx("lock:resource", Value::String("worker-1".into()))? {
    println!("Lock acquired!");
} else {
    println!("Lock already held");
}
```

#### getset

Get and set atomically.

```rust
fn getset(&self, key: &str, value: Value) -> StrataResult<Option<Value>>;
```

**Returns**: The old value (if any).

```rust
let old = facade.getset("counter", Value::Int(0))?;
println!("Previous value: {:?}", old);
```

---

### Atomic Operations

#### incr

Increment by 1.

```rust
fn incr(&self, key: &str) -> StrataResult<i64>;
```

**Semantics**:
- Creates key with value `1` if it doesn't exist
- Returns the new value after increment
- Type-safe: fails on non-integer values
- Overflow returns error

**Desugars to**: `kv_incr(default_run, key, 1)`

```rust
let views = facade.incr("page:views")?; // Returns 1, 2, 3, ...
```

#### incrby

Increment by delta.

```rust
fn incrby(&self, key: &str, delta: i64) -> StrataResult<i64>;
```

**Desugars to**: `kv_incr(default_run, key, delta)`

```rust
let new_count = facade.incrby("inventory", -5)?; // Decrement by 5
let new_score = facade.incrby("score", 100)?;    // Add 100
```

#### decr

Decrement by 1. Equivalent to `incrby(key, -1)`.

```rust
fn decr(&self, key: &str) -> StrataResult<i64>;
```

```rust
let remaining = facade.decr("quota")?;
```

#### decrby

Decrement by delta. Equivalent to `incrby(key, -delta)`.

```rust
fn decrby(&self, key: &str, delta: i64) -> StrataResult<i64>;
```

```rust
let remaining = facade.decrby("stock", 10)?;
```

---

### Batch Operations

Batch operations are atomic: all succeed or all fail.

#### mget

Get multiple keys.

```rust
fn mget(&self, keys: &[&str]) -> StrataResult<Vec<Option<Value>>>;
```

**Returns**: Values in same order as keys, with `None` for missing keys.

**Desugars to**: `kv_mget(default_run, keys).map(strip_versions)`

```rust
let results = facade.mget(&["user:1", "user:2", "user:3"])?;
// results[0] = Some(Value::String("Alice"))
// results[1] = None (if missing)
// results[2] = Some(Value::String("Charlie"))
```

#### mset

Set multiple key-value pairs atomically.

```rust
fn mset(&self, entries: &[(&str, Value)]) -> StrataResult<()>;
```

**Desugars to**: `kv_mput(default_run, entries)`

```rust
facade.mset(&[
    ("config:timeout", Value::Int(30)),
    ("config:retries", Value::Int(3)),
    ("config:enabled", Value::Bool(true)),
])?;
```

#### mdel

Delete multiple keys.

```rust
fn mdel(&self, keys: &[&str]) -> StrataResult<u64>;
```

**Returns**: Count of keys that existed (were actually deleted).

**Desugars to**: `kv_mdelete(default_run, keys)`

```rust
let deleted = facade.mdel(&["temp:1", "temp:2", "temp:3"])?;
println!("Deleted {} keys", deleted);
```

#### mexists

Count existing keys.

```rust
fn mexists(&self, keys: &[&str]) -> StrataResult<u64>;
```

**Desugars to**: `kv_mexists(default_run, keys)`

```rust
let exists_count = facade.mexists(&["user:1", "user:2", "user:3"])?;
println!("{} of 3 users exist", exists_count);
```

---

## Configuration

### FacadeConfig

```rust
pub struct FacadeConfig {
    pub default_timeout: Option<Duration>,
    pub return_versions: bool,
    pub auto_commit: bool,
}

impl FacadeConfig {
    fn new() -> Self;
    fn with_timeout(self, timeout: Duration) -> Self;
    fn with_versions(self) -> Self;
    fn without_auto_commit(self) -> Self;
}
```

### SetOptions

Options for `set_with_options`:

```rust
pub struct SetOptions {
    pub only_if_not_exists: bool,  // NX
    pub only_if_exists: bool,      // XX
    pub get_old_value: bool,       // GET
    pub expected_version: Option<u64>,
}

// Builder methods
SetOptions::new()
    .nx()              // Only set if not exists
    .xx()              // Only set if exists
    .get()             // Return old value
    .if_version(42)    // Optimistic locking
```

### GetOptions

Options for `get_with_options`:

```rust
pub struct GetOptions {
    pub with_version: bool,
    pub at_version: Option<u64>,
}

GetOptions::new()
    .with_version()    // Include version in result
    .at_version(42)    // Get value at specific version
```

---

## Desugaring Reference

Complete mapping from Facade to Substrate calls:

| Facade Call | Substrate Equivalent |
|-------------|---------------------|
| `get(key)` | `kv_get(default, key).map(\|v\| v.value)` |
| `getv(key)` | `kv_get(default, key)` |
| `set(key, val)` | `kv_put(default, key, val)` |
| `del(key)` | `kv_delete(default, key)` |
| `exists(key)` | `kv_exists(default, key)` |
| `incr(key)` | `kv_incr(default, key, 1)` |
| `incrby(key, n)` | `kv_incr(default, key, n)` |
| `decr(key)` | `kv_incr(default, key, -1)` |
| `decrby(key, n)` | `kv_incr(default, key, -n)` |
| `setnx(key, val)` | `kv_cas_version(default, key, None, val)` |
| `getset(key, val)` | `get + put` (atomic) |
| `mget(keys)` | `kv_mget(default, keys).map(strip_versions)` |
| `mset(entries)` | `kv_mput(default, entries)` |
| `mdel(keys)` | `kv_mdelete(default, keys)` |
| `mexists(keys)` | `kv_mexists(default, keys)` |

---

## Error Handling

Facade operations return `StrataResult<T>`.

### Common Errors

| Error | Cause |
|-------|-------|
| `InvalidKey` | Key is empty, contains NUL, or exceeds 1024 bytes |
| `WrongType` | Increment on non-integer value |
| `Overflow` | Integer overflow in incr/decr |

### Example

```rust
match facade.incr("counter") {
    Ok(new_value) => println!("Counter: {}", new_value),
    Err(e) if e.is_wrong_type() => println!("Counter is not an integer"),
    Err(e) => return Err(e),
}
```

---

## See Also

- [Substrate API Reference](substrate-api.md) - Full-power API with versioning and runs
- [Getting Started Guide](getting-started.md)
- [Architecture Overview](architecture.md)
