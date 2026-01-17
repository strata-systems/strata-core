# Project Milestones: In-Memory Agent Database

## MVP Target: Single-Node, Embedded Library with Core Primitives + Performance + Replay

---

## Milestone 1: Foundation ✅
**Goal**: Basic storage and WAL without transactions

**Deliverable**: Can store/retrieve KV pairs and append to WAL, recover from WAL on restart

**Status**: Complete

**Success Criteria**:
- [x] Cargo workspace builds
- [x] Core types defined (RunId, Key, Value, TypeTag)
- [x] UnifiedStore stores and retrieves values
- [x] WAL appends entries and can be read back
- [x] Basic recovery: restart process, replay WAL, restore state
- [x] Unit tests pass

**Risk**: Foundation bugs will cascade. Must get this right.

---

## Milestone 2: Transactions ✅
**Goal**: OCC with snapshot isolation and conflict detection

**Deliverable**: Concurrent transactions with proper isolation and rollback

**Status**: Complete

**Success Criteria**:
- [x] TransactionContext with read/write sets
- [x] Snapshot isolation (ClonedSnapshotView)
- [x] Conflict detection at commit
- [x] CAS operations work
- [x] Multi-threaded tests show proper isolation
- [x] Conflict resolution (retry/abort) works

**Risk**: Concurrency bugs are subtle. Need thorough testing.

---

## Milestone 3: Primitives ✅
**Goal**: All 5 MVP primitives working (KV, Event Log, StateCell, Trace, Run Index)

**Deliverable**: Agent can use all primitive APIs

**Status**: Complete

**Success Criteria**:
- [x] KV store: get, put, delete, list
- [x] Event log: append, read, simple chaining (non-crypto hash)
- [x] StateCell: read, init, cas, set, transition
- [x] Trace store: record tool calls, decisions, queries
- [x] Run Index: create_run, get_run, update_status, query_runs
- [x] All primitives are stateless facades over engine
- [x] Integration tests cover primitive interactions

**Risk**: Layer boundaries. Primitives must not leak into each other.

---

## Milestone 4: Performance ✅
**Goal**: Remove architectural blockers to Redis-class latency

**Deliverable**: Database achieves 250K ops/sec in InMemory mode with <10µs read latency

**Status**: Complete

**Philosophy**: M4 does not aim to be fast. M4 aims to be *fastable*. M4 removes blockers; M5+ achieves parity.

**Critical Invariants** (validated via codebase analysis):
- **Atomicity Scope**: Transactions atomic within single RunId only; cross-run atomicity not guaranteed
- **Snapshot Semantics**: Fast-path reads must be observationally equivalent to snapshot-based transactions
- **Dependencies**: Use `rustc-hash` (not `fxhash`), `dashmap`, `parking_lot`

**Success Criteria**:

### Gate 1: Durability Modes
- [x] Three modes implemented: InMemory, Buffered, Strict
- [x] InMemory mode: `engine/put_direct` < 3µs
- [x] InMemory mode: 250K ops/sec (1-thread)
- [x] Buffered mode: `kvstore/put` < 30µs
- [x] Buffered mode: 50K ops/sec throughput
- [x] Buffered mode: Thread lifecycle managed (shutdown flag + join)
- [x] Strict mode: Same behavior as M3 (backwards compatible)

### Gate 2: Hot Path Optimization
- [x] Transaction pooling: Zero allocations in A1 hot path
- [x] Snapshot acquisition: < 500ns, allocation-free
- [x] Read optimization: `kvstore/get` < 10µs

### Gate 3: Scaling
- [x] Lock sharding: DashMap + HashMap replaces RwLock + BTreeMap
- [x] Disjoint scaling ≥ 1.8× at 2 threads
- [x] Disjoint scaling ≥ 3.2× at 4 threads
- [x] 4-thread disjoint throughput: ≥ 800K ops/sec

### Gate 4: Facade Tax
- [x] A1/A0 < 10× (InMemory mode)
- [x] B/A1 < 5×
- [x] B/A0 < 30×

### Gate 5: Infrastructure
- [x] Baseline tagged: `m3_baseline_perf`
- [x] Per-layer instrumentation working
- [x] Backwards compatibility: M3 code unchanged

