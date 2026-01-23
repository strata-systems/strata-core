# M11 Implementation Plan: Public API & SDK Contract

## Overview

This document provides the high-level implementation plan for M11 (Public API & SDK Contract).

**M11 is split into two parts:**

| Part | Focus | Epics | Stories |
|------|-------|-------|---------|
| **M11a** | Core Contract & API | 80, 81, 82, 83, 85 | ~36 |
| **M11b** | Consumer Surfaces | 84, 86, 87 | ~18 |

**Total Scope**: 8 Epics, ~54 Stories (split across M11a and M11b)

**References**:
- [M11 Architecture Specification](../../architecture/M11_ARCHITECTURE.md) - Authoritative spec
- [M11 Contract Specification](./M11_CONTRACT.md) - Full contract details

**Critical Framing**:
> M11 is a **contract milestone**, not a feature milestone. It freezes the public API surface so all downstream consumers (wire protocol, CLI, SDKs, server) have a stable foundation.
>
> After M11, breaking changes require a major version bump. The contract defines what users observe. Internal implementation details remain flexible.
>
> **M11 does NOT add new capabilities.** It stabilizes, documents, and validates the existing API surface. The engine's seven primitives already exist. M11 ensures they are exposed consistently across all surfaces.

### M11a: Core Contract & API

M11a establishes the **foundation contract** that cannot change:
- Value Model (8 types, equality, limits)
- Version Model (tagged union: Txn/Sequence/Counter)
- Versioned<T> (frozen structure with microsecond timestamps)
- Wire Encoding (JSON, $bytes, $f64, $absent)
- Error Model (codes, shapes, reasons)
- Facade API (Redis-like surface)
- Substrate API (power-user surface)
- Facade-Substrate Desugaring Verification

**M11a Exit Criteria**: Core contract frozen, Facade↔Substrate parity verified, all core validation tests passing.

### M11b: Consumer Surfaces

M11b builds **user-facing surfaces** on top of the frozen M11a contract:
- CLI (all facade operations)
- SDK Foundation (Rust SDK, Python/JS mappings)
- Full Conformance Suite (CLI tests, SDK conformance, regression tests)

**M11b Exit Criteria**: CLI complete, Rust SDK complete, full validation suite passing.

---

**Epic Details**:

**M11a Epics:**
- [Epic 80: Value Model & Wire Encoding](./EPIC_80_VALUE_MODEL.md)
- [Epic 81: Error Model Standardization](./EPIC_81_ERROR_MODEL.md)
- [Epic 82: Facade API Implementation](./EPIC_82_FACADE_API.md)
- [Epic 83: Substrate API Implementation](./EPIC_83_SUBSTRATE_API.md)
- [Epic 85: Facade-Substrate Desugaring](./EPIC_85_DESUGARING.md)

**M11b Epics:**
- [Epic 84: CLI Implementation](./EPIC_84_CLI.md)
- [Epic 86: SDK Foundation](./EPIC_86_SDK_FOUNDATION.md)
- [Epic 87: Contract Conformance Testing](./EPIC_87_CONFORMANCE.md)

---

## Architectural Integration Rules (NON-NEGOTIABLE)

These rules ensure M11 produces a stable, consistent contract.

### Rule 1: Facade Desugars to Substrate

Every facade operation MUST map to a deterministic sequence of substrate operations. No hidden semantics.

**FORBIDDEN**: Facade operations with behavior that cannot be expressed in substrate terms.

### Rule 2: No Hidden Errors

The facade MUST surface all substrate errors unchanged. No swallowing, transforming, or hiding errors.

**FORBIDDEN**: Error transformation, silent failures, best-effort fallbacks.

### Rule 3: No Type Coercion

Values MUST NOT be implicitly converted between types. `Int(1)` does not equal `Float(1.0)`.

**FORBIDDEN**: Implicit widening, lossy conversions, type promotion.

### Rule 4: Explicit Run Scoping

Substrate operations MUST require explicit `run_id`. Facade operations MUST target the default run.

**FORBIDDEN**: Substrate operations with implicit run, facade operations with explicit run parameters.

### Rule 5: Wire Encoding Preserves Types

Wire encoding MUST preserve the distinction between Value types. Round-trip must be lossless.

**FORBIDDEN**: Encoding that loses type information, ambiguous representations.

### Rule 6: Errors Are Explicit

All invalid inputs MUST produce explicit errors. No silent failures or best-effort handling.

**FORBIDDEN**: Silent truncation, silent coercion, partial results without indication.

### Rule 7: Contract Stability

Frozen elements MUST NOT change without major version bump. This includes operation names, parameter shapes, return shapes, error codes, wire encodings.

**FORBIDDEN**: Changing frozen elements, removing operations, altering semantics.

### Rule 8: Default Run Is Literal "default"

The default run has the canonical name `"default"` (literal string, not UUID). It always exists.

**FORBIDDEN**: UUID for default run, lazy creation visible to users, closeable default run.

---

## Core Invariants

### Determinism Invariants (DET)

| # | Invariant | Test Strategy |
|---|-----------|---------------|
| DET-1 | Same sequence of substrate operations produces same state | Replay verification tests |
| DET-2 | Timestamps are metadata, not inputs to state transitions | Timestamp independence tests |
| DET-3 | WAL replay produces identical state | WAL replay determinism tests |
| DET-4 | Compacted state is indistinguishable from uncompacted (except trimmed history) | Compaction invisibility tests |
| DET-5 | Wire encoding does not affect logical semantics (JSON and MessagePack produce identical results) | Cross-encoding equivalence tests |

### Facade Invariants (FAC)

| # | Invariant | Test Strategy |
|---|-----------|---------------|
| FAC-1 | Every facade operation maps to deterministic substrate operations | Desugaring unit tests |
| FAC-2 | Facade adds no semantic behavior beyond defaults | Parity tests facade vs substrate |
| FAC-3 | Facade never swallows substrate errors | Error propagation tests |
| FAC-4 | Facade does not reorder operations | Ordering verification tests |
| FAC-5 | All behavior traces to explicit substrate operation | Audit all code paths |

### Value Model Invariants (VAL)

