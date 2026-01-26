# Expert Code Review: strata-core

## Summary

Thorough expert review of the `strata-core` crate looking for inconsistencies, dead code, bad practices, and architectural gaps.

---

## Critical Architectural Issues

### 1. Timestamp Unit Inconsistency

**Location**: `primitives/event.rs:24`, `primitives/state.rs:22`, `contract/timestamp.rs`

The crate has an inconsistent timestamp model:

| Type | Field | Unit |
|------|-------|------|
| `Event` | `timestamp` | `i64` **milliseconds** |
| `State` | `updated_at` | `i64` **milliseconds** |
| `contract::Timestamp` | inner | `u64` **microseconds** |

This is a **significant architectural gap**. The contract types use microseconds but the primitive data types use milliseconds. Any code mixing these will have subtle bugs (1000x difference).

**Impact**: Data corruption potential when timestamps are compared or stored across boundaries.

### 2. JsonDocId Forces UUID While Other Primitives Use String Keys

**Location**: `types.rs:196-263`, `crates/primitives/src/json_store.rs`, `crates/api/src/substrate/impl_.rs:167-200`

The JSON primitive uses system-generated UUIDs (`JsonDocId`) for document identification while all other primitives accept user-provided string keys:

| Primitive | Key Type | Who Assigns |
|-----------|----------|-------------|
| KV | `&str` | User provides |
| State | `&str` | User provides |
| Vector | `&str` | User provides |
| **JSON** | `JsonDocId` (UUID) | **System generates** |

**Problems:**

1. **Lossy string-to-UUID conversion**: The API layer (`parse_doc_id()`) hashes user strings into UUIDs, losing the original key name:
   ```rust
   // User passes "my-document", gets converted to opaque UUID hash
   fn parse_doc_id(doc_id: &str) -> StrataResult<JsonDocId> {
       // ... hashes string to deterministic UUID
   }
   ```

2. **Broken prefix scanning**: With string keys you can do `kv.list(prefix: "user:")`. With JSON's UUID storage, prefix filtering is meaningless.

3. **No UUID-specific features used**: The codebase never uses UUID version bits, variant bits, or any UUID-specific functionality. A string key would work identically.

4. **Manual ID management burden**: Tests must track UUIDs manually instead of using natural string identifiers.

**Impact**: API inconsistency, lost functionality (prefix scanning), unnecessary complexity, cognitive load for users who must learn different patterns for JSON vs other primitives.

**Recommendation**: Change JSON to accept string keys like all other primitives. Remove `JsonDocId` type entirely.

### 3. Four Competing Error Systems (One is Dead Code)

**Location**: `core/error.rs`, `core/api_error.rs`, `executor/error.rs`

The codebase has **four** error types when it should have one or two:

| Type | Location | Variants | Status |
|------|----------|----------|--------|
| `Error` (legacy) | `core/error.rs:395-467` | 13 | Transitional |
| `StrataError` | `core/error.rs:718-1098` | 21 | **Primary** |
| `executor::Error` | `executor/error.rs:44-153` | 23 | API layer |
| `ApiError` | `core/api_error.rs:89-182` | 14 | **DEAD CODE** |

**Problems:**

1. **ApiError is completely unused**: Defined with 14 variants and `WireError` struct, but **zero usages** anywhere in the codebase. Pure dead code that should be deleted.

2. **Lossy conversions lose information**:
   ```rust
   // TransactionTimeout loses message entirely!
   Error::TransactionTimeout(msg) → StrataError::TransactionTimeout { duration_ms: 0 }

   // TransactionConflict creates fake EntityRef
   Error::TransactionConflict(msg) → StrataError::WriteConflict { entity_ref: placeholder }

   // CapacityExceeded loses structured fields
   StrataError::CapacityExceeded { resource, limit, requested }
     → executor::Error::ConstraintViolation { reason: "flat string" }
   ```

3. **Redundant variants across all types**: Every type has its own NotFound, WrongType, Conflict, InvalidInput, Internal variants.

4. **Conversion chain is convoluted**:
   ```
   Error (legacy) → StrataError → executor::Error → User APIs
   ```

**Recommendation**:

| Phase | Action | Risk |
|-------|--------|------|
| **Immediate** | Delete `api_error.rs` entirely | Zero |
| **Short-term** | Fix lossy conversions in `Error → StrataError` | Low |
| **Medium-term** | Evaluate merging executor::Error into StrataError | Medium |
| **Long-term** | Retire legacy `Error` type | Low |

