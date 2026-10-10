//! [`PatternRedactor`]: the production [`Redactor`].
//!
//! Plain text is scanned once, left to right, by hand-written matchers (no regular-expression engine, so there is no
//! pathological backtracking on hostile process output). Each matcher recognizes one class of secret:
//!
//! | Class | Example | What is replaced |
//! |---|---|---|
//! | named value | `DB_PASSWORD=hunter2`, `"api_key": "k"`, `--token abc`, `Authorization: Basic eA==`, `Cookie: sid=1` | the value |
//! | bearer token | `Bearer eyJ...` | the token |
//! | known token shape | GitHub, GitLab, Google, AWS, Slack, Stripe, npm and `sk-` keys, JSON Web Tokens | the token |
//! | URL credentials | `https://user:pass@host/` | the user information |
//! | private key | `-----BEGIN ... PRIVATE KEY-----` to its `END` line | the whole block |
//! | registered secret | a value given to [`PatternRedactor::with_secret`] | every occurrence |

use std::borrow::Cow;
use std::fmt;

use super::names::{MAX_NAME_LEN, is_credential_name};
use super::{REDACTED, Redactor, json};

/// Registered secrets shorter than this are ignored: replacing every occurrence of a two-letter value would destroy
/// the output without protecting anything.
pub const MIN_SECRET_LEN: usize = 6;

const PEM_BEGIN: &[u8] = b"-----BEGIN ";
const PEM_END: &[u8] = b"-----END ";
const PEM_DASHES: &[u8] = b"-----";
const PRIVATE_KEY: &[u8] = b"PRIVATE KEY";

/// The shortest run after `Bearer ` that is treated as a token rather than a word.
const MIN_BEARER_LEN: usize = 16;

/// Bare values that are never secrets.
const EMPTY_VALUES: [&[u8]; 6] = [b"true", b"false", b"null", b"none", b"nil", b"undefined"];

/// Authentication schemes: in `token=Bearer abc` the credential is the word after the scheme.
const SCHEMES: [&[u8]; 5] = [b"bearer", b"basic", b"digest", b"token", b"negotiate"];

/// Redacts tokens, authorization headers, credential-like environment values, cookies and known secret patterns.
///
/// Stateless apart from the registered secrets, so one instance is shared by every ingestion point.
#[derive(Clone, Default)]
pub struct PatternRedactor {
    /// Known secret values, longest first so a secret is replaced before any secret it contains.
    secrets: Vec<String>,
}

/// `Debug` never shows the registered secrets.
impl fmt::Debug for PatternRedactor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PatternRedactor")
            .field("secrets", &self.secrets.len())
            .finish()
    }
}

impl PatternRedactor {
    /// A redactor with the built-in patterns and no registered secrets.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a known secret value; every occurrence of it is redacted wherever it appears. Values shorter than
    /// [`MIN_SECRET_LEN`] are ignored.
    #[must_use]
    pub fn with_secret(mut self, value: &str) -> Self {
        self.add_secret(value);
        self
    }

    /// Registers the value of every credential-like variable of `environment` (see [`is_credential_name`]), so a
    /// process that echoes its own environment cannot leak it.
    #[must_use]
    pub fn with_environment<'e>(
        mut self,
        environment: impl IntoIterator<Item = (&'e str, &'e str)>,
    ) -> Self {
        for (name, value) in environment {
            if is_credential_name(name) {
                self.add_secret(value);
            }
        }
        self
    }

    /// Registers a known secret value. Returns whether it was accepted (long enough and not part of the marker).
    pub fn add_secret(&mut self, value: &str) -> bool {
        if value.len() < MIN_SECRET_LEN || REDACTED.contains(value) {
            return false;
        }
        if !self.secrets.iter().any(|known| known == value) {
            self.secrets.push(value.to_owned());
            self.secrets
                .sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        }
        true
    }

    fn redact_plain<'a>(&self, text: &'a str) -> Cow<'a, str> {
        let mut current = Cow::Borrowed(text);
        for secret in &self.secrets {
            if current.contains(secret.as_str()) {
                current = Cow::Owned(current.replace(secret.as_str(), REDACTED));
            }
        }
        let scanned = match scan(&current) {
            Cow::Borrowed(_) => None,
            Cow::Owned(scanned) => Some(scanned),
        };
        scanned.map_or(current, Cow::Owned)
    }
}

