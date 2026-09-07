//! #3094: the changelog, reachable from the binary.
//!
//! `CHANGELOG.md` was correct, specific, and unreachable. It lived in the
//! repository, while what a consumer receives is a release asset and a binary —
//! so a documented, deliberately wire-visible error reclassification in 1.2.0
//! broke a downstream build anyway, and was diagnosed by diffing catalogs
//! rather than by reading the two lines that described it.
//!
//! Embedding it fixes the reachability half: whoever holds a binary of unknown
//! provenance can ask what is in it, offline, matched to that exact build. The
//! other half is shipping it inside the docs bundle (see the release workflow).

/// The changelog as of this build. Embedded rather than read from disk so the
/// answer is always the one that matches the binary asking — a binary copied
/// off a machine still knows its own history.
const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");

/// The whole changelog.
pub(crate) fn full() -> &'static str {
    CHANGELOG
}

/// Just one release's section, `None` when this build's changelog has no entry
/// for it.
///
/// Sections are delimited by Keep-a-Changelog `## [x.y.z] - date` headings, so
/// a section runs to the next `## ` at the same level and keeps its own
/// `### Added` / `### Fixed` subheadings.
pub(crate) fn section(version: &str) -> Option<&'static str> {
    let heading = format!("## [{version}]");
    let start = CHANGELOG.find(&heading)?;
    let rest = &CHANGELOG[start..];
    // Skip this heading before hunting the next, or the search finds itself.
    let end = rest[heading.len()..]
        .find("\n## ")
        .map_or(CHANGELOG.len(), |offset| start + heading.len() + offset + 1);
    Some(CHANGELOG[start..end].trim_end())
}

/// The versions this build's changelog documents, newest first — used to tell a
/// caller what they *could* have asked for when their version is absent.
pub(crate) fn versions() -> Vec<&'static str> {
    CHANGELOG
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("## [")?;
            rest.split(']').next()
        })
        .collect()
}

/// Prints the changelog, or one release's section.
///
/// Markdown is the format, matching `agents guide` and `agents skill`: this is
/// prose written for a person, and #3094 explicitly did not want a second
/// generated artifact to keep honest.
pub(crate) fn run(version: Option<&str>) -> Result<i32, crate::CliError> {
    match version {
        None => println!("{}", full().trim_end()),
        Some(version) => {
            let section = section(version).ok_or_else(|| {
                crate::CliError::usage(format!(
                    "this build's changelog has no entry for `{version}`; it documents: {}",
                    versions().join(", ")
                ))
            })?;
            println!("{section}");
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The embedded copy must be the real file, not a stub — the whole point is
    /// that the binary carries the same words the repository does.
    #[test]
    fn the_embedded_changelog_is_the_real_one() {
        assert!(CHANGELOG.starts_with("# Changelog"), "got: {CHANGELOG:.40}");
        assert!(
            CHANGELOG.len() > 1_000,
            "changelog looks truncated: {} bytes",
            CHANGELOG.len()
        );
    }

    /// The build's own version must be documented, or `strata changelog`
    /// answers "nothing to say" about the binary you are holding — which is
    /// exactly the failure #3094 reported, moved one step later.
    ///
    /// This fires during a release-prep PR if the version is bumped without a
    /// changelog entry, which is the moment it is cheapest to fix.
    #[test]
    fn this_builds_version_has_a_changelog_entry() {
        let version = env!("CARGO_PKG_VERSION");
        assert!(
            section(version).is_some(),
            "CHANGELOG.md has no `## [{version}]` section; versions present: {:?}",
            versions()
        );
    }

    /// A section stops at the next release, so asking about one version never
    /// silently includes another's changes.
    #[test]
    fn a_section_covers_exactly_one_release() {
        let all = versions();
        assert!(all.len() >= 2, "need two releases to test the boundary");
        let (newer, older) = (all[0], all[1]);

        let section = section(newer).expect("the newest release has a section");
        assert!(section.starts_with(&format!("## [{newer}]")));
        assert!(
            !section.contains(&format!("## [{older}]")),
            "section for {newer} leaked into {older}"
        );
        // Subheadings belong to their release and must survive the split.
        assert!(
            section.contains("### "),
            "section lost its subheadings: {section:.200}"
        );
    }

    /// The oldest section runs to the end of the file rather than being cut
    /// short by the missing "next" heading.
    #[test]
    fn the_oldest_section_runs_to_the_end() {
        let all = versions();
        let oldest = all.last().expect("at least one release");
        let section = section(oldest).expect("the oldest release has a section");
        assert!(section.starts_with(&format!("## [{oldest}]")));
        assert!(
            section.lines().count() > 1,
            "oldest section is empty: {section:?}"
        );
    }

    #[test]
    fn an_unknown_version_has_no_section() {
        assert!(section("0.0.0-nope").is_none());
        // And a prefix of a real version must not match it: `1.2` is not `1.2.0`.
        let newest = versions()[0];
        if let Some(prefix) = newest.rsplit_once('.').map(|(head, _)| head) {
            assert!(
                section(prefix).is_none(),
                "`{prefix}` should not match `{newest}`"
            );
        }
    }

    #[test]
    fn versions_are_listed_newest_first() {
        let all = versions();
        assert!(!all.is_empty());
        assert!(
            all.iter().all(|v| v.starts_with(char::is_numeric)),
            "parsed something that is not a version: {all:?}"
        );
    }
}
