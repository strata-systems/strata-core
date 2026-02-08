# Vector Primitive - Architecture Reference

## Overview

The Vector primitive provides semantic search over embedding vectors organized into named collections. Each collection has a fixed dimension and distance metric. Vectors are stored in both a persistent KV layer (for durability and metadata) and an in-memory index backend (for search performance).

- **Version semantics**: `Version::Counter(u64)` - per-vector counter starting at 1, incremented on upsert
- **Key construction**:
  - Vector entry: `Key { namespace: Namespace::for_branch(branch_id), type_tag: TypeTag::Vector (0x10), user_key: "collection/key".as_bytes() }`
  - Collection config: `Key { ..., type_tag: TypeTag::VectorConfig (0x12), user_key: collection_name.as_bytes() }`
- **Storage format**: `Value::Bytes(MessagePack)` wrapping `VectorRecord` or `CollectionRecord`
- **Transactional**: No - vector operations bypass the Session transaction layer entirely

## Layer Architecture

```
+------------------------------------------------------------------+
|  CLIENT                                                          |
|  Command::VectorUpsert { branch, collection, key, vector, meta } |
+------------------------------------------------------------------+
        |
        v
+------------------------------------------------------------------+
|  SESSION (session.rs)                                            |
|  ALWAYS routes to executor (non-transactional)                   |
|  Even if a transaction is active, vectors bypass it              |
+------------------------------------------------------------------+
        |
        v
+------------------------------------------------------------------+
|  EXECUTOR (executor.rs)                                          |
|  Dispatches to: crate::handlers::vector::vector_upsert(...)      |
+------------------------------------------------------------------+
        |
        v
+------------------------------------------------------------------+
|  HANDLER (handlers/vector.rs + bridge.rs)                        |
|  1. to_core_branch_id(&branch) -> core::BranchId                |
|  2. validate_not_internal_collection(&collection)                |
|  3. Auto-create collection if not exists                         |
|  4. Convert metadata: Value -> serde_json::Value                 |
|  5. Call primitives.vector.insert(...)                            |
|  6. Return Output::Version(u64)                                  |
+------------------------------------------------------------------+
        |
        v
+------------------------------------------------------------------+
|  ENGINE PRIMITIVE (primitives/vector/store.rs - VectorStore)     |
|  Dual storage:                                                   |
|  1. KV layer: MessagePack-encoded VectorRecord -> persistence    |
|  2. In-memory backend: raw f32 embeddings -> search performance  |
|                                                                  |
|  For insert:                                                     |
|  - Validate dimension matches collection config                  |
|  - Allocate VectorId (monotonic per collection)                  |
|  - Insert into in-memory backend                                 |
|  - Serialize VectorRecord to MessagePack                         |
|  - Write to KV via db.transaction()                              |
+------------------------------------------------------------------+
        |
        v
+------------------------------------------------------------------+
|  VECTOR INDEX BACKEND (pluggable)                                |
|                                                                  |
|  BruteForceBackend (brute_force.rs):                             |
|  - VectorHeap: BTreeMap<VectorId, offset>                        |
|  - O(n) search: compute similarity for all vectors               |
|  - Sort by (score desc, VectorId asc)                            |
|                                                                  |
|  HnswBackend (hnsw.rs):                                         |
|  - Multi-layer navigable small world graph                       |
|  - O(log n) approximate nearest neighbor search                  |
|  - Greedy descent through layers + beam search at layer 0        |
|  - BTreeMap<VectorId, HnswNode> for deterministic iteration      |
+------------------------------------------------------------------+
        |
        v
+------------------------------------------------------------------+
|  STORAGE (storage/sharded.rs)                                    |
|  Persistent storage for VectorRecord and CollectionRecord        |
|  Used for durability and restart recovery                        |
+------------------------------------------------------------------+
```

## Index Backends

### VectorIndexBackend Trait

All backends implement this trait (`backend.rs`):

