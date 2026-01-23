# EventLog: Substrate → Primitive Translation

## Contract Promises (M11_CONTRACT.md)

### Facade API (Section 10.3)

| Operation | Signature | Return | Notes |
|-----------|-----------|--------|-------|
| `xadd` | `(stream, payload: Object)` | `Version` | Returns event ID as Version |
| `xrange` | `(stream, start?, end?, limit?)` | `Vec<Versioned<Value>>` | Read events in range |
| `xlen` | `(stream)` | `u64` | Count of events in stream |

**Payload rules:**
- Empty object `{}` is allowed
- Bytes are allowed in payloads (encoded via `$bytes` wrapper on JSON wire)

**xrange default behavior:** No bounds = all events. Can be expensive for large streams.

### Substrate API (Section 11.5)

```rust
event_append(run, stream, payload: Value::Object) -> Version
event_range(run, stream, start?, end?, limit?) -> Vec<Versioned<Value>>
```

Note: Contract only specifies `event_append` and `event_range`. Additional methods (`event_get`, `event_len`, `event_latest_sequence`) are implementation convenience.

---

## Substrate API Promises

| Method | Signature | Purpose |
|--------|-----------|---------|
| `event_append` | `(run, stream, payload) → Version` | Append event to stream |
| `event_range` | `(run, stream, start, end, limit) → Vec<Versioned<Value>>` | Read events in range |
| `event_get` | `(run, stream, sequence) → Option<Versioned<Value>>` | Get event by sequence |
| `event_len` | `(run, stream) → u64` | Count events in stream |
| `event_latest_sequence` | `(run, stream) → Option<u64>` | Get latest sequence number |

## Primitive Provides

| Method | Signature | Version Exposed? |
|--------|-----------|------------------|
| `append(run_id, event_type, payload)` | `→ Result<Version>` | ✅ Yes (Sequence) |
| `read(run_id, sequence)` | `→ Result<Option<Versioned<Event>>>` | ✅ Yes |
| `read_range(run_id, start, end)` | `→ Result<Vec<Versioned<Event>>>` | ✅ Yes |
| `head(run_id)` | `→ Result<Option<Versioned<Event>>>` | ✅ Yes |
| `len(run_id)` | `→ Result<u64>` | ❌ No |
| `is_empty(run_id)` | `→ Result<bool>` | ❌ No |
| `read_by_type(run_id, event_type)` | `→ Result<Vec<Versioned<Event>>>` | ✅ Yes |
| `verify_chain(run_id)` | `→ Result<ChainVerification>` | N/A |

## Critical Semantic Difference: Streams

### Substrate Model
- Events organized into **named streams** (like Redis XADD/XRANGE)
- Each stream has independent sequence numbers
- Example: `event_append(run, "tool_calls", payload)` → stream "tool_calls"
- Example: `event_append(run, "thoughts", payload)` → stream "thoughts"

### Primitive Model
- Single event log per run (**no stream abstraction**)
- Uses `event_type` field for categorization (metadata, not partition)
- All events share a single sequence space
- Hash chain spans all events regardless of type

### Gap Analysis

This is a **fundamental semantic gap**:

| Aspect | Substrate | Primitive |
|--------|-----------|-----------|
| Partitioning | Named streams | Single log |
| Sequences | Per-stream | Per-run |
| Isolation | Stream-scoped | Run-scoped |

### Resolution Options

**Option A: Treat stream as event_type** (Lossy)
- Map `stream` → `event_type`
- Sequences won't match (shared sequence space)
- `event_range` must filter by type

**Option B: Composite key for streams** (Recommended)
- Namespace keys: `<run_id>:<stream>:event:<seq>`
- Maintain per-stream metadata
- Requires primitive modification or substrate implementation

**Option C: Substrate implements streams over primitive**
- Use event_type for filtering
- Accept shared sequence space
- Document the difference

---

## Type Differences

| Substrate | Primitive | Conversion Needed |
|-----------|-----------|-------------------|
| `&str` (stream) | N/A | Map to event_type or ignore |
| `Value` (payload) | `Value` (payload) | Same type |
| `ApiRunId` | `RunId` | `run.to_run_id()` |
| `Versioned<Value>` | `Versioned<Event>` | Extract payload from Event |

## Return Type Difference

### Substrate Returns: `Versioned<Value>`
```rust
Versioned {
    value: Value,        // Just the payload
    version: Version,
    timestamp: Timestamp,
}
```

