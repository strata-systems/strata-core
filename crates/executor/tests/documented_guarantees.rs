//! Which commands publish their guaranteed semantics (#3115).
//!
//! A `# Guaranteed semantics` block on a `Command` variant becomes the
//! generated schema's `description`, which stratadb.org renders. Those blocks
//! are the most valuable thing in the reference: they remove work a caller
//! would otherwise do defensively — checking for a partial batch, re-reading
//! after a write, wondering whether a listing can shift mid-pagination.
//!
//! When #3115 was filed the blocks existed on **3 of 135 commands**, and
//! nothing made that visible. This guard makes coverage a fact the build
//! knows: families listed as complete must stay complete, and the count of
//! documented commands may not fall.

use std::collections::BTreeSet;
use std::path::PathBuf;

/// Families where every command publishes its guarantees. Adding a command to
/// one of these without a block fails the test.
///
/// **This list may only grow.** Extending guarantees to a family and adding it
/// here is the intended direction of travel.
const COMPLETE_FAMILIES: &[&str] = &["kv"];

/// The number of commands carrying a block, which may not fall.
///
/// Raise it when you document more. It is a ratchet, not a target: the point
/// is that coverage cannot quietly regress the way it sat at 3 unnoticed.
const DOCUMENTED_FLOOR: usize = 15;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonicalize repository root above crates/executor")
}

/// Every `family.verb` id in the IDL command catalog.
fn command_ids() -> BTreeSet<String> {
    let catalog = repo_root().join("crates/executor/idl/v1/commands");
    let mut ids = BTreeSet::new();
    let entries = std::fs::read_dir(&catalog)
        .unwrap_or_else(|error| panic!("read {}: {error}", catalog.display()));
    for entry in entries {
        let path = entry.expect("catalog dir entry").path();
        if path.extension().is_none_or(|ext| ext != "yaml") {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        for line in text.lines() {
            if let Some(id) = line.trim().strip_prefix("- id:") {
                ids.insert(id.trim().to_owned());
            }
        }
    }
    assert!(
        ids.len() > 100,
        "parsed only {} command ids — the catalog parser has drifted",
        ids.len()
    );
    ids
}

/// True when the command's generated schema publishes a guarantee block.
///
/// Reads the schema rather than the Rust source: the schema is what actually
/// ships to readers, so a block that failed to reach it does not count.
fn publishes_guarantees(id: &str) -> bool {
    let schema = repo_root()
        .join("crates/executor/idl/v1/generated/schemas")
        .join(format!("{id}.json"));
    let Ok(text) = std::fs::read_to_string(&schema) else {
        return false;
    };
    text.contains("# Guaranteed semantics")
}

/// Every command in a complete family publishes its guarantees.
#[test]
fn complete_families_document_every_command() {
    let mut missing = Vec::new();
    for id in command_ids() {
        let family = id.split('.').next().expect("ids are family.verb");
        if COMPLETE_FAMILIES.contains(&family) && !publishes_guarantees(&id) {
            missing.push(id);
        }
    }
    assert!(
        missing.is_empty(),
        "these commands are in a family listed COMPLETE but publish no \
         `# Guaranteed semantics` block: {missing:?}\n\
         Add the block to the Command variant, regenerate the schemas, and \
         pin each claim with a test."
    );
}

/// Guarantee coverage never falls.
#[test]
fn documented_command_count_does_not_regress() {
    let documented: Vec<String> = command_ids()
        .into_iter()
        .filter(|id| publishes_guarantees(id))
        .collect();
    assert!(
        documented.len() >= DOCUMENTED_FLOOR,
        "guarantee coverage fell to {} commands, below the floor of {DOCUMENTED_FLOOR}: {documented:?}",
        documented.len()
    );
}

/// A family claimed complete must actually exist in the catalog — otherwise the
/// first test above passes by covering nothing.
#[test]
fn complete_families_are_real_and_non_empty() {
    let ids = command_ids();
    for family in COMPLETE_FAMILIES {
        let count = ids
            .iter()
            .filter(|id| id.split('.').next() == Some(*family))
            .count();
        assert!(
            count > 0,
            "COMPLETE_FAMILIES names `{family}`, which has no commands in the catalog"
        );
    }
}
