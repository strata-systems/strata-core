# EventLog Design Decisions

> **Status**: Awaiting Decision
> **Date**: 2026-01-23
> **Purpose**: Document decision points requiring resolution before EventLog implementation

---

## Decision 1: Stream Sequence Model

### The Question

When a user appends events to different "streams" within a run, should each stream have its own independent sequence numbers, or should all streams share a single global sequence?

### Current State

The Substrate API presents a "streams" concept:
```rust
event_append(run, "orders", payload);     // What sequence?
event_append(run, "payments", payload);   // What sequence?
event_append(run, "orders", payload);     // What sequence?
```

The Primitive has a single log per run with `event_type` field. Substrate maps `stream` → `event_type`.

**Current behavior**: Sequences are global.
```rust
event_append(run, "orders", p1);    // seq = 0
event_append(run, "payments", p2);  // seq = 1  (not 0!)
event_append(run, "orders", p3);    // seq = 2  (not 1!)
```

### Option A: Global Sequences (Current)

All events in a run share one sequence space. "Streams" are logical filters on event_type.

**Semantics**:
```rust
event_append(run, "orders", p1);    // seq = 0
event_append(run, "payments", p2);  // seq = 1
event_append(run, "orders", p3);    // seq = 2

event_range(run, "orders", None, None);
// Returns: [seq=0, seq=2] (gaps in sequence)

event_len(run, "orders");  // Returns 2
event_latest_sequence(run, "orders");  // Returns 2
```

**Pros**:
- Simple implementation (current design)
- Single hash chain for entire run (simpler integrity model)
- Total ordering across all events in a run
- No primitive redesign needed

**Cons**:
- Unintuitive for users expecting Redis Streams semantics
- Gaps in per-stream sequences
- `event_range(stream, 0, 10)` might return fewer than 10 events even if stream has 10+ events
- Documentation burden to explain the model

**Implementation Effort**: None (current state)

---

### Option B: Per-Stream Sequences

Each stream has its own independent sequence space. Streams are truly isolated partitions.

**Semantics**:
```rust
event_append(run, "orders", p1);    // orders:seq = 0
event_append(run, "payments", p2);  // payments:seq = 0
event_append(run, "orders", p3);    // orders:seq = 1

event_range(run, "orders", None, None);
// Returns: [seq=0, seq=1] (contiguous)

event_len(run, "orders");  // Returns 2
event_latest_sequence(run, "orders");  // Returns 1
```

**Pros**:
- Matches Redis Streams, Kafka partition semantics
- Intuitive sequence numbers within each stream
- `event_range(stream, 0, 10)` behaves as expected
- Clean consumer position model per stream

**Cons**:
- Significant primitive redesign required
- Multiple hash chains (one per stream) or no cross-stream integrity
- No total ordering across streams without additional mechanism
- Cross-stream queries become complex
- Higher implementation effort

**Implementation Effort**: High (primitive redesign)

---

### Option C: Hybrid - Global Sequences with Per-Stream Counters

Global sequences for ordering/integrity, but also track per-stream count.

**Semantics**:
```rust
event_append(run, "orders", p1);    // global_seq=0, stream_offset=0
event_append(run, "payments", p2);  // global_seq=1, stream_offset=0
event_append(run, "orders", p3);    // global_seq=2, stream_offset=1

// Primary access by global sequence
event_get(run, "orders", 0);  // Returns event at global_seq=0

// Stream-relative access (new API)
event_get_by_offset(run, "orders", 1);  // Returns 2nd event in orders stream

event_len(run, "orders");  // Returns 2 (stream count)
```

**Pros**:
- Maintains single hash chain
- Provides both global ordering and per-stream counting
- Moderate implementation effort

**Cons**:
- Two numbering schemes to understand
- API complexity
- Still unintuitive if users expect Redis semantics

**Implementation Effort**: Medium

---

### Comparison Table