| # | Invariant | Test Strategy |
|---|-----------|---------------|
| VAL-1 | Eight types only: Null, Bool, Int, Float, String, Bytes, Array, Object | Type exhaustiveness tests |
| VAL-2 | No implicit type coercions | Cross-type comparison tests |
| VAL-3 | `Int(1)` != `Float(1.0)` | Explicit inequality tests |
| VAL-4 | `Bytes` are not `String` | Type distinction tests |
| VAL-5 | Float uses IEEE-754 equality: `NaN != NaN`, `-0.0 == 0.0` | Float edge case tests |

### Wire Encoding Invariants (WIRE)

| # | Invariant | Test Strategy |
|---|-----------|---------------|
| WIRE-1 | JSON encoding is mandatory | Encoding availability tests |
| WIRE-2 | Bytes encode as `{"$bytes": "<base64>"}` | Bytes round-trip tests |
| WIRE-3 | Non-finite floats encode as `{"$f64": "NaN\|+Inf\|-Inf\|-0.0"}` | Float special value tests |
| WIRE-4 | Absent values encode as `{"$absent": true}` | CAS absent value tests |
| WIRE-5 | Round-trip preserves exact type and value | Full round-trip suite |

### Error Invariants (ERR)

| # | Invariant | Test Strategy |
|---|-----------|---------------|
| ERR-1 | All errors surface through structured error model | Error shape validation |
| ERR-2 | All errors include code, message, details | Error completeness tests |
| ERR-3 | No operation has undefined behavior | Exhaustive edge case tests |
| ERR-4 | `Conflict` = temporal; `ConstraintViolation` = structural | Error categorization tests |

### Versioned<T> Invariants (VER)

| # | Invariant | Test Strategy |
|---|-----------|---------------|
| VER-1 | Version is tagged union (txn/sequence/counter) | Version type preservation |
| VER-2 | Timestamp is microseconds since Unix epoch | Timestamp format tests |
| VER-3 | Version types are not comparable across tags | Cross-type comparison rejection |
| VER-4 | Versioned<T> shape is frozen | Shape validation tests |

---

## Epic Overview

### M11a Epics (Core Contract & API)

| Epic | Name | Stories | Dependencies | Status |
|------|------|---------|--------------|--------|
| 80 | Value Model & Wire Encoding | 10 | M10 complete | Pending |
| 81 | Error Model Standardization | 6 | Epic 80 | Pending |
| 82 | Facade API Implementation | 9 | Epic 80, 81 | Pending |
| 83 | Substrate API Implementation | 10 | Epic 80, 81 | Pending |
| 85 | Facade-Substrate Desugaring | 4 | Epic 82, 83 | Pending |

**M11a Total**: ~39 stories

### M11b Epics (Consumer Surfaces)

| Epic | Name | Stories | Dependencies | Status |
|------|------|---------|--------------|--------|
| 84 | CLI Implementation | 8 | M11a complete | Pending |
| 86 | SDK Foundation | 5 | M11a complete | Pending |
| 87 | Contract Conformance Testing | 5 | Epics 84, 86 | Pending |

**M11b Total**: ~18 stories

---

## Epic 80: Value Model & Wire Encoding

**Goal**: Freeze the canonical value model with all eight types, version types, Versioned<T> structure, and wire encoding

| Story | Description | Priority |
|-------|-------------|----------|
| #550 | Value Enum Implementation (8 types) | FOUNDATION |
| #551 | Value Equality Semantics (structural, IEEE-754 floats) | FOUNDATION |
| #552 | Value Size Limits and Validation | CRITICAL |
| #553 | Version Tagged Union (Txn/Sequence/Counter) | CRITICAL |
| #554 | Versioned<T> Structure Finalization | CRITICAL |
| #555 | RunId Format (UUID + "default" literal) | CRITICAL |
| #556 | JSON Wire Encoding (basic types) | CRITICAL |
| #557 | $bytes Wrapper (base64 encoding) | CRITICAL |
| #558 | $f64 Wrapper (NaN, ±Inf, -0.0) | CRITICAL |
| #559 | $absent Wrapper (for CAS) | HIGH |

**Acceptance Criteria**:
- [ ] `Value` enum with exactly 8 variants: Null, Bool, Int(i64), Float(f64), String, Bytes, Array, Object
- [ ] Float preserves all IEEE-754 values including NaN (all payload variants), +Inf, -Inf, -0.0
- [ ] `-0.0` preserved in storage and wire encoding (even though `-0.0 == 0.0` for equality)
- [ ] Structural equality implemented: `NaN != NaN`, `-0.0 == 0.0`
- [ ] **No total ordering defined**: Future ordering APIs must be explicit and opt-in
- [ ] No implicit type coercions in equality
- [ ] Size limits configurable and enforced:
  - `max_key_bytes`: 1024 (default)
  - `max_string_bytes`: 16 MiB
  - `max_bytes_len`: 16 MiB
  - `max_value_bytes_encoded`: 32 MiB
  - `max_array_len`: 1,000,000
  - `max_object_entries`: 1,000,000
  - `max_nesting_depth`: 128
  - `max_vector_dim`: 8192
- [ ] Violations return `ConstraintViolation` with reason codes
- [ ] **Version tagged union implemented**:
  - `Version::Txn(u64)` - For KV, JSON, Vector, Run
  - `Version::Sequence(u64)` - For Events (append-only)
  - `Version::Counter(u64)` - For StateCell (per-entity CAS)
- [ ] **Versioned<T> structure frozen**:
  - `value: T`
  - `version: Version`
  - `timestamp: u64` (microseconds since Unix epoch)
  - Timestamps are **monotonic within a run**
- [ ] **RunId format**:
  - Normal runs: UUID lowercase hyphenated (e.g., `f47ac10b-58cc-4372-a567-0e02b2c3d479`)
  - Default run: literal string `"default"`
- [ ] JSON encoding for all 8 types
- [ ] `{"$bytes": "<base64>"}` wrapper for Bytes
- [ ] `{"$f64": "NaN|+Inf|-Inf|-0.0"}` wrapper for non-finite floats
- [ ] `{"$absent": true}` wrapper for CAS missing values
- [ ] Round-trip encoding preserves exact type and value
- [ ] **Return shape encoding per operation** (see Wire Encoding section)

---

## Epic 81: Error Model Standardization

**Goal**: Freeze all error codes and structured payloads

| Story | Description | Priority |
|-------|-------------|----------|
| #560 | StrataError Enum with All Codes | FOUNDATION |
| #561 | Error Wire Shape (code, message, details) | CRITICAL |
| #562 | ConstraintViolation Reason Codes | CRITICAL |
| #563 | Error-Producing Conditions Coverage | HIGH |
| #564 | Overflow Error for Numeric Operations | HIGH |
| #565 | Error Documentation | HIGH |

