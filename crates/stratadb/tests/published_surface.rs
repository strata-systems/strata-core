//! Guards on what `stratadb` publishes.
//!
//! `stratadb` is the only published Rust surface; `strata-engine` beneath it is
//! internal. The facade re-exports engine names one by one rather than by glob,
//! so that adding a public type to the engine is a decision here rather than an
//! accident on crates.io (#3140). These tests keep that property honest: the
//! first proves the two lists have not drifted apart, the second proves the
//! types the API hands back can actually be named by a caller (#3190).

use std::collections::BTreeSet;
use std::path::PathBuf;

use stratadb::prelude::*;
use stratadb::{CommitVersion, Timestamp};

/// Engine root names the facade deliberately does not publish.
///
/// Empty today. An entry here is a claim that a name is engine-internal
/// despite being `pub` at the engine root — write the reason beside it.
const NOT_PUBLISHED: &[&str] = &[];

fn workspace_file(relative: &str) -> String {
    let path: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

/// Collect the names inside every `pub use <source>::{ ... };` block.
fn reexported_names(source_text: &str, from: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let opener = format!("pub use {from}::{{");
    let mut rest = source_text;
    while let Some(start) = rest.find(&opener) {
        let body_start = start + opener.len();
        let body_end = body_start
            + rest[body_start..]
                .find("};")
                .expect("re-export block is closed with `};`");
        for name in rest[body_start..body_end].split(',') {
            let name = name.trim();
            if !name.is_empty() {
                names.insert(name.to_owned());
            }
        }
        rest = &rest[body_end..];
    }
    names
}

/// Every name the engine publishes at its root is either re-exported by the
/// facade or explicitly listed as unpublished. A new engine type fails this
/// test until someone decides which it is.
#[test]
fn facade_publishes_every_engine_root_name() {
    let engine = workspace_file("crates/engine/src/lib.rs");
    let facade = workspace_file("crates/stratadb/src/lib.rs");

    let engine_names: BTreeSet<String> = reexported_names(&engine, "api")
        .union(&reexported_names(&engine, "strata_core"))
        .cloned()
        .collect();
    let facade_names = reexported_names(&facade, "strata_engine");
    let unpublished: BTreeSet<String> = NOT_PUBLISHED
        .iter()
        .map(|name| (*name).to_owned())
        .collect();

    assert!(
        engine_names.len() > 150,
        "parsed only {} engine names — the parser has lost the re-export block",
        engine_names.len()
    );

    let missing: Vec<&String> = engine_names
        .iter()
        .filter(|name| !facade_names.contains(*name) && !unpublished.contains(*name))
        .collect();
    assert!(
        missing.is_empty(),
        "engine names reachable nowhere in the published facade: {missing:?}\n\
         Re-export them from the matching `stratadb` module, or add them to NOT_PUBLISHED."
    );

    let stale: Vec<&String> = unpublished
        .iter()
        .filter(|name| !engine_names.contains(*name))
        .collect();
    assert!(
        stale.is_empty(),
        "NOT_PUBLISHED names the engine no longer exports: {stale:?}"
    );
}

/// The facade re-exports whole engine modules nowhere: a glob or module
/// re-export would put engine additions on crates.io without review, which is
/// the property the test above exists to protect.
#[test]
fn facade_does_not_re_export_engine_modules_or_globs() {
    let facade = workspace_file("crates/stratadb/src/lib.rs");
    for line in facade.lines() {
        let line = line.trim();
        assert!(
            !line.starts_with("pub use strata_engine::*"),
            "glob re-export of the engine defeats the curated surface: {line}"
        );
        assert!(
            !(line.starts_with("pub use strata_engine::") && line.ends_with("api;")),
            "re-exporting the engine's flat `api` module defeats the module split: {line}"
        );
    }
}

/// A caller must be able to name the types the API hands back. Before #3190
/// `CommitVersion` and `Timestamp` were reachable through no path at all, so a
/// write acknowledgement could be used but never stored in a typed binding.
#[test]
fn types_the_api_returns_can_be_named() {
    let database = Database::open_cache(CacheOpenOptions::new())
        .expect("cache open")
        .into_database();
    let mut kv = database
        .kv(
            BranchName::new("default").expect("branch"),
            ProductSpace::new("default").expect("space"),
        )
        .expect("kv service");

    let first = kv
        .put(KvKey::new("k").expect("key"), KvValue::new(b"v1".to_vec()))
        .expect("first put");
    let second = kv
        .put(KvKey::new("k").expect("key"), KvValue::new(b"v2".to_vec()))
        .expect("second put");

    let first_version: CommitVersion = first.commit().version();
    let second_version: CommitVersion = second.commit().version();
    let first_timestamp: Timestamp = first.commit().timestamp();

    assert!(
        second_version > first_version,
        "{second_version:?} should follow {first_version:?}"
    );

    let read = kv
        .get_at(&KvKey::new("k").expect("key"), first_timestamp)
        .expect("read at timestamp")
        .expect("present at its own commit");
    assert_eq!(read.as_bytes(), b"v1");
}