| Aspect | Option A: Global | Option B: Per-Stream | Option C: Hybrid |
|--------|------------------|---------------------|------------------|
| Redis Streams compatibility | No | Yes | Partial |
| Implementation effort | None | High | Medium |
| Hash chain model | Single (simple) | Multiple (complex) | Single (simple) |
| Total run ordering | Yes | No | Yes |
| Intuitive sequences | No | Yes | Partial |
| Primitive changes | None | Major | Moderate |

---

## Decision 2: Hash Algorithm

### The Question

What hash algorithm should EventLog use for its tamper-evidence chain?

### Current State

Uses `std::collections::hash_map::DefaultHasher`:

```rust
fn compute_event_hash(...) -> [u8; 32] {
    let mut hasher = DefaultHasher::new();
    sequence.hash(&mut hasher);
    event_type.hash(&mut hasher);
    // ...
    let h = hasher.finish();  // u64
    let mut result = [0u8; 32];
    result[0..8].copy_from_slice(&h.to_le_bytes());  // Pad to 32 bytes
    result
}
```

### The Problem

From Rust documentation:
> "The default hashing algorithm is currently SipHash 1-3, though this is **subject to change at any point in the future**. This isn't something you should rely upon."

**Risks**:
1. Hash values may change between Rust compiler versions
2. Hash values may differ between platforms (32-bit vs 64-bit)
3. Chain verification could fail after Rust upgrade
4. Not cryptographically secure (stated as acceptable in M3 docs)

---

### Option A: Keep DefaultHasher

Continue using `DefaultHasher` as-is.

**Pros**:
- No changes needed
- No migration complexity
- Fast (SipHash is optimized)
- No new dependencies

**Cons**:
- Chain verification may break on Rust upgrade
- Not reproducible across different builds
- Only uses 8 bytes of the 32-byte field
- Violates "deterministic replay" principle

**When chain breaks**: If Rust changes DefaultHasher, all existing chains become unverifiable. No recovery path.

---

### Option B: SHA-256

Use cryptographic SHA-256 hash.

```rust
use sha2::{Sha256, Digest};

fn compute_event_hash(...) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(&sequence.to_le_bytes());
    hasher.update(event_type.as_bytes());
    hasher.update(&serde_json::to_vec(&payload).unwrap_or_default());
    hasher.update(&timestamp.to_le_bytes());
    hasher.update(prev_hash);
    hasher.finalize().into()
}
```

**Pros**:
- Deterministic across all platforms and versions
- Cryptographically secure (if needed in future)
- Industry standard
- Full 32-byte hash
- Well-tested implementation

**Cons**:
- Slower than DefaultHasher (~10x, but still microseconds)
- New dependency (`sha2` crate)
- Migration needed for existing data

**Migration Strategy**:
```rust
struct EventLogMeta {
    next_sequence: u64,
    head_hash: [u8; 32],
    hash_version: u8,  // 0 = DefaultHasher, 1 = SHA-256
}
```
- New events use SHA-256 (v1)
- Old events verified with DefaultHasher (v0) if hash_version=0
- Mixed chains: verify each segment with appropriate algorithm

---

### Option C: BLAKE3

Use BLAKE3, a modern high-performance hash.

```rust
fn compute_event_hash(...) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&sequence.to_le_bytes());
    // ...
    *hasher.finalize().as_bytes()
}
```

**Pros**:
- Deterministic
- Very fast (faster than SHA-256, comparable to DefaultHasher)
- Modern design
- Full 32-byte hash

**Cons**:
- Newer, less ubiquitous than SHA-256
- New dependency (`blake3` crate)
- Migration needed
- Less tooling support for verification

---

### Option D: No Hash Chain

Remove hash chaining entirely. Events are just stored by sequence.

**Pros**:
- Simplest implementation
- No algorithm concerns
- Slightly faster appends

**Cons**:
- Loses tamper-evidence feature
- Architectural regression (hash chain was intentional)
- Cannot detect corrupted events

---

### Comparison Table

