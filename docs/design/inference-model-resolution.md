# Inference: one resolver for model specs

**Status:** proposed 2026-09-09. Follows the #3124 design
(`inference-developer-experience.md`); nothing here is built. Tracking issue
**#3261** (slices S0–S4 and T). Every file:line below was read on `main` at
`98f8f324`.

The #3222 fix (PR #3259, "a non-provider prefix is a local model name") was a
25-line parser change. Reviewing it produced four new issues (#3255, #3256,
#3257, #3258), and inventorying the path for this document produced a fifth
(#3260). None of the five is a regression from #3259; every one was already
there. That ratio — one fix, five findings — is the signal that the inference
model-resolution path is not a set of bugs but one design gap with many
symptoms, and that fixing it one `/audit-fix` at a time will not converge.

This document does what #3216 asked for on one bounded surface: name the
mechanism that produces the defects, replace it, and leave behind an
*executable* contract that catches the next one.

---

## 1. The principle

> **One question, one answerer.** "Can this model run here — and if not, why?"
> is answered once, as data, and every surface renders that data. No surface
> re-derives the answer from a message string, a command field, or its own
> reading of the model directory.

Three corollaries, each of which today's code violates:

- **Codes come from the answer, not from prose.** An error code is chosen at
  the site that knows why the operation cannot proceed. It is never recovered
  afterwards by matching words in a message (#3216 pattern 2).
- **Surfaces render; they do not resolve.** The CLI's download offer, the
  `capability` payload, `status`, the human error line and the `--json`
  envelope all show the same `ResolvedModel`. None of them owns a second copy
  of the logic.
- **The contract is a table that runs.** The grammar and resolution rules live
  in a matrix test that enumerates spec forms × registry state × key state ×
  build × verb. Prose (architecture rule 14, IDL prose, README) points at the
  matrix; it does not restate cells.

---

## 2. The path today

A model spec reaches an inference call along three layers, and each layer
re-derives part of the answer on its own.

| Layer | What it decides | Where | Notes |
|---|---|---|---|
| spec → provider | `parse_model_spec` | `crates/inference/src/lib.rs:369` | first-colon split; non-provider prefix = local name (#3222) |
| model → availability | runtime + registry | `crates/inference/src/runtime.rs`, `registry/mod.rs` | downloaded? key? feature? — decided per entry point |
| collection → model | engine | `recorded_embedding_model[_at]`, called from `crates/executor/src/executor/vector.rs:223-224` | the `--text` paths never see a spec in the command |

Inside the middle layer, the runtime's public entry points do not agree with
each other about the first two questions:

| Entry point (`runtime.rs`) | parses spec? | directory it reads | existence check | key check |
|---|---|---|---|---|
| `capability` :415 | yes | `self.registry()` → honours `config.models_dir` | `registry.info()` (None on miss) | provider readiness |
| `status` :370 | per-provider | `self.registry()` | list_local | `provider_is_ready` :1293 |
| `list_models` / `list_local_models` :318/:323 | no | `self.registry()` | — | — |
| `pull_model` :328 | **no** (#3255) | `self.registry()` | `resolve_or_pull(raw string)` | — |
| `generate` / `chat` :466/:523 | yes | **`ModelRegistry::new()`** in `generate.rs:164` — ignores `config.models_dir` (#3260) | `registry.resolve` | env only, `lib.rs:511` |
| `embeddings` / `embed[_batch]` :588/:655/:704 | yes | **`ModelRegistry::new()`** in `embed.rs:82` (#3260) | `registry.resolve` | env only, `lib.rs:589` |
| `rank` :763 | yes | **`ModelRegistry::new()`** in `rank.rs:75` (#3260) | `registry.resolve` | env only |
| `tokenize` / `detokenize` :618/:639 | yes | as generate | as generate | — |
| `unload` :791 | yes | — | — | — |

So `capability` and `status` can report a model as present and runnable from
one directory while `generate` loads from another; `pull` looks up
`local:qwen3:1.7b` verbatim and fails on a string every other verb accepts.

The registry itself has one "downloaded" predicate (`model_file_is_downloaded`,
`registry/mod.rs:39`: regular file, non-empty) and one catalog
(`registry/catalog.rs:6`, 16 entries, case-insensitive `find_entry` :288). Those
are sound. What is missing is a single caller that composes them.

### 2.1 How a refusal gets its code

`InferenceError` (`crates/inference/src/error.rs:9`) has eight variants. Six are
`String`s; two are typed (`RegistryFailed { kind }`, `ProviderFailed { kind }`,
added by #3217). Construction sites on `main`:

| Variant | shape | sites |
|---|---|---|
| `Provider(String)` | string | 99 |
| `NotSupported(String)` | string | 63 |
| `LlamaCpp(String)` | string | 39 |
| `Registry(String)` | string | 34 |
| `InvalidSpec(String)` | string | 8 |
| `ProviderFailed { kind, .. }` | typed | 6 |
| `RegistryFailed { kind, .. }` | typed | 1 |
| `Io(String)` | string | 0 (#3252) |

For the string variants, `code()` (`error.rs:335`) recovers the code by
substring-matching the message: `registry_code` :435 maps "unknown model" and
"not found locally" to `inference.missing_model`, "download" to
`download_failed`, anything else to `registry_corrupt`; `not_supported_code`
:452 maps the word "provider" to `unsupported_provider`. The consequence is
visible in product code — a comment in `runtime.rs:1350-1356` explains that the
refusal text for a lean build must avoid the *word* "provider" so that the
classifier does not silently change its code. Message wording is load-bearing;
`provider_classification.rs` pins twenty-eight such checks.

This is exactly why #3256 exists: a catalog miss (`Registry("Unknown model
…")`, `registry/mod.rs:396`) and a catalogued-but-not-downloaded model
(`RegistryFailed { MissingModel }`, :245) are different facts that arrive at
the same code, because one of them was classified from prose.

### 2.2 What reaches the wire

`From<InferenceError> for ExecutorError` (`crates/executor/src/error.rs:521-543`)
takes `value.code()`, looks up the registry row (single authority since #3243)
and passes **`Vec::new()` for `details`**. Every inference error on the wire
carries zero structured details. The schema name
`strata.error.details.inference.v1` is declared on every row
(`error_registry.rs:16`, rows :450-626) and defined nowhere — a contract with no
implementation (#3216 pattern 1).

The CLI's download offer (`crates/cli/src/lib.rs:957-996`) therefore has to
reconstruct "why unavailable" itself: it compares the code to the literal
`b"inference.missing_model"` (:1019-1021) and takes the model name from the
command's own `model` field for five `Inference*` variants (:1024-1035). It
cannot serve `vector --text` (the model is in the collection record, #3226); it
offers names that are not in the catalog and could never be pulled (#3256);
and it re-issues the spec verbatim to `pull`, which does not parse it (#3255).

### 2.3 What the test lanes reach

`crates/executor/idl/v1/unreplayed-error-codes.yaml:53-72` lists **every**
`inference.*` code as unreplayed, with the reason "a live inference provider".
That is true for the nine `provider_*` codes. It is not true for
`missing_model`, `missing_api_key`, `invalid_request`, `unsupported_provider`,
`unsupported_operation` and `download_disabled`: those are resolution-time
refusals that need no network, no model file and no key — they need a resolver
that can be pointed at an empty temp directory. They are unreplayed because the
replay lane's `FakeInferenceService` (`idl_tooling/verify.rs:37-42`) fakes
*resolution* along with execution: it keys off fixed names (`fake-embed`,
`fake-generate`, `fake-rank`, `testkit.rs:374`) and never touches the parser or
the registry. The IDL's drift guards stop exactly where resolution begins.

---

## 3. The defects are symptoms

Grouped by the mechanism that produces them. Fixing a row without its root
leaves the root to produce the next row.

### Root A — no single resolver

| Issue | Symptom | Where the re-derivation lives |
|---|---|---|
| #3222 (fixed, #3259) | `qwen3:1.7b` rejected as unknown provider | parser guessed at catalog membership |
| #3255 | `pull local:qwen3:1.7b` fails `missing_model` | `pull_model` skips `parse_model_spec` (`runtime.rs:328`) |
| #3260 | `capability` says present, `generate` says missing | three loaders build their own `ModelRegistry::new()` |
| #3226 | no download offer for `vector --text` | CLI reads the model off the command, not the answer |
| #3221 | library callers get `missing_api_key` for a configured key | key lookup is env-only inside inference; the config bridge is a CLI-side `set_var` (`cli/src/lib.rs:1054-1066`) |

### Root B — the code carries less than the resolver knew

| Issue | Symptom | Cause |
|---|---|---|
| #3256 | D8 offers to download names that are not in the catalog; refusal names `strata models list` (no such verb) | catalog-miss and not-downloaded share `missing_model`; hint is class-generic; verb spelled in prose |
| #3252 | `inference.io_failure` declared, documented, never produced | code exists in the registry, no site can raise it |
| #3216 p.2 | wording changes change codes | string variants + substring classifiers |
| (this doc) | zero `details` on every inference error | `From<InferenceError>` passes `Vec::new()`; schema name has no definition |

### Root C — facts restated by hand

| Issue | Restated fact | Copies |
|---|---|---|
| #3257 | spec grammar | rule 14 (`inference-architecture.md:215-230`) and closing item 2 (:769) say **case-sensitive** and **whitespace invalid**; `parse_model_spec` trims (`lib.rs:370,380`) and `ProviderKind::from_str` lowercases (`lib.rs:220`); `api_contract.rs:256` pins the lenient behaviour |
| #3235 | model size | CLI divides by `1_048_576` and prints "MB" (`render.rs:413,446`, test :1681); inference `format_size` is decimal (`registry/mod.rs:445`); same file prints 638.9 vs 670 |
| #3250 | the 16-code embed error set | hand-copied into `inference.embed`, `vector.upsert`, `vector.query`; aligned only by an after-the-fact test |
| #3233 | bare `strata inference …` | README :137-157 shows four bare commands; none runs (`invalid_argument.cli.no_database`); `provider-api-keys.md` quietly uses `--cache` |
| #3045 | catalog `hf_repo` | two reranker repos do not exist; nothing checks |
| #3224 | Homebrew formula name | `strata-local` named in a refusal, absent from the tap |
| — | `api_key_env_var(Local) => "STRATA_LOCAL_API_KEY"` (`lib.rs:408`, "unused, but complete") | a variable nothing reads, presented as a fact |
| — | `docs/product/strata-v1-cli-sdk-experience.md:373,392,483` | `strata models pull` — verb without its `inference` segment |

### Root D — the gates that should have caught A–C are blind here

- Mutation gate: #3225 (exit-3 precedence), #3227 (`Result` alias unviable),
  #3254 (`local`-gated code in mixed files), #3258 (non-`Default` enum arms),
  #3220. A diff to `parse_model_spec` under the default lane sees the `local`
  arms as unreachable.
- Replay lane: §2.3 — no inference code is replayed.
- Doc gates: `check-docs` proves generated docs match the IDL; nothing checks
  that a command named in README, a design doc, or a registry hint parses.

---

## 4. Decisions this proposes

| # | Decision | Closes / enables |
|---|---|---|
| R1 | One `resolve(spec, task) -> ResolvedModel` in `strata-inference`; `Availability` is a typed enum; `require_ready()` is the only place an availability becomes an error | A: #3255, #3260; the substrate for everything below |
| R2 | Split `inference.missing_model` (catalogued, not downloaded) from a new `inference.unknown_model` (not in catalog / path absent) | #3256 |
| R3 | Inference errors carry `details` under `strata.error.details.inference.v1`, defined by one Rust type; `capability` returns the same data; D8 reads details, not codes and command fields | #3226, #3256, #3216 p.1, #3244 (data not sentinel) |
| R4 | Provider keys come from an injected `ProviderKeySource`; executor installs env-then-config; the CLI `set_var` bridge is deleted | #3221, rule 9 |
| R5 | One `ModelRegistry`, built once in `InferenceRuntime::new`, threaded into every loader | #3260 |
| R6 | `pull_model` goes through the resolver; a cloud spec is `unsupported_operation`; network-disabled is the typed `DownloadDisabled` | #3255 |
| R7 | One size formatter (decimal), exported from inference, used by the CLI | #3235 |
| R8 | Prose derives or points: rule 14 becomes a two-sentence grammar plus a pointer to the matrix; commands named in docs and hints must parse; catalog repos are checked nightly | #3257, #3233, #3045, `strata models pull` |
| R9 | Bare `strata inference <verb>` (all but `install-local`) opens an ephemeral cache connection when no database target is given | #3233 |

### R1 — the resolver

```rust
// crates/inference/src/resolve.rs (new)

pub struct ResolvedModel {
    /// The spec as given, outer whitespace removed.
    pub spec: String,
    pub provider: ProviderKind,
    /// Provider-side model name, catalog name, or path — after provider parsing.
    pub name: String,
    pub source: ModelSource,
    pub availability: Availability,
}

pub enum ModelSource {
    Catalog { entry: &'static CatalogEntry, variant: &'static QuantVariant },
    GgufPath(PathBuf),
    Cloud,
}

/// Why the model can or cannot be used for `task` by this binary, right now.
/// Adding a variant forces a decision in `require_ready` and in the wire
/// rendering — the compiler is the guard.
pub enum Availability {
    Ready,
    NotDownloaded { pull_spec: String, size_bytes: u64 },
    NotInCatalog,
    PathMissing,
    LocalExecutionNotBuilt,
    ProviderNotBuilt,
    TaskNotSupported { task: ModelTask },
    KeyMissing { env_var: &'static str, config_key: &'static str },
}

impl InferenceRuntime {
    /// Errs only for a malformed spec (`inference.invalid_request`). Every
    /// other outcome is data.
    /// `task` is `None` for task-neutral verbs (`pull`, `unload`).
    pub fn resolve(&self, spec: &str, task: Option<ModelTask>) -> Result<ResolvedModel, InferenceError>;
}

impl ResolvedModel {
    /// Total match over `Availability` → typed `InferenceError`.
    pub fn require_ready(&self) -> Result<(), InferenceError>;
}
```

`resolve` composes what already exists: `parse_model_spec`, `find_entry` /
`find_entry_by_parts`, `model_file_is_downloaded`, `looks_like_path`, the
`cfg!` feature checks, and the key source (R4). It reads the catalog as an
input (`&[CatalogEntry]`, default `CATALOG`) so that a test or the testkit fake
can supply a small one. Every runtime entry point in §2 calls `resolve` first;
`capability` returns it; the loaders receive it and never look anything up
again.

`resolve` is a pure function of (spec, task, catalog, directory listing, key
source, build). That is what makes the matrix in §5 possible.

### R2 — two codes for two facts

| Availability | Code | Class / retry | Hint (registry, class-generic) |
|---|---|---|---|
| `NotDownloaded` | `inference.missing_model` (kept; meaning narrowed) | FailedPrecondition / AfterStateChange | "Pull the model before retrying." |
| `NotInCatalog`, `PathMissing` | **`inference.unknown_model`** (new) | NotFound / Never | "Check the model name against the catalog." |
| `LocalExecutionNotBuilt` | `inference.unsupported_operation` (as today) | Unsupported / Never | unchanged |
| `ProviderNotBuilt` | `inference.unsupported_provider` (as today) | | unchanged |
| `TaskNotSupported` | `inference.unsupported_operation` (as today) | | unchanged |
| `KeyMissing` | `inference.missing_api_key` (as today) | | unchanged |

`unknown_model` is a wire change: `errors.yaml`, a registry row, the IDL error
sets (via the named set #3250 proposes, so it is added once), a CHANGELOG line,
and a `/e/unknown_model` page at the next release. Whether a missing GGUF path
is `unknown_model` or `model_load_failed` is open (§10); the recommendation is
`unknown_model`, because nothing was loaded.

The typed errors are constructed at the raise site with their kind. The
substring classifiers in `error.rs` keep working for the string variants the
resolver does not own (llama.cpp and provider HTTP paths); the resolver never
produces a string variant, and `provider_classification.rs` gains no new
cases. Retiring the classifiers for the remaining ~230 sites is #3216's
second step, not this document's.

### R3 — the answer travels as data

```rust
// crates/inference/src/resolve.rs
/// The wire shape of `Availability`. This type *is* the definition of
/// `strata.error.details.inference.v1`.
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct AvailabilityDetails {
    pub model: String,
    pub provider: ProviderKind,
    pub availability: AvailabilityKind,          // snake_case string on the wire
    #[serde(skip_serializing_if = "Option::is_none")] pub pull_spec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")] pub key_env_var: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")] pub config_key: Option<&'static str>,
    /// Set by the executor when the spec came from a collection record.
    #[serde(skip_serializing_if = "Option::is_none")] pub collection: Option<String>,
}
```

- `From<InferenceError> for ExecutorError` converts `AvailabilityDetails` into
  `Vec<ErrorDetail>` (`engine/src/diagnostics/error.rs:100`). Keys are the
  struct's field names; a test serializes every `Availability` variant and
  asserts the key set, so the schema finally has a definition and a guard.
- `InferenceCapability` (`runtime.rs:150`) gains `availability` and the
  same optional fields. `capability` and a refusal show the same answer; the
  `can_*` booleans are derived from it.
- D8 (`cli/src/lib.rs:957-996`) deserializes the details through the
  executor's re-export of `AvailabilityDetails`, offers only when
  `availability == NotDownloaded`, and pulls `pull_spec`. No literal code
  compare (`is_missing_model` :1019 goes), no `missing_model_spec` (:1024
  goes), and `vector upsert/query --text` are covered for free because the
  model arrived in the answer, not the command (#3226). Names not in the
  catalog are never offered (#3256).
- The vector `--text` path (`executor/src/executor/vector.rs:207-228`) adds
  `collection` to the details so the refusal says which record the spec came
  from.

The registry hints stay class-generic (one string per code, no interpolation).
The one surface that types a concrete verb is the CLI, which owns its verb
spelling; §R8 makes that spelling parse-checked.

### R4 — keys are looked up, not copied into the environment

```rust
// crates/inference/src/lib.rs
pub trait ProviderKeySource: Send + Sync {
    fn key(&self, provider: ProviderKind) -> Option<ProviderKey>; // value + source label
}
pub struct EnvKeySource;                       // default: OPENAI_API_KEY etc.
impl InferenceRuntimeConfig { pub key_source: Arc<dyn ProviderKeySource> }
```

- Inference imports nothing from the workspace (rule 3), so it cannot read
  `~/.config/strata/config.toml`. Executor imports `strata-hub` behind its
  `hub` feature (`executor/Cargo.toml:22,61`), so executor installs
  `EnvThenConfig { env: EnvKeySource, config: strata_hub::read_global_provider_key }`
  under `#[cfg(all(feature = "inference", feature = "hub"))]`. Env still wins
  (D5).
- `load_provider_keys_into_env` (`cli/src/lib.rs:1054-1066`), the
  `std::env::set_var` at :1061, `config_backed_keys` (`context.rs:21-42`) and
  `name_config_key_sources` (:1080-1094) are deleted. `status` reports the key
  source itself because the source told it. `set_var` is process-global state
  (rule 9) and becomes `unsafe` in edition 2024; this removes the last one on
  the inference path.
- Every executor caller — CLI, IPC, MCP, wasm-less SDK paths — gets
  config-file keys (#3221). `crates/stratadb` (the embedded facade) has no
  inference surface at all (`stratadb/Cargo.toml:15` depends on engine only),
  so #3221 is closed for every path that exists; the facade is stated plainly
  as out of scope rather than half-served.
- Tests inject a `NoKeys`/`FixedKeys` source instead of mutating process
  environment — the matrix's key dimension becomes hermetic.

### R5, R6, R7 — mechanical

- R5: `InferenceRuntime::new` builds `registry: ModelRegistry` once from
  `config.models_dir` (else `STRATA_MODELS_DIR`, else `~/.strata/models`);
  `GenerationEngine::from_registry_with_config`, `EmbeddingEngine::from_registry`,
  `RankingEngine::from_registry` take `&ModelRegistry` (or the `ResolvedModel`)
  and the three `ModelRegistry::new()` calls in `generate.rs:164`,
  `embed.rs:82`, `rank.rs:75` are deleted (#3260).
- R6: `pull_model(spec)` → `resolve(spec, None)`; `Cloud` →
  `unsupported_operation` (declared on `inference.models.pull`, which today
  lists only the download/registry codes, `inference.yaml:30-36`);
  `NotInCatalog` → `unknown_model`; `NotDownloaded` → download; `Ready` →
  return the path. Network disabled → `RegistryFailed { DownloadDisabled }`
  instead of `NotSupported("… network access")` classified by the word
  "network" (#3255).
- R7: `format_size` (`registry/mod.rs:445`, decimal MB/GB) becomes `pub`,
  re-exported by executor as `format_model_size`; `render.rs:413-446` uses it
  and the `"1.0 MB"` assertion at :1681 moves to the one formatter (#3235).

### R8 — prose derives or points

- Rule 14 and closing item 2 in `inference-architecture.md` shrink to: the
  grammar (`provider:model`, first colon, non-provider prefix = local name in
  full, task from the operation), the leniency decision (§10 Q2), and *"the
  authoritative behaviour is `crates/inference/tests/resolution_matrix.rs`; this
  rule does not restate cells"* (#3257).
- `prose/commands/inference.capability.md:6` remains the one hand-written list
  of spec forms; each form it names is a matrix row, and `check-docs` carries it
  to the generated docs.
- A `cli` test extracts every `strata …` invocation from `README.md`,
  `docs/inference/*.md`, `docs/design/inference*.md`, the registry hints and
  the D8 prompt, and runs it through the real clap parser
  (`Cli::try_parse_from`). It catches `strata models pull`, and it makes the
  #3124 rule *"every command named on this surface must exist"* a guard
  instead of a sentence.
- Catalog: a net-gated nightly test issues a `HEAD` for every `hf_repo` /
  `hf_file` in `CATALOG` (#3045). The two dead reranker entries are a product
  decision (§10 Q3) — remove now, or publish the repos.

### R9 — inference verbs do not need a database

The inference verbs are machine-global (shared model directory, no database
state). `open_connection` (`cli/src/open.rs:44-121`) gains one rule: an
inference verb with no `--db`/positional/`STRATA_DB`/`--cache` opens the
ephemeral cache connection, the same object `--cache` opens today (:53-64).
`install-local` keeps its pre-open interception (`lib.rs:196-206`). README
:137-157 becomes true as written; `docs/inference/provider-api-keys.md` drops
`--cache` from its examples.

---

## 5. The contract matrix

The matrix is the acceptance instrument for every slice and the thing that
outlives them. It replaces the prose contract as the authority.

### 5.1 Dimensions

| Dimension | Values |
|---|---|
| spec form | `""`, `"   "`, `"openai:"`, `"local:"`, `"miniLM"`, `"MINILM"`, `"  miniLM  "`, `"local:miniLM"`, `"qwen3:1.7b"`, `"qwen3:1.7b:q8_0"`, `"tinyllama:q8_0"`, `"tinyllama:q99"`, `"nope"`, `"nope:thing"`, `"local:nope"`, `"a:b:c:d"`, `"<tmp>/present.gguf"`, `"<tmp>/absent.gguf"`, `"openai:gpt-4o-mini"`, `"OpenAI:gpt-4o-mini"`, `"anthropic:claude-x"`, `"google:x"`, `"openai-compatible:ep:m"` |
| registry state | empty dir; dir holding a non-empty `miniLM` file; dir holding a zero-length file |
| key state | none; env; config (executor level, after R4) |
| build | `cfg!(feature = "local")`, per-provider `cfg!` — the expectation is computed, and the matrix runs under both mutation-lane feature sets |
| task / verb | generate, embed, rank, tokenize, pull, capability |

A cell's expectation is `(provider, name, source kind, availability)` or a
malformed-spec error; at executor level it is `(code, class, details keys)`.

### 5.2 Where it lives

- `crates/inference/tests/resolution_matrix.rs` (feature `testkit`): a `const`
  table of cells; the harness builds `InferenceRuntime::new(config)` over a
  `tempdir` with an injected key source and asserts every cell. No process
  environment is touched.
- Executor level: the cells become **replay fixtures** (`fixtures.error_cases`)
  for `inference.generate`, `inference.embed`, `inference.models.pull`,
  `inference.capability`, `vector.upsert` and `vector.query`. For that to
  work the testkit fake must fake *execution only*: `FakeInferenceService`
  composes the real `resolve` over a fake catalog + tempdir + no-key source,
  and fakes what happens after `require_ready`. Then `missing_model`,
  `unknown_model`, `missing_api_key`, `invalid_request`,
  `unsupported_provider`, `unsupported_operation` and `download_disabled`
  leave `unreplayed-error-codes.yaml` (budget 114 → ~107) and the IDL's
  existing guards reach resolution for the first time. No parallel harness.
- Cloud `Ready` cells assert `capability` only; nothing in the matrix sends a
  request.

### 5.3 Known red, and falsification

- The matrix lands in S0 against today's code. Cells whose result differs from
  the expectation go in a `KNOWN_RED` list keyed by issue number. A test
  asserts every `KNOWN_RED` cell **still fails**, so a fix must delete its
  entry — the same shrink-only discipline as `unreplayed-error-codes.yaml`.
  The red cells *are* the bug inventory; any red cell without an issue gets
  one when S0 opens.
- Before S0 merges, #3222 is re-planted locally (revert the `parse_model_spec`
  arm) and the matrix must go red on exactly the `qwen3:1.7b` /
  `tinyllama:q8_0` / `nope:thing` rows. #3255 and #3260 are live and serve as
  proof that the pull and directory dimensions detect what the audit found.
  A guard that has not been shown to fail is not evidence (#3216).

### 5.4 Cells where contract and code disagree today

These are decided in S0, not discovered later:

| Cell | Rule 14 says | Code does | Recommendation |
|---|---|---|---|
| `"OpenAI:gpt-4o-mini"` | case-sensitive → not a provider → local name | `from_str` lowercases → OpenAI (`lib.rs:220`) | keep lenient; fix the rule (#3257) |
| `"  miniLM  "` | whitespace invalid | trimmed → `miniLM` (`lib.rs:370`) | keep lenient; fix the rule |
| `"openai-compatible:ep:m"` | reserved grammar | local name → today `missing_model`, after R2 `unknown_model` | pin `unknown_model`; reserving means no promise, and the future grammar change becomes a visible contract change |
| `"a:b:c:d"` | opaque after first colon | local name; `find_entry_by_parts` returns None for ≥4 parts (`catalog.rs:301`) | `unknown_model` |
| embed model asked to `generate` | task from the operation path | not pinned anywhere | `TaskNotSupported` → `unsupported_operation` (Q7) |
| zero-length model file | — | `model_file_is_downloaded` false → `missing_model`; `check_and_clean_corrupt` on load | `NotDownloaded` (the file is not a model); pull overwrites |

---

## 6. Slices

Each slice is one PR, ≤1,500 LOC, with the matrix as its acceptance: the
slice's cells leave `KNOWN_RED`, no other cell changes. Issues in the "held"
column are **not** to be `/audit-fix`ed individually while this plan runs.

| Slice | Content | Wire change | Held issues it closes |
|---|---|---|---|
| **S0** | Matrix (inference level, all cells) + `KNOWN_RED`; executor-level cells for the local-spec and malformed rows; falsification by re-planting #3222 | none | — (files issues for any red cell without one) |
| **S1** | R1 `resolve` / `ResolvedModel` / `Availability` / `require_ready`; R5 one registry; R6 pull through the resolver; loaders take the registry | none (codes unchanged except cloud `pull` → `unsupported_operation`, declared on `inference.models.pull`) | #3255, #3260 |
| **S2** | R2 `unknown_model`; R3 `AvailabilityDetails` on the wire and in `capability`; D8 reads details; testkit fake composes the real resolver; replay fixtures for resolution-time codes; #3252 decided (Q4) | **yes** — new code, new details, `capability` fields; CHANGELOG + release note | #3256, #3226, #3252 |
| **S3** | R4 key source (executor installs env-then-config; CLI bridge deleted); R9 implicit cache for inference verbs; key dimension at executor level; docs drop `--cache` | `status` key-source field semantics (same values, produced by the runtime) | #3221, #3233 |
| **S4** | R7 one size formatter; R8 rule 14 rewrite, clap-parse guard over docs and hints, nightly catalog check, dead-entry decision; `STRATA_LOCAL_API_KEY` removed; `strata models pull` fixed | none | #3235, #3257, #3045 |
| **T** | Tooling lane: mutation-gate self-check (a diff that touches product code and yields zero viable mutants fails the gate); #3225 exit-3 precedence; #3227 `Result` alias; #3254 `local`-gated code in mixed files; #3258 non-`Default` enum arms; #3220 | none | #3225, #3227, #3254, #3258, #3220 |

Sequencing: **T runs in parallel from the start** — every slice S1–S4 will
touch `local`-gated code in mixed files, and the gate must be able to see it.
S0 → S1 → S2 → S3 → S4 are ordered by dependency (S2 needs the resolver; S3's
key dimension needs R4; S4's rule-14 rewrite needs the matrix to point at).

Two pieces of error-registry mechanism work are prerequisites for S2 and stay
their own PRs, ahead of it: **#3250** (named error sets, so `unknown_model` is
added to one set, not three lists) and **#3244** (registry row unconditional,
hint override explicit). They are not symptoms of the resolver; they are the
tooling S2 lands on.

Not in this plan: #3224 (Homebrew formula — "we'll deal with homebrew
separately"), #3234, #3039, the intelligence layer, and #3216's second step
(carrying codes at all ~288 raise sites). The resolver's own errors are typed
from day one, which is that step applied to the surface this document owns.

---

## 7. Who is reading this

The caller is usually a coding agent running `--json`. What it needs from a
refusal is not a better sentence but the facts: which model, which provider,
why it is unavailable, and the exact argument to pass to the command that
fixes it. R3 puts those in `details`; `availability` is a closed string enum
documented in the IDL; `pull_spec` is the string to type. The human renderer
(`render.rs:962-994`) shows the same fields. Every command any surface names —
registry hints, the D8 prompt, README, design docs — parses under the real
CLI, by test (R8).

---

## 8. Where we stand

Verified against `main` at `98f8f324`.

| Area | State | Gap |
|---|---|---|
| Parser | `parse_model_spec` correct after #3259; lenient on case and whitespace | rule 14 contradicts it (#3257) |
| Catalog / registry | sound predicates; case-insensitive lookup; one directory resolver | three loaders bypass the configured directory (#3260); `pull` bypasses the parser (#3255) |
| Availability | decided per entry point; `capability` has `can_*` but no "why not" | no `Availability`; `info()` returns `None` on miss |
| Error codes | typed kinds exist (#3217) but 6 of 8 variants are strings, ~240 of ~250 construction sites; substring classifiers load-bearing | `missing_model` conflates two facts (#3256); `io_failure` unproducible (#3252) |
| Wire details | schema name declared on every row | no definition, no producer — every inference error has empty `details` |
| D8 offer | works for five `Inference*` verbs on a TTY | keyed on a code literal + command field; misses `vector --text` (#3226); offers uncatalogued names (#3256) |
| Keys | `strata config set <provider>.api_key` stored 0600; env wins | reaches inference only via the CLI's `set_var` bridge (#3221) |
| No-database use | `install-local` intercepted pre-open | every other inference verb refuses without a DB target (#3233); docs disagree with each other |
| Sizes | one decimal formatter in inference | CLI has a second, mislabelled one (#3235) |
| Test reach | parser pinned (`api_contract.rs`); capability honesty pinned; wire==registry pinned | zero inference codes replayed; no matrix; mutation gate blind to `local` arms (#3254/#3258) |

---

## 9. Out of scope

- Retiring the substring classifiers for llama.cpp and provider-HTTP errors
  (#3216 step 2).
- `crates/stratadb` inference surface — the facade has none; adding one is a
  product decision, not a resolver fix.
- The intelligence layer (deferred, #3171).
- Homebrew (`strata-local` formula, #3224).
- Streaming, OpenAI-compatible endpoints, multi-model routing.

---

## 10. Decisions to make

| # | Question | Recommendation |
|---|---|---|
| Q1 | Name and class of the new code | `inference.unknown_model`, class NotFound, retry Never — as #3256 proposes |
| Q2 | Case / whitespace leniency | keep the code lenient; rewrite rule 14 and closing item 2; the matrix pins it |
| Q3 | Dead reranker catalog entries (#3045) | remove them in S4 unless the repos are published first; a catalogued model that cannot be pulled is a false fact |
| Q4 | `inference.io_failure` (#3252) | wire it: the resolver's single filesystem touchpoint maps non-`NotFound` `io::Error`s to `Io`; the `every_constructible_inference_error` floor becomes an equality |
| Q5 | Missing GGUF path | `unknown_model` with `details.model` = the path; `model_load_failed` stays for files that exist and fail to load |
| Q6 | `openai-compatible:` reserved prefix | parses as a local name today; pin `unknown_model`; the future grammar lands as a visible contract change |
| Q7 | Wrong-task model (embed model asked to generate) | `Availability::TaskNotSupported` → `unsupported_operation` |
| Q8 | T before S0, or parallel | parallel; S0 does not touch product code, so the gate's blind spots do not affect it |

---

## Appendix — restatements register

What is derived (generated or guarded from a single source) and what is
written by hand on this path. The plan's job is to move rows from the second
table to the first or delete them.

**Derived today**

| Fact | Source | Carried to |
|---|---|---|
| per-command error sets | `commands/*.yaml` | generated docs, `command-index.json`, SDKs, `strata agents errors` |
| code → class / message / hint / retry | `error_registry.rs` rows | wire envelope, `/e/` pages, `docs/errors/registry.md`; pinned wire==registry (`inference_behavior.rs:301`) |
| command docs | IDL + `prose/commands/*.md` | `generated/docs/`, enforced by `check-docs` |
| examples | `examples/*.yaml` | replayed by `verify-examples` |
| parser behaviour | `api_contract.rs` | — (pins only; no prose derives from it) |

**Hand-written today (targets)**

| Fact | Copies | Plan |
|---|---|---|
| spec grammar | rule 14, closing item 2, `inference.capability.md:6`, README examples | matrix is the authority; rule 14 points; IDL prose is the one list |
| "why unavailable" | `error.code()` substring tables; D8 `is_missing_model` + `missing_model_spec`; `render.rs` status prose | `Availability` computed once, rendered everywhere |
| 16-code embed set | `inference.embed`, `vector.upsert`, `vector.query` | named set (#3250) |
| model size | `render.rs` MiB math + inference decimal | one formatter (R7) |
| model directory | `ModelRegistry::new()` ×4 | one registry (R5) |
| provider key | env read ×3 + CLI `set_var` bridge + CLI status relabel | one `ProviderKeySource` (R4) |
| commands named in prose | README, `provider-api-keys.md`, `strata-v1-cli-sdk-experience.md`, registry hints, D8 prompt | clap-parse guard (R8) |
| catalog repos | `catalog.rs` `hf_repo` strings | nightly HEAD check (R8) |
| `STRATA_LOCAL_API_KEY` | `lib.rs:408` | delete |
| `strata.error.details.inference.v1` | name on 20 rows, no definition | `AvailabilityDetails` is the definition (R3) |
