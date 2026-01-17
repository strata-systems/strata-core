M6 Architecture Specification: Retrieval Surfaces and Composite Search

Version: 1.0
Status: Design Complete
Last Updated: 2026-01-16

Executive Summary

M6 adds a retrieval surface to in-mem that enables fast experimentation with search and ranking across all primitives, without baking “search opinions” into the engine or forcing a leaky abstraction over primitives.

M6 does not attempt to ship a world-class search engine. It ships:

Primitive-native search hooks (so each primitive can be queried using its own best path)

A composite search planner (so we can fuse multiple primitive result sets consistently)

A minimal, table-stakes composite algorithm (hello world retrieval) to validate the surface

M6 is the “retrieval substrate milestone”.

1. Scope Boundaries
1.1 What M6 IS

M6 defines:

A standard search request/response model across primitives

Primitive-native search entrypoints: kv.search(), json.search(), etc.

A composite search orchestration layer: db.hybrid.search()

Pluggable ranking and fusion interfaces (RRF-first)

Minimal indexing needed to make keyword search non-embarrassing

1.2 What M6 is NOT

Explicitly deferred:

Vector search (HNSW, IVF, PQ) as a core capability (later milestone)

Learning-to-rank / cross-encoders / LLM rerankers (research project milestones)

Full query DSL like Elasticsearch

Complex analyzers (stemming, synonyms, language detection)

Full-text highlighting, aggregations, faceting

2. The Six Architectural Rules for M6 (Non-Negotiable)
Rule 1: No “Unified Search Abstraction” That Forces Data Movement

Composite search must not require copying primitive state into a new store.
Search must run against each primitive’s native storage layout and access paths.

Rule 2: Primitive Search Is a First-Class API

Each primitive must have its own direct search call:

Database().kv.search(..)

Database().json.search(..)

Database().event.search(..)

Database().state.search(..)

Database().trace.search(..)

Database().run_index.search(..)

Rule 3: Composite Search Orchestrates, It Does Not Replace

Database().hybrid.search(..) is a planner and fusion layer.
It does not “own” indexing, conflict semantics, or storage.

Rule 4: Search Must Be Snapshot-Consistent

Search executes against a SnapshotView (like reads today), so results are stable for that search invocation.

Rule 5: Zero Overhead When Not Used

If no search APIs are invoked:

no extra allocations per transaction

no extra write amplification

no background indexing work

Rule 6: The Surface Must Enable Algorithm Swaps Without Engine Rewrites

The retrieval algorithm must be replaceable behind stable interfaces:

candidate generation

scoring

fusion

reranking hooks

3. Core Concepts
3.1 Searchable Corpus Unit: SearchDoc

Search works over logical “documents” extracted from primitive state.

A SearchDoc is not stored as a new primitive. It is an ephemeral view:

it references a source record (KV key, JSON doc id, event offset, etc.)

it has one or more text fields

it has metadata fields used for ranking/filters

pub struct SearchDoc {
    pub ref_: DocRef,                 // pointer back to the primitive record
    pub primitive: PrimitiveKind,      // KV, JSON, Event, State, Trace, Run
    pub run_id: RunId,
    pub title: Option<String>,
    pub body: String,                 // primary searchable text
    pub tags: SmallVec<[String; 8]>,   // optional
    pub ts_micros: Option<u64>,        // recency signals when available
    pub bytes: Option<u32>,            // size signal
}

3.2 Stable Pointer Back to Source: DocRef

DocRef is how results round-trip into retrieval + follow-up reads.

pub enum DocRef {
    Kv { key: Key },
    Json { key: Key, doc_id: JsonDocId },
    Event { log_key: Key, seq: u64 },
    State { key: Key },
    Trace { key: Key, span_id: u64 },
    Run { run_id: RunId },
}

3.3 Search Request

M6 starts with a minimal request model.

pub struct SearchRequest {
    pub run_id: RunId,
    pub query: String,

    // Control
    pub k: usize,              // top-k results desired
    pub budget: SearchBudget,  // time and work limits
    pub mode: SearchMode,      // Keyword now, hybrid later

    // Filters (optional, minimal)
    pub primitive_filter: Option<Vec<PrimitiveKind>>,
    pub time_range: Option<(u64, u64)>,   // micros
    pub tags_any: SmallVec<[String; 8]>,
}

3.4 Search Response
pub struct SearchHit {
    pub ref_: DocRef,
    pub score: f32,
    pub rank: u32,
    pub snippet: Option<String>,     // optional and cheap in M6
    pub debug: Option<HitDebug>,     // gated, off by default
}

pub struct SearchResponse {
    pub hits: Vec<SearchHit>,
    pub truncated: bool,             // budget hit
    pub stats: SearchStats,
}

4. Primitive Search Surfaces

Each primitive exposes a fast path that is allowed to be specialized.

4.1 KV Search

Contract: Searches over values that are string-like (or have extractable text).

impl KVStore {
    pub fn search(&self, req: &SearchRequest) -> Result<SearchResponse>;
}


Default extractor:

Value::String => body

Value::Map / Value::Array => stable JSON stringification for indexing

4.2 JSON Search

Contract: Searches over serialized JSON docs with a default text projection.

impl JsonStore {
    pub fn search(&self, req: &SearchRequest) -> Result<SearchResponse>;
}


Default extractor:

