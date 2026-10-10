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
        if let Some(bad) = value.chars().find(|c| {
            !(c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '~' | '-'))
        }) {
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
    /// lowercased. When anything had to change (case, encoding, a bad start or end, a reserved name, truncation) a `~`
    /// plus a stable hash of the *original* text is appended, so distinct originals map to distinct components,
    /// including originals that differ only in case.
    ///
    /// **Identity guarantee:** text that consists only of `[a-z0-9._-]`, starts with a letter or digit, does not end
    /// with a dot, contains no `..`, is not a reserved device name and is at most [`MAX_COMPONENT_LEN`] bytes maps to
    /// itself, so `PathComponent::encode(s) == PathComponent::new(s)` for every such `s`, at every length up to the
    /// limit. (`new` additionally accepts `~`, which only encoded names contain.) Because identifiers cannot contain
    /// `~`, an encoded name never equals a literal identifier.
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
        if body.len() > MAX_COMPONENT_LEN {
            changed = true;
        }
        if changed || is_windows_reserved_name(&body) {
            // A suffix is needed, so the body must leave room for it. The body is ASCII by construction, so truncating
            // at a byte index is on a character boundary. (Text that needs no change keeps its identity up to the full
            // MAX_COMPONENT_LEN bytes; only changed, reserved or over-long text is shortened, to at most
            // MAX_COMPONENT_LEN - SUFFIX_LEN bytes, so body plus suffix is at most MAX_COMPONENT_LEN.)
            body.truncate(body.len().min(MAX_COMPONENT_LEN - SUFFIX_LEN));
            let hash = stable_hash(original);
            if is_windows_reserved_name(&body) {
                // `con.txt` is still the console device; the stem itself must change, so the hash goes before the first
                // dot (a trailing dot left by truncation is dropped first, since Windows strips it).
                while body.ends_with('.') {
                    body.pop();
                }
                let stem_end = body.find('.').unwrap_or(body.len());
                body.insert_str(stem_end, &format!("~{hash}"));
            } else {
                body.push('~');
                body.push_str(&hash);
            }
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
            "con",
            "CON",
            "Prn",
            "aux",
            "NUL",
            "com1",
            "COM9",
            "lpt0",
            "LPT7",
            "con.txt",
            "nul.tar.gz",
            "CONIN$",
        ] {
            assert!(is_windows_reserved_name(name), "{name}");
        }
        for name in [
            "console", "com", "com10", "lpt", "comma", "x.con", "a-con", "F02",
        ] {
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
            "F02",
            "CON",
            "con.txt",
            "Run-1",
            "a#b",
            "-rf",
            ".",
            "..",
            "a.",
            "a..b",
            "a b",
            "C:",
            "a/b",
            "a\\b",
            "nul",
            "COM1",
            "é",
            "名前",
            "a?b",
            "~",
            &"A".repeat(200),
            &"a".repeat(200),
            "x\u{0}y",
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
            "run-1", "Run-1", "RUN-1", "rUn-1", "f02", "F02", "con", "CON", "Con", "a#b", "a%23b",
            "a.", "a~2e", "x", "X", "-x", "x-",
        ];
        let mut seen = std::collections::BTreeMap::new();
        for original in originals {
            let key = PathComponent::encode(original)
                .as_str()
                .to_ascii_lowercase();
            if let Some(previous) = seen.insert(key.clone(), original) {
                panic!("{previous:?} and {original:?} both map to {key}");
            }
        }
    }

    #[test]
    fn safe_text_up_to_the_limit_is_its_own_component_at_every_length() {
        for len in [1, 2, 52, 53, 54, 55, 60, 63, 64] {
            let text = "x".repeat(len);
            let encoded = PathComponent::encode(&text);
            assert_eq!(encoded.as_str(), text, "len {len}");
            assert_eq!(PathComponent::new(text.as_str()), Ok(encoded), "len {len}");
        }
        // A safe text with dots and dashes, 64 bytes long, is also unchanged.
        let mixed = format!("a{}z", "-.".repeat(31));
        assert_eq!(mixed.len(), 64);
        assert_eq!(PathComponent::encode(&mixed).as_str(), mixed);
    }

    #[test]
    fn text_over_the_limit_or_needing_a_change_near_it_stays_within_the_limit_and_distinct() {
        let sixty_five = "x".repeat(65);
        let encoded = PathComponent::encode(&sixty_five);
        assert_eq!(encoded.as_str().len(), MAX_COMPONENT_LEN);
        assert_ne!(encoded.as_str(), sixty_five);
        // Uppercase text of 54..=64 bytes needs a suffix, which must still fit.
        for len in [53, 54, 60, 64] {
            let upper = "X".repeat(len);
            let encoded = PathComponent::encode(&upper);
            assert!(encoded.as_str().len() <= MAX_COMPONENT_LEN, "len {len}");
            assert!(PathComponent::new(encoded.as_str()).is_ok(), "len {len}");
            assert_ne!(encoded, PathComponent::encode(&"x".repeat(len)));
        }
        // Distinct over-long originals stay distinct.
        assert_ne!(
            PathComponent::encode(&format!("{}a", "x".repeat(70))),
            PathComponent::encode(&format!("{}b", "x".repeat(70)))
        );
    }

    #[test]
    fn reserved_names_near_the_limit_still_yield_valid_components_within_the_limit() {
        for stem in ["con", "CON", "nul", "aux", "prn", "com1", "LPT9", "com0"] {
            for tail_len in [0usize, 1, 40, 49, 50, 53, 54, 58, 59, 60, 61, 62, 100] {
                for tail_char in ['a', '.', '-'] {
                    let tail: String = std::iter::repeat_n(tail_char, tail_len).collect();
                    for original in [format!("{stem}.{tail}"), format!("{stem}{tail}")] {
                        let encoded = PathComponent::encode(&original);
                        assert!(
                            encoded.as_str().len() <= MAX_COMPONENT_LEN,
                            "{original:?} encoded to {} bytes",
                            encoded.as_str().len()
                        );
                        assert!(
                            PathComponent::new(encoded.as_str()).is_ok(),
                            "{original:?} encoded to the invalid {encoded}"
                        );
                    }
                }
            }
        }
        // The documented example from review five: a reserved stem plus a 60-byte tail.
        let encoded = PathComponent::encode(&format!("con.{}", "a".repeat(60)));
        assert!(encoded.as_str().len() <= MAX_COMPONENT_LEN);
        assert!(PathComponent::new(encoded.as_str()).is_ok());
    }

    /// Deterministic property-style test: a fixed-seed generator produces thousands of hostile and benign strings and
    /// checks the guarantees of `encode` on all of them.
    #[test]
    fn encode_properties_hold_for_generated_inputs() {
        // SplitMix64: small, fixed, reproducible.
        struct Rng(u64);
        impl Rng {
            fn step(&mut self) -> u64 {
                self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
                let mut z = self.0;
                z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
                z ^ (z >> 31)
            }
        }
        const FRAGMENTS: [&str; 24] = [
            "a", "b", "Z", "7", "-", "_", ".", "..", "~", "#", " ", "/", "\\", ":", "con", "NUL", "com1",
            "Lpt3", "é", "名", "\u{0}", "?", "*", "run-1",
        ];
        let mut rng = Rng(0x5eed);
        let mut seen: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
        for _ in 0..6000 {
            let pieces = (rng.step() % 40) as usize;
            let mut original = String::new();
            for _ in 0..pieces {
                original.push_str(FRAGMENTS[(rng.step() % FRAGMENTS.len() as u64) as usize]);
            }
            let encoded = PathComponent::encode(&original);
            // 1. Always valid and within the limit.
            assert!(
                encoded.as_str().len() <= MAX_COMPONENT_LEN,
                "{original:?} -> {} bytes",
                encoded.as_str().len()
            );
            assert!(
                PathComponent::new(encoded.as_str()).is_ok(),
                "{original:?} -> invalid {encoded}"
            );
            // 2. Deterministic.
            assert_eq!(encoded, PathComponent::encode(&original));
            // 3. Safe, in-limit input is its own component (identity), and nothing else is.
            if let Ok(already) = PathComponent::new(original.as_str()) {
                if !original.contains('~') {
                    assert_eq!(encoded, already, "{original:?}");
                }
            }
            // 4. Distinct originals never share a component, not even case-insensitively.
            let key = encoded.as_str().to_ascii_lowercase();
            if let Some(previous) = seen.insert(key.clone(), original.clone()) {
                assert_eq!(previous, original, "{previous:?} and {original:?} both map to {key}");
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
