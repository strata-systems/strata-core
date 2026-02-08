# Time-Travel Queries

StrataDB supports **time-travel queries** — reading any primitive's state as it existed at a past point in time. This lets you answer "what did the database look like when the agent made that decision?" without manual logging or replaying from scratch.

## How It Works

Every write in StrataDB is timestamped (microseconds since epoch). The storage layer retains a version chain for each key with all historical versions. Time-travel reads scan this chain to find the version that was current at the requested timestamp.

```
Version chain for key "config":
  [v3, ts=1700003000] "production"  ← current
  [v2, ts=1700002000] "staging"
  [v1, ts=1700001000] "development"

get_at(ts=1700002500) → "staging"   (latest version with ts <= 1700002500)
get_at(ts=1700000000) → None        (no version exists at this time)
```

## The `as_of` Parameter

All read commands accept an optional `as_of` timestamp (microseconds since epoch). When provided, the command returns the state as it existed at that time instead of the current state.

```
$ strata --cache
strata:default/default> kv put config development
(version) 1
strata:default/default> kv put config staging
(version) 2
strata:default/default> kv put config production
(version) 3
strata:default/default> kv get config
"production"
strata:default/default> kv get config --as-of 1700002000
"staging"
```

## Supported Primitives

| Primitive | Time-Travel Read | Time-Travel List/Search |
|-----------|-----------------|------------------------|
| **KV Store** | `kv get --as-of` | `kv list --as-of` |
| **State Cell** | `state get --as-of` | `state list --as-of` |
| **Event Log** | `event get --as-of` | `event list --as-of` |
| **JSON Store** | `json get --as-of` | `json list --as-of` |
| **Vector Store** | `vector get --as-of` | `vector search --as-of` |

## Dual Strategy

StrataDB uses two strategies for time-travel, depending on the primitive:

### Version Chain Lookup (KV, State, Event, JSON)

These primitives store all versions in an in-memory version chain. Time-travel reads scan the chain (newest-first) to find the version with timestamp <= the requested time. Cost: O(versions per key).

### Temporal HNSW Filtering (Vector)

Vector search uses the live HNSW index with temporal filtering. Each HNSW node tracks `created_at` and `deleted_at` timestamps. During `search_at()`, the graph traversal filters nodes by liveness at the target time. Cost: O(log n) — same as a normal search, with zero reconstruction overhead.

## Time Range Discovery

Use the `time_range` command to discover the available time window for a branch:

```
strata:default/default> time_range
oldest: 1700001000 (2023-11-14T22:16:40Z)
latest: 1700009000 (2023-11-14T22:30:00Z)
```

This returns the oldest and newest timestamps across all keys in the branch. Querying outside this range returns `None` (for timestamps before the oldest data) or the current state (for future timestamps).

## Edge Cases

- **Timestamp 0**: Treated as "the beginning of time" — returns the oldest available version if one exists.
- **Future timestamps**: Returns the current/latest state (same as a normal read).
- **Deleted keys**: If the key was deleted before the target timestamp, returns `None`.
- **No history available**: Returns `None` (not an error). Data before the last compaction or WAL truncation may be unavailable.
- **Events**: Events are immutable — time-travel simply filters events by timestamp. All events with timestamp <= `as_of` are included.
- **After restart**: Version chains are rebuilt from WAL replay. Versions from before the last snapshot may be lost.

## Use Cases

### Debugging Agent Decisions

When an agent makes a bad decision, inspect the exact state it saw:

```bash
# What did the agent see when it decided to escalate?
DECISION_TIME=1700005000
strata --db ./data kv get agent:context --as-of $DECISION_TIME
strata --db ./data state get agent:status --as-of $DECISION_TIME
strata --db ./data event list tool_call --as-of $DECISION_TIME
```

### Audit and Compliance

Retrieve the exact configuration or state at any historical point:

```bash
# What was the policy at the time of the incident?
strata --db ./data json get policy:access $ --as-of $INCIDENT_TIME
```

### Temporal Vector Search

Find what documents were relevant at a past point in time:

```bash
# What context was available when the agent made its recommendation?
strata --db ./data vector search knowledge "[0.1,0.2,...]" 5 --as-of $DECISION_TIME
```

## Next

- [Primitives](primitives.md) — the six data primitives
- [KV Store Guide](../guides/kv-store.md) — KV time-travel examples
- [Vector Store Guide](../guides/vector-store.md) — temporal vector search
- [Cookbook: Deterministic Replay](../cookbook/deterministic-replay.md) — combining time-travel with replay
