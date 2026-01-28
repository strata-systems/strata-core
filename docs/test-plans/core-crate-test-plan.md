# strata-core: Test Refactoring & Strengthening Plan

## Crate Overview

`strata-core` is the foundational type system for the Strata database. It defines the contract between all layers: types, traits, error model, validation rules, and the "Seven Invariants" (Addressable, Versioned, Transactional, Lifecycle-managed, Run-scoped, Introspectable, Read/Write separated).

**Source files**: 14 files, ~12,141 lines
**Public API surface**: ~80 types/traits/functions across 8 modules

## Current Test Inventory

### Unit Tests (inside the crate): 454 tests

| Module | Tests | Coverage |
|--------|-------|----------|
| `contract/entity_ref.rs` | 12 | Constructors, accessors, Display, Eq, Hash, Serde, type checks |
| `contract/version.rs` | 20 | Constructors, ordering (same/cross-variant), increment, saturating, Serde |
| `contract/versioned.rs` | 16 | Accessors, map, into_parts, age, Default, AsRef/AsMut, Serde |
| `contract/timestamp.rs` | 16 | Constructors (secs/millis/micros), now(), ordering, arithmetic, Serde |
| `contract/primitive_type.rs` | 15 | all(), names, ids, from_id, roundtrip, CRUD/append-only, entry_type_range |
| `contract/run_name.rs` | 20 | Validation (empty, length, chars, start), accessors, pattern matching, TryFrom, unchecked |
| `error.rs` | 57 | All constructors, classification, entity_ref/run_id extraction, From conversions, ErrorCode, ConstraintReason, ErrorDetails |
| `value.rs` | 14 | All 8 variants, type checks, extractors, VAL-3 (Int!=Float), VAL-5 (IEEE-754), Serde |
| `key.rs` | 29 | Valid/invalid patterns, NUL bytes, reserved prefix, length limits, custom limits, multi-byte |
| `types.rs` | 59 | RunId (uniqueness, from_string, Serde), Namespace, TypeTag (bytes, ordering), Key (construction, ordering, prefix) |
| `limits.rs` | 24 | Default values, custom limits, validation for all limit types |
| `traits.rs` | 4 | Object safety, Send+Sync bounds |
| `run_types.rs` | 8 | RunStatus, RunMetadata |
| `primitives/json.rs` | 160+ | JSON path operations, patches, merge, limits, nesting |

### Integration Tests (`tests/core/`): ~258 tests across 9 files

| File | Tests | What it tests |
|------|-------|---------------|
| `entity_ref_invariants.rs` | 25 | Constructors, uniqueness, hashing, accessors, type checks, Serde, Display, Clone, Debug |
| `version_invariants.rs` | 28 | Constructors, ordering, increment, overflow, is_zero, Serde, Display, Copy, Clone, Default |
| `versioned_invariants.rs` | 34 | Accessors, map, into_parts, age, generic types, Serde, AsRef/AsMut, Default, Clone |
| `timestamp_invariants.rs` | 31 | Constructors, monotonicity, precision, arithmetic, overflow, Serde, Display, Copy, Default |
| `primitive_type_invariants.rs` | 24 | Variants, all(), names, ids, from_id, CRUD/append-only, Hash, Serde, Display, Copy |
| `run_name_invariants.rs` | 38 | Validation rules, accessors, pattern matching, TryFrom, error display, Hash, Serde |
| `strata_error.rs` | 58 | All constructors, classification, entity_ref extraction, From conversions, Display, Debug |
| `cross_type_integration.rs` | 20 | EntityRef+PrimitiveType consistency, Versioned+Version combos, Versioned+Timestamp |
| `error_handling.rs` | ~32 | Transaction error propagation, retry semantics, panic safety, concurrent errors |

## Diagnosis

### Problem 1: Massive Redundancy (7 of 9 files)

Seven integration test files are near-complete duplicates of the unit tests:

| Integration File | Overlapping Unit Module | Unique Tests |
|-----------------|------------------------|--------------|
| `entity_ref_invariants.rs` | `contract/entity_ref.rs` | ~3 (Debug trait check, clone, two-ref inequality) |
| `version_invariants.rs` | `contract/version.rs` | ~3 (saturating_increment, cross-variant sorting) |
| `versioned_invariants.rs` | `contract/versioned.rs` | ~5 (generic type tests: String, Vec, Option, unit, nested) |
| `timestamp_invariants.rs` | `contract/timestamp.rs` | ~4 (monotonicity stress, large values, zero-duration ops) |
| `primitive_type_invariants.rs` | `contract/primitive_type.rs` | ~2 (names/ids non-empty checks) |
| `run_name_invariants.rs` | `contract/run_name.rs` | ~3 (unicode rejected, numeric start, all-digits) |
| `strata_error.rs` | `error.rs` | ~5 (entity type coverage, wire encoding cross-checks) |