```rust
pub trait VectorIndexBackend: Send + Sync {
    fn allocate_id(&mut self) -> VectorId;
    fn insert(&mut self, id: VectorId, embedding: &[f32]) -> Result<(), VectorError>;
    fn insert_with_id(&mut self, id: VectorId, embedding: &[f32]) -> Result<(), VectorError>;
    fn delete(&mut self, id: VectorId) -> Result<bool, VectorError>;
    fn search(&self, query: &[f32], k: usize) -> Vec<(VectorId, f32)>;
    fn len(&self) -> usize;
    fn dimension(&self) -> usize;
    fn metric(&self) -> DistanceMetric;
    fn config(&self) -> VectorConfig;
    fn get(&self, id: VectorId) -> Option<&[f32]>;
    fn contains(&self, id: VectorId) -> bool;
    fn index_type_name(&self) -> &'static str;    // "brute_force" or "hnsw"
    fn memory_usage(&self) -> usize;               // Approximate bytes
    fn rebuild_index(&mut self);                    // Post-recovery graph reconstruction
    fn vector_ids(&self) -> Vec<VectorId>;
    fn snapshot_state(&self) -> (u64, Vec<usize>);
    fn restore_snapshot_state(&mut self, next_id: u64, free_slots: Vec<usize>);
}
```

### IndexBackendFactory

```rust
pub enum IndexBackendFactory {
    BruteForce,                    // O(n) exact search (default)
    Hnsw(HnswConfig),             // O(log n) approximate search
}
```

### BruteForce Backend

- **Complexity**: O(n) per search
- **Recall**: 100% (exact)
- **Structure**: Linear scan over `VectorHeap` contiguous f32 storage
- **Determinism**: BTreeMap iteration + (score desc, VectorId asc) tie-break

### HNSW Backend

- **Complexity**: O(log n) per search
- **Recall**: ~95%+ with default parameters
- **Algorithm**: Hierarchical Navigable Small World (Malkov & Yashunin, 2016)
- **Structure**: Multi-layer graph with `BTreeMap<VectorId, HnswNode>`
- **Determinism**: Fixed RNG seed (SplitMix64) + BTreeSet neighbors + BTreeMap nodes

**Key HNSW internals:**

| Component | Purpose |
|-----------|---------|
| `HnswConfig` | M, ef_construction, ef_search, ml parameters |
| `HnswNode` | Per-node neighbors (BTreeSet per layer), max_layer, deleted flag, `created_at`/`deleted_at` timestamps |
| `ScoredId` | Candidate with score for heap-based beam search |
| `search_layer()` | Algorithm 2: beam search at a single layer with max-heap candidates |
| `greedy_search_to_layer()` | Algorithm 5: greedy descent through upper layers |
| `insert_into_graph()` | Algorithm 1: insert with M-neighbor selection and Mmax pruning |
| `rebuild_graph()` | Post-recovery reconstruction from heap embeddings |
| `serialize_graph_state()` | Snapshot: serialize full graph topology |
| `deserialize_graph_state()` | Snapshot: restore graph from bytes |
| `is_alive_at(ts)` | Temporal check: `created_at <= ts && (deleted_at.is_none() \|\| deleted_at > ts)` |
| `search_at(query, k, ts)` | Temporal search: same as `search()` but filters by `is_alive_at(ts)` |

### Distance Functions (distance.rs)

Shared by both backends:

| Function | Formula | Range |
|----------|---------|-------|
| `cosine_similarity(a, b)` | `dot(a,b) / (||a|| * ||b||)` | [-1, 1] |
| `euclidean_similarity(a, b)` | `1 / (1 + l2_distance(a,b))` | (0, 1] |
| `dot_product(a, b)` | `sum(a[i] * b[i])` | unbounded |

All normalized: **higher = more similar** (Invariant R2).

## Operation Flows

### VectorUpsert

**Steps:**

1. **Handler**: Validates collection name is not internal (`_` prefix). Auto-creates collection if it doesn't exist (uses vector dimension and Cosine metric by default). Converts metadata `Value` to `serde_json::Value`.
2. **Engine (VectorStore)**:
   - Validates embedding dimension matches collection config
   - Ensures collection is loaded in memory (`ensure_collection_loaded`)
   - Checks if vector already exists by reading the KV key
   - **New vector**: Allocates a `VectorId` from the backend's monotonic counter, inserts embedding into in-memory backend
   - **Update vector**: Keeps the same `VectorId`, updates embedding in backend (HNSW removes old graph connections and re-inserts)
   - Serializes `VectorRecord` to MessagePack, writes to KV storage via `db.transaction()`