Flatten all scalar strings and “key: value” pairs into body

Include stable paths (like .a.b) in the body for keyword matching

4.3 Event, StateCell, Trace, RunIndex Search

All follow the same pattern: each provides a search method and a default text projection.

Key point: the primitive decides how to enumerate candidates cheaply:

EventLog can scan recent events first

TraceStore can prioritize latest spans

RunIndex can search run summaries and metadata

5. Composite Search Surface
5.1 API
impl Database {
    pub fn hybrid(&self) -> HybridSearch {
        HybridSearch { db: self.clone() }
    }
}

pub struct HybridSearch {
    db: Arc<Database>,
}

impl HybridSearch {
    pub fn search(&self, req: &SearchRequest) -> Result<SearchResponse>;
}

5.2 Planner Model

Composite search is a tiny query planner:

Determine which primitives to query

Assign per-primitive budgets (time and candidate caps)

Execute primitive searches against the same snapshot

Fuse results (RRF in M6 hello world)

Return top-k

6. Minimal Keyword Indexing in M6

M6 must be “blazing fast” for keyword search on a working set, without building Elasticsearch.

6.1 Index Type: Inverted Index per Primitive (Optional per Primitive)

M6 provides a small indexing component that primitives may opt into:

token -> postings list of DocRef (or a compact doc id)

postings store tf and a small amount of metadata (ts, doc_len)

supports top-k retrieval with early termination heuristics later

This index is not a new database primitive. It is an internal acceleration structure maintained by the primitive.

6.2 Write Path

Index updates happen on commit of primitive writes (not inside the transaction context).
If index is disabled, the primitive search falls back to scan-with-budget.

6.3 Snapshot Consistency

Search uses snapshot reads. Index must be “commit-consistent”:

either reflects committed state up to a version watermark

or search uses the snapshot watermark and ignores newer postings

M6 supports the simplest approach:

index is updated synchronously during commit for the primitive that opts in

index watermark equals storage version after commit

7. The Table-Stakes Composite Algorithm (Hello World Retrieval)

This exists to validate that M6 did its job. It is intentionally simple.

7.1 Per-Primitive Retrieval: “KeywordTopK”

For each primitive:

Tokenize query: lowercase, split on non-alnum, drop tokens length < 2

Retrieve candidate hits:

if primitive has an inverted index: pull postings for each token and accumulate scores

else: scan limited corpus and compute score

Score each candidate with a BM25-lite approximation:

BM25-lite

score(doc) = Σ over tokens t in query: idf(t) * tf_norm(t, doc)

idf(t) = ln(1 + (N / (df(t) + 1)))

tf_norm = tf / (tf + 1)

Add a small recency bump if timestamp exists:

score *= (1.0 + recency_boost)

recency_boost can be a cheap piecewise function by age buckets

Return top k_per_primitive results sorted by score.

7.2 Fusion: RRF (Reciprocal Rank Fusion)

Let each primitive return a ranked list L_p.

RRF score:

rrf(doc) = Σ_p 1 / (k_rrf + rank_p(doc))

Use k_rrf = 60 as the standard safe default

If a doc appears in multiple lists (rare, but possible), it accumulates

Final score:

final = rrf_score + 0.01 * normalized_primitive_score

That tiny tie-break keeps deterministic ordering without overfitting BM25-lite

Select global top-k.

7.3 Why This Is the Right Hello World

Exercises primitive-native search paths

Exercises composite orchestration and fusion

Gives reasonable results without any ML

Creates a stable baseline to beat later with vectors, rerankers, or more advanced indexing

8. Performance Model and Budgets
8.1 Search Budget
pub struct SearchBudget {
    pub max_wall_time_micros: u64,     // hard stop
    pub max_candidates: usize,         // total across primitives
    pub max_candidates_per_primitive: usize,
}

8.2 Blazing Fast Requirement

M6 must make the fast path fast:

if keyword index is enabled: avoid scanning full documents

return top-k with bounded candidate work

never deserialize huge JSON blobs unless the primitive chooses to

Composite search must be parallelizable later, but M6 can be single-threaded if it respects strict budgets.

9. Testing Strategy for M6
9.1 API Contract Tests

Each primitive: search() returns stable DocRef that can be dereferenced to read underlying data

Composite search: honors primitive filters, time filters, budgets

9.2 Determinism Tests

Same snapshot + same request => identical ordered results

9.3 Budget Enforcement Tests

Ensure hard stops do not overrun wall time or candidate caps

Ensure truncated = true when budgets stop execution

9.4 Correctness Baselines

Verify RRF fusion properties with hand-constructed rankings

Verify BM25-lite monotonicity: more query term hits should not lower score

10. M6 Success Criteria Checklist

 Each primitive exposes search(&SearchRequest) and returns SearchResponse

 Composite db.hybrid.search() exists and fuses results

 Stable DocRef pointers work end-to-end

 Snapshot-consistent results per search invocation

 Hard budgets enforced

 Hello-world algorithm produces sensible results in demo workloads

 Zero overhead in non-search paths

Table-Stakes Composite Algorithm Summary (for the spec)

Algorithm: KeywordTopK per primitive + RRF fusion
Per primitive: BM25-lite over extracted text + tiny recency bump
Composite: RRF with k_rrf=60, then top-k selection
Purpose: Validate the retrieval surface, not win relevance benchmarks