# Strata Cloud Sync

**Theme**: Local branch, cloud branch, in sync. Git-style collaboration for database branches.

Strata already has the raw materials: ACID transactions, branch isolation, portable branch bundles, structured diff, transactional merge. What's missing is a sync protocol and a remote to sync with.

This document proposes **StrataHub** — a hosted registry for Strata branches — and the sync protocol that connects local databases to it. Local-first speed. Cloud collaboration. Offline by default, online when you want.

---

## The git analogy (and where it breaks)

| Git | Strata | Status |
|---|---|---|
| Repository | Database | Exists |
| Branch | Branch | Exists |
| Commit | Transaction commit (versioned) | Exists |
| `.git/objects` | Storage engine (KV, JSON, Event, State, Vector) | Exists |
| `git bundle` | `.branchbundle.tar.zst` | **Exists** |
| `git diff` | `branch_diff` (structured per-space diff) | **Exists** |
| `git merge` | `branch_merge` (LWW or strict, transactional) | **Exists** |
| `git push` / `git pull` | **Not built yet** | This proposal |
| GitHub | StrataHub | This proposal |
| `git clone` | **Not built yet** | This proposal |

The analogy holds because branches are the unit of isolation in both systems. Where it breaks: git tracks file-level diffs with content addressing. Strata tracks key-level versions with MVCC. Strata's model is actually simpler for sync because the version counter provides a total order within each branch — no DAG resolution needed.

---

## What already exists

### BranchlogPayload — the replication unit

Every branch export produces a sequence of `BranchlogPayload` records, each representing one committed transaction:

```rust
pub struct BranchlogPayload {
    pub branch_id: String,
    pub version: u64,          // monotonic transaction version
    pub puts: Vec<(Key, Value)>,
    pub deletes: Vec<Key>,
}
```

This is already serialized as MessagePack, CRC32-checksummed, and stored in `.branchbundle.tar.zst` archives. It's compact, deterministic, and portable across machines.

### Branch diff — structured delta

`branch_diff(a, b)` produces a per-space breakdown of added, removed, and modified entries across all primitives:

```rust
pub struct SpaceDiff {
    pub space: String,
    pub added: Vec<BranchDiffEntry>,    // in B, not in A
    pub removed: Vec<BranchDiffEntry>,  // in A, not in B
    pub modified: Vec<BranchDiffEntry>, // in both, different values
}
```

### Branch merge — transactional apply

`branch_merge(source, target, strategy)` applies changes atomically with two strategies:

- **LastWriterWins** — source overwrites target on conflict, appended as new version (MVCC-safe)
- **Strict** — fails if any conflicts exist, returns conflict list, no writes

### WAL with branch IDs

Every WAL record contains a `branch_id` field. The global WAL can be filtered per-branch. WAL segments are files on disk — they can be shipped.

---

## Sync protocol

### Model: explicit push/pull (git-style)

Real-time sync (Figma/Google Docs style) requires continuous connectivity and CRDT complexity. For a database that's embedded-first and offline-capable, git-style explicit sync is the right starting point.

### The origin mirror branch

The missing primitive. For each (remote, branch) pair, the client maintains a local **origin mirror branch** — `_origin:{remote}:{branch}`. This is a read-only local branch that tracks what the remote has. Think `origin/main` in git.

The origin mirror answers two questions:
1. **What's unpushed?** → `branch_diff("main", "_origin:hub:main")` — entries in `main` but not in the mirror
2. **What came from the remote?** → the mirror's contents, which are never pushed back

Remote config is stored as a KV entry in a `_sync` space:

```
KV: _sync:remotes/hub → { "url": "https://stratahub.io/acme/agent-memory", "token": "sk-abc123" }
```

---

## End-to-end walkthrough

No abstractions — just what actually happens at each step.

### Setup

**Agent A** opens a local Strata database and does some work:

```python
db = Strata.open("/agent-a/data", auto_embed=True)

db.kv_put("config", {"model": "gpt-4", "temperature": 0.7})
db.doc_put("notes/research.md", "# Research\n\n## Findings\n...")
db.kv_put("task:1", {"status": "done", "result": "..."})
db.kv_put("task:2", {"status": "done", "result": "..."})
```