### Red Flag Check (hard stops)
- [x] Snapshot acquisition ≤ 2µs
- [x] A1/A0 ≤ 20×
- [x] B/A1 ≤ 8×
- [x] Disjoint scaling (4 threads) ≥ 2.5×
- [x] p99 ≤ 20× mean
- [x] Zero hot-path allocations

**Risk**: Performance work can be unbounded. M4 is scoped to *de-blocking*, not *optimization*. Red flags define hard stops. ✅ Mitigated

**Architecture Doc**: [M4_ARCHITECTURE.md](../architecture/M4_ARCHITECTURE.md)
**Diagrams**: [m4-architecture.md](../diagrams/m4-architecture.md)

---

## Milestone 5: JSON Primitive ✅
**Goal**: Native JSON primitive with path-level mutation semantics

**Deliverable**: JsonStore primitive with region-based conflict detection, integrated into transaction system

**Status**: Complete

**Philosophy**: JSON is not a value type. It defines **mutation semantics**. M5 freezes the semantic model. M6+ optimizes the implementation.

**Success Criteria**:

### Gate 1: Core Semantics
- [x] JsonStore::create() works
- [x] JsonStore::get(path) works
- [x] JsonStore::set(path) works
- [x] JsonStore::delete(path) works
- [x] JsonStore::cas() works with document version
- [x] JsonStore::patch() applies multiple operations atomically

### Gate 2: Conflict Detection
- [x] Sibling paths do not conflict
- [x] Ancestor/descendant paths conflict
- [x] Same path conflicts
- [x] Different documents do not conflict
- [x] Root path conflicts with all paths

### Gate 3: WAL Integration
- [x] JSON WAL entries written correctly (0x20-0x23)
- [x] WAL replay is deterministic
- [x] WAL replay is idempotent
- [x] Recovery works after simulated crash

### Gate 4: Transaction Integration
- [x] JSON participates in transactions
- [x] Read-your-writes works
- [x] Cross-primitive atomicity works
- [x] Conflict detection fails transaction correctly

### Gate 5: Non-Regression
- [x] KV performance unchanged
- [x] Event performance unchanged
- [x] State performance unchanged
- [x] Trace performance unchanged
- [x] Non-JSON transactions have zero overhead

**Risk**: Semantic complexity. Must lock in semantics before optimization. ✅ Mitigated

**Architecture Doc**: [M5_ARCHITECTURE.md](../architecture/M5_ARCHITECTURE.md)

---

## Milestone 6: Retrieval Surfaces (Current)
**Goal**: Add retrieval surface for fast experimentation with search and ranking across all primitives

**Deliverable**: Primitive-native search hooks + composite search planner + minimal keyword search algorithm

**Philosophy**: M6 is the "retrieval substrate milestone". It does not ship a world-class search engine. It ships the **surface** that enables algorithm swaps without engine rewrites.

**Success Criteria**:

### Gate 1: Primitive Search APIs
- [ ] `kv.search(&SearchRequest)` returns `SearchResponse`
- [ ] `json.search(&SearchRequest)` returns `SearchResponse`
- [ ] `event.search(&SearchRequest)` returns `SearchResponse`
- [ ] `state.search(&SearchRequest)` returns `SearchResponse`
- [ ] `trace.search(&SearchRequest)` returns `SearchResponse`
- [ ] `run_index.search(&SearchRequest)` returns `SearchResponse`

### Gate 2: Composite Search
- [ ] `db.hybrid.search(&SearchRequest)` orchestrates across primitives
- [ ] RRF (Reciprocal Rank Fusion) with k_rrf=60 implemented
- [ ] Primitive filters honored
- [ ] Time range filters work
- [ ] Budget enforcement (time and candidate caps)

### Gate 3: Core Types
- [ ] `SearchDoc` ephemeral view with DocRef back-pointer
- [ ] `DocRef` variants for all primitives (Kv, Json, Event, State, Trace, Run)
- [ ] `SearchRequest` with query, k, budget, mode, filters
- [ ] `SearchResponse` with hits, truncated flag, stats

