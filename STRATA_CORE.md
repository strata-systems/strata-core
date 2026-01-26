# Strata-Core Crate: Complete Detailed Analysis

## Overview

`strata-core` is the **foundational crate** of the Strata database. It has **zero dependencies** on any other Strata crates and defines all the canonical types, traits, and contracts that the entire system is built upon.

**Location**: `crates/core/`

**Cargo.toml Dependencies**:
- `serde` (with `derive` feature) - serialization
- `serde_json` - JSON handling
- `thiserror` - error type derivation
- `uuid` (with `v4`, `serde` features) - UUID generation

---

## Module Structure

```
crates/core/src/
├── lib.rs              # Public API exports
├── types.rs            # RunId, Namespace, TypeTag, JsonDocId
├── key.rs              # Composite Key type + key validation utilities
├── value.rs            # Universal Value enum
├── traits.rs           # Storage and SnapshotView traits
├── error.rs            # StrataError, ErrorCode, WireError (~100KB)
├── api_error.rs        # ApiError for wire encoding
├── limits.rs           # System-wide limits (keys, strings, vectors, nesting)
├── json.rs             # JsonValue, JsonPath, JsonPatch, PathSegment
├── search_types.rs     # SearchRequest, SearchResponse, SearchHit, etc.
├── run_types.rs        # RunStatus, RunMetadata, RunEventOffsets
├── contract/           # API contract types (Seven Invariants)
│   ├── mod.rs
│   ├── entity_ref.rs   # EntityRef - universal entity addressing
│   ├── versioned.rs    # Versioned<T> generic wrapper
│   ├── version.rs      # Version enum (Txn, Sequence, Counter)
│   ├── timestamp.rs    # Microsecond-precision Timestamp
│   ├── primitive_type.rs # PrimitiveType enum (6 primitives)
│   └── run_name.rs     # RunName with validation
└── primitives/         # Canonical primitive data structures
    ├── mod.rs
    ├── event.rs        # Event, ChainVerification
    ├── state.rs        # State (for StateCell)
    └── vector.rs       # VectorEntry, VectorMatch, VectorConfig, etc.
```

---

## 1. Core Identity Types (`types.rs`)

### RunId
```rust
pub struct RunId(Uuid);
```
- **Purpose**: Unique identifier for an execution run (agent session)
- **Implementation**: UUID v4 (random), 16 bytes
- **Key Methods**:
  - `new()` - Generate new random RunId
  - `nil()` - Nil UUID (all zeros)
  - `from_bytes()`/`as_bytes()` - Raw byte access
  - `from_str()`/`to_string()` - String conversion
- **Traits**: Copy, Clone, PartialEq, Eq, Hash, Ord, Serialize, Deserialize

### Namespace
```rust
pub struct Namespace {
    pub tenant: String,
    pub app: String,
    pub agent: String,
    pub run_id: RunId,
}
```
- **Purpose**: Hierarchical scoping for multi-tenancy
- **Format**: `tenant/app/agent/run_id`
- **Default Factory**: `Namespace::default_for_run(run_id)` creates `"default/default/default/{run_id}"`
- **Additional Methods**:
  - `for_run(run_id)` - Create namespace scoped to a specific run (same as `default_for_run`)

### TypeTag
```rust
#[repr(u8)]
pub enum TypeTag {
    KV = 0x01,           // Key-Value primitive data
    Event = 0x02,        // Event log entries
    State = 0x03,        // State cell records
    Trace = 0x04,        // DEPRECATED since 0.12.0 (reserved)
    Run = 0x05,          // Run index entries
    Vector = 0x10,       // Vector store entries
    Json = 0x11,         // JSON document store entries
    VectorConfig = 0x12, // Vector collection configuration
}
```
- **Purpose**: Discriminates storage entries by primitive type
- **Key Methods**: `as_u8()`, `from_u8()`, `as_str()`
- **Note**: Values are hex discriminants, not sequential integers

### JsonDocId
```rust
pub struct JsonDocId(Uuid);
```
- **Purpose**: Unique identifier for JSON documents
- **Implementation**: UUID v4, same pattern as RunId

> **Note**: There is no standalone `TxnId` type in strata-core. Transaction IDs are represented as `u64` counters in the legacy system or `TxId(Uuid)` in the modern durability system (defined in strata-durability).

---

## 2. Composite Key (`types.rs`)