3. **Backend**: Inserts/updates the embedding. For HNSW, this also updates the graph structure.

### VectorBatchUpsert

**Steps:**

1. **Handler**: Validates collection. Converts all entries.
2. **Engine (VectorStore)**:
   - Validates all entries (dimension, key, embedding) before acquiring write lock
   - Acquires single write lock for the entire batch
   - Commits all KV writes in one transaction
   - Updates backend for each entry
   - Returns vector of versions
3. **Atomicity**: If any validation fails, the entire batch is rejected. No partial writes.

### VectorSearch

**Steps:**

1. **Handler**: Validates collection. Converts metadata filter (all 8 FilterOp variants). **Ignores** the `metric` parameter (uses collection's configured metric).
2. **Engine (VectorStore)**: Validates query dimension. Ensures collection loaded. Calls `backend.search(query, k)`.
3. **Backend**:
   - **BruteForce**: Computes similarity for every vector (O(n)). Sorts by (score desc, VectorId asc). Truncates to top-k.
   - **HNSW**: Greedy descent through upper layers, beam search at layer 0 with ef_search width. Filters deleted nodes from results.
4. **Post-search**: For each result, loads metadata from KV. Applies metadata filter with adaptive over-fetch (3x -> 6x -> 12x -> all multipliers). Resolves VectorId to user key.

### VectorCollectionStats

Returns `CollectionInfo` with `index_type` ("brute_force" or "hnsw") and `memory_bytes` (approximate heap + graph memory usage).

## Temporal Search (Time-Travel)

### HnswNode Temporal Tracking

Each `HnswNode` tracks two timestamps:
- `created_at` — set when the node is first inserted (microseconds since epoch)
- `deleted_at` — set when the node is soft-deleted (via `delete_with_timestamp`)

Legacy nodes (from before temporal tracking) have `created_at == 0` and are treated as "always existed".

### `is_alive_at(ts)` Check

```
fn is_alive_at(&self, as_of_ts: u64) -> bool {
    (self.created_at == 0 || self.created_at <= as_of_ts)
        && self.deleted_at.map_or(true, |d| d > as_of_ts)
}
```

### `search_at()` Flow

1. Same entry point selection and greedy descent as `search()`
2. Same ef-search beam search at layer 0
3. Different filter: instead of `!n.is_deleted()`, uses `n.is_alive_at(as_of_ts)`
4. Results contain only vectors that existed at the target timestamp
5. Metadata filtering and adaptive over-fetch still apply on top of temporal filtering
6. Metadata is loaded via `get_at()` (historical KV lookup) to get the metadata as it was at the target time

### WAL Replay Timestamp Preservation

WAL replay uses `insert_with_id_and_timestamp()` and `delete_with_timestamp()` to preserve the original `created_at`/`deleted_at` timestamps on HNSW nodes. This ensures `search_at()` returns correct results after database recovery.

## Metadata Filtering

### FilterOp Operators

| Operator | Comparison | Type Handling |
|----------|-----------|---------------|
| `Eq` | Exact equality | All JsonScalar types |
| `Ne` | Not equal | Inverse of Eq |
| `Gt` | Greater than | Numeric only (non-numeric returns false) |
| `Gte` | Greater than or equal | Numeric only |
| `Lt` | Less than | Numeric only |
| `Lte` | Less than or equal | Numeric only |
| `In` | Value in set | Repeated Eq conditions |
| `Contains` | Substring match | String only (non-string returns false) |

Filters are applied post-search via `MetadataFilter::matches()`. The engine uses adaptive over-fetch to compensate for filtering losses.

## Storage Format

```
Vector entries:
  TypeTag:         0x10 (Vector)
  Key format:      Namespace + TypeTag::Vector + "collection/key".as_bytes()
  Value format:    Value::Bytes(MessagePack) containing VectorRecord

Collection configs:
  TypeTag:         0x12 (VectorConfig)
  Key format:      Namespace + TypeTag::VectorConfig + collection_name.as_bytes()
  Value format:    Value::Bytes(MessagePack) containing CollectionRecord
```

### VectorRecord (stored as MessagePack)

```
VectorRecord {
    vector_id:   u64                    // Internal ID for backend
    embedding:   Vec<f32>               // Full embedding (for recovery)
    metadata:    Option<serde_json::Value>
    version:     u64                    // Per-vector counter
    created_at:  u64                    // Microseconds (used for time-travel)
    updated_at:  u64                    // Microseconds
    source_ref:  Option<EntityRef>      // Cross-reference to source entity
}
```

Note: `created_at` and `updated_at` are used by time-travel queries. The HNSW node mirrors these as `created_at`/`deleted_at` for temporal graph filtering during `search_at()`.

### CollectionRecord (stored as MessagePack)

```
CollectionRecord {
    config:     VectorConfigSerde       // { dimension, metric }
    created_at: u64                     // Microseconds
}
```

### In-Memory Backend State

```
BruteForceBackend {
    heap: VectorHeap {
        data:       Vec<f32>                        // Contiguous embedding storage
        id_to_offset: BTreeMap<VectorId, usize>     // ID -> data offset
        dimension:  usize
        metric:     DistanceMetric
        next_id:    u64                             // Monotonic ID allocator
        free_slots: Vec<usize>                      // Reusable storage slots
    }
}

HnswBackend {
    config:     HnswConfig                          // M, ef_construction, ef_search, ml
    heap:       VectorHeap                          // Same contiguous storage as brute-force
    nodes:      BTreeMap<VectorId, HnswNode>        // Graph structure
    entry_point: Option<VectorId>                   // Top-level entry node
    max_level:  usize                               // Current max graph level
    rng_seed:   u64                                 // Fixed seed for deterministic levels
    rng_counter: u64                                // Monotonic RNG counter
}
```

### Snapshot Format

The snapshot header includes:

```
CollectionSnapshotHeader {
    branch_id:        [u8; 16]
    name:             String
    dimension:        u32
    metric:           u8
    storage_dtype:    u8
    next_id:          u64
    free_slots:       Vec<u64>
    count:            u64
    index_type:       u8           // 0 = BruteForce, 1 = HNSW
    hnsw_graph_state: Vec<u8>      // Serialized graph (empty for brute-force)
}
```

## Reserved Internal Namespace

Collection names starting with `_system_` are reserved for internal use by the intelligence layer:

- `validate_system_collection_name()` — validates `_system_*` prefix
- `create_system_collection()`, `system_insert()`, `system_search()` — internal API that bypasses user-facing `_` prefix restriction
- `vector_list_collections` filters out `_`-prefixed collections from user-visible results

## Transaction Behavior

| Aspect | Behavior |
|--------|----------|
| Transactional | **No** - bypasses Session transaction layer |
| Isolation | None (direct writes) |
| Engine transactions | Used internally for KV persistence only |
| In-memory consistency | Backend writes are immediate |
| Crash recovery | Rebuild in-memory index from KV records on restart |
| HNSW recovery | `rebuild_graph()` reconstructs graph from heap after recovery |
| Search metric | Fixed per collection at creation time |

## Consistency Notes

- Vector is the only primitive with **dual storage**: in-memory backend (for search) + persistent KV (for durability). All other primitives go through the standard transaction -> storage path.
- Vector operations are **non-transactional** at the Session level. Even within an active Session transaction, vector operations execute immediately and are not rolled back on `TxnRollback`. This is a design choice for performance.
- The `metric` parameter on `VectorSearch` is **ignored** - the collection's configured metric (set at creation) is always used.
- **Auto-creation**: `VectorUpsert` auto-creates collections with Cosine metric and the dimension of the first vector.
- **Post-filter search**: Metadata filtering happens after the backend returns candidates. The engine uses adaptive over-fetch (3x, 6x, 12x, all) to compensate.
- The `VectorId` is an internal monotonic counter per collection, separate from the user-provided key string. The mapping is maintained through the KV-stored `VectorRecord`.
- Collection names starting with `_` are reserved for internal use and rejected by the handler's `validate_not_internal_collection()` check.
