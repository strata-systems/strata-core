# Substrate API Reference

The **Substrate API** is Strata's power-user interface, exposing the full capabilities of the database with explicit control over runs, versioning, and transactions.

**Version**: 0.11.0

## Table of Contents

- [Overview](#overview)
- [Core Types](#core-types)
- [KVStore](#kvstore)
- [EventLog](#eventlog)
- [StateCell](#statecell)
- [TraceStore](#tracestore)
- [VectorStore](#vectorstore)
- [JsonStore](#jsonstore)
- [RunIndex](#runindex)
- [RetentionSubstrate](#retentionsubstrate)
- [Error Handling](#error-handling)

---

## Overview

The Substrate API is the canonical semantic contract for Strata. It exposes:

- **All primitives explicitly**: KVStore, JsonStore, EventLog, StateCell, VectorStore, TraceStore
- **All versioning**: `Versioned<T>` returns on all reads, `Version` on all writes
- **All run scoping**: Explicit `run_id` on every operation
- **All transactional semantics**: Begin/commit/rollback support

### Design Philosophy

The Substrate API is:

- **Deterministic and replayable**: Every operation is tagged with run_id
- **Minimal, not friendly**: Power over convenience
- **Unambiguous and stable**: Clear contracts, no magic

### Usage

```rust
use strata_api::substrate::{
    SubstrateImpl, ApiRunId, KVStore, KVStoreBatch,
    EventLog, StateCell, VectorStore, TraceStore, JsonStore,
    RunIndex, RetentionSubstrate, RetentionPolicy,
};
use strata_core::Value;
use strata_engine::Database;
use std::sync::Arc;

// Create substrate
let db = Arc::new(Database::open("./my-db")?);
let substrate = SubstrateImpl::new(db);

// Use default run or create new run
let run = ApiRunId::default();  // or ApiRunId::new()

// Perform operations
substrate.kv_put(&run, "key", Value::Int(42))?;
let value = substrate.kv_get(&run, "key")?;
```

---

## Core Types

### ApiRunId

Run identifier for the Substrate API.

```rust
pub struct ApiRunId(String);
```

A run ID is either:
- The literal string `"default"` for the default run
- A UUID v4 in lowercase hyphenated format

#### Methods

| Method | Description |
|--------|-------------|
| `new()` | Create a new random UUID run ID |
| `default()` | Get the default run ID |
| `parse(s: &str)` | Parse from string, returns `Option<ApiRunId>` |
| `is_default()` | Check if this is the default run |
| `is_uuid()` | Check if this is a UUID run |
| `as_str()` | Get string representation |

#### Examples

```rust
// Default run
let default = ApiRunId::default();
assert!(default.is_default());

// New UUID run
let custom = ApiRunId::new();
assert!(!custom.is_default());

// Parse from string
let parsed: ApiRunId = "default".parse().unwrap();
```

### Version

Version identifier for values.

```rust
pub enum Version {
    Txn(u64),      // Transaction-based (KV, JSON)
    Sequence(u64), // Sequence-based (EventLog)
    Counter(u64),  // Counter-based (StateCell)
}
```

### Versioned<T>

A value with version metadata.

```rust
pub struct Versioned<T> {
    pub value: T,
    pub version: Version,
    pub timestamp: Timestamp,
}
```

### RetentionPolicy

Controls how long historical versions are retained.

```rust
pub enum RetentionPolicy {
    KeepAll,              // Keep all versions indefinitely
    KeepLast(u64),        // Keep N most recent versions
    KeepFor(Duration),    // Keep versions within time window
    Composite(Vec<Self>), // Union of policies (most permissive wins)
}
```

### RunState

Run lifecycle state.

```rust
pub enum RunState {
    Active,     // Run is executing
    Completed,  // Run completed successfully
    Failed,     // Run failed with error
    Cancelled,  // Run was cancelled
    Paused,     // Run is paused (can resume)
    Archived,   // Terminal state
}
```

---

## KVStore

Key-value store with versioned reads, history access, and atomic operations.

### Trait Definition

```rust
pub trait KVStore {
    fn kv_put(&self, run: &ApiRunId, key: &str, value: Value) -> StrataResult<Version>;
    fn kv_get(&self, run: &ApiRunId, key: &str) -> StrataResult<Option<Versioned<Value>>>;
    fn kv_get_at(&self, run: &ApiRunId, key: &str, version: Version) -> StrataResult<Versioned<Value>>;
    fn kv_delete(&self, run: &ApiRunId, key: &str) -> StrataResult<bool>;
    fn kv_exists(&self, run: &ApiRunId, key: &str) -> StrataResult<bool>;
    fn kv_history(&self, run: &ApiRunId, key: &str, limit: Option<u64>, before: Option<Version>) -> StrataResult<Vec<Versioned<Value>>>;
    fn kv_incr(&self, run: &ApiRunId, key: &str, delta: i64) -> StrataResult<i64>;
    fn kv_cas_version(&self, run: &ApiRunId, key: &str, expected: Option<Version>, value: Value) -> StrataResult<bool>;
    fn kv_cas_value(&self, run: &ApiRunId, key: &str, expected: Option<Value>, value: Value) -> StrataResult<bool>;
    fn kv_keys(&self, run: &ApiRunId, prefix: &str, limit: Option<usize>) -> StrataResult<Vec<String>>;
    fn kv_scan(&self, run: &ApiRunId, prefix: &str, limit: usize, cursor: Option<&str>) -> StrataResult<KVScanResult>;
}
```

### Key Constraints

Keys must be:
- Valid UTF-8 strings
- Non-empty
- No NUL bytes
- Not starting with `_strata/` (reserved prefix)
- Maximum 1024 bytes

### Methods

#### kv_put

Stores a key-value pair, returning the version created.

```rust
fn kv_put(&self, run: &ApiRunId, key: &str, value: Value) -> StrataResult<Version>;
```

**Example**:
```rust
let version = substrate.kv_put(&run, "user:1", Value::String("Alice".into()))?;
```

#### kv_get

Retrieves a value with version information.

```rust
fn kv_get(&self, run: &ApiRunId, key: &str) -> StrataResult<Option<Versioned<Value>>>;
```

**Example**:
```rust
if let Some(versioned) = substrate.kv_get(&run, "user:1")? {
    println!("Value: {:?}, Version: {:?}", versioned.value, versioned.version);
}
```

#### kv_get_at

Retrieves a value at a specific historical version.

```rust
fn kv_get_at(&self, run: &ApiRunId, key: &str, version: Version) -> StrataResult<Versioned<Value>>;
```

**Example**:
```rust
let old_value = substrate.kv_get_at(&run, "config", Version::Txn(42))?;
```

#### kv_delete

Deletes a key, returning `true` if it existed.

```rust
fn kv_delete(&self, run: &ApiRunId, key: &str) -> StrataResult<bool>;
```

#### kv_exists

Checks if a key exists.

```rust
fn kv_exists(&self, run: &ApiRunId, key: &str) -> StrataResult<bool>;
```

#### kv_history

Returns version history for a key (newest first).

```rust
fn kv_history(
    &self,
    run: &ApiRunId,
    key: &str,
    limit: Option<u64>,
    before: Option<Version>,
) -> StrataResult<Vec<Versioned<Value>>>;
```

**Pagination Example**:
```rust
// First page
let page1 = substrate.kv_history(&run, "key", Some(10), None)?;

// Next page (if more results)
if let Some(last) = page1.last() {
    let page2 = substrate.kv_history(&run, "key", Some(10), Some(last.version))?;
}
```

#### kv_incr

Atomic increment operation.

```rust
fn kv_incr(&self, run: &ApiRunId, key: &str, delta: i64) -> StrataResult<i64>;
```

**Semantics**:
- Missing key is treated as `0`
- Returns the new value after increment
- Type-safe: only works on `Value::Int`
- Overflow returns error

**Example**:
```rust
let count = substrate.kv_incr(&run, "page_views", 1)?;
let decremented = substrate.kv_incr(&run, "stock", -5)?;
```

#### kv_cas_version

Compare-and-swap by version.

```rust
fn kv_cas_version(
    &self,
    run: &ApiRunId,
    key: &str,
    expected_version: Option<Version>,
    new_value: Value,
) -> StrataResult<bool>;
```

**Semantics**:
- `expected_version = None`: Succeeds only if key doesn't exist
- `expected_version = Some(v)`: Succeeds only if current version == v
- Returns `true` on success, `false` on mismatch

**Example**:
```rust
// Create only if new
let created = substrate.kv_cas_version(&run, "lock", None, Value::Bool(true))?;

// Update with optimistic locking
let current = substrate.kv_get(&run, "config")?.unwrap();
let updated = substrate.kv_cas_version(
    &run, "config",
    Some(current.version),
    Value::String("new_config".into())
)?;
```

#### kv_cas_value

Compare-and-swap by value.

```rust
fn kv_cas_value(
    &self,
    run: &ApiRunId,
    key: &str,
    expected_value: Option<Value>,
    new_value: Value,
) -> StrataResult<bool>;
```

#### kv_keys

List keys with optional prefix filter.

```rust
fn kv_keys(&self, run: &ApiRunId, prefix: &str, limit: Option<usize>) -> StrataResult<Vec<String>>;
```

**Example**:
```rust
let user_keys = substrate.kv_keys(&run, "user:", Some(100))?;
```

#### kv_scan

Cursor-based key iteration.

```rust
fn kv_scan(
    &self,
    run: &ApiRunId,
    prefix: &str,
    limit: usize,
    cursor: Option<&str>,
) -> StrataResult<KVScanResult>;

pub struct KVScanResult {
    pub entries: Vec<(String, Versioned<Value>)>,
    pub cursor: Option<String>,
}
```

**Example**:
```rust
let mut cursor = None;
loop {
    let result = substrate.kv_scan(&run, "user:", 100, cursor.as_deref())?;
    for (key, versioned) in result.entries {
        process(key, versioned.value);
    }
    cursor = result.cursor;
    if cursor.is_none() {
        break;
    }
}
```

### Batch Operations (KVStoreBatch)

```rust
pub trait KVStoreBatch: KVStore {
    fn kv_mget(&self, run: &ApiRunId, keys: &[&str]) -> StrataResult<Vec<Option<Versioned<Value>>>>;
    fn kv_mput(&self, run: &ApiRunId, entries: &[(&str, Value)]) -> StrataResult<Version>;
    fn kv_mdelete(&self, run: &ApiRunId, keys: &[&str]) -> StrataResult<u64>;
    fn kv_mexists(&self, run: &ApiRunId, keys: &[&str]) -> StrataResult<u64>;
}
```

**Example**:
```rust
// Get multiple values
let results = substrate.kv_mget(&run, &["user:1", "user:2", "user:3"])?;

// Set multiple values atomically
substrate.kv_mput(&run, &[
    ("config:a", Value::Int(1)),
    ("config:b", Value::Int(2)),
])?;

// Delete multiple keys
let deleted_count = substrate.kv_mdelete(&run, &["temp:1", "temp:2"])?;
```

---

## EventLog

Append-only event streams for logging and messaging.

### Trait Definition

```rust
pub trait EventLog {
    fn event_append(&self, run: &ApiRunId, stream: &str, payload: Value) -> StrataResult<Version>;
    fn event_range(&self, run: &ApiRunId, stream: &str, start: Option<u64>, end: Option<u64>, limit: Option<usize>) -> StrataResult<Vec<Versioned<Value>>>;
    fn event_head(&self, run: &ApiRunId, stream: &str) -> StrataResult<Option<Versioned<Value>>>;
    fn event_len(&self, run: &ApiRunId, stream: &str) -> StrataResult<u64>;
}
```

### Stream Model

- Events are organized into named streams
- Each stream has independent sequence numbers
- Events are immutable (append-only, no updates or deletes)
- Payloads must be `Value::Object`

### Methods

#### event_append

Appends an event to a stream.

```rust
fn event_append(&self, run: &ApiRunId, stream: &str, payload: Value) -> StrataResult<Version>;
```

**Example**:
```rust
let seq = substrate.event_append(&run, "audit", Value::Object(
    [("action".into(), Value::String("login".into()))].into()
))?;
```

#### event_range

Reads events within a sequence range.

```rust
fn event_range(
    &self,
    run: &ApiRunId,
    stream: &str,
    start: Option<u64>,
    end: Option<u64>,
    limit: Option<usize>,
) -> StrataResult<Vec<Versioned<Value>>>;
```

**Pagination Example**:
```rust
// First 100 events
let page1 = substrate.event_range(&run, "audit", None, None, Some(100))?;

// Next 100 events
let last_seq = match page1.last() {
    Some(v) => match v.version { Version::Sequence(n) => n, _ => 0 },
    None => 0,
};
let page2 = substrate.event_range(&run, "audit", Some(last_seq + 1), None, Some(100))?;
```

---

## StateCell

Compare-and-swap cells for coordination.

### Trait Definition

```rust
pub trait StateCell {
    fn state_set(&self, run: &ApiRunId, cell: &str, value: Value) -> StrataResult<Version>;
    fn state_get(&self, run: &ApiRunId, cell: &str) -> StrataResult<Option<Versioned<Value>>>;
    fn state_cas(&self, run: &ApiRunId, cell: &str, expected: Option<u64>, value: Value) -> StrataResult<Option<Version>>;
    fn state_delete(&self, run: &ApiRunId, cell: &str) -> StrataResult<bool>;
}
```

### Counter Model

- Every StateCell has a counter starting at 0
- Every write increments the counter by 1
- CAS operations compare against the counter

### Use Cases

- Leader election
- Distributed locks
- Single-writer coordination

### Methods

#### state_cas

Compare-and-swap with counter check.

```rust
fn state_cas(
    &self,
    run: &ApiRunId,
    cell: &str,
    expected_counter: Option<u64>,
    value: Value,
) -> StrataResult<Option<Version>>;
```

**Example - Acquire Lock**:
```rust
// Try to acquire lock (only if cell doesn't exist)
let acquired = substrate.state_cas(&run, "leader_lock", None, Value::String("worker-1".into()))?;
if acquired.is_some() {
    println!("Lock acquired!");
}
```

---

## TraceStore

Hierarchical trace recording for debugging.

### Trait Definition

```rust
pub trait TraceStore {
    fn trace_record(&self, run: &ApiRunId, trace_type: TraceType, tags: Vec<String>, metadata: Value) -> StrataResult<String>;
    fn trace_record_child(&self, run: &ApiRunId, parent_id: &str, trace_type: TraceType, tags: Vec<String>, metadata: Value) -> StrataResult<String>;
    fn trace_get(&self, run: &ApiRunId, trace_id: &str) -> StrataResult<Option<TraceEntry>>;
    fn trace_query_by_type(&self, run: &ApiRunId, trace_type: &str) -> StrataResult<Vec<TraceEntry>>;
    fn trace_query_by_tag(&self, run: &ApiRunId, tag: &str) -> StrataResult<Vec<TraceEntry>>;
}
```

### TraceType

```rust
pub enum TraceType {
    ToolCall { tool_name: String, arguments: Value, result: Option<Value>, duration_ms: Option<u64> },
    Decision { question: String, options: Vec<String>, chosen: String, reasoning: Option<String> },
    Query { query_type: String, query: String, results_count: Option<u32> },
    Thought { content: String, confidence: Option<f64> },
    Error { error_type: String, message: String, recoverable: bool },
    Custom { name: String, data: Value },
}
```

---

## VectorStore

Vector similarity search with embedding storage.

### Trait Definition

```rust
pub trait VectorStore {
    fn vector_upsert(&self, run: &ApiRunId, namespace: &str, id: &str, vector: VectorData, metadata: Value) -> StrataResult<Version>;
    fn vector_get(&self, run: &ApiRunId, namespace: &str, id: &str) -> StrataResult<Option<Versioned<VectorData>>>;
    fn vector_delete(&self, run: &ApiRunId, namespace: &str, id: &str) -> StrataResult<bool>;
    fn vector_search(&self, run: &ApiRunId, namespace: &str, query: VectorData, limit: usize, filter: Option<SearchFilter>) -> StrataResult<Vec<VectorMatch>>;
}
```

### Types

```rust
pub enum VectorData {
    F32(Vec<f32>),
    F64(Vec<f64>),
}

pub enum DistanceMetric {
    Cosine,
    Euclidean,
    DotProduct,
}

pub struct VectorMatch {
    pub id: String,
    pub score: f64,
    pub metadata: Value,
}
```

---

## JsonStore

JSON document storage with path-level mutations.

### Trait Definition

```rust
pub trait JsonStore {
    fn json_put(&self, run: &ApiRunId, key: &str, value: serde_json::Value) -> StrataResult<Version>;
    fn json_get(&self, run: &ApiRunId, key: &str) -> StrataResult<Option<Versioned<serde_json::Value>>>;
    fn json_delete(&self, run: &ApiRunId, key: &str) -> StrataResult<bool>;
    fn json_get_path(&self, run: &ApiRunId, key: &str, path: &str) -> StrataResult<Option<serde_json::Value>>;
    fn json_set_path(&self, run: &ApiRunId, key: &str, path: &str, value: serde_json::Value) -> StrataResult<Version>;
    fn json_delete_path(&self, run: &ApiRunId, key: &str, path: &str) -> StrataResult<bool>;
    fn json_array_push(&self, run: &ApiRunId, key: &str, path: &str, value: serde_json::Value) -> StrataResult<Version>;
    fn json_array_pop(&self, run: &ApiRunId, key: &str, path: &str) -> StrataResult<Option<serde_json::Value>>;
}
```

### Path Operations

Paths use JSONPath-like syntax:
- `$.field` - Object field
- `$.array[0]` - Array index
- `$.nested.path.to.value` - Nested access

**Example**:
```rust
// Store document
substrate.json_put(&run, "user:1", serde_json::json!({
    "name": "Alice",
    "settings": { "theme": "dark" }
}))?;

// Update nested field
substrate.json_set_path(&run, "user:1", "$.settings.theme", serde_json::json!("light"))?;
```

---

## RunIndex

Run lifecycle management.

### Trait Definition

```rust
pub trait RunIndex {
    fn run_create(&self, metadata: Value) -> StrataResult<ApiRunId>;
    fn run_get(&self, run: &ApiRunId) -> StrataResult<Option<RunInfo>>;
    fn run_update_state(&self, run: &ApiRunId, state: RunState, error: Option<String>) -> StrataResult<()>;
    fn run_list(&self, state: Option<RunState>, limit: Option<usize>) -> StrataResult<Vec<RunInfo>>;
    fn run_delete(&self, run: &ApiRunId) -> StrataResult<bool>;
}
```

---

## RetentionSubstrate

Retention policy management.

### Trait Definition

```rust
pub trait RetentionSubstrate {
    fn retention_get(&self, run: &ApiRunId) -> StrataResult<Option<RetentionPolicy>>;
    fn retention_set(&self, run: &ApiRunId, policy: RetentionPolicy) -> StrataResult<RetentionVersion>;
    fn retention_clear(&self, run: &ApiRunId) -> StrataResult<bool>;
}
```

---

## Error Handling

All Substrate operations return `StrataResult<T>`.

### Error Types

| Error | Description |
|-------|-------------|
| `InvalidKey` | Key is empty, contains NUL, has reserved prefix, or exceeds max length |
| `NotFound` | Run or entity does not exist |
| `ConstraintViolation` | Run is closed, value exceeds limits, or operation not allowed |
| `WrongType` | Type mismatch (e.g., incr on non-integer) |
| `Overflow` | Integer overflow in arithmetic operation |
| `HistoryTrimmed` | Requested version has been garbage collected |
| `Conflict` | Transaction conflict (OCC failure) |

### Example Error Handling

```rust
use strata_core::StrataError;

match substrate.kv_get(&run, "key") {
    Ok(Some(versioned)) => println!("Found: {:?}", versioned.value),
    Ok(None) => println!("Key not found"),
    Err(e) if e.is_not_found() => println!("Run doesn't exist"),
    Err(e) => return Err(e),
}
```

---

## See Also

- [Facade API Reference](facade-api.md) - Simplified Redis-like API
- [Getting Started Guide](getting-started.md)
- [Architecture Overview](architecture.md)
