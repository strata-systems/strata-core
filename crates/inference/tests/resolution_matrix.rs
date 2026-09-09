//! The model-resolution contract, as a matrix (design:
//! `docs/design/inference-model-resolution.md` §5, tracker #3261).
//!
//! One question, one answerer: "can this model run here, and if not, why?"
//! This file pins the answer for every spec form × registry directory ×
//! network setting × build × verb, as data:
//!
//! - `SPECS` pins what each spec form **is** (its identity: malformed, a
//!   cloud provider, a catalogued local model, an uncatalogued name, a GGUF
//!   path).
//! - `expected` pins what every verb must answer for it. Its precedence rules
//!   are the contract (§5.4 and the decisions recorded at the top of that fn).
//! - `KNOWN_RED` lists the cells the code answers wrongly today, keyed by
//!   issue and pinning today's wrong answer. It is shrink-only: an entry
//!   whose cells all pass fails this test ("fixed — delete the entry"), an
//!   entry whose cells moved to a different wrong answer fails it, and a red
//!   cell without an entry fails it ("file an issue"). Slices S1–S4 empty it.
//!
//! Hermetic by construction. The matrix runs in a child process with a
//! scrubbed environment — no provider keys, `HOME` and `STRATA_MODELS_DIR`
//! under a fresh tempdir — so no cell can read the developer's real models
//! directory or keys (#3260 makes the loaders do exactly that), and no test
//! mutates the environment of the process `cargo test` runs. Nothing sends a
//! request or downloads a model: cloud cells stop at the network gate or the
//! key check, `pull` runs with the network off unless the file is already
//! present, and every "present" model is a junk file, so a cell that reaches
//! a loader observes `model_load_failed` — proof the resolver said Ready,
//! never a loaded model.
//!
//! Written to be correct under either feature set: the expectation for a cell
//! is computed from `cfg!`, so the same file grades a `local,download` build
//! (run it with `--features local,download`; no CI lane does today) and the
//! released `native + cloud` shape.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use strata_inference::registry::catalog::CATALOG;
use strata_inference::{
    ChatRequest, EmbedInput, EmbeddingsRequest, InferenceError, InferenceRuntime,
    InferenceRuntimeConfig, InferenceService, ModelTask, ProviderKind, RankRequest,
    CLOUD_PROVIDER_KEYS,
};

// ---------------------------------------------------------------------------
// Build facts. Every expectation below is a function of these, never of a
// hard-coded lane.
// ---------------------------------------------------------------------------

const LOCAL_BUILT: bool = cfg!(feature = "local");
const DOWNLOAD_BUILT: bool = cfg!(feature = "download");

fn provider_built(provider: ProviderKind) -> bool {
    match provider {
        ProviderKind::Local => LOCAL_BUILT,
        ProviderKind::Anthropic => cfg!(feature = "anthropic"),
        ProviderKind::OpenAI => cfg!(feature = "openai"),
        ProviderKind::Google => cfg!(feature = "google"),
    }
}

// ---------------------------------------------------------------------------
// Codes. Names, not prose: every assertion compares `InferenceError::code()`.
// ---------------------------------------------------------------------------

const INVALID_REQUEST: &str = "inference.invalid_request";
const MISSING_MODEL: &str = "inference.missing_model";
/// R2: the split of `missing_model` into "catalogued, not downloaded" and
/// "not a model this binary knows". Lands in S2; every cell expecting it is
/// `KNOWN_RED` until then.
const UNKNOWN_MODEL: &str = "inference.unknown_model";
const UNSUPPORTED_OPERATION: &str = "inference.unsupported_operation";
const UNSUPPORTED_PROVIDER: &str = "inference.unsupported_provider";
const MISSING_API_KEY: &str = "inference.missing_api_key";
const DOWNLOAD_DISABLED: &str = "inference.download_disabled";
const MODEL_LOAD_FAILED: &str = "inference.model_load_failed";
const REGISTRY_CORRUPT: &str = "inference.registry_corrupt";

// ---------------------------------------------------------------------------
// Dimensions.
// ---------------------------------------------------------------------------

/// What a spec form is. The identity is pinned by hand — it is the contract —
/// while the file the registry dimension plants is derived from the catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Identity {
    /// Rejected by the parser: empty, blank, or a provider with no model.
    Malformed,
    /// A cloud provider spec.
    Cloud(ProviderKind),
    /// A catalogued local model. `planted` marks the specs that resolve to the
    /// file the registry dimension plants (miniLM's default variant), so the
    /// directory state is observable through them.
    Catalog { task: ModelTask, planted: bool },
    /// A local name the catalog does not know (including an unknown quant of
    /// a known model, a four-part name, and an `openai-compatible:` prefix —
    /// §5.4 / Q6).
    NotInCatalog,
    /// A GGUF path that exists (as a junk file).
    PathPresent,
    /// A GGUF path that does not exist (Q5).
    PathAbsent,
}