**Minimum needed**: Just `StrataError` (primary) + optionally `executor::Error` (API specialization).

### 4. Architecture: Storage Trait Foundation with Peer Primitives

**Location**: `traits.rs`, `types.rs`, `primitives/`

The codebase follows a **peer primitives on unified storage** architecture:

```
┌─────────────────────────────────────────────────────────┐
│                    User-Facing API                       │
│         (consistent abstraction for all primitives)      │
├──────────┬──────────┬──────────┬──────────┬─────────────┤
│ KVStore  │ EventLog │StateCell │JsonStore │ VectorStore │
│  .get()  │ .append()│  .read() │ .create()│  .insert()  │
│  .put()  │  .read() │ .write() │  .get()  │  .search()  │
├──────────┴──────────┴──────────┴──────────┴─────────────┤
│              Storage Trait (internal foundation)         │
│                    Key + Value + TypeTag                 │
└─────────────────────────────────────────────────────────┘
```

**Key + Value serve dual purposes:**
- Internal storage plumbing (used by ALL primitives via Storage trait)
- User-facing semantic types for KVStore (the primitive where users work with raw key-value pairs)

**Semantic types per primitive:**

| Primitive | User-Facing Type | Location | Status |
|-----------|------------------|----------|--------|
| KV | `Value` (directly) | `core/value.rs` | ✓ Correct - no wrapper needed |
| Event | `Event` | `core/primitives/event.rs` | ✓ Correct |
| State | `State` | `core/primitives/state.rs` | ✓ Correct |
| Vector | `VectorEntry`, `VectorMatch` | `core/primitives/vector.rs` | ✓ Correct |
| JSON | `JsonValue`, `JsonDoc` | Scattered (see below) | ⚠️ Inconsistent |

**Remaining Issue - JSON types scattered:**
- `JsonValue`, `JsonPath`, `JsonPatch` → `core/src/json.rs` (root level, not in primitives/)
- `JsonDocId` → `core/src/types.rs` (mixed with other types)
- `JsonDoc` → `primitives/src/json_store.rs` (not in core at all)

**Recommendation**:
- Move JSON types from `core/json.rs` to `core/primitives/json.rs` for consistency
- Move `JsonDoc` to core (currently only in primitives crate)

---

## Moderate Issues

### 5. Version Consistency Issue in Primitives

**Location**: `primitives/vector.rs:222`, `primitives/state.rs:19`

| Type | Version Field | Type Used |
|------|---------------|-----------|
| `VectorEntry` | `version` | `u64` |
| `State` | `version` | `u64` |
| Contract types | - | `Version` enum |

The primitive types use raw `u64` for versioning while the contract layer uses `Version` enum with semantic variants (Txn, Sequence, Counter). This bypasses type safety.

**Impact**: No compile-time enforcement of correct version types.

### 6. Duplicate LimitError Names

**Location**: `json.rs:54-91` and `limits.rs:62-84`

Two unrelated `LimitError` types exist:
- `json::LimitError` - Document size/depth/array limits
- `limits::LimitError` - Key/string/vector validation limits

**Impact**: Import confusion, potential name collisions.

### 7. Documentation Claims 9 Error Codes, Enum Has 10

**Location**: `error.rs:12` (doc comment) vs `error.rs:93-115` (enum)

The module doc states "The following 9 error codes" but `ErrorCode` has 10 variants.

**Status**: Fixed in STRATA_CORE.md - now correctly states "10 error codes".

---

## Minor Issues & Dead Code

### 8. Dead Code: `placeholder()` Function

**Location**: `lib.rs`

```rust
pub fn placeholder() {
    // This crate will contain core types once implemented
}
```

This function does nothing and should be removed.

### 9. Deprecated Items That Should Be Removed

| Item | Location | Deprecated Since |
|------|----------|------------------|
| `TypeTag::Trace` | `types.rs` | 0.12.0 |
| `PrimitiveKind` alias | `lib.rs`, `search_types.rs` | 0.9.0 |
| `primitive_kind()` method | `entity_ref.rs` | 0.9.0 |
| `is_txn_id()` method | `version.rs` | 0.11.0 |

The `PrimitiveKind` deprecation is duplicated in two files.

### 10. `RunName::new_unchecked()` Bypasses Validation

**Location**: `contract/run_name.rs:125-127`

```rust
pub fn new_unchecked(name: impl Into<String>) -> Self {
    RunName(name.into())
}
```