**Acceptance Criteria**:
- [ ] All error codes implemented:
  - `NotFound` - Entity or key not found
  - `WrongType` - Wrong primitive or value type
  - `InvalidKey` - Key syntax invalid
  - `InvalidPath` - JSON path invalid
  - `HistoryTrimmed` - Requested version no longer retained
  - `ConstraintViolation` - Schema/shape/invariant violation
  - `Conflict` - CAS failure, transaction conflict, version mismatch
  - `SerializationError` - Value encode/decode failure
  - `StorageError` - Disk, WAL, or IO failure
  - `InternalError` - Bug or invariant violation
  - `Overflow` - Numeric overflow/underflow (for incr)
- [ ] Wire error shape: `{"ok": false, "error": {"code": "...", "message": "...", "details": {...}}}`
- [ ] ConstraintViolation reason codes:
  - `value_too_large`
  - `nesting_too_deep`
  - `key_too_long`
  - `vector_dim_exceeded`
  - `vector_dim_mismatch`
  - `root_not_object`
  - `reserved_prefix`
  - `run_closed`
- [ ] HistoryTrimmed includes `requested` and `earliest_retained` versions
- [ ] **Complete error-producing conditions**:

| Condition | Error Code |
|-----------|------------|
| Invalid UTF-8 in key | `InvalidKey` |
| NUL byte in key | `InvalidKey` |
| Key exceeds max length | `InvalidKey` |
| Key uses reserved prefix (`_strata/`) | `InvalidKey` |
| Empty key | `InvalidKey` |
| Value exceeds size limits | `ConstraintViolation` |
| Nesting exceeds max depth | `ConstraintViolation` |
| JSON path syntax error | `InvalidPath` |
| JSON path targets non-existent intermediate | `InvalidPath` |
| JSON root set to non-Object | `ConstraintViolation` |
| Vector dimension mismatch | `ConstraintViolation` |
| Vector dimension exceeds max | `ConstraintViolation` |
| Comparing incompatible version types | `WrongType` |
| Operating on closed run | `ConstraintViolation` |
| Using stale/committed transaction handle | `Conflict` |
| `use_run` on non-existent run | `NotFound` |
| CAS on wrong primitive type | `WrongType` |
| `incr` on non-Int value | `WrongType` |
| `incr` causes overflow/underflow | `Overflow` |

---

## Epic 82: Facade API Implementation

**Goal**: Implement the Redis-like facade API targeting default run

| Story | Description | Priority |
|-------|-------------|----------|
| #566 | KV Operations (set, get, getv, mget, mset, delete, exists, exists_many, incr) | CRITICAL |
| #567 | JSON Operations (json_set, json_get, json_getv, json_del, json_merge) | CRITICAL |
| #568 | Event Operations (xadd, xrange, xlen) | CRITICAL |
| #569 | Vector Operations (vset, vget, vdel) | CRITICAL |
| #570 | State Operations (cas_set, cas_get) | CRITICAL |
| #571 | History Operations (history, get_at, latest_version) | HIGH |
| #572 | Run Operations (runs, use_run) | HIGH |
| #573 | Capability Discovery (capabilities) | HIGH |
| #574 | Facade Auto-Commit Semantics | CRITICAL |

**Acceptance Criteria**:
- [ ] All KV operations implemented with correct signatures:
  - `set(key, value) -> ()`
  - `get(key) -> Option<Value>`
  - `getv(key) -> Option<Versioned<Value>>`
  - `mget(keys) -> Vec<Option<Value>>`
  - `mset(entries) -> ()` (atomic, all-or-nothing on validation failure)
  - `delete(keys) -> u64` (count of keys that **existed**)
  - `exists(key) -> bool`
  - `exists_many(keys) -> u64`
  - `incr(key, delta=1) -> i64` (atomic engine operation, missing key = 0)
- [ ] All JSON operations with correct path syntax (JSONPath-style):
  - `$` = root (entire document)
  - `$.a.b` = object field access
  - `$.items[0]` = array index
  - `$.items[-]` = array append (json_set only)
  - Negative indices `[-1]` NOT supported → `InvalidPath`
  - Deleting root (`$`) is forbidden → use `delete` to remove key
- [ ] **JSON merge semantics (RFC 7396)**:
  - `null` deletes a field
  - Objects merge recursively
  - Arrays replace (not merge)
  - Scalars replace
- [ ] `json_getv` returns **document-level version** (not subpath version)
- [ ] **Event operations**:
  - `xadd(stream, payload: Object) -> Version` (sequence type)
  - `xrange(stream, start?, end?, limit?) -> Vec<Versioned<Value>>`
  - `xlen(stream) -> u64`
  - Empty object `{}` is allowed as payload
  - Bytes are allowed in payloads (via `$bytes` wrapper)
- [ ] **Vector operations**:
  - `vset(key, vector, metadata) -> ()`
  - `vget(key) -> Option<Versioned<VectorEntry>>` where `VectorEntry = {vector: Vec<f32>, metadata: Value}`
  - `vdel(key) -> bool`
  - Dimension rules: 1 to max_vector_dim (8192), mismatch returns `ConstraintViolation`
- [ ] **State/CAS operations**:
  - `cas_set(key, expected, new) -> bool`
  - `cas_get(key) -> Option<Value>`
  - `expected = None` means "only set if key is missing" (create-if-not-exists)
  - `expected = Some(Value::Null)` means "only set if current value is null"
  - Type matters: `Int(1)` != `Float(1.0)` in CAS comparison
- [ ] `use_run` returns `NotFound` if run doesn't exist (no lazy creation)
- [ ] **Capabilities object structure**:
  ```json
  {
    "version": "1.0.0",
    "operations": ["kv.set", "kv.get", ...],
    "limits": {
      "max_key_bytes": 1024,
      "max_string_bytes": 16777216,
      ...
    },
    "encodings": ["json"],
    "features": ["history", "retention", "cas"]
  }
  ```
- [ ] All operations target default run implicitly
- [ ] All operations auto-commit (each call is atomic)

---

## Epic 83: Substrate API Implementation

**Goal**: Implement the explicit substrate API with run/version/txn access

