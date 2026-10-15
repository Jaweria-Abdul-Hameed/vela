//! Reading capacity and access failures out of Antigravity's text (A13). Exit codes and `result.status` are never trusted: a quota
//! error can exit 0, so the text is what is read. Three things come out: a capacity limit with its reset time when one is given,
//! a missing sign-in, and a provider policy block (see [`policy_block`]). Anything else that looks like an access failure but
//! matches none of them is **unclassified**: it is surfaced with its evidence for a person to look at, never retried, and never
//! treated as a policy block.

pub mod policy_block;

/// Redacts what must not be kept in an evidence excerpt: addresses, URL query strings and long opaque tokens.
pub fn redact(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            if word.contains('@') && word.contains('.') {
                "[address]".to_owned()
            } else if let Some((base, _)) = word.split_once('?').filter(|_| word.contains("://")) {
                format!("{base}?[query]")
            } else if word.len() >= 24
                && word
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '='))
            {
                "[token]".to_owned()
            } else {
                word.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The first non-empty line, redacted and bounded.
pub fn excerpt(text: &str) -> String {
    let line = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    redact(line).chars().take(300).collect()
}

/// How long until the provider's quota resets, when the text says so: `reset after 3h45m12s`, `resets in 45 minutes`,
/// `try again in 90 seconds`. Returns milliseconds. A reset time that is not there is `None`, never a guess.
pub fn parse_reset_after_ms(text: &str) -> Option<i64> {
    let lower = text.to_ascii_lowercase();
    for marker in [
        "reset after ",
        "resets after ",
        "reset in ",
        "resets in ",
        "try again in ",
        "retry after ",
        "retry in ",
    ] {
        if let Some(at) = lower.find(marker) {
            let tail = &lower[at + marker.len()..];
            if let Some(ms) = parse_duration(tail) {
                return Some(ms);
            }
        }
    }
    None
}

/// `3h45m12s`, `1h`, `45 minutes`, `90 seconds`, `2 hours 5 minutes`.
fn parse_duration(tail: &str) -> Option<i64> {
    let mut total: i64 = 0;
    let mut found = false;
    let mut chars = tail.chars().peekable();
    loop {
        while chars.peek().is_some_and(|c| *c == ' ' || *c == ',') {
            chars.next();
        }
        let mut digits = String::new();
        while chars.peek().is_some_and(|c| c.is_ascii_digit()) {
            digits.push(chars.next()?);
        }
        if digits.is_empty() {
            break;
        }
        while chars.peek() == Some(&' ') {
            chars.next();
        }
        let mut unit = String::new();
        while chars.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            unit.push(chars.next()?);
        }
        let n: i64 = digits.parse().ok()?;
        let per = match unit.as_str() {
            "h" | "hr" | "hrs" | "hour" | "hours" => 3_600_000,
            "m" | "min" | "mins" | "minute" | "minutes" => 60_000,
            "s" | "sec" | "secs" | "second" | "seconds" => 1_000,
            _ => break,
        };
        total = total.checked_add(n.checked_mul(per)?)?;
        found = true;
    }
    found.then_some(total)
}

/// Phrases that mean the provider ran out of capacity.
const CAPACITY: &[&str] = &[
    "resource_exhausted",
    "exhausted your capacity",
    "quota will reset",
    "rate limit",
    "too many requests",
    "quota exceeded",
    "(code 429)",
];

/// Phrases that mean the user is not signed in.
const SIGN_IN: &[&str] = &[
    "not signed in",
    "not authenticated",
    "unauthenticated",
    "sign in with your google",
    "authentication required",
    "please log in",
    "login required",
];

/// Wording that suggests an access problem without saying which: it is only ever surfaced, never acted on.
const ACCESS_HINTS: &[&str] = &[
    "permission denied",
    "access denied",
    "forbidden",
    "unauthorized",
    "(code 401)",
    "(code 403)",
    "status 401",
    "status 403",
    "account",
];

/// What an access-related text came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessClass {
    /// Capacity is exhausted; `reset_after_ms` is the provider's reset time when it gave one.
    Capacity {
        /// Redacted excerpt.
        evidence: String,
        /// Milliseconds until reset.
        reset_after_ms: Option<i64>,
    },
    /// The user is not signed in.
    SignInRequired {
        /// Redacted excerpt.
        evidence: String,
    },
    /// A documented or recorded policy-block signal matched.
    PolicyBlock(policy_block::Detected),
    /// It reads like an access failure but matches nothing known: surfaced with evidence, never retried, never a policy block.
    Unclassified {
        /// Redacted excerpt.
        evidence: String,
    },
}

/// Classifies a provider text. The order is deliberate: a policy block outranks everything, because it must never be softened
/// into a capacity wait or a sign-in prompt; a clear capacity or sign-in message outranks the vague access hints.
pub fn classify_access(text: &str) -> Option<AccessClass> {
    if let Some(d) = policy_block::detect(text) {
        return Some(AccessClass::PolicyBlock(d));
    }
    let lower = text.to_ascii_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| lower.contains(n));
    if has(SIGN_IN) {
        return Some(AccessClass::SignInRequired {
            evidence: excerpt(text),
        });
    }
    if has(CAPACITY) {
        return Some(AccessClass::Capacity {
            evidence: excerpt(text),
            reset_after_ms: parse_reset_after_ms(text),
        });
    }
    if has(ACCESS_HINTS) {
        return Some(AccessClass::Unclassified {
            evidence: excerpt(text),
        });
    }
    None
}

#[cfg(test)]
mod tests;