```rust
pub struct Key {
    pub namespace: Namespace,
    pub type_tag: TypeTag,
    pub user_key: Vec<u8>,  // Binary user-defined key bytes
}
```

- **Purpose**: Universal key for all storage operations
- **Note**: `user_key` is `Vec<u8>` to support arbitrary binary keys, not just UTF-8 strings
- **Format**: `{namespace}/{type_tag}/{user_key}`
- **Key Methods**:
  - `new(namespace, type_tag, user_key)` - Full construction
  - `for_kv(namespace, key)` - KV shorthand
  - `for_event(namespace, seq)` - Event shorthand (key = sequence number)
  - `for_state(namespace, name)` - StateCell shorthand
  - `for_json(namespace, doc_id)` - JSON doc shorthand
  - `for_vector(namespace, collection, vector_key)` - Vector shorthand
  - `run_id()` - Extract RunId from key
  - `to_bytes()` / `from_bytes()` - Binary serialization
- **Traits**: Clone, PartialEq, Eq, Hash, Ord (for BTreeMap ordering)

### Key Validation (`key.rs`)

The module also provides key validation utilities:

```rust
pub fn validate_key(key: &str) -> Result<(), KeyError>;

pub enum KeyError {
    Empty,           // Key cannot be empty
    TooLong,         // Key exceeds MAX_KEY_SIZE (4096 bytes)
    ContainsNul,     // Key contains NUL byte
    ReservedPrefix,  // Key starts with reserved prefix (e.g., "__")
}
```

**Additional Key Methods** (undocumented in overview but public):
- `Key::new_event_meta(namespace)` - For event log metadata
- `Key::new_run_index(namespace)` - For secondary run indexes
- `Key::new_vector_config(namespace, collection)` - For vector collection config
- `Key::vector_collection_prefix(namespace)` - For prefix scans of vector collections
- `Key::new_json_prefix(namespace)` - For JSON prefix scans
- `Key::user_key_string()` - Extract user key as UTF-8 string

---

## 3. Universal Value (`value.rs`)

```rust
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
}
```

- **Purpose**: Universal value type for all primitives
- **Type Checking**: `is_null()`, `is_bool()`, `is_int()`, `is_float()`, `is_string()`, `is_bytes()`, `is_array()`, `is_object()`
- **Extraction**: `as_bool()`, `as_int()`, `as_float()`, `as_str()`, `as_bytes()`, `as_array()`, `as_object()`
- **Conversion**: `into_bool()`, `into_string()`, `into_bytes()`, `into_array()`, `into_object()`
- **From Implementations**: From<bool>, From<i64>, From<String>, From<Vec<u8>>, From<Vec<Value>>, From<HashMap<String, Value>>, From<serde_json::Value>

---

## 4. Core Traits (`traits.rs`)

### Storage Trait
```rust
pub trait Storage: Send + Sync {
    fn get(&self, key: &Key) -> Result<Option<VersionedValue>>;

    fn get_versioned(&self, key: &Key, max_version: u64) -> Result<Option<VersionedValue>>;

    fn get_history(
        &self,
        key: &Key,
        limit: Option<usize>,
        before_version: Option<u64>,
    ) -> Result<Vec<VersionedValue>>;

    fn put(&self, key: Key, value: Value, ttl: Option<Duration>) -> Result<u64>;

    fn delete(&self, key: &Key) -> Result<Option<VersionedValue>>;

    fn scan_prefix(&self, prefix: &Key, max_version: u64) -> Result<Vec<(Key, VersionedValue)>>;

    fn scan_by_run(&self, run_id: RunId, max_version: u64) -> Result<Vec<(Key, VersionedValue)>>;

    fn current_version(&self) -> u64;

    fn put_with_version(
        &self,
        key: Key,
        value: Value,
        version: u64,
        ttl: Option<Duration>,
    ) -> Result<()>;

    fn delete_with_version(&self, key: &Key, version: u64) -> Result<Option<VersionedValue>>;
}
```
- **Implementors**: `UnifiedStore`, `ShardedStore`
- **Purpose**: Abstract interface for in-memory storage backends
- **Note**: Returns `VersionedValue` (not `StoredValue`), supports TTL and version history

