# Intelligence Layer Architecture

The intelligence layer provides derived operations over Strata's six primitives. Search is the first capability; the architecture is designed to accommodate future composite operations such as graph traversal and cross-primitive analytics.

This document covers the `strata-intelligence` crate itself and all supporting infrastructure added to `strata-engine`, `strata-core`, and `strata-executor`.

---

## Crate Map

```text
strata-core           Defines EntityRef, PrimitiveType (contract types)
     |
strata-engine         Owns search_types, tokenizer, InvertedIndex,
     |                scoring (BM25Lite / Simple), Searchable trait,
     |                and all six primitive facades
     |
strata-intelligence   Orchestration: HybridSearch, Fuser, DatabaseSearchExt
     |
strata-executor       Exposes Command::Search / Output::SearchResults
```

`strata-intelligence` depends on `strata-engine` and `strata-core`.
`strata-executor` depends on `strata-intelligence` plus the usual engine and core crates.

---

## 1. Search Types (`strata-engine::search_types`)

All search operations share a single set of request/response types.
These live in `crates/engine/src/search_types.rs` and are re-exported from `strata_engine`.

| Type | Purpose |
|------|---------|
| `SearchRequest` | Universal request: query, run_id, k, budget, mode, primitive_filter, time_range, tags |
| `SearchResponse` | Ranked hits plus execution metadata |
| `SearchHit` | Single result: `EntityRef`, score (f32), rank (u32, 1-indexed), optional snippet |
| `SearchBudget` | Wall-time and candidate limits; defaults to 100 ms / 10 000 candidates |
| `SearchStats` | Elapsed time, candidates considered, per-primitive breakdown, `index_used` flag |
| `SearchMode` | `Keyword` (default), `Vector` (reserved), `Hybrid` (reserved) |

Contract types `EntityRef` and `PrimitiveType` are defined in `strata-core::contract` and re-exported through `search_types`.

### EntityRef

`EntityRef` is the back-pointer from a search hit to the source record.
Each variant carries the minimum information needed to retrieve the original data:

| Variant | Fields | Example |
|---------|--------|---------|
| `Kv` | `run_id`, `key` | A key-value pair |
| `Json` | `run_id`, `doc_id` | A JSON document |
| `Event` | `run_id`, `sequence` | An event log entry |
| `State` | `run_id`, `name` | A state cell |
| `Run` | `run_id` | A run's metadata |
| `Vector` | `run_id`, `key` | A vector embedding |

### SearchRequest Invariant

> The same `SearchRequest` type is used for primitive-level search (`kv.search(&req)`) and composite search (`hybrid.search(&req)`). This invariant must not change.

---

## 2. Tokenizer (`strata-engine::primitives::tokenizer`)

A minimal tokenizer used by both the inverted index and the BM25 scorer.

```rust
pub fn tokenize(text: &str) -> Vec<String>
```

Lowercases, splits on non-alphanumeric boundaries, drops tokens shorter than 2 characters. No stemming or stop-word removal.

```rust
pub fn tokenize_unique(text: &str) -> Vec<String>
```

Same as `tokenize` but deduplicates while preserving first-occurrence order.

The tokenizer lives in engine so that primitive write paths can call it without depending on the intelligence crate.
`strata-intelligence::tokenizer` re-exports both functions for backward compatibility.

---

## 3. Inverted Index (`strata-engine::primitives::index`)

A concurrent, lock-free inverted index backed by `DashMap`. Primitives register documents on writes; the index supplies corpus statistics for BM25 scoring at query time.

### Data Structures

```
InvertedIndex
  postings:    DashMap<String, PostingList>     term -> posting list
  doc_freqs:   DashMap<String, usize>           term -> document frequency
  doc_lengths: DashMap<EntityRef, u32>          per-document token count
  total_docs:  AtomicUsize                      corpus size
  total_doc_len: AtomicUsize                    sum of all document lengths
  enabled:     AtomicBool                       global on/off switch
  version:     AtomicU64                        monotonic watermark
```

A `PostingList` contains `Vec<PostingEntry>` where each entry holds an `EntityRef`, term frequency, document length, and optional timestamp.

### Enable / Disable

The index defaults to **disabled**. All mutation methods (`index_document`, `remove_document`) check `enabled` with `Acquire` ordering and return immediately when off. This gives **zero overhead** to workloads that do not need search.

### Write-Path Operations

**`index_document(&self, doc_ref, text, ts_micros)`**