| Aspect | A: DefaultHasher | B: SHA-256 | C: BLAKE3 | D: No Chain |
|--------|------------------|------------|-----------|-------------|
| Deterministic | No | Yes | Yes | N/A |
| Speed | Fast | Moderate | Fast | Fastest |
| New dependency | No | Yes (`sha2`) | Yes (`blake3`) | No |
| Migration needed | No | Yes | Yes | Yes |
| Cryptographic | No | Yes | Yes | N/A |
| Industry standard | No | Yes | Emerging | N/A |
| Future-proof | No | Yes | Yes | N/A |

---

## Decision 3: Event Payload Type Restriction

### The Question

Should event payloads be restricted to `Value::Object`, or can they be any `Value` type?

### Current State

**Documentation says**: Payloads must be `Value::Object` (from `event.rs` contract comments)

**Implementation does**: Accepts any `Value` type (validation not enforced)

```rust
// Documentation contract:
/// Payload must be `Value::Object`. Empty objects `{}` are allowed.

// Actual behavior:
event_append(run, "stream", Value::String("hello"));  // Succeeds (bug)
event_append(run, "stream", Value::Int(42));          // Succeeds (bug)
event_append(run, "stream", Value::Null);             // Succeeds (bug)
```

---

### Option A: Object Only (Enforce Documentation)

Reject non-Object payloads with `ConstraintViolation` error.

```rust
fn event_append(&self, run: &ApiRunId, stream: &str, payload: Value) -> StrataResult<Version> {
    if !matches!(payload, Value::Object(_)) {
        return Err(StrataError::ConstraintViolation(
            "Event payload must be Object".into()
        ));
    }
    // ...
}
```

**Pros**:
- Consistent with documentation
- Structured data (can always add fields)
- Easier to query/filter by payload fields
- Matches event sourcing best practices
- Self-documenting events

**Cons**:
- Less flexible
- Breaking change if anyone relies on non-Object payloads
- Requires wrapping simple values: `{"value": 42}` instead of `42`

**Example**:
```rust
// Allowed
event_append(run, "user_action", json!({"action": "click", "target": "button"}));
event_append(run, "metric", json!({"name": "latency", "value": 42}));
event_append(run, "empty", json!({}));  // Empty object OK

// Rejected
event_append(run, "simple", Value::Int(42));        // Error
event_append(run, "simple", Value::String("x"));    // Error
```

---

### Option B: Any Value (Flexible)

Allow any `Value` type as payload. Update documentation.

```rust
fn event_append(&self, run: &ApiRunId, stream: &str, payload: Value) -> StrataResult<Version> {
    // No type restriction
    // ...
}
```

**Pros**:
- Maximum flexibility
- No breaking change
- Simple values don't need wrapping

**Cons**:
- Inconsistent event structure
- Harder to add metadata later
- Violates current documentation
- Less structured/queryable

**Example**:
```rust
// All allowed
event_append(run, "stream", json!({"action": "click"}));
event_append(run, "stream", Value::Int(42));
event_append(run, "stream", Value::String("hello"));
event_append(run, "stream", Value::Null);
event_append(run, "stream", Value::Array(vec![1, 2, 3]));
```

---

### Option C: Object or Array

Allow `Object` or `Array`, reject primitives.

**Pros**:
- Allows batch-style payloads `[event1, event2]`
- Still structured

**Cons**:
- Unusual restriction
- Neither fully flexible nor fully strict

---

### Comparison Table

| Aspect | A: Object Only | B: Any Value | C: Object or Array |
|--------|----------------|--------------|-------------------|
| Matches current docs | Yes | No | No |
| Flexibility | Low | High | Medium |
| Structure guarantee | Yes | No | Partial |
| Breaking change risk | Low (bug fix) | None | Low |
| Query/filter capability | High | Low | Medium |
| Event sourcing alignment | Yes | No | Partial |

---

## Decision 4: EventLog's Role in Replay

### The Question

What is EventLog's relationship to the replay system? Is it:
- **Event Sourcing**: EventLog IS the source of truth; all state derives from events
- **Record/Replay**: EventLog captures external inputs for deterministic re-execution

### Context