### SnapshotView Trait
```rust
pub trait SnapshotView: Send + Sync {
    fn get(&self, key: &Key) -> Result<Option<VersionedValue>>;
    fn max_version(&self) -> u64;
}
```
- **Purpose**: Read-only snapshot for transaction isolation
- **Implementors**: `ClonedSnapshotView`
- **Note**: Minimal interface - only `get` and `max_version` methods

### VersionedValue (Type Alias)
```rust
pub type VersionedValue = Versioned<Value>;
```
- **Purpose**: The standard versioned value type used throughout the system
- **Note**: There is no `StoredValue` type - use `VersionedValue` instead

---

## 5. Contract Types (`contract/` module)

These types express the **Seven Invariants** that all primitives must follow:

### Invariant 1: Everything is Addressable - `EntityRef`
```rust
pub enum EntityRef {
    Kv { run_id: RunId, key: String },
    Event { run_id: RunId, sequence: u64 },
    State { run_id: RunId, name: String },
    Run { run_id: RunId },
    Json { run_id: RunId, doc_id: JsonDocId },
    Vector { run_id: RunId, collection: String, key: String },
}
```
- **Purpose**: Universal reference to any entity
- **Constructors**: `kv()`, `event()`, `state()`, `run()`, `json()`, `vector()`
- **Display Format**: `kv://{run_id}/{key}`, `event://{run_id}/{seq}`, etc.

### Invariant 2: Everything is Versioned - `Version` and `Versioned<T>`

**Version** (three variants for different primitives):
```rust
pub enum Version {
    Txn(u64),      // Transaction-based (KV, Json, Vector, Run)
    Sequence(u64), // Position-based (EventLog)
    Counter(u64),  // Per-entity counter (StateCell CAS)
}
```

**Versioned<T>** (generic wrapper):
```rust
pub struct Versioned<T> {
    pub value: T,
    pub version: Version,
    pub timestamp: Timestamp,
}
```
- **Key Methods**:
  - `new(value, version)` - Create with current timestamp
  - `with_timestamp(value, version, timestamp)` - Create with explicit timestamp
  - `map<U>(f: impl FnOnce(T) -> U)` - Transform value, preserving version/timestamp
  - `into_value()` - Extract value, discarding metadata
  - `is_older_than(&other)` - Compare timestamps
  - `age()` - Duration since creation
- **Type Alias**: `VersionedValue = Versioned<Value>`

### Invariant 2 (temporal): `Timestamp`
```rust
pub struct Timestamp(u64); // Microseconds since Unix epoch
```
- **Constants**: `EPOCH`, `MAX`
- **Constructors**: `now()`, `from_micros()`, `from_millis()`, `from_secs()`
- **Accessors**: `as_micros()`, `as_millis()`, `as_secs()`
- **Operations**: `duration_since()`, `saturating_add()`, `saturating_sub()`, `is_before()`, `is_after()`

> **Note: Timestamp Unit Inconsistency**
> The `Timestamp` type uses **microseconds** internally. However, the primitive data types (`Event.timestamp`, `State.updated_at`) use **milliseconds** as `i64`. This 1000x difference can cause subtle bugs when mixing these values. See STRATA_CORE_REVIEW.md for details.

### Invariant 5: Run-Scoped - `RunName`
```rust
pub struct RunName(String);
```
- **Purpose**: User-facing semantic identifier (vs RunId which is internal UUID)
- **Validation Rules**:
  - Length: 1-256 characters
  - Characters: `[a-zA-Z0-9_.-]`
  - Cannot start with `-` or `.`
- **Error Type**: `RunNameError { Empty, TooLong, InvalidChar, InvalidStart }`

### Invariant 6: Introspectable - `PrimitiveType`
```rust
pub enum PrimitiveType {
    Kv,     // Key-value store
    Event,  // Append-only event log
    State,  // State cells with CAS
    Run,    // Run lifecycle
    Json,   // JSON documents
    Vector, // Vector similarity search
}
```
- **Key Methods**:
  - `name()` - Human-readable display name (e.g., "Key-Value Store")
  - `id()` - Short identifier for serialization (e.g., "kv")
  - `from_id(&str)` - Parse from string identifier
  - `supports_crud()` - Query if primitive supports CRUD operations
  - `is_append_only()` - Query if primitive is append-only (Event)
  - `all()` - Returns slice of all primitives `&'static [PrimitiveType]`
- **Constant**: `ALL: [PrimitiveType; 6]`

---

## 6. Primitive Data Types (`primitives/` module)

