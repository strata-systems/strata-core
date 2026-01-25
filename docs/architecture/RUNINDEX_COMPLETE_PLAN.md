# RunIndex Complete Plan

> Substrate API completion for the RunIndex primitive.
> Goal: Expose all primitive capabilities with a tight, disciplined API surface.

---

## The One Conceptual Line We Must Protect

**RunIndex is an execution index, not an execution engine.**

Everything exposed must reinforce this sentence. RunIndex:
- Tracks execution contexts (runs) as first-class entities
- Enforces valid lifecycle state transitions
- Provides indexed queries over run metadata
- Does NOT schedule, orchestrate, or execute anything

This distinction prevents RunIndex from becoming a workflow engine, scheduler, or orchestration layer.

---

## Executive Summary

RunIndex has a **complete, production-ready implementation at the primitive level** that is partially hidden at substrate. This plan exposes the full capability while maintaining API discipline.

**Current State:**
- Primitive: 20+ methods, fully tested
- Substrate: 13 methods (2 stubbed, 8 hidden features)
- Tests: No comprehensive substrate test suite

**Target State:**
- Substrate: 24 methods (all working)
- Tests: 110+ tests across 10 modules

---

## Design Principles

### 1. One Substrate Method = One Primitive Method
No complex orchestration. Each substrate method is a thin wrapper over exactly one primitive call.

### 2. Preserve Primitive Semantics
Don't change behavior at substrate. If primitive returns `Vec<RunMetadata>`, substrate converts to `Vec<Versioned<RunInfo>>` - nothing more.

### 3. Validate at Boundaries Only
- Substrate validates `ApiRunId` format
- Substrate validates "default run cannot be deleted/closed"
- All other validation delegated to primitive

### 4. Consistent Error Mapping
```rust
fn convert_error(e: strata_core::error::Error) -> StrataError {
    // Already exists - reuse consistently
}
```

### 5. No New Primitive Changes
The primitive is complete. All work is substrate exposure + tests.

---

## Phase 0: Audit Current State

### Substrate Trait Methods (current)

```rust
// RunIndex trait in crates/api/src/substrate/run.rs
pub trait RunIndex {
    // CRUD
    fn run_create(run_id?, metadata?) -> (RunInfo, Version);
    fn run_get(run) -> Option<Versioned<RunInfo>>;
    fn run_list(state?, limit?, offset?) -> Vec<Versioned<RunInfo>>;
    fn run_exists(run) -> bool;
    fn run_update_metadata(run, metadata) -> Version;

    // Lifecycle
    fn run_close(run) -> Version;          // Maps to complete_run
    fn run_pause(run) -> Version;
    fn run_resume(run) -> Version;
    fn run_fail(run, error) -> Version;
    fn run_delete(run) -> ();

    // Query
    fn run_query_by_status(state) -> Vec<Versioned<RunInfo>>;

    // Retention (STUBBED)
    fn run_set_retention(run, policy) -> Version;
    fn run_get_retention(run) -> RetentionPolicy;
}
```

### Primitive Methods (available but hidden)

```rust
// RunIndex in crates/primitives/src/run_index.rs
impl RunIndex {
    // Hidden lifecycle
    fn cancel_run(run_id) -> Versioned<RunMetadata>;
    fn archive_run(run_id) -> Versioned<RunMetadata>;

    // Hidden queries
    fn query_by_tag(tag) -> Vec<RunMetadata>;
    fn get_child_runs(parent) -> Vec<RunMetadata>;
    fn count() -> usize;
    fn search(req) -> SearchResponse;

    // Hidden tag management
    fn add_tags(run_id, tags) -> Versioned<RunMetadata>;
    fn remove_tags(run_id, tags) -> Versioned<RunMetadata>;

    // Hidden parent-child
    fn create_run_with_options(run_id, parent, tags, metadata) -> Versioned<RunMetadata>;
}
```

---

## Phase 1: Expose Hidden Lifecycle Methods

### Canonical State Transition Table

This table is the single source of truth for valid transitions. No exceptions.

```
From State   → Valid Transitions
─────────────────────────────────────────────────────────────
Active       → Completed | Failed | Cancelled | Paused | Archived
Paused       → Active | Cancelled | Archived
Completed    → Archived
Failed       → Archived
Cancelled    → Archived
Archived     → (TERMINAL - no transitions allowed)
```

