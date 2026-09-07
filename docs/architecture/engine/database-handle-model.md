# The database handle model

Status: design (ready for review) · Gates: #3126, #3127, #3128, #3131, #3156,
#3180, #3191 directly; the 24-issue facade and naming cluster indirectly.

## Why this document exists before the facade work

`Database` is an exclusive `&mut self` handle. Twenty branches queue on one
mutex, capability services cannot be held together, readers serialize behind
writers, and no commit spans two capabilities.

That shape is what the published facade (`crates/stratadb`) will be curated
*over*. Designing the front door before deciding the handle model means
designing it twice — and the front door is the surface hardest to change once
it is on crates.io.

The question that had to be answered before anything could be planned: **is a
multi-writer handle a systemic change (2.0) or contained engine work (1.2.x)?**

---

## Part 1 — What the spike found

### Storage is not merely shareable; it is a tuned multi-writer engine

The layer that would have made this expensive is already built. `RuntimeSlot`,
the thing every runtime holds, is a concurrency structure:

```rust
pub(super) struct RuntimeSlot<R> {
    runtime: Arc<ParkingMutex<R>>,
    /// Off-lock read handles (BS2.4): … so a read never takes the runtime lock.
    visible: Arc<AtomicU64>,
    snapshot_registry: Arc<BranchSnapshotRegistry>,
    /// Write-group join queue (BS5.1): contended durable commits enqueue here
    /// and are executed in groups by whichever caller holds the runtime lock.
    commit_groups: CommitGroupQueue,
    /// Covering-fsync chain (BS5.2): serializes the pipelined groups' off-lock
    /// syncs so the device flush stays fat.
    wal_sync: WalSyncChain,
    /// Commits currently blocked on (or about to take) the runtime lock (BS5.3)
    /// — measured at 10-18 µs of writer lock-wait PER COMMIT …
    commit_waiters: Arc<AtomicUsize>,
}
```

Group commit, a covering-fsync chain, off-lock reads, and *measured* writer
lock-wait tuning. This is the BS5 write-concurrency workstream, built and
benchmarked.

Two consequences, both verified by reading the code rather than inferred:

**Reads take no lock at all.** `read_point` → `load_published_snapshot` reads
an `AtomicU64` with Acquire ordering and an `Arc<BranchSnapshotRegistry>`.
There is no `.lock()` on the path.

**Writes are `&self` by design.** `StorageRuntime::commit(&self, …)`, and
`execute_commit` documents why:

> An internally generated base uses the CLAMPING policy: **with concurrent
> writers**, another commit can advance the monotonic floor between this
> pre-lock read and the allocator — ordinary interleaving that must not reject
> the commit.

### The engine adapter throws that away, mostly for a test hook

`guard_fault` — the thing that forces `&mut` on every read — **is already
`&self` in production builds**:

```rust
#[cfg(not(any(test, feature = "testkit")))]
#[allow(clippy::unused_self, clippy::unnecessary_wraps)]
fn guard_fault(&self, _op: FaultOp) -> EngineResult<()> { Ok(()) }
```

Only the test build's variant takes `&mut self`, because `FaultSchedule::take`
mutates a `Vec`. The method signatures across the whole read path were then
written to satisfy the *test* build.

So the shipped read path is exclusive to match a testkit signature. Nothing in
the production build requires it.

`commit` adds exactly one further mutation: `replay_commit_timestamp.take()`,
a one-shot consume belonging to artifact import — a single-writer mode by
construction.

`ControlPlane` is an ordinary in-memory catalog (`BTreeMap<BranchName,
BranchCatalogRecord>`, a default-branch name, a terminal-error latch). A lock
candidate, not a redesign.

The adapter is already 19 `&self` methods against 11 `&mut self`.

### The call

**Contained engine work. 1.2.x, not 2.0.**

Under the project's version tiers, this closes a *gap in a capability the
product already claims*: Strata advertises branch-per-agent concurrency, the
storage engine implements it and has benchmarks for it, and only the engine's
handle types withhold it. Nothing here changes the storage substrate, the
durable format, the commit protocol, or the MVCC model.

The honest framing is not "add concurrency." It is **stop hiding the
concurrency that was already built, measured and tuned.**

---

## Part 2 — The end-to-end design

Five steps. Each is independently shippable and each retires issues on its own.
The ordering is a dependency order, not a priority order.

### Step 1 — The read path becomes `&self`

