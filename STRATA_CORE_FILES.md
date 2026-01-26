# strata-core: File-by-File Documentation

This document describes each file in the `strata-core` crate in plain English. The crate provides foundational types with zero dependencies on other Strata crates.

---

## Root Module

### `lib.rs` (81 lines)
**Purpose:** Entry point for the crate that declares all modules and re-exports commonly used types.

- Declares public modules: `types`, `key`, `value`, `traits`, `error`, `api_error`, `run_types`, `limits`, `search_types`, `json`, `contract`, `primitives`
- Re-exports frequently used types at crate root for convenience (e.g., `RunId`, `Value`, `Key`, `Versioned`, `EntityRef`)
- Contains a deprecated `placeholder()` function that should be removed

---

## Core Types

### `types.rs` (1639 lines)
**Purpose:** Defines the fundamental identity types used throughout the system.

**Key Types:**
- **`RunId`**: A UUID v4 wrapper that uniquely identifies a run. Immutable, Copy-able, serializable. Created with `RunId::new()` or `RunId::parse()`.
- **`Namespace`**: Hierarchical scoping in the format `tenant/app/agent/run_id`. Supports construction from parts and string parsing.
- **`TypeTag`**: A single-byte discriminator enum that identifies primitive types. Uses hex values: `0x01`=KV, `0x02`=Event, `0x03`=State, `0x04`=Run, `0x05`=Json, `0x06`=Vector, etc.
- **`JsonDocId`**: A UUID v4 wrapper for JSON document identifiers.
- **`Key`**: A composite key combining `namespace`, `type_tag`, and `user_key` (which is `Vec<u8>`, supporting binary keys).

---

### `key.rs` (347 lines)
**Purpose:** Key validation rules and error types.

**Validation Rules:**
- Keys must be non-empty
- Keys cannot contain NUL bytes (`\x00`)
- Keys cannot start with reserved prefix (`__strata_`)
- Keys must not exceed maximum length (1024 bytes by default)

**Key Type:**
- **`KeyError`**: Enum with variants `Empty`, `ContainsNul`, `ReservedPrefix`, `TooLong`
- Provides `validate_key()` function to check key validity

---

### `value.rs` (531 lines)
**Purpose:** The universal value type for storing arbitrary data.

**The `Value` Enum (8 variants):**
1. `Null` - Represents absence of value
2. `Bool(bool)` - Boolean true/false
3. `Int(i64)` - 64-bit signed integer
4. `Float(f64)` - 64-bit IEEE-754 floating point
5. `String(String)` - UTF-8 string
6. `Bytes(Vec<u8>)` - Raw binary data
7. `Array(Vec<Value>)` - Ordered collection
8. `Object(HashMap<String, Value>)` - Key-value map

**Special Behaviors:**
- Custom `PartialEq` implementation: NaN != NaN, -0.0 == 0.0 (IEEE-754 semantics)
- Implements `From` traits for ergonomic construction (e.g., `Value::from("hello")`)
- Type checking methods: `is_null()`, `is_bool()`, `is_int()`, etc.
- Accessor methods: `as_bool()`, `as_int()`, `as_str()`, etc.

---

### `traits.rs` (259 lines)
**Purpose:** Defines the core storage abstraction traits.

**`Storage` Trait (10 methods):**
- `get(&self, key: &Key) -> Result<Option<Value>>` - Get current value
- `get_versioned(&self, key: &Key) -> Result<Option<Versioned<Value>>>` - Get with version info
- `get_history(&self, key: &Key, limit: usize) -> Result<Vec<Versioned<Value>>>` - Get version history
- `put(&self, key: &Key, value: Value) -> Result<Version>` - Store value
- `delete(&self, key: &Key) -> Result<bool>` - Remove value
- `scan_prefix(&self, prefix: &Key) -> Result<Vec<(Key, Value)>>` - Prefix scan
- `scan_by_run(&self, run_id: RunId) -> Result<Vec<(Key, Value)>>` - Scan by run
- `current_version(&self) -> Version` - Get current storage version
- `put_with_version(&self, key: &Key, value: Value, expected: Version) -> Result<Version>` - CAS put
- `delete_with_version(&self, key: &Key, expected: Version) -> Result<bool>` - CAS delete

**`SnapshotView` Trait (3 methods):**
- `get(&self, key: &Key) -> Option<&Value>` - Point-in-time read
- `scan_prefix(&self, prefix: &Key) -> Vec<(Key, Value)>` - Snapshot prefix scan
- `version(&self) -> Version` - Snapshot version