### Gate 4: Indexing (Optional)
- [ ] Inverted index per primitive (opt-in)
- [ ] BM25-lite scoring over extracted text
- [ ] Index updates on commit (synchronous)
- [ ] Snapshot-consistent search results

### Gate 5: Non-Regression
- [ ] Zero overhead when search APIs not used
- [ ] No extra allocations per transaction when search disabled
- [ ] No background indexing unless opted in

**Risk**: Scope creep into full search engine. M6 validates the surface only.

**Architecture Doc**: [M6_ARCHITECTURE.md](../architecture/M6_ARCHITECTURE.md)

---

## Milestone 7: Durability & Snapshots
**Goal**: Production-ready persistence with snapshots and recovery

**Deliverable**: Database survives crashes and restarts correctly with efficient snapshot-based recovery

**Success Criteria**:

### Gate 1: Snapshot System
- [ ] Periodic snapshots (time-based and size-based)
- [ ] Snapshot metadata includes version and WAL offset
- [ ] WAL truncation after snapshot
- [ ] Full recovery: load snapshot + replay WAL

### Gate 2: Crash Recovery
- [ ] Crash simulation tests pass
- [ ] Durability modes from M4 integrate with snapshot system
- [ ] Bounded recovery time (proportional to WAL size since last snapshot)

### Gate 3: JSON Recovery
- [ ] JSON documents recovered correctly from WAL
- [ ] JSON patches replayed in order
- [ ] Cross-primitive transactions recover atomically

**Risk**: Data loss bugs. Must test recovery thoroughly.

---

## Milestone 8: Replay & Polish
**Goal**: Deterministic replay and production readiness

**Deliverable**: Production-ready MVP with replay

**Success Criteria**:
- [ ] replay_run(run_id) reconstructs database state
- [ ] Run Index enables O(run size) replay (not O(WAL size))
- [ ] diff_runs(run_a, run_b) compares two runs
- [ ] Example agent application works end-to-end
- [ ] Benchmarks show >250K ops/sec (InMemory), >10K ops/sec (Buffered)
- [ ] Integration test coverage >90%
- [ ] Documentation: README, API docs, examples
- [ ] Run lifecycle (begin_run, end_run) fully working

**Risk**: Replay correctness. Must validate determinism.

---

## Post-MVP Milestones (Future)

### Milestone 9: Vector Store
- Implement vector primitive with HNSW index
- Semantic search with metadata filters
- Integration with KV/Event/Trace/JSON primitives
- Plugs into M6 retrieval surface for hybrid search

### Milestone 10: Network Layer
- RPC server (gRPC or similar)
- Client libraries (Rust, Python)
- Multi-client support

### Milestone 11: MCP Integration
- MCP server implementation
- Tool definitions for agent access
- IDE integration demos

### Milestone 12: Advanced Features
- Query DSL for complex filters
- Run forking and lineage tracking
- Incremental snapshots
- Advanced sharding strategies

### Milestone 13: JSON Optimization (Structural Storage)
- Per-node versioning / subtree MVCC
- Structural sharing for efficient snapshots
- Array insert/remove with stable identities
- Diff operations

### Milestone 14: Performance Phase 2 (Redis Parity)
- Arena allocators and memory management
- Cache-line alignment and SoA transforms
- Lock-free reads (epoch-based/RCU)
- Prefetching and branch optimization
- Target: Millions ops/sec (Redis internal loop parity)

---

## MVP Definition

**MVP = Milestones 1-8 Complete**

At MVP completion, the system should:
1. Store agent state in 6 primitives (KV, Events, StateCell, Trace, RunIndex, **JSON**)
2. Support concurrent transactions with OCC
3. **Achieve Redis-competitive performance in InMemory mode (250K+ ops/sec)**
4. Persist data with WAL and snapshots
5. Survive crashes and recover correctly
6. Replay runs deterministically
7. Run as embedded library (single-node)
8. Scale near-linearly for disjoint keys (multi-thread)
9. Have >90% test coverage
10. **JSON primitive with path-level mutations and region-based conflict detection**
11. **Retrieval surface with primitive-native search and composite hybrid search**

