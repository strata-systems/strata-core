# Getting Started with in-mem

**in-mem** is a fast, durable, embedded database designed for AI agent workloads. This guide will help you get started quickly.

## Installation

Add `in-mem` to your `Cargo.toml`:

```toml
[dependencies]
in-mem = "0.1"
```

**Note**: Currently in development. M1 Foundation is complete but not yet published to crates.io.

## Quick Start

### Opening a Database

```rust
use in_mem::Database;

// Open or create a database
let db = Database::open("./my-agent-db")?;
```

### Basic Operations

```rust
use in_mem::{Database, RunId};

// Open database
let db = Database::open("./data")?;

// Begin a run (all operations are run-scoped)
let run_id = db.begin_run();

// Put a key-value pair
db.put(run_id, b"user:123", b"Alice")?;

// Get a value
let value = db.get(run_id, b"user:123")?;
assert_eq!(value, Some(b"Alice".to_vec()));

// Delete a key
db.delete(run_id, b"user:123")?;

// End the run
db.end_run(run_id)?;
```

### Using the KV Primitive

```rust
use in_mem::{Database, primitives::KVStore};

let db = Database::open("./data")?;
let kv = KVStore::new(&db);

let run_id = db.begin_run();

// Store structured data
kv.put(run_id, "config:max_retries", 3)?;
kv.put(run_id, "config:timeout_ms", 5000)?;

// Retrieve values
let retries: i64 = kv.get(run_id, "config:max_retries")?.unwrap();
let timeout: i64 = kv.get(run_id, "config:timeout_ms")?.unwrap();

db.end_run(run_id)?;
```

## Key Concepts

### Runs

Every operation in **in-mem** is associated with a `RunId`. Runs represent agent execution sessions and enable:

- **Deterministic Replay**: Reconstruct exact agent state from any run
- **Debugging**: Trace what an agent did during a specific run
- **Isolation**: Separate data from different agent executions

```rust
// Create a new run
let run_id = db.begin_run();

// All operations use this run_id
db.put(run_id, key, value)?;

// End the run when done
db.end_run(run_id)?;
```

### Durability Modes

Control how writes are persisted:

```rust
use in_mem::{Database, DurabilityMode};

// Strict: fsync after every commit (safest, slowest)
let db = Database::open_with_mode("./data", DurabilityMode::Strict)?;

// Batched: fsync every 100ms (balanced, default)
let db = Database::open_with_mode(
    "./data",
    DurabilityMode::Batched { interval_ms: 100, max_commits: 1000 }
)?;

// Async: background fsync (fastest, may lose recent writes on crash)
let db = Database::open_with_mode(
    "./data",
    DurabilityMode::Async { interval_ms: 1000 }
)?;
```

### Data Model

**in-mem** stores data as versioned key-value pairs:

```rust
pub struct Key {
    namespace: Namespace,
    type_tag: TypeTag,
    user_key: Vec<u8>,
}

pub struct VersionedValue {
    value: Value,
    version: u64,      // Monotonically increasing
    timestamp: Timestamp,
    ttl: Option<Duration>,
}
```

Keys are ordered by:
1. Namespace (tenant → app → agent → run)
2. Type tag (KV, Event, StateMachine, Trace, etc.)
3. User key (your application key)

This enables efficient prefix scans and cross-primitive queries.

## Common Patterns

### Time-to-Live (TTL)

Store temporary data that expires automatically:

```rust
use std::time::Duration;

let run_id = db.begin_run();

// Store with 1-hour TTL
db.put_with_ttl(
    run_id,
    b"session:abc123",
    b"user-data",
    Duration::from_secs(3600)
)?;

// Value expires after 1 hour
db.end_run(run_id)?;
```

### Listing Keys

Scan all keys with a prefix:

```rust
let run_id = db.begin_run();

// List all user keys
let users = db.list(run_id, b"user:")?;
for (key, value) in users {
    println!("Found user: {:?}", key);
}

db.end_run(run_id)?;
```

### Crash Recovery

**in-mem** automatically recovers from crashes:

```rust
// First run: write data
{
    let db = Database::open("./data")?;
    let run_id = db.begin_run();
    db.put(run_id, b"key", b"value")?;
    // Crash here! Database is dropped without proper shutdown
}

// Second run: data is recovered
{
    let db = Database::open("./data")?;
    // Automatic recovery happens during open()
    let run_id = db.begin_run();
    let value = db.get(run_id, b"key")?;
    assert_eq!(value, Some(b"value".to_vec())); // Data recovered!
}
```

## Best Practices

### 1. Use Appropriate Durability Mode

- **Strict**: Financial transactions, critical data
- **Batched** (default): Agent workflows, tool outputs
- **Async**: High-throughput logging, caching

### 2. End Runs Properly

Always end runs to release resources:

```rust
let run_id = db.begin_run();

// Do work...

db.end_run(run_id)?; // Don't forget!
```

Or use a guard pattern:

```rust
struct RunGuard<'a> {
    db: &'a Database,
    run_id: RunId,
}

impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        let _ = self.db.end_run(self.run_id);
    }
}
```

### 3. Use Namespaces for Multi-Tenancy

```rust
use in_mem::Namespace;

let namespace = Namespace::new("tenant1", "my-app", "agent-v1", run_id);
// Keys in this namespace are isolated from other tenants
```

### 4. Set TTLs for Temporary Data

Avoid manual cleanup:

```rust
// Session data expires automatically
db.put_with_ttl(run_id, b"session:xyz", data, Duration::from_secs(3600))?;
```

## Next Steps

- [API Reference](api-reference.md) - Complete API documentation
- [Architecture](architecture.md) - How in-mem works internally
- [Primitives Guide](primitives.md) - Using Event Log, State Machine, Trace Store
- [Performance Tuning](performance.md) - Optimization tips

## Troubleshooting

### Database won't open

```rust
// Check permissions and disk space
let db = Database::open("./data")
    .expect("Failed to open database");
```

### High memory usage

- Check for large values being stored
- Verify TTL cleanup is running
- Monitor run count (end runs when done)

### Slow writes

- Check durability mode (Batched or Async may be better)
- Verify disk I/O is not bottleneck
- Consider batching operations

### Data not persisted after crash

- Using `DurabilityMode::Async`? Recent writes may be lost
- Switch to `Batched` or `Strict` for better durability

## Support

- **GitHub Issues**: [anibjoshi/in-mem/issues](https://github.com/anibjoshi/in-mem/issues)
- **Documentation**: [GitHub Pages](https://anibjoshi.github.io/in-mem)

---

**Current Version**: 0.1.0 (M1 Foundation)
**Status**: Production-ready embedded database, network layer in M7
