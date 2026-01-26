# M13: Command Execution Layer (strata-executor)

> **Status**: Planning
> **Author**: Architecture Team
> **Date**: 2026-01-25
> **Prerequisites**: M11 (Primitives Complete)
> **Estimated Scope**: Major architectural addition

---

## Executive Summary

This milestone introduces the **Command Execution Layer** - a standardized interface between all external APIs and the core database engine. Every operation in Strata becomes a `Command` that is executed by a single `Executor` to produce a `Result`.

```
Rust API     Python API     CLI     MCP
     ↓            ↓          ↓        ↓
     └────────────┴──────────┴────────┘
                      ↓
         ┌───────────────────────┐
         │   strata-executor     │
         │  ┌─────────────────┐  │
         │  │ Command (enum)  │  │
         │  │ Output (enum)   │  │
         │  │ Error (enum)    │  │
         │  │ Executor        │  │
         │  └─────────────────┘  │
         └───────────┬───────────┘
                     ↓
         ┌───────────────────────┐
         │   Engine/Primitives   │
         └───────────────────────┘
```

**This is not a wire protocol.** It is a deterministic, in-process execution boundary.

---

## Motivation

### The Problem

Currently, each API surface (and potentially future Python/Node/CLI/MCP surfaces) would need to:
- Understand internal primitive traits
- Enforce Strata's invariants (run scoping, versioning, isolation)
- Handle errors consistently
- Implement the same logic multiple times

This leads to:
- Invariants reimplemented in each SDK
- Bugs that drift between implementations
- Semantic divergence across surfaces
- Fragile replay/export capabilities

### The Solution

A single **Command Execution Layer** that:
- Defines what can happen in Strata (the "instruction set")
- Enforces all invariants exactly once
- Provides a stable, language-agnostic execution model
- Enables true black-box testing
- Makes replay, diffing, and export first-class concepts

---

## Core Principles

### 1. Single Source of Truth

Every mutation, read, or lifecycle operation must be representable as a `Command`.

If something cannot be expressed as a command, it is not part of Strata's public behavior.

### 2. Determinism First

Given the same:
- Initial database state
- Ordered sequence of Commands

The engine must produce the same Results.

This property underpins:
- Crash recovery
- Run replay
- Diffing
- RunBundles
- Correctness guarantees

### 3. Explicit Execution Context

Every Command executes with explicit context:
- Run identity
- Transactional scope (future)
- Durability semantics
- Version visibility

Nothing implicit leaks in from the API layer.

### 4. Transport-Agnostic

Commands are:
- Serializable (for logging, replay, debugging)
- Self-contained
- Synchronous

Commands do not assume:
- Networking
- Async execution
- Remote clients
- Authentication

Transport is a future concern. Semantics are defined now.

---

## Design

### Command Enum