From `DURABILITY_REPLAY_CONTRACT.md`:
```
P1: replay(run_id) = f(Snapshot, WAL, EventLog)
P3: "The replayed view is computed from EventLog... The EventLog is the source of truth."
```

These statements appear contradictory:
- P1: Replay uses Snapshot + WAL + EventLog (multiple sources)
- P3: EventLog is THE source of truth (single source)

---

### Option A: Event Sourcing Model

EventLog is the ONLY source of truth. All state is derived from events.

**Model**:
```
Agent does: kv.put("key", value)
System does:
  1. event_append("state_change", {"op": "kv_put", "key": "key", "value": value})
  2. Apply to KV store

Replay:
  1. Start with empty state
  2. For each event in EventLog:
     - Apply event to rebuild state
  3. Final state = sum of all events
```

**Implications**:
- EVERY state change must be in EventLog
- KV, StateCell, JsonStore are projections/views
- TraceStore might also be a projection
- Higher write amplification (every op → event + apply)

**Pros**:
- Clean conceptual model
- Complete audit trail
- Can replay to any point in time
- True "source of truth" semantics

**Cons**:
- Significant architecture change
- All primitives must log to EventLog
- Performance impact (2x writes)
- Circular dependency concerns (EventLog append is itself a state change)
- Not current design

---

### Option B: Record/Replay Model

EventLog captures EXTERNAL INPUTS. Internal computation is deterministic and re-executable.

**Model**:
```
Agent execution:
  1. Receive user input → event_append("user_input", {...})
  2. Call external API → event_append("api_response", {...})
  3. Internal computation (deterministic, not logged)
  4. Write to KV (not logged to EventLog)

Replay:
  1. Load snapshot (starting state)
  2. Re-execute agent code
  3. When agent would call external API:
     - Instead, read from EventLog (recorded response)
  4. Internal computation produces same results (deterministic)
  5. Final state matches original
```

**Implications**:
- Only non-deterministic operations logged
- KV, StateCell operations NOT in EventLog
- Agent code must be re-executable
- Snapshot + WAL provides state; EventLog provides external inputs

**Pros**:
- Lower write amplification
- Clear separation of concerns
- Matches "record/replay debugging" pattern
- Closer to current design intent

**Cons**:
- More complex replay logic
- Requires agent code to be available for replay
- Can't replay if agent code changed
- "Source of truth" is split (Snapshot+WAL for state, EventLog for inputs)

---

### Option C: Audit Log Model

EventLog is a supplementary audit log. Not required for replay.

**Model**:
```
State recovery: Snapshot + WAL (traditional database recovery)
Audit trail: EventLog (optional, for compliance/debugging)

Replay:
  1. Recover from Snapshot + WAL
  2. EventLog available for inspection but not required
```

**Implications**:
- EventLog is optional/supplementary
- Recovery works without EventLog
- Simpler model

**Pros**:
- Simplest model
- No replay complexity
- EventLog failures don't break recovery

**Cons**:
- No deterministic replay capability
- Loses key Strata differentiator
- "Source of truth" language is misleading
- Why have EventLog if not for replay?

---

### Option D: Hybrid - Configurable Per-Run

Let the application choose the model per run.

```rust
enum ReplayMode {
    EventSourced,   // All state from events
    RecordReplay,   // Events = external inputs
    AuditOnly,      // Events for audit, not replay
}

run_create(run_id, RunConfig { replay_mode: ReplayMode::RecordReplay });
```

**Pros**:
- Maximum flexibility
- Different use cases served

**Cons**:
- Complexity
- Multiple code paths
- Testing burden

---

### Comparison Table

| Aspect | A: Event Sourcing | B: Record/Replay | C: Audit Log | D: Hybrid |
|--------|-------------------|------------------|--------------|-----------|
| All state in EventLog | Yes | No (inputs only) | No | Configurable |
| Replay without agent code | Yes | No | No | Depends |
| Write amplification | High | Low | Low | Varies |
| Architecture change | Major | Minor | None | Major |
| Conceptual clarity | High | Medium | High | Low |
| Current design alignment | Low | Medium | Medium | Low |

