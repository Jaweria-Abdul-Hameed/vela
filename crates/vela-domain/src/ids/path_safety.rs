//! Filesystem-safety rules for Windows, shared by the identifier validators and by [`PathComponent`].
//!
//! Vela is Windows-first (NTFS, case-insensitive), so an identifier that ends up as a directory name (a worktree, a
//! per-worker profile directory) must survive these rules:
//!
//! - no path separators, drive colons, traversal (`.`/`..`) or Windows-illegal punctuation (`< > : " / \ | ? *`);
//! - no reserved device names (`CON`, `PRN`, `AUX`, `NUL`, `COM0`..`COM9`, `LPT0`..`LPT9`), with or without an
//!   extension, in any case;
//! - no trailing dot or space (Windows strips them, which makes two names alias one directory);
//! - no leading `-` (it would be read as an option if an adapter passes the name to a command line);
//! - names that differ only in letter case denote the same directory, so a *path component* is always lowercase and any
//!   id that is not already in canonical form is disambiguated by a stable hash of the original.
//!
//! Logical identifiers (`RunId`, `TicketId`, `WorkerId`) are validated against the strict subset of these rules so that
//! they are always encodable; [`PathComponent::encode`] then turns any of them into a component that is safe to use
//! directly.

use std::fmt;

/// Longest path component Vela produces (keeps worktree paths well under the Windows path limit).
pub const MAX_COMPONENT_LEN: usize = 64;

/// Length of the stable hash suffix (`~` plus ten hexadecimal digits).
const SUFFIX_LEN: usize = 11;

/// True for Windows reserved device names: the part of `name` before the first `.`, compared case-insensitively.
pub fn is_windows_reserved_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or("").trim_end_matches(' ');
    let upper = stem.to_ascii_uppercase();
    match upper.as_str() {
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$" => true,
        _ => {
            (upper.starts_with("COM") || upper.starts_with("LPT"))
                && upper.len() == 4
                && upper.as_bytes()[3].is_ascii_digit()
        }
    }
}

/// Characters Windows forbids in a file name (separators included).
pub fn is_windows_illegal_name_char(c: char) -> bool {
    c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
}

/// A stable 40-bit hash (FNV-1a, 64-bit, truncated) rendered as ten lowercase hexadecimal digits. It is stable across
/// platforms and releases, unlike `std::hash`, so a derived name never changes between runs.
pub fn stable_hash(text: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:010x}", hash & 0xff_ffff_ffff)
}

/// Why a candidate path component was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentError {
    /// Empty.
    Empty,
    /// Longer than [`MAX_COMPONENT_LEN`].
    TooLong,
    /// Contains a character outside `[a-z0-9._~-]`.
    InvalidCharacter(char),
    /// Does not start with a letter or digit.
    BadStart,
    /// Ends with a dot.
    TrailingDot,
    /// Contains `..`.
    Traversal,
    /// A Windows reserved device name.
    Reserved,
}

impl fmt::Display for ComponentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("path component is empty"),
            Self::TooLong => write!(f, "path component is longer than {MAX_COMPONENT_LEN} bytes"),
            Self::InvalidCharacter(c) => write!(f, "path component contains {c:?}"),
            Self::BadStart => f.write_str("path component must start with a letter or digit"),
            Self::TrailingDot => f.write_str("path component cannot end with a dot"),
            Self::Traversal => f.write_str("path component cannot contain `..`"),
            Self::Reserved => f.write_str("path component is a Windows reserved device name"),
        }
    }
}

impl std::error::Error for ComponentError {}

/// A single directory or file name that is safe to join onto a Vela-managed root on Windows (and elsewhere): lowercase
/// `[a-z0-9._~-]`, starts with a letter or digit, no trailing dot, no `..`, not a reserved device name, at most
/// [`MAX_COMPONENT_LEN`] bytes. Because it is lowercase, two components never collide case-insensitively unless they are
/// equal.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PathComponent(String);