---

### `error.rs` (Large file, ~2500 lines)
**Purpose:** Comprehensive error handling for the entire system.

**Key Types:**
- **`StrataError`**: The main error enum with variants for all error conditions:
  - `NotFound { key }` - Entity not found
  - `WrongType { expected, actual }` - Type mismatch
  - `InvalidKey { key, reason }` - Invalid key
  - `InvalidPath { path, reason }` - Invalid JSON path
  - `HistoryTrimmed { requested, earliest_retained }` - Version unavailable
  - `ConstraintViolation { reason }` - Invariant violation
  - `Conflict { expected, actual }` - Version mismatch
  - `Overflow` - Numeric overflow
  - `RunNotFound`, `RunClosed`, `RunExists` - Run lifecycle errors
  - `InvalidInput { message }` - Generic input validation error
  - `StorageError { message }` - Storage backend error
  - `Internal { message }` - Bug or invariant violation

- **`ErrorCode`**: Enum mapping to canonical error codes (10 codes)
- Implements `std::error::Error` and `Display`

---

### `api_error.rs` (425 lines)
**Purpose:** API-level error types for wire encoding.

**Key Types:**
- **`WireError`**: JSON wire format for errors with `code`, `message`, and optional `details`
- **`ApiError`**: High-level API error enum with 14 variants matching all error conditions

**Wire Format:**
```json
{
  "code": "NotFound",
  "message": "Key not found: mykey",
  "details": {"key": "mykey"}
}
```

Each `ApiError` variant can convert to `WireError` for JSON serialization.

---

### `run_types.rs` (271 lines)
**Purpose:** Run lifecycle types for durability and replay.

**Key Types:**
- **`RunStatus`**: Enum with 4 states:
  - `Active` - Run in progress (begin_run called)
  - `Completed` - Run finished normally (end_run called)
  - `Orphaned` - Run was never ended (crash without end_run marker)
  - `NotFound` - Run doesn't exist

- **`RunMetadata`**: Struct containing:
  - `run_id`, `status`, `started_at`, `ended_at`
  - `event_count`, `begin_wal_offset`, `end_wal_offset`

- **`RunEventOffsets`**: Maps run to its event offsets in EventLog for O(run size) replay

---

### `limits.rs` (543 lines)
**Purpose:** Configurable size limits enforced by the engine.

**Default Limits:**
| Limit | Default Value |
|-------|---------------|
| `max_key_bytes` | 1,024 |
| `max_string_bytes` | 16 MB |
| `max_bytes_len` | 16 MB |
| `max_value_bytes_encoded` | 32 MB |
| `max_array_len` | 1,000,000 |
| `max_object_entries` | 1,000,000 |
| `max_nesting_depth` | 128 |
| `max_vector_dim` | 8,192 |

**Key Types:**
- **`Limits`**: Struct holding all limit values
- **`LimitError`**: Error enum for violations (`KeyTooLong`, `ValueTooLarge`, `NestingTooDeep`, `VectorDimExceeded`, `VectorDimMismatch`)

Provides `validate_key_length()`, `validate_value()`, `validate_vector()` methods.

---

### `search_types.rs` (608 lines)
**Purpose:** Search types for retrieval operations.

**Key Types:**
- **`SearchBudget`**: Limits on search execution
  - `max_wall_time_micros`: 100ms default
  - `max_candidates`: 10,000 default
  - `max_candidates_per_primitive`: 2,000 default

- **`SearchMode`**: Enum (`Keyword`, `Vector`, `Hybrid`)

- **`SearchRequest`**: Universal search request with:
  - `run_id`, `query`, `k` (top-k), `budget`, `mode`
  - Optional `primitive_filter`, `time_range`, `tags_any`

- **`SearchHit`**: Single result with `doc_ref`, `score`, `rank`, optional `snippet`

- **`SearchStats`**: Execution statistics (`elapsed_micros`, `candidates_considered`, `index_used`)

- **`SearchResponse`**: Results containing `hits`, `truncated` flag, `stats`

---

### `json.rs` (Large file)
**Purpose:** JSON types for the JSON document primitive.