```rust
/// A command is a self-contained, serializable operation.
/// This is the "instruction set" of Strata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    // ==================== KV ====================
    KvGet {
        run: RunId,
        key: String,
    },
    KvPut {
        run: RunId,
        key: String,
        value: Value,
    },
    KvDelete {
        run: RunId,
        key: String,
    },
    KvExists {
        run: RunId,
        key: String,
    },
    KvGetAt {
        run: RunId,
        key: String,
        version: u64,
    },
    KvHistory {
        run: RunId,
        key: String,
        limit: Option<u64>,
    },
    KvScan {
        run: RunId,
        prefix: String,
        limit: Option<u64>,
        cursor: Option<String>,
    },
    KvMget {
        run: RunId,
        keys: Vec<String>,
    },
    KvMput {
        run: RunId,
        entries: Vec<(String, Value)>,
    },
    KvMdelete {
        run: RunId,
        keys: Vec<String>,
    },
    KvIncr {
        run: RunId,
        key: String,
        delta: i64,
    },

    // ==================== JSON ====================
    JsonGet {
        run: RunId,
        key: String,
    },
    JsonSet {
        run: RunId,
        key: String,
        value: Value,
    },
    JsonGetPath {
        run: RunId,
        key: String,
        path: String,
    },
    JsonSetPath {
        run: RunId,
        key: String,
        path: String,
        value: Value,
    },
    JsonDeletePath {
        run: RunId,
        key: String,
        path: String,
    },
    JsonMergePatch {
        run: RunId,
        key: String,
        patch: Value,
    },

    // ==================== Events ====================
    EventAppend {
        run: RunId,
        stream: String,
        event_type: String,
        payload: Value,
    },
    EventRead {
        run: RunId,
        stream: String,
        start: u64,
        limit: u64,
    },
    EventReadRange {
        run: RunId,
        stream: String,
        start: u64,
        end: u64,
    },
    EventReadByType {
        run: RunId,
        stream: String,
        event_type: String,
    },
    EventLatest {
        run: RunId,
        stream: String,
    },
    EventCount {
        run: RunId,
        stream: String,
    },
    EventVerifyChain {
        run: RunId,
        stream: String,
    },

    // ==================== State ====================
    StateGet {
        run: RunId,
        cell: String,
    },
    StateSet {
        run: RunId,
        cell: String,
        value: Value,
    },
    StateTransition {
        run: RunId,
        cell: String,
        from: Value,
        to: Value,
    },
    StateDelete {
        run: RunId,
        cell: String,
    },
    StateList {
        run: RunId,
    },

    // ==================== Vectors ====================
    VectorCreateCollection {
        run: RunId,
        name: String,
        dimensions: usize,
        metric: DistanceMetric,
    },
    VectorDeleteCollection {
        run: RunId,
        name: String,
    },
    VectorInsert {
        run: RunId,
        collection: String,
        id: String,
        embedding: Vec<f32>,
        metadata: Option<Value>,
    },
    VectorSearch {
        run: RunId,
        collection: String,
        query: Vec<f32>,
        k: usize,
        filter: Option<MetadataFilter>,
    },
    VectorGet {
        run: RunId,
        collection: String,
        id: String,
    },
    VectorDelete {
        run: RunId,
        collection: String,
        id: String,
    },
    VectorCount {
        run: RunId,
        collection: String,
    },

    // ==================== Runs ====================
    RunCreate {
        name: Option<String>,
        metadata: Option<Value>,
    },
    RunGet {
        run: RunId,
    },
    RunList {
        status: Option<RunStatus>,
        limit: Option<u64>,
    },
    RunUpdateStatus {
        run: RunId,
        status: RunStatus,
    },
    RunUpdateMetadata {
        run: RunId,
        metadata: Value,
    },
    RunDelete {
        run: RunId,
    },
    RunExport {
        run: RunId,
        path: String,
    },

    // ==================== Database ====================
    Ping,
    Info,
    Flush,
    Compact,
}
```

### Output Enum

```rust
/// Successful command outputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Output {
    /// No return value (e.g., delete, flush)
    Unit,

    /// Single value
    Value(Value),

    /// Value with version metadata
    Versioned {
        value: Value,
        version: u64,
        timestamp: u64,
    },

    /// Optional value (get operations)
    Maybe(Option<Value>),

    /// Optional versioned value
    MaybeVersioned(Option<VersionedValue>),

    /// Multiple optional values (mget)
    Values(Vec<Option<Value>>),

    /// Version number
    Version(u64),

    /// Boolean result
    Bool(bool),

    /// Integer result (count, incr)
    Int(i64),

    /// List of keys
    Keys(Vec<String>),

    /// List of events
    Events(Vec<Event>),

    /// Version history
    History(Vec<VersionedValue>),

    /// Vector search results
    SearchResults(Vec<SearchResult>),

    /// Single run info
    Run(RunInfo),

    /// Multiple run infos
    Runs(Vec<RunInfo>),

    /// Database info
    Info(DatabaseInfo),

    /// Pong response
    Pong { version: String },

    /// Scan results with cursor
    Scan {
        keys: Vec<String>,
        cursor: Option<String>,
    },

    /// Chain verification result
    ChainValid(bool),
}

/// A value with version metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionedValue {
    pub value: Value,
    pub version: u64,
    pub timestamp: u64,
}
```

### Error Enum

```rust
/// Command execution errors.
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
pub enum Error {
    #[error("key not found: {key}")]
    KeyNotFound { key: String },

    #[error("run not found: {run}")]
    RunNotFound { run: String },

    #[error("collection not found: {collection}")]
    CollectionNotFound { collection: String },

    #[error("wrong type: expected {expected}, got {actual}")]
    WrongType { expected: String, actual: String },

    #[error("invalid key: {reason}")]
    InvalidKey { reason: String },

    #[error("invalid path: {reason}")]
    InvalidPath { reason: String },

    #[error("version conflict: expected {expected}, got {actual}")]
    VersionConflict { expected: u64, actual: u64 },

    #[error("transition failed: expected {expected}, got {actual}")]
    TransitionFailed { expected: String, actual: String },

    #[error("run closed: {run}")]
    RunClosed { run: String },

    #[error("run already exists: {run}")]
    RunExists { run: String },

    #[error("collection already exists: {collection}")]
    CollectionExists { collection: String },

    #[error("dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("constraint violation: {reason}")]
    ConstraintViolation { reason: String },

    #[error("history trimmed: requested version {requested} but earliest is {earliest}")]
    HistoryTrimmed { requested: u64, earliest: u64 },

    #[error("overflow: {reason}")]
    Overflow { reason: String },

    #[error("I/O error: {reason}")]
    Io { reason: String },

    #[error("serialization error: {reason}")]
    Serialization { reason: String },

    #[error("internal error: {reason}")]
    Internal { reason: String },
}
```

