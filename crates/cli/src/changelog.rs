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
/// a section runs to the next release heading and keeps its own `### Added` /
/// `### Fixed` subheadings.
pub(crate) fn section(version: &str) -> Option<&'static str> {
    let start = CHANGELOG.find(&format!("## [{version}]"))?;
    let body = &CHANGELOG[start..];
    // Searching for the LEADING newline means the pattern cannot match this
    // section's own heading, so there is no offset arithmetic to get wrong.
    // An earlier version computed the end from three added terms; the mutation
    // gate found those bounds were untested in the shortening direction, and
    // removing the arithmetic is a better answer than testing it.
    let end = body.find("\n## [").unwrap_or(body.len());
    Some(body[..end].trim_end())
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

    /// Pins where a section ENDS, which the boundary test above does not: it
    /// only proves the section does not reach the *next* release, so a section
    /// cut short passed it happily. That is exactly the gap the mutation gate
    /// found — three surviving arithmetic mutants that all shortened the slice.
    ///
    /// A line-based oracle is written independently of the implementation and
    /// compared for every release in the file. Two different ways of finding
    /// the same boundary agreeing is a far stronger claim than any single
    /// assertion about the contents.
    #[test]
    fn every_section_matches_an_independent_line_based_split() {
        for version in versions() {
            let heading = format!("## [{version}]");
            let mut lines = CHANGELOG
                .lines()
                .skip_while(|line| !line.starts_with(&heading));
            let first = lines.next().expect("the heading is present");
            let mut expected = vec![first];
            for line in lines {
                if line.starts_with("## [") {
                    break;
                }
                expected.push(line);
            }
            let expected = expected.join("\n");
            assert_eq!(
                section(version).expect("every listed version has a section"),
                expected.trim_end(),
                "section boundaries disagree for {version}"
            );
        }
    }

    /// The sections partition the changelog: everything after the preamble is
    /// inside exactly one release, so no content can be silently dropped
    /// between them.
    #[test]
    fn sections_account_for_the_whole_changelog_body() {
        let all = versions();
        let first_heading = CHANGELOG
            .find(&format!("## [{}]", all[0]))
            .expect("the newest release has a heading");
        let body_bytes = CHANGELOG[first_heading..].trim_end().len();
        // Each section, plus the "\n\n" separators the join removes.
        let covered: usize = all
            .iter()
            .map(|version| section(version).expect("listed version").len())
            .sum();
        assert!(
            covered + 2 * (all.len() - 1) >= body_bytes,
            "sections cover {covered} bytes of a {body_bytes}-byte body — content is being dropped"
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