impl Redactor for PatternRedactor {
    fn redact<'a>(&self, text: &'a str) -> Cow<'a, str> {
        let first = text.bytes().find(|byte| !byte.is_ascii_whitespace());
        if matches!(first, Some(b'{' | b'['))
            && let Some(document) = json::redact_document(text, self)
        {
            return document;
        }
        self.redact_plain(text)
    }

    fn redact_value<'a>(&self, name: &str, value: &'a str) -> Cow<'a, str> {
        if value.is_empty() {
            Cow::Borrowed(value)
        } else if is_credential_name(name) {
            Cow::Borrowed(REDACTED)
        } else {
            self.redact(value)
        }
    }
}

/// The byte range of one secret.
struct Hit {
    start: usize,
    end: usize,
}

impl Hit {
    fn non_empty(start: usize, end: usize) -> Option<Self> {
        (end > start).then_some(Self { start, end })
    }
}

fn scan(text: &str) -> Cow<'_, str> {
    let bytes = text.as_bytes();
    let mut out = String::new();
    let mut copied = 0;
    let mut index = 0;
    while index < bytes.len() {
        match hit_at(bytes, index) {
            // Every hit starts and ends next to an ASCII byte or at the end of the text, so the slices below are on
            // character boundaries.
            Some(hit) => {
                out.push_str(&text[copied..hit.start]);
                out.push_str(REDACTED);
                copied = hit.end;
                index = hit.end;
            }
            None => index += 1,
        }
    }
    if out.is_empty() {
        return Cow::Borrowed(text);
    }
    out.push_str(&text[copied..]);
    Cow::Owned(out)
}