These files test the same constructors, the same accessors, the same Serde roundtrips, and the same trait implementations. Integration tests exist to test cross-crate behavior -- testing `EntityRef::kv()` returns the right variant is a unit test, not an integration test.

### Problem 2: Broken Imports (3 files)

Three files reference `JsonDocId`, a type that has been deleted from the codebase:
- `entity_ref_invariants.rs` (5 references)
- `strata_error.rs` (6 references)
- `cross_type_integration.rs` (2 references)

None of these files compile.

### Problem 3: Misplaced File (1 file)

`error_handling.rs` tests `Database`, `Transaction`, `RetryConfig`, concurrent thread behavior, and panic safety. It imports from `strata_engine`, not just `strata_core`. This is an **engine/concurrency** integration test, not a core type test.

### Problem 4: Unit Test Gaps

Despite 454 unit tests, there are specific gaps:

1. **`value.rs` (14 tests)** -- Weakest coverage relative to complexity:
   - No tests for `From<i64>`, `From<f64>`, `From<String>`, `From<bool>`, `From<Vec<u8>>` conversions
   - No tests for `From<serde_json::Value>` / `Into<serde_json::Value>` roundtrip
   - No tests for nested Array/Object construction
   - No tests for `Value::Object` key ordering behavior
   - No tests for `as_*()` returning `None` for wrong types
   - No edge cases: empty string, empty bytes, empty array, empty object

2. **`traits.rs` (4 tests)** -- Only tests trait object safety and Send+Sync. No mock-based tests for `Storage` or `SnapshotView` method contracts.

3. **`primitives/event.rs`** -- No unit tests found. `Event` and `ChainVerification` are untested at the unit level.

4. **`primitives/state.rs`** -- No unit tests found. `State` type is untested.

5. **`primitives/vector.rs`** -- No unit tests found for `VectorId`, `VectorEntry`, `VectorMatch`, `VectorConfig`, `CollectionInfo`, `DistanceMetric`, `StorageDtype`, `MetadataFilter`, `JsonScalar`.

6. **`run_types.rs` (8 tests)** -- Light coverage. No tests for `RunEventOffsets`, no edge cases for `RunMetadata` serialization.

7. **`contract/entity_ref.rs`** -- No test for `EntityRef::json()` with various doc_id types (it takes `impl Into<String>`).

8. **`limits.rs`** -- No tests for interaction between limits (e.g., max_value_bytes_encoded vs max_string_bytes).

## Plan

### Step 1: Delete redundant integration tests

Remove 7 files that duplicate unit tests:

```
DELETE tests/core/entity_ref_invariants.rs
DELETE tests/core/version_invariants.rs
DELETE tests/core/versioned_invariants.rs
DELETE tests/core/timestamp_invariants.rs
DELETE tests/core/primitive_type_invariants.rs
DELETE tests/core/run_name_invariants.rs
DELETE tests/core/strata_error.rs
```

Before deletion, salvage any unique tests (listed above) and add them to the unit test modules inside the crate.

### Step 2: Relocate `error_handling.rs`

Move `tests/core/error_handling.rs` to `tests/engine/error_handling.rs`. It tests `Database::transaction()`, `transaction_with_timeout()`, `transaction_with_retry()`, and `RetryConfig` -- all engine-layer concepts.

### Step 3: Fix or delete `cross_type_integration.rs`

This file references deleted `JsonDocId`. Two options:
- **Fix**: Replace `JsonDocId::new()` with `"test_doc".to_string()` (since `EntityRef::json()` now takes `impl Into<String>`). Keep only the tests that validate cross-type consistency (EntityRef+PrimitiveType mapping, Versioned+Version composition).
- **Preferred**: Delete it entirely. The unit tests in `entity_ref.rs` already test `primitive_type()`, and `versioned.rs` already tests composition. The 20 tests here are redundant.

### Step 4: Strengthen unit tests inside the crate

#### 4a. `value.rs` -- Add ~15 tests