*Enabler. Zero production behaviour change. Does not retire #3156 by itself —
see the correction under "One PR or several?".*

Give the test build's `FaultSchedule` and `CorruptionSchedule` interior
mutability (`RefCell` is sufficient — they are `#[cfg(any(test, feature =
"testkit"))]` and single-threaded in every current test), so the test variant of
`guard_fault` can also take `&self`. Then relax to `&self`:

```
read, read_row, read_history, scan_prefix, scan_prefix_after_version,
scan_range, scan_immutable_sources, branch_exists, describe_branch,
branch_timeline_head, resolve_wall_clock, committed_at_for_versions
```

**This breaks no caller.** Relaxing `&mut self` to `&self` is a widening: code
holding `&mut Database` still compiles unchanged. The 527 service-acquisition
call sites across the workspace are untouched.

### Step 2 — The commit path becomes `&self`

*Enabler. Unblocks the shared services in step 3.*

The replay timestamps are the only non-test mutation left. Two options:

- **(a)** interior mutability, smallest diff;
- **(b)** move them into the artifact-import driver, which is their honest home
  — they exist only for `crate::artifact` and for #3070's multi-branch import
  ordering.

**Prefer (b)** if it does not disturb #3070's structural-timestamp ordering;
fall back to (a) if it does. Either way `commit` becomes `&self`, and
`Database` can hand out several capability services at once.

Also a widening. No caller breaks.

### Step 3 — Services hold `&'a`, and `Database` hands them out shared

*Retires #3156 and the handle half of #3126. Non-breaking.*

```rust
// before
pub fn kv(&mut self, branch: BranchName, space: ProductSpace) -> EngineResult<KvService<'_>>
// after
pub fn kv(&self, branch: &BranchName, space: &ProductSpace) -> EngineResult<KvService<'_>>
```

`KvService<'a>` changes from `&'a mut StoragePersistence` to `&'a`. `ControlPlane`
goes behind a `RwLock` (branch mutation is rare; reads are frequent).

**This is the breaking step.** Callers passing owned `BranchName`/`ProductSpace`
must pass references. That is the 527-call-site sweep, and it is why this step
is separated from 1 and 2 rather than bundled with them.

### Step 4 — Establish and test `Database: Send + Sync`

*Completes #3126's product claim.*

Steps 1–3 make the *borrow checker* permit concurrent use. They do not by
themselves prove the type is thread-safe. This step adds the `static_assertions`
bounds, resolves whatever is not `Sync` (the `RwLock` from step 3 and any
remaining `RefCell` from steps 1–2 will need `parking_lot` equivalents once
threads are real), and lands a test that actually commits on twenty branches
from twenty threads.

**This is where the throughput claim gets earned or withdrawn.** Storage has BS5
group-commit benchmarks; the engine layer has none. No parallel-throughput claim
should reach the README before this step measures one.

### Step 5 — Cross-capability atomic commit is a separate design

*#3127 is not a handle problem.*

A KV write, a JSON document and an event append produce three commits because
each service builds its own `CommitPlan`, not because the handle is exclusive.
Making the handle shared does not make them atomic.

This needs its own design covering how a plan composes mutations from several
capabilities, which layer owns that composition, and how it interacts with
per-capability derived state. Recommend: after steps 1–4 land, informed by what
they reveal about where plans are assembled.

The same is true of **#3128** (library-opened databases hosting IPC), **#3131**
(mixed put+delete in one commit) and **#3180** (waiting for a prior commit).
Each is worth re-scoping after step 3, since a shared read path plausibly makes
#3128 much cheaper.

---

## Part 3 — One PR or several?

**Several. Four, and they are not equal.**

The deciding fact is that steps 1–2 are *widenings* and step 3 is a *breaking
signature change*. Bundling them would bury roughly forty lines of genuine
semantic content — the interior-mutability decisions, the replay-timestamp
relocation — inside a 527-call-site mechanical sweep. Nobody can review that
honestly, and the repo's own guidance is ≤1,500 LOC of net change per slice.

