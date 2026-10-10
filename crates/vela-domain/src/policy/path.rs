//! Lexical Windows path resolution for the policy engine.
//!
//! The policy engine is pure and does no I/O, so it can only reason about paths lexically. A path is resolved against a
//! base into a case-folded component list and compared with the assigned worktree root. Anything the engine cannot
//! establish lexically (device namespaces, alternate data streams, short names, environment or home expansion, a name
//! Windows would silently rewrite) is [`Placement::Unresolvable`], which no rule turns into an `ALLOW`.
//!
//! **Precondition:** the caller supplies paths with symbolic links and junctions already resolved (the process and Git
//! adapters own that); a lexically inside path that a link redirects outside the root is outside the engine's view.

use serde::{Deserialize, Serialize};

use crate::ids::{case_fold_key, is_windows_reserved_name};

/// Longest path, in bytes, the engine will resolve.
const MAX_PATH_LEN: usize = 1024;

/// Where a path lies relative to the assigned worktree root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    /// At or below the worktree root.
    InsideRoot,
    /// Resolved, and not at or below the worktree root.
    OutsideRoot,
    /// Could not be resolved lexically; never treated as inside.
    Unresolvable,
}

/// Why a path could not be resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PathIssue {
    Empty,
    TooLong,
    ControlOrWildcard,
    DeviceNamespace,
    NotAbsolute,
    DriveRelative,
    HomeOrVariable,
    AlternateStream,
    ShortNameOrAlias,
    ReservedName,
    EscapesPrefix,
}

/// A path resolved lexically to an absolute, case-folded component list.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct ResolvedPath {
    /// Case-folded drive (`c:`) or UNC share (`\\server\share`).
    prefix: String,
    /// The prefix with its original case.
    prefix_shown: String,
    /// Components below the prefix as (case-folded, original).
    parts: Vec<(String, String)>,
}

impl std::fmt::Debug for ResolvedPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Paths can carry user names; the debug form shows only the depth.
        write!(f, "ResolvedPath(depth {})", self.parts.len())
    }
}

/// How a resolved path relates to a root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Relation {
    /// The same path.
    Equal,
    /// Strictly below the root.
    Below,
    /// A strict ancestor of the root.
    Above,
    /// Neither.
    Disjoint,
}

impl ResolvedPath {
    /// Resolves `raw` against `base` (itself absolute and already resolved) or, when `raw` is absolute, on its own.
    pub(crate) fn resolve(
        base: Option<&ResolvedPath>,
        raw: &str,
    ) -> Result<ResolvedPath, PathIssue> {
        if raw.trim().is_empty() {
            return Err(PathIssue::Empty);
        }
        if raw.len() > MAX_PATH_LEN {
            return Err(PathIssue::TooLong);
        }
        if raw.chars().any(|c| c.is_control() || "<>|\"*?".contains(c)) {
            return Err(PathIssue::ControlOrWildcard);
        }
        if raw.contains('$') || raw.contains('%') || raw.starts_with('~') {
            return Err(PathIssue::HomeOrVariable);
        }
        let text = raw.replace('/', "\\");
        if text.starts_with("\\\\?\\") || text.starts_with("\\\\.\\") {
            return Err(PathIssue::DeviceNamespace);
        }

        let (prefix, prefix_shown, mut parts, rest): (String, String, Vec<(String, String)>, &str) =
            if let Some(unc) = text.strip_prefix("\\\\") {
                let mut it = unc.splitn(3, '\\');
                let (server, share) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
                if server.is_empty() || share.is_empty() {
                    return Err(PathIssue::NotAbsolute);
                }
                let shown = format!("\\\\{server}\\{share}");
                (
                    case_fold_key(&shown),
                    shown,
                    Vec::new(),
                    it.next().unwrap_or(""),
                )
            } else if let Some(drive) = drive_prefix(&text) {
                let after = &text[2..];
                if !after.starts_with('\\') {
                    return Err(PathIssue::DriveRelative);
                }
                let shown = format!("{}:", drive.to_ascii_uppercase());
                (case_fold_key(&shown), shown, Vec::new(), &after[1..])
            } else if text.starts_with('\\') {
                // Rooted on the current drive: depends on state the engine does not have.
                return Err(PathIssue::NotAbsolute);
            } else if text.contains(':') {
                return Err(PathIssue::AlternateStream);
            } else {
                match base {
                    Some(b) => (
                        b.prefix.clone(),
                        b.prefix_shown.clone(),
                        b.parts.clone(),
                        text.as_str(),
                    ),
                    None => return Err(PathIssue::NotAbsolute),
                }
            };

        for segment in rest.split('\\') {
            match segment {
                "" | "." => {}
                ".." => {
                    if parts.pop().is_none() {
                        return Err(PathIssue::EscapesPrefix);
                    }
                }
                other => {
                    check_segment(other)?;
                    parts.push((case_fold_key(other), other.to_string()));
                }
            }
        }
        Ok(ResolvedPath {
            prefix,
            prefix_shown,
            parts,
        })
    }

    /// The resolved path with its original case.
    pub(crate) fn display(&self) -> String {
        let mut out = self.prefix_shown.clone();
        for (_, shown) in &self.parts {
            out.push('\\');
            out.push_str(shown);
        }
        if self.parts.is_empty() {
            out.push('\\');
        }
        out
    }

