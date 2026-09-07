# The database handle model

Status: decision draft (spike complete, decision open) · Gates: #3126, #3127,
#3128, #3131, #3156, #3180, #3191 and, indirectly, the 24-issue facade and
naming cluster.

## Why this document exists first

`Database` is an exclusive `&mut self` handle. Twenty branches queue on one
mutex, capability services cannot be held together, readers serialize behind
writers, and no commit spans two capabilities.

That shape is also the thing the published facade (`crates/stratadb`) will be
curated *over*. Designing the front door before deciding the handle model means
designing it twice — and the front door is the surface hardest to change once
it is on crates.io. So this decision is sequenced first, and it is a decision
rather than an implementation.

The question that actually needed answering before anything could be planned:
**is a multi-writer handle a systemic change (2.0) or contained engine work
(1.2.x)?**

## Spike result: the constraint is almost entirely incidental

Storage — the layer with the WAL, MVCC rows, and the durable lock, the layer
that would have made this expensive — **is already shared-reference and already
designed for concurrent writers.**

```rust
// crates/storage/src/api/runtime/mod.rs
pub fn commit(&self, batch: &CommitBatch) -> StorageApiResult<CommitSummary>
pub fn commit_at(&self, batch: &CommitBatch, timestamp: Timestamp) -> …
```

This is not `&self` by accident. `execute_commit` documents the concurrency it
was built for:

> An internally generated base uses the CLAMPING policy: **with concurrent
> writers**, another commit can advance the monotonic floor between this
> pre-lock read and the allocator — ordinary interleaving that must not reject
> the commit.

The exclusivity is imposed one layer up, in the engine adapter, and mostly for
reasons that have nothing to do with correctness under concurrency.

### What actually forces `&mut` today

`StoragePersistence` owns exactly this:

```rust
pub(crate) struct StoragePersistence {
    runtime: StorageRuntime<'static>,          // already &self
    durable: bool,                             // immutable after open
    replay_commit_timestamp: Option<Timestamp>,     // artifact import only
    replay_structural_timestamp: Option<Timestamp>, // import setup only
    #[cfg(any(test, feature = "testkit"))]
    faults: FaultSchedule,                     // TEST ONLY
    #[cfg(any(test, feature = "testkit"))]
    corruption: CorruptionSchedule,            // TEST ONLY
}
```

| Method | Why it takes `&mut self` | Load-bearing? |
|---|---|---|
| `read_row` | `self.guard_fault(FaultOp::Read)?` — nothing else | **No.** Test-only hook |
| `read`, `read_history`, `scan_*` | same, via `read_row` / `guard_fault` | **No.** Test-only hook |
| `commit` | `guard_fault`, plus `replay_commit_timestamp.take()` | **Narrow.** Test hook + a one-shot import mode |
| branch ops | mutate the `ControlPlane` catalog | **Yes**, but it is a `BTreeMap` |
| `close`, `force_creation_durability` | genuine lifecycle transitions | **Yes**, and correctly exclusive |

The adapter is already 19 `&self` methods against 11 `&mut self`.

**The entire read path is `&mut` because of a `#[cfg(test)]` fault injector.**
That is the whole of #3156 — "no `&Database` read path; in-process readers
serialize on the same `&mut` as writers." The cause is a testkit hook, not any
requirement of the storage engine.

`ControlPlane` is likewise ordinary:

```rust
pub(crate) struct ControlPlane {
    default_branch: BranchName,
    branches: BTreeMap<BranchName, BranchCatalogRecord>,
    terminal_error: Option<EngineError>,
}
```

An in-memory catalog mutated on branch create/delete. A lock candidate, not a
redesign.

## The call

**Contained engine work. 1.2.x, not 2.0.**

Under the project's version tiers (2.x = new architecture or systemic change;
1.3 = brand-new feature; 1.2.x = bugs and gaps in existing capabilities), a
multi-writer handle closes a *gap in a capability the product already claims*.
Strata already advertises branch-per-agent concurrency; the storage engine
already implements it. Only the engine's handle types withhold it.

Nothing here proposes changing the storage substrate, the durable format, the
commit protocol, or the MVCC model.

## Proposed shape

Three changes, in dependency order. Each is independently shippable and each
retires issues on its own.

### 1. Make the read path `&self` — retires #3156

Move `FaultSchedule` and `CorruptionSchedule` behind interior mutability. They
are already `#[cfg(any(test, feature = "testkit"))]`, so this costs the
production build nothing and changes no public behavior. Then `read`,
`read_row`, `read_history` and the `scan_*` family become `&self`, and
`Database` gains a shared read path.

Smallest change, largest immediate relief: in-process readers stop queueing
behind writers.

### 2. Make the commit path `&self` — retires #3126, #3191

The two replay timestamps are the only non-test mutation. They are consumed by
artifact import, a mode with a single writer by construction. Interior
mutability (or moving them into the import driver, which is the more honest
home for them) makes `commit` `&self`, and `Database` can then hand out several
capability services at once.

With `&self` services, `kv` / `json` / `event` / `graph` can also take
`&BranchName` / `&ProductSpace` instead of owned values, which is #3191 — a
per-call clone on a value that never changes for the session.

**Open question this does not answer:** whether concurrent writers gain
*throughput* or only ergonomics. Storage admits concurrent commits, but whether
they proceed in parallel or serialize at the memtable is unmeasured. Ergonomics
alone justifies the change; a throughput claim must be measured before it is
made.

### 3. Cross-capability atomic commit — #3127 is a different problem

This one is genuinely harder and should not be folded into the above. It is not
about the handle: it is about `CommitPlan` carrying mutations from more than one
capability, and about which layer composes them. A KV write, a JSON document and
an event append currently produce three commits because each service builds its
own plan — not because the handle is exclusive.

Recommend: separate design, after 1 and 2 land, informed by whatever the
handle work reveals about where plans are assembled.

## What this unblocks

With the handle model decided — even before it is implemented — the facade work
(#3137 and its fifteen satellites) and the naming work (eight issues) can be
designed against a known target: a `Database` that hands out shared services,
takes borrowed names, and reads without exclusivity.

That is 24 issues that can now be designed once instead of twice.

## Open questions for the decision

1. **Interior mutability or restructure?** A `Cell`/`RefCell` around the replay
   timestamps is the smallest diff; moving them into the import driver is the
   cleaner model. The latter is preferred if it does not disturb #3070's
   multi-branch import ordering.
2. **Does `&self` commit buy parallelism?** Unmeasured. Needs a benchmark before
   any throughput claim reaches the README.
3. **Does `ControlPlane` want a `RwLock` or a redesign?** Branch mutation is
   rare and reads are frequent; a `RwLock` is probably right, but the
   `terminal_error` field is a latch that may want different treatment.
4. **Does IPC hosting (#3128) fall out of this?** A library-opened database that
   can be read through `&self` is much closer to being able to host a read-only
   IPC surface. Worth checking whether #3128 becomes cheap once 1 lands.

## Evidence

All findings above were read from `main` at `acff6cb4` (v1.2.1):

- `crates/storage/src/api/runtime/mod.rs` — `commit`/`commit_at` are `&self`;
  `execute_commit`'s clamping-policy comment documents concurrent writers
- `crates/engine/src/persistence/adapter.rs` — `StoragePersistence` fields;
  `commit` and `read_row` bodies; 19 `&self` vs 11 `&mut self`
- `crates/engine/src/control/bootstrap.rs` — `ControlPlane` fields
- `crates/engine/src/data/kv/service.rs` — `KvService<'a>` holds
  `&'a mut StoragePersistence` and `&'a mut ControlPlane`