This allows creating invalid `RunName` values without any safety mechanism. The function should have stronger documentation warnings or be `#[doc(hidden)]`.

---

## Potential Future Issues

### 11. Fixed Hash Size in Event Chain

**Location**: `primitives/event.rs:26-28`

```rust
pub prev_hash: [u8; 32],
pub hash: [u8; 32],
```

The hash size is hardcoded to 32 bytes. If SHA-256 ever needs to be replaced (e.g., for SHA-3 or post-quantum algorithms), this is a breaking change affecting the on-disk format.

### 12. Version Cross-Variant Comparison

**Location**: `contract/version.rs:180-202`

The `Ord` implementation defines a total ordering across version variants:
```rust
// TxnId < Sequence < Counter
```

But the documentation says "Cross-variant comparison is undefined". This mismatch could cause confusion.

### 13. `Value` Cannot Be Used as HashMap Key

**Location**: `value.rs:43-61`

`Value` implements `PartialEq` but not `Eq` (intentionally, due to IEEE-754 float semantics where `NaN != NaN`). This means `Value` cannot be used in `HashSet` or as `HashMap` keys, which may surprise users.

### 14. JsonScalar Float Comparison

**Location**: `primitives/vector.rs:392-393`

```rust
(JsonScalar::Number(a), serde_json::Value::Number(b)) => {
    b.as_f64().is_some_and(|n| (a - n).abs() < f64::EPSILON)
}
```

This comparison uses `f64::EPSILON` which is suitable only for values near 1.0. For very large or very small numbers, this comparison will be incorrect.

---

## Recommendations

### Immediate Fixes (Low Risk)

1. Remove `placeholder()` function
2. Fix documentation: "9 error codes" → "10 error codes"
3. Consolidate `PrimitiveKind` deprecation to single location
4. **Delete `api_error.rs`**: Remove dead code (`ApiError`, `WireError`) - zero risk, never used
5. Add `#[deprecated]` to legacy `Error` enum pointing to `StrataError`

### Short-Term Improvements

6. Unify timestamp units: Choose either milliseconds or microseconds for all types
7. Use `Version` enum in primitive types instead of raw `u64`
8. Rename one of the `LimitError` types (e.g., `KeyLimitError`, `JsonLimitError`)
9. **Remove `JsonDocId`**: Change JSON primitive to accept string keys like KV/State/Vector
10. **Normalize JSON types**: Move `json.rs` to `primitives/json.rs`, move `JsonDoc` to core
11. **Fix lossy error conversions**: Preserve `TransactionTimeout` message, don't create fake `EntityRef` placeholders

### Long-Term Considerations

12. **Consolidate error types**: Evaluate merging `executor::Error` into `StrataError`, retire legacy `Error`
13. Consider making hash size configurable or using a newtype wrapper
14. Document the Version ordering behavior explicitly
15. Consider adding a `ValueKey` wrapper that excludes floats for use as map keys

---

## Test Coverage Assessment

The crate has comprehensive test coverage. Most modules have substantial test suites verifying:
- Serialization roundtrips
- Edge cases (empty values, max values)
- Error conditions
- Type conversions

This is a positive sign for code quality.

---

## Overall Assessment

**Rating**: Functional with moderate architectural debt

The `strata-core` crate has a sound foundational architecture: **peer primitives on unified storage**, where all primitives (KV, Event, State, JSON, Vector) use the Storage trait internally while providing consistent user-facing APIs. `Key` and `Value` serve dual purposes as both internal plumbing and KV's user-facing types.

**Main issues:**

1. **Timestamp inconsistency** between primitive types (milliseconds) and contract types (microseconds) - causes 1000x bugs when mixed
2. **JsonDocId UUID asymmetry** - JSON uses system-generated UUIDs while all other primitives use user-provided string keys, breaking consistency and losing functionality
3. **Four competing error systems** - `Error`, `StrataError`, `executor::Error`, and `ApiError` (dead code) with lossy conversions between them
4. **JSON types scattered** - `JsonValue` at root level, `JsonDocId` in types.rs, `JsonDoc` only in primitives crate (not core)
5. **Dead code** - `ApiError`/`WireError` never used, `placeholder()` function, deprecated items

The crate follows good Rust practices (derive macros, visibility modifiers, comprehensive tests). The architecture is intentional, though some inconsistencies remain from incremental development.

**Immediate wins**: Delete `api_error.rs` (dead code), remove `placeholder()`, deprecate legacy `Error` type.
