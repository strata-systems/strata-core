# RunIndex Defects and Gaps

> Consolidated from architecture review, primitive vs substrate analysis, and workflow/agent lifecycle best practices.
> Source: `crates/api/src/substrate/run.rs` and `crates/primitives/src/run_index.rs`

## Summary

| Category | Count | Priority |
|----------|-------|----------|
| Stubbed APIs | 2 | P0 |
| Lifecycle State Collapse | 1 | P0 |
| Hidden Primitive Features | 8 | P0-P1 |
| Missing Table Stakes APIs | 4 | P1 |
| API Design Issues | 2 | P1 |
| World-Class Features | 4 | P2 |
| **Total Issues** | **21** | |

---

## What is RunIndex?

RunIndex is **first-class run lifecycle management** - tracking runs as entities with explicit lifecycle states, metadata, tags, and parent-child relationships.

**Purpose:**
- Track agent/workflow runs as first-class entities
- Enforce valid status transitions (no resurrection from terminal states)
- Support run metadata, tags, and hierarchical relationships
- Enable querying runs by status, tags, parent
- Support cascading deletion of all run-scoped data

**Key Design:** RunIndex is a global index (uses nil UUID sentinel) that manages ALL runs in the system, not scoped to any particular run.

---

## Current Substrate API (8 methods, 2 stubbed)

```rust
// Working
fn run_create(run_id?, metadata?) -> (ApiRunId, Version);
fn run_get(run) -> Option<Versioned<RunInfo>>;
fn run_list(state?, limit?, offset?) -> Vec<Versioned<RunInfo>>;
fn run_close(run) -> Version;
fn run_update_metadata(run, metadata) -> Version;
fn run_exists(run) -> bool;

// STUBBED (not implemented)
fn run_set_retention(run, policy) -> Version;  // Returns default
fn run_get_retention(run) -> RetentionPolicy;  // Returns KeepAll
```

---

## Part 1: Critical Lifecycle State Collapse (P0)

### Issue: 6 States Collapsed to 2

**Primitive RunStatus (6 states):**
```rust
enum RunStatus {
    Active,     // Currently executing
    Paused,     // Temporarily suspended (can resume)
    Completed,  // Finished successfully
    Failed,     // Finished with error
    Cancelled,  // User-cancelled
    Archived,   // Terminal soft-delete
}
```

**Substrate RunState (2 states):**
```rust
enum RunState {
    Active,  // Maps to: Active + Paused
    Closed,  // Maps to: Completed + Failed + Cancelled + Archived
}
```

**What's Lost:**

| Primitive State | Substrate Mapping | Capability Lost |
|-----------------|-------------------|-----------------|
| `Paused` | `Active` | Cannot distinguish paused vs running |
| `Failed` | `Closed` | Cannot distinguish failure vs success |
| `Cancelled` | `Closed` | Cannot distinguish cancel vs complete |
| `Archived` | `Closed` | Cannot distinguish soft-delete vs finished |

**Impact:**
- Cannot query "show me all failed runs"
- Cannot query "show me paused runs to resume"
- Cannot distinguish why a run ended
- Cannot implement proper error tracking

**Proposed Fix:** Expose full RunStatus enum at substrate:
```rust
enum RunStatus {
    Active,
    Paused,
    Completed,
    Failed,
    Cancelled,
    Archived,
}
```

---

## Part 2: Stubbed APIs (P0)

### Stub 1: `run_set_retention` - Data Retention Policy

**Priority:** P0

**Current State:**
```rust
fn run_set_retention(run, policy) -> StrataResult<Version> {
    // Not implemented - returns default version
    Ok(Version::default())
}
```

**Why Critical:**
- Cannot configure data retention per run
- No way to auto-cleanup old data
- Storage grows unbounded

**Proposed Implementation:**
```rust
enum RetentionPolicy {
    KeepAll,
    KeepLatestN(u64),
    KeepDuration(Duration),
    DeleteAfter(Duration),
}
```

---

### Stub 2: `run_get_retention` - Get Retention Policy

**Priority:** P0 (paired with Stub 1)

**Current State:**
```rust
fn run_get_retention(run) -> StrataResult<RetentionPolicy> {
    Ok(RetentionPolicy::KeepAll)  // Always returns default
}
```

---

## Part 3: Hidden Primitive Features (P0-P1)

### Gap 1: `run_pause` / `run_resume` - Suspend and Resume

**Priority:** P0

**What Primitive Has:**
```rust
fn pause_run(&self, run_id: &RunId) -> Result<Versioned<u64>>;
fn resume_run(&self, run_id: &RunId) -> Result<Versioned<u64>>;
```

**What Substrate Exposes:** Nothing

**Why Critical:**
- Long-running agents need pause/resume
- Cannot suspend execution for user input
- Cannot implement checkpointing