### Event (for EventLog)
```rust
pub struct Event {
    pub sequence: u64,        // Auto-assigned monotonic number
    pub event_type: String,   // User-defined category
    pub payload: Value,       // Arbitrary data
    pub timestamp: i64,       // Milliseconds since epoch
    pub prev_hash: [u8; 32],  // Hash of previous event (chaining)
    pub hash: [u8; 32],       // Hash of this event
}
```

### ChainVerification (for EventLog)
```rust
pub struct ChainVerification {
    pub is_valid: bool,
    pub length: u64,
    pub first_invalid: Option<u64>,
    pub error: Option<String>,
}
```

### State (for StateCell)
```rust
pub struct State {
    pub value: Value,
    pub version: u64,
    pub updated_at: i64,
}
```

### Vector Types (for VectorStore)

**VectorId** (internal identifier):
```rust
pub struct VectorId(pub u64);
```
- **Purpose**: Internal vector identifier (stable within collection)
- **Key Methods**: `new(u64)`, `as_u64()`
- **Important**: VectorIds are never reused; storage slots may be reused but ID values are monotonically increasing

**CollectionId** (unique collection reference):
```rust
pub struct CollectionId {
    pub run_id: RunId,
    pub name: String,
}
```
- **Purpose**: Unique identifier for a collection within a run
- **Constructor**: `new(run_id, name)`

**CollectionInfo** (collection metadata):
```rust
pub struct CollectionInfo {
    pub name: String,
    pub config: VectorConfig,
    pub count: usize,
    pub created_at: u64,  // Microseconds since epoch
}
```

**DistanceMetric**:
```rust
pub enum DistanceMetric {
    Cosine,      // dot(a,b) / (||a|| * ||b||), range [-1, 1]
    Euclidean,   // 1 / (1 + l2_distance), range (0, 1]
    DotProduct,  // Raw dot product (unbounded)
}
```

**VectorConfig** (immutable after creation):
```rust
pub struct VectorConfig {
    pub dimension: usize,
    pub metric: DistanceMetric,
    pub storage_dtype: StorageDtype, // Only F32 currently
}
```
- **Presets**: `for_openai_ada()` (1536d), `for_openai_large()` (3072d), `for_minilm()` (384d), `for_mpnet()` (768d)

**VectorEntry**:
```rust
pub struct VectorEntry {
    pub key: String,
    pub embedding: Vec<f32>,
    pub metadata: Option<serde_json::Value>,
    pub vector_id: VectorId,
    pub version: u64,
    pub source_ref: Option<EntityRef>,
}
```

**VectorMatch** (search result):
```rust
pub struct VectorMatch {
    pub key: String,
    pub score: f32,  // Higher = more similar (normalized)
    pub metadata: Option<serde_json::Value>,
}
```

**MetadataFilter** (for search):
```rust
pub struct MetadataFilter {
    pub equals: HashMap<String, JsonScalar>,
}
```

---

## 7. JSON Types (`json.rs`)

### Document Size Limits

| Limit | Value | Constant |
|-------|-------|----------|
| Max document size | 16 MB | `MAX_DOCUMENT_SIZE` |
| Max nesting depth | 100 levels | `MAX_NESTING_DEPTH` |
| Max path length | 256 segments | `MAX_PATH_LENGTH` |
| Max array size | 1M elements | `MAX_ARRAY_SIZE` |

### JsonValue
```rust
pub struct JsonValue(serde_json::Value);
```
- **Newtype wrapper** around serde_json::Value
- **Constructors**: `null()`, `object()`, `array()`, `from_value()`
- **Validation**: `validate_size()`, `validate_depth()`, `validate_array_size()`, `validate()`
- **Analysis**: `size_bytes()`, `nesting_depth()`, `max_array_size()`
- **Traits**: Deref, DerefMut to serde_json::Value

### PathSegment
```rust
pub enum PathSegment {
    Key(String),   // Object property: `.foo`
    Index(usize),  // Array index: `[0]`
}
```

### JsonPath
```rust
pub struct JsonPath {
    segments: Vec<PathSegment>,
}
```
- **Constructors**: `root()`, `from_segments()`
- **Builder**: `key()`, `index()` (chainable)
- **Navigation**: `parent()`, `last_segment()`
- **Relationships**: `is_ancestor_of()`, `is_descendant_of()`, `overlaps()`, `common_ancestor()`
- **Conflict Detection**: `is_affected_by()` - checks if path would be affected by a write
- **Parsing**: Supports `FromStr` - `"user.name"`, `"items[0].foo"`