impl PathComponent {
    /// Validates an already-safe component.
    pub fn new(value: impl Into<String>) -> Result<Self, ComponentError> {
        let value = value.into();
        let Some(first) = value.chars().next() else {
            return Err(ComponentError::Empty);
        };
        if value.len() > MAX_COMPONENT_LEN {
            return Err(ComponentError::TooLong);
        }
        if let Some(bad) = value
            .chars()
            .find(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '~' | '-')))
        {
            return Err(ComponentError::InvalidCharacter(bad));
        }
        if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
            return Err(ComponentError::BadStart);
        }
        if value.ends_with('.') {
            return Err(ComponentError::TrailingDot);
        }
        if value.contains("..") {
            return Err(ComponentError::Traversal);
        }
        if is_windows_reserved_name(&value) {
            return Err(ComponentError::Reserved);
        }
        Ok(Self(value))
    }

    /// Deterministically turns any text into a safe component.
    ///
    /// Characters outside `[a-z0-9._-]` are replaced by `~` and two hex digits per UTF-8 byte; uppercase letters are
    /// lowercased. When anything had to change (case, encoding, a bad start or end, a reserved name, truncation to
    /// [`MAX_COMPONENT_LEN`]) a `~` plus a stable hash of the *original* text is appended, so distinct originals map to
    /// distinct components, including originals that differ only in case. Text that is already a safe component maps to
    /// itself. Because identifiers cannot contain `~`, an encoded name never equals a literal identifier.
    pub fn encode(original: &str) -> Self {
        let mut body = String::with_capacity(original.len());
        let mut changed = false;
        for c in original.chars() {
            if c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-') {
                body.push(c);
            } else {
                changed = true;
                if c.is_ascii_uppercase() {
                    body.push(c.to_ascii_lowercase());
                } else {
                    let mut buf = [0u8; 4];
                    for byte in c.encode_utf8(&mut buf).bytes() {
                        body.push_str(&format!("~{byte:02x}"));
                    }
                }
            }
        }
        if !body.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit()) {
            body.insert(0, 'x');
            changed = true;
        }
        while body.contains("..") {
            body = body.replace("..", ".~2e");
            changed = true;
        }
        if body.ends_with('.') {
            body.pop();
            body.push_str("~2e");
            changed = true;
        }
        if body.len() > MAX_COMPONENT_LEN - SUFFIX_LEN {
            // The body is ASCII by construction, so truncating at a byte index is on a character boundary. The hash
            // keeps truncated names distinct.
            body.truncate(MAX_COMPONENT_LEN - SUFFIX_LEN);
            changed = true;
        }
        if is_windows_reserved_name(&body) {
            // `con.txt` is still the console device; the stem itself must change, so the hash goes before the first dot.
            let stem_end = body.find('.').unwrap_or(body.len());
            body.insert_str(stem_end, &format!("~{}", stable_hash(original)));
        } else if changed {
            body.push('~');
            body.push_str(&stable_hash(original));
        }
        Self(body)
    }

    /// The component as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PathComponent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_device_names_are_detected_in_any_case_and_with_extensions() {
        for name in [
            "con", "CON", "Prn", "aux", "NUL", "com1", "COM9", "lpt0", "LPT7", "con.txt", "nul.tar.gz", "CONIN$",
        ] {
            assert!(is_windows_reserved_name(name), "{name}");
        }
        for name in ["console", "com", "com10", "lpt", "comma", "x.con", "a-con", "F02"] {
            assert!(!is_windows_reserved_name(name), "{name}");
        }
    }

    #[test]
    fn stable_hash_is_ten_hex_digits_and_does_not_change() {
        assert_eq!(stable_hash(""), "e484222325");
        assert_eq!(stable_hash("run-1"), stable_hash("run-1"));
        assert_ne!(stable_hash("Run-1"), stable_hash("run-1"));
        assert_eq!(stable_hash("abc").len(), 10);
    }

    #[test]
    fn components_accept_only_the_safe_alphabet() {
        assert!(PathComponent::new("f02").is_ok());
        assert!(PathComponent::new("run-1_a.b~0123456789").is_ok());
        let cases = [
            ("", ComponentError::Empty),
            ("-rf", ComponentError::BadStart),
            (".hidden", ComponentError::BadStart),
            ("a.", ComponentError::TrailingDot),
            ("a..b", ComponentError::Traversal),
            ("con", ComponentError::Reserved),
            ("aux.txt", ComponentError::Reserved),
            ("Run", ComponentError::InvalidCharacter('R')),
            ("a b", ComponentError::InvalidCharacter(' ')),
            ("a/b", ComponentError::InvalidCharacter('/')),
            ("a\\b", ComponentError::InvalidCharacter('\\')),
            ("a:b", ComponentError::InvalidCharacter(':')),
            ("a?b", ComponentError::InvalidCharacter('?')),
            ("a*b", ComponentError::InvalidCharacter('*')),
            ("a<b", ComponentError::InvalidCharacter('<')),
            ("a|b", ComponentError::InvalidCharacter('|')),
            ("a\"b", ComponentError::InvalidCharacter('"')),
            (&"x".repeat(65), ComponentError::TooLong),
        ];
        for (input, expected) in cases {
            assert_eq!(PathComponent::new(input), Err(expected), "{input:?}");
        }
        assert!(PathComponent::new("x".repeat(64)).is_ok());
    }

    #[test]
    fn encode_is_identity_for_safe_text_and_always_yields_a_valid_component() {
        for safe in ["f02", "run-1", "a.b_c", "7"] {
            assert_eq!(PathComponent::encode(safe).as_str(), safe);
        }
        let hostile = [
            "F02", "CON", "con.txt", "Run-1", "a#b", "-rf", ".", "..", "a.", "a..b", "a b", "C:", "a/b", "a\\b",
            "nul", "COM1", "é", "名前", "a?b", "~", &"A".repeat(200), &"a".repeat(200), "x\u{0}y",
        ];
        for original in hostile {
            let component = PathComponent::encode(original);
            assert!(
                PathComponent::new(component.as_str()).is_ok(),
                "{original:?} encoded to the invalid {component}"
            );
        }
    }

    #[test]
    fn encode_never_collides_case_insensitively_for_distinct_originals() {
        let originals = [
            "run-1", "Run-1", "RUN-1", "rUn-1", "f02", "F02", "con", "CON", "Con", "a#b", "a%23b", "a.", "a~2e", "x",
            "X", "-x", "x-",
        ];
        let mut seen = std::collections::BTreeMap::new();
        for original in originals {
            let key = PathComponent::encode(original).as_str().to_ascii_lowercase();
            if let Some(previous) = seen.insert(key.clone(), original) {
                panic!("{previous:?} and {original:?} both map to {key}");
            }
        }
    }

    #[test]
    fn long_names_are_truncated_with_a_hash_and_stay_distinct() {
        let a = PathComponent::encode(&format!("{}1", "a".repeat(100)));
        let b = PathComponent::encode(&format!("{}2", "a".repeat(100)));
        assert_eq!(a.as_str().len(), MAX_COMPONENT_LEN);
        assert_ne!(a, b);
    }
}