```
test_from_i64                          # From<i64> conversion
test_from_f64                          # From<f64> conversion
test_from_bool                         # From<bool> conversion
test_from_string                       # From<String> conversion
test_from_str_ref                      # From<&str> conversion
test_from_vec_u8                       # From<Vec<u8>> conversion
test_serde_json_value_roundtrip        # Value <-> serde_json::Value
test_as_wrong_type_returns_none        # as_int() on String, etc.
test_empty_string                      # Value::String("")
test_empty_bytes                       # Value::Bytes(vec![])
test_empty_array                       # Value::Array(vec![])
test_empty_object                      # Value::Object(BTreeMap::new())
test_nested_array                      # Array containing Arrays
test_nested_object                     # Object containing Objects
test_value_debug                       # Debug formatting
```

#### 4b. `primitives/event.rs` -- Add ~8 tests

```
test_event_construction                # Event::new() or equivalent
test_event_payload_accessor            # Accessing event payload
test_event_sequence_accessor           # Accessing sequence number
test_event_timestamp                   # Event timestamp
test_event_serialization               # Serde roundtrip
test_event_clone                       # Clone independence
test_chain_verification_valid          # ChainVerification for valid chain
test_chain_verification_broken         # ChainVerification for broken chain
```

#### 4c. `primitives/state.rs` -- Add ~6 tests

```
test_state_construction                # State creation
test_state_value_accessor              # Getting the value
test_state_version_accessor            # Getting the version
test_state_serialization               # Serde roundtrip
test_state_equality                    # PartialEq
test_state_clone                       # Clone
```

#### 4d. `primitives/vector.rs` -- Add ~12 tests

```
test_vector_id_creation                # VectorId construction
test_vector_id_as_u64                  # VectorId -> u64
test_vector_id_ordering                # VectorId comparison
test_vector_id_zero                    # VectorId(0) behavior
test_vector_config_construction        # VectorConfig with dimension + metric
test_vector_config_serialization       # Serde roundtrip
test_distance_metric_variants          # All 3 metrics
test_storage_dtype_variants            # F32, F16
test_vector_entry_construction         # VectorEntry with data + metadata
test_vector_match_construction         # VectorMatch with score
test_collection_info_construction      # CollectionInfo fields
test_metadata_filter_construction      # MetadataFilter
```

#### 4e. `contract/entity_ref.rs` -- Add ~3 tests

```
test_entity_ref_json_with_string       # EntityRef::json(run_id, String)
test_entity_ref_json_with_str          # EntityRef::json(run_id, &str)
test_entity_ref_wrong_accessor_none    # kv_key() on Event returns None, etc.
```

#### 4f. `run_types.rs` -- Add ~4 tests

```
test_run_event_offsets_construction    # RunEventOffsets fields
test_run_event_offsets_serialization   # Serde roundtrip
test_run_metadata_serialization        # Serde roundtrip
test_run_status_transitions            # Valid status transitions
```

### Step 5: Update `tests/core/main.rs`

After all changes, the file should contain only:

```rust
mod common;
// No test modules -- all core tests are unit tests inside the crate.
// (Or just delete the tests/core/ directory entirely if empty)
```

If `cross_type_integration.rs` is kept (fixed):

```rust
mod common;
mod cross_type_integration;
```

### Step 6: Clean up `tests/common/mod.rs`

Remove helper functions that only served the deleted integration tests:
- `all_entity_refs()` (if unused elsewhere)
- `all_primitive_types()` (if unused elsewhere)
- `test_run_id()` (if unused elsewhere)
- `assert_hashable()` (if unused elsewhere)
- `assert_same_hash()` (if unused elsewhere)

Check all usages across `tests/` before removing.

## Summary

| Action | Files | Tests Removed | Tests Added |
|--------|-------|---------------|-------------|
| Delete redundant integration tests | -7 files | ~238 | 0 |
| Salvage unique tests to unit modules | 0 | 0 | ~25 |
| Relocate error_handling.rs | move 1 file | 0 | 0 |
| Fix or delete cross_type_integration.rs | -1 file | ~20 | 0 |
| Strengthen value.rs unit tests | 1 file | 0 | ~15 |
| Add primitives unit tests | 3 files | 0 | ~26 |
| Add entity_ref unit tests | 1 file | 0 | ~3 |
| Add run_types unit tests | 1 file | 0 | ~4 |
| **Total** | **-8 files** | **~258** | **~73** |

**Net effect**: 258 redundant integration tests removed, 73 targeted unit tests added. The crate goes from 454+258=712 tests (with massive overlap) to 454+73=527 tests (no overlap, better coverage of actual gaps).