**Proposed Substrate API:**
```rust
fn run_pause(&self, run: &ApiRunId) -> StrataResult<Version>;
fn run_resume(&self, run: &ApiRunId) -> StrataResult<Version>;
```

---

### Gap 2: `run_fail` - Mark Run as Failed with Error

**Priority:** P0

**What Primitive Has:**
```rust
fn fail_run(&self, run_id: &RunId, error: &str) -> Result<Versioned<u64>>;
// Stores error message in RunMetadata.error field
```

**What Substrate Exposes:** Nothing (only `run_close`)

**Why Critical:**
- Cannot record why a run failed
- Cannot distinguish failure from success
- Cannot build error dashboards

**Proposed Substrate API:**
```rust
fn run_fail(&self, run: &ApiRunId, error: &str) -> StrataResult<Version>;
```

---

### Gap 3: `run_cancel` - User-Initiated Cancellation

**Priority:** P1

**What Primitive Has:**
```rust
fn cancel_run(&self, run_id: &RunId) -> Result<Versioned<u64>>;
```

**What Substrate Exposes:** Nothing

**Why Important:**
- User cancellation is different from failure
- Cannot track voluntary vs involuntary termination

**Proposed Substrate API:**
```rust
fn run_cancel(&self, run: &ApiRunId) -> StrataResult<Version>;
```

---

### Gap 4: `run_archive` - Soft Delete

**Priority:** P1

**What Primitive Has:**
```rust
fn archive_run(&self, run_id: &RunId) -> Result<Versioned<u64>>;
// Sets status to Archived (terminal), data preserved
```

**What Substrate Exposes:** Nothing

**Why Important:**
- Soft delete preserves data for compliance
- Archived runs hidden from normal queries
- Different from hard delete

**Proposed Substrate API:**
```rust
fn run_archive(&self, run: &ApiRunId) -> StrataResult<Version>;
```

---

### Gap 5: `run_delete` - Hard Delete with Cascade

**Priority:** P0

**What Primitive Has:**
```rust
fn delete_run(&self, run_id: &RunId) -> Result<()>;
// CASCADING DELETE: Removes run AND all run-scoped data:
// - KV entries
// - Events
// - State cells
// - Traces
```

**What Substrate Exposes:** Nothing

**Why Critical:**
- Cannot clean up old runs
- Cannot comply with data deletion requests (GDPR)
- Storage grows forever

**Proposed Substrate API:**
```rust
fn run_delete(&self, run: &ApiRunId) -> StrataResult<bool>;
// Returns true if deleted, false if not found
// Documents cascading behavior clearly
```

---

### Gap 6: `run_query_by_status` - Status-Based Queries

**Priority:** P0

**What Primitive Has:**
```rust
fn query_by_status(&self, status: RunStatus) -> Result<Vec<RunMetadata>>;
// Uses secondary index for efficient lookups
```

**What Substrate Exposes:** `run_list(state?)` but only Active/Closed

**Why Critical:**
- Cannot query "all failed runs"
- Cannot query "all paused runs"
- Cannot build status dashboards

**Proposed Substrate API:**
```rust
fn run_query_by_status(&self, status: RunStatus) -> StrataResult<Vec<Versioned<RunInfo>>>;
```

---

### Gap 7: `run_query_by_tag` - Tag-Based Queries

**Priority:** P1

**What Primitive Has:**
```rust
fn query_by_tag(&self, tag: &str) -> Result<Vec<RunMetadata>>;
// Uses by-tag secondary index
```

**What Substrate Exposes:** Nothing

**Why Important:**
- Runs have tags but cannot query by them
- Cannot filter runs by project, user, type
- Tags exist but are useless without queries

**Proposed Substrate API:**
```rust
fn run_query_by_tag(&self, tag: &str) -> StrataResult<Vec<Versioned<RunInfo>>>;
```

---

### Gap 8: Parent-Child Run Relationships

**Priority:** P1

**What Primitive Has:**
```rust
fn create_run_with_options(run_id, parent: Option<&str>, tags, metadata) -> ...;
fn get_child_runs(&self, parent_id: &RunId) -> Result<Vec<RunMetadata>>;
// RunMetadata.parent_run tracks hierarchy
```

**What Substrate Exposes:** Nothing

**Why Important:**
- Cannot track forked/nested runs
- Cannot build run hierarchies
- Cannot query "all child runs of X"

**Proposed Substrate API:**
```rust
fn run_create_child(&self, parent: &ApiRunId, metadata: Option<Value>)
    -> StrataResult<(ApiRunId, Version)>;

fn run_get_children(&self, parent: &ApiRunId)
    -> StrataResult<Vec<Versioned<RunInfo>>>;

fn run_get_parent(&self, run: &ApiRunId)
    -> StrataResult<Option<ApiRunId>>;
```