At this point, Agent A's local database has 4 writes. The internal MVCC version counter is at, say, version 4. Everything is local, nothing is synced.

### Registering a remote

Agent A decides to sync:

```python
db.remote_add("hub", "https://stratahub.io/acme/agent-memory", token="sk-abc123")
```

This stores the remote config locally and creates `_origin:hub:main` — an empty origin mirror branch. Right now it's empty because we haven't synced yet.

### First push

```python
db.push("hub", "main")
```

**Step 1: Compute the delta.**

Diff local `main` against `_origin:hub:main` (the origin mirror). Since `_origin:hub:main` is empty, the diff is everything — all 4 entries.

```
branch_diff("main", "_origin:hub:main") →
    added: config, notes/research.md, task:1, task:2
    modified: (none)
    removed: (none)
```

**Step 2: Export the delta as BranchlogPayload.**

```rust
export_delta("main", "_origin:hub:main") → Vec<BranchlogPayload> [
    BranchlogPayload { version: 1, puts: [(config, {...})], deletes: [] },
    BranchlogPayload { version: 2, puts: [(notes/research.md, ...)], deletes: [] },
    BranchlogPayload { version: 3, puts: [(task:1, {...})], deletes: [] },
    BranchlogPayload { version: 4, puts: [(task:2, {...})], deletes: [] },
]
```

This is the same serialization format branch bundles already use. MessagePack, CRC32-checksummed.

**Step 3: Compress and upload.**

```
zstd compress → ~2KB blob
PUT https://stratahub.io/acme/agent-memory/main/0001-0004.branchlog
PUT https://stratahub.io/acme/agent-memory/main/cursor.json → { "version": 4 }
```

The hub (a Cloudflare Worker) validates the API key, stores the blob in R2, updates the cursor. It never reads the blob contents.

**Step 4: Update the origin mirror.**

Locally, fast-forward `_origin:hub:main` to match `main`:

```
fork main → _origin:hub:main  (or merge main into _origin:hub:main)
```

Now `_origin:hub:main` is identical to `main`. The diff between them is zero. Nothing unpushed.

### Agent B clones

A second agent on a different machine wants to work on the same project:

```python
db = Strata.open("/agent-b/data", auto_embed=True)
db.remote_add("hub", "https://stratahub.io/acme/agent-memory", token="sk-xyz789")
db.pull("hub", "main")
```

**Step 1: Check what the hub has.**

```
GET https://stratahub.io/acme/agent-memory/main/cursor.json → { "version": 4 }
```

**Step 2: Check what we have locally.**

`_origin:hub:main` doesn't exist yet. We have nothing. Need everything from version 0.

**Step 3: Download.**

```
GET https://stratahub.io/acme/agent-memory/main/0001-0004.branchlog
```

**Step 4: Import into the origin mirror.**

```
import_delta("_origin:hub:main", payloads)
```

This replays the 4 BranchlogPayloads into the `_origin:hub:main` branch. Now Agent B has a local copy of what's on the hub.

**Step 5: Merge origin mirror into main.**

```
branch_merge("_origin:hub:main", "main", LastWriterWins)
```

Agent B's `main` was empty, so this is a simple copy. Now Agent B has all 4 entries locally.

**State after clone:**

```
Agent B's database:
    main:              config, notes/research.md, task:1, task:2
    _origin:hub:main:  config, notes/research.md, task:1, task:2  (identical)
```

### Both agents work independently

**Agent A** (still running):
```python
db.kv_put("task:3", {"status": "done", "result": "..."})
db.kv_put("task:4", {"status": "done", "result": "..."})
```

**Agent B** (on a different machine):
```python
db.kv_put("task:5", {"status": "done", "result": "..."})
db.kv_put("task:6", {"status": "done", "result": "..."})
```

Neither has pushed. Both have unpushed changes that only exist locally.

**Agent A's state:**
```
main:              config, research.md, task:1-4  (6 entries)
_origin:hub:main:  config, research.md, task:1-2  (4 entries, stale)
                   ↑ diff = task:3, task:4 (2 entries ahead of hub)
```

**Agent B's state:**
```
main:              config, research.md, task:1-2, task:5-6  (6 entries)
_origin:hub:main:  config, research.md, task:1-2            (4 entries, stale)
                   ↑ diff = task:5, task:6 (2 entries ahead of hub)
```

