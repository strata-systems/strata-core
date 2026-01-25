# VectorStore: Complete Architecture & Implementation Plan

> Comprehensive document covering VectorStore as both a user-facing primitive
> and foundational infrastructure for Strata's internal search architecture.

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Dual Purpose: User-Facing & Internal Infrastructure](#part-1-dual-purpose)
3. [Core Invariants](#part-2-core-invariants)
4. [Current State Analysis](#part-3-current-state)
5. [Source Reference Linking](#part-4-source-reference-linking)
6. [Internal vs External Collections](#part-5-internal-vs-external-collections)
7. [Future Graph Integration](#part-6-future-graph-integration)
8. [API Gaps & Implementation Plan](#part-7-api-gaps--implementation-plan)
9. [Implementation Priority](#part-8-implementation-priority)
10. [Design Decisions](#part-9-design-decisions)
11. [Appendices](#appendices)

---

## Executive Summary

VectorStore serves **two distinct purposes**:

| Purpose | Description | Visibility |
|---------|-------------|------------|
| **User-Facing** | Developers store and query their own embeddings | Substrate + Facade |
| **Internal Infrastructure** | Powers semantic search across KV, JSON, EventLog | Primitive only |

**Key Architectural Decisions:**

1. **Source Reference Linking**: Embeddings link back to source documents via `EntityRef`
2. **Internal Collections**: Prefixed with `_`, invisible to substrate/facade APIs
3. **Embedding Generation**: External concern (Python library with HuggingFace, etc.) - not in database primitives

**Implementation Status:**

| Phase | Goal | Status |
|-------|------|--------|
| Phase 0 | Foundation (source refs, internal collections) | ✅ Complete |
| Phase 1 | User API (count, list, budget search) | ✅ Complete |
| Phase 2 | Batch Operations | ✅ Complete |
| Phase 2.5 | WAL Recovery Fix | ✅ Complete |
| Phase 3 | History & Advanced | ✅ Complete |

**Durability:** Vector embeddings survive database restart via WAL recovery.

---

## Part 1: Dual Purpose

### User-Facing: Embedding Storage

Developers need a place to store and query their embeddings:

```python
# User code (future Python SDK)
from strata import Database

db = Database("./my_app")
run = db.create_run()

# Store embeddings
run.vector.add("products", "sku-123", embedding, {"category": "electronics"})

# Query similar items
results = run.vector.search("products", query_embedding, k=10)
```

### Internal Infrastructure: Semantic Search

Strata uses vectors internally to power search across all primitives:

```
┌─────────────────────────────────────────────────────────────────┐
│                     Hybrid Search Flow                           │
│                                                                  │
│   "Find documents about authentication"                          │
│                    │                                             │
│                    ▼                                             │
│   ┌──────────────────────────────────────┐                      │
│   │  1. External: Generate Query Embedding│  ← Python/HuggingFace│
│   └──────────────────┬───────────────────┘                      │
│                      │                                           │
│                      ▼                                           │
│   ┌──────────────────────────────────────┐                      │
│   │  2. Vector Search (internal coll)     │                      │
│   │     "_json_embeddings" collection     │                      │
│   │     Returns: [(EntityRef, score)]     │                      │
│   └──────────────────┬───────────────────┘                      │
│                      │                                           │
│                      ▼                                           │
│   ┌──────────────────────────────────────┐                      │
│   │  3. Hydrate from Source Primitives    │                      │
│   │     EntityRef::Json → JsonStore.get() │                      │
│   │     EntityRef::Kv → KVStore.get()     │                      │
│   └──────────────────┬───────────────────┘                      │
│                      │                                           │
│                      ▼                                           │
│   Rich SearchResponse with actual documents                      │
└─────────────────────────────────────────────────────────────────┘
```

### Embedding Generation is External

**Critical Design Decision**: Embedding generation is **not** a database concern.

The database primitives:
- ✅ Store embeddings
- ✅ Index embeddings
- ✅ Search embeddings
- ❌ Generate embeddings ← External

**Future Python SDK** (separate project):
```python
from strata import Database
from strata.embeddings import HuggingFaceEmbedder

# Embedding generation is SDK concern, not database concern
embedder = HuggingFaceEmbedder("sentence-transformers/all-MiniLM-L6-v2")

db = Database("./my_app")
run = db.create_run()

# SDK generates embedding, database stores it
text = "Product description here"
embedding = embedder.encode(text)
run.vector.add("products", "sku-123", embedding, {"text": text})
```

---

## Part 2: Core Invariants

These invariants must not be violated. They ensure VectorStore behaves consistently with other primitives.

### Invariant V1: Version Increments on Any Mutation

**A vector version increments on any mutation, including re-upsert of the same embedding or metadata.**

```rust
pub version: u64  // Increments on EVERY write, even if content unchanged
```

This matches other mutable primitives (KV, JSON, StateCell):
- Keeps version semantics uniform across the system
- Avoids "did this really change?" ambiguity
- Essential for history APIs (when implemented)
- Enables optimistic concurrency control

### Invariant V2: Internal Collections Are Per-Run

**Internal collections like `_json_embeddings` are scoped to runs, not global.**

```
Run A: _json_embeddings → [embeddings for run A's JSON documents]
Run B: _json_embeddings → [embeddings for run B's JSON documents]
```

This is consistent with the Primitive Contract:
- Runs are commits, not tables
- Cross-run data sharing violates isolation guarantees
- Each run has its own semantic index

**Why not one global semantic index?**
Because runs are independent execution contexts. A global index would:
- Leak information between runs
- Create consistency nightmares during run lifecycle
- Violate the fundamental run isolation principle

### Invariant V3: VectorStore Returns Vector-Native Results

**VectorStore returns vector-native results (`VectorMatch`, `VectorMatchWithSource`). Search orchestration builds `SearchResponse`.**

```rust
// VectorStore primitive returns:
pub struct VectorMatchWithSource {
    pub key: String,
    pub score: f32,
    pub metadata: Option<JsonValue>,
    pub source_ref: Option<EntityRef>,
    pub version: u64,
}

// Search orchestrator (HybridSearch) converts to:
pub struct SearchResponse {
    pub hits: Vec<SearchHit>,
    pub truncated: bool,
    pub stats: SearchStats,
}
```

VectorStore must **never** emit `SearchResponse` directly. The orchestrator owns result assembly.

This separation ensures:
- VectorStore remains a pure storage primitive
- Search orchestration can evolve independently
- Result formats can be unified across primitives at the orchestration layer

### Invariant V4: Embedding Storage, Not Computation

**Strata stores meaning, it does not compute it.**

The database:
- ✅ Stores embeddings (accepts `Vec<f32>`)
- ✅ Indexes embeddings (for fast search)
- ✅ Searches embeddings (KNN, filtered)
- ❌ Generates embeddings (external concern)

Embedding generation belongs in:
- Python SDK with HuggingFace/OpenAI
- User application code
- External embedding services

This avoids dependency hell, model churn, and inflexible APIs.

---

## Part 3: Current State

### Existing Search Infrastructure

Strata already has substantial search infrastructure:

| Component | Location | Purpose |
|-----------|----------|---------|
| `EntityRef` | `crates/core/src/contract/entity_ref.rs` | Universal entity addressing |
| `SearchRequest` | `crates/core/src/search_types.rs` | Query with mode, budget, filters |
| `SearchResponse` | `crates/core/src/search_types.rs` | Ranked hits with stats |
| `Searchable` trait | `crates/primitives/src/searchable.rs` | Standard interface for all primitives |
| `HybridSearch` | `crates/search/src/hybrid.rs` | Orchestrates cross-primitive search |
| `SearchBudget` | `crates/core/src/search_types.rs` | Time/candidate limits |

### Primitive Layer (`crates/primitives/src/vector/store.rs`)

| Method | Purpose | Exposed at Substrate? |
|--------|---------|----------------------|
| `create_collection` | Create collection | ✅ Yes |
| `delete_collection` | Delete collection | ✅ Yes |
| `list_collections` | List all collections | ✅ Yes |
| `get_collection` | Get collection info | ✅ Yes (partial) |
| `collection_exists` | Check existence | ✅ Yes |
| `insert` | Upsert vector | ✅ Yes |
| `get` | Get vector | ✅ Yes |
| `delete` | Delete vector | ✅ Yes |
| `count` | Get vector count | ❌ Only via collection_info |
| `search` | KNN search | ✅ Yes |
| `search_with_budget` | Budget-limited search | ❌ **No** |
| `search_response` | Search → SearchResponse | ❌ No |

### Substrate Layer (`crates/api/src/substrate/vector.rs`)

| Method | Status | Notes |
|--------|--------|-------|
| `vector_upsert` | ✅ Works | |
| `vector_get` | ✅ Works | |
| `vector_delete` | ✅ Works | |
| `vector_search` | ⚠️ Partial | Metric param ignored |
| `vector_collection_info` | ⚠️ Partial | Returns tuple, not struct |
| `vector_create_collection` | ✅ Works | |
| `vector_drop_collection` | ✅ Works | |
| `vector_list_collections` | ✅ Works | |
| `vector_collection_exists` | ✅ Works | |

### Facade Layer (`crates/api/src/facade/vector.rs`)

| Method | Status | Notes |
|--------|--------|-------|
| `vadd` | ✅ Works | |
| `vget` | ⚠️ Partial | No versioned variant |
| `vdel` | ✅ Works | |
| `vsim` | ✅ Works | |
| `vsim_with_options` | ⚠️ Limited | Only equality filter |
| `vcollection_info` | ✅ Works | |
| `vcollection_drop` | ✅ Works | |

---

## Part 4: Source Reference Linking

### The Problem: Orphaned Embeddings

Today, embeddings are disconnected from their source data:

```rust
// No formal link between these!
vector_store.insert(run_id, "embeddings", "doc:123", embedding, metadata);
json_store.set(run_id, doc_id, json_document);
```

When vector search returns `"doc:123"`, we cannot:
1. Know this embedding came from a JSON document
2. Hydrate the actual document
3. Return rich results with both similarity score AND document content

### The Solution: Source Reference Field

Add optional source reference to vector entries:

```rust
// In crates/core/src/primitives/vector.rs

pub struct VectorEntry {
    pub key: String,
    pub embedding: Vec<f32>,
    pub metadata: Option<JsonValue>,
    pub vector_id: VectorId,
    pub version: u64,

    // NEW: Link to source document
    pub source_ref: Option<EntityRef>,
}
```

### API Changes

**Primitive Layer:**

```rust
// New method alongside existing insert()
pub fn insert_with_source(
    &self,
    run_id: RunId,
    collection: &str,
    key: &str,
    embedding: &[f32],
    metadata: Option<JsonValue>,
    source_ref: Option<EntityRef>,
) -> VectorResult<Version>;

// New search method that returns source refs
pub fn search_with_sources(
    &self,
    run_id: RunId,
    collection: &str,
    query: &[f32],
    k: usize,
    filter: Option<MetadataFilter>,
) -> VectorResult<Vec<VectorMatchWithSource>>;

#[derive(Debug, Clone)]
pub struct VectorMatchWithSource {
    pub key: String,
    pub score: f32,
    pub metadata: Option<JsonValue>,
    pub source_ref: Option<EntityRef>,
    pub version: u64,
}
```

**Substrate Layer:**

```rust
// New method (user-facing - source_ref optional)
fn vector_upsert_with_source(
    &self,
    run: &ApiRunId,
    collection: &str,
    key: &str,
    vector: &[f32],
    metadata: Option<Value>,
    source_ref: Option<EntityRef>,
) -> StrataResult<Version>;
```

### Storage Changes

**VectorRecord** (MessagePack serialized):

```rust
// In crates/primitives/src/vector/types.rs

pub struct VectorRecord {
    pub vector_id: u64,
    pub metadata: Option<JsonValue>,
    pub version: u64,
    pub created_at: u64,
    pub updated_at: u64,
    pub source_ref: Option<EntityRef>,  // NEW
}
```

### Usage in HybridSearch

```rust
// In crates/search/src/hybrid.rs
fn search_semantic(&self, req: &SearchRequest, embedding: &[f32]) -> Result<Vec<SearchHit>> {
    let matches = self.vector.search_with_sources(
        req.run_id,
        "_json_embeddings",  // Internal collection
        embedding,
        req.k,
        None,
    )?;

    // Convert to SearchHit using source_ref
    matches.into_iter().map(|m| {
        SearchHit {
            doc_ref: m.source_ref.unwrap_or_else(||
                EntityRef::vector(req.run_id, "_json_embeddings", &m.key)
            ),
            score: m.score,
            rank: 0,  // Set by fuser
            snippet: None,
        }
    }).collect()
}
```

---

## Part 5: Internal vs External Collections

### Collection Naming Convention

| Prefix | Visibility | Purpose |
|--------|------------|---------|
| `_` (underscore) | Internal only | Search infrastructure |
| No prefix | User-visible | User embeddings |

**Reserved Internal Collections:**

```rust
const INTERNAL_KV_EMBEDDINGS: &str = "_kv_embeddings";
const INTERNAL_JSON_EMBEDDINGS: &str = "_json_embeddings";
const INTERNAL_EVENT_EMBEDDINGS: &str = "_event_embeddings";
```

### Substrate Layer Enforcement

Internal collections are **invisible** to substrate/facade APIs:

```rust
// In crates/api/src/substrate/vector.rs

fn is_internal_collection(name: &str) -> bool {
    name.starts_with('_')
}

impl VectorStore for SubstrateImpl {
    fn vector_upsert(&self, run: &ApiRunId, collection: &str, ...) -> StrataResult<Version> {
        // Block writes to internal collections
        if is_internal_collection(collection) {
            return Err(StrataError::forbidden(
                "Cannot access internal collection"
            ));
        }
        // ... existing logic
    }

    fn vector_list_collections(&self, run: &ApiRunId) -> StrataResult<Vec<VectorCollectionInfo>> {
        let run_id = run.to_run_id();
        let collections = self.vector().list_collections(run_id)?;

        // Filter out internal collections
        Ok(collections
            .into_iter()
            .filter(|c| !is_internal_collection(&c.name))
            .map(|c| VectorCollectionInfo { ... })
            .collect())
    }
}
```

### Primitive Layer Access

Internal collections accessible only at primitive layer:

```rust
// In crates/primitives/src/vector/store.rs

impl VectorStore {
    /// Internal insert - used by search infrastructure.
    /// Not exposed at substrate/facade layers.
    ///
    /// # Safety
    ///
    /// This method bypasses substrate-level visibility rules and must only be
    /// used by internal search infrastructure. It is intentionally `pub(crate)`
    /// to prevent external access while allowing internal orchestrators like
    /// `HybridSearch` to index embeddings for semantic search.
    ///
    /// **DO NOT** expose this method at the substrate or facade layers.
    /// Users must go through standard `insert()` which enforces collection naming rules.
    pub(crate) fn internal_insert(
        &self,
        run_id: RunId,
        collection: &str,  // Must start with _
        key: &str,
        embedding: &[f32],
        source_ref: EntityRef,
    ) -> VectorResult<Version> {
        debug_assert!(collection.starts_with('_'), "Internal collections must start with _");
        self.insert_with_source(run_id, collection, key, embedding, None, Some(source_ref))
    }
}
```

---

## Part 6: Future Graph Integration

### Graph Primitive (Not Exposed to Users)

The future graph primitive will be **internal only** - not exposed to users:

```rust
// Future: crates/primitives/src/graph/store.rs

pub struct GraphStore {
    db: Arc<Database>,
}

pub struct GraphEdge {
    pub from: EntityRef,      // e.g., EntityRef::Json { run_id, doc_id }
    pub to: EntityRef,        // e.g., EntityRef::Kv { run_id, key }
    pub edge_type: String,    // "CITES", "SIMILAR_TO", "REFERENCES"
    pub weight: f32,
    pub metadata: Option<JsonValue>,
}

impl GraphStore {
    /// Add edges (internal use only)
    pub(crate) fn add_edges(&self, run_id: RunId, edges: Vec<GraphEdge>) -> GraphResult<()>;

    /// Traverse from entity
    pub(crate) fn traverse(
        &self,
        run_id: RunId,
        start: EntityRef,
        edge_types: &[&str],
        max_depth: usize,
    ) -> GraphResult<Vec<(EntityRef, f32)>>;  // (entity, path_weight)

    /// Get neighbors
    pub(crate) fn neighbors(
        &self,
        run_id: RunId,
        entity: EntityRef,
        edge_type: Option<&str>,
    ) -> GraphResult<Vec<(EntityRef, GraphEdge)>>;
}
```

### Vector + Graph Synergy

```
┌─────────────────────────────────────────────────────────────────┐
│                    Graph + Vector Synergy                        │
│                                                                  │
│   Vector: "What's semantically similar?" (continuous)            │
│   Graph:  "What's structurally related?" (discrete)              │
│                                                                  │
│   ┌─────────────────────────────────────────────────────────┐   │
│   │                     Knowledge Graph                      │   │
│   │                                                          │   │
│   │   (doc:1) ──CITES──▶ (doc:5)                            │   │
│   │      │                   │                               │   │
│   │      │ SIMILAR_TO        │ SIMILAR_TO                    │   │
│   │      │ (from vectors)    │ (from vectors)                │   │
│   │      ▼                   ▼                               │   │
│   │   (doc:2) ◀──AUTHORED_BY── (user:alice)                 │   │
│   │                                                          │   │
│   └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│   Example Query: "docs by Alice about authentication"            │
│   1. Keyword: "auth" → [doc:1, doc:3]                           │
│   2. Graph: user:alice --AUTHORED_BY→ [doc:2, doc:4]            │
│   3. Vector: similar to query embedding → [doc:1, doc:5]        │
│   4. Fuse: rank by combined relevance                           │
└─────────────────────────────────────────────────────────────────┘
```

### Similarity Edges from Vectors

Graph stores "SIMILAR_TO" edges computed from vector search:

```rust
// In search orchestrator (future)
fn compute_similarity_edges(
    &self,
    run_id: RunId,
    entity: EntityRef,
    embedding: &[f32],
    threshold: f32,
) -> Result<Vec<GraphEdge>> {
    let similar = self.vector.search_with_sources(
        run_id,
        "_all_embeddings",
        embedding,
        k: 10,
        None,
    )?;

    similar
        .into_iter()
        .filter(|m| m.score >= threshold && m.source_ref.is_some())
        .map(|m| GraphEdge {
            from: entity.clone(),
            to: m.source_ref.unwrap(),
            edge_type: "SIMILAR_TO".to_string(),
            weight: m.score,
            metadata: None,
        })
        .collect()
}
```

---

## Part 7: API Gaps & Implementation Plan

### P0: Critical (Source Reference)

| Item | Layer | Effort | Description |
|------|-------|--------|-------------|
| Add `source_ref` to `VectorEntry` | Core | Low | Optional field linking to source |
| Add `source_ref` to `VectorRecord` | Primitive | Low | Serialization support |
| Add `insert_with_source()` | Primitive | Low | Insert with source link |
| Add `search_with_sources()` | Primitive | Medium | Search returning source refs |
| Update WAL serialization | Primitive | Low | Include source_ref in WAL entries |

### P1: Missing Core APIs

| Item | Layer | Effort | Description |
|------|-------|--------|-------------|
| Hide internal collections | Substrate | Low | Filter `_` prefixed collections |
| Add `vector_count` | Substrate | Low | Direct count method |
| Expose `search_with_budget` | Substrate | Medium | Budget-limited search |
| Return struct for collection_info | Substrate | Medium | `VectorCollectionInfo` instead of tuple |
| Add `vcollection_list` | Facade | Low | List user collections |
| Add `vgetv` (versioned) | Facade | Low | Get with version info |
| Remove/document metric param | Substrate | Low | Currently ignored |

### P1: Batch Operations

| Item | Layer | Effort | Description |
|------|-------|--------|-------------|
| `insert_batch` | Primitive | Medium | Batch insert |
| `get_batch` | Primitive | Low | Batch get |
| `delete_batch` | Primitive | Low | Batch delete |
| Substrate wrappers | Substrate | Medium | Expose batch ops |
| `VectorFacadeBatch` trait | Facade | Medium | User-facing batch ops |

### P1: List/Scan

| Item | Layer | Effort | Description |
|------|-------|--------|-------------|
| `list_keys` | Primitive | Medium | Enumerate keys with pagination |
| `scan` | Primitive | Medium | Scan with cursor |
| Substrate wrappers | Substrate | Low | Expose list/scan |

### P2: History APIs

| Item | Layer | Effort | Description |
|------|-------|--------|-------------|
| Version chain storage | Primitive | High | Store vector history |
| `history()` method | Primitive | Medium | Get version history |
| `get_at()` method | Primitive | Low | Get at specific version |
| Substrate wrappers | Substrate | Low | Expose history |

### P2: Advanced Features

| Item | Effort | Description |
|------|--------|-------------|
| Multi-vector search | Medium | Search with multiple query vectors |
| Metadata-only update | Low | Update metadata without re-uploading vector |
| Enhanced search options | Medium | More filter types, pagination |

### P3: Performance (Future)

| Item | Effort | Description |
|------|--------|-------------|
| HNSW indexing | Very High | O(log n) approximate search |
| Quantization (F16/Int8) | High | Memory reduction |
| Parallel search | Medium | Multi-core utilization |

---

## Part 8: Implementation Priority

### Phase 0: Foundation ✅ COMPLETE

**Goal**: Enable internal search infrastructure

| Item | Effort | Files | Status |
|------|--------|-------|--------|
| Add `source_ref` to `VectorEntry` | 1h | `crates/core/src/primitives/vector.rs` | ✅ |
| Add `source_ref` to `VectorRecord` | 1h | `crates/primitives/src/vector/types.rs` | ✅ |
| Add `insert_with_source()` | 2h | `crates/primitives/src/vector/store.rs` | ✅ |
| Add `search_with_sources()` | 3h | `crates/primitives/src/vector/store.rs` | ✅ |
| Update WAL entries | 2h | `crates/primitives/src/vector/wal.rs` | ✅ |
| Hide internal collections | 1h | `crates/api/src/substrate/vector.rs` | ✅ |

**Total: ~10 hours**

### Phase 1: User API Improvements ✅ COMPLETE

**Goal**: Complete user-facing API

| Item | Effort | Files | Status |
|------|--------|-------|--------|
| Add `vector_count` | 1h | Substrate | ✅ |
| Add `vcollection_list` | 30m | Facade | ✅ |
| Add `vgetv` | 30m | Facade | ✅ |
| Expose `search_with_budget` | 2h | Substrate + Facade | ✅ |
| Collection info returns struct | 2h | Substrate + Facade | ✅ |

**Total: ~6 hours**

### Phase 2: Batch Operations ✅ COMPLETE

**Goal**: Efficient bulk operations

| Item | Effort | Status |
|------|--------|--------|
| Primitive batch methods (`insert_batch`, `get_batch`, `delete_batch`) | 6h | ✅ |
| Substrate wrappers (`vector_upsert_batch`, `vector_get_batch`, `vector_delete_batch`) | 3h | ✅ |
| Facade trait (`vadd_batch`, `vget_batch`, `vdel_batch`) | 2h | ✅ |
| Tests (19 new batch tests) | 3h | ✅ |

**Total: ~14 hours**

### Phase 2.5: WAL Recovery Fix ✅ COMPLETE

**Goal**: Ensure vectors survive database restart

**Root Cause**: The `substrate_api_comprehensive` tests were not calling
`register_vector_recovery()` before opening the database. The recovery
participant was never registered, so embeddings weren't restored.

**Fix**: Added `register_vector_recovery()` call to `create_persistent_db()`
in test setup. The existing recovery code in `recovery.rs` was already correct.

| Item | Effort | Status |
|------|--------|--------|
| Register vector recovery in test setup | 30m | ✅ |
| Un-ignore and verify 6 durability tests | 30m | ✅ |

**Total: ~1 hour** (simpler than estimated - recovery code was already working)

**Tests Enabled (all pass):**
- `test_vector_persist_after_restart` ✅
- `test_vector_metadata_persist` ✅
- `test_vector_delete_persist` ✅
- `test_vector_run_isolation_persists` ✅
- `test_vector_update_persist` ✅
- `test_vector_search_after_restart` ✅

### Phase 3: History & Advanced ✅ COMPLETE

**Goal**: Full feature parity with other primitives

| Item | Effort | Status |
|------|--------|--------|
| Add embedding to VectorRecord for history support | 2h | ✅ |
| History at primitive (`history()`, `get_at()`) | 4h | ✅ |
| History at substrate (`vector_history()`, `vector_get_at()`) | 2h | ✅ |
| History at facade (`vhistory()`) | 1h | ✅ |
| List/scan at primitive (`list_keys()`, `scan()`) | 3h | ✅ |
| List/scan at substrate (`vector_list_keys()`, `vector_scan()`) | 1h | ✅ |
| List/scan at facade (`vlist()`, `vscan()`) | 1h | ✅ |
| Comprehensive tests (18 new tests in history.rs) | 3h | ✅ |

**Total: ~17 hours**

**Implementation Notes:**
- VectorRecord now stores the embedding alongside metadata for history support
- Embeddings are stored in versioned KV storage, enabling `storage.get_history()` to work
- The in-memory backend still stores embeddings for fast search operations
- History returns complete vector snapshots including embedding data
- List/scan operations use `scan_prefix()` from storage layer

---

## Part 9: Design Decisions

### Decision 1: Source Ref is Optional

**Rationale**:
- User-facing VectorStore doesn't need source refs
- Only internal search infrastructure uses them
- Backwards compatible - existing code unaffected

### Decision 2: Internal Collections Prefixed with `_`

**Rationale**:
- Simple convention (like `_id` in MongoDB)
- Easy to check programmatically
- User collections can't accidentally conflict
- Invisible at substrate/facade layer

### Decision 3: Embedding Generation is External

**Rationale**:
- Primitives are storage, not computation
- Embedding models change rapidly (don't couple to database)
- Python SDK is better place for ML integrations
- Keeps Rust codebase focused on storage/retrieval

### Decision 4: Graph Primitive is Internal Only

**Rationale**:
- Graph is for search infrastructure, not user data
- Simpler API surface for users
- Can evolve without breaking changes
- "SIMILAR_TO" edges computed from vectors

### Decision 5: EntityRef as Universal Link

**Rationale**:
- Already exists and covers all primitives
- Serializable, hashable, comparable
- Graph edges naturally use same type
- Single addressing scheme for entire system

---

## Appendices

### Appendix A: Type Changes Summary

**VectorEntry** (core):
```rust
pub struct VectorEntry {
    pub key: String,
    pub embedding: Vec<f32>,
    pub metadata: Option<JsonValue>,
    pub vector_id: VectorId,
    pub version: u64,
    pub source_ref: Option<EntityRef>,  // NEW
}
```

**VectorRecord** (primitive):
```rust
pub struct VectorRecord {
    pub vector_id: u64,
    pub metadata: Option<JsonValue>,
    pub version: u64,
    pub created_at: u64,
    pub updated_at: u64,
    pub source_ref: Option<EntityRef>,  // NEW
}
```

**VectorMatchWithSource** (new type):
```rust
pub struct VectorMatchWithSource {
    pub key: String,
    pub score: f32,
    pub metadata: Option<JsonValue>,
    pub source_ref: Option<EntityRef>,
    pub version: u64,
}
```

### Appendix B: File Locations

| Component | File |
|-----------|------|
| EntityRef | `crates/core/src/contract/entity_ref.rs` |
| VectorEntry | `crates/core/src/primitives/vector.rs` |
| VectorRecord | `crates/primitives/src/vector/types.rs` |
| VectorStore (primitive) | `crates/primitives/src/vector/store.rs` |
| VectorStore (substrate) | `crates/api/src/substrate/vector.rs` |
| VectorFacade | `crates/api/src/facade/vector.rs` |
| HybridSearch | `crates/search/src/hybrid.rs` |
| Searchable trait | `crates/primitives/src/searchable.rs` |
| SearchRequest | `crates/core/src/search_types.rs` |

### Appendix C: Test Plan

**Phase 0 Tests:**
```rust
#[test]
fn test_insert_with_source_ref() {
    let store = setup_vector_store();
    let run_id = RunId::new();

    let source = EntityRef::kv(run_id, "my-document");
    store.insert_with_source(
        run_id, "embeddings", "emb:my-document",
        &[0.1, 0.2, 0.3], None, Some(source.clone()),
    ).unwrap();

    let entry = store.get(run_id, "embeddings", "emb:my-document").unwrap().unwrap();
    assert_eq!(entry.value.source_ref, Some(source));
}

#[test]
fn test_search_with_sources_returns_refs() {
    let store = setup_vector_store();
    let run_id = RunId::new();

    let source = EntityRef::json(run_id, JsonDocId::new());
    store.insert_with_source(
        run_id, "test", "key1", &[1.0, 0.0], None, Some(source.clone())
    ).unwrap();

    let results = store.search_with_sources(run_id, "test", &[1.0, 0.0], 10, None).unwrap();
    assert_eq!(results[0].source_ref, Some(source));
}

#[test]
fn test_internal_collections_hidden_from_substrate() {
    // Create internal collection at primitive level
    primitive.create_collection(run_id, "_internal", config).unwrap();
    primitive.create_collection(run_id, "user_collection", config).unwrap();

    // Substrate should only see user collection
    let collections = substrate.vector_list_collections(&api_run).unwrap();
    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0].name, "user_collection");
}

#[test]
fn test_substrate_blocks_internal_collection_access() {
    let result = substrate.vector_upsert(
        &api_run, "_internal", "key", &[1.0], None
    );
    assert!(matches!(result, Err(StrataError::Forbidden { .. })));
}
```

### Appendix D: Migration Notes

**Backwards Compatibility:**
- `source_ref` is optional - existing code works unchanged
- `insert()` still works, just doesn't set source_ref
- `search()` still works, use `search_with_sources()` for source refs

**WAL Compatibility:**
- New WAL entries include source_ref field
- Old WAL entries read as source_ref = None
- No migration needed for existing data

### Appendix E: Future Python SDK (Out of Scope)

The Python SDK for embedding generation is a **separate project**, not part of the database:

```python
# Future: strata-python package

from strata import Database
from strata.embeddings import (
    HuggingFaceEmbedder,
    OpenAIEmbedder,
    CohereEmbedder,
)

# Choose your embedding provider
embedder = HuggingFaceEmbedder("sentence-transformers/all-MiniLM-L6-v2")
# or
embedder = OpenAIEmbedder(api_key="...")

# Database knows nothing about embeddings
db = Database("./my_app")
run = db.create_run()

# SDK generates embedding, database stores it
def index_document(doc_id: str, text: str):
    embedding = embedder.encode(text)
    run.vector.add("documents", doc_id, embedding, {"text": text})

# Search
def search(query: str, k: int = 10):
    query_embedding = embedder.encode(query)
    return run.vector.search("documents", query_embedding, k)
```

This separation ensures:
1. Database remains a pure storage system
2. Users can choose any embedding provider
3. Embedding models can be updated independently
4. No ML dependencies in core database