| Story | Description | Priority |
|-------|-------------|----------|
| #575 | KVStore Substrate Operations | CRITICAL |
| #576 | JsonStore Substrate Operations | CRITICAL |
| #577 | EventLog Substrate Operations | CRITICAL |
| #578 | StateCell Substrate Operations | CRITICAL |
| #579 | VectorStore Substrate Operations | CRITICAL |
| #580 | TraceStore Substrate Operations | HIGH |
| #581 | RunIndex Substrate Operations | CRITICAL |
| #582 | Transaction Control (begin, commit, rollback) | CRITICAL |
| #583 | Retention Operations (retention_get, retention_set) | HIGH |
| #584 | Core Types (RunId, RunInfo, RunState, RetentionPolicy) | FOUNDATION |

**Acceptance Criteria**:
- [ ] All substrate operations require explicit `run_id` parameter
- [ ] All read operations return `Versioned<T>`
- [ ] All write operations return `Version`
- [ ] **KVStore operations**:
  - `kv_put(run, key, value) -> Version`
  - `kv_get(run, key) -> Option<Versioned<Value>>`
  - `kv_get_at(run, key, version) -> Versioned<Value> | HistoryTrimmed`
  - `kv_delete(run, key) -> bool`
  - `kv_exists(run, key) -> bool`
  - `kv_history(run, key, limit?, before?) -> Vec<Versioned<Value>>`
  - `kv_incr(run, key, delta) -> i64` (atomic engine operation)
  - `kv_cas_version(run, key, expected_version, new_value) -> bool`
  - `kv_cas_value(run, key, expected_value, new_value) -> bool`
- [ ] **JsonStore operations**:
  - `json_set(run, key, path, value) -> Version`
  - `json_get(run, key, path) -> Option<Versioned<Value>>`
  - `json_delete(run, key, path) -> u64`
  - `json_merge(run, key, path, value) -> Version`
  - `json_history(run, key, limit?, before?) -> Vec<Versioned<Value>>`
- [ ] **EventLog operations**:
  - `event_append(run, stream, payload) -> Version`
  - `event_range(run, stream, start?, end?, limit?) -> Vec<Versioned<Value>>`
- [ ] **StateCell operations**:
  - `state_get(run, key) -> Option<Versioned<Value>>`
  - `state_set(run, key, value) -> Version`
  - `state_cas(run, key, expected, new) -> bool`
  - `state_history(run, key, limit?, before?) -> Vec<Versioned<Value>>`
- [ ] **VectorStore operations**:
  - `vector_set(run, key, vector, metadata) -> Version`
  - `vector_get(run, key) -> Option<Versioned<{vector, metadata}>>`
  - `vector_delete(run, key) -> bool`
  - `vector_history(run, key, limit?, before?) -> Vec<Versioned<Value>>`
- [ ] **TraceStore operations** (substrate-only):
  - `trace_record(run, trace_type: String, payload: Value) -> Version`
  - `trace_get(run, id) -> Option<Versioned<Value>>`
  - `trace_range(run, start?, end?, limit?) -> Vec<Versioned<Value>>`
- [ ] **Transaction control**:
  - `begin(run_id) -> Txn`
  - `commit(txn) -> ()`
  - `rollback(txn) -> ()`
  - Transactions are scoped to a single run
  - Snapshot isolation (OCC validation at commit)
  - Using stale/committed transaction handle → `Conflict`
- [ ] **Run lifecycle**:
  - `run_create(metadata) -> RunId` (UUID format)
  - `run_get(run_id) -> Option<RunInfo>`
  - `run_list() -> Vec<RunInfo>`
  - `run_close(run_id) -> ()` (marks run as closed)
  - Default run (`"default"`) cannot be closed
  - Default run created lazily on first write or on DB open
  - No run deletion in M11 (deferred to GC milestone)
- [ ] **Retention operations**:
  - `retention_get(run_id) -> Option<Versioned<RetentionPolicy>>`
  - `retention_set(run_id, policy) -> Version`
  - Default policy is `KeepAll`
  - Per-key retention NOT supported in M11
- [ ] **Core types**:
  ```rust
  struct RunId(String)  // UUID format or "default"

  struct RunInfo {
      run_id: RunId,
      created_at: u64,  // microseconds
      metadata: Value,
      state: RunState
  }

  enum RunState { Active, Closed }

  enum RetentionPolicy {
      KeepAll,
      KeepLast(u64),
      KeepFor(Duration),
      Composite(Vec<RetentionPolicy>)
  }
  ```

---

## Epic 84: CLI Implementation

**Goal**: Implement Redis-like CLI with frozen parsing rules

| Story | Description | Priority |
|-------|-------------|----------|
| #585 | CLI Argument Parser | FOUNDATION |
| #586 | KV Commands (set, get, mget, mset, delete, exists, incr) | CRITICAL |
| #587 | JSON Commands (json.set, json.get, json.del, json.merge) | CRITICAL |
| #588 | Event Commands (xadd, xrange, xlen) | HIGH |
| #589 | Vector Commands (vset, vget, vdel) | HIGH |
| #590 | State Commands (cas.set, cas.get) | HIGH |
| #591 | History and Run Commands | HIGH |
| #592 | Output Formatting and Exit Codes | CRITICAL |

**Acceptance Criteria**:
- [ ] CLI command interface: `strata <command> [args...]`
- [ ] **Argument parsing rules (FROZEN)**:
  - `123` → Int
  - `-456` → Int
  - `1.23` → Float
  - `-1.23` → Float
  - `"hello"` → String (quotes stripped)
  - `hello` → String (bare word)
  - `true`/`false` → Bool
  - `null` → Null
  - `{...}` → Object (must be valid JSON)
  - `[...]` → Array (must be valid JSON)
  - `b64:SGVsbG8=` → Bytes (base64 decoded)
- [ ] **Output conventions (FROZEN)**:
  - Missing value: `(nil)`
  - Integer/count: `(integer) N`
  - Boolean: `(integer) 0` or `(integer) 1`
  - String: `"text"`
  - Null value: `null`
  - Object/Array: JSON formatted
  - Bytes: `{"$bytes": "<base64>"}`
  - Error: JSON on stderr, non-zero exit code