**Document Limits:**
| Limit | Value |
|-------|-------|
| `MAX_DOCUMENT_SIZE` | 16 MB |
| `MAX_NESTING_DEPTH` | 100 levels |
| `MAX_PATH_LENGTH` | 256 segments |
| `MAX_ARRAY_SIZE` | 1M elements |

**Key Types:**
- **`JsonValue`**: Newtype wrapper around `serde_json::Value` with validation methods
- **`JsonPath`**: Path into JSON document (e.g., `user.name` or `items[0]`)
- **`PathSegment`**: Individual path component (`Key(String)` or `Index(usize)`)
- **`JsonPatch`**: Patch operation (`Set` or `Delete`)
- **`LimitError`**: Document limit violations

Implements `Deref` to `serde_json::Value` for direct method access.

---

## Contract Module (`contract/`)

The contract module defines types that express the "Seven Invariants" all primitives must follow.

### `contract/mod.rs` (45 lines)
**Purpose:** Module entry point with re-exports.

Re-exports: `EntityRef`, `DocRef`, `Versioned`, `VersionedValue`, `Version`, `Timestamp`, `PrimitiveType`, `RunName`

---

### `contract/entity_ref.rs` (489 lines)
**Purpose:** Universal entity reference type (Invariant 1: Everything is Addressable).

**`EntityRef` Enum (6 variants):**
1. `Kv { run_id, key }` - KV entry reference
2. `Event { run_id, sequence }` - Event log entry
3. `State { run_id, name }` - State cell
4. `Run { run_id }` - Run metadata
5. `Json { run_id, doc_id }` - JSON document
6. `Vector { run_id, collection, key }` - Vector entry

Provides constructors (`EntityRef::kv()`, etc.), accessors (`run_id()`, `primitive_type()`), type checks (`is_kv()`, etc.), and extraction methods (`kv_key()`, etc.).

Display format: `kv://run_id/key`, `event://run_id/sequence`, etc.

**`DocRef`**: Type alias for `EntityRef` (backwards compatibility).

---

### `contract/versioned.rs` (419 lines)
**Purpose:** Generic versioned wrapper (Invariant 2: Everything is Versioned).

**`Versioned<T>` Struct:**
- `value: T` - The actual data
- `version: Version` - Version identifier
- `timestamp: Timestamp` - Creation time

Methods: `new()`, `with_timestamp()`, `map()`, `value()`, `into_value()`, `is_older_than()`, `age()`, `into_parts()`

**`VersionedValue`**: Type alias for `Versioned<Value>` with additional convenience methods for type checking.

---

### `contract/version.rs` (553 lines)
**Purpose:** Version identifier types.

**`Version` Enum (3 variants):**
1. `Txn(u64)` - Transaction-based versioning (KV, Json, Vector, Run)
2. `Sequence(u64)` - Position-based versioning (EventLog)
3. `Counter(u64)` - Per-entity counter (StateCell)

Methods: `txn()`, `seq()`, `counter()`, `as_u64()`, `increment()`, `saturating_increment()`, `is_zero()`

Implements `PartialOrd` and `Ord` - versions are comparable within the same variant type.

Display format: `txn:42`, `seq:100`, `cnt:5`

---

### `contract/timestamp.rs` (341 lines)
**Purpose:** Microsecond-precision timestamp type.

**`Timestamp` Struct:**
- Wraps `u64` representing microseconds since Unix epoch
- Constants: `EPOCH` (0), `MAX` (u64::MAX)

Constructors: `now()`, `from_micros()`, `from_millis()`, `from_secs()`

Accessors: `as_micros()`, `as_millis()`, `as_secs()`

Duration operations: `duration_since()`, `saturating_add()`, `saturating_sub()`, `is_before()`, `is_after()`

Display format: `seconds.microseconds` (e.g., `1234.567890`)

---

### `contract/primitive_type.rs` (283 lines)
**Purpose:** Primitive type enumeration (Invariant 6: Everything is Introspectable).

**`PrimitiveType` Enum (6 variants):**
| Variant | Display Name | Short ID | Versioning | CRUD Support |
|---------|--------------|----------|------------|--------------|
| `Kv` | KVStore | kv | TxnId | Full |
| `Event` | EventLog | event | Sequence | Append-only |
| `State` | StateCell | state | Counter | Full |
| `Run` | RunIndex | run | TxnId | Full |
| `Json` | JsonStore | json | TxnId | Full |
| `Vector` | VectorStore | vector | TxnId | Full |

