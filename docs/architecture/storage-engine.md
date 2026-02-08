# Storage Engine

The storage engine is the lowest data layer in StrataDB. It provides a concurrent, sharded key-value store that all primitives share.

## ShardedStore

The `ShardedStore` (in `strata-storage`) is backed by `DashMap` — a lock-free concurrent hash map. Data is sharded across multiple segments to reduce contention.

### Key Structure

Every key in storage is a composite of:

```
{branch_id}:{primitive_type}:{user_key}
```

For example:
- `default:kv:user:name` — KV key "user:name" in the "default" branch
- `experiment-1:event:0000001` — Event at sequence 1 in "experiment-1"
- `default:state:status` — State cell "status" in the "default" branch
- `default:json:config` — JSON document "config" in the "default" branch
- `default:vector:embeddings:doc-1` — Vector "doc-1" in collection "embeddings"

This encoding provides:
- **Branch isolation** — keys from different branches never collide
- **Primitive isolation** — KV key "status" and state cell "status" are distinct
- **Prefix scanning** — list all keys in a branch, or all keys of a primitive type in a branch

### Namespace

The `Namespace` type encapsulates the branch-scoped key prefix. When you call `db.kv_put("key", value)`, the executor:

1. Resolves the current branch ID
2. Creates a `Namespace` for that branch
3. Builds the full storage key: `{branch}:kv:key`
4. Writes to the `ShardedStore`

### StoredValue

Values in the store are wrapped in `StoredValue`, which includes:
- The serialized `Value`
- Version metadata
- Timestamp

## MVCC (Multi-Version Concurrency Control)

StrataDB supports versioned reads via `getv()` operations. The storage layer retains version history for keys, allowing you to read the value at a specific version.

Version history is subject to the retention policy — old versions may be trimmed.

### Version Chain Temporal Access

The storage layer also supports **timestamp-based lookups** for time-travel queries:

- `get_at_timestamp(key, max_timestamp)` — scans the version chain (newest-first) to find the version whose timestamp <= the requested time
- `scan_prefix_at_timestamp(prefix, max_timestamp)` — prefix scan with timestamp filtering, returning all keys that existed at the target time

These methods power the `as_of` parameter on all read commands. Since versions are stored newest-first, the scan is efficient for recent timestamps — it stops at the first matching version.

Tombstoned and expired values are filtered out: if the matching version is a tombstone (the key was deleted by that time), `None` is returned.

## Branch Registry

The `BranchRegistry` tracks all known branchs and their metadata. It is consulted during:
- Branch existence checks
- Branch listing
- Branch deletion (cascading key deletion)

## Inverted Index

The `InvertedIndex` indexes text content from KV values, event payloads, and JSON documents. It supports BM25 scoring for keyword search, used by the intelligence layer's hybrid search.

## Thread Safety

`ShardedStore` is fully thread-safe. Multiple threads can read and write concurrently without external synchronization. The DashMap sharding ensures that concurrent writes to different keys have no contention.

## Performance Characteristics

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| Point read | O(1) | Hash lookup |
| Point write | O(1) | Hash insert |
| Prefix scan | O(n) | Scans matching prefix |
| Temporal read | O(v) | Scans version chain; v = versions per key |
| Temporal prefix scan | O(n * v) | Prefix scan + version chain scan per key |
| Branch deletion | O(n) | Scans and deletes all keys in branch |

Where n is the number of keys matching the prefix/branch, and v is the number of versions per key.