- [ ] Run scoping: `--run=<run_id>` option (default is `"default"`)
- [ ] All facade commands working:
  ```bash
  # KV
  strata set x 123
  strata get x                    # prints: 123
  strata get missing              # prints: (nil)
  strata mget a b c               # prints: [123, (nil), "hello"]
  strata mset a 1 b 2 c 3         # atomic multi-set
  strata delete x y               # prints: (integer) 2
  strata exists x                 # prints: (integer) 1
  strata incr counter             # prints: (integer) 1

  # JSON
  strata json.set doc $.name "Alice"
  strata json.get doc $.name      # prints: "Alice"
  strata json.del doc $.temp      # prints: (integer) 1
  strata json.merge doc $ '{"age": 30}'

  # Events
  strata xadd stream '{"type":"login"}'  # prints version
  strata xrange stream
  strata xlen stream              # prints: (integer) N

  # Vectors
  strata vset doc1 "[0.1, 0.2, 0.3]" '{"tag":"test"}'
  strata vget doc1
  strata vdel doc1               # prints: (integer) 1

  # State (CAS)
  strata cas.set mykey null 123  # prints: (integer) 1 (created)
  strata cas.get mykey           # prints: 123
  strata cas.set mykey 123 456   # prints: (integer) 1 (updated)
  strata cas.set mykey 999 0     # prints: (integer) 0 (mismatch)

  # History
  strata history mykey
  strata history mykey --limit 10

  # System
  strata runs
  strata capabilities
  ```
- [ ] CLI is facade-only (no substrate operations exposed)
- [ ] **Exit codes (FROZEN)**:
  - `0`: Success
  - `1`: General error (e.g., `NotFound`, `WrongType`, `InvalidKey`)
  - `2`: Usage error (invalid arguments, unknown command)
  - Error details written to stderr as JSON

---

## Epic 85: Facade-Substrate Desugaring

**Goal**: Verify and document that every facade operation desugars correctly to substrate

| Story | Description | Priority |
|-------|-------------|----------|
| #593 | KV Desugaring Implementation | CRITICAL |
| #594 | JSON/Event/Vector Desugaring Implementation | CRITICAL |
| #595 | State/History/Run Desugaring Implementation | HIGH |
| #596 | Desugaring Verification Tests | CRITICAL |

**Acceptance Criteria**:
- [ ] All desugaring verified with tests comparing facade vs substrate execution
- [ ] See "Facade→Substrate Desugaring Reference" section for complete mapping

---

## Epic 86: SDK Foundation

**Goal**: Define SDK mappings and implement Rust SDK

| Story | Description | Priority |
|-------|-------------|----------|
| #597 | SDK Value Mapping Specification | FOUNDATION |
| #598 | Rust SDK Implementation | CRITICAL |
| #599 | Python SDK Mapping Definition | HIGH |
| #600 | JavaScript SDK Mapping Definition | HIGH |
| #601 | SDK Conformance Test Harness | CRITICAL |

**Acceptance Criteria**:
- [ ] **Python mapping defined**:
  ```python
  # Value mapping
  Null    -> None
  Bool    -> bool
  Int     -> int
  Float   -> float
  String  -> str
  Bytes   -> bytes
  Array   -> list
  Object  -> dict[str, Any]

  # Versioned wrapper
  class Versioned(Generic[T]):
      value: T
      version: Version
      timestamp: int  # microseconds

  # Error class
  class StrataError(Exception):
      code: str       # e.g., "NotFound", "WrongType"
      message: str    # Human-readable message
      details: dict | None  # Structured details
  ```
- [ ] **JavaScript mapping defined**:
  ```javascript
  // Value mapping
  Null    -> null
  Bool    -> boolean
  Int     -> number | BigInt (outside safe integer range)
  Float   -> number
  String  -> string
  Bytes   -> Uint8Array
  Array   -> Array<any>
  Object  -> Record<string, any>
  ```
  **Note**: JavaScript cannot safely represent all i64 values. SDK must use `BigInt` for integers outside `Number.MIN_SAFE_INTEGER` to `Number.MAX_SAFE_INTEGER`.
- [ ] Rust SDK uses native `Value` enum
- [ ] **SDK requirements enforced**:
  - Preserve numeric widths (i64, f64)
  - Preserve Bytes vs String distinction
  - Preserve None/missing vs Null distinction
  - Preserve Versioned wrapper shape
  - Surface structured errors with code, message, details
  - Use same operation names as facade
- [ ] Conformance test harness validates SDK behavior

---

## Epic 87: Contract Conformance Testing

**Goal**: Comprehensive tests verifying all contract invariants

| Story | Description | Priority |
|-------|-------------|----------|
| #602 | Facade-Substrate Parity Tests | CRITICAL |
| #603 | Value Model & Wire Encoding Tests | CRITICAL |
| #604 | Error Model Coverage Tests | CRITICAL |
| #605 | CLI Conformance Tests | CRITICAL |
| #606 | SDK Conformance Tests | CRITICAL |

**Acceptance Criteria**:
- [ ] **Facade-Substrate parity**: every facade operation produces same result as desugared substrate
- [ ] **Value round-trip**: all 8 types survive encode/decode
- [ ] **Float edge cases**: NaN, +Inf, -Inf, -0.0 all preserved
- [ ] **Bytes vs String distinction preserved**
- [ ] **$absent distinguishes missing from null**
- [ ] **All error codes produce correct wire shape**
- [ ] **Determinism verification**:
  - Same substrate operations produce same state
  - Timestamp independence (different timestamps, same logical state)
  - WAL replay produces identical state
  - Compaction is invisible (except trimmed history)
- [ ] **CLI conformance**:
  - Argument parsing tests pass
  - Output formatting tests pass
  - Exit codes correct
- [ ] **SDK conformance**:
  - Value mapping tests pass
  - Error handling tests pass
- [ ] **Minimum 70 conformance tests** covering all invariants

---

## Files to Create/Modify

### New Files