fn hit_at(bytes: &[u8], index: usize) -> Option<Hit> {
    let byte = bytes[index];
    if byte == b'-'
        && bytes[index..].starts_with(PEM_BEGIN)
        && let Some(hit) = private_key(bytes, index)
    {
        return Some(hit);
    }
    if byte == b':' {
        return url_credentials(bytes, index);
    }
    let word_start = index == 0 || !is_word_byte(bytes[index - 1]);
    if !word_start || !is_name_byte(byte) {
        return None;
    }
    token(bytes, index)
        .or_else(|| bearer(bytes, index))
        .or_else(|| named_value(bytes, index))
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Bytes of a name: `AWS_SECRET_ACCESS_KEY`, `x-api-key`, `db.password`, `--token`.
fn is_name_byte(byte: u8) -> bool {
    is_word_byte(byte) || byte == b'-' || byte == b'.'
}

fn blank_len(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .take_while(|byte| matches!(**byte, b' ' | b'\t'))
        .count()
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

// ---- Known token shapes ----

#[derive(Clone, Copy)]
enum Charset {
    /// `A-Z a-z 0-9`
    Alphanumeric,
    /// `A-Z 0-9`
    UpperDigits,
    /// `A-Z a-z 0-9 _`
    Word,
    /// `A-Z a-z 0-9 _ -`
    UrlSafe,
    /// `A-Z a-z 0-9 _ - .`
    Dotted,
}

impl Charset {
    fn contains(self, byte: u8) -> bool {
        match self {
            Self::Alphanumeric => byte.is_ascii_alphanumeric(),
            Self::UpperDigits => byte.is_ascii_uppercase() || byte.is_ascii_digit(),
            Self::Word => is_word_byte(byte),
            Self::UrlSafe => is_word_byte(byte) || byte == b'-',
            Self::Dotted => is_name_byte(byte),
        }
    }
}

/// A token recognizable by its prefix: `prefix` followed by `min..=max` bytes of `charset`.
struct Shape {
    prefix: &'static [u8],
    min: usize,
    max: usize,
    charset: Charset,
}

const fn shape(prefix: &'static [u8], min: usize, charset: Charset) -> Shape {
    Shape {
        prefix,
        min,
        max: usize::MAX,
        charset,
    }
}

const SHAPES: [Shape; 24] = [
    // GitHub personal, OAuth, user-to-server, server-to-server and refresh tokens; fine-grained tokens.
    shape(b"ghp_", 20, Charset::Word),
    shape(b"gho_", 20, Charset::Word),
    shape(b"ghu_", 20, Charset::Word),
    shape(b"ghs_", 20, Charset::Word),
    shape(b"ghr_", 20, Charset::Word),
    shape(b"github_pat_", 20, Charset::Word),
    // GitLab personal access tokens.
    shape(b"glpat-", 20, Charset::UrlSafe),
    // Google API keys, OAuth access tokens and refresh tokens.
    shape(b"AIza", 30, Charset::UrlSafe),
    shape(b"ya29.", 20, Charset::Dotted),
    shape(b"1//", 30, Charset::UrlSafe),
    // AWS access key identifiers (exactly 20 characters).
    Shape {
        prefix: b"AKIA",
        min: 16,
        max: 16,
        charset: Charset::UpperDigits,
    },
    Shape {
        prefix: b"ASIA",
        min: 16,
        max: 16,
        charset: Charset::UpperDigits,
    },
    // Slack tokens.
    shape(b"xoxb-", 10, Charset::UrlSafe),
    shape(b"xoxp-", 10, Charset::UrlSafe),
    shape(b"xoxa-", 10, Charset::UrlSafe),
    shape(b"xoxr-", 10, Charset::UrlSafe),
    shape(b"xoxs-", 10, Charset::UrlSafe),
    shape(b"xapp-", 10, Charset::UrlSafe),
    // `sk-` API keys (OpenAI, Anthropic and others) and Stripe keys.
    shape(b"sk-", 20, Charset::UrlSafe),
    shape(b"sk_live_", 16, Charset::Alphanumeric),
    shape(b"sk_test_", 16, Charset::Alphanumeric),
    shape(b"rk_live_", 16, Charset::Alphanumeric),
    shape(b"rk_test_", 16, Charset::Alphanumeric),
    // npm access tokens.
    shape(b"npm_", 30, Charset::Alphanumeric),
];

fn token(bytes: &[u8], index: usize) -> Option<Hit> {
    let rest = &bytes[index..];
    if let Some(len) = json_web_token(rest) {
        return Some(Hit {
            start: index,
            end: index + len,
        });
    }
    for shape in &SHAPES {
        let Some(body) = rest.strip_prefix(shape.prefix) else {
            continue;
        };
        let mut len = body
            .iter()
            .take_while(|byte| shape.charset.contains(**byte))
            .count();
        // A sentence-ending dot is not part of the token.
        while len > 0 && matches!(body[len - 1], b'.' | b'-') {
            len -= 1;
        }
        if (shape.min..=shape.max).contains(&len) {
            return Some(Hit {
                start: index,
                end: index + shape.prefix.len() + len,
            });
        }
    }
    None
}

/// Three base64url segments separated by dots, the first starting with `eyJ` (an encoded `{"`).
fn json_web_token(rest: &[u8]) -> Option<usize> {
    if !rest.starts_with(b"eyJ") {
        return None;
    }
    let segment = |from: usize| {
        rest[from..]
            .iter()
            .take_while(|byte| Charset::UrlSafe.contains(**byte))
            .count()
    };
    let header = segment(0);
    if header < 10 || rest.get(header) != Some(&b'.') {
        return None;
    }
    let payload = segment(header + 1);
    let signature_at = header + 1 + payload;
    if payload < 10 || rest.get(signature_at) != Some(&b'.') {
        return None;
    }
    Some(signature_at + 1 + segment(signature_at + 1))
}

// ---- Bearer tokens ----

fn is_credential_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'~' | b'+' | b'/' | b'=' | b'-')
}