### Primitive Returns: `Versioned<Event>`
```rust
Versioned {
    value: Event {       // Full event struct
        sequence: u64,
        event_type: String,
        payload: Value,
        timestamp: i64,
        prev_hash: [u8; 32],
        hash: [u8; 32],
    },
    version: Version,
    timestamp: Timestamp,
}
```

**Translation**: Extract `event.payload` for substrate return.

---

## Method Translations

### `event_append` - SEMANTIC MAPPING

**Substrate**: Appends to named stream, returns sequence in that stream.

**Primitive**: Appends with event_type, returns sequence in global log.

**Translation** (Option C - treat stream as event_type):
```rust
fn event_append(&self, run: &ApiRunId, stream: &str, payload: Value) -> StrataResult<Version> {
    let run_id = run.to_run_id();

    // Validate payload is Object (substrate contract)
    if !payload.is_object() {
        return Err(StrataError::ConstraintViolation(
            "Event payload must be Object".into()
        ));
    }

    // Map stream → event_type
    self.event_log.append(&run_id, stream, payload)
}
```

**Gap**: Sequence numbers are global, not per-stream.

---

### `event_range` - PARAMETER MAPPING + FILTERING

**Substrate**: `(run, stream, start, end, limit)` - inclusive ranges, optional limit.

**Primitive**: `read_range(run_id, start, end)` - exclusive end, no limit, no stream filter.

**Translation**:
```rust
fn event_range(
    &self,
    run: &ApiRunId,
    stream: &str,
    start: Option<u64>,
    end: Option<u64>,
    limit: Option<u64>,
) -> StrataResult<Vec<Versioned<Value>>> {
    let run_id = run.to_run_id();

    // Get total length for unbounded reads
    let total = self.event_log.len(&run_id)?;

    // Convert to primitive parameters
    let range_start = start.unwrap_or(0);
    let range_end = end.map(|e| e + 1).unwrap_or(total); // Inclusive → exclusive

    // Read all events in range
    let events = self.event_log.read_range(&run_id, range_start, range_end)?;

    // Filter by stream (treated as event_type)
    let filtered: Vec<_> = events
        .into_iter()
        .filter(|v| v.value.event_type == stream)
        .collect();

    // Apply limit
    let limited = match limit {
        Some(n) => filtered.into_iter().take(n as usize).collect(),
        None => filtered,
    };

    // Convert Versioned<Event> → Versioned<Value>
    Ok(limited
        .into_iter()
        .map(|v| Versioned {
            value: v.value.payload,
            version: v.version,
            timestamp: v.timestamp,
        })
        .collect())
}
```

**Gap**: Inefficient for large logs (reads all, then filters).

---

### `event_get` - TYPE CONVERSION

**Substrate**: Returns `Versioned<Value>` (just payload).

**Primitive**: Returns `Versioned<Event>` (full struct).

**Translation**:
```rust
fn event_get(
    &self,
    run: &ApiRunId,
    stream: &str,
    sequence: u64,
) -> StrataResult<Option<Versioned<Value>>> {
    let run_id = run.to_run_id();

    match self.event_log.read(&run_id, sequence)? {
        Some(versioned) => {
            // Verify event_type matches stream
            if versioned.value.event_type != stream {
                return Ok(None);  // Different stream
            }

            Ok(Some(Versioned {
                value: versioned.value.payload,
                version: versioned.version,
                timestamp: versioned.timestamp,
            }))
        }
        None => Ok(None),
    }
}
```

---

### `event_len` - FILTERED COUNT

**Substrate**: Count events in specific stream.

**Primitive**: `len()` returns total count (all types).

**Translation**:
```rust
fn event_len(&self, run: &ApiRunId, stream: &str) -> StrataResult<u64> {
    let run_id = run.to_run_id();

    // Must read all events and filter by type
    let total = self.event_log.len(&run_id)?;
    let events = self.event_log.read_range(&run_id, 0, total)?;

    let count = events
        .iter()
        .filter(|v| v.value.event_type == stream)
        .count() as u64;

    Ok(count)
}
```

**Gap**: O(n) scan required. Primitive could provide `len_by_type()`.

---

### `event_latest_sequence` - FILTERED MAX

**Substrate**: Latest sequence in specific stream.

**Primitive**: No per-type tracking.