| File | Description |
|------|-------------|
| **Value Model** | |
| `crates/core/src/value/mod.rs` | Value module entry |
| `crates/core/src/value/types.rs` | Value enum (8 types) |
| `crates/core/src/value/equality.rs` | Structural equality, IEEE-754 floats |
| `crates/core/src/value/limits.rs` | Size limits and validation |
| **Wire Encoding** | |
| `crates/wire/src/lib.rs` | Wire crate entry |
| `crates/wire/src/json/mod.rs` | JSON encoding module |
| `crates/wire/src/json/value.rs` | Value JSON encoding |
| `crates/wire/src/json/wrappers.rs` | $bytes, $f64, $absent wrappers |
| `crates/wire/src/json/envelope.rs` | Request/response envelopes |
| `crates/wire/src/json/version.rs` | Version encoding |
| **Error Model** | |
| `crates/core/src/error/codes.rs` | Error code enum |
| `crates/core/src/error/constraint.rs` | ConstraintViolation reasons |
| `crates/core/src/error/wire.rs` | Wire error shape |
| **Facade API** | |
| `crates/api/src/facade/mod.rs` | Facade module entry |
| `crates/api/src/facade/kv.rs` | KV facade operations |
| `crates/api/src/facade/json.rs` | JSON facade operations |
| `crates/api/src/facade/event.rs` | Event facade operations |
| `crates/api/src/facade/vector.rs` | Vector facade operations |
| `crates/api/src/facade/state.rs` | State (CAS) facade operations |
| `crates/api/src/facade/history.rs` | History facade operations |
| `crates/api/src/facade/run.rs` | Run facade operations |
| `crates/api/src/facade/capabilities.rs` | Capability discovery |
| **Substrate API** | |
| `crates/api/src/substrate/mod.rs` | Substrate module entry |
| `crates/api/src/substrate/kv.rs` | KVStore substrate |
| `crates/api/src/substrate/json.rs` | JsonStore substrate |
| `crates/api/src/substrate/event.rs` | EventLog substrate |
| `crates/api/src/substrate/state.rs` | StateCell substrate |
| `crates/api/src/substrate/vector.rs` | VectorStore substrate |
| `crates/api/src/substrate/trace.rs` | TraceStore substrate |
| `crates/api/src/substrate/run.rs` | RunIndex substrate |
| `crates/api/src/substrate/retention.rs` | Retention substrate |
| `crates/api/src/substrate/transaction.rs` | Transaction control |
| **CLI** | |
| `crates/cli/src/main.rs` | CLI entry point |
| `crates/cli/src/parser.rs` | Argument parser |
| `crates/cli/src/commands/mod.rs` | Command module |
| `crates/cli/src/commands/kv.rs` | KV commands |
| `crates/cli/src/commands/json.rs` | JSON commands |
| `crates/cli/src/commands/event.rs` | Event commands |
| `crates/cli/src/commands/vector.rs` | Vector commands |
| `crates/cli/src/commands/state.rs` | State/CAS commands |
| `crates/cli/src/commands/history.rs` | History commands |
| `crates/cli/src/commands/run.rs` | Run/capabilities commands |
| `crates/cli/src/output.rs` | Output formatting |
| **Tests** | |
| `crates/api/tests/desugaring_tests.rs` | Desugaring verification |
| `crates/api/tests/conformance_tests.rs` | Contract conformance |
| `crates/wire/tests/roundtrip_tests.rs` | Wire encoding round-trip |

### Modified Files

| File | Changes |
|------|---------|
| `crates/core/src/value.rs` | Finalize Value enum, add equality |
| `crates/core/src/version.rs` | Add tagged union Version type |
| `crates/core/src/error.rs` | Add all error codes |
| `crates/engine/src/lib.rs` | Wire facade/substrate layers |
| `Cargo.toml` | Add wire, cli, api crates |

---

## Dependency Order

```
            Epic 80 (Value Model & Wire Encoding)
                        ↓
            ┌───────────┼───────────┐
            ↓           ↓           ↓
        Epic 81     Epic 82     Epic 83
        (Error)     (Facade)    (Substrate)
            ↓           ↓           ↓
            └───────────┼───────────┘
                        ↓
            Epic 85 (Desugaring Verification)
                        ↓
    ════════════════════════════════════════
                M11a COMPLETE
    ════════════════════════════════════════
                        ↓
                ┌───────┴───────┐
                ↓               ↓
            Epic 84         Epic 86
            (CLI)           (SDK)
                ↓               ↓
                └───────┬───────┘
                        ↓
            Epic 87 (Conformance Testing)
                        ↓
    ════════════════════════════════════════
                M11b COMPLETE
    ════════════════════════════════════════
```

**M11a Recommended Implementation Order**:
1. Epic 80: Value Model & Wire Encoding (foundation for everything)
2. Epic 81: Error Model Standardization (needed by API layers)
3. Epic 83: Substrate API Implementation (canonical semantics)
4. Epic 82: Facade API Implementation (builds on substrate)
5. Epic 85: Facade-Substrate Desugaring (validates core contract)

**M11b Recommended Implementation Order** (after M11a complete):
6. Epic 84: CLI Implementation (uses facade + wire)
7. Epic 86: SDK Foundation (uses facade + wire + error)
8. Epic 87: Contract Conformance Testing (validates everything)

---

## Phased Implementation Strategy

> **Guiding Principle**: Stabilize the data model first. Wire encoding must work before APIs. APIs must work before CLI/SDK. Each phase produces a testable, validated increment. M11a must be fully validated before starting M11b.

### M11a Phases

#### Phase 1: Data Model Foundation

Stabilize value model and wire encoding:
- Value enum finalization
- Float edge case handling (NaN, Inf, -0.0)
- Size limits enforcement
- Key validation
- Version tagged union (Txn/Sequence/Counter)
- Versioned<T> structure
- JSON wire encoding with special wrappers

**Exit Criteria**: All 8 value types encode/decode correctly. Round-trip tests pass.

#### Phase 2: Error Model

Freeze all error codes and payloads:
- Error code enumeration
- Wire error shape
- ConstraintViolation reasons
- Details payload shapes
- Complete error-producing conditions

**Exit Criteria**: All error conditions produce correct structured errors.

#### Phase 3: API Layers

Implement facade and substrate APIs:
- Substrate API with all operations and explicit parameters
- Facade API with all operations
- Transaction control (begin/commit/rollback)
- Retention operations
- Auto-commit semantics

**Exit Criteria**: Both API layers complete.

#### Phase 4: Desugaring Verification (M11a Exit Gate)

Verify facade-substrate mapping:
- Each facade operation desugars correctly
- No hidden semantics
- Error propagation verified
- Parity tests pass

**Exit Criteria**: All M11a contract guarantees validated. Zero defects in core contract.

---

### M11b Phases

**Prerequisite**: M11a must be fully complete and validated before starting M11b.

#### Phase 5: CLI

Implement consumer surface:
- CLI with Redis-like ergonomics
- Argument parsing
- Output formatting
- Exit codes

