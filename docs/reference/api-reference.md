# API Reference

Complete API reference for **in-mem** v0.1.0 (M1 Foundation).

## Core Types

### `Database`

Main entry point for interacting with **in-mem**.

```rust
pub struct Database {
    // Internal fields (opaque)
}
```

#### Methods

##### `open`

Opens or creates a database at the specified path.

```rust
pub fn open<P: AsRef<Path>>(path: P) -> Result<Database>
```

**Parameters**:
- `path`: Directory path for database storage (created if doesn't exist)

**Returns**: `Result<Database>`

**Example**:
```rust
let db = Database::open("./my-database")?;
```

##### `open_with_mode`

Opens a database with a specific durability mode.

```rust
pub fn open_with_mode<P: AsRef<Path>>(
    path: P,
    mode: DurabilityMode
) -> Result<Database>
```

**Parameters**:
- `path`: Directory path for database storage
- `mode`: Durability mode (Strict, Batched, or Async)

**Returns**: `Result<Database>`

**Example**:
```rust
let db = Database::open_with_mode(
    "./data",
    DurabilityMode::Batched { interval_ms: 100, max_commits: 1000 }
)?;
```

##### `begin_run`

Creates a new run and returns its ID.

```rust
pub fn begin_run(&self) -> RunId
```

**Returns**: `RunId` - Unique identifier for this run

**Example**:
```rust
let run_id = db.begin_run();
```

##### `end_run`

Ends a run and releases its resources.

```rust
pub fn end_run(&self, run_id: RunId) -> Result<()>
```

**Parameters**:
- `run_id`: Run to end

**Returns**: `Result<()>`

**Example**:
```rust
db.end_run(run_id)?;
```

##### `put`

Stores a key-value pair.

```rust
pub fn put(
    &self,
    run_id: RunId,
    key: &[u8],
    value: &[u8]
) -> Result<u64>
```

**Parameters**:
- `run_id`: Run ID for this operation
- `key`: Key bytes
- `value`: Value bytes

**Returns**: `Result<u64>` - Version number assigned to this write

**Example**:
```rust
let version = db.put(run_id, b"user:123", b"Alice")?;
```

##### `put_with_ttl`

Stores a key-value pair with time-to-live.

```rust
pub fn put_with_ttl(
    &self,
    run_id: RunId,
    key: &[u8],
    value: &[u8],
    ttl: Duration
) -> Result<u64>
```

**Parameters**:
- `run_id`: Run ID for this operation
- `key`: Key bytes
- `value`: Value bytes
- `ttl`: Time-to-live duration

**Returns**: `Result<u64>` - Version number

**Example**:
```rust
use std::time::Duration;

db.put_with_ttl(
    run_id,
    b"session:abc",
    b"data",
    Duration::from_secs(3600)
)?;
```

##### `get`

Retrieves a value by key.

```rust
pub fn get(
    &self,
    run_id: RunId,
    key: &[u8]
) -> Result<Option<Vec<u8>>>
```

**Parameters**:
- `run_id`: Run ID for this operation
- `key`: Key bytes

**Returns**: `Result<Option<Vec<u8>>>` - Value if exists, None if not found

**Example**:
```rust
let value = db.get(run_id, b"user:123")?;
match value {
    Some(v) => println!("Found: {:?}", v),
    None => println!("Not found"),
}
```

##### `delete`

Deletes a key-value pair.

```rust
pub fn delete(
    &self,
    run_id: RunId,
    key: &[u8]
) -> Result<bool>
```

**Parameters**:
- `run_id`: Run ID for this operation
- `key`: Key bytes

**Returns**: `Result<bool>` - true if key existed, false if not found

**Example**:
```rust
let deleted = db.delete(run_id, b"user:123")?;
```

##### `list`

Lists all keys with a given prefix.

```rust
pub fn list(
    &self,
    run_id: RunId,
    prefix: &[u8]
) -> Result<Vec<(Vec<u8>, Vec<u8>)>>
```

**Parameters**:
- `run_id`: Run ID for this operation
- `prefix`: Key prefix to match

**Returns**: `Result<Vec<(key, value)>>` - Vector of matching key-value pairs

**Example**:
```rust
let users = db.list(run_id, b"user:")?;
for (key, value) in users {
    println!("Key: {:?}, Value: {:?}", key, value);
}
```

##### `flush`

Forces all pending writes to disk.

```rust
pub fn flush(&self) -> Result<()>
```

**Returns**: `Result<()>`

**Example**:
```rust
db.flush()?; // Ensure all writes are durable
```

---

## Enums

### `DurabilityMode`

Controls how writes are persisted to disk.

```rust
pub enum DurabilityMode {
    Strict,
    Batched { interval_ms: u64, max_commits: usize },
    Async { interval_ms: u64 },
}
```

#### Variants

**`Strict`**

Every commit is immediately followed by fsync. Maximum durability, lowest performance.

**Use when**: Financial transactions, critical data that cannot be lost.

**`Batched { interval_ms, max_commits }`**

Writes are fsynced either:
- Every `interval_ms` milliseconds, OR
- After `max_commits` commits

Balanced trade-off between durability and performance. **Default mode**.

**Use when**: Agent workflows, tool outputs, general use.

**Parameters**:
- `interval_ms`: Maximum time between fsyncs (milliseconds)
- `max_commits`: Maximum commits before forced fsync

**Example**:
```rust
DurabilityMode::Batched {
    interval_ms: 100,  // fsync at least every 100ms
    max_commits: 1000  // or after 1000 commits
}
```

**`Async { interval_ms }`**

Background thread fsyncs every `interval_ms` milliseconds. Highest performance, may lose recent writes on crash.

**Use when**: High-throughput logging, caching, non-critical data.

**Parameters**:
- `interval_ms`: Time between background fsyncs

**Example**:
```rust
DurabilityMode::Async {
    interval_ms: 1000  // fsync every second
}
```

---

## Primitives

### `KVStore`

Key-value store primitive with type-safe value encoding.

```rust
pub struct KVStore<'a> {
    db: &'a Database,
}
```

#### Methods

##### `new`

Creates a new KVStore instance.

```rust
pub fn new(db: &Database) -> KVStore
```

**Example**:
```rust
let kv = KVStore::new(&db);
```

##### `put`

Stores a typed value.

```rust
pub fn put<T: Serialize>(
    &self,
    run_id: RunId,
    key: &str,
    value: T
) -> Result<u64>
```

**Parameters**:
- `run_id`: Run ID
- `key`: String key
- `value`: Any serializable value

**Returns**: `Result<u64>` - Version number

**Example**:
```rust
kv.put(run_id, "user:123:name", "Alice")?;
kv.put(run_id, "user:123:age", 30)?;
kv.put(run_id, "config", vec!["opt1", "opt2"])?;
```

##### `get`

Retrieves a typed value.

```rust
pub fn get<T: DeserializeOwned>(
    &self,
    run_id: RunId,
    key: &str
) -> Result<Option<T>>
```

**Parameters**:
- `run_id`: Run ID
- `key`: String key

**Returns**: `Result<Option<T>>` - Deserialized value if exists

**Example**:
```rust
let name: Option<String> = kv.get(run_id, "user:123:name")?;
let age: Option<i32> = kv.get(run_id, "user:123:age")?;
```

##### `delete`

Deletes a key.

```rust
pub fn delete(
    &self,
    run_id: RunId,
    key: &str
) -> Result<bool>
```

**Example**:
```rust
kv.delete(run_id, "user:123:name")?;
```

---

## Core Data Types

### `RunId`

Unique identifier for an agent run.

```rust
pub struct RunId(Uuid);
```

**Properties**:
- Globally unique (UUID v4)
- Serializable
- Cloneable

**Example**:
```rust
let run_id = db.begin_run();
println!("Run ID: {}", run_id); // Prints UUID
```

### `Namespace`

Hierarchical namespace for multi-tenancy.

```rust
pub struct Namespace {
    pub tenant: String,
    pub app: String,
    pub agent: String,
    pub run: RunId,
}
```

**Example**:
```rust
let ns = Namespace {
    tenant: "acme-corp".to_string(),
    app: "customer-service".to_string(),
    agent: "chat-bot-v2".to_string(),
    run: run_id,
};
```

### `Key`

Internal key structure (generally not used directly).

```rust
pub struct Key {
    namespace: Namespace,
    type_tag: TypeTag,
    user_key: Vec<u8>,
}
```

Keys are automatically ordered by:
1. Namespace (tenant → app → agent → run)
2. Type tag
3. User key

This enables efficient prefix scans.

### `Value`

Flexible value type supporting multiple primitives.

```rust
pub enum Value {
    Bytes(Vec<u8>),
    String(String),
    I64(i64),
    F64(f64),
    Bool(bool),
    Null,
    Array(Vec<Value>),
    Map(BTreeMap<String, Value>),
}
```

**Example**:
```rust
// Values are automatically encoded/decoded
let v1 = Value::String("hello".to_string());
let v2 = Value::I64(42);
let v3 = Value::Array(vec![Value::I64(1), Value::I64(2)]);
```

---

## Error Types

### `Error`

All errors in **in-mem** use this type.

```rust
pub enum Error {
    Io(std::io::Error),
    Serialization(bincode::Error),
    Corruption(String),
    KeyNotFound(Vec<u8>),
    InvalidOperation(String),
    // ... other variants
}
```

**Common Errors**:

- `Error::Io`: File system errors (permissions, disk full)
- `Error::Serialization`: Value encoding/decoding failed
- `Error::Corruption`: WAL corruption detected
- `Error::KeyNotFound`: Key doesn't exist (get operations)
- `Error::InvalidOperation`: Invalid operation for current state

**Example**:
```rust
match db.get(run_id, b"key") {
    Ok(Some(value)) => println!("Found: {:?}", value),
    Ok(None) => println!("Not found"),
    Err(Error::Corruption(msg)) => eprintln!("Corruption: {}", msg),
    Err(e) => eprintln!("Error: {:?}", e),
}
```

---

## Type Aliases

```rust
pub type Result<T> = std::result::Result<T, Error>;
```

All functions return `Result<T>` where errors are of type `Error`.

---

## Feature Flags

Currently no feature flags. All features are enabled by default.

---

## Platform Support

**Tested Platforms**:
- macOS (Darwin)
- Linux
- Windows (planned for M2)

**Requirements**:
- Rust 1.70 or later
- File system with fsync support

---

## Performance Characteristics

### Time Complexity

| Operation | Complexity | Notes |
|-----------|------------|-------|
| `put` | O(log n) | BTreeMap insertion |
| `get` | O(log n) | BTreeMap lookup |
| `delete` | O(log n) | BTreeMap removal |
| `list` | O(k log n) | k = result size |
| `begin_run` | O(1) | UUID generation |
| `end_run` | O(1) | Cleanup |

### Space Complexity

- **Memory**: O(n) where n = total values in database
- **Disk**: O(n + m) where m = WAL size

**Note**: M1 keeps all data in memory. Disk-based storage planned for M6.

---

## Thread Safety

All types are `Send` and `Sync`:

```rust
let db = Database::open("./data")?;
let db = Arc::new(db); // Can share across threads

// Concurrent access is safe
let handle1 = thread::spawn({
    let db = Arc::clone(&db);
    move || {
        let run_id = db.begin_run();
        db.put(run_id, b"key1", b"value1")?;
        db.end_run(run_id)
    }
});

let handle2 = thread::spawn({
    let db = Arc::clone(&db);
    move || {
        let run_id = db.begin_run();
        db.put(run_id, b"key2", b"value2")?;
        db.end_run(run_id)
    }
});
```

**Concurrency Model**: M1 uses RwLock for simplicity. Writers block readers. M2 will introduce OCC (Optimistic Concurrency Control) for non-blocking reads.

---

## Version History

### v0.1.0 (M1 Foundation) - 2026-01-11

**Initial release**:
- ✅ Basic KV operations (put, get, delete, list)
- ✅ Run-scoped operations
- ✅ Write-ahead logging (WAL)
- ✅ Crash recovery
- ✅ Three durability modes
- ✅ TTL support
- ✅ 297 tests, 95.45% coverage

**Limitations**:
- In-memory only (no disk-based storage)
- RwLock concurrency (writers block readers)
- No transactions yet (M2)
- No event log, state machine, trace primitives (M3)

---

## See Also

- [Getting Started Guide](getting-started.md)
- [Architecture Overview](architecture.md)
- [Performance Tuning](performance.md)
- [GitHub Repository](https://github.com/anibjoshi/in-mem)
