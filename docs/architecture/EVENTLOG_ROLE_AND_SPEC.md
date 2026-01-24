# EventLog: Role, Purpose, and World-Class Specification

> **Status**: APPROVED
> **Date**: 2026-01-23
> **Decisions Locked**: 2026-01-23
> **Purpose**: Define EventLog's role in Strata (architectural commitment)

---

## Executive Summary

Before fixing EventLog's 25+ documented issues, we need clarity on fundamental questions:

1. **What is EventLog FOR in Strata?**
2. **How does it relate to other primitives (especially TraceStore)?**
3. **What does a world-class spec look like for this use case?**

This document proposes answers to these questions.

---

## Part 1: The Confusion

### Current Documentation Says

| Source | Statement |
|--------|-----------|
| M3_ARCHITECTURE.md | "Immutable, append-only event stream for capturing agent actions, observations, and state changes" |
| DURABILITY_REPLAY_CONTRACT.md | "The EventLog is the source of truth" for replay |
| DURABILITY_REPLAY_CONTRACT.md | "replay(run_id) = f(Snapshot, WAL, EventLog)" |

### The Tension

If EventLog is "the source of truth" for replay, does that mean:

**Option A: Event Sourcing Model**
- All state changes derive from events
- KV, StateCell, etc. are projections of events
- Replay = apply events to rebuild state
- EventLog is THE ONLY source of truth

**Option B: Event Logging Model**
- Snapshot + WAL = canonical state (traditional database)
- EventLog = supplementary audit log
- Replay = combine multiple sources
- EventLog is ONE OF several sources

The current architecture appears to be **Option B** (Snapshot + WAL + EventLog), but the language ("source of truth") suggests Option A.

### Overlap with TraceStore

| Primitive | Purpose (from M3_ARCHITECTURE.md) |
|-----------|-----------------------------------|
| EventLog | "agent actions, observations, and state changes" |
| TraceStore | "tool calls, decisions, queries, and thought processes" |

Both capture "what the agent did." What's the difference?

| Aspect | EventLog | TraceStore |
|--------|----------|------------|
| Structure | Generic (event_type + payload) | Typed (ToolCall, Decision, Query, Thought) |
| Hierarchy | Flat (sequence only) | Hierarchical (parent/child spans) |
| Chaining | Hash chain for integrity | No integrity chain |
| Performance | Higher throughput intended | "Optimized for debuggability, not throughput" |
| Indexes | Sequence only | Multiple (type, time, parent) |

---

## Part 2: EventLog's Role (LOCKED)

### The Definitive Answer

**EventLog is the determinism boundary recorder.**

It records everything that would otherwise make replay nondeterministic.

> EventLog defines the determinism boundary for a run by recording all nondeterministic external inputs required to replay execution faithfully.

It is NOT:
- An event sourcing system (all state derives from events)
- A general-purpose message queue
- An audit log for all operations
- Sufficient for replay on its own

It IS:
- The record of nondeterministic inputs enabling deterministic replay
- A tamper-evident chain for integrity verification
- One component of replay (alongside Snapshot, WAL, and deterministic code)

### Source of Truth Clarification

**Critical distinction that must be maintained everywhere**:

| Source | Truth About |
|--------|-------------|
| **Snapshot + WAL** | State |
| **EventLog** | Nondeterministic inputs |

The phrase "EventLog is the source of truth" (without qualification) is **incorrect** and must not appear in documentation. EventLog is the source of **nondeterminism**, not the source of **state**.

### What EventLog Captures

**MUST log to EventLog** (nondeterministic):
- External API responses
- User inputs
- Random values consumed
- Current time when used for logic
- External state reads

**MUST NOT log to EventLog** (deterministic):
- Internal computation results
- KV/StateCell state changes
- Derived values
- Internal decisions (use TraceStore)

**Key Invariant**: The absence of an EventLog entry is a guarantee of determinism. If an operation does not appear in EventLog, it must be reproducible from code alone.

### The Replay Model (Record/Replay)

Replay depends on THREE components, not just EventLog:

```
Replay(run_id):
  1. Load initial state from Snapshot + WAL
  2. Re-execute agent code (must be deterministic)
  3. When agent would make external call:
     - Inject recorded response from EventLog
  4. Internal computation produces identical results
  5. Final state matches original execution
```

This is **record/replay debugging**, not **event sourcing**.

### Relationship to TraceStore