**Exit Criteria**: CLI works for all facade operations.

#### Phase 6: SDK + Conformance (M11b Exit Gate)

Implement SDK and full validation:
- Rust SDK implementation
- Python/JavaScript mapping definitions
- SDK conformance harness
- Full conformance test suite

**Exit Criteria**: All M11b contract guarantees validated. No regressions in M11a.

---

### Phase Summary

| Phase | Milestone | Epics | Key Deliverable | Status |
|-------|-----------|-------|-----------------|--------|
| 1 | M11a | 80 | Data model + wire encoding | Pending |
| 2 | M11a | 81 | Error model | Pending |
| 3 | M11a | 82, 83 | API layers | Pending |
| 4 | M11a | 85 | Desugaring verification | Pending |
| 5 | M11b | 84 | CLI | Pending |
| 6 | M11b | 86, 87 | SDK + conformance | Pending |

---

## Testing Strategy

### Unit Tests

- Value type construction and properties
- Float edge cases (NaN, Inf, -0.0)
- Value equality semantics
- Size limit enforcement
- Key validation logic
- Wire encoding for each value type
- Special wrapper encoding ($bytes, $f64, $absent)
- Version tagged union preservation
- Error code mapping
- CLI argument parsing

### Integration Tests

- Facade operation → substrate desugaring
- Full request/response cycle
- Multi-operation transactions
- Run scoping with `use_run`
- History pagination
- CAS operations with $absent
- Transaction isolation (snapshot isolation)

### Contract Tests

- Every facade operation produces same result as desugared substrate
- All 8 value types round-trip through wire encoding
- Float edge cases preserve exact representation
- Bytes vs String distinction maintained
- $absent distinguishes missing from null
- All error codes produce correct wire shape
- Version tagged union preserved
- Return shape per operation matches spec

### Determinism Tests

- Same substrate operations produce same state
- WAL replay produces identical state
- Timestamp independence verified
- Compaction invisibility maintained

### SDK Parity Tests

- Same operations produce same results across SDKs
- Value mapping consistent
- Error handling consistent
- Versioned wrapper shape consistent

### CLI Tests

- Argument parsing for all input types
- Output formatting for all return types
- Error output on stderr with non-zero exit
- Run scoping with --run option
- Exit codes correct per error type

---

## Success Metrics

**Functional**: All ~54 stories passing, 100% acceptance criteria met

**Contract Stability**:
- All frozen elements documented
- No breaking changes in frozen elements
- Contract versioned and dated

**API Completeness**:
- All facade operations implemented
- All substrate operations implemented
- All escape hatches working (getv, use_run, db.substrate())
- Transaction control working (begin/commit/rollback)

**Wire Conformance**:
- JSON encoding mandatory and working
- All special wrappers implemented
- Round-trip tests pass for all types
- Version tagged union preserved

**Error Model**:
- All error codes implemented
- All ConstraintViolation reasons implemented
- Structured details for all relevant errors
- All error-producing conditions tested

**CLI**:
- All facade commands working
- Argument parsing correct
- Output formatting correct
- Exit codes correct

**SDK**:
- Rust SDK complete
- Python/JavaScript mappings defined
- Conformance harness passing

**Quality**: Test coverage > 90% for new code

---

## Risk Mitigation

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Breaking change introduced | Medium | High | Contract validation suite, careful review |
| Float edge cases mishandled | Medium | High | Comprehensive float tests, IEEE-754 compliance |
| Wire encoding ambiguity | Low | High | Explicit wrapper semantics, round-trip tests |
| Facade-Substrate divergence | Medium | Medium | Parity tests, mechanical desugaring |
| CLI parsing inconsistency | Low | Medium | Frozen parsing rules, extensive tests |
| SDK mapping errors | Medium | Medium | Conformance harness, type preservation tests |
| Version type confusion | Medium | High | Tagged union with explicit type field |
| Transaction isolation bugs | Medium | High | OCC validation tests, conflict detection |

---

## Not In Scope (Explicitly Deferred)

1. **Python SDK implementation** - M14 (mappings defined only)
2. **JavaScript SDK implementation** - Post-MVP (mappings defined only)
3. **MessagePack wire encoding** - Optional, not required
4. **Search/vector query DSL** - Post-MVP
5. **JSONPath advanced features** - Filters, wildcards, recursive descent
6. **TTL/EXPIRE semantics** - Post-MVP
7. **Consumer groups for events** - Post-MVP
8. **Diff semantics** - Post-MVP
9. **Run deletion** - Deferred to GC milestone
10. **Per-key retention** - Post-MVP
11. **Serializable isolation** - Snapshot isolation only
12. **Negative array indices** - `[-1]` returns `InvalidPath`

---

## Post-M11 Expectations

After M11 completion:
1. Public API contract is frozen and documented
2. All facade operations have consistent behavior
3. All substrate operations expose full power-user control
4. Wire encoding is stable (JSON with $bytes, $f64, $absent)
5. Version types are preserved (Txn/Sequence/Counter distinction)
6. CLI provides Redis-like ergonomics
7. Rust SDK is complete and conformant
8. Python/JavaScript mappings are defined for future implementation
9. Breaking changes require major version bump
10. Contract validation suite catches regressions
11. All downstream consumers (server, SDKs) have stable foundation

---

## Facade→Substrate Desugaring Reference

For quick reference, the complete desugaring table:

### KV Operations

| Facade | Substrate |
|--------|-----------|
| `set(key, value)` | `begin(); kv_put(default, key, value); commit()` |
| `get(key)` | `kv_get(default, key).map(\|v\| v.value)` |
| `getv(key)` | `kv_get(default, key)` |
| `mget(keys)` | `batch { kv_get(default, k) for k in keys }` |
| `mset(entries)` | `begin(); for (k,v): kv_put(default, k, v); commit()` |
| `delete(keys)` | `begin(); for k: kv_delete(default, k); commit()` — returns count existed |
| `exists(key)` | `kv_get(default, key).is_some()` |
| `exists_many(keys)` | `keys.filter(\|k\| kv_get(default, k).is_some()).count()` |
| `incr(key, delta)` | `kv_incr(default, key, delta)` — **atomic engine operation** |

### JSON Operations