### Agent A pushes

```python
db.push("hub", "main")
```

**Step 1: Diff main vs origin mirror.**

```
branch_diff("main", "_origin:hub:main") →
    added: task:3, task:4
```

**Step 2: Export the delta.** Just the 2 new payloads.

**Step 3: Upload.**

```
PUT .../main/0005-0006.branchlog   (Agent A's task:3, task:4)
PUT .../main/cursor.json → { "version": 6 }
```

**Step 4: Update origin mirror.** Fast-forward `_origin:hub:main` to match `main`.

**Hub state after Agent A's push:**
```
R2: acme/agent-memory/main/
    0001-0004.branchlog    (initial push)
    0005-0006.branchlog    (Agent A's second push)
    cursor.json → { "version": 6 }
```

### Agent B tries to push (rejected)

```python
db.push("hub", "main")
```

**Step 1: Check remote cursor.**

```
GET .../main/cursor.json → { "version": 6 }
```

But Agent B's `_origin:hub:main` only knows about version 4. The hub is at version 6 — someone else pushed. Agent B is behind.

**Result: error.** "Remote has changes. Pull first."

This is exactly `git push` being rejected because the remote has diverged.

### Agent B pulls, then pushes

```python
db.pull("hub", "main")
```

**Step 1: Figure out what's new on the hub.**

Agent B's origin mirror is at version 4. Hub cursor is at version 6. Need versions 5-6.

**Step 2: Download the delta.**

```
GET .../main/0005-0006.branchlog    (Agent A's task:3, task:4)
```

**Step 3: Import into origin mirror.**

```
import_delta("_origin:hub:main", payloads)
```