**Invariants:**
- No resurrection: Once `Completed`, `Failed`, or `Cancelled`, cannot return to `Active`
- Terminal is final: `Archived` accepts no further transitions
- Pause is reversible: `Paused` can resume to `Active`

This table anchors all tests and prevents "just let resume from failed" arguments.

### Task 1.1: Add `run_cancel` to substrate

**Primitive:** `cancel_run(run_id: &str) -> Result<Versioned<RunMetadata>>`

**Substrate Signature:**
```rust
/// Cancel a run
///
/// Transitions the run to Cancelled state.
/// This is distinct from failure (user-initiated vs error).
///
/// ## Errors
/// - `NotFound`: Run does not exist
/// - `ConstraintViolation`: Run is in terminal state or cannot be cancelled
fn run_cancel(&self, run: &ApiRunId) -> StrataResult<Version>;
```

**Implementation:**
```rust
fn run_cancel(&self, run: &ApiRunId) -> StrataResult<Version> {
    let run_str = api_run_id_to_string(run);
    let versioned = self.run().cancel_run(&run_str).map_err(convert_error)?;
    Ok(versioned.version)
}
```

### Task 1.2: Add `run_archive` to substrate

**Primitive:** `archive_run(run_id: &str) -> Result<Versioned<RunMetadata>>`

**Substrate Signature:**
```rust
/// Archive a run (soft delete)
///
/// Transitions the run to Archived state (terminal).
/// Data is preserved but run is hidden from normal queries.
///
/// ## Errors
/// - `NotFound`: Run does not exist
/// - `ConstraintViolation`: Run is already archived or cannot be archived
fn run_archive(&self, run: &ApiRunId) -> StrataResult<Version>;
```

**Implementation:**
```rust
fn run_archive(&self, run: &ApiRunId) -> StrataResult<Version> {
    let run_str = api_run_id_to_string(run);
    let versioned = self.run().archive_run(&run_str).map_err(convert_error)?;
    Ok(versioned.version)
}
```

### Task 1.3: Fix `run_close` semantics

**Current:** `run_close` calls `complete_run` - marks as Completed.
**Problem:** Semantically confusing. "Close" sounds like "archive" but does "complete".

**Decision:** Keep current behavior but document clearly:
```rust
/// Close a run (mark as completed)
///
/// Close is synonymous with successful completion. Transitions the run
/// to Completed state. This is the happy-path termination.
///
/// For other termination modes:
/// - `run_fail` for failures (with error message)
/// - `run_cancel` for user-initiated cancellation
/// - `run_archive` for soft delete (terminal)
///
/// NOTE: The default run cannot be closed.
fn run_close(&self, run: &ApiRunId) -> StrataResult<Version>;
```

---

## Phase 2: Expose Query Methods

### Task 2.1: Add `run_query_by_tag` to substrate

**Primitive:** `query_by_tag(tag: &str) -> Result<Vec<RunMetadata>>`

**Substrate Signature:**
```rust
/// Query runs by tag
///
/// Returns all runs that have the specified tag.
///
/// ## Parameters
/// - `tag`: The tag to search for
///
/// ## Return Value
/// Vector of run info for matching runs.
fn run_query_by_tag(&self, tag: &str) -> StrataResult<Vec<Versioned<RunInfo>>>;
```

**Implementation:**
```rust
fn run_query_by_tag(&self, tag: &str) -> StrataResult<Vec<Versioned<RunInfo>>> {
    let runs = self.run().query_by_tag(tag).map_err(convert_error)?;
    Ok(runs.into_iter().map(metadata_to_versioned_info).collect())
}
```

### Task 2.2: Add `run_count` to substrate

**Primitive:** `count() -> Result<usize>`

**Substrate Signature:**
```rust
/// Count runs
///
/// Returns the total number of runs, optionally filtered by status.
///
/// ## Parameters
/// - `status`: Optional status filter
fn run_count(&self, status: Option<RunState>) -> StrataResult<u64>;
```

**Implementation:**
```rust
fn run_count(&self, status: Option<RunState>) -> StrataResult<u64> {
    match status {
        Some(s) => {
            let primitive_status = convert_run_state_to_status(s);
            let runs = self.run().query_by_status(primitive_status).map_err(convert_error)?;
            Ok(runs.len() as u64)
        }
        None => {
            let count = self.run().count().map_err(convert_error)?;
            Ok(count as u64)
        }
    }
}
```

### Task 2.3: Add `run_search` to substrate