| Facade | Substrate |
|--------|-----------|
| `json_set(key, path, value)` | `begin(); json_set(default, key, path, value); commit()` |
| `json_get(key, path)` | `json_get(default, key, path).map(\|v\| v.value)` |
| `json_getv(key, path)` | `json_get(default, key, path)` — **document-level version** |
| `json_del(key, path)` | `begin(); json_delete(default, key, path); commit()` |
| `json_merge(key, path, value)` | `begin(); json_merge(default, key, path, value); commit()` |

### Event Operations

| Facade | Substrate |
|--------|-----------|
| `xadd(stream, payload)` | `event_append(default, stream, payload)` |
| `xrange(stream, start, end, limit)` | `event_range(default, stream, start, end, limit)` |
| `xlen(stream)` | `event_range(default, stream, None, None, None).len()` |

### Vector Operations

| Facade | Substrate |
|--------|-----------|
| `vset(key, vector, metadata)` | `begin(); vector_set(default, key, vector, metadata); commit()` |
| `vget(key)` | `vector_get(default, key)` |
| `vdel(key)` | `begin(); vector_delete(default, key); commit()` |

### State/CAS Operations

| Facade | Substrate |
|--------|-----------|
| `cas_set(key, expected, new)` | `state_cas(default, key, expected, new)` |
| `cas_get(key)` | `state_get(default, key).map(\|v\| v.value)` |

### History Operations

| Facade | Substrate |
|--------|-----------|
| `history(key, limit, before)` | `kv_history(default, key, limit, before)` |
| `get_at(key, version)` | `kv_get_at(default, key, version)` |
| `latest_version(key)` | `kv_get(default, key).map(\|v\| v.version)` |

### Run Operations

| Facade | Substrate |
|--------|-----------|
| `runs()` | `run_list()` |
| `use_run(run_id)` | Returns facade with `default = run_id` (client-side binding) |
| `capabilities()` | Returns system capabilities object |

---

## Keyspace Partitioning

Keyspaces are **partitioned by primitive**. The same key can exist independently in KV, JSON, Vector, etc.

| Primitive | Namespace | Facade delete |
|-----------|-----------|---------------|
| KV | `kv:{key}` | `delete(keys)` |
| JSON | `json:{key}` | `json_del(key, "$")` then `delete` |
| Vector | `vector:{key}` | `vdel(key)` |
| State | `state:{key}` | No facade delete |
| Events | `event:{stream}` | No facade delete |

**Facade `delete(keys)` targets KV only.**

---

## Wire Encoding Contract

### Request Envelope

```json
{
  "id": "client-generated-request-id",
  "op": "kv.set",
  "params": { "key": "x", "value": 123 }
}
```

### Response Envelope (Success)

```json
{
  "id": "client-generated-request-id",
  "ok": true,
  "result": <operation-specific>
}
```

### Response Envelope (Error)

```json
{
  "id": "client-generated-request-id",
  "ok": false,
  "error": {
    "code": "NotFound",
    "message": "Key not found",
    "details": null
  }
}
```

### Frozen Operation Names

**Facade:**
`kv.set`, `kv.get`, `kv.getv`, `kv.mget`, `kv.mset`, `kv.delete`, `kv.exists`, `kv.exists_many`, `kv.incr`,
`json.set`, `json.get`, `json.getv`, `json.del`, `json.merge`,
`event.add`, `event.range`, `event.len`,
`vector.set`, `vector.get`, `vector.del`,
`state.cas_set`, `state.get`,
`history.list`, `history.get_at`, `history.latest_version`,
`run.list`, `run.use`,
`system.capabilities`

**Substrate:**
`substrate.kv.put`, `substrate.kv.get`, `substrate.kv.get_at`, etc.,
`substrate.json.set`, `substrate.json.get`, etc.,
`substrate.event.append`, `substrate.event.range`,
`substrate.vector.set`, `substrate.vector.get`, etc.,
`substrate.state.set`, `substrate.state.get`, `substrate.state.cas`,
`substrate.trace.record`, `substrate.trace.get`, `substrate.trace.range`,
`substrate.run.create`, `substrate.run.get`, `substrate.run.list`, `substrate.run.close`,
`substrate.retention.get`, `substrate.retention.set`,
`txn.begin`, `txn.commit`, `txn.rollback`

### Return Shape Encoding Per Operation

| Operation | Wire Shape |
|-----------|------------|
| `set`, `mset`, `json_set`, `json_merge`, `vset` | `null` |
| `get`, `json_get`, `cas_get` | `Value` or `null` |
| `getv`, `json_getv`, `vget` | `Versioned<Value>` or `null` |
| `mget` | `Array<Value or null>` |
| `delete`, `exists_many`, `json_del`, `xlen` | `int64` |
| `exists`, `vdel`, `cas_set` | `bool` |
| `incr` | `int64` |
| `xadd` | `Version` |
| `xrange`, `history` | `Array<Versioned<Value>>` |

### MessagePack Encoding Constraints (Optional for M11)

Defined for future use:
- All integers MUST be encoded as int64
- All floats MUST be encoded as float64 (preserves NaN/Inf/-0.0)
- Bytes MUST use MessagePack `bin` type
- No extension types allowed

---

## Quick Reference: Versioned<T> Contract

```rust
struct Versioned<T> {
    value: T,
    version: Version,
    timestamp: u64  // microseconds since Unix epoch
}

enum Version {
    Txn(u64),      // For KV, JSON, Vector, Run
    Sequence(u64), // For Events (append-only)
    Counter(u64)   // For StateCell (per-entity CAS)
}
```

Wire encoding:
```json
{
  "value": <Value>,
  "version": { "type": "txn", "value": 123 },
  "timestamp": 1700000000000000
}
```

---

## Quick Reference: Special Wire Wrappers

```json
{"$bytes": "<base64>"}     // Bytes
{"$f64": "NaN"}            // Float NaN
{"$f64": "+Inf"}           // Float +Infinity
{"$f64": "-Inf"}           // Float -Infinity
{"$f64": "-0.0"}           // Float negative zero
{"$absent": true}          // None/missing (for CAS)
```

---

## Document History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-01-22 | Initial M11 implementation plan |
| 1.1 | 2026-01-22 | Cherry-picked from existing M11 plan: M11a/M11b split, VER invariants, DET-5, Rules 7-8, detailed SDK mappings, wire encoding contract, keyspace partitioning, return shape table, CLI exit codes, complete error conditions table, retention/trace operations, MessagePack constraints |

---

**This is the implementation plan. All work must conform to it.**