---

## Summary of Decisions Needed

| # | Decision | Options | Default if No Decision |
|---|----------|---------|----------------------|
| 1 | Stream Sequences | A: Global, B: Per-Stream, C: Hybrid | A (current behavior) |
| 2 | Hash Algorithm | A: DefaultHasher, B: SHA-256, C: BLAKE3, D: None | A (current, risky) |
| 3 | Payload Types | A: Object Only, B: Any Value, C: Object/Array | B (current bug) |
| 4 | Replay Model | A: Event Sourcing, B: Record/Replay, C: Audit, D: Hybrid | Unclear |

---

## Final Decisions (2026-01-23)

### Decision 1: Stream Sequences → **Option A (Global)**

**Rationale**: Strata fundamentally cares about *run-level determinism*, not stream-local consumption semantics. A single total order across all nondeterministic inputs is the simplest and strongest invariant for replay, integrity, and debugging.

Per-stream sequences introduce:
1. Loss of natural total order without extra abstraction
2. Pressure to treat EventLog as a messaging system instead of a determinism boundary

**Guidance**:
- Keep global sequences
- Be explicit that streams are filters, not partitions
- Do not promise Redis-like semantics in naming or docs
- If users want per-stream offsets, they can compute them at read time

---

### Decision 2: Hash Algorithm → **Option B (SHA-256)**

**Rationale**: DefaultHasher is unacceptable for something claiming deterministic replay and integrity. "Works today" is irrelevant when failure modes are real and enumerated.

SHA-256 wins over BLAKE3 on ecosystem trust, auditability, and long-term reliability. Performance differences are irrelevant at EventLog scale.

**Guidance**:
- Canonicalize payload serialization explicitly
- Include sequence, stream, timestamp, payload bytes, and previous hash in fixed order
- Version the hash algorithm in metadata
- Do NOT drop the hash chain (Option D is architectural regression)

---

### Decision 3: Payload Types → **Option A (Object Only)**

**Rationale**: This is enforcing an existing contract, not introducing a breaking change. The system is currently lying to itself: documentation promises structure, implementation allows entropy.

EventLog entries are records, not values. Records should always be objects.

If someone wants to log a primitive, the correct representation is:
```json
{ "value": 42 }
```

This buys: extensibility, metadata injection, consistent hashing, queryability.

**Guidance**:
- Treat non-Object payload acceptance as a bug and fix it
- Validate at primitive layer, not just substrate

---

### Decision 4: Replay Model → **Option B (Record/Replay)**

**Rationale**: Event sourcing is the wrong model for Strata.

Strata is not a business ledger, CQRS system, or append-only database. It is a deterministic execution substrate for agents.

That means:
- **Snapshot + WAL define state**
- **EventLog defines nondeterminism**
- **Replay is re-execution, not state reconstruction**

Event sourcing would explode write amplification, entangle primitives, and collapse clean separation between execution and durability.

**Critical Correction Required**:

The phrase "EventLog is the source of truth" must be deleted or rewritten everywhere. It is actively misleading.

**Correct framing**:
> - Snapshot + WAL are the source of truth for **state**
> - EventLog is the source of truth for **nondeterministic inputs**

---

## Architectural Invariants to Encode

These decisions must become enforced invariants, not just documentation:

1. **Assert payload object-ness at the primitive layer**
2. **Assert hash determinism with test vectors**
3. **Assert that replay correctness does not depend on EventLog containing internal state mutations**
4. **Assert transactional coupling between EventLog appends and guarded state changes**

Once these are encoded, EventLog stops being "a log" and becomes what it should be: **a correctness boundary for Strata itself**.

---

## Summary

| Decision | Choice | Key Insight |
|----------|--------|-------------|
| Stream Sequences | **A: Global** | Run-level determinism > messaging semantics |
| Hash Algorithm | **B: SHA-256** | Determinism is non-negotiable |
| Payload Types | **A: Object Only** | Enforce existing contract |
| Replay Model | **B: Record/Replay** | EventLog = nondeterminism boundary, not state |