### The Executor

```rust
/// The command executor - single entry point to Strata's engine.
pub struct Executor {
    engine: Arc<Database>,
    kv: KVStore,
    json: JsonStore,
    events: EventLog,
    state: StateCell,
    vectors: VectorStore,
    runs: RunIndex,
}

impl Executor {
    /// Create a new executor wrapping a database.
    pub fn new(engine: Arc<Database>) -> Self {
        Self {
            kv: KVStore::new(engine.clone()),
            json: JsonStore::new(engine.clone()),
            events: EventLog::new(engine.clone()),
            state: StateCell::new(engine.clone()),
            vectors: VectorStore::new(engine.clone()),
            runs: RunIndex::new(engine.clone()),
            engine,
        }
    }

    /// Execute a single command.
    pub fn execute(&self, cmd: Command) -> Result<Output, Error> {
        match cmd {
            // KV commands
            Command::KvGet { run, key } => self.kv_get(run, key),
            Command::KvPut { run, key, value } => self.kv_put(run, key, value),
            // ... dispatch all commands
        }
    }

    /// Execute multiple commands, returning all results.
    /// Commands execute sequentially. Stops on first error.
    pub fn execute_many(&self, cmds: Vec<Command>) -> Vec<Result<Output, Error>> {
        cmds.into_iter().map(|cmd| self.execute(cmd)).collect()
    }

    /// Execute multiple commands atomically (all or nothing).
    /// Future: transaction support
    pub fn execute_atomic(&self, run: RunId, cmds: Vec<Command>) -> Result<Vec<Output>, Error> {
        // TODO: Wrap in transaction
        todo!()
    }
}
```

---

## Crate Structure

```
crates/executor/
├── Cargo.toml
└── src/
    ├── lib.rs           # Public API, re-exports
    ├── command.rs       # Command enum
    ├── output.rs        # Output enum
    ├── error.rs         # Error enum
    ├── executor.rs      # Executor implementation
    ├── convert.rs       # Conversions from internal errors
    └── json.rs          # JSON utilities (for CLI/MCP output)
```

### Dependencies

```toml
[package]
name = "strata-executor"
version = "0.1.0"
edition = "2021"

[dependencies]
strata-core = { path = "../core" }
strata-engine = { path = "../engine" }
strata-primitives = { path = "../primitives" }
serde = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
serde_json = { workspace = true }
```

---

## Implementation Phases

### Phase 1: Core Types (Command, Output, Error)

**Goal**: Define the canonical types that represent all Strata operations.

#### Tasks

- [ ] Create `crates/executor/` directory
- [ ] Create `Cargo.toml` with dependencies
- [ ] Implement `Command` enum with all variants
- [ ] Implement `Output` enum with all variants
- [ ] Implement `Error` enum with all variants
- [ ] Add `Serialize`/`Deserialize` derives
- [ ] Write unit tests for serialization roundtrips

#### Validation

```rust
// Every command should serialize/deserialize cleanly
let cmd = Command::KvPut {
    run: RunId::default(),
    key: "test".into(),
    value: Value::Int(42),
};
let json = serde_json::to_string(&cmd)?;
let decoded: Command = serde_json::from_str(&json)?;
assert_eq!(cmd, decoded);
```

---

### Phase 2: Executor Implementation

**Goal**: Implement the executor that dispatches commands to primitives.

#### Tasks

- [ ] Create `Executor` struct
- [ ] Implement `execute()` method with match dispatch
- [ ] Implement each command handler:
  - [ ] KV commands (12 variants)
  - [ ] JSON commands (6 variants)
  - [ ] Event commands (7 variants)
  - [ ] State commands (5 variants)
  - [ ] Vector commands (7 variants)
  - [ ] Run commands (7 variants)
  - [ ] Database commands (4 variants)
- [ ] Implement error conversion from internal errors
- [ ] Implement `execute_many()`

#### Validation