1. Return if disabled.
2. If the document already exists, remove it first (prevents double-counting).
3. Tokenize the text and count per-term frequencies.
4. Insert a `PostingEntry` into each term's `PostingList`.
5. Update `doc_freqs`, `doc_lengths`, `total_docs`, `total_doc_len`.
6. Increment `version` with `Release` ordering.

**`remove_document(&self, doc_ref)`**

1. Return if disabled.
2. Look up the document's tracked length from `doc_lengths`.
3. Remove entries from every posting list, decrement `doc_freqs`.
4. Decrement `total_docs` and `total_doc_len`.
5. Increment `version` with `Release` ordering.

### Query-Path Operations

**`lookup(term) -> Option<PostingList>`** — returns a clone of the posting list for a term.

**`doc_freq(term) -> usize`** — document frequency for IDF.

**`compute_idf(term) -> f32`** — standard BM25 IDF: `ln((N - df + 0.5) / (df + 0.5) + 1)`.

**`avg_doc_len() -> f32`** — average token count across all indexed documents.

### Access Pattern

Primitives obtain the index via the engine's extension mechanism:

```rust
let index = self.db.extension::<InvertedIndex>();
```

`Database::extension<T>()` lazily initializes a `T: Default + Send + Sync + 'static` the first time it is requested. `InvertedIndex::default()` creates a disabled instance, so the first call is free.

`strata-intelligence::index` re-exports the types for backward compatibility.

---

## 4. Scoring (`strata-engine::primitives::searchable`)

### Scorer Trait

```rust
pub trait Scorer: Send + Sync {
    fn score(&self, doc: &SearchDoc, query: &str, ctx: &ScorerContext) -> f32;
    fn name(&self) -> &str;
}
```

### BM25LiteScorer

The primary scorer. For each query term *t* present in the document:

```
score += IDF(t) * (tf * (k1 + 1)) / (tf + k1 * (1 - b + b * dl/avgdl))
```

| Parameter | Default | Role |
|-----------|---------|------|
| `k1` | 1.2 | Term-frequency saturation |
| `b` | 0.75 | Document-length normalization |
| `recency_boost` | 0.1 | Multiplier for time-decay bonus |

Additional boosts applied after the BM25 sum:

- **Recency**: `score *= 1.0 + recency_boost * (1.0 / (1.0 + age_hours / 24.0))` when a timestamp is present.
- **Title match**: 20% multiplicative boost if any query term appears in the document's title field.

### SimpleScorer (Legacy)

A simpler TF-based scorer kept for backward compatibility and as the fallback when the inverted index is disabled. Scores are clamped to `[0.01, 1.0]` using a token-overlap ratio and a length-normalization divisor.

### ScorerContext

Carries corpus-level statistics needed by BM25:

| Field | Source |
|-------|--------|
| `total_docs` | `InvertedIndex::total_docs()` |
| `doc_freqs` | `InvertedIndex::doc_freq()` per query term |
| `avg_doc_len` | `InvertedIndex::avg_doc_len()` |
| `now_micros` | `SystemTime::now()` |

### SearchDoc

An ephemeral scoring view constructed per candidate during search. Fields: `body`, `title`, `tags`, `ts_micros`, `byte_size`.

### Searchable Trait

```rust
pub trait Searchable {
    fn search(&self, req: &SearchRequest) -> Result<SearchResponse>;
    fn primitive_kind(&self) -> PrimitiveType;
}
```

Implemented by all six primitives: `KVStore`, `JsonStore`, `EventLog`, `StateCell`, `RunIndex`, `VectorStore`.

### build_search_response_with_index

The single integration point used by every primitive's `search()` implementation:

```rust
pub fn build_search_response_with_index(
    candidates: Vec<SearchCandidate>,
    query: &str,
    k: usize,
    truncated: bool,
    elapsed_micros: u64,
    index: Option<&InvertedIndex>,
) -> SearchResponse
```

Decision logic:
- If `index` is `Some`, enabled, and has documents: build `ScorerContext` from index stats, score with `BM25LiteScorer`, set `stats.index_used = true`.
- Otherwise: fall back to `SimpleScorer::score_and_rank`.

The backward-compatible wrapper `build_search_response` passes `None` for the index parameter.

`strata-intelligence::scorer` re-exports the scoring types.

---

## 5. Primitive Write-Path Integration

Each primitive indexes documents after successful transaction commits. The pattern is identical across all five text-oriented primitives:

```rust
// After transaction commit:
let index = self.db.extension::<InvertedIndex>();
if index.is_enabled() {
    let text = /* primitive-specific text extraction */;
    let entity_ref = /* primitive-specific EntityRef */;
    index.index_document(&entity_ref, &text, timestamp);
}
```

### Per-Primitive Details

| Primitive | Write Ops Indexed | Text Extraction | Delete Handling |
|-----------|-------------------|-----------------|-----------------|
| **KVStore** | `put` | `extract_kv_text(key, value)` — key + JSON-serialized value | `delete` calls `remove_document` |
| **JsonStore** | `create` | `flatten_json(value)` — recursive JSON flattening | Not yet wired (complex partial-update transactions) |
| **EventLog** | `append` | `"{event_type} {json_payload}"` | Append-only; no deletes |
| **StateCell** | `set` (CAS) | `"{name} {json_value}"` | `delete` calls `remove_document` |
| **RunIndex** | `create_run_with_options` | `extract_run_text(run_info)` | Not applicable |
| **VectorStore** | — | Not indexed (uses embeddings) | — |

VectorStore is excluded because vector search operates on embeddings, not keyword text. The `Searchable` implementation for VectorStore returns empty results for keyword queries by design.

---

## 6. HybridSearch Orchestrator (`strata-intelligence::hybrid`)

`HybridSearch` is the composite search entry point. It is **stateless**: it holds only `Arc` references to the database and primitive facades.

### Construction

```rust
let hybrid = HybridSearch::new(db.clone());
// or via the extension trait:
let hybrid = db.hybrid(); // requires `use DatabaseSearchExt`
```

### Search Flow

```text
SearchRequest
     |
     v
1. select_primitives()     Filter by request's primitive_filter, or use all 6
     |
2. allocate_budgets()      Divide wall-time evenly across selected primitives
     |
3. for each primitive:
   |  search_primitive()   Calls primitive.search(sub_request)
   |  - check wall-time budget, break if exceeded
   |  - collect SearchResponse + stats
     |
4. fuser.fuse()            Combine all results into a single ranked list
     |
     v
SearchResponse
```

### Snapshot Consistency

Each primitive's `search()` uses its own MVCC snapshot. True cross-primitive snapshot consistency would require a shared snapshot token, which is deferred. For the current design, per-primitive snapshots are acceptable.

### Vector Search Gap

`VectorStore` implements `Searchable` but returns empty results for keyword queries. To perform semantic search, callers should use `hybrid.vector()` to get a direct reference to the `VectorStore` and call its embedding-based search methods. Hybrid keyword + vector search would require an embedding field on `SearchRequest` (future work).

---

## 7. Result Fusion (`strata-intelligence::fuser`)

The `Fuser` trait defines how results from multiple primitives are combined:

```rust
pub trait Fuser: Send + Sync {
    fn fuse(&self, results: Vec<(PrimitiveType, SearchResponse)>, k: usize) -> FusedResult;
    fn name(&self) -> &str;
}
```

### SimpleFuser (Default)

1. Concatenate all hits from all primitives.
2. Sort by score descending.
3. Take top-k, re-assign ranks 1..k.

Deterministic for identical scores only by insertion order (not stable across runs with different primitive orderings).

### RRFFuser (Reciprocal Rank Fusion)

Designed for combining results from different ranking algorithms where raw scores are not comparable.

For each document appearing in any result list:

```
RRF_score = sum( 1 / (k_rrf + rank) )    across all lists containing the document
```

Default `k_rrf = 60`. Documents appearing in multiple lists accumulate higher RRF scores and rank higher in the final output. Tie-breaking is deterministic: first by original score, then by `EntityRef` hash.

---

## 8. Executor Integration (`strata-executor`)

The executor exposes search as a first-class command.

### Command

```rust
Command::Search {
    run: Option<RunId>,        // defaults to "default" run
    query: String,             // search query
    k: Option<u64>,            // max results (default 10)
    primitives: Option<Vec<String>>,  // e.g. ["kv", "json"]
}
```

### Output

```rust
Output::SearchResults(Vec<SearchResultHit>)
```

Where `SearchResultHit` is:

```rust
pub struct SearchResultHit {
    pub entity: String,       // human-readable entity identifier
    pub primitive: String,    // "kv", "json", "event", "state", "run", "vector"
    pub score: f32,
    pub rank: u32,            // 1-indexed
    pub snippet: Option<String>,
}
```

