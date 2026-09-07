# Strata-Core: Claude Code Instructions

## Status

The **V1 architecture is the product**. `main` carries the V1 line; the `v1` integration branch has been promoted and retired. Pre-V1 code exists only in git history — do not resurrect it.

Workspace version is inherited from `[workspace.package]` in the root `Cargo.toml` — read it there rather than trusting a number restated here, which is how this line came to claim `1.0.0` two releases after that was true (#3133). Publish names are decided: `stratadb` (embedded-library facade crate) and `strata-cli` (the `strata` binary). All crates remain `publish = false`; note this means the crate the README tells people to depend on is not on crates.io (#3144).

The V1 line is a clean break. No compatibility shims between old and new code, no migration tooling for pre-V1 databases, no parallel old/new paths held alive indefinitely. Opening a pre-V1 database directory fails with `failed_precondition.engine.layout_version`.

## V1 Stack

```text
core
  → storage
  → engine
  → executor / CLI / SDK / Strata AI
  → inference
```

> **`intelligence` is NOT in this stack — milestone M8 is DEFERRED** (decided
> 2026-09-07, #3171). There is no `crates/intelligence` and there never was one
> in the V1 line. The layer was designed, scheduled as M8, and deferred with no
> target release; the design is retained in `intelligence-architecture.md` as
> the starting point if it is ever built (#3136, #3166).
>
> Anything below that reads like a rule about it describes that deferred design,
> not code you can call. Do not write against it.

- **core** — smallest shared atoms (`BranchId`, `CommitVersion`, timestamp, type-local validation errors). No `Value`, no `EntityRef`, no storage transaction IDs.
- **storage** — generic persistence mechanics, L1-L9 layered. Knows nothing about KV/JSON/event/vector/graph semantics.
- **engine** — product semantics, data capabilities, branches, time travel, retrieval, IPC classification, clone artifacts, derived-state manifests. Owns adapter traits used by intelligence.
- **intelligence** — *(designed, not built)* autoembedding, query expansion, reranking, RAG, generation orchestration. The intended shape is: consumes engine surfaces, never imports storage, never speaks provider HTTP. No crate implements it in 1.2.x; inference is invoked directly from executor behind a feature flag.
- **inference** — provider execution and model artifact resolution. `Generator` / `Embedder` / `Reranker` traits. Knows nothing about Strata databases.
- **executor / CLI / SDK / Strata AI** — consume engine. Never import storage directly. (The rule against importing inference directly describes the intended end state; today executor reaches inference behind a feature flag, because the intelligence layer that was to mediate it does not exist.)

## Where To Read Before Working On A Slice

1. **What shipped** — the root `CHANGELOG.md`, or `strata changelog` from any
   build. This is the fastest way to learn what is actually true of the current
   product; the V1 milestone roadmap below is now history (#3154, #3135).
2. **Roadmap** — `docs/architecture/strata-v1-implementation-roadmap.md`, for
   how V1 was structured. The `m1`–`m11` milestones are **complete**; their
   plans live in `docs/architecture/archive/implementation-plans/`. Do not read
   a milestone plan as a description of current work.
3. **Layer architecture** — `docs/architecture/{layer}-architecture.md`
4. **Contracts** — `docs/architecture/engine/<contract>.md` or `docs/architecture/storage/<layer>.md` (docs keep their design-phase names)
5. **Test inventory** — `docs/architecture/v1-existing-test-inventory-and-porting-plan.md`
6. **Engineering standards** — `docs/architecture/v1-engineering-standards.md`
7. **Error contract** — `docs/architecture/v1-error-and-diagnostics-contract.md`
8. **Storage format spec** — `docs/spec/strata-storage-format-v1.md`
9. **Adding/changing a command** — `crates/executor/idl/v1/README.md` (the IDL runbook: authored files, regenerate/verify sequence, coverage guards, SDK wiring)

Architecture docs are authoritative. This file restates only the hard invariants needed during slice work — when in doubt, the contract wins.

## Hard Rules

### Dependency direction (CI-enforced)

```text
core  ← storage  ← engine  ← executor / CLI / SDK
                            ← inference   ←
```

Rules 4, 5 and 10 below describe an `intelligence` layer that **does not
exist** (#3136). They are retained as the intended end state, not as rules a
current PR can violate — there is nothing to violate.

1. Only engine imports storage, and only inside `persistence/`.
2. Engine never imports intelligence or inference.
3. Inference imports nothing from the Strata workspace.
4. Intelligence imports engine and inference only.
5. Executor and CLI consume intelligence; never import inference directly.
6. The dependency DAG is enforced by a workspace guard test on every PR.

### Authority

7. Engine owns semantics. Executor is a thin transport/session adapter.
8. One canonical path per operation. No two public surfaces expose the same behavior.
9. No process-global semantic state. Per-database state only.
10. *(Not in force — the traits do not exist.)* The design has engine owning adapter traits (`QueryExpander`, `ResultReranker`, `RagGenerator`, embedding contracts) that an intelligence layer installs per database. `QueryExpander`, `ResultReranker` and `RagGenerator` appear **nowhere in `crates/`** as of 1.2.x. Do not `use` them; do not write code against them (#3161).

### Storage substrate

11. Branch-aware MVCC KV row is the only physical storage primitive. KV / JSON / event / vector / graph are engine capabilities layered over it.
12. WAL writer halts on fsync failure. Recovery via explicit resume.
13. Codec is uniform across WAL, snapshots, manifest, and table blocks. Durable format is frozen at M3 and gated by golden vectors.
14. Cache mode is non-durable by design — no WAL, manifest, snapshot, checkpoint, durable table, quarantine, or lock objects.

### Branch and capability

15. One canonical `BranchId` lives in core; derivation lives in engine.
16. Branch generations are monotonic, scoped per branch name.
17. Every capability declares lifecycle, branch adapter, search adapter, relationship adapter, and derived-state hooks.
18. Cross-branch references are rejected.
19. Empty branch creation is required.
20. Branch compare, preview-promotion (merge-base and three-way diff, read-only), and **promote (merge)** are present as of the M12 branch-operations work. Promote applies a source branch's changes into a target as a single atomic commit under `Strict` (refuse on conflict with `conflict.engine.promotion` and zero target mutation) or `SourceWins` strategies, recording authoritative promotion lineage on the target. The remaining **mutating** promotion operations — cherry-pick and revert (each writes a new commit) — remain absent in V1. Their absence is enforced by a guard (`crates/engine/tests/branch_merge_absence.rs`), narrowed as each op lands (M12C dropped preview vocabulary, M12D1 dropped merge vocabulary); each remaining op lands with its strict-refusal tests (M12E cherry-pick, M12F revert) and drops its token from the guard when it does.
21. JSON merge is document-level (V1).

### Retrieval and derived state

22. Engine owns deterministic retrieval, recipes, derived-state manifests, and source validation.
23. Intelligence owns model-dependent stages. Engine never calls model providers.
24. Embedding-model mismatch is detected by engine retrieval and surfaces `failed_precondition.embedding_model_mismatch`.
25. Shadow vectors are engine-owned derived rows. Intelligence decides what to embed; engine owns the row.
26. Source rows are authoritative — derived state may accelerate retrieval, never replace it.

### Errors and diagnostics

27. Error codes use `<class>.<area>.<detail>` format. See `v1-error-and-diagnostics-contract.md` for the registry.
28. Public error enums are `#[non_exhaustive]`.
29. Tests assert on error class and code, never on display text.
30. Storage errors do not contain product wording.
31. Provider keys, signed URLs, prompts, and document contents are redacted by default.

### Public surface

32. Engine D4 public surface is documented in `engine-architecture.md`. New public types require reviewer approval.
33. `pub(crate)` by default; `pub` only for D4 items.
34. `unreachable_pub` denies after visibility tightening.
35. Newtypes use `#[repr(transparent)]` + `#[serde(transparent)]` for wire stability.

### Quality

36. `[workspace.lints]` is the single source of truth for lint config.
37. `#![deny(unsafe_code)]` on safe crates: core, storage (above backend FFI), engine, intelligence.
38. Inference denies unsafe outside `local/`; audited unsafe is allowed only inside `local/`.
39. Typed reason enums replace string-factory error methods.

### Cutover

40. No permanent compatibility layer between old storage and new engine, or old engine and new storage.
41. No migration tooling for pre-V1 development databases.
42. Pre-V1 databases are rejected after cutover with structured format/layout errors.
43. Crates shed the `-next` suffix in M9B before V1 promotion to `main`.

## Milestone Nomenclature

Slice codes follow the roadmap:

```text
M{milestone}{epic-letter}{slice-number}        e.g., M3B2
M{milestone}T{test-epic-letter}{slice-number}  e.g., M3TB2
```

Every PR title includes its slice code. Every milestone has both an implementation track (M*) and a test track (M*T); the milestone closes only when both pass their exit gates.

Milestones:

| Code | Title |
|---|---|
| M0 | Architecture freeze and tracking |
| M1 | Core |
| M2 | Storage testkit and crate skeleton |
| M3 | Storage backend, layout, format, durable services |
| M4 | Storage table, branch, commit, recovery, L9 API |
| M5 | Engine persistence adapter and control plane |
| M6 | Engine product semantics |
| M7 | Inference hardening |
| M8 | Intelligence orchestration |
| M9 | Executor, CLI, SDK, tests, benches, docs cutover |
| M10 | V1 readiness hardening |

M7 may run in parallel with M2-M6 once M1 ships (inference has no dependency on storage or engine).

## PR Discipline

Every PR:

1. One slice code in the title (e.g., `M3B2: implement object layout`).
2. One owner per changed behavior.
3. Implementation work and matching test work converge within the milestone.
4. Old competing path deleted or explicitly marked transitional with a deletion condition.
5. PR description states the change class (refactor / cutover / intentional semantic change) and assurance class (S4 / S3 / S2).
6. Tests use error codes and classes, not prose messages.
7. No `let _ = ...`, `.ok()`, or `.unwrap_or_default()` without a rationale comment.
8. Aim for ≤1,500 LOC net change per slice. Split larger slices before opening the PR.

Never:

- Add business logic to executor.
- Add a second public way to do the same operation.
- Keep old and new implementations alive without a cutover boundary.
- Mix unrelated changes or multiple milestones in one PR.
- Add migration machinery for pre-V1 databases.
- Skip the matching test slice.
- Mark items `pub` outside the D4 surface without reviewer approval.

Prefer:

- Authority clarity over flexibility.
- Shorter canonical paths over more options.
- Deletion over documentation of obsolete code.
- Moving semantics into engine over wrapping in executor.
- Explicit enums over boolean-control APIs.
- `pub(crate)` by default.
- Integrated behavioral tests over unit tests of internal helpers.

## Workspace Commands

The V1 branch progressively breaks old commands as milestones land. Use what's available for the milestone you're in:

```bash
# Build (workspace may not build cleanly during transition slices)
cargo build -p strata-core             # M1+
cargo build -p strata-storage          # M2+
cargo build -p strata-engine           # M5+
cargo build -p strata-inference        # M7+
cargo build -p strata-intelligence     # M8+

# Test
cargo test -p <crate>
cargo test -p <crate> --test <integration-target>

# Lint
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

# Feature matrix
cargo hack check -p <crate> --feature-powerset --depth 2

# Conformance harnesses (per milestone)
cargo test -p strata-storage --test format_golden               # M3+
cargo test -p strata-engine --test capability_conformance       # M6+
cargo test -p strata-intelligence --test fake_provider_paths    # M8+
```

Benchmark suites and threshold policy will be re-baselined in M9F/M10D. The old `strata-benchmarks` regression harness still exists but its thresholds apply to the pre-V1 architecture only.

## Out Of V1 Scope

- **Strata Foundry** (SwiftUI macOS app) — on ice during V1. The FFI bridge will be revisited post-V1 once engine APIs stabilize. Do not couple V1 implementation slices to Foundry.
- **Intelligence layer (roadmap M8) — deferred 2026-09-07 (#3171).** Autoembedding,
  query expansion, reranking, RAG and generation orchestration. Designed, never
  built, no `crates/intelligence`, no target release. Design retained in
  `intelligence-architecture.md`.
- Network server mode.
- Cross-machine sync / fleet management. StrataHub V1 substrate is metadata-only; sync is post-V1.
- Migration of pre-V1 development databases.
- OpenAI-compatible on-prem endpoint adapter (vLLM, NIM, Ollama, LM Studio, llama.cpp server) — extension point reserved, adapter post-V1.
- Streaming generation — post-V1 unless product pulls it forward.
- Autosearch optimizer — substrate preserved, optimizer post-V1.
- Follower mode (removed).
- Public manual transaction sessions (removed).
- Disk-backed cache mode (removed).
- Branch bundles — replaced by clone artifacts.
- Tags and notes (removed).
- User-facing `strata compact` / `strata checkpoint` / similar manual maintenance commands (removed).

## Skills

| Skill | When to use |
|-------|------------|
| `/implement` | TDD-driven feature implementation from a GitHub issue |
| `/epic-implement` | Execute slices from a milestone implementation plan |
| `/epic-verify` | Verify slice changes — quick (pre-commit) or full (pre-PR) |
| `/audit-fix` | Fix a bug found during a formal audit (pass the issue number) |
| `/review` | General code review |
| `/ultrareview` | Multi-agent cloud review of the current branch |

## Help And Feedback

- `/help` — get help with using Claude Code.
- Issues — https://github.com/anthropics/claude-code/issues