**Primitive:** `search(req: &SearchRequest) -> Result<SearchResponse>`

**IMPORTANT CONSTRAINT:**

`run_search` is **metadata and index search only**. It:
- Searches run IDs, status, tags, and user-provided metadata
- Does NOT execute code
- Does NOT rehydrate run state
- Does NOT inspect run contents (KV, Events, State, JSON)
- Does NOT search across primitives scoped to the run

This is **RunIndex search**, not Strata-wide search. The search scope is strictly the RunMetadata fields.

**Substrate Signature:**
```rust
/// Search runs (metadata and index only)
///
/// Searches run IDs, status, tags, and metadata fields.
/// Does NOT search run contents (KV, Events, State, etc.).
///
/// ## Parameters
/// - `query`: Search query string
/// - `limit`: Maximum results to return
fn run_search(&self, query: &str, limit: Option<u64>) -> StrataResult<Vec<Versioned<RunInfo>>>;
```

**Implementation:**
```rust
fn run_search(&self, query: &str, limit: Option<u64>) -> StrataResult<Vec<Versioned<RunInfo>>> {
    use strata_core::{SearchRequest, SearchBudget};

    let req = SearchRequest {
        run_id: RunId::from_bytes([0; 16]), // Global namespace
        query: query.to_string(),
        k: limit.unwrap_or(10) as usize,
        budget: SearchBudget::default(),
        time_range: None,
        filter: None,
    };

    let response = self.run().search(&req).map_err(convert_error)?;

    // Convert hits to RunInfo
    let mut results = Vec::new();
    for hit in response.hits {
        if let strata_core::search_types::DocRef::Run { run_id } = hit.doc_ref {
            // Look up the run
            if let Some(info) = self.run_get(&ApiRunId::from_uuid(
                uuid::Uuid::from_bytes(run_id.to_bytes())
            ))? {
                results.push(info);
            }
        }
    }
    Ok(results)
}
```

---

## Phase 3: Expose Tag Management

### Task 3.1: Add `run_add_tags` to substrate

**Primitive:** `add_tags(run_id: &str, tags: Vec<String>) -> Result<Versioned<RunMetadata>>`

**Substrate Signature:**
```rust
/// Add tags to a run
///
/// Tags are used for categorization and querying.
/// Duplicate tags are ignored.
///
/// ## Parameters
/// - `run`: The run to tag
/// - `tags`: Tags to add
fn run_add_tags(&self, run: &ApiRunId, tags: &[String]) -> StrataResult<Version>;
```

**Implementation:**
```rust
fn run_add_tags(&self, run: &ApiRunId, tags: &[String]) -> StrataResult<Version> {
    let run_str = api_run_id_to_string(run);
    let versioned = self.run().add_tags(&run_str, tags.to_vec()).map_err(convert_error)?;
    Ok(versioned.version)
}
```

### Task 3.2: Add `run_remove_tags` to substrate

**Primitive:** `remove_tags(run_id: &str, tags: Vec<String>) -> Result<Versioned<RunMetadata>>`

**Substrate Signature:**
```rust
/// Remove tags from a run
///
/// Tags that don't exist are ignored.
///
/// ## Parameters
/// - `run`: The run to untag
/// - `tags`: Tags to remove
fn run_remove_tags(&self, run: &ApiRunId, tags: &[String]) -> StrataResult<Version>;
```

**Implementation:**
```rust
fn run_remove_tags(&self, run: &ApiRunId, tags: &[String]) -> StrataResult<Version> {
    let run_str = api_run_id_to_string(run);
    let versioned = self.run().remove_tags(&run_str, tags.to_vec()).map_err(convert_error)?;
    Ok(versioned.version)
}
```

### Task 3.3: Add `run_get_tags` to substrate

**Note:** Primitive doesn't have a dedicated method - extract from RunMetadata.

**Substrate Signature:**
```rust
/// Get tags for a run
///
/// ## Parameters
/// - `run`: The run to get tags for
fn run_get_tags(&self, run: &ApiRunId) -> StrataResult<Vec<String>>;
```

**Implementation:**
```rust
fn run_get_tags(&self, run: &ApiRunId) -> StrataResult<Vec<String>> {
    let run_str = api_run_id_to_string(run);
    let meta = self.run().get_run(&run_str).map_err(convert_error)?
        .ok_or_else(|| StrataError::not_found(
            strata_core::EntityRef::run(run.to_run_id()),
            "Run not found"
        ))?;
    Ok(meta.value.tags)
}
```