**Translation**:
```rust
fn event_latest_sequence(&self, run: &ApiRunId, stream: &str) -> StrataResult<Option<u64>> {
    let run_id = run.to_run_id();

    // Scan backwards for efficiency
    let total = self.event_log.len(&run_id)?;
    if total == 0 {
        return Ok(None);
    }

    // Read in reverse order until we find matching type
    for seq in (0..total).rev() {
        if let Some(event) = self.event_log.read(&run_id, seq)? {
            if event.value.event_type == stream {
                return Ok(Some(seq));
            }
        }
    }

    Ok(None)
}
```

**Gap**: O(n) worst case. Primitive could track per-type metadata.

---

## Summary Table

| Substrate Method | Primitive Method | Gap |
|-----------------|------------------|-----|
| `event_append` | `append` | Stream → event_type mapping |
| `event_range` | `read_range` + filter | Must filter post-read, inefficient |
| `event_get` | `read` | Must verify event_type matches |
| `event_len` | `len` + filter | O(n) scan for per-stream count |
| `event_latest_sequence` | N/A | O(n) scan required |

## Gaps Requiring Primitive Enhancement

| Method | What's Needed |
|--------|---------------|
| All stream methods | Per-stream indexing OR accept shared sequence |
| `event_len(stream)` | `len_by_type(event_type)` for efficiency |
| `event_latest_sequence` | `latest_by_type(event_type)` for efficiency |

## Gaps Handled in Substrate

| Method | How Handled |
|--------|-------------|
| `event_append` | Map stream → event_type |
| `event_range` | Filter by event_type post-read |
| `event_get` | Verify event_type matches stream |
| `event_len` | Count filtered results |
| `event_latest_sequence` | Reverse scan until match |

## Design Decision Required

The stream abstraction gap needs a decision:

1. **Accept shared sequences**: Substrate documents that sequences are global, not per-stream.
   - Pro: Simple, no primitive changes
   - Con: Deviates from Redis stream semantics

2. **Implement streams in substrate**: Use composite keys for true per-stream isolation.
   - Pro: Full stream semantics
   - Con: Complex, bypasses primitive

3. **Enhance primitive**: Add stream support to primitive.
   - Pro: Clean layering
   - Con: Significant primitive redesign

**Recommendation**: Option 1 for M11, with clear documentation. Defer stream redesign to future milestone.

## Additional Primitive Capabilities

The primitive has features NOT exposed in substrate:

| Primitive Feature | Description | Substrate Equivalent? |
|-------------------|-------------|----------------------|
| `verify_chain()` | Hash chain integrity | ❌ Not exposed |
| `head()` | Get latest event | `event_latest_sequence` + `event_get` |
| `read_by_type()` | Filter by type | `event_range` with filtering |
| `event_types()` | List distinct types | ❌ Not exposed |
| Hash chaining | Tamper evidence | ❌ Not exposed |

Consider exposing these in substrate for full value.

---

## Contract Gap Summary

### Facade → Substrate: FULLY COVERED

| Facade | Substrate | Status |
|--------|-----------|--------|
| `xadd(stream, payload)` | `event_append(default, stream, payload)` | ✅ |
| `xrange(stream, start, end, limit)` | `event_range(default, stream, start, end, limit)` | ✅ |
| `xlen(stream)` | `event_range(default, stream, None, None, None).len()` | ✅ |

### Substrate → Primitive: SEMANTIC GAP

| Substrate Method | Primitive Support | Gap |
|------------------|-------------------|-----|
| `event_append` | `append()` ✅ | Stream → event_type mapping |
| `event_range` | `read_range()` + filter ✅ | Post-read filtering required |

### Critical Semantic Gap: Streams

**Contract Model:**
- Events organized into **named streams** (like Redis XADD/XRANGE)
- Each stream has independent sequence numbers
- `xrange("tool_calls", ...)` only returns events from that stream

**Primitive Model:**
- Single event log per run (no stream abstraction)
- Uses `event_type` field for categorization
- All events share a single global sequence space

**Resolution for M11:** Treat `stream` as `event_type`. Accept:
- Sequences are global, not per-stream
- `xrange` must filter by event_type post-read (O(n))
- `xlen` must count filtered results (O(n))

**Document this deviation** in contract/SDK docs.

### Performance Considerations

| Operation | Complexity | Notes |
|-----------|------------|-------|
| `xadd` | O(1) | Direct append |
| `xrange` (filtered) | O(n) | Must read all, then filter |
| `xlen` (filtered) | O(n) | Must count filtered results |

**Future Enhancement:** Add `read_by_type()` index to primitive for O(1) stream access.