struct SpecRow {
    /// The spec as typed. `<tmp>` is replaced by the cell's tempdir.
    spec: &'static str,
    identity: Identity,
}

const PLANTED_EMBED: Identity = Identity::Catalog {
    task: ModelTask::Embed,
    planted: true,
};
const GENERATE: Identity = Identity::Catalog {
    task: ModelTask::Generate,
    planted: false,
};

/// §5.1 spec forms.
const SPECS: &[SpecRow] = &[
    // Malformed.
    SpecRow {
        spec: "",
        identity: Identity::Malformed,
    },
    SpecRow {
        spec: "   ",
        identity: Identity::Malformed,
    },
    SpecRow {
        spec: "openai:",
        identity: Identity::Malformed,
    },
    SpecRow {
        spec: "local:",
        identity: Identity::Malformed,
    },
    // Catalogued local models, and the lenient spellings of one (Q2).
    SpecRow {
        spec: "miniLM",
        identity: PLANTED_EMBED,
    },
    SpecRow {
        spec: "MINILM",
        identity: PLANTED_EMBED,
    },
    SpecRow {
        spec: "  miniLM  ",
        identity: PLANTED_EMBED,
    },
    SpecRow {
        spec: "local:miniLM",
        identity: PLANTED_EMBED,
    },
    SpecRow {
        spec: "qwen3:1.7b",
        identity: GENERATE,
    },
    SpecRow {
        spec: "qwen3:1.7b:q8_0",
        identity: GENERATE,
    },
    SpecRow {
        spec: "tinyllama:q8_0",
        identity: GENERATE,
    },
    // Not in the catalog.
    SpecRow {
        spec: "tinyllama:q99",
        identity: Identity::NotInCatalog,
    },
    SpecRow {
        spec: "nope",
        identity: Identity::NotInCatalog,
    },
    SpecRow {
        spec: "nope:thing",
        identity: Identity::NotInCatalog,
    },
    SpecRow {
        spec: "local:nope",
        identity: Identity::NotInCatalog,
    },
    SpecRow {
        spec: "a:b:c:d",
        identity: Identity::NotInCatalog,
    },
    SpecRow {
        spec: "openai-compatible:ep:m",
        identity: Identity::NotInCatalog,
    },
    // GGUF paths.
    SpecRow {
        spec: "<tmp>/present.gguf",
        identity: Identity::PathPresent,
    },
    SpecRow {
        spec: "<tmp>/absent.gguf",
        identity: Identity::PathAbsent,
    },
    // Cloud.
    SpecRow {
        spec: "openai:gpt-4o-mini",
        identity: Identity::Cloud(ProviderKind::OpenAI),
    },
    SpecRow {
        spec: "OpenAI:gpt-4o-mini",
        identity: Identity::Cloud(ProviderKind::OpenAI),
    },
    SpecRow {
        spec: "anthropic:claude-x",
        identity: Identity::Cloud(ProviderKind::Anthropic),
    },
    SpecRow {
        spec: "google:x",
        identity: Identity::Cloud(ProviderKind::Google),
    },
];

/// Registry directory state (§5.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dir {
    Empty,
    /// miniLM's default variant present as a non-empty junk file.
    Present,
    /// miniLM's default variant present as a zero-length file — an interrupted
    /// download. Not downloaded (§5.4).
    ZeroLength,
}

const DIRS: [Dir; 3] = [Dir::Empty, Dir::Present, Dir::ZeroLength];

/// `InferenceRuntimeConfig::network_enabled`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Net {
    Off,
    On,
}

const NETS: [Net; 2] = [Net::Off, Net::On];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verb {
    Capability,
    Generate,
    Embed,
    Rank,
    Tokenize,
    Pull,
}

const VERBS: [Verb; 6] = [
    Verb::Capability,
    Verb::Generate,
    Verb::Embed,
    Verb::Rank,
    Verb::Tokenize,
    Verb::Pull,
];

impl Verb {
    /// Which catalogued task a local verb needs. Every local GGUF tokenizes
    /// (`ModelAbilities::of`).
    fn applies_to(self, task: ModelTask) -> bool {
        match self {
            Verb::Generate => task == ModelTask::Generate,
            Verb::Embed => task == ModelTask::Embed,
            Verb::Rank => task == ModelTask::Rank,
            Verb::Tokenize => true,
            Verb::Capability | Verb::Pull => unreachable!("not a load verb"),
        }
    }
}