---

## Phase 4: Expose Parent-Child Relationships

### Frozen Semantics: Informational, Not Transactional

**Parent-child relationships are informational, not transactional.**

This means:
- **No cascading state:** Parent state changes do NOT affect children
- **No implicit propagation:** Tags, metadata, retention do NOT inherit
- **No shared lifecycle:** Parent completion does NOT complete children
- **No transactional coupling:** Parent and child are independent execution contexts

The parent pointer is metadata for querying and organization only. It enables:
- "Show me all runs forked from X"
- "What was the parent of this run?"
- Hierarchical visualization

It does NOT enable:
- DAG workflow execution
- Distributed transactions
- Inherited configuration

**Delete semantics:** Deleting a parent does NOT delete children. Children become orphaned (parent_run becomes dangling reference). This is intentional - runs are independent execution contexts.

### Task 4.1: Add `run_create_child` to substrate

**Primitive:** `create_run_with_options(run_id, parent: Option<String>, tags, metadata)`

**Substrate Signature:**
```rust
/// Create a child run
///
/// Creates a new run with a parent relationship.
/// Useful for forked/nested runs.
///
/// ## Parameters
/// - `parent`: The parent run
/// - `metadata`: Optional metadata for the new run
///
/// ## Return Value
/// Returns the new run info and version.
fn run_create_child(
    &self,
    parent: &ApiRunId,
    metadata: Option<Value>,
) -> StrataResult<(RunInfo, Version)>;
```

**Implementation:**
```rust
fn run_create_child(
    &self,
    parent: &ApiRunId,
    metadata: Option<Value>,
) -> StrataResult<(RunInfo, Version)> {
    let parent_str = api_run_id_to_string(parent);
    let child_id = uuid::Uuid::new_v4().to_string();

    let versioned = self.run().create_run_with_options(
        &child_id,
        Some(parent_str),
        vec![],
        metadata.unwrap_or(Value::Null),
    ).map_err(convert_error)?;

    let info = metadata_to_run_info(&versioned.value);
    Ok((info, versioned.version))
}
```

### Task 4.2: Add `run_get_children` to substrate

**Primitive:** `get_child_runs(parent_id: &str) -> Result<Vec<RunMetadata>>`

**Substrate Signature:**
```rust
/// Get child runs
///
/// Returns all runs that have the specified run as their parent.
///
/// ## Parameters
/// - `parent`: The parent run
fn run_get_children(&self, parent: &ApiRunId) -> StrataResult<Vec<Versioned<RunInfo>>>;
```

**Implementation:**
```rust
fn run_get_children(&self, parent: &ApiRunId) -> StrataResult<Vec<Versioned<RunInfo>>> {
    let parent_str = api_run_id_to_string(parent);
    let children = self.run().get_child_runs(&parent_str).map_err(convert_error)?;
    Ok(children.into_iter().map(metadata_to_versioned_info).collect())
}
```

### Task 4.3: Add `run_get_parent` to substrate

**Note:** Primitive stores parent in RunMetadata.parent_run.

**Substrate Signature:**
```rust
/// Get parent run
///
/// Returns the parent run ID if this run has a parent.
///
/// ## Parameters
/// - `run`: The run to get parent for
fn run_get_parent(&self, run: &ApiRunId) -> StrataResult<Option<ApiRunId>>;
```

**Implementation:**
```rust
fn run_get_parent(&self, run: &ApiRunId) -> StrataResult<Option<ApiRunId>> {
    let run_str = api_run_id_to_string(run);
    let meta = self.run().get_run(&run_str).map_err(convert_error)?
        .ok_or_else(|| StrataError::not_found(
            strata_core::EntityRef::run(run.to_run_id()),
            "Run not found"
        ))?;

    match meta.value.parent_run {
        Some(parent_name) => Ok(ApiRunId::parse(&parent_name)),
        None => Ok(None),
    }
}
```

---

## Phase 5: Implement Retention (Currently Stubbed)

### Analysis

Retention policy is specified in the contract but not implemented in primitive.

**Options:**
1. **Store in metadata** - Use reserved key `_strata_retention`
2. **Add to primitive** - New field in RunMetadata
3. **Defer** - Keep stubbed, document as not implemented

**Recommendation:** Option 1 (store in metadata) for now. This:
- Requires no primitive changes
- Is backwards compatible
- Can be migrated later