Now `_origin:hub:main` has task:1-4 (all 6 hub entries including Agent A's new ones).

**Step 4: Merge origin mirror into main.**

```
branch_merge("_origin:hub:main", "main", LastWriterWins)
```

Agent B's `main` has: task:1, task:2, task:5, task:6.
Origin mirror has: task:1, task:2, task:3, task:4.

Merge result: task:1, task:2, task:3, task:4, task:5, task:6. No key overlap — clean merge.

**Agent B's state after pull:**
```
main:              task:1-6  (8 entries total)
_origin:hub:main:  task:1-4  (6 entries — hub state)
                   ↑ diff = task:5, task:6 (B's unpushed work)
```

Now Agent B pushes:

```python
db.push("hub", "main")
```

**Step 1: Diff main vs origin mirror.**

```
branch_diff("main", "_origin:hub:main") →
    added: task:5, task:6    ← only Agent B's work, not Agent A's
```

This is the critical part. Because the origin mirror already contains Agent A's changes (we merged them during pull), the diff only contains Agent B's original contributions. No re-uploading Agent A's data.

**Step 2: Export, upload, update origin mirror.**

```
PUT .../main/0007-0008.branchlog   (Agent B's task:5, task:6)
PUT .../main/cursor.json → { "version": 8 }
```

### Conflict scenario

What if both agents write to the **same key**?

Agent A: `db.kv_put("task:1", {"status": "done", "result": "approach A"})`
Agent B: `db.kv_put("task:1", {"status": "done", "result": "approach B"})`

Agent A pushes first (succeeds). Agent B pulls:

```
branch_merge("_origin:hub:main", "main", LastWriterWins)
```

Both modified `task:1`. With **LastWriterWins**, Agent A's version overwrites Agent B's (since it came from the remote and is being merged in). Agent B's local change is lost.

With **Strict** strategy:

```
branch_merge("_origin:hub:main", "main", Strict)
→ Error: conflict on task:1
→ Returns: [BranchDiffEntry { key: "task:1", value_a: "approach A", value_b: "approach B" }]
```

The caller decides: keep mine, keep theirs, or keep both on separate branches.

**Branch-per-agent** avoids this entirely — each agent works on its own branch. No conflicts. Merge at the application level. This is the pattern Strata is built for.

### sync_status

At any point, an agent can check where they stand:

```python
status = db.sync_status("hub", "main")
# → { "ahead": 2, "behind": 0 }
```

This is just `branch_diff("main", "_origin:hub:main")`:
- **ahead** = entries in `main` but not in origin mirror (unpushed local changes)
- **behind** = entries in origin mirror but not in `main` (shouldn't happen after pull, but possible if pull happened without merge)

### The complete picture

```
Agent A (embedded)          StrataHub (R2 + Worker)         Agent B (embedded)
    │                              │                             │
    │  local work                  │                             │  local work
    │  (ACID transactions,         │                             │  (same Strata,
    │   fork/merge, search)        │    blob storage             │   independent)
    │                              │    + API key auth           │
    │                              │    (never opens Strata,     │
    ├── push ─────────────────────►│     never parses blobs)     │
    │   diff main vs origin        │                             │
    │   export delta               ├── store .branchlog in R2    │
    │   upload blob                │                             │
    │   update origin mirror       │                             │
    │                              │                      pull ──┤
    │                              ├── serve .branchlog ────────►│
    │                              │                             │  import into origin
    │                              │                             │  merge into main
    │                              │                             │  (conflicts resolved
    │                              │                             │   HERE, on client)
    │                              │                      push ──┤
    │                              │◄── upload blob ─────────────┤
    │                              │                             │  update origin mirror
    │                              │                             │
    ├── pull ◄─────────────────────┤                             │
    │   import + merge locally     │                             │
```

The origin mirror branch (`_origin:hub:main`) is the key mechanism. It's git's remote tracking branch. It answers two questions:
1. **What's unpushed?** → `diff(main, _origin:hub:main)`
2. **What came from the remote?** → the origin mirror's contents, which are never pushed back

---

## StrataHub

A hosted registry for Strata branches. Like GitHub, but for database state.

### Strata does not need a server mode

This is a deliberate architectural decision. Strata is embedded. Building a server database means building encryption, user management, connection pooling, latency management, caching, wire protocols — an enormous surface area that doesn't serve the core use case (short-lived agent workloads with fork/execute/merge).

StrataHub doesn't need Strata to be a server. It needs **dumb storage + auth**. All the intelligence stays in the client.

The git analogy is exact: git doesn't have a "server mode." GitHub is just blob storage + auth + a web UI. The git client computes diffs, serializes pack files, handles merges. The server never runs `git merge`. Same model here.

### Architecture: dumb hub, smart client

```
Embedded Strata (client)                    StrataHub (storage + auth)
    │                                           │
    ├── export_delta(branch, since_version)     │
    ├── serialize BranchlogPayload[]            │
    ├── zstd compress                           │
    ├── PUT /{project}/{branch}/{v}.branchlog ──► validate API key
    │                                           ├── store blob in R2/S3
    │                                           ├── update cursor metadata
    │                                           │
    ├── GET /{project}/{branch}/cursor ◄──────── return latest version
    ├── GET /{project}/{branch}/{v}.branchlog ◄─ serve blob
    ├── decompress                              │
    ├── deserialize BranchlogPayload[]          │
    ├── import_delta(branch, payloads, strategy)│
    │   (merge happens HERE, on client)         │
    └── update local sync cursor                │
```

The hub never opens a Strata database. Never parses `BranchlogPayload`. Never runs merge logic. It stores blobs and checks API keys.

### What the hub actually is

An object storage bucket with a thin auth layer:

```
R2/S3 bucket: stratahub-data
    │
    ├── acme/agent-memory/
    │   ├── meta.json                    ← project metadata (owner, visibility, created_at)
    │   ├── main/
    │   │   ├── cursor.json              ← { version: 47, updated_at: ... }
    │   │   ├── 0000-0012.branchlog      ← BranchlogPayload versions 0-12
    │   │   ├── 0013-0030.branchlog      ← versions 13-30
    │   │   └── 0031-0047.branchlog      ← versions 31-47
    │   └── experiment-3/
    │       ├── cursor.json
    │       └── 0000-0008.branchlog
    │
    └── bob/research-notes/
        └── main/
            ├── cursor.json
            └── 0000-0150.branchlog
```

**Auth layer**: A Cloudflare Worker, AWS Lambda, or a 200-line stateless proxy. Validates API keys against a key-value store (R2 metadata, DynamoDB, or even a JSON file). No long-running process. No database connection. Scales to zero.

**TLS**: Provided by the cloud provider. Not our problem.

**Encryption at rest**: Provided by R2/S3. Not our problem.

**User management**: API keys stored as hashed values in object metadata. `POST /v1/keys` generates a key, stores the hash. Every request validates against it. No sessions, no cookies, no OAuth (initially).

**Caching**: R2/S3 serves blobs with HTTP caching headers. CDN-friendly. Not our problem.

### What Strata doesn't need to build

| Concern | Who handles it |
|---|---|
| TLS/encryption in transit | Cloud provider (Cloudflare, AWS) |
| Encryption at rest | R2/S3 server-side encryption |
| User management | Stateless auth proxy + API key hash store |
| Connection management | No connections — stateless HTTP |
| Caching | HTTP cache headers + CDN |
| Rate limiting | Cloud provider (Cloudflare rate limiting, API Gateway) |
| Availability | Cloud provider SLA |
| Backups | R2/S3 durability (11 nines) |

Zero of these require Strata to become a server database.

### API

```
# Auth
POST   /v1/keys                                        Generate API key

# Projects
POST   /v1/projects                                    Create project
GET    /v1/projects/:owner/:name                       Get project metadata
DELETE /v1/projects/:owner/:name                       Delete project

# Branches
GET    /v1/projects/:owner/:name/branches              List branches
GET    /v1/projects/:owner/:name/branches/:branch/cursor   Get remote cursor

# Sync (the only endpoints that move data)
PUT    /v1/projects/:owner/:name/branches/:branch/push     Upload delta blob
GET    /v1/projects/:owner/:name/branches/:branch/pull     Download delta since version
GET    /v1/projects/:owner/:name/branches/:branch/bundle   Download full branch bundle
```

### Wire format

Same as branch bundles. The client serializes/deserializes. The hub stores/serves opaque blobs.

```
Delta blob (.branchlog):
    Header: { from_version, to_version, entry_count }
    Body: BranchlogPayload[] (msgpack, length-prefixed, CRC32)
    Compressed: zstd
```

---

## Local API

### CLI

```
strata remote add hub https://stratahub.io/myorg/myproject --token <token>
strata remote list

strata push hub main                    Push main to hub
strata push hub --all                   Push all branches
strata pull hub main                    Pull main from hub
strata pull hub --all                   Pull all branches

strata clone https://stratahub.io/myorg/myproject ./local-dir
strata status hub main                  Show sync status (ahead/behind)
```

### Python

```python
db = stratadb.Strata.open("/data")

db.remote_add("hub", "https://stratahub.io/myorg/myproject", token="...")

db.push("hub", "main")
db.pull("hub", "main")

status = db.sync_status("hub", "main")
# → { "ahead": 3, "behind": 1, "last_synced": "2025-06-03T..." }
```

### Node.js

```javascript
const db = Strata.open('/data');

db.remoteAdd('hub', 'https://stratahub.io/myorg/myproject', { token: '...' });

db.push('hub', 'main');
db.pull('hub', 'main');

const status = db.syncStatus('hub', 'main');
```

---

## Multi-agent collaboration

The real payoff. Branches are already the unit of agent thought. Cloud sync makes them the unit of agent collaboration.

### Pattern 1: Shared workspace

Multiple agents push/pull to the same branch. LastWriterWins keeps it simple.

```
Agent A (local)                    StrataHub                    Agent B (local)
    │                                  │                            │
    ├── kv_put("task:1", done)         │                            │
    ├── push("main") ────────────────► │                            │
    │                                  │ ◄──────────── pull("main") ─┤
    │                                  │                            ├── sees task:1 = done
    │                                  │                            ├── kv_put("task:2", done)
    │                                  │ ◄──────────── push("main") ─┤
    ├── pull("main") ◄──────────────── │                            │
    ├── sees task:2 = done             │                            │
```

### Pattern 2: Branch-per-agent, merge coordinator

Each agent works on its own branch. A coordinator merges results. Zero conflicts.

```
Agent A: push("agent-a")  ──►  StrataHub  ◄──  push("agent-b"): Agent B
                                    │
                                    ▼
                              Coordinator:
                                pull("agent-a")
                                pull("agent-b")
                                merge into "main"
                                push("main")
```

### Pattern 3: Speculative execution with remote evaluation

Fork N branches, run agents on different machines, push results to hub, evaluate centrally.

```
Hub: fork main → plan-a, plan-b, plan-c

Machine 1: pull plan-a → run agent → push plan-a
Machine 2: pull plan-b → run agent → push plan-b
Machine 3: pull plan-c → run agent → push plan-c

Evaluator: pull all → score → merge winner → push main
```

---

## What's NOT in v1

- **Server mode for Strata** — Strata stays embedded. StrataHub is an application that stores blobs, not a Strata server. This is a permanent architectural decision, not a deferral.
- **Real-time sync** — Automatic push on every write. Requires WebSocket/SSE connection. Design for it, don't build it yet.
- **Partial branch sync** — Sync only specific spaces or key prefixes. Full branch is the unit.
- **P2P sync** — Direct sync between two local Strata instances without a hub. Possible but adds complexity. Start with hub-mediated.
- **CRDT merge strategy** — Conflict-free replicated data types for specific value types. Powerful but complex. LWW and Strict cover the common cases.
- **Webhooks/notifications** — Hub notifies agents when a branch changes. Important for real-time patterns but not core sync.

---

## Implementation sequence

### Phase 1: Delta protocol (strata-core)

Add to the engine crate:

- `export_delta(branch, origin_mirror)` → `Vec<BranchlogPayload>` — diff-based incremental export
- `import_delta(branch, payloads)` — replay payloads into a branch
- Origin mirror branch management (`_origin:{remote}:{branch}`) — create, fast-forward after push, update after pull
- Remote config storage in `_sync` space

The delta is computed by diffing `main` against the origin mirror — the same `branch_diff` we already have. The bundle infrastructure already does the serialization.

No networking. Testable locally with two Strata instances.

### Phase 2: HTTP client (new crate: `strata-sync`)

Client-side only. No server.

- HTTP client using `ureq` (already a dependency)
- Push: serialize delta → PUT to hub (R2/S3)
- Pull: GET delta from hub → deserialize → `import_delta`
- Auth: API key in header
- Wire through to CLI (`strata push`, `strata pull`)
- Remote registration stored as KV entries in a `_sync` space

### Phase 3: StrataHub (separate repo)

The dumb hub:

- Cloudflare Worker or small stateless proxy (Rust, Go, or TypeScript)
- R2/S3 for blob storage (delta files + cursor metadata)
- API key auth (key hashes in R2 metadata or a small KV store)
- No long-running process. Scales to zero. Deploy in minutes.

### Phase 4: SDK surface

Wire push/pull/status through Python, Node.js, MCP. Four methods: `remote_add`, `push`, `pull`, `sync_status`.

### Phase 5: Web UI (separate repo)

Static site (or small app) that reads from R2/S3 to browse projects and branches. Can render branch contents by downloading bundles and opening them with embedded Strata (WASM build).

---

## Open questions

- **Conflict UX**: When strict merge fails, how does the caller see conflicts? Structured conflict list? Create a "conflict" branch?
- **Compaction**: Over time, many small delta blobs accumulate. Should the client (or hub) compact them into larger blobs? When?
- **Clone performance**: First clone downloads all deltas for a branch. Should the hub maintain a pre-built full bundle for fast clones?
- **Origin mirror storage cost**: The origin mirror is a full branch copy. For large branches this doubles local storage. Is a lightweight version (e.g., just a version number + hash) sufficient?

---

## Dependencies

- v0.5 (branch operations, branch bundles, diff, merge) — **shipped**
- No dependency on server mode — Strata stays embedded. The hub is cloud storage + auth.
- Format stability is desirable but not blocking — the wire format is `BranchlogPayload` (MessagePack), which can be versioned independently of the storage format.

## Key files (existing)

| Component | File |
|---|---|
| BranchlogPayload | `crates/durability/src/branch_bundle/types.rs` |
| Bundle export/import | `crates/engine/src/bundle.rs` |
| Branch diff/merge | `crates/engine/src/branch_ops.rs` |
| WAL format | `crates/durability/src/format/wal_record.rs` |
| Version types | `crates/core/src/contract/version.rs` |
| Recovery/replay | `crates/durability/src/recovery/coordinator.rs` |