---

## Part 4: Missing Table Stakes APIs (P1)

### Gap 9: Tag Management

**Priority:** P1

**What Primitive Has:**
```rust
fn add_tags(&self, run_id: &RunId, tags: &[&str]) -> Result<Versioned<u64>>;
fn remove_tags(&self, run_id: &RunId, tags: &[&str]) -> Result<Versioned<u64>>;
```

**What Substrate Exposes:** Nothing (tags set at creation only)

**Why Important:**
- Cannot add tags after creation
- Cannot remove/correct tags
- Tags are immutable at substrate level

**Proposed Substrate API:**
```rust
fn run_add_tags(&self, run: &ApiRunId, tags: &[&str]) -> StrataResult<Version>;
fn run_remove_tags(&self, run: &ApiRunId, tags: &[&str]) -> StrataResult<Version>;
fn run_get_tags(&self, run: &ApiRunId) -> StrataResult<Vec<String>>;
```

---

### Gap 10: `run_count` - Run Statistics

**Priority:** P1

**What Primitive Has:**
```rust
fn count(&self) -> Result<usize>;
```

**What Substrate Exposes:** Nothing

**Why Important:**
- Basic monitoring: "How many runs?"
- Pagination: Know total before paginating

**Proposed Substrate API:**
```rust
fn run_count(&self, status: Option<RunStatus>) -> StrataResult<u64>;
```

---

### Gap 11: `run_search` - Full-Text Search

**Priority:** P1

**What Primitive Has:**
```rust
fn search(&self, req: &SearchRequest) -> Result<SearchResponse>;
// Searches run_id, status, tags, metadata
// Respects budget constraints
```

**What Substrate Exposes:** Nothing

**Why Important:**
- Cannot search across run metadata
- Cannot find runs by content

**Proposed Substrate API:**
```rust
fn run_search(&self, query: &str, limit: Option<u64>)
    -> StrataResult<Vec<Versioned<RunInfo>>>;
```

---

### Gap 12: `run_list_all` - List All Run IDs

**Priority:** P1

**What Primitive Has:**
```rust
fn list_runs(&self) -> Result<Vec<RunId>>;
```

**What Substrate Has:** `run_list` returns `RunInfo` (heavier)

**Why Important:**
- Sometimes just need IDs, not full metadata
- More efficient for enumeration

---

## Part 5: API Design Issues (P1)

### Design Issue 1: RunInfo Missing Error Field

**Primitive RunMetadata Has:**
```rust
pub error: Option<String>,  // Error message if failed
```

**Substrate RunInfo:** Does not expose error field

**Impact:** Cannot see why a run failed

---

### Design Issue 2: Timestamps Inconsistent

**Primitive:** `i64` milliseconds since epoch
**Substrate:** Converted to `u64` microseconds

**Impact:** Potential confusion, conversion overhead

---

## Part 6: World-Class Features (P2)

### Gap 13: Run Metrics/Statistics

**Priority:** P2

**Problem:** No aggregate statistics about runs

**Proposed API:**
```rust
struct RunStats {
    total: u64,
    by_status: HashMap<RunStatus, u64>,
    avg_duration_ms: Option<f64>,
    error_rate: f64,
}

fn run_stats(&self) -> StrataResult<RunStats>;
```

---

### Gap 14: Run Duration Tracking

**Priority:** P2

**Problem:** No built-in duration calculation

**Primitive Has:** `created_at` and `completed_at` but no `duration_ms`

**Proposed:** Calculate and expose duration in RunInfo

---

### Gap 15: Run Templates/Presets

**Priority:** P2

**Problem:** Cannot create runs from templates

**Use Case:** "Create a run with these standard tags and metadata"

**Proposed API:**
```rust
fn run_create_from_template(&self, template: &str, overrides: Option<Value>)
    -> StrataResult<(ApiRunId, Version)>;
```

---

### Gap 16: Run Cloning

**Priority:** P2

**Problem:** Cannot clone a run's configuration

**Use Case:** "Create a new run like this one"

**Proposed API:**
```rust
fn run_clone(&self, source: &ApiRunId, new_metadata: Option<Value>)
    -> StrataResult<(ApiRunId, Version)>;
```

---

## Priority Matrix

