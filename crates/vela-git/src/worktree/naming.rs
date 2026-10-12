//! Deterministic, bounded, collision-safe names for worktree directories and branches (G02).
//!
//! A name is a lowercase slug (at most [`MAX_SLUG`] characters) plus a short hash of the full, case-folded key, so a long
//! title is truncated the same way every time and two different keys never share a name. Because NTFS is case-insensitive,
//! two keys that differ only in case would hash alike; [`unique_name`] detects that against the names already in use and
//! derives a second, case-sensitive suffix.

use std::collections::BTreeSet;

use vela_domain::ids::case_fold_key;

/// The longest slug part of a name.
pub const MAX_SLUG: usize = 32;

/// The longest complete directory name (slug, dash, 8 hex digits, optional collision suffix).
pub const MAX_DIR_NAME: usize = MAX_SLUG + 1 + 8 + 9;

/// Lowercase ASCII letters and digits separated by single dashes, at most [`MAX_SLUG`] characters, never empty.
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(c.to_ascii_lowercase());
        } else {
            pending_dash = true;
        }
        if out.len() >= MAX_SLUG {
            break;
        }
    }
    out.truncate(MAX_SLUG);
    let trimmed = out.trim_end_matches('-').to_owned();
    if trimmed.is_empty() {
        "x".to_owned()
    } else {
        trimmed
    }
}

/// FNV-1a over the bytes, as eight lowercase hex digits.
pub fn hash8(text: &str) -> String {
    let mut h: u32 = 0x811c_9dc5;
    for b in text.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    format!("{h:08x}")
}

/// The base name for a key: `<slug>-<hash of the case-folded key>`.
pub fn base_name(key: &str) -> String {
    format!("{}-{}", slug(key), hash8(&case_fold_key(key)))
}

/// A name for `key` that is not in `existing_folded` (case-folded names already in use). The base name is used when it is
/// free; otherwise a suffix from the case-sensitive key is added; if even that is taken a counter is appended.
pub fn unique_name(key: &str, existing_folded: &BTreeSet<String>) -> String {
    let base = base_name(key);
    if !existing_folded.contains(&case_fold_key(&base)) {
        return base;
    }
    let with_case = format!("{base}-{}", &hash8(key)[..4]);
    if !existing_folded.contains(&case_fold_key(&with_case)) {
        return with_case;
    }
    (2u32..)
        .map(|n| format!("{with_case}-{n}"))
        .find(|candidate| !existing_folded.contains(&case_fold_key(candidate)))
        .expect("an unused counter exists")
}

/// The branch a worker's ticket is built on: `vela/<run slug>/<ticket name>`.
pub fn worker_branch(run: &str, ticket_name: &str) -> String {
    format!("vela/{}/{ticket_name}", slug(run))
}

/// The integration branch of a run: `vela/<run slug>/integration`.
pub fn integration_branch(run: &str) -> String {
    format!("vela/{}/integration", slug(run))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_lowercase_dashed_bounded_and_never_empty() {
        assert_eq!(slug("Add Login Page!"), "add-login-page");
        assert_eq!(slug("  --weird__name--  "), "weird-name");
        assert_eq!(slug("日本語"), "x");
        assert_eq!(slug(""), "x");
        let long = slug(&"very-long-title ".repeat(20));
        assert!(long.len() <= MAX_SLUG, "{long}");
        assert!(!long.ends_with('-'));
    }

    #[test]
    fn names_are_deterministic_bounded_and_distinct_for_long_titles_sharing_a_prefix() {
        let a = base_name(&format!(
            "{}A",
            "a very long ticket title that goes on ".repeat(5)
        ));
        let b = base_name(&format!(
            "{}B",
            "a very long ticket title that goes on ".repeat(5)
        ));
        assert_ne!(a, b, "the hash keeps truncated names apart");
        assert!(a.len() <= MAX_DIR_NAME && b.len() <= MAX_DIR_NAME);
        assert_eq!(
            a,
            base_name(&format!(
                "{}A",
                "a very long ticket title that goes on ".repeat(5)
            ))
        );
    }

    #[test]
    fn case_colliding_keys_get_distinct_names() {
        let mut used = BTreeSet::new();
        let first = unique_name("Feature-Login", &used);
        used.insert(case_fold_key(&first));
        let second = unique_name("feature-login", &used);
        assert_ne!(case_fold_key(&first), case_fold_key(&second));
        used.insert(case_fold_key(&second));
        let third = unique_name("FEATURE-LOGIN", &used);
        assert!(!used.contains(&case_fold_key(&third)));
        assert!(third.len() <= MAX_DIR_NAME + 4);
    }

    #[test]
    fn branch_names_are_namespaced_per_run() {
        assert_eq!(
            worker_branch("Run 7", "login-1a2b3c4d"),
            "vela/run-7/login-1a2b3c4d"
        );
        assert_eq!(integration_branch("Run 7"), "vela/run-7/integration");
    }
}