### Handler Flow

1. Convert executor `RunId` to core `RunId`.
2. Parse primitive filter strings to `PrimitiveType` enum values.
3. Build `SearchRequest` with query, k, default budget, and optional primitive filter.
4. Create `HybridSearch::new(db)` and call `hybrid.search(&req)`.
5. Convert each `SearchHit` to `SearchResultHit` via `format_entity_ref`, which maps `EntityRef` variants to human-readable `(entity, primitive)` string pairs.
6. Return `Output::SearchResults`.

Errors from the intelligence layer are mapped to `Error::Internal { reason }`.

---

## 9. Data Flow Summary

### Write Path

```text
Application
  -> Primitive.put / append / set / create
    -> Transaction commit
    -> db.extension::<InvertedIndex>()
    -> if enabled:
         tokenize(text)
         index_document(EntityRef, text, timestamp)
           -> update postings, doc_freqs, doc_lengths
           -> increment version (Release)
```

### Query Path

```text
Application
  -> executor.execute(Command::Search { ... })
    -> HybridSearch::new(db)
    -> hybrid.search(SearchRequest)
      -> select_primitives(filter)
      -> allocate_budgets(time / N)
      -> for each primitive:
           primitive.search(sub_request)
             -> enumerate candidates (snapshot scan)
             -> build_search_response_with_index(candidates, index)
               -> if index enabled: BM25LiteScorer + corpus stats
               -> else: SimpleScorer fallback
             -> SearchResponse
      -> Fuser.fuse(all_responses, k)
        -> SimpleFuser: sort by score, take k
        -> or RRFFuser: reciprocal rank fusion, take k
    -> convert SearchHit -> SearchResultHit
    -> Output::SearchResults
```

---

## 10. Design Invariants

| # | Invariant | Rationale |
|---|-----------|-----------|
| 1 | All search operations use `SearchRequest` / `SearchResponse` | Single type eliminates adapter code between layers |
| 2 | InvertedIndex is zero-overhead when disabled | Default state is disabled; no cost to workloads that don't search |
| 3 | Primitives hold only `Arc<Database>` | Stateless facade pattern; no per-primitive state management |
| 4 | `InvertedIndex` accessed via `Database::extension<T>()` | Same lazy-init pattern used by VectorStore; no constructor changes |
| 5 | Intelligence crate re-exports moved types | Tokenizer, index, and scorer moved to engine but remain importable from intelligence |
| 6 | Write-path indexing happens after transaction commit | Index reflects committed state only; no partial or rolled-back data |
| 7 | HybridSearch is stateless | Holds only `Arc` references; constructed per-request or cached freely |
| 8 | No data duplication in the index | PostingEntry stores `EntityRef`, not content; text is extracted on demand during search |

---

## 11. Module Layout

```
crates/
  core/src/
    contract.rs            EntityRef, PrimitiveType definitions
    search_types.rs        Re-export stub (types moved to engine)

  engine/src/
    search_types.rs        SearchRequest, SearchResponse, SearchBudget,
                           SearchHit, SearchStats, SearchMode
    primitives/
      tokenizer.rs         tokenize(), tokenize_unique()
      index.rs             InvertedIndex, PostingList, PostingEntry
      searchable.rs        Searchable trait, SearchCandidate, SearchDoc,
                           ScorerContext, Scorer trait, BM25LiteScorer,
                           SimpleScorer, build_search_response[_with_index]
      kv.rs                KVStore (indexes on put, removes on delete)
      json_store.rs        JsonStore (indexes on create)
      event_log.rs         EventLog (indexes on append)
      state_cell.rs        StateCell (indexes on set, removes on delete)
      run_index.rs         RunIndex (indexes on create_run)
      vector/store.rs      VectorStore (no keyword indexing)

  intelligence/src/
    lib.rs                 DatabaseSearchExt trait, re-exports
    hybrid.rs              HybridSearch orchestrator
    fuser.rs               Fuser trait, SimpleFuser, RRFFuser
    tokenizer.rs           Re-exports from engine
    index.rs               Re-exports from engine
    scorer.rs              Re-exports from engine

  executor/src/
    command.rs             Command::Search variant
    output.rs              Output::SearchResults variant
    types.rs               SearchResultHit
    handlers/search.rs     Search handler (HybridSearch integration)
    executor.rs            Dispatch for Command::Search
```