// ---------------------------------------------------------------------------
// The contract.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Expect {
    /// The verb succeeds.
    Ok,
    /// The verb refuses with this code.
    Code(&'static str),
    /// The cell would send a request or download a model. Never executed;
    /// the string says why.
    NeverRun(&'static str),
}

/// What every verb must answer. This function *is* §5.4 plus the precedence
/// decisions S0 makes (flagged in the PR as Q9–Q12):
///
/// - **Q9 — identity before capability.** Whether a spec names something is
///   answered before whether this build or network can run it: malformed →
///   unknown (not in catalog / path missing) → wrong task → execution not
///   built → network disabled → key missing → not downloaded. A user is
///   never told to install local execution for a model that does not exist.
/// - **Q10 — `pull` is task-neutral and needs no execution build.** It
///   resolves first and downloads only when a catalogued model is not
///   downloaded; a present file is returned without network; a cloud spec is
///   `unsupported_operation` (R6); a path is returned when present.
/// - **Q11 — network before key.** With the network off, a cloud verb is
///   refused as `unsupported_operation` (a `NetworkDisabled` availability the
///   design must add) before any key is looked at.
/// - **Q12 — a zero-length file is not downloaded** (§5.4), for `pull` too.
fn expected(identity: Identity, dir: Dir, net: Net, verb: Verb) -> Expect {
    match identity {
        Identity::Malformed => Expect::Code(INVALID_REQUEST),
        Identity::Cloud(provider) => cloud(provider, net, verb),
        Identity::NotInCatalog | Identity::PathAbsent => match verb {
            Verb::Capability => Expect::Ok,
            _ => Expect::Code(UNKNOWN_MODEL),
        },
        Identity::PathPresent => match verb {
            Verb::Capability | Verb::Pull => Expect::Ok,
            _ if LOCAL_BUILT => Expect::Code(MODEL_LOAD_FAILED),
            _ => Expect::Code(UNSUPPORTED_OPERATION),
        },
        Identity::Catalog { task, planted } => {
            let downloaded = planted && dir == Dir::Present;
            match verb {
                Verb::Capability => Expect::Ok,
                Verb::Pull if downloaded => Expect::Ok,
                Verb::Pull if net == Net::Off || !DOWNLOAD_BUILT => Expect::Code(DOWNLOAD_DISABLED),
                Verb::Pull => Expect::NeverRun("would download the model"),
                _ if !verb.applies_to(task) => Expect::Code(UNSUPPORTED_OPERATION),
                _ if !LOCAL_BUILT => Expect::Code(UNSUPPORTED_OPERATION),
                _ if downloaded => Expect::Code(MODEL_LOAD_FAILED),
                _ => Expect::Code(MISSING_MODEL),
            }
        }
    }
}

fn cloud(provider: ProviderKind, net: Net, verb: Verb) -> Expect {
    match verb {
        Verb::Capability => Expect::Ok,
        // Local-only verbs (rank, tokenize) and pull (R6).
        Verb::Pull | Verb::Rank | Verb::Tokenize => Expect::Code(UNSUPPORTED_OPERATION),
        // Anthropic has no embedding API: a task the provider lacks, not a
        // provider this build lacks.
        Verb::Embed if provider == ProviderKind::Anthropic => Expect::Code(UNSUPPORTED_OPERATION),
        Verb::Generate | Verb::Embed => {
            if !provider_built(provider) {
                Expect::Code(UNSUPPORTED_PROVIDER)
            } else if net == Net::Off {
                Expect::Code(UNSUPPORTED_OPERATION)
            } else if keys_in_env() {
                Expect::NeverRun("key present and network on: would send a request")
            } else {
                Expect::Code(MISSING_API_KEY)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Known red cells. Shrink-only. Each entry names the cells it covers, the
// issue that owns them, and today's wrong answer.
// ---------------------------------------------------------------------------

/// Which builds an entry applies to. `None` = every build.
#[derive(Clone, Copy)]
struct Lane {
    local: Option<bool>,
    download: Option<bool>,
}

impl Lane {
    const ANY: Lane = Lane {
        local: None,
        download: None,
    };
    const LOCAL: Lane = Lane {
        local: Some(true),
        download: None,
    };
    const NOT_LOCAL: Lane = Lane {
        local: Some(false),
        download: None,
    };
    const DOWNLOAD: Lane = Lane {
        local: None,
        download: Some(true),
    };
    const NOT_DOWNLOAD: Lane = Lane {
        local: None,
        download: Some(false),
    };
    fn applies(self) -> bool {
        self.local.is_none_or(|local| local == LOCAL_BUILT)
            && self
                .download
                .is_none_or(|download| download == DOWNLOAD_BUILT)
    }
}

/// Which registry directories an entry covers. `None` = every one.
type Dirs = Option<&'static [Dir]>;
/// Which network settings an entry covers. `None` = both.
type Nets = Option<Net>;

struct KnownRed {
    issue: &'static str,
    why: &'static str,
    lane: Lane,
    verbs: &'static [Verb],
    specs: &'static [&'static str],
    dirs: Dirs,
    nets: Nets,
    today: Expect,
}

impl KnownRed {
    fn covers(&self, cell: &Cell) -> bool {
        self.lane.applies()
            && self.verbs.contains(&cell.verb)
            && self.specs.contains(&cell.row.spec)
            && self.dirs.is_none_or(|dirs| dirs.contains(&cell.dir))
            && self.nets.is_none_or(|net| net == cell.net)
    }
}

const LOAD_VERBS: &[Verb] = &[Verb::Generate, Verb::Embed, Verb::Rank, Verb::Tokenize];
const UNPARSED_BEFORE_LOCAL_CHECK: &[Verb] = &[Verb::Rank, Verb::Tokenize];
const MALFORMED: &[&str] = &["", "   ", "openai:", "local:"];
const CLOUD: &[&str] = &[
    "openai:gpt-4o-mini",
    "OpenAI:gpt-4o-mini",
    "anthropic:claude-x",
    "google:x",
];
const PLANTED: &[&str] = &["miniLM", "MINILM", "  miniLM  ", "local:miniLM"];
const UNPLANTED_CATALOG: &[&str] = &["qwen3:1.7b", "qwen3:1.7b:q8_0", "tinyllama:q8_0"];
const NOT_PRESENT: &[Dir] = &[Dir::Empty, Dir::ZeroLength];

/// Specs `pull` never resolves today: it goes to the download gate and then
/// straight to the registry, skipping the parser and the resolver (#3255).
const UNRESOLVED_BY_PULL: &[&str] = &[
    "",
    "   ",
    "openai:",
    "local:",
    "openai:gpt-4o-mini",
    "OpenAI:gpt-4o-mini",
    "anthropic:claude-x",
    "google:x",
    "tinyllama:q99",
    "nope",
    "nope:thing",
    "local:nope",
    "a:b:c:d",
    "openai-compatible:ep:m",
    "<tmp>/present.gguf",
    "<tmp>/absent.gguf",
];
/// The same set minus the unknown quant, which the registry answers
/// differently (#3264).
const UNKNOWN_TO_REGISTRY: &[&str] = &[
    "",
    "   ",
    "openai:",
    "local:",
    "openai:gpt-4o-mini",
    "OpenAI:gpt-4o-mini",
    "anthropic:claude-x",
    "google:x",
    "nope",
    "nope:thing",
    "local:nope",
    "a:b:c:d",
    "openai-compatible:ep:m",
    "<tmp>/present.gguf",
    "<tmp>/absent.gguf",
];

const KNOWN_RED: &[KnownRed] = &[
    // ----- #3255: pull does not go through the resolver -----
    KnownRed {
        issue: "#3255",
        why: "with the network off, pull refuses before looking at the spec: a \
              malformed spec, a cloud spec, an uncatalogued name or a GGUF path \
              is `download_disabled`",
        lane: Lane::ANY,
        verbs: &[Verb::Pull],
        specs: UNRESOLVED_BY_PULL,
        dirs: None,
        nets: Some(Net::Off),
        today: Expect::Code(DOWNLOAD_DISABLED),
    },
    KnownRed {
        issue: "#3255",
        why: "same, in a build without download support: the download gate \
              answers before the spec is looked at",
        lane: Lane::NOT_DOWNLOAD,
        verbs: &[Verb::Pull],
        specs: UNRESOLVED_BY_PULL,
        dirs: None,
        nets: Some(Net::On),
        today: Expect::Code(DOWNLOAD_DISABLED),
    },
    KnownRed {
        issue: "#3255",
        why: "in a download build with the network on, pull hands the raw spec \
              to the registry, which knows none of these: a malformed spec, a \
              cloud spec and a GGUF path are all `missing_model`",
        lane: Lane::DOWNLOAD,
        verbs: &[Verb::Pull],
        specs: UNKNOWN_TO_REGISTRY,
        dirs: None,
        nets: Some(Net::On),
        today: Expect::Code(MISSING_MODEL),
    },
    KnownRed {
        issue: "#3255",
        why: "the registry lookup pull uses does not apply the parser's \
              leniency: an untrimmed or `local:`-prefixed spelling of a \
              present model is not found",
        lane: Lane::DOWNLOAD,
        verbs: &[Verb::Pull],
        specs: &["  miniLM  ", "local:miniLM"],
        dirs: Some(&[Dir::Present]),
        nets: Some(Net::On),
        today: Expect::Code(MISSING_MODEL),
    },
    KnownRed {
        issue: "#3255",
        why: "pull of a model that is already present needs no download, but \
              the network gate runs before resolution",
        lane: Lane::ANY,
        verbs: &[Verb::Pull],
        specs: PLANTED,
        dirs: Some(&[Dir::Present]),
        nets: Some(Net::Off),
        today: Expect::Code(DOWNLOAD_DISABLED),
    },
    KnownRed {
        issue: "#3255",
        why: "pull of a present model in a build without download support \
              refuses instead of returning the file",
        lane: Lane::NOT_DOWNLOAD,
        verbs: &[Verb::Pull],
        specs: PLANTED,
        dirs: Some(&[Dir::Present]),
        nets: Some(Net::On),
        today: Expect::Code(DOWNLOAD_DISABLED),
    },
    // ----- #3262: a non-local build answers "not built" before identity -----
    KnownRed {
        issue: "#3262",
        why: "tokenize and rank refuse on the missing local feature without \
              parsing the spec, so a malformed spec is not `invalid_request`",
        lane: Lane::NOT_LOCAL,
        verbs: UNPARSED_BEFORE_LOCAL_CHECK,
        specs: MALFORMED,
        dirs: None,
        nets: None,
        today: Expect::Code(UNSUPPORTED_OPERATION),
    },
    KnownRed {
        issue: "#3262",
        why: "a name the catalog does not know, or a GGUF path that does not \
              exist, is told to install local execution",
        lane: Lane::NOT_LOCAL,
        verbs: LOAD_VERBS,
        specs: &[
            "tinyllama:q99",
            "nope",
            "nope:thing",
            "local:nope",
            "a:b:c:d",
            "openai-compatible:ep:m",
            "<tmp>/absent.gguf",
        ],
        dirs: None,
        nets: None,
        today: Expect::Code(UNSUPPORTED_OPERATION),
    },
    // ----- #3263: a wrong-task model is never answered as such -----
    KnownRed {
        issue: "#3263",
        why: "embedding with Anthropic is refused as `unsupported_provider` — \
              the provider is built, it has no embedding API (with the network \
              off the network gate answers first, with the right code)",
        lane: Lane::ANY,
        verbs: &[Verb::Embed],
        specs: &["anthropic:claude-x"],
        dirs: None,
        nets: Some(Net::On),
        today: Expect::Code(UNSUPPORTED_PROVIDER),
    },
    KnownRed {
        issue: "#3263",
        why: "ranking with a cloud spec is refused as `unsupported_provider` \
              in a local build (\"local ranking requires local provider\")",
        lane: Lane::LOCAL,
        verbs: &[Verb::Rank],
        specs: CLOUD,
        dirs: None,
        nets: None,
        today: Expect::Code(UNSUPPORTED_PROVIDER),
    },
    KnownRed {
        issue: "#3263",
        why: "a catalogued model asked for a task it does not have is answered \
              by its download state, not its task",
        lane: Lane::LOCAL,
        verbs: &[Verb::Generate, Verb::Rank],
        specs: PLANTED,
        dirs: Some(NOT_PRESENT),
        nets: None,
        today: Expect::Code(MISSING_MODEL),
    },
    KnownRed {
        issue: "#3263",
        why: "same, for generation models asked to embed or rank",
        lane: Lane::LOCAL,
        verbs: &[Verb::Embed, Verb::Rank],
        specs: UNPLANTED_CATALOG,
        dirs: None,
        nets: None,
        today: Expect::Code(MISSING_MODEL),
    },
    // ----- #3260: loaders build their own registry from the environment -----
    KnownRed {
        issue: "#3260",
        why: "a present model is not found because the loader reads \
              STRATA_MODELS_DIR / ~/.strata/models, not config.models_dir",
        lane: Lane::LOCAL,
        verbs: &[Verb::Embed, Verb::Tokenize],
        specs: PLANTED,
        dirs: Some(&[Dir::Present]),
        nets: None,
        today: Expect::Code(MISSING_MODEL),
    },
    KnownRed {
        issue: "#3260",
        why: "same loader, wrong task: the task answer (#3263) is also masked",
        lane: Lane::LOCAL,
        verbs: &[Verb::Generate, Verb::Rank],
        specs: PLANTED,
        dirs: Some(&[Dir::Present]),
        nets: None,
        today: Expect::Code(MISSING_MODEL),
    },
    // ----- #3256: catalog-miss and not-downloaded share missing_model -----
    KnownRed {
        issue: "#3256",
        why: "an uncatalogued name is `missing_model` (\"Unknown model\" \
              substring), indistinguishable from a model awaiting download",
        lane: Lane::LOCAL,
        verbs: LOAD_VERBS,
        specs: &[
            "nope",
            "nope:thing",
            "local:nope",
            "a:b:c:d",
            "openai-compatible:ep:m",
        ],
        dirs: None,
        nets: None,
        today: Expect::Code(MISSING_MODEL),
    },
    KnownRed {
        issue: "#3256",
        why: "a GGUF path that does not exist is a loader failure, not an \
              unknown model (Q5)",
        lane: Lane::LOCAL,
        verbs: LOAD_VERBS,
        specs: &["<tmp>/absent.gguf"],
        dirs: None,
        nets: None,
        today: Expect::Code(MODEL_LOAD_FAILED),
    },
    // ----- #3264: an unknown quant is reported as registry corruption -----
    KnownRed {
        issue: "#3264",
        why: "`tinyllama:q99` falls through the registry substring classifier \
              to `registry_corrupt` (class Corruption) — nothing is corrupt",
        lane: Lane::LOCAL,
        verbs: LOAD_VERBS,
        specs: &["tinyllama:q99"],
        dirs: None,
        nets: None,
        today: Expect::Code(REGISTRY_CORRUPT),
    },
    KnownRed {
        issue: "#3264",
        why: "same classifier, reached through pull once the download gate \
              lets it through (with the network off or no download support \
              the gate answers first — #3255)",
        lane: Lane::DOWNLOAD,
        verbs: &[Verb::Pull],
        specs: &["tinyllama:q99"],
        dirs: None,
        nets: Some(Net::On),
        today: Expect::Code(REGISTRY_CORRUPT),
    },
];

// ---------------------------------------------------------------------------
// Cells and their execution.
// ---------------------------------------------------------------------------

struct Cell {
    row: &'static SpecRow,
    dir: Dir,
    net: Net,
    verb: Verb,
}

impl Cell {
    fn name(&self) -> String {
        format!(
            "{:?} × {:?} × {:?} × {:?}",
            self.row.spec, self.dir, self.net, self.verb
        )
    }
}

fn cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    for row in SPECS {
        for dir in DIRS {
            for net in NETS {
                for verb in VERBS {
                    cells.push(Cell {
                        row,
                        dir,
                        net,
                        verb,
                    });
                }
            }
        }
    }
    cells
}

#[derive(Debug, PartialEq, Eq)]
enum Observed {
    Ok,
    Code(&'static str),
}

impl Observed {
    fn of<T>(result: Result<T, InferenceError>) -> Self {
        match result {
            Ok(_) => Observed::Ok,
            Err(error) => Observed::Code(error.code()),
        }
    }

    fn matches(&self, expect: Expect) -> bool {
        match (self, expect) {
            (Observed::Ok, Expect::Ok) => true,
            (Observed::Code(observed), Expect::Code(expected)) => *observed == expected,
            _ => false,
        }
    }
}

/// The file the registry dimension plants: miniLM's default variant, named
/// by the catalog, never by hand.
fn planted_file() -> &'static str {
    let entry = CATALOG
        .iter()
        .find(|entry| entry.name == "miniLM")
        .expect("miniLM is catalogued");
    entry
        .variants
        .iter()
        .find(|variant| variant.name == entry.default_quant)
        .expect("default variant exists")
        .hf_file
}

/// Bytes that are not a GGUF file. Large enough that a loader that gets this
/// far fails on content, not on an empty read.
const JUNK: &[u8; 4096] = &[0x5a; 4096];

struct Fixture {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    runtime: InferenceRuntime,
}

fn fixture(dir: Dir, net: Net) -> Fixture {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    let models = root.join("models");
    fs::create_dir_all(&models).expect("models dir");
    match dir {
        Dir::Empty => {}
        Dir::Present => fs::write(models.join(planted_file()), JUNK).expect("plant"),
        Dir::ZeroLength => fs::write(models.join(planted_file()), b"").expect("plant"),
    }
    fs::write(root.join("present.gguf"), JUNK).expect("present.gguf");
    let runtime = InferenceRuntime::new(InferenceRuntimeConfig {
        models_dir: Some(models),
        network_enabled: net == Net::On,
    });
    Fixture {
        _tmp: tmp,
        root,
        runtime,
    }
}

fn spec_for(row: &SpecRow, root: &Path) -> String {
    row.spec
        .replace("<tmp>", root.to_str().expect("utf-8 tempdir"))
}

/// Runs one cell through the [`InferenceService`] trait — the path the
/// executor dispatches to — never through the inherent single-item helpers.
fn run(cell: &Cell) -> Observed {
    let fixture = fixture(cell.dir, cell.net);
    let runtime: &dyn InferenceService = &fixture.runtime;
    let spec = spec_for(cell.row, &fixture.root);
    match cell.verb {
        Verb::Capability => {
            let result = runtime.capability(&spec);
            if let (Ok(capability), Identity::Cloud(provider)) = (&result, cell.row.identity) {
                assert_eq!(
                    capability.provider,
                    provider,
                    "{}: provider identity",
                    cell.name()
                );
            }
            if let (Ok(capability), Identity::Catalog { .. } | Identity::NotInCatalog) =
                (&result, cell.row.identity)
            {
                assert_eq!(
                    capability.provider,
                    ProviderKind::Local,
                    "{}: provider identity",
                    cell.name()
                );
            }
            Observed::of(result)
        }
        Verb::Generate => Observed::of(runtime.chat(
            &spec,
            &ChatRequest {
                prompt: Some("hi".to_owned()),
                max_tokens: Some(1),
                ..ChatRequest::default()
            },
        )),
        Verb::Embed => Observed::of(runtime.embeddings(
            &spec,
            &EmbeddingsRequest {
                input: EmbedInput::One("hi".to_owned()),
                dimensions: None,
                normalize: None,
                input_type: None,
                instruction: None,
            },
        )),
        Verb::Rank => Observed::of(runtime.rank(
            &spec,
            &RankRequest {
                query: "q".to_owned(),
                passages: vec!["p".to_owned()],
            },
        )),
        Verb::Tokenize => Observed::of(runtime.tokenize(&spec, "hi", true)),
        Verb::Pull => Observed::of(runtime.pull_model(&spec)),
    }
}

// ---------------------------------------------------------------------------
// The child: runs every cell in the scrubbed environment and grades it.
// ---------------------------------------------------------------------------

const CHILD_MODE: &str = "STRATA_RESOLUTION_MATRIX_CHILD";
const FAKE_KEY: &str = "matrix-fake-key-never-sent";

fn keys_in_env() -> bool {
    std::env::var(CHILD_MODE).as_deref() == Ok("keys")
}

/// The environment the parent built, as the surfaces that report it see it:
/// no key unless this is the keys phase, and a models directory that is not
/// the developer's.
fn assert_child_environment(mode: &str) {
    for key in CLOUD_PROVIDER_KEYS {
        assert_eq!(
            std::env::var_os(key.env_var).is_some(),
            mode == "keys",
            "{} in the child environment",
            key.env_var
        );
    }

    // The key dimension as observed today: `status` reports presence, never
    // the key. (S3 moves this onto `capability` as an availability.)
    let status = fixture(Dir::Empty, Net::On).runtime.status();
    for row in &status.providers {
        if row.requires_api_key {
            assert_eq!(
                row.key_present,
                mode == "keys",
                "{:?} key_present in mode {mode}",
                row.provider
            );
        }
    }

    // The directory dimension as observed by the surfaces that list models:
    // a zero-length file is not downloaded.
    for dir in DIRS {
        let fixture = fixture(dir, Net::Off);
        let downloaded = fixture.runtime.status().models_downloaded;
        let listed = fixture.runtime.list_local_models().len();
        let expected = usize::from(dir == Dir::Present);
        assert_eq!(
            (downloaded, listed),
            (expected, expected),
            "{dir:?}: models_downloaded / list_local_models"
        );
    }
}

/// Every cell graded against the contract and against `KNOWN_RED`.
#[derive(Default)]
struct Grade {
    executed: usize,
    skipped: usize,
    /// Cells covered by each `KNOWN_RED` entry, by index.
    covered: Vec<usize>,
    /// Red, no entry: file an issue and add one.
    unexpected_red: Vec<String>,
    /// Red, entry present, but not today's pinned answer.
    moved: Vec<String>,
    /// Green with an entry: shrink `KNOWN_RED`.
    fixed: Vec<String>,
}

fn grade_cells() -> Grade {
    let mut grade = Grade {
        covered: vec![0; KNOWN_RED.len()],
        ..Grade::default()
    };
    for cell in cells() {
        let expect = expected(cell.row.identity, cell.dir, cell.net, cell.verb);
        if let Expect::NeverRun(_) = expect {
            grade.skipped += 1;
            continue;
        }
        grade.executed += 1;
        let observed = run(&cell);
        let entry = KNOWN_RED.iter().position(|entry| entry.covers(&cell));
        match (observed.matches(expect), entry) {
            (true, None) => {}
            (true, Some(index)) => grade.fixed.push(format!(
                "{} — {} ({}): passes; delete or narrow the entry",
                cell.name(),
                KNOWN_RED[index].issue,
                KNOWN_RED[index].why
            )),
            (false, None) => grade.unexpected_red.push(format!(
                "{}: expected {expect:?}, observed {observed:?} — file an issue and \
                 add a KNOWN_RED entry",
                cell.name()
            )),
            (false, Some(index)) => {
                grade.covered[index] += 1;
                if !observed.matches(KNOWN_RED[index].today) {
                    grade.moved.push(format!(
                        "{} — {}: pinned today={:?}, observed {observed:?}, contract {expect:?}",
                        cell.name(),
                        KNOWN_RED[index].issue,
                        KNOWN_RED[index].today
                    ));
                }
            }
        }
    }
    grade
}

#[test]
#[ignore = "subprocess phase: re-invoked by `matrix_holds_in_a_scrubbed_environment`"]
fn matrix_child() {
    let Ok(mode) = std::env::var(CHILD_MODE) else {
        return;
    };
    assert!(
        matches!(mode.as_str(), "no-keys" | "keys"),
        "unknown child mode {mode:?}"
    );
    assert_child_environment(&mode);

    let grade = grade_cells();
    let dead: Vec<String> = KNOWN_RED
        .iter()
        .zip(&grade.covered)
        .filter(|(entry, count)| entry.lane.applies() && **count == 0)
        .map(|(entry, _)| format!("{} ({}) covers no executed cell", entry.issue, entry.why))
        .collect();

    println!(
        "resolution matrix [{mode}; local={LOCAL_BUILT} download={DOWNLOAD_BUILT}]: \
         {} executed, {} never-run, {} known red",
        grade.executed,
        grade.skipped,
        grade.covered.iter().sum::<usize>()
    );
    let mut report = String::new();
    for (title, lines) in [
        ("UNEXPECTED RED", &grade.unexpected_red),
        ("MOVED (known red now fails differently)", &grade.moved),
        ("FIXED (shrink KNOWN_RED)", &grade.fixed),
        ("DEAD ENTRIES", &dead),
    ] {
        if !lines.is_empty() {
            // `fmt::Write` for `String` is infallible.
            writeln!(report, "\n{title}:").expect("String never fails to write");
            for line in lines {
                writeln!(report, "  {line}").expect("String never fails to write");
            }
        }
    }
    assert!(report.is_empty(), "resolution matrix [{mode}]:{report}");
}

// ---------------------------------------------------------------------------
// The parent: builds the scrubbed environment and runs the child twice —
// once with no key and once with a fake key in every provider variable.
// ---------------------------------------------------------------------------

fn run_child(mode: &str) {
    let scrub = tempfile::tempdir().expect("scrub dir");
    let home = scrub.path().join("home");
    let models = scrub.path().join("models");
    fs::create_dir_all(&home).expect("home");
    fs::create_dir_all(&models).expect("models");

    let exe = std::env::current_exe().expect("current test binary");
    let mut command = Command::new(exe);
    command
        .args(["matrix_child", "--exact", "--ignored", "--nocapture"])
        .env_clear()
        .env(CHILD_MODE, mode)
        .env("HOME", &home)
        .env("STRATA_MODELS_DIR", &models);
    // Keep the parent's temp-dir convention so the child's tempdirs land in
    // the same place; nothing else from the environment crosses over.
    if let Some(tmpdir) = std::env::var_os("TMPDIR") {
        command.env("TMPDIR", tmpdir);
    }
    if mode == "keys" {
        for key in CLOUD_PROVIDER_KEYS {
            command.env(key.env_var, FAKE_KEY);
        }
    }
    let output = command.output().expect("spawn matrix child");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "matrix child [{mode}] failed:\n{stdout}\n{stderr}"
    );
    let summary = stdout
        .lines()
        .find(|line| line.starts_with("resolution matrix ["))
        .unwrap_or_else(|| {
            panic!("matrix child [{mode}] did not run the matrix:\n{stdout}\n{stderr}")
        });
    // Surface the cell counts under `--nocapture`; they are the evidence a
    // slice cites when it shrinks `KNOWN_RED`.
    println!("{summary}");
}

/// Every cell of §5, graded against the contract, with `KNOWN_RED` exact.
#[test]
fn matrix_holds_in_a_scrubbed_environment() {
    run_child("no-keys");
}

/// With a key in the environment nothing changes except what `status`
/// reports and which cloud cells become un-runnable: with the network off a
/// key is never consulted, and no local cell looks at one.
#[test]
fn a_key_in_the_environment_changes_only_key_cells() {
    run_child("keys");
}