    /// True when the path is a drive or share root.
    pub(crate) fn is_prefix_root(&self) -> bool {
        self.parts.is_empty()
    }

    /// How `self` relates to `root`.
    pub(crate) fn relation_to(&self, root: &ResolvedPath) -> Relation {
        if self.prefix != root.prefix {
            return Relation::Disjoint;
        }
        let common = self
            .parts
            .iter()
            .zip(&root.parts)
            .take_while(|(a, b)| a.0 == b.0)
            .count();
        match (common == self.parts.len(), common == root.parts.len()) {
            (true, true) => Relation::Equal,
            (false, true) => Relation::Below,
            (true, false) => Relation::Above,
            (false, false) => Relation::Disjoint,
        }
    }

    /// Placement for reads and writes: at or below the root is inside.
    pub(crate) fn placement(&self, root: &ResolvedPath) -> Placement {
        match self.relation_to(root) {
            Relation::Equal | Relation::Below => Placement::InsideRoot,
            Relation::Above | Relation::Disjoint => Placement::OutsideRoot,
        }
    }

    /// The case-folded components below the prefix.
    pub(crate) fn components_folded(&self) -> Vec<String> {
        self.folded().map(str::to_string).collect()
    }

    fn folded(&self) -> impl Iterator<Item = &str> {
        self.parts.iter().map(|p| p.0.as_str())
    }

    /// True when any component equals one of `names` (already case-folded).
    pub(crate) fn has_component(&self, names: &[&str]) -> bool {
        self.folded().any(|c| names.contains(&c))
    }
}

fn drive_prefix(text: &str) -> Option<char> {
    let mut chars = text.chars();
    let letter = chars.next()?;
    (letter.is_ascii_alphabetic() && chars.next() == Some(':')).then_some(letter)
}

/// Rejects names Windows would silently rewrite or reinterpret.
fn check_segment(segment: &str) -> Result<(), PathIssue> {
    if segment.ends_with('.') || segment.ends_with(' ') || segment.starts_with(' ') {
        return Err(PathIssue::ShortNameOrAlias);
    }
    if segment.contains(':') {
        return Err(PathIssue::AlternateStream);
    }
    // 8.3 short names (`PROGRA~1`) alias a long name the engine cannot see.
    let bytes = segment.as_bytes();
    if bytes
        .windows(2)
        .any(|w| w[0] == b'~' && w[1].is_ascii_digit())
    {
        return Err(PathIssue::ShortNameOrAlias);
    }
    if is_windows_reserved_name(segment) {
        return Err(PathIssue::ReservedName);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> ResolvedPath {
        ResolvedPath::resolve(None, r"C:\Work\Repo").unwrap()
    }

    #[test]
    fn resolves_relative_paths_against_the_base_and_folds_case() {
        let r = root();
        let p = ResolvedPath::resolve(Some(&r), r"src\..\SRC/main.rs").unwrap();
        assert_eq!(p.display(), r"C:\Work\Repo\SRC\main.rs");
        assert_eq!(p.placement(&r), Placement::InsideRoot);
        let q = ResolvedPath::resolve(None, r"c:\WORK\repo\x").unwrap();
        assert_eq!(q.placement(&r), Placement::InsideRoot);
    }

    #[test]
    fn classifies_siblings_prefixes_and_parents_as_outside() {
        let r = root();
        for raw in [
            r"C:\Work\Repo2\x",
            r"C:\Work",
            r"D:\Work\Repo",
            r"C:\Work\Repo\..\Other",
            r"\\srv\share\Work\Repo",
        ] {
            let p = ResolvedPath::resolve(Some(&r), raw).unwrap();
            assert_eq!(p.placement(&r), Placement::OutsideRoot, "{raw}");
        }
        let root_itself = ResolvedPath::resolve(Some(&r), ".").unwrap();
        assert_eq!(root_itself.relation_to(&r), Relation::Equal);
        let parent = ResolvedPath::resolve(Some(&r), "..").unwrap();
        assert_eq!(parent.relation_to(&r), Relation::Above);
    }

    #[test]
    fn refuses_what_it_cannot_establish() {
        let r = root();
        for raw in [
            "",
            r"\\?\C:\Work\Repo\x",
            r"\\.\PhysicalDrive0",
            r"C:foo",
            r"\rooted",
            r"~\x",
            r"$env:TEMP\x",
            r"%TEMP%\x",
            r"file.txt:stream",
            r"PROGRA~1\x",
            r"dir.\x",
            r"dir \x",
            r"NUL",
            r"con.txt",
            r"..\..\..\..\..\x",
            "a\0b",
            r"a*b",
        ] {
            assert!(ResolvedPath::resolve(Some(&r), raw).is_err(), "{raw:?}");
        }
        assert!(ResolvedPath::resolve(None, "relative").is_err());
    }

    #[test]
    fn component_helpers_see_protected_names() {
        let r = root();
        let p = ResolvedPath::resolve(Some(&r), r".GIT\hooks\pre-commit").unwrap();
        assert!(p.has_component(&[".git"]));
        assert_eq!(
            p.components_folded(),
            ["work", "repo", ".git", "hooks", "pre-commit"]
        );
    }
}