### Reserved Namespace

The key `_strata_retention` is **reserved for system use**. Users should not:
- Read this key directly
- Write to this key manually
- Depend on its format

All keys prefixed with `_strata_` are reserved for future system use.

### Enforcement Semantics

**Critical:** Setting retention does NOT immediately delete data.

Retention is **declarative policy**, not immediate action:
- `run_set_retention` stores the policy
- Enforcement occurs during **storage compaction**
- Timing of enforcement is not guaranteed
- Data may persist beyond policy until next compaction

This prevents surprise data loss and aligns with database compaction semantics.

### Task 5.1: Implement `run_set_retention`

**Substrate Signature:** (already defined)
```rust
fn run_set_retention(&self, run: &ApiRunId, policy: RetentionPolicy) -> StrataResult<Version>;
```

**Implementation:**
```rust
fn run_set_retention(&self, run: &ApiRunId, policy: RetentionPolicy) -> StrataResult<Version> {
    let run_str = api_run_id_to_string(run);

    // Get current metadata
    let meta = self.run().get_run(&run_str).map_err(convert_error)?
        .ok_or_else(|| StrataError::not_found(
            strata_core::EntityRef::run(run.to_run_id()),
            "Run not found"
        ))?;

    // Merge retention into metadata
    let mut obj = match meta.value.metadata {
        Value::Object(o) => o,
        Value::Null => std::collections::HashMap::new(),
        _ => return Err(StrataError::invalid_operation(
            strata_core::EntityRef::run(run.to_run_id()),
            "Run metadata must be object or null"
        )),
    };

    obj.insert("_strata_retention".to_string(), retention_to_value(&policy));

    let versioned = self.run().update_metadata(&run_str, Value::Object(obj))
        .map_err(convert_error)?;
    Ok(versioned.version)
}

fn retention_to_value(policy: &RetentionPolicy) -> Value {
    match policy {
        RetentionPolicy::KeepAll => Value::String("keep_all".into()),
        RetentionPolicy::KeepLatestN(n) => Value::Object(
            [("keep_latest".into(), Value::Int(*n as i64))].into_iter().collect()
        ),
        RetentionPolicy::KeepDuration(d) => Value::Object(
            [("keep_duration_secs".into(), Value::Int(d.as_secs() as i64))].into_iter().collect()
        ),
    }
}
```

### Task 5.2: Implement `run_get_retention`

**Substrate Signature:** (already defined)
```rust
fn run_get_retention(&self, run: &ApiRunId) -> StrataResult<RetentionPolicy>;
```

**Implementation:**
```rust
fn run_get_retention(&self, run: &ApiRunId) -> StrataResult<RetentionPolicy> {
    let run_str = api_run_id_to_string(run);

    let meta = self.run().get_run(&run_str).map_err(convert_error)?
        .ok_or_else(|| StrataError::not_found(
            strata_core::EntityRef::run(run.to_run_id()),
            "Run not found"
        ))?;

    if let Value::Object(obj) = &meta.value.metadata {
        if let Some(retention_val) = obj.get("_strata_retention") {
            return value_to_retention(retention_val);
        }
    }

    Ok(RetentionPolicy::KeepAll) // Default
}

fn value_to_retention(v: &Value) -> StrataResult<RetentionPolicy> {
    match v {
        Value::String(s) if s == "keep_all" => Ok(RetentionPolicy::KeepAll),
        Value::Object(obj) => {
            if let Some(Value::Int(n)) = obj.get("keep_latest") {
                return Ok(RetentionPolicy::KeepLatestN(*n as u64));
            }
            if let Some(Value::Int(secs)) = obj.get("keep_duration_secs") {
                return Ok(RetentionPolicy::KeepDuration(
                    std::time::Duration::from_secs(*secs as u64)
                ));
            }
            Ok(RetentionPolicy::KeepAll)
        }
        _ => Ok(RetentionPolicy::KeepAll),
    }
}
```

---

## Phase 6: Add Helper Conversion Functions

### Task 6.1: Add `metadata_to_versioned_info` helper

