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

## Milestone 4: Performance (Current)
**Goal**: Remove architectural blockers to Redis-class latency

**Deliverable**: Database achieves 250K ops/sec in InMemory mode with <10µs read latency

**Philosophy**: M4 does not aim to be fast. M4 aims to be *fastable*. M4 removes blockers; M5+ achieves parity.

**Critical Invariants** (validated via codebase analysis):
- **Atomicity Scope**: Transactions atomic within single RunId only; cross-run atomicity not guaranteed
- **Snapshot Semantics**: Fast-path reads must be observationally equivalent to snapshot-based transactions
- **Dependencies**: Use `rustc-hash` (not `fxhash`), `dashmap`, `parking_lot`

**Success Criteria**:

### Gate 1: Durability Modes
- [ ] Three modes implemented: InMemory, Buffered, Strict
- [ ] InMemory mode: `engine/put_direct` < 3µs
- [ ] InMemory mode: 250K ops/sec (1-thread)
- [ ] Buffered mode: `kvstore/put` < 30µs
- [ ] Buffered mode: 50K ops/sec throughput
- [ ] Buffered mode: Thread lifecycle managed (shutdown flag + join)
- [ ] Strict mode: Same behavior as M3 (backwards compatible)

### Gate 2: Hot Path Optimization
- [ ] Transaction pooling: Zero allocations in A1 hot path
- [ ] Snapshot acquisition: < 500ns, allocation-free
- [ ] Read optimization: `kvstore/get` < 10µs

### Gate 3: Scaling
- [ ] Lock sharding: DashMap + HashMap replaces RwLock + BTreeMap
- [ ] Disjoint scaling ≥ 1.8× at 2 threads
- [ ] Disjoint scaling ≥ 3.2× at 4 threads
- [ ] 4-thread disjoint throughput: ≥ 800K ops/sec

### Gate 4: Facade Tax
- [ ] A1/A0 < 10× (InMemory mode)
- [ ] B/A1 < 5×
- [ ] B/A0 < 30×

### Gate 5: Infrastructure
- [ ] Baseline tagged: `m3_baseline_perf`
- [ ] Per-layer instrumentation working
- [ ] Backwards compatibility: M3 code unchanged

### Red Flag Check (hard stops)
- [ ] Snapshot acquisition ≤ 2µs
- [ ] A1/A0 ≤ 20×
- [ ] B/A1 ≤ 8×
- [ ] Disjoint scaling (4 threads) ≥ 2.5×
- [ ] p99 ≤ 20× mean
- [ ] Zero hot-path allocations

**Risk**: Performance work can be unbounded. M4 is scoped to *de-blocking*, not *optimization*. Red flags define hard stops.

**Architecture Doc**: [M4_ARCHITECTURE.md](../architecture/M4_ARCHITECTURE.md)
**Diagrams**: [m4-architecture.md](../diagrams/m4-architecture.md)

---

## Milestone 5: Durability
**Goal**: Production-ready persistence with snapshots and recovery

**Deliverable**: Database survives crashes and restarts correctly

**Success Criteria**:
- [ ] Periodic snapshots (time-based and size-based)
- [ ] Snapshot metadata includes version and WAL offset
- [ ] WAL truncation after snapshot
- [ ] Full recovery: load snapshot + replay WAL
- [ ] Crash simulation tests pass
- [ ] Durability modes from M4 integrate with snapshot system

**Risk**: Data loss bugs. Must test recovery thoroughly.

---

## Milestone 6: Replay & Polish
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

### Milestone 7: Vector Store
- Implement vector primitive with HNSW index
- Semantic search with metadata filters
- Integration with KV/Event/Trace primitives

### Milestone 8: Network Layer
- RPC server (gRPC or similar)
- Client libraries (Rust, Python)
- Multi-client support

### Milestone 9: MCP Integration
- MCP server implementation
- Tool definitions for agent access
- IDE integration demos

### Milestone 10: Advanced Features
- Query DSL for complex filters
- Run forking and lineage tracking
- Incremental snapshots
- Advanced sharding strategies

### Milestone 11: Performance Phase 2 (Redis Parity)
- Arena allocators and memory management
- Cache-line alignment and SoA transforms
- Lock-free reads (epoch-based/RCU)
- Prefetching and branch optimization
- Target: Millions ops/sec (Redis internal loop parity)

---

## MVP Definition

**MVP = Milestones 1-6 Complete**

At MVP completion, the system should:
1. Store agent state in 5 primitives (KV, Events, StateCell, Trace, RunIndex)
2. Support concurrent transactions with OCC
3. **Achieve Redis-competitive performance in InMemory mode (250K+ ops/sec)**
4. Persist data with WAL and snapshots
5. Survive crashes and recover correctly
6. Replay runs deterministically
7. Run as embedded library (single-node)
8. Scale near-linearly for disjoint keys (multi-thread)
9. Have >90% test coverage

**Not in MVP**:
- Vector store (Milestone 7)
- Network layer (Milestone 8)
- MCP server (Milestone 9)
- Query DSL (Milestone 10)
- Redis internal loop parity (Milestone 11)
- Distributed mode (far future)

---

## Timeline

```
Completed:
- M1 (Foundation)     ✅
- M2 (Transactions)   ✅
- M3 (Primitives)     ✅

Current:
- M4 (Performance)    ← YOU ARE HERE

Remaining:
- M5 (Durability)
- M6 (Replay & Polish)
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
M4 (Performance) ← Current
  ↓
M5 (Durability) ← Depends on M4 durability modes
  ↓
M6 (Replay & Polish)
```

**Note**: M4 introduces durability *modes* (InMemory/Buffered/Strict). M5 adds durability *infrastructure* (snapshots, WAL rotation). M4 must complete before M5 because snapshot behavior depends on durability mode.

---

## Risk Mitigation

### High-Risk Areas
1. **Concurrency (M2)**: OCC bugs are subtle ✅ Mitigated
   - Mitigation: Extensive multi-threaded tests completed
2. **Recovery (M5)**: Data loss is unacceptable
   - Mitigation: Crash simulation tests, fuzzing WAL corruption
3. **Layer boundaries (M3)**: Primitives leaking into each other ✅ Mitigated
   - Mitigation: Mock tests, strict dependency rules enforced
4. **Performance unbounded (M4)**: Optimization work can expand infinitely
   - Mitigation: Red flag thresholds define hard stops; M4 is de-blocking only

### Medium-Risk Areas
1. **Performance targets (M4)**: May not hit 250K ops/sec
   - Mitigation: DashMap + HashMap architecture designed; benchmarks guide work
2. **Replay correctness (M6)**: Determinism is hard
   - Mitigation: Property-based tests, replay verification

### Low-Risk Areas
1. **Foundation (M1)**: Well-understood patterns ✅ Complete
2. **API design (M3)**: Can iterate post-MVP ✅ Complete

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
- M11 target: Millions ops/sec (Redis parity)

---

## Document History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | Initial | Original 5-milestone plan |
| 2.0 | 2026-01-15 | Inserted M4 Performance; MVP now 6 milestones |
