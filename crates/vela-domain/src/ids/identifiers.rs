//! Validated identifier value objects.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use super::path_safety::{PathComponent, is_windows_illegal_name_char, is_windows_reserved_name};

/// Why a candidate identifier was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdError {
    /// The value is empty.
    Empty,
    /// The value is longer than the type allows.
    TooLong {
        /// Maximum accepted length in bytes.
        max: usize,
    },
    /// The value contains a character the type does not allow.
    InvalidCharacter {
        /// The offending character.
        found: char,
    },
    /// The value violates a structural rule of the type (the message names it).
    Malformed(&'static str),
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("value is empty"),
            Self::TooLong { max } => write!(f, "value is longer than {max} bytes"),
            Self::InvalidCharacter { found } => write!(f, "value contains {found:?}"),
            Self::Malformed(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for IdError {}

const MAX_ID_LEN: usize = 128;

/// Logical identifiers (`RunId`, `TicketId`, `WorkerId`) are opaque tokens drawn from `[A-Za-z0-9._#-]`.
///
/// They are strict enough that every one of them can be encoded into a Windows-safe path component
/// ([`PathComponent::encode`]) and used inside a branch name: the first character is a letter or digit (no leading `-`,
/// `.` or `#`), there is no trailing `.` and no `..`, `~` is excluded (it marks encoded path components), and Windows
/// reserved device names are rejected. Letter case is preserved in the id and is significant, so ids that differ only in
/// case are distinct ids; they are kept apart on disk by [`PathComponent::encode`] (see [`case_fold_key`]).
fn validate_token(value: &str) -> Result<(), IdError> {
    if value.is_empty() {
        return Err(IdError::Empty);
    }
    if value.len() > MAX_ID_LEN {
        return Err(IdError::TooLong { max: MAX_ID_LEN });
    }
    if let Some(found) = value
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '#' | '-')))
    {
        return Err(IdError::InvalidCharacter { found });
    }
    if !value.starts_with(|c: char| c.is_ascii_alphanumeric()) {
        return Err(IdError::Malformed(
            "an identifier must start with a letter or digit",
        ));
    }
    if value.ends_with('.') {
        return Err(IdError::Malformed("an identifier cannot end with `.`"));
    }
    if value.contains("..") {
        return Err(IdError::Malformed("an identifier cannot contain `..`"));
    }
    if is_windows_reserved_name(value) {
        return Err(IdError::Malformed(
            "an identifier cannot be a Windows reserved device name",
        ));
    }
    // Git refuses a ref component ending in `.lock`, and ids are used as branch-name components.
    if value.to_ascii_lowercase().ends_with(".lock") {
        return Err(IdError::Malformed("an identifier cannot end with `.lock`"));
    }
    Ok(())
}

/// The case-folded form of an identifier, branch or path. Two values with the same fold key name the same thing on a
/// case-insensitive filesystem (NTFS), so adapters compare fold keys to detect collisions between distinct ids.
pub fn case_fold_key(value: &str) -> String {
    value.to_ascii_lowercase()
}