```rust
fn metadata_to_versioned_info(meta: RunMetadata) -> Versioned<RunInfo> {
    let info = RunInfo {
        run_id: ApiRunId::parse(&meta.name).unwrap_or_else(ApiRunId::new),
        created_at: (meta.created_at.max(0) as u64).saturating_mul(1000),
        metadata: meta.metadata,
        state: convert_run_status(&meta.status),
        error: meta.error,
    };
    Versioned {
        value: info,
        version: Version::counter(meta.version),
        timestamp: strata_core::Timestamp::from_millis(meta.created_at.max(0) as u64),
    }
}

fn metadata_to_run_info(meta: &RunMetadata) -> RunInfo {
    RunInfo {
        run_id: ApiRunId::parse(&meta.name).unwrap_or_else(ApiRunId::new),
        created_at: (meta.created_at.max(0) as u64).saturating_mul(1000),
        metadata: meta.metadata.clone(),
        state: convert_run_status(&meta.status),
        error: meta.error.clone(),
    }
}
```

---

## Phase 7: Comprehensive Test Suite

### Directory Structure

```
tests/substrate_api_comprehensive/runindex/
├── mod.rs              # Module declaration
├── basic_ops.rs        # Create, get, exists, update_metadata
├── lifecycle.rs        # close, pause, resume, fail, cancel, archive
├── queries.rs          # list, query_by_status, query_by_tag, count, search
├── tags.rs             # add_tags, remove_tags, get_tags
├── hierarchy.rs        # create_child, get_children, get_parent
├── retention.rs        # set_retention, get_retention
├── delete.rs           # delete with cascade verification
├── edge_cases.rs       # Default run protection, invalid transitions
├── concurrency.rs      # Thread safety
└── invariants.rs       # Contract invariants (versioning, state, read, delete, index)
```

### Test Counts by Module

| Module | Tests | Coverage |
|--------|-------|----------|
| basic_ops | 12 | create, get, exists, update |
| lifecycle | 20 | All 6 states, transitions, errors |
| queries | 15 | list, query_by_status, query_by_tag, count, search |
| tags | 10 | add, remove, get, query integration |
| hierarchy | 12 | parent-child relationships |
| retention | 8 | set, get, persistence |
| delete | 10 | cascade to all primitives, scope boundaries |
| edge_cases | 10 | default run, invalid ops |
| concurrency | 5 | thread safety |
| invariants | 12 | contract invariants |
| **Total** | **114** | |

### Task 7.1: Create `mod.rs`

```rust
//! RunIndex Comprehensive Test Suite
//!
//! Tests organized by functionality:
//! - basic_ops: CRUD operations
//! - lifecycle: State transitions
//! - queries: List, query, search
//! - tags: Tag management
//! - hierarchy: Parent-child relationships
//! - retention: Retention policy
//! - delete: Cascading delete
//! - edge_cases: Validation and boundaries
//! - concurrency: Thread safety
//! - invariants: Contract invariants that must always hold

mod basic_ops;
mod lifecycle;
mod queries;
mod tags;
mod hierarchy;
mod retention;
mod delete;
mod edge_cases;
mod concurrency;
```

### Task 7.2: Test Categories

#### basic_ops.rs
```rust
// Create
test_run_create_basic
test_run_create_with_metadata
test_run_create_duplicate_error
test_run_create_generates_uuid

// Get
test_run_get_exists
test_run_get_not_found
test_run_get_default

// Exists
test_run_exists_true
test_run_exists_false
test_run_exists_default_always_true

// Update metadata
test_run_update_metadata
test_run_update_metadata_not_found
```

#### lifecycle.rs
```rust
// State transitions
test_active_to_completed
test_active_to_failed
test_active_to_cancelled
test_active_to_paused
test_active_to_archived
test_paused_to_active
test_paused_to_cancelled
test_paused_to_archived
test_completed_to_archived
test_failed_to_archived
test_cancelled_to_archived

// Invalid transitions
test_completed_cannot_activate (no resurrection)
test_failed_cannot_activate
test_archived_is_terminal
test_paused_cannot_complete

// Error cases
test_close_default_run_error
test_transition_not_found_error
```

#### queries.rs
```rust
test_run_list_all
test_run_list_by_status
test_run_list_limit
test_run_list_offset
test_run_query_by_status_active
test_run_query_by_status_failed
test_run_query_by_tag
test_run_query_by_tag_not_found
test_run_count_all
test_run_count_by_status
test_run_search_by_name
test_run_search_by_metadata
test_run_search_respects_limit
```

#### tags.rs
```rust
test_add_tags
test_add_duplicate_tags_ignored
test_remove_tags
test_remove_nonexistent_tags_ignored
test_get_tags
test_get_tags_empty
test_tags_query_integration
test_tags_persist_across_restart
```