fn bearer(bytes: &[u8], index: usize) -> Option<Hit> {
    let rest = &bytes[index..];
    if rest.len() < 7 || !rest[..6].eq_ignore_ascii_case(b"bearer") {
        return None;
    }
    let blanks = blank_len(&rest[6..]);
    if blanks == 0 {
        return None;
    }
    let start = index + 6 + blanks;
    let len = bytes[start..]
        .iter()
        .take_while(|byte| is_credential_byte(**byte))
        .count();
    (len >= MIN_BEARER_LEN).then_some(Hit {
        start,
        end: start + len,
    })
}

// ---- Named values ----

fn named_value(bytes: &[u8], index: usize) -> Option<Hit> {
    let name_len = bytes[index..]
        .iter()
        .take(MAX_NAME_LEN + 1)
        .take_while(|byte| is_name_byte(**byte))
        .count();
    if name_len > MAX_NAME_LEN {
        return None;
    }
    let name_end = index + name_len;
    let name = std::str::from_utf8(&bytes[index..name_end]).ok()?;
    if !is_credential_name(name) {
        return None;
    }

    // The closing quote of a quoted name: `"password": ...`, `'password': ...`, `\"password\": ...`.
    let mut at = name_end;
    if bytes[at..].starts_with(b"\\\"") {
        at += 2;
    } else if matches!(bytes.get(at), Some(b'"' | b'\'')) {
        at += 1;
    }
    let quoted_name = at > name_end;
    let gap = blank_len(&bytes[at..]);
    at += gap;

    let separator = match bytes.get(at) {
        // `==`, `=>` and `=~` are operators, not assignments.
        Some(b'=') if !matches!(bytes.get(at + 1), Some(b'=' | b'>' | b'~')) => b'=',
        // `::` is a path separator.
        Some(b':') if bytes.get(at + 1) != Some(&b':') => b':',
        // A command-line flag: `--password hunter2`.
        _ if bytes[index] == b'-' && !quoted_name && gap > 0 => b' ',
        _ => return None,
    };
    if separator != b' ' {
        at += 1;
        at += blank_len(&bytes[at..]);
    }

    let rest = &bytes[at..];
    if rest.is_empty() || rest.starts_with(REDACTED.as_bytes()) {
        return None;
    }
    let line_end = at
        + rest
            .iter()
            .take_while(|byte| !matches!(**byte, b'\n' | b'\r'))
            .count();

    // A quoted value ends at its closing quote, or at the end of the line when the quote is never closed.
    if rest.starts_with(b"\\\"") {
        let start = at + 2;
        let end = find(&bytes[start..line_end], b"\\\"").map_or(line_end, |offset| start + offset);
        return Hit::non_empty(start, end);
    }
    if let quote @ (b'"' | b'\'') = rest[0] {
        let start = at + 1;
        let mut end = start;
        while end < line_end && bytes[end] != quote {
            end += if bytes[end] == b'\\' { 2 } else { 1 };
        }
        return Hit::non_empty(start, end.min(line_end));
    }
    if separator == b' ' && rest[0] == b'-' {
        return None;
    }

    // An unquoted value: the rest of the line after `:` (headers, YAML), one word after `=` or a flag (shell).
    let mut end = if separator == b':' {
        line_end
    } else {
        let word = word_end(bytes, at, line_end);
        if SCHEMES
            .iter()
            .any(|scheme| bytes[at..word].eq_ignore_ascii_case(scheme))
        {
            let credential = word + blank_len(&bytes[word..line_end]);
            word_end(bytes, credential, line_end)
        } else {
            word
        }
    };
    while end > at && matches!(bytes[end - 1], b' ' | b'\t') {
        end -= 1;
    }
    // The comma that separates this member from the next stays readable.
    if separator == b':' && end > at + 1 && bytes[end - 1] == b',' {
        end -= 1;
    }
    let value = &bytes[at..end];
    let not_a_secret = EMPTY_VALUES.iter().any(|empty| value.eq_ignore_ascii_case(empty))
        // The value continues on the following lines (`"auth": {`); its members are examined on their own.
        || matches!(value, b"{" | b"[");
    if not_a_secret {
        return None;
    }
    Hit::non_empty(at, end)
}