Methods: `all()`, `name()`, `id()`, `from_id()`, `supports_crud()`, `is_append_only()`

---

### `contract/run_name.rs` (400 lines)
**Purpose:** User-facing semantic identifier for runs.

**`RunName` Struct:**
- Wraps validated `String`
- Max length: 256 characters
- Allowed characters: `[a-zA-Z0-9_.-]`
- Cannot start with `-` or `.`

**`RunNameError` Enum:**
- `Empty` - Name is empty
- `TooLong { length, max }` - Exceeds maximum
- `InvalidChar { char, position }` - Invalid character
- `InvalidStart { char }` - Invalid starting character

Methods: `new()`, `new_unchecked()`, `validate()`, `as_str()`, `into_inner()`, `starts_with()`, `ends_with()`, `contains()`

---

## Primitives Module (`primitives/`)

Defines canonical data structures for all primitives, shared between engine and primitives crates.

### `primitives/mod.rs` (25 lines)
**Purpose:** Module entry point with re-exports.

Re-exports all types from `event`, `state`, and `vector` submodules.

---

### `primitives/event.rs` (66 lines)
**Purpose:** Event types for the EventLog primitive.

**`Event` Struct:**
- `sequence: u64` - Auto-assigned, monotonic per run
- `event_type: String` - User-defined category
- `payload: Value` - Arbitrary data
- `timestamp: i64` - Milliseconds since epoch
- `prev_hash: [u8; 32]` - Hash of previous event (chaining)
- `hash: [u8; 32]` - Hash of this event

**`ChainVerification` Struct:**
- `is_valid: bool` - Chain validity
- `length: u64` - Chain length
- `first_invalid: Option<u64>` - First invalid sequence
- `error: Option<String>` - Error description

---

### `primitives/state.rs` (51 lines)
**Purpose:** State types for the StateCell primitive.

**`State` Struct:**
- `value: Value` - Current value
- `version: u64` - Monotonically increasing counter
- `updated_at: i64` - Last update timestamp (milliseconds)

Methods: `new()`, `with_version()`, `now()`

---

### `primitives/vector.rs` (506 lines)
**Purpose:** Vector types for the VectorStore primitive.

**Key Types:**

**`DistanceMetric` Enum:**
- `Cosine` - Cosine similarity (default), range [-1, 1]
- `Euclidean` - Euclidean similarity, range (0, 1]
- `DotProduct` - Raw dot product, unbounded

**`StorageDtype` Enum:**
- `F32` - 32-bit floating point (only supported type)

**`VectorConfig` Struct:**
- `dimension: usize` - Embedding dimension (immutable)
- `metric: DistanceMetric` - Distance metric (immutable)
- `storage_dtype: StorageDtype` - Storage type

Presets: `for_openai_ada()` (1536d), `for_openai_large()` (3072d), `for_minilm()` (384d), `for_mpnet()` (768d)

**`VectorId`**: Internal identifier wrapping `u64` (never reused).

**`VectorEntry` Struct:**
- `key: String` - User-provided key
- `embedding: Vec<f32>` - Vector data
- `metadata: Option<serde_json::Value>` - Optional metadata
- `vector_id: VectorId` - Internal ID
- `version: u64` - For optimistic concurrency
- `source_ref: Option<EntityRef>` - Link to source document

**`VectorMatch` Struct:**
- `key: String` - User key
- `score: f32` - Similarity score (higher = more similar)
- `metadata: Option<serde_json::Value>` - Optional metadata

**`CollectionInfo`**: Metadata about a collection (name, config, count, created_at).

**`CollectionId`**: Unique identifier combining `run_id` and `name`.

**`JsonScalar` Enum**: Scalar values for filtering (`Null`, `Bool`, `Number`, `String`).

**`MetadataFilter` Struct**: Equality-based metadata filtering with AND semantics.

---

## Summary

The `strata-core` crate provides 22 Rust source files organized into:

| Category | Files | Purpose |
|----------|-------|---------|
| Root | 1 | Module declarations and re-exports |
| Core Types | 10 | Identity types, values, traits, errors, limits, search |
| Contract | 7 | API stability types (versioning, addressing, timestamps) |
| Primitives | 4 | Data structures for Event, State, and Vector primitives |

All types are designed to be:
- Zero-dependency within Strata (no imports from other Strata crates)
- Serializable (via serde)
- Immutable where appropriate
- Well-tested (each file contains comprehensive unit tests)