#### hierarchy.rs
```rust
test_create_child
test_create_child_parent_not_found
test_get_children
test_get_children_empty
test_get_parent
test_get_parent_none
test_hierarchy_three_levels
test_delete_parent_orphans_children
```

#### retention.rs
```rust
test_set_retention_keep_all
test_set_retention_keep_latest
test_set_retention_keep_duration
test_get_retention_default
test_get_retention_after_set
test_retention_persists_metadata
test_retention_not_found_error
```

#### delete.rs

**Cascading Delete Scope:**

Cascading delete affects **only entities scoped to the run**:
- KV entries in the run's namespace
- Events in the run's namespace
- State cells in the run's namespace
- JSON documents in the run's namespace

Cascading delete does **NOT**:
- Delete other runs (including children)
- Delete entities in other runs
- Affect global indices (except removing this run from them)
- Propagate to parent runs

```rust
test_delete_run
test_delete_run_not_found
test_delete_default_run_error
test_delete_cascades_kv
test_delete_cascades_events
test_delete_cascades_state
test_delete_cascades_json
test_delete_removes_indices
test_delete_does_not_affect_children  // Children become orphaned
test_delete_does_not_affect_parent
```

#### edge_cases.rs
```rust
test_default_run_always_exists
test_default_run_cannot_close
test_default_run_cannot_delete
test_default_run_cannot_archive
test_invalid_run_id_format
test_empty_run_id_error
test_run_metadata_must_be_object_or_null
test_fail_requires_error_message
test_status_index_consistency
test_version_increments_on_update
```

#### concurrency.rs
```rust
test_concurrent_creates
test_concurrent_status_updates
test_concurrent_tag_modifications
test_read_during_write
test_no_lost_updates
```

#### invariants.rs

**Contract invariants** that must always hold, regardless of feature behavior:

```rust
// Versioning invariants
test_version_increments_monotonically
test_version_never_decreases
test_version_increments_on_any_mutation

// State invariants
test_terminal_states_are_terminal       // Archived cannot transition
test_no_resurrection_from_finished      // Completed/Failed/Cancelled cannot -> Active
test_default_run_is_immortal            // Cannot close/delete/archive default

// Read invariants
test_read_does_not_mutate               // get_run has no side effects
test_list_does_not_mutate               // run_list has no side effects
test_query_does_not_mutate              // queries have no side effects

// Delete invariants
test_delete_removes_addressability      // After delete, get returns None
test_delete_is_permanent                // No way to recover deleted run

// Index invariants
test_status_index_consistent_with_state // Query by status matches actual state
test_tag_index_consistent_with_tags     // Query by tag matches actual tags
```

---

## Phase 8: Update Types if Needed

### Task 8.1: Verify RunInfo has error field

Already present:
```rust
pub struct RunInfo {
    pub run_id: ApiRunId,
    pub created_at: u64,
    pub metadata: Value,
    pub state: RunState,
    pub error: Option<String>,  // ✅ Present
}
```

### Task 8.2: Verify RunState has all 6 states

Already present:
```rust
pub enum RunState {
    Active,
    Completed,
    Failed,
    Cancelled,
    Paused,
    Archived,
}
```

No changes needed to types.

---

## Final API Surface

### Complete RunIndex Trait (24 methods)

```rust
pub trait RunIndex {
    // === CRUD (5 methods) ===
    fn run_create(run_id?, metadata?) -> (RunInfo, Version);
    fn run_get(run) -> Option<Versioned<RunInfo>>;
    fn run_list(state?, limit?, offset?) -> Vec<Versioned<RunInfo>>;
    fn run_exists(run) -> bool;
    fn run_update_metadata(run, metadata) -> Version;

    // === Lifecycle (6 methods) ===
    fn run_close(run) -> Version;      // -> Completed
    fn run_pause(run) -> Version;      // -> Paused
    fn run_resume(run) -> Version;     // -> Active
    fn run_fail(run, error) -> Version; // -> Failed
    fn run_cancel(run) -> Version;     // -> Cancelled  [NEW]
    fn run_archive(run) -> Version;    // -> Archived   [NEW]

    // === Delete (1 method) ===
    fn run_delete(run) -> ();          // Cascading delete

    // === Queries (5 methods) ===
    fn run_query_by_status(state) -> Vec<Versioned<RunInfo>>;
    fn run_query_by_tag(tag) -> Vec<Versioned<RunInfo>>;  [NEW]
    fn run_count(status?) -> u64;                          [NEW]
    fn run_search(query, limit?) -> Vec<Versioned<RunInfo>>; [NEW]

    // === Tags (3 methods) ===
    fn run_add_tags(run, tags) -> Version;     [NEW]
    fn run_remove_tags(run, tags) -> Version;  [NEW]
    fn run_get_tags(run) -> Vec<String>;       [NEW]

    // === Hierarchy (3 methods) ===
    fn run_create_child(parent, metadata?) -> (RunInfo, Version);  [NEW]
    fn run_get_children(parent) -> Vec<Versioned<RunInfo>>;        [NEW]
    fn run_get_parent(run) -> Option<ApiRunId>;                    [NEW]

    // === Retention (2 methods) ===
    fn run_set_retention(run, policy) -> Version;  // [IMPLEMENT]
    fn run_get_retention(run) -> RetentionPolicy;  // [IMPLEMENT]
}
```