| ID | Issue | Priority | Effort | Category |
|----|-------|----------|--------|----------|
| Issue 1 | 6→2 state collapse | P0 | Medium | Lifecycle |
| Stub 1 | Retention policy stubbed | P0 | Medium | Stubbed |
| Stub 2 | Get retention stubbed | P0 | Low | Stubbed |
| Gap 1 | Pause/resume hidden | P0 | Low | Hidden |
| Gap 2 | Fail with error hidden | P0 | Low | Hidden |
| Gap 5 | Delete (cascade) hidden | P0 | Low | Hidden |
| Gap 6 | Query by status hidden | P0 | Low | Hidden |
| Gap 3 | Cancel hidden | P1 | Low | Hidden |
| Gap 4 | Archive hidden | P1 | Low | Hidden |
| Gap 7 | Query by tag hidden | P1 | Low | Hidden |
| Gap 8 | Parent-child hidden | P1 | Medium | Hidden |
| Gap 9 | Tag management | P1 | Low | Missing API |
| Gap 10 | Run count | P1 | Low | Missing API |
| Gap 11 | Full-text search | P1 | Low | Missing API |
| Gap 12 | List all IDs | P1 | Low | Missing API |
| Design 1 | Error field missing | P1 | Low | Design |
| Design 2 | Timestamp units | P1 | Low | Design |
| Gap 13 | Statistics | P2 | Medium | World-Class |
| Gap 14 | Duration tracking | P2 | Low | World-Class |
| Gap 15 | Templates | P2 | Medium | World-Class |
| Gap 16 | Cloning | P2 | Low | World-Class |

---

## Recommended Fix Order

### Phase 1: Expose Lifecycle States (Medium Effort)
1. Expose full RunStatus enum (Issue 1) - **CRITICAL**
2. Expose `run_fail` with error (Gap 2)
3. Expose `run_pause` / `run_resume` (Gap 1)
4. Expose `run_cancel` (Gap 3)
5. Expose `run_archive` (Gap 4)
6. Add error field to RunInfo (Design 1)

### Phase 2: Expose Queries (Low Effort)
7. Expose `run_query_by_status` (Gap 6)
8. Expose `run_query_by_tag` (Gap 7)
9. Expose `run_count` (Gap 10)
10. Expose `run_search` (Gap 11)

### Phase 3: Expose Management (Low-Medium Effort)
11. Expose `run_delete` with cascade (Gap 5)
12. Expose tag management (Gap 9)
13. Expose parent-child relationships (Gap 8)
14. Implement retention policies (Stub 1, 2)

### Phase 4: World-Class Features (Medium Effort)
15. Statistics (Gap 13)
16. Duration tracking (Gap 14)
17. Templates (Gap 15)
18. Cloning (Gap 16)

---

## Lifecycle State Transition Diagram

**Primitive (Full):**
```
                    ┌─────────────────────────────────────┐
                    │                                     │
                    v                                     │
[Create] ──> Active <────────> Paused ───────────────────┤
               │                  │                       │
               │                  v                       │
               ├──────────> Cancelled ───────────────────┤
               │                                          │
               ├──────────> Failed ──────────────────────┤
               │                                          │
               v                                          │
           Completed ─────────────────────────────────────┤
                                                          │
                                                          v
                                                      Archived
                                                      (TERMINAL)
```

**Substrate (Collapsed):**
```
[Create] ──> Active ──> Closed
```

---

## Comparison with Industry Standards

| Feature | Strata RunIndex | Temporal | Airflow | Prefect |
|---------|-----------------|----------|---------|---------|
| Create run | ✅ | ✅ | ✅ | ✅ |
| **6+ states** | ❌ (hidden) | ✅ | ✅ | ✅ |
| Pause/Resume | ❌ (hidden) | ✅ | ✅ | ✅ |
| Failed with error | ❌ (hidden) | ✅ | ✅ | ✅ |
| Cancel | ❌ (hidden) | ✅ | ✅ | ✅ |
| Archive | ❌ (hidden) | ✅ | ✅ | ✅ |
| Delete | ❌ (hidden) | ✅ | ✅ | ✅ |
| Query by status | ❌ (hidden) | ✅ | ✅ | ✅ |
| Tags | ❌ (hidden) | ✅ | ✅ | ✅ |
| Parent-child | ❌ (hidden) | ✅ | ✅ | ✅ |
| Retention | ❌ (stubbed) | ✅ | ✅ | ✅ |
| Statistics | ❌ | ✅ | ✅ | ✅ |

**Strata's Reality:** Primitive has all features, substrate hides them.

---

## Key Finding

**RunIndex has a complete, production-ready lifecycle system at the primitive level that is almost entirely hidden at substrate.**

The primitive provides:
- 6 lifecycle states with valid transition enforcement
- Error tracking on failure
- Pause/resume for long-running workflows
- Soft delete (archive) vs hard delete (cascade)
- Tags with indexed queries
- Parent-child relationships
- Full-text search

But substrate collapses everything to:
- 2 states (Active/Closed)
- No error tracking
- No pause/resume
- No deletion
- No tag queries
- No hierarchy

**This is the most severe case of capability hiding across all primitives analyzed.**

**Recommendation:** RunIndex substrate needs the most work of any primitive. The full lifecycle system should be exposed - it's already implemented and tested at the primitive level.