**Not in MVP**:
- Vector store (Milestone 9)
- Network layer (Milestone 10)
- MCP server (Milestone 11)
- Query DSL (Milestone 12)
- JSON structural optimization (Milestone 13)
- Redis internal loop parity (Milestone 14)
- Distributed mode (far future)

---

## Timeline

```
Completed:
- M1 (Foundation)        ✅
- M2 (Transactions)      ✅
- M3 (Primitives)        ✅
- M4 (Performance)       ✅
- M5 (JSON Primitive)    ✅

Current:
- M6 (Retrieval Surfaces) ← YOU ARE HERE

Remaining:
- M7 (Durability & Snapshots)
- M8 (Replay & Polish)
```

---

## Critical Path

```
M1 (Foundation) ✅
  ↓
M2 (Transactions) ✅
  ↓
M3 (Primitives) ✅
  ↓
M4 (Performance) ✅
  ↓
M5 (JSON Primitive) ✅
  ↓
M6 (Retrieval Surfaces) ← Current
  ↓
M7 (Durability & Snapshots)
  ↓
M8 (Replay & Polish)
```

**Notes**:
- M4 introduced durability *modes* (InMemory/Buffered/Strict). M7 adds durability *infrastructure* (snapshots, WAL rotation).
- M5 locked in JSON mutation semantics. JSON optimization (structural storage, per-node versioning) is M13.
- M6 adds retrieval surface that M9 (Vector Store) will plug into for hybrid search.
- M7 must handle JSON recovery in addition to original primitives.

---

## Risk Mitigation

### High-Risk Areas
1. **Concurrency (M2)**: OCC bugs are subtle ✅ Mitigated
   - Mitigation: Extensive multi-threaded tests completed
2. **Recovery (M7)**: Data loss is unacceptable
   - Mitigation: Crash simulation tests, fuzzing WAL corruption
3. **Layer boundaries (M3)**: Primitives leaking into each other ✅ Mitigated
   - Mitigation: Mock tests, strict dependency rules enforced
4. **Performance unbounded (M4)**: Optimization work can expand infinitely ✅ Mitigated
   - Mitigation: Red flag thresholds defined hard stops; M4 completed within scope

### Medium-Risk Areas
1. **Performance targets (M4)**: May not hit 250K ops/sec ✅ Mitigated
   - Mitigation: DashMap + HashMap architecture delivered; benchmarks validated
2. **JSON semantic complexity (M5)**: Mutation semantics can drift ✅ Mitigated
   - Mitigation: Six architectural rules enforced; semantics frozen before optimization
3. **Retrieval scope creep (M6)**: Risk of building full search engine
   - Mitigation: Six architectural rules; M6 validates surface only, not relevance
4. **Replay correctness (M8)**: Determinism is hard
   - Mitigation: Property-based tests, replay verification

### Low-Risk Areas
1. **Foundation (M1)**: Well-understood patterns ✅ Complete
2. **API design (M3)**: Can iterate post-MVP ✅ Complete
3. **JSON API (M5)**: Follows established primitive patterns ✅ Complete

---

## Performance Targets Summary

| Mode | Latency Target | Throughput Target |
|------|----------------|-------------------|
| **InMemory** | <8µs put, <5µs get | 250K ops/sec |
| **Buffered** | <30µs put, <5µs get | 50K ops/sec |
| **Strict** | ~2ms put, <5µs get | ~500 ops/sec |

**Comparison**:
- Redis over TCP: ~100K-200K ops/sec
- Redis internal loop: Millions ops/sec
- M4 target: 250K ops/sec (removes blockers)
- M14 target: Millions ops/sec (Redis parity)

---

## Document History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | Initial | Original 5-milestone plan |
| 2.0 | 2026-01-15 | Inserted M4 Performance; MVP now 6 milestones |
| 3.0 | 2026-01-16 | M4 complete; M5 JSON Primitive complete; MVP now 7 milestones (M1-M7) |
| 4.0 | 2026-01-16 | Inserted M6 Retrieval Surfaces; Durability→M7, Replay→M8; MVP now 8 milestones (M1-M8) |