### JsonPatch
```rust
pub enum JsonPatch {
    Set { path: JsonPath, value: JsonValue },
    Delete { path: JsonPath },
}
```
- **Subset of RFC 6902** - only Set and Delete operations
- **Not Supported**: add, test, move, copy (reserved for future)
- **Conflict Detection**: `conflicts_with()` checks path overlap

---

## 8. Search Types (`search_types.rs`)

### SearchBudget
```rust
pub struct SearchBudget {
    pub max_wall_time_micros: u64,           // Default: 100,000 (100ms)
    pub max_candidates: usize,               // Default: 10,000
    pub max_candidates_per_primitive: usize, // Default: 2,000
}
```

### SearchMode
```rust
pub enum SearchMode {
    Keyword, // BM25-lite (default)
    Vector,  // Reserved
    Hybrid,  // Reserved
}
```

### SearchRequest
```rust
pub struct SearchRequest {
    pub run_id: RunId,
    pub query: String,
    pub k: usize,              // Top-k results (default: 10)
    pub budget: SearchBudget,
    pub mode: SearchMode,
    pub primitive_filter: Option<Vec<PrimitiveType>>,
    pub time_range: Option<(u64, u64)>,
    pub tags_any: Vec<String>,
}
```

### SearchHit
```rust
pub struct SearchHit {
    pub doc_ref: EntityRef,
    pub score: f32,
    pub rank: u32,
    pub snippet: Option<String>,
}
```

### SearchResponse
```rust
pub struct SearchResponse {
    pub hits: Vec<SearchHit>,
    pub truncated: bool,
    pub stats: SearchStats,
}
```

### SearchStats
```rust
pub struct SearchStats {
    pub elapsed_micros: u64,
    pub candidates_considered: usize,
    pub candidates_by_primitive: HashMap<PrimitiveType, usize>,
    pub index_used: bool,
}
```

---

## 9. Run Lifecycle Types (`run_types.rs`)

### RunStatus
```rust
pub enum RunStatus {
    Active,     // begin_run called, end_run not yet
    Completed,  // end_run called
    Orphaned,   // Crash without end_run marker
    NotFound,   // Run doesn't exist
}
```

### RunMetadata
```rust
pub struct RunMetadata {
    pub run_id: RunId,
    pub status: RunStatus,
    pub started_at: u64,
    pub ended_at: Option<u64>,
    pub event_count: u64,
    pub begin_wal_offset: u64,
    pub end_wal_offset: Option<u64>,
}
```

### RunEventOffsets
```rust
pub struct RunEventOffsets {
    pub offsets: Vec<u64>,  // WAL offsets for O(run size) replay
}
```

---

## 10. Error Model (`error.rs`)

### StrataError (Main Error Type)
```rust
pub enum StrataError {
    NotFound { entity: String, message: String },
    WrongType { expected: String, actual: String, message: String },
    InvalidKey { key: String, message: String },
    InvalidPath { path: String, message: String },
    HistoryTrimmed { version: u64, message: String },
    ConstraintViolation { constraint: String, message: String },
    Conflict { message: String, details: Option<ConflictDetails> },
    SerializationError { message: String },
    StorageError { message: String },
    InternalError { message: String },
    InvalidInput { message: String },
}
```

### ErrorCode (Canonical Wire Codes)

The following **10 error codes** are the canonical wire representation:

```rust
pub enum ErrorCode {
    NotFound,           // Entity/key not found
    WrongType,          // Wrong primitive/value type
    InvalidKey,         // Key syntax invalid
    InvalidPath,        // JSON path invalid
    HistoryTrimmed,     // Requested version unavailable
    ConstraintViolation,// Structural failure
    Conflict,           // Temporal failure (retryable)
    SerializationError, // Invalid Value encoding
    StorageError,       // Disk/WAL failure
    InternalError,      // Bug/invariant violation
}
```

### Error Classification

| Type | Examples | Resolution |
|------|----------|------------|
| **Temporal failures (Conflict)** | Version conflicts, write conflicts, transaction aborts | Retryable - may succeed with fresh data |
| **Structural failures (ConstraintViolation)** | Invalid input, dimension mismatch, capacity exceeded | Requires input changes |