| Primitive | Records | Purpose | When to Use |
|-----------|---------|---------|-------------|
| **EventLog** | WHAT happened (external I/O) | Deterministic replay | Input arrived, API responded |
| **TraceStore** | WHY it happened (reasoning) | Debugging/observability | Decision made, thought formed |

**Critical Distinction**:
- **EventLog entries must be sufficient but minimal.** Every entry is required for replay correctness.
- **TraceStore entries may be verbose, partial, or lossy.** They exist for human understanding, not correctness.

This justifies why EventLog needs hash chaining and strict validation, while TraceStore does not.

---

## Part 3: Strata's 7 Invariants Applied to EventLog

From PRIMITIVE_CONTRACT.md, every primitive must satisfy:

| Invariant | EventLog Compliance | Notes |
|-----------|---------------------|-------|
| I1: Addressable | Yes | run + sequence |
| I2: Versioned | Yes | Version::Sequence |
| I3: Transactional | **Yes (REQUIRED)** | See critical requirement below |
| I4: Lifecycle | Yes (CR only) | Append-only, no update/delete |
| I5: Run-scoped | Yes | All events belong to a run |
| I6: Introspectable | Yes | Can read, check length, verify chain |
| I7: Consistent R/W | Yes | Append is write, read is read |

### I3: Transactional Requirement (CRITICAL)

**Hard Answer**: EventLog append MUST be part of the same transaction as any state mutation it logically guards.

If this is not true, replay has a correctness hole: you could observe external input without atomically committing corresponding state transitions.

**Required Behavior**:
```rust
// CORRECT: EventLog append and state mutation in same transaction
db.transaction(run_id, |txn| {
    let response = external_api_call();
    txn.event_append("api_response", response.clone())?;  // Record nondeterminism
    txn.kv_put("result", process(response))?;             // Guarded state change
    Ok(())
})?;

// WRONG: EventLog append outside transaction boundary
event_log.append(run_id, "api_response", response)?;  // Committed
kv.put(run_id, "result", process(response))?;          // Might fail separately
```

This is not a minor detail. It affects replay fidelity and crash consistency.

### Special Consideration: Append-Only

EventLog is one of two append-only primitives (with TraceStore). This affects:

- **No update/delete**: History cannot be altered
- **No CAS on events**: No optimistic locking on existing events
- **Version = position**: Sequence number is the version

---

## Part 4: World-Class Specification

### Industry Comparison

| Feature | Kafka | Redis Streams | EventStoreDB | Strata EventLog (Current) | Strata EventLog (Proposed) |
|---------|-------|---------------|--------------|---------------------------|---------------------------|
| Append | Yes | Yes | Yes | Yes | Yes |
| Batch append | Yes | Yes | Yes | No | **Yes** |
| Point read | No | Yes | Yes | Yes | Yes |
| Range read | Yes | Yes | Yes | Yes | Yes |
| Reverse range | No | Yes | Yes | No | **Yes** |
| Time-based query | Yes | Yes | Yes | No | **Yes** |
| Stream/topic isolation | Yes | Yes | Yes | **No** (global seq) | **No** (by design) |
| Consumer positions | Yes | Yes | Yes | No | **Yes** |
| Blocking read | Yes | Yes | Yes | No | **Optional** |
| Hash chain | No | No | No | Yes (hidden) | **Yes (exposed)** |
| Retention/trimming | Yes | Yes | No | No | **Future** |

### Streams: Global Sequences (LOCKED)

**Decision**: Streams are logical filters, not isolated logs. Sequences are global within a run.

**Rationale**: Run-level determinism is the invariant. A single total order across all nondeterministic inputs is the simplest and strongest guarantee for replay, integrity, and debugging.

---

**SEMANTIC FOOTGUN WARNING**

Users familiar with Redis Streams or Kafka will expect per-stream sequences. They will be surprised.

**This must be unavoidable in docs and API comments:**

> Streams in EventLog are logical filters, not isolated logs. Sequence numbers are global within a run and must not be interpreted as per-stream offsets.

**Behavior Example**:
```rust
event_append(run, "orders", p1);    // seq = 0
event_append(run, "payments", p2);  // seq = 1  (NOT 0)
event_append(run, "orders", p3);    // seq = 2  (NOT 1)

event_range(run, "orders", None, None);
// Returns: events at seq=0 and seq=2 (gaps in sequence)
```

---

If users want per-stream offsets, they can compute them at read time from the returned events.

---

## Part 5: Proposed Substrate API

### Tier 1: Core Operations (Must Have)