macro_rules! validated_string {
    ($(#[$doc:meta])* $name:ident, $validate:expr) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
        #[cfg_attr(feature = "full", derive(ts_rs::TS))]
        pub struct $name(String);

        impl $name {
            /// The value as a string slice.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = IdError;
            fn from_str(s: &str) -> Result<Self, IdError> {
                Self::new(s)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = String::deserialize(deserializer)?;
                Self::new(raw).map_err(serde::de::Error::custom)
            }
        }

        impl $name {
            /// Validates and wraps `value`.
            pub fn new(value: impl Into<String>) -> Result<Self, IdError> {
                let value: String = value.into();
                let validate: fn(&str) -> Result<String, IdError> = $validate;
                validate(&value).map(Self)
            }
        }
    };
}

validated_string! {
    /// Identifies one build run (immutable once created).
    RunId,
    |v| validate_token(v).map(|()| v.to_owned())
}

validated_string! {
    /// Identifies a ticket: the symbolic key (for example `F02`) or the tracker's identity, never a title.
    TicketId,
    |v| validate_token(v).map(|()| v.to_owned())
}

validated_string! {
    /// Identifies one worker: one ticket execution attempt bound to one session, branch and worktree.
    WorkerId,
    |v| validate_token(v).map(|()| v.to_owned())
}

validated_string! {
    /// Identifies one persisted operation (a Git operation record and its per-worktree lock, a merge-lane step).
    OperationId,
    |v| validate_token(v).map(|()| v.to_owned())
}

validated_string! {
    /// A full Git object id: 40 (SHA-1) or 64 (SHA-256) hexadecimal digits, normalized to lowercase.
    ///
    /// Abbreviated ids are rejected on purpose: a fixed point or base recorded as an abbreviation can become ambiguous.
    CommitSha,
    validate_commit_sha
}

validated_string! {
    /// A Git branch name (without the `refs/heads/` prefix) checked against the `git check-ref-format` rules.
    ///
    /// This is a conservative subset: anything it accepts is a valid ref name, but it may reject exotic valid names.
    BranchName,
    validate_branch_name
}

validated_string! {
    /// The filesystem path of a Vela-managed worktree, carried as a string (the domain performs no I/O).
    ///
    /// Rejected here: an empty path, control characters (NUL included), `..` components, Windows reserved device names
    /// (`CON`, `NUL`, `COM1`, ... with or without an extension) and components ending in a dot or space, the characters
    /// `< > " | ? *`, and any colon other than a leading drive (`C:`, optionally behind `\\?\`). Both `/` and `\`
    /// separate components. Whether the path exists, is inside the managed root, or is short enough is decided by the Git
    /// adapter and preflight, which can touch the filesystem.
    WorktreePath,
    validate_worktree_path
}

fn validate_commit_sha(value: &str) -> Result<String, IdError> {
    if value.is_empty() {
        return Err(IdError::Empty);
    }
    if let Some(found) = value.chars().find(|c| !c.is_ascii_hexdigit()) {
        return Err(IdError::InvalidCharacter { found });
    }
    if !matches!(value.len(), 40 | 64) {
        return Err(IdError::Malformed(
            "a commit id must be 40 or 64 hexadecimal digits",
        ));
    }
    Ok(value.to_ascii_lowercase())
}

fn validate_branch_name(value: &str) -> Result<String, IdError> {
    if value.is_empty() {
        return Err(IdError::Empty);
    }
    // Besides the `git check-ref-format` characters, reject what Windows forbids in a file name: loose refs are files.
    if let Some(found) = value
        .chars()
        .find(|c| c.is_control() || c.is_whitespace() || "~^?*[\\<>|\"".contains(*c) || *c == ':')
    {
        return Err(IdError::InvalidCharacter { found });
    }
    if value == "@" || value == "HEAD" || value.contains("@{") {
        return Err(IdError::Malformed(
            "a branch name cannot be `@` or `HEAD`, or contain `@{`",
        ));
    }
    if value.contains("..") || value.contains("//") {
        return Err(IdError::Malformed(
            "a branch name cannot contain `..` or `//`",
        ));
    }
    if value.starts_with('/') || value.ends_with('/') || value.starts_with('-') {
        return Err(IdError::Malformed(
            "a branch name cannot start with `/` or `-`, or end with `/`",
        ));
    }
    if value.ends_with('.') {
        return Err(IdError::Malformed("a branch name cannot end with `.`"));
    }
    for part in value.split('/') {
        if part.starts_with('.') || part.ends_with(".lock") {
            return Err(IdError::Malformed(
                "a branch name component cannot start with `.` or end with `.lock`",
            ));
        }
        if part.ends_with('.') {
            return Err(IdError::Malformed(
                "a branch name component cannot end with `.` (Windows strips it)",
            ));
        }
        if is_windows_reserved_name(part) {
            return Err(IdError::Malformed(
                "a branch name component cannot be a Windows reserved device name",
            ));
        }
    }
    Ok(value.to_owned())
}

fn validate_worktree_path(value: &str) -> Result<String, IdError> {
    if value.trim().is_empty() {
        return Err(IdError::Empty);
    }
    if let Some(found) = value.chars().find(|c| c.is_control()) {
        return Err(IdError::InvalidCharacter { found });
    }
    // An optional `\\?\` or `\\.\` verbatim prefix and an optional drive (`C:`) may precede the components; any other
    // colon, and the other Windows-illegal characters, are rejected.
    let rest = value
        .strip_prefix(r"\\?\")
        .or_else(|| value.strip_prefix(r"\\.\"))
        .unwrap_or(value);
    let rest = match rest.as_bytes() {
        [drive, b':'] if drive.is_ascii_alphabetic() => "",
        [drive, b':', b'/' | b'\\', ..] if drive.is_ascii_alphabetic() => &rest[2..],
        _ => rest,
    };
    if let Some(found) = rest
        .chars()
        .find(|c| is_windows_illegal_name_char(*c) && !matches!(c, '/' | '\\'))
    {
        return Err(IdError::InvalidCharacter { found });
    }
    for component in rest.split(['/', '\\']) {
        if component == ".." {
            return Err(IdError::Malformed(
                "a worktree path cannot contain a `..` component",
            ));
        }
        if component != "." && (component.ends_with('.') || component.ends_with(' ')) {
            return Err(IdError::Malformed(
                "a worktree path component cannot end with `.` or a space (Windows strips it)",
            ));
        }
        if is_windows_reserved_name(component) {
            return Err(IdError::Malformed(
                "a worktree path component cannot be a Windows reserved device name",
            ));
        }
    }
    Ok(value.to_owned())
}

/// Path-component and case-collision helpers for the logical identifiers.
macro_rules! path_safe_id {
    ($($name:ident),+) => {$(
        impl $name {
            /// The Windows-safe directory name for this identifier ([`PathComponent::encode`]): lowercase, never a
            /// reserved name, bounded in length, and distinct for distinct ids even when they differ only in case.
            pub fn path_component(&self) -> PathComponent {
                PathComponent::encode(&self.0)
            }

            /// Case-folded form; equal fold keys mean a collision on a case-insensitive filesystem.
            pub fn case_fold_key(&self) -> String {
                case_fold_key(&self.0)
            }
        }
    )+};
}

path_safe_id!(RunId, TicketId, WorkerId, OperationId, BranchName);

impl CommitSha {
    /// The first `len` digits (at most the full id), for display only.
    pub fn short(&self, len: usize) -> &str {
        &self.0[..len.min(self.0.len())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_ids_accept_the_logical_alphabet_and_reject_the_rest() {
        assert_eq!(RunId::new("run-01H").unwrap().as_str(), "run-01H");
        assert_eq!(TicketId::new("F02").unwrap().to_string(), "F02");
        assert!(TicketId::new("gh#42").is_ok());
        assert_eq!(WorkerId::new(""), Err(IdError::Empty));
        assert_eq!(
            WorkerId::new("a b"),
            Err(IdError::InvalidCharacter { found: ' ' })
        );
        assert_eq!(
            TicketId::new("x".repeat(129)),
            Err(IdError::TooLong { max: 128 })
        );
    }

    #[test]
    fn token_ids_reject_windows_unsafe_values() {
        let unsafe_ids = [
            "a/b", "a\\b", "C:", "a:b", "..", "a..b", ".", "-rf", "-", ".hidden", "#x", "run-1.",
            "a.", "a<b", "a>b", "a|b", "a\"b", "a*b", "a?b", "a~b", "a b", "a\tb", "a\0b", "é",
            "名前", "CON", "con", "Nul", "aux.txt", "COM1", "lpt9", "com1.log",
        ];
        for id in unsafe_ids {
            assert!(RunId::new(id).is_err(), "RunId accepted {id:?}");
            assert!(TicketId::new(id).is_err(), "TicketId accepted {id:?}");
            assert!(WorkerId::new(id).is_err(), "WorkerId accepted {id:?}");
        }
        for fine in ["console", "com10", "lpt", "a.b", "a_b", "9", "x-"] {
            assert!(RunId::new(fine).is_ok(), "{fine:?}");
        }
    }

    #[test]
    fn ids_ending_in_lock_are_rejected_because_git_refuses_such_ref_components() {
        for id in ["F02.lock", "run.lock", "a.LOCK", "x.Lock"] {
            assert!(TicketId::new(id).is_err(), "{id}");
        }
        assert!(TicketId::new("locksmith").is_ok());
        assert!(TicketId::new("a.lock.b").is_ok());
    }

    #[test]
    fn every_accepted_id_is_usable_as_a_branch_name_component() {
        let accepted = [
            "F02",
            "f02",
            "A",
            "9",
            "a.b",
            "a_b",
            "a-b",
            "x-",
            "gh#42",
            "Run-1",
            "a.lockx",
            "z".repeat(60).as_str(),
        ];
        for id in accepted {
            assert!(RunId::new(id).is_ok(), "{id}");
            let branch = format!("vela/{id}/{id}");
            assert!(BranchName::new(branch.as_str()).is_ok(), "{branch}");
        }
    }

    #[test]
    fn head_is_not_a_branch_name() {
        assert!(BranchName::new("HEAD").is_err());
        assert!(BranchName::new("head").is_ok());
        assert!(BranchName::new("HEAD2").is_ok());
        assert!(BranchName::new("feature/HEAD").is_ok());
    }

    #[test]
    fn every_accepted_id_encodes_to_a_valid_path_component() {
        for id in [
            "F02",
            "Run-1",
            "run-1",
            "gh#42",
            "a.b",
            "A",
            "x".repeat(128).as_str(),
        ] {
            let run = RunId::new(id).unwrap();
            assert!(
                PathComponent::new(run.path_component().as_str()).is_ok(),
                "{id}"
            );
        }
    }

    #[test]
    fn ids_differing_only_in_case_are_distinct_ids_with_distinct_path_components() {
        let lower = RunId::new("run-1").unwrap();
        let upper = RunId::new("Run-1").unwrap();
        assert_ne!(lower, upper);
        assert_eq!(lower.case_fold_key(), upper.case_fold_key());
        assert_ne!(lower.path_component(), upper.path_component());
        assert_ne!(
            lower.path_component().as_str().to_ascii_lowercase(),
            upper.path_component().as_str().to_ascii_lowercase()
        );
        // An already-canonical id is its own component, so existing layouts are stable.
        assert_eq!(lower.path_component().as_str(), "run-1");
        // `#` is allowed in an id but never in a directory name.
        assert!(
            !TicketId::new("gh#42")
                .unwrap()
                .path_component()
                .as_str()
                .contains('#')
        );
    }

    #[test]
    fn worker_ids_whose_names_alias_on_ntfs_do_not_share_a_directory() {
        // "f02" and "F02" are the same directory on NTFS; their components must differ.
        let a = WorkerId::new("F02").unwrap().path_component();
        let b = WorkerId::new("f02").unwrap().path_component();
        assert_ne!(
            a.as_str().to_ascii_lowercase(),
            b.as_str().to_ascii_lowercase()
        );
    }

    #[test]
    fn commit_sha_requires_a_full_hex_id_and_lowercases() {
        let sha = CommitSha::new("3E5AAFE745EF11887380E0EC9A5DAD76FDB3D41D").unwrap();
        assert_eq!(sha.as_str(), "3e5aafe745ef11887380e0ec9a5dad76fdb3d41d");
        assert_eq!(sha.short(7), "3e5aafe");
        assert!(CommitSha::new("3e5aafe").is_err());
        assert!(CommitSha::new("z".repeat(40)).is_err());
        assert!(CommitSha::new("a".repeat(64)).is_ok());
        assert_eq!(CommitSha::new(""), Err(IdError::Empty));
    }

    #[test]
    fn branch_names_follow_the_ref_format_rules() {
        assert!(BranchName::new("f02/domain-primitives").is_ok());
        assert!(BranchName::new("vela/run-1/issue-7-slug").is_ok());
        for bad in [
            "",
            "a..b",
            "a//b",
            "/a",
            "a/",
            "-a",
            "a.",
            "a b",
            "a~1",
            "a^",
            "a:b",
            "a?",
            "a*",
            "a[",
            "a\\b",
            "@",
            "a@{b",
            "a/.hidden",
            "a/b.lock",
            "a<b",
            "a>b",
            "a|b",
            "a\"b",
            "con",
            "f02/CON",
            "a/nul.txt",
            "x/aux",
            "a./b",
            "a/b.",
        ] {
            assert!(BranchName::new(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn branch_names_that_differ_only_in_case_collide_by_fold_key_but_not_as_path_components() {
        let a = BranchName::new("F02/Domain").unwrap();
        let b = BranchName::new("f02/domain").unwrap();
        assert_ne!(a, b);
        assert_eq!(a.case_fold_key(), b.case_fold_key());
        assert_ne!(
            a.path_component().as_str().to_ascii_lowercase(),
            b.path_component().as_str().to_ascii_lowercase()
        );
    }

    #[test]
    fn worktree_path_accepts_normal_windows_and_posix_paths() {
        for ok in [
            r"C:\Users\x\Vela\worktrees\f02",
            r"c:/Users/x/Vela",
            r"\\?\C:\Users\x\Vela",
            r"\\server\share\vela",
            "/repo",
            "relative/path",
            "./here",
        ] {
            assert!(WorktreePath::new(ok).is_ok(), "{ok:?}");
        }
    }

    #[test]
    fn worktree_path_rejects_traversal_reserved_names_and_illegal_characters() {
        assert_eq!(WorktreePath::new("  "), Err(IdError::Empty));
        for bad in [
            "a\0b",
            r"C:\x\..\y",
            "../x",
            "a/../b",
            r"C:\x\CON",
            r"C:\x\nul.txt",
            r"C:\x\aux\y",
            r"C:\x\a.",
            r"C:\x\a ",
            r"C:\x\a?b",
            r"C:\x\a*b",
            r"C:\x\a<b",
            r"C:\x\a|b",
            "a:b/c",
            "x\ny",
        ] {
            assert!(WorktreePath::new(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[cfg(feature = "full")]
    #[test]
    fn identifiers_round_trip_as_plain_json_strings_and_validate_on_read() {
        let id = RunId::new("run-1").unwrap();
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"run-1\"");
        assert_eq!(serde_json::from_str::<RunId>("\"run-1\"").unwrap(), id);
        assert!(serde_json::from_str::<RunId>("\"bad id\"").is_err());
        assert!(serde_json::from_str::<CommitSha>("\"abc\"").is_err());
        assert!(serde_json::from_str::<BranchName>("\"a..b\"").is_err());
    }
}