fn word_end(bytes: &[u8], from: usize, limit: usize) -> usize {
    from + bytes[from..limit]
        .iter()
        .take_while(|byte| !byte.is_ascii_whitespace())
        .count()
}

// ---- URL credentials ----

/// `scheme://user:password@host`: `index` is the colon. The user information is everything up to the last `@` of the
/// authority.
fn url_credentials(bytes: &[u8], index: usize) -> Option<Hit> {
    if !bytes[index..].starts_with(b"://")
        || index == 0
        || !bytes[index - 1].is_ascii_alphanumeric()
    {
        return None;
    }
    let start = index + 3;
    let authority_len = bytes[start..]
        .iter()
        .take_while(|byte| !matches!(**byte, b'/' | b'?' | b'#') && !byte.is_ascii_whitespace())
        .count();
    let authority = &bytes[start..start + authority_len];
    let at = authority.iter().rposition(|byte| *byte == b'@')?;
    let user_info = &authority[..at];
    // `ssh://git@host/` carries no credential.
    if user_info.is_empty() || user_info == REDACTED.as_bytes() || user_info == b"git" {
        return None;
    }
    Some(Hit {
        start,
        end: start + at,
    })
}

// ---- Private keys ----

/// A PEM private key block starting at `index`. Without an `END` line the rest of the text is the key.
fn private_key(bytes: &[u8], index: usize) -> Option<Hit> {
    let label_start = index + PEM_BEGIN.len();
    let label_window = &bytes[label_start..bytes.len().min(label_start + MAX_NAME_LEN)];
    let label_len = find(label_window, PEM_DASHES)?;
    find(&label_window[..label_len], PRIVATE_KEY)?;
    let body_start = label_start + label_len + PEM_DASHES.len();
    let end = find(&bytes[body_start..], PEM_END)
        .and_then(|offset| {
            let end_label = body_start + offset + PEM_END.len();
            find(&bytes[end_label..], PEM_DASHES).map(|close| end_label + close + PEM_DASHES.len())
        })
        .unwrap_or(bytes.len());
    Some(Hit { start: index, end })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::redact::corpus::{Case, corpus};

    fn redact(text: &str) -> String {
        PatternRedactor::new().redact(text).into_owned()
    }

    #[test]
    fn the_corpus_is_redacted_as_plain_text() {
        let redactor = PatternRedactor::new();
        for Case {
            name,
            text,
            secrets,
            kept,
        } in corpus()
        {
            let shown = redactor.redact(&text);
            for secret in &secrets {
                assert!(
                    !shown.contains(secret.as_str()),
                    "{name}: `{secret}` survived in `{shown}`"
                );
            }
            for expected in &kept {
                assert!(
                    shown.contains(expected),
                    "{name}: `{expected}` was lost from `{shown}`"
                );
            }
            assert!(
                shown.contains(REDACTED),
                "{name}: nothing was redacted in `{shown}`"
            );
        }
    }

    #[test]
    fn the_corpus_is_redacted_in_journal_payloads() {
        // A journal payload is a serialized JSON document. The secret is carried as a member value, inside an array
        // and inside free text, the three places a typed payload can hold it.
        let redactor = PatternRedactor::new();
        for case in corpus() {
            let mut text = String::new();
            for character in case.text.chars() {
                match character {
                    '"' => text.push_str("\\\""),
                    '\\' => text.push_str("\\\\"),
                    '\n' => text.push_str("\\n"),
                    other => text.push(other),
                }
            }
            let payload = format!(
                "{{\"kind\":\"CommandOutput\",\"seq\":7,\"payload\":{{\"text\":\"{text}\",\"lines\":[\"{text}\"],\"ok\":true}}}}"
            );
            let shown = redactor.redact(&payload);
            for secret in &case.secrets {
                assert!(
                    !shown.contains(secret.as_str()),
                    "{}: `{secret}` survived in {shown}",
                    case.name
                );
            }
            assert!(
                shown.starts_with("{\"kind\":\"CommandOutput\",\"seq\":7,\"payload\":{\"text\":\""),
                "{shown}"
            );
            assert!(shown.ends_with("\"],\"ok\":true}}"), "{shown}");
            assert_eq!(redactor.redact(&shown), shown, "{}", case.name);
        }
    }

    #[test]
    fn redaction_is_idempotent() {
        let redactor = PatternRedactor::new();
        for case in corpus() {
            let once = redactor.redact(&case.text).into_owned();
            let twice = redactor.redact(&once).into_owned();
            assert_eq!(once, twice, "{}", case.name);
        }
    }

    #[test]
    fn ordinary_text_is_returned_unchanged_and_borrowed() {
        let redactor = PatternRedactor::new();
        for text in [
            "",
            "cargo test --workspace --locked",
            "test result: ok. 134 passed; 0 failed",
            "PASS: 12 tests, FAIL: 0",
            "input_tokens=1523 output_tokens=88 max_tokens: 4096",
            "token_count: 12",
            "if token == expected { return Ok(()) }",
            "use token::Kind;",
            "password=",
            "password: null,",
            "secret=false",
            "\"auth\": {",
            "the bearer of this message",
            "Bearer shorttoken",
            "author: Ada Lovelace",
            "https://registry.npmjs.org/@scope/package",
            "ssh://git@github.com/owner/repo.git",
            "git@github.com:owner/repo.git",
            "mail ada@example.com about https://example.com/path?x=1",
            "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----",
            "AKIA is a prefix; AKIAIOSFODNN7EXAMPLEEXTRA is too long",
            "task-force-alpha-bravo-charlie-delta",
            "run_id=run-1 ticket_id=F07 worker_id=w-1 operation_id=op-9",
            "{\"run_id\":\"run-1\",\"tokens\":[1,2],\"ok\":true}",
            "C:\\Users\\ada\\AppData\\Local\\Vela\\logs",
        ] {
            assert!(
                matches!(redactor.redact(text), Cow::Borrowed(_)),
                "`{text}` became `{}`",
                redactor.redact(text)
            );
        }
    }

    #[test]
    fn named_values_keep_their_surroundings() {
        for (text, expected) in [
            (
                "DB_PASSWORD=hunter2 npm test",
                "DB_PASSWORD=[REDACTED] npm test",
            ),
            (
                "export API_KEY='a b c' && run",
                "export API_KEY='[REDACTED]' && run",
            ),
            (
                "$env:GITHUB_TOKEN = \"abc\"; gh pr list",
                "$env:GITHUB_TOKEN = \"[REDACTED]\"; gh pr list",
            ),
            (
                "  \"password\": \"p\\\"q\",",
                "  \"password\": \"[REDACTED]\",",
            ),
            (
                "  \"client_secret\": 12345,",
                "  \"client_secret\": [REDACTED],",
            ),
            (
                "{\\\"token\\\":\\\"abc\\\"} trailing",
                "{\\\"token\\\":\\\"[REDACTED]\\\"} trailing",
            ),
            ("password: correct horse battery", "password: [REDACTED]"),
            (
                "gh auth login --with-token abc",
                "gh auth login --with-token [REDACTED]",
            ),
            (
                "mysql --password=p@ss -u root",
                "mysql --password=[REDACTED] -u root",
            ),
            ("tool --password --verbose", "tool --password --verbose"),
            (
                "Authorization: Basic dXNlcjpwYXNz\r\nHost: x",
                "Authorization: [REDACTED]\r\nHost: x",
            ),
            ("auth=Bearer abc next", "auth=[REDACTED] next"),
            (
                "Set-Cookie: sid=abc; Path=/; HttpOnly",
                "Set-Cookie: [REDACTED]",
            ),
            ("password='unterminated value", "password='[REDACTED]"),
            ("?access_token=abc&page=2", "?access_token=[REDACTED]"),
            // Source code is indistinguishable from configuration, so a type annotation is redacted too.
            (
                "let password: Option<String> = None;",
                "let password: [REDACTED]",
            ),
        ] {
            assert_eq!(redact(text), expected, "{text}");
        }
    }

    #[test]
    fn url_credentials_and_private_keys() {
        assert_eq!(
            redact("git clone https://ada:s3cr3t@github.com/o/r.git"),
            "git clone https://[REDACTED]@github.com/o/r.git"
        );
        assert_eq!(
            redact("remote: https://x-access-token@example.com"),
            "remote: https://[REDACTED]@example.com"
        );
        let key = "before\n-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaA==\nAAAA\n-----END OPENSSH PRIVATE KEY-----\nafter";
        assert_eq!(redact(key), "before\n[REDACTED]\nafter");
        let open = "x -----BEGIN RSA PRIVATE KEY-----\nMIIE\nmore";
        assert_eq!(redact(open), "x [REDACTED]");
    }

    #[test]
    fn registered_secrets_are_redacted_everywhere() {
        let redactor = PatternRedactor::new()
            .with_secret("correct-horse")
            .with_secret("correct-horse-battery")
            .with_secret("abc")
            .with_environment([
                ("DB_PASSWORD", "tr0ub4dor&3"),
                ("HOME", "/home/ada-lovelace"),
            ]);
        assert_eq!(
            redactor.redact(
                "got correct-horse-battery and correct-horse, tr0ub4dor&3 in /home/ada-lovelace abc"
            ),
            "got [REDACTED] and [REDACTED], [REDACTED] in /home/ada-lovelace abc"
        );
        assert_eq!(
            redactor.redact("{\"note\":\"quoted \\\"tr0ub4dor&3\\\"\"}"),
            "{\"note\":\"quoted \\\"[REDACTED]\\\"\"}"
        );
        let shown = format!("{redactor:?}");
        assert!(!shown.contains("horse") && shown.contains('3'), "{shown}");
        let mut redactor = PatternRedactor::new();
        assert!(!redactor.add_secret("short"));
        assert!(!redactor.add_secret("REDACTED"));
        assert!(redactor.add_secret("long-enough"));
    }

    #[test]
    fn named_values_redact_by_name() {
        let redactor = PatternRedactor::new();
        assert_eq!(redactor.redact_value("GITHUB_TOKEN", "anything"), REDACTED);
        assert_eq!(redactor.redact_value("GITHUB_TOKEN", ""), "");
        assert_eq!(redactor.redact_value("PATH", "/usr/bin"), "/usr/bin");
        assert_eq!(
            redactor.redact_value("GIT_URL", "https://ada:pw@example.com/r"),
            "https://[REDACTED]@example.com/r"
        );
    }

    #[test]
    fn hostile_input_stays_linear_and_on_character_boundaries() {
        let redactor = PatternRedactor::new();
        let dashes = "-".repeat(200_000);
        assert!(matches!(redactor.redact(&dashes), Cow::Borrowed(_)));
        let dots = "-.".repeat(100_000);
        assert!(matches!(redactor.redact(&dots), Cow::Borrowed(_)));
        assert_eq!(
            redactor.redact("пароль password=секрет✓ конец"),
            "пароль password=[REDACTED] конец"
        );
        assert_eq!(
            redactor.redact("密码 token: 秘密\n次"),
            "密码 token: [REDACTED]\n次"
        );
        assert_eq!(
            redactor.redact("é\"password\":\"ü\\"),
            "é\"password\":\"[REDACTED]"
        );
    }
}