```rust
let db = Database::open_in_memory()?;
let executor = Executor::new(Arc::new(db));

let result = executor.execute(Command::KvPut {
    run: RunId::default(),
    key: "foo".into(),
    value: Value::String("bar".into()),
})?;
assert!(matches!(result, Output::Version(_)));

let result = executor.execute(Command::KvGet {
    run: RunId::default(),
    key: "foo".into(),
})?;
assert!(matches!(result, Output::MaybeVersioned(Some(_))));
```

---

### Phase 3: JSON Utilities

**Goal**: Provide JSON serialization for CLI and MCP output.

#### Tasks

- [ ] Create `json.rs` module
- [ ] Implement `Value` → JSON encoding (from strata-wire, cleaned up)
- [ ] Handle special cases (`$bytes`, `$f64` wrappers)
- [ ] Implement `Output` → JSON for human-readable output
- [ ] Implement `Error` → JSON for error reporting

#### Note

This is NOT a wire protocol. It's utility code for:
- CLI output formatting
- MCP payload construction
- Debugging/logging

---

### Phase 4: Integration

**Goal**: Wire the executor into the existing API layer.

#### Tasks

- [ ] Add `strata-executor` to workspace
- [ ] Update `strata-api` to use executor (or prepare for M12)
- [ ] Update tests to use executor where appropriate
- [ ] Write integration tests for full command flows

---

### Phase 5: Remove strata-wire

**Goal**: Clean up the codebase by removing the unused wire crate.

#### Tasks

- [ ] Delete `crates/wire/` directory
- [ ] Remove from workspace `Cargo.toml`
- [ ] Update documentation references
- [ ] Update test reports

---

## What This Enables

### 1. True Black-Box Testing

Test the engine by feeding command sequences and asserting results:

```rust
#[test]
fn test_kv_put_get_roundtrip() {
    let executor = setup_executor();

    executor.execute(Command::KvPut {
        run: RunId::default(),
        key: "k".into(),
        value: Value::Int(42),
    }).unwrap();

    let result = executor.execute(Command::KvGet {
        run: RunId::default(),
        key: "k".into(),
    }).unwrap();

    assert_eq!(result.unwrap_value(), Value::Int(42));
}
```

No Rust types leaking, no internal trait coupling.

### 2. Deterministic Replay

Commands can be logged and replayed:

```rust
// Log commands
for cmd in commands {
    log.append(&serde_json::to_string(&cmd)?);
    executor.execute(cmd)?;
}

// Replay on fresh database
let executor2 = Executor::new(fresh_db);
for line in log.lines() {
    let cmd: Command = serde_json::from_str(line)?;
    executor2.execute(cmd)?;
}
// Databases now identical
```

### 3. Thin SDKs

Python, Node, CLI, MCP become trivial:

```python
# Python SDK (pseudocode)
def put(self, key: str, value: Any) -> int:
    cmd = {"KvPut": {"run": self.run_id, "key": key, "value": value}}
    result = self._executor.execute(cmd)
    return result["Version"]
```

SDKs don't need to understand primitives or enforce invariants.

### 4. RunBundle Integration

Commands provide the semantic log for RunBundle:

```rust
pub struct RunBundle {
    pub metadata: RunInfo,
    pub commands: Vec<Command>,  // Semantic history
    pub snapshots: Vec<Snapshot>,
}
```

---

## Success Criteria

1. **Complete Coverage**: Every primitive operation has a Command variant
2. **Type Safety**: Commands are fully typed, no `Generic(Value)` fallback
3. **Serializable**: All types serialize/deserialize cleanly
4. **Tested**: Every command variant has execution tests
5. **Integrated**: Executor can be used standalone or via existing API
6. **Clean**: strata-wire removed, no dead code

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Large enum becomes unwieldy | Group by primitive, good docs |
| Performance overhead | Measure; dispatch is cheap |
| Breaking existing code | Executor is additive; old API unchanged |
| Missing edge cases | Comprehensive test coverage |

---

## Future Extensions

### Transaction Batching

```rust
executor.execute_atomic(run, vec![
    Command::KvPut { ... },
    Command::EventAppend { ... },
])?;
```

### Command Middleware

```rust
executor.with_middleware(LoggingMiddleware::new())
        .with_middleware(MetricsMiddleware::new())
        .execute(cmd)?;
```

### Async Execution

```rust
executor.execute_async(cmd).await?;
```

These are future enhancements. The initial implementation is synchronous and in-process.

---

## Appendix: Command Count

| Primitive | Commands |
|-----------|----------|
| KV | 12 |
| JSON | 6 |
| Events | 7 |
| State | 5 |
| Vectors | 7 |
| Runs | 7 |
| Database | 4 |
| **Total** | **48** |

This matches the complete API surface of Strata's primitives.