```rust
/// Append a single event
fn event_append(
    &self,
    run: &ApiRunId,
    stream: &str,
    payload: Value,  // Must be Object
) -> StrataResult<Version>;

/// Append multiple events atomically
fn event_append_batch(
    &self,
    run: &ApiRunId,
    events: &[(&str, Value)],  // (stream, payload)
) -> StrataResult<Vec<Version>>;

/// Read events in a range (forward)
fn event_range(
    &self,
    run: &ApiRunId,
    stream: &str,
    start: Option<u64>,
    end: Option<u64>,
    limit: Option<u64>,
) -> StrataResult<Vec<Versioned<Value>>>;

/// Read events in a range (reverse, newest first)
fn event_rev_range(
    &self,
    run: &ApiRunId,
    stream: &str,
    start: Option<u64>,
    end: Option<u64>,
    limit: Option<u64>,
) -> StrataResult<Vec<Versioned<Value>>>;

/// Get a single event by sequence
fn event_get(
    &self,
    run: &ApiRunId,
    stream: &str,
    sequence: u64,
) -> StrataResult<Option<Versioned<Value>>>;

/// Get the most recent event in a stream
fn event_head(
    &self,
    run: &ApiRunId,
    stream: &str,
) -> StrataResult<Option<Versioned<Value>>>;
```

### Tier 2: Metadata & Discovery (Should Have)

```rust
/// Stream metadata (O(1) access)
struct StreamInfo {
    count: u64,
    first_sequence: Option<u64>,
    last_sequence: Option<u64>,
    first_timestamp: Option<Timestamp>,
    last_timestamp: Option<Timestamp>,
}

fn event_stream_info(
    &self,
    run: &ApiRunId,
    stream: &str,
) -> StrataResult<StreamInfo>;

/// List all streams in a run
fn event_streams(
    &self,
    run: &ApiRunId,
) -> StrataResult<Vec<String>>;

/// Count events in a stream (O(1))
fn event_len(
    &self,
    run: &ApiRunId,
    stream: &str,
) -> StrataResult<u64>;

/// Latest sequence in a stream (O(1))
fn event_latest_sequence(
    &self,
    run: &ApiRunId,
    stream: &str,
) -> StrataResult<Option<u64>>;
```

### Tier 3: Time-Based Access (Should Have)

```rust
/// Read events by time range
fn event_range_by_time(
    &self,
    run: &ApiRunId,
    stream: &str,
    start_time: Option<Timestamp>,
    end_time: Option<Timestamp>,
    limit: Option<u64>,
) -> StrataResult<Vec<Versioned<Value>>>;

/// Get events since a timestamp
fn event_since(
    &self,
    run: &ApiRunId,
    stream: &str,
    since: Timestamp,
    limit: Option<u64>,
) -> StrataResult<Vec<Versioned<Value>>>;
```

### Tier 4: Consumer Support (Should Have)

```rust
/// Get consumer's last processed position
fn event_consumer_position(
    &self,
    run: &ApiRunId,
    stream: &str,
    consumer_id: &str,
) -> StrataResult<Option<u64>>;

/// Set consumer's position (checkpoint)
fn event_consumer_checkpoint(
    &self,
    run: &ApiRunId,
    stream: &str,
    consumer_id: &str,
    position: u64,
) -> StrataResult<()>;

/// List all consumers for a stream
fn event_consumers(
    &self,
    run: &ApiRunId,
    stream: &str,
) -> StrataResult<Vec<ConsumerInfo>>;
```

### Tier 5: Integrity (Must Have)

```rust
/// Verify hash chain integrity
fn event_verify_chain(
    &self,
    run: &ApiRunId,
) -> StrataResult<ChainVerification>;

/// Get event with full metadata including hash
fn event_get_with_proof(
    &self,
    run: &ApiRunId,
    sequence: u64,
) -> StrataResult<Option<EventWithProof>>;

struct EventWithProof {
    event: Versioned<Value>,
    prev_hash: [u8; 32],
    hash: [u8; 32],
}
```

### Tier 6: Future (Nice to Have)

```rust
/// Blocking read (wait for new events)
fn event_wait(
    &self,
    run: &ApiRunId,
    stream: &str,
    after: Option<u64>,
    timeout: Duration,
) -> StrataResult<Vec<Versioned<Value>>>;

/// Subscribe to new events (async)
fn event_subscribe(
    &self,
    run: &ApiRunId,
    stream: &str,
    after: Option<u64>,
) -> StrataResult<EventSubscription>;
```

