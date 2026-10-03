//! Validated identifier value objects.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

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

/// Identifiers are opaque tokens: visible ASCII only, no whitespace or control characters.
fn validate_token(value: &str) -> Result<(), IdError> {
    if value.is_empty() {
        return Err(IdError::Empty);
    }
    if value.len() > MAX_ID_LEN {
        return Err(IdError::TooLong { max: MAX_ID_LEN });
    }
    // Identifiers become directory and branch-name components, so path separators, drive colons and `..` are rejected.
    if let Some(found) = value
        .chars()
        .find(|c| !c.is_ascii_graphic() || matches!(c, '/' | '\\' | ':'))
    {
        return Err(IdError::InvalidCharacter { found });
    }
    if value.contains("..") {
        return Err(IdError::Malformed("an identifier cannot contain `..`"));
    }
    Ok(())
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
    /// Only emptiness and NUL bytes are rejected here; whether the path exists, is inside the managed root, or is short
    /// enough is decided by the Git adapter and preflight, which can touch the filesystem.
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
    if let Some(found) = value
        .chars()
        .find(|c| c.is_control() || c.is_whitespace() || "~^:?*[\\".contains(*c))
    {
        return Err(IdError::InvalidCharacter { found });
    }
    if value == "@" || value.contains("@{") {
        return Err(IdError::Malformed(
            "a branch name cannot be `@` or contain `@{`",
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
    if value
        .split('/')
        .any(|part| part.starts_with('.') || part.ends_with(".lock"))
    {
        return Err(IdError::Malformed(
            "a branch name component cannot start with `.` or end with `.lock`",
        ));
    }
    Ok(value.to_owned())
}

fn validate_worktree_path(value: &str) -> Result<String, IdError> {
    if value.trim().is_empty() {
        return Err(IdError::Empty);
    }
    if value.contains('\0') {
        return Err(IdError::InvalidCharacter { found: '\0' });
    }
    Ok(value.to_owned())
}

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
    fn token_ids_accept_visible_ascii_and_reject_the_rest() {
        assert_eq!(RunId::new("run-01H").unwrap().as_str(), "run-01H");
        assert_eq!(TicketId::new("F02").unwrap().to_string(), "F02");
        assert_eq!(WorkerId::new(""), Err(IdError::Empty));
        assert_eq!(
            WorkerId::new("a b"),
            Err(IdError::InvalidCharacter { found: ' ' })
        );
        for unsafe_id in ["a/b", "a\\b", "C:", "..", "a..b"] {
            assert!(RunId::new(unsafe_id).is_err(), "{unsafe_id:?}");
        }
        assert_eq!(
            TicketId::new("x".repeat(129)),
            Err(IdError::TooLong { max: 128 })
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
        ] {
            assert!(BranchName::new(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn worktree_path_rejects_empty_and_nul() {
        assert!(WorktreePath::new(r"C:\Users\x\Vela\worktrees\f02").is_ok());
        assert_eq!(WorktreePath::new("  "), Err(IdError::Empty));
        assert!(WorktreePath::new("a\0b").is_err());
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