---

## Implementation Order

### Phase 1: Lifecycle (Tasks 1.1-1.3)
- Add `run_cancel`
- Add `run_archive`
- Document `run_close` semantics

### Phase 2: Queries (Tasks 2.1-2.3)
- Add `run_query_by_tag`
- Add `run_count`
- Add `run_search`

### Phase 3: Tags (Tasks 3.1-3.3)
- Add `run_add_tags`
- Add `run_remove_tags`
- Add `run_get_tags`

### Phase 4: Hierarchy (Tasks 4.1-4.3)
- Add `run_create_child`
- Add `run_get_children`
- Add `run_get_parent`

### Phase 5: Retention (Tasks 5.1-5.2)
- Implement `run_set_retention`
- Implement `run_get_retention`

### Phase 6: Helpers (Task 6.1)
- Add conversion helper functions

### Phase 7: Tests (Tasks 7.1-7.2)
- Create test directory structure
- Implement 114+ tests

### Phase 8: Types (Tasks 8.1-8.2)
- Verify types (already complete)

---

## Checklist

- [x] Phase 1: Add `run_cancel`, `run_archive`
- [x] Phase 2: Add `run_query_by_tag`, `run_count`, `run_search`
- [x] Phase 3: Add `run_add_tags`, `run_remove_tags`, `run_get_tags`
- [x] Phase 4: Add `run_create_child`, `run_get_children`, `run_get_parent`
- [x] Phase 5: Implement retention (metadata-based)
- [x] Phase 6: Add helper functions (already done in Phase 2)
- [ ] Phase 7: Create comprehensive tests (114+)
- [ ] Phase 8: Verify types
- [ ] Update RUNINDEX_DEFECTS.md to mark items resolved
- [ ] Commit and push

---

## Never Add List

**These features must NEVER be added to RunIndex.** They would violate the "execution index, not execution engine" principle.

| Feature | Why Never |
|---------|-----------|
| `run_execute` | RunIndex tracks runs, doesn't execute them |
| `run_schedule` | Scheduling is orchestration, not indexing |
| `run_retry` | Retry logic belongs in execution layer |
| `run_timeout` | Timeout enforcement is execution, not indexing |
| `run_callback` | Callbacks are execution hooks, not index features |
| `run_inherit_from_parent` | Parent-child is informational, not transactional |
| `run_cascade_state_to_children` | No state propagation between runs |
| `run_wait_for_children` | No cross-run synchronization |
| `run_search_contents` | Search is metadata only, not run-scoped data |
| `run_fork_with_data` | Data copying is not an index operation |
| `run_merge_from` | Data merging is not an index operation |
| `run_gc` | Garbage collection is storage layer, not index |

If someone proposes any of these, the answer is: **"That belongs in a different layer."**

---

## Success Criteria

1. **All 24 substrate methods implemented and tested**
2. **114+ tests passing** across 10 modules
3. **No primitive changes** - all work at substrate layer
4. **Consistent error handling** - all errors go through convert_error
5. **Default run protection** - cannot close/delete/archive default
6. **Cascading delete verified** - delete removes KV, Events, State, JSON
7. **Documentation complete** - all methods have doc comments
8. **State transitions enforced** - matches canonical state table
9. **Parent-child is informational only** - no state inheritance
10. **Search is metadata only** - no run content inspection
11. **Retention is declarative** - no immediate deletion