| PR | Content | Breaking | Size | Retires |
|---|---|---|---|---|
| **H1** | Test-build schedules gain interior mutability; read, scan, history and branch-inspection methods relax to `&self` | No | Small | — *(enabler)* |
| **H2** | Replay timestamps relocated (or made interior); `commit` relaxes to `&self` | No | Small | — *(enabler)* |
| **H3** | `KvService<'a>` and siblings hold `&'a`; `ControlPlane` behind a lock; `Database::kv/json/event/graph` take `&self` — **still with owned names** | No | Medium | **#3156, #3126** (handle) |
| **H4** | Borrowed `&BranchName`/`&ProductSpace`; the call-site sweep | **Yes** | Large, mechanical | #3191 |
| **H5** | `Send + Sync` bounds, twenty-thread commit test, throughput measurement | No | Medium | #3126 (the claim) |

> **Corrected after building H1.** The first draft had four PRs and credited
> H1 with retiring #3156. It does not. `KvService` holds `&'a mut
> StoragePersistence` because `put`/`delete` call `commit`, so the service
> cannot go shared until the commit path does — H1 and H2 are *enablers* that
> deliver nothing user-visible on their own.
>
> Building H1 also revealed a better split. The user-visible win — a shared
> `Database` handing out several services at once — does **not** require the
> breaking name change. `Database::kv(&self, branch: BranchName, …)` keeps its
> owned parameters and breaks no caller. So H3 now delivers the shared handle
> non-breakingly, and the 527-call-site sweep for borrowed names moves to H4
> where it can be reviewed as the purely mechanical change it is.
>
> That matters for sequencing: **the concurrency benefit can ship without ever
> taking the breaking change**, if the borrow-a-name ergonomics turn out not to
> be worth the churn.

Then separate designs for #3127, #3128, #3131, #3180.

**Why not four** (folding the shared-services step into the name sweep): they
were one step in the first draft, and building H1 showed they separate cleanly —
one is a non-breaking win, the other is a breaking sweep. Keeping them apart
means the concurrency benefit is available without committing to the churn.

**Why not three PRs** (folding H1 into H2): they touch the same methods but for
different reasons — H1's is a pure test-harness change with zero production
delta, H2's is a decision about where import state lives. Keeping them apart
means H1 can land immediately and uncontroversially while H2's relocation
question is still being argued.

**Why not five** (splitting H3's sweep from its signature change): the sweep
*is* the signature change. Splitting them leaves the tree uncompilable in
between.

### Suggested review posture

- **H1** — mechanical, safe, land it fast. Reviewer question: does any test
  actually need `&mut` on the schedules across threads?
- **H2** — the one real design decision. Reviewer question: does moving the
  replay timestamps disturb #3070's multi-branch import ordering?
- **H3** — large but boring. Reviewer question: is the call-site diff uniform,
  and does the mutation gate still have coverage on the touched glue?
- **H4** — the claim-earning PR. Reviewer question: does the benchmark show
  parallelism, or only ergonomics? Say whichever is true.

---

## Open questions for the decision

1. **Interior mutability or relocation for the replay timestamps?** (H2's
   central question, above.)
2. **Does `&self` commit buy throughput or only ergonomics?** Storage has BS5
   group-commit benchmarks; the engine layer has none. H4 answers it; until
   then, no claim.
3. **Does `ControlPlane`'s `terminal_error` latch want the same lock as the
   branch catalog?** It is a one-way health latch, read on every call — arguably
   an `AtomicBool` plus a separately-locked error payload.
4. **Does #3128 become cheap after H1?** A library-opened database that can be
   read through `&self` is much closer to hosting a read-only IPC surface.

## Evidence

Read from `main` at `acff6cb4` (v1.2.1):

- `crates/storage/src/api/runtime/background.rs` — `RuntimeSlot` fields:
  group commit, covering-fsync chain, off-lock read handles, lock-wait counter
- `crates/storage/src/api/runtime/mod.rs` — `commit`/`commit_at` are `&self`;
  `execute_commit`'s clamping-policy comment; `read_point` →
  `load_published_snapshot` takes no lock
- `crates/engine/src/persistence/adapter.rs` — `StoragePersistence` fields; the
  two `guard_fault` variants; `commit` and `read_row` bodies; 19 `&self` vs
  11 `&mut self`
- `crates/engine/src/persistence/fault.rs` — `FaultSchedule` is a `Vec`
- `crates/engine/src/control/bootstrap.rs` — `ControlPlane` fields
- `crates/engine/src/data/kv/service.rs` — `KvService<'a>` holds two `&'a mut`
- 527 service-acquisition call sites workspace-wide (`grep` for `.kv(` / `.json(`
  / `.event(` / `.graph(` / `.vector(`)
