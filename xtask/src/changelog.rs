//! The changelog: one `CHANGELOG.md`
//! for the workspace, since every crate is released at one version, with
//! the newest entry first. A version is released only with its entry: a
//! `## <version>` heading, once, at the top, with something under it.
//!
//! `cargo xtask release` (A7) refuses a version these checks find a problem
//! with, and reads the versions released before it from here; the test
//! below also holds the tree to them.

use std::path::Path;

use crate::error::Result;
use crate::fsx;

/// The heading an entry starts with.
const ENTRY: &str = "## ";

/// The changelog's text.
pub(crate) fn read(root: &Path) -> Result<String> {
    fsx::read_to_string(&root.join("CHANGELOG.md"))
}

/// Each entry, newest first: its version and whether any text follows the
/// heading.
fn entries(changelog: &str) -> Vec<(&str, bool)> {
    let mut entries: Vec<(&str, bool)> = Vec::new();
    for line in changelog.lines() {
        if let Some(heading) = line.strip_prefix(ENTRY) {
            let v = heading.split_whitespace().next().unwrap_or_default();
            entries.push((v, false));
        } else if let Some((_, body)) = entries.last_mut() {
            *body |= !line.trim().is_empty();
        }
    }
    entries
}

/// The versions released before `version`: the entries below its own,
/// newest first. Empty for the first release.
pub(crate) fn earlier<'c>(changelog: &'c str, version: &str) -> Vec<&'c str> {
    entries(changelog)
        .into_iter()
        .map(|(v, _)| v)
        .skip_while(|v| *v != version)
        .skip(1)
        .collect()
}

/// Every problem with `changelog` as the entry for `version`, one line each.
pub(crate) fn problems(changelog: &str, version: &str) -> Vec<String> {
    let mut out = Vec::new();
    let entries = entries(changelog);
    let found: Vec<bool> = entries
        .iter()
        .filter(|(v, _)| *v == version)
        .map(|(_, body)| *body)
        .collect();
    match found.as_slice() {
        [] => out.push(format!("CHANGELOG.md: no `{ENTRY}{version}` entry")),
        [body] => {
            if !body {
                out.push(format!("CHANGELOG.md: the {version} entry is empty"));
            }
        }
        more => out.push(format!(
            "CHANGELOG.md: {} `{ENTRY}{version}` entries",
            more.len()
        )),
    }
    if let Some((first, _)) = entries.first()
        && *first != version
        && !found.is_empty()
    {
        out.push(format!(
            "CHANGELOG.md: the newest entry is {first}, not {version}"
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{earlier, problems};
    use crate::fsx::repo_root;

    /// `[workspace.package] version`: what every published crate is
    /// released at.
    fn workspace_version() -> String {
        let manifest = std::fs::read_to_string(repo_root().join("Cargo.toml")).unwrap();
        let manifest: toml::Table = manifest.parse().unwrap();
        manifest["workspace"]["package"]["version"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn the_version_has_its_entry() {
        let text = std::fs::read_to_string(repo_root().join("CHANGELOG.md")).unwrap();
        let found = problems(&text, &workspace_version());
        assert!(found.is_empty(), "{}", found.join("\n"));
    }

    // Negative controls: each way a release could go out without its
    // entry is caught.

    const TWO: &str = "# Changelog\n\n## 1.0.1\n\nFixed.\n\n## 1.0.0\n\nFirst.\n";

    #[test]
    fn a_version_with_no_entry_is_refused() {
        assert_eq!(
            problems(TWO, "1.1.0"),
            ["CHANGELOG.md: no `## 1.1.0` entry"]
        );
    }

    #[test]
    fn an_empty_entry_is_refused() {
        let text = "## 1.0.1\n\n\n## 1.0.0\n\nFirst.\n";
        assert_eq!(
            problems(text, "1.0.1"),
            ["CHANGELOG.md: the 1.0.1 entry is empty"]
        );
    }

    #[test]
    fn an_entry_below_a_newer_one_is_refused() {
        assert_eq!(
            problems(TWO, "1.0.0"),
            ["CHANGELOG.md: the newest entry is 1.0.1, not 1.0.0"]
        );
    }

    #[test]
    fn a_repeated_entry_is_refused() {
        let text = "## 1.0.0\n\nFirst.\n\n## 1.0.0\n\nAgain.\n";
        assert_eq!(
            problems(text, "1.0.0"),
            ["CHANGELOG.md: 2 `## 1.0.0` entries"]
        );
    }

    #[test]
    fn the_newest_entry_is_accepted() {
        assert!(problems(TWO, "1.0.1").is_empty(), "{:?}", problems(TWO, "1.0.1"));
    }

    #[test]
    fn the_earlier_versions_are_those_below_the_entry() {
        assert_eq!(earlier(TWO, "1.0.1"), ["1.0.0"]);
        assert!(earlier(TWO, "1.0.0").is_empty(), "{:?}", earlier(TWO, "1.0.0"));
    }
}