### Wire Encoding
```json
{
  "code": "NotFound",
  "message": "Entity not found: kv:default/config",
  "details": { "entity": "kv:default/config" }
}
```

---

## 11. System Limits (`limits.rs`)

The `limits.rs` module provides **system-wide limits** (not just JSON limits):

### Limits Struct
```rust
pub struct Limits {
    pub max_key_bytes: usize,       // Default: 4096 (4 KB)
    pub max_string_bytes: usize,    // Default: 16 MB
    pub max_array_length: usize,    // Default: 1,000,000
    pub max_nesting_depth: usize,   // Default: 100
    pub max_vector_dimension: usize,// Default: 4096
    pub max_metadata_bytes: usize,  // Default: 64 KB
    pub max_batch_size: usize,      // Default: 1000
    pub max_scan_limit: usize,      // Default: 10,000
}
```

### LimitError Enum
```rust
pub enum LimitError {
    KeyTooLong { max: usize, actual: usize },
    ValueTooLarge { max: usize, actual: usize },
    StringTooLong { max: usize, actual: usize },
    ArrayTooLong { max: usize, actual: usize },
    NestingTooDeep { max: usize, actual: usize },
    VectorDimensionTooLarge { max: usize, actual: usize },
    MetadataTooLarge { max: usize, actual: usize },
    BatchTooLarge { max: usize, actual: usize },
}
```

### Validation Methods
- `Limits::validate_key(&str)` - Validate key size
- `Limits::validate_value(&Value)` - Validate value size
- `Limits::validate_vector(&[f32])` - Validate vector dimension

### Constants (for convenience)
```rust
pub const MAX_KEY_SIZE: usize = 4096;           // 4 KB
pub const MAX_VALUE_SIZE: usize = 16_777_216;   // 16 MB
pub const MAX_BATCH_SIZE: usize = 1000;         // Operations per batch
pub const MAX_SCAN_LIMIT: usize = 10_000;       // Results per scan
pub const MAX_COLLECTIONS_PER_RUN: usize = 100; // Vector collections
pub const MAX_VECTOR_DIMENSION: usize = 4096;   // Embedding dimensions
```

> **Note**: The `json.rs` module has its own `LimitError` enum for JSON-specific limits (document size, nesting depth, array size). These are separate from the system-wide limits in `limits.rs`.

---

## Summary: What strata-core Provides

| Category | Types | Purpose |
|----------|-------|---------|
| **Identity** | RunId, JsonDocId, Namespace, TypeTag | Unique identifiers |
| **Storage Key** | Key, VersionedValue | Universal storage addressing |
| **Values** | Value (enum) | Universal value representation |
| **Traits** | Storage, SnapshotView | Backend abstraction |
| **Contract** | EntityRef, Version, Versioned<T>, Timestamp, PrimitiveType, RunName | API invariants |
| **Primitives** | Event, State, VectorEntry, VectorConfig, VectorId, CollectionId, CollectionInfo | Primitive-specific data |
| **JSON** | JsonValue, JsonPath, JsonPatch, PathSegment | JSON document support |
| **Search** | SearchRequest, SearchResponse, SearchHit, SearchBudget | Search infrastructure |
| **Lifecycle** | RunStatus, RunMetadata, RunEventOffsets | Run management |
| **Errors** | StrataError, ErrorCode, ApiError, WireError | Error handling |
| **Limits** | Various constants | System boundaries |

---

## The Seven Invariants

The contract types in strata-core express these invariants that all primitives must follow:

1. **Addressable**: Every entity has a stable identity via `EntityRef`
2. **Versioned**: Every read returns `Versioned<T>`, every write returns `Version`
3. **Transactional**: Every primitive participates in transactions
4. **Lifecycle**: Every primitive follows create/exist/evolve/destroy
5. **Run-scoped**: Every entity belongs to exactly one run
6. **Introspectable**: Every primitive has `exists()` or equivalent
7. **Read/Write**: Reads never modify state, writes always produce versions

---

## Dependency Position

This crate is the **single source of truth** for all type definitions in Strata. Every other crate depends on it, and it depends on nothing else in the Strata workspace.

```
strata-core (no Strata dependencies)
    ↑
    ├── strata-storage
    ├── strata-concurrency
    ├── strata-durability
    ├── strata-engine
    ├── strata-primitives
    ├── strata-search
    ├── strata-api
    └── strata-executor
```