---

## Part 6: Validation Requirements

### Input Validation

| Input | Constraint | Error |
|-------|------------|-------|
| stream name | Non-empty, no NUL, max 1024 bytes | `InvalidKey` |
| payload | Must be `Value::Object` | `ConstraintViolation` |
| payload floats | No NaN, no Infinity | `ConstraintViolation` |
| sequence | Must exist for reads | `NotFound` |

### Semantic Constraints

| Constraint | Behavior |
|------------|----------|
| Append-only | No update or delete operations exist |
| Monotonic sequences | Sequences always increase |
| Hash chain | Each event's hash includes previous event's hash |
| Atomic batch | All events in batch succeed or all fail |

---

## Part 7: Performance Requirements

| Operation | Target Complexity | Notes |
|-----------|-------------------|-------|
| event_append | O(1) amortized | May retry on contention |
| event_append_batch | O(n) where n = batch size | Single transaction |
| event_get | O(1) | Direct key lookup |
| event_range | O(k) where k = result size | Efficient range scan |
| event_len | **O(1)** | Requires per-stream metadata |
| event_latest_sequence | **O(1)** | Requires per-stream metadata |
| event_head | **O(1)** | Requires per-stream metadata |
| event_stream_info | **O(1)** | Requires per-stream metadata |
| event_streams | O(s) where s = stream count | Scan metadata |

---

## Part 8: Architectural Decisions (LOCKED)

These decisions are now architectural commitments. See `EVENTLOG_DECISIONS.md` for full rationale.

### Decision 1: Stream Semantics → **Global Sequences**

Streams are filters, not partitions. All events in a run share one sequence space.

**Rationale**: Run-level determinism is the invariant. Per-stream sequences introduce complexity without benefit for replay.

### Decision 2: Hash Algorithm → **SHA-256**

Determinism is non-negotiable. DefaultHasher may change between Rust versions.

**Implementation**:
- Canonicalize payload serialization
- Fixed field order: sequence, stream, timestamp, payload, prev_hash
- Version hash algorithm in metadata for migration

### Decision 3: Event Payload Types → **Object Only**

Enforce existing documentation. Non-Object acceptance is a bug.

**Implementation**:
- Validate at primitive layer
- Reject with `ConstraintViolation` error
- Wrap primitives as `{"value": x}` if needed

### Decision 4: Replay Model → **Record/Replay**

EventLog captures nondeterministic inputs. Replay is re-execution, not state reconstruction.

**Critical invariant**: Replay correctness MUST NOT depend on EventLog containing internal state mutations.

---

## Part 9: Implementation Priority

Based on this analysis, the priority order is:

### Phase 1: Foundation (Primitive)
1. Per-stream metadata for O(1) operations
2. Deterministic hash algorithm (SHA-256)
3. Input validation at primitive layer

### Phase 2: Core API (Substrate)
1. Input validation enforcement
2. Wire to efficient primitive methods
3. Expose event_head, event_streams, event_verify_chain

### Phase 3: Batch & Time (Primitive + Substrate)
1. Batch append
2. Timestamp index
3. Time-based queries

### Phase 4: Consumer Support (Primitive + Substrate)
1. Consumer position tracking
2. Consumer listing

### Phase 5: Advanced (Future)
1. Blocking reads
2. Subscriptions

---

## Summary

**EventLog's Role**: The determinism boundary recorder. It captures nondeterministic inputs required for faithful replay.

**Source of Truth Clarification**:
- Snapshot + WAL → source of truth for **state**
- EventLog → source of truth for **nondeterministic inputs**

**Key Distinction from TraceStore**:
- EventLog: sufficient but minimal (required for correctness)
- TraceStore: verbose and lossy (for human understanding)

**Locked Decisions**:
1. Global sequences (streams are filters, not partitions)
2. SHA-256 hash algorithm (determinism non-negotiable)
3. Object-only payloads (enforce existing contract)
4. Record/replay model (EventLog ≠ state source)

**Critical Invariants**:
1. EventLog append must be transactional with guarded state mutations
2. Absence of EventLog entry guarantees determinism
3. Replay correctness must not depend on EventLog containing internal state mutations

---

## Document History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-01-23 | Initial RFC |
| 2.0 | 2026-01-23 | Decisions locked. Refined framing: "determinism boundary recorder". Added I3 transactional requirement. Added semantic footgun warning for streams. |
