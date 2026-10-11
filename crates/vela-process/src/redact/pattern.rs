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

/// The separator of a two-column listing (a wide gap), as [`named_value`] records it.
const COLUMN: u8 = b'\t';

/// How far past a URL's authority the `@` of its user information is looked for.
const URL_LOOKAHEAD: usize = 256;

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
            // A path is not a secret even under a credential-like name (`GOOGLE_APPLICATION_CREDENTIALS`), and
            // registering one would redact every file name below it.
            if is_credential_name(name) && !looks_like_path(name, value) {
                self.add_secret(value);
            }
        }
        self
    }

    /// Registers a known secret value. Returns whether it was accepted: long enough, not part of the marker, and not
    /// a placeholder such as `undefined` (replacing every occurrence of one would only damage the output).
    pub fn add_secret(&mut self, value: &str) -> bool {
        let placeholder = EMPTY_VALUES
            .iter()
            .any(|empty| value.as_bytes().eq_ignore_ascii_case(empty));
        if value.len() < MIN_SECRET_LEN || REDACTED.contains(value) || placeholder {
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
    /// The value at `start..end`, unless it is empty or is already the marker. Only a value that is exactly the
    /// marker is left alone, so a secret cannot hide behind a marker written in front of it.
    fn value(bytes: &[u8], start: usize, end: usize) -> Option<Self> {
        (end > start && &bytes[start..end] != REDACTED.as_bytes()).then_some(Self { start, end })
    }
}

/// An absolute or explicitly relative file path: `~/x`, `./x`, `C:\x`, `\\server\x`, and `/usr/lib/x` when the
/// variable's name says it holds a location. A base64 secret can also start with `/` and contain another, so for any
/// other name such a value is taken to be the secret.
fn looks_like_path(name: &str, value: &str) -> bool {
    let bytes = value.as_bytes();
    let drive = bytes.len() > 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/');
    let name = name.to_ascii_lowercase();
    let names_a_location = ["credentials", "config", "home", "dir"]
        .iter()
        .any(|word| name.contains(word));
    let rooted = names_a_location
        && bytes.first() == Some(&b'/')
        && bytes[1..].contains(&b'/')
        && !bytes.iter().any(|byte| matches!(byte, b'+' | b'='));
    drive
        || rooted
        || ["~/", "./", "../", "\\\\"]
            .iter()
            .any(|prefix| value.starts_with(prefix))
}

/// What one scan has already learned, so that hostile input cannot make it look at the same bytes again and again.
struct Memo {
    /// No JSON Web Token starts before this index (a failed attempt covered everything up to it).
    token_floor: usize,
    /// The end of the line that contains every index in `line_start..=line_end`.
    line_start: usize,
    line_end: usize,
    /// Where the line being scanned begins, when the scan saw its line break (it has not after a hit that spans
    /// lines).
    line_begin: Option<usize>,
}

impl Memo {
    fn new() -> Self {
        Self {
            token_floor: 0,
            line_start: usize::MAX,
            line_end: 0,
            line_begin: Some(0),
        }
    }

    /// What stands on the line before `index`, when that is known and short. A longer prefix is not looked at: the
    /// scan must not walk back over a long line once per name on it.
    fn line_prefix<'b>(&self, bytes: &'b [u8], index: usize) -> Option<&'b [u8]> {
        let begin = self.line_begin?;
        (index - begin <= MAX_LINE_PREFIX).then(|| &bytes[begin..index])
    }

    /// The index of the line break that ends the line containing `at` (or the end of the text).
    fn line_end(&mut self, bytes: &[u8], at: usize) -> usize {
        if at < self.line_start || at > self.line_end {
            self.line_start = at;
            self.line_end = at
                + bytes[at..]
                    .iter()
                    .take_while(|byte| !matches!(**byte, b'\n' | b'\r'))
                    .count();
        }
        self.line_end
    }
}

fn scan(text: &str) -> Cow<'_, str> {
    let bytes = text.as_bytes();
    let mut memo = Memo::new();
    let mut out = String::new();
    let mut copied = 0;
    let mut index = 0;
    while index < bytes.len() {
        if matches!(bytes[index], b'\n' | b'\r') {
            memo.line_begin = Some(index + 1);
        }
        match hit_at(bytes, index, &mut memo) {
            // Every hit starts and ends next to an ASCII byte or at the end of the text, so the slices below are on
            // character boundaries.
            Some(hit) => {
                out.push_str(&text[copied..hit.start]);
                out.push_str(REDACTED);
                // A hit that spans lines (a private key) hides where the current line began.
                if bytes[hit.start..hit.end].contains(&b'\n') {
                    memo.line_begin = None;
                }
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

fn hit_at(bytes: &[u8], index: usize, memo: &mut Memo) -> Option<Hit> {
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
    if !is_name_byte(byte) {
        return None;
    }
    let word_start = starts_word(bytes, index);
    if let Some(hit) = token(bytes, index, word_start, memo) {
        return Some(hit);
    }
    if !word_start {
        return None;
    }
    bearer(bytes, index).or_else(|| named_value(bytes, index, memo))
}

/// Whether a word can start at `index`: at the start of the text, after a byte that is not part of a word, or after
/// something that only looks like part of one: an escape (`\n` in a `Debug`-formatted string), a percent escape
/// (`%3D`) or a terminal colour sequence (`ESC[1m`).
fn starts_word(bytes: &[u8], index: usize) -> bool {
    let Some(&previous) = index.checked_sub(1).and_then(|at| bytes.get(at)) else {
        return true;
    };
    if !is_word_byte(previous) {
        return true;
    }
    let escape = index >= 2
        && bytes[index - 2] == b'\\'
        && matches!(
            previous,
            b'n' | b'r' | b't' | b'b' | b'f' | b'v' | b'a' | b'e' | b'0'
        );
    let percent = index >= 3
        && bytes[index - 3] == b'%'
        && bytes[index - 2].is_ascii_hexdigit()
        && previous.is_ascii_hexdigit();
    if escape || percent {
        return true;
    }
    // `ESC [ parameters letter`: walk back over the parameters (a short, bounded distance).
    if previous.is_ascii_alphabetic() {
        let floor = index.saturating_sub(16);
        let mut at = index - 1;
        while at > floor && matches!(bytes[at - 1], b'0'..=b'9' | b';' | b'?') {
            at -= 1;
        }
        return at >= 2 && bytes[at - 1] == b'[' && bytes[at - 2] == 0x1b;
    }
    false
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

/// A token at `index`. Most prefixes identify a token wherever they appear, so they are recognized even in the middle
/// of a word (after an escape, inside an encoded string); the two short, ambiguous ones need a word start.
fn token(bytes: &[u8], index: usize, word_start: bool, memo: &mut Memo) -> Option<Hit> {
    let rest = &bytes[index..];
    if index >= memo.token_floor && rest.starts_with(b"eyJ") {
        match json_web_token(rest) {
            Ok(len) => {
                return Some(Hit {
                    start: index,
                    end: index + len,
                });
            }
            // Nothing inside the bytes that attempt looked at can start a token either.
            Err(examined) => memo.token_floor = index + examined,
        }
    }
    for shape in &SHAPES {
        let Some(body) = rest.strip_prefix(shape.prefix) else {
            continue;
        };
        if !word_start && matches!(shape.prefix, b"sk-" | b"1//") {
            continue;
        }
        let mut len = body
            .iter()
            .take(shape.max.saturating_add(1))
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

/// Three base64url segments separated by dots, the first starting with `eyJ` (an encoded `{"`). Returns the length of
/// the token, or the number of bytes examined when `rest` (which starts with `eyJ`) is not one: a token starting
/// anywhere inside those bytes would end at the same place and fail the same way.
fn json_web_token(rest: &[u8]) -> Result<usize, usize> {
    let segment = |from: usize| {
        rest[from..]
            .iter()
            .take_while(|byte| Charset::UrlSafe.contains(**byte))
            .count()
    };
    let header = segment(0);
    if header < 10 || rest.get(header) != Some(&b'.') {
        return Err(header);
    }
    let payload = segment(header + 1);
    let signature_at = header + 1 + payload;
    if payload < 10 || rest.get(signature_at) != Some(&b'.') {
        return Err(signature_at);
    }
    Ok(signature_at + 1 + segment(signature_at + 1))
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

/// The longest line prefix that is examined to decide whether a name starts its line.
const MAX_LINE_PREFIX: usize = 64;

/// Keywords that may stand directly before an assignment: `export NAME=value`, `$env:NAME = value`.
const ASSIGNMENT_KEYWORDS: [&[u8]; 8] = [
    b"declare -x",
    b"readonly",
    b"export",
    b"local",
    b"$env:",
    b"setx",
    b"set",
    b"env",
];

/// Whether a name with this line `prefix` is the first word on its line, as in the output of `env`, `set` and
/// `printenv`, in a `.env` file, and in the same lines behind a marker. There the value is the rest of the line.
///
/// Before the name there may be an assignment keyword, and before that anything that is not a word: indentation,
/// the markers of a list, a diff or a quotation (`-`, `+`, `>`, `*`), numbering, a timestamp.
fn starts_line(prefix: &[u8]) -> bool {
    let mut prefix = prefix.trim_ascii();
    for keyword in ASSIGNMENT_KEYWORDS {
        let Some(split) = prefix.len().checked_sub(keyword.len()) else {
            continue;
        };
        let (head, tail) = prefix.split_at(split);
        if tail.eq_ignore_ascii_case(keyword)
            && head.last().is_none_or(|byte| !byte.is_ascii_alphanumeric())
        {
            prefix = head;
            break;
        }
    }
    // `T` and `Z` are the letters of an ISO 8601 timestamp.
    prefix
        .iter()
        .all(|byte| byte.is_ascii() && (!byte.is_ascii_alphabetic() || matches!(byte, b'T' | b'Z')))
}

/// The end of a value that opens with an escaped quote (`\"`): its closing `\"`, or `limit`. Inside such a value an
/// escaped quote is written `\\\"` and an escaped backslash `\\\\`; neither ends it.
fn escaped_quote_end(bytes: &[u8], start: usize, limit: usize) -> usize {
    let mut at = start;
    while at < limit {
        if bytes[at] != b'\\' {
            at += 1;
        } else if bytes.get(at + 1) == Some(&b'"') {
            return at;
        } else if bytes.get(at + 1) == Some(&b'\\') {
            // `\\` introduces an inner escape; skip it together with what it escapes.
            at += 2;
            let escaped = &bytes[at.min(limit)..limit];
            at += if escaped.starts_with(b"\\\"") || escaped.starts_with(b"\\\\") {
                2
            } else {
                1
            };
        } else {
            at += 1;
        }
    }
    limit
}

fn named_value(bytes: &[u8], index: usize, memo: &mut Memo) -> Option<Hit> {
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

    let next = bytes.get(at + 1).copied();
    // `a == b`, `a === b` and `a =~ b`, written with blanks on both sides, compare; they do not assign.
    let operator_len = if next == Some(b'=') && bytes.get(at + 2) == Some(&b'=') {
        3
    } else {
        2
    };
    let comparison = gap > 0
        && matches!(next, Some(b'=' | b'~'))
        && matches!(bytes.get(at + operator_len), Some(b' ' | b'\t'));
    let prefix = memo.line_prefix(bytes, index);
    let flag = bytes[index] == b'-';
    // A two-column listing, as PowerShell prints the environment: the name, a wide gap, the value.
    let column = !flag
        && !quoted_name
        && (gap >= 2 || bytes[at - gap..at].contains(&b'\t'))
        && prefix.is_some_and(|prefix| prefix.trim_ascii().is_empty());
    // `.netrc`: `machine host login user password value`.
    let netrc = gap > 0
        && !quoted_name
        && (name.eq_ignore_ascii_case("password") || name.eq_ignore_ascii_case("passwd"))
        && prefix.is_some_and(|prefix| {
            find(prefix, b"login ").is_some() || find(prefix, b"machine ").is_some()
        });
    let (separator, width) = match bytes.get(at) {
        // `'password' => 'hunter2'` assigns.
        Some(b'=') if next == Some(b'>') => (b'=', 2),
        Some(b'=') if !comparison => (b'=', 1),
        // `::` is a path separator.
        Some(b':') if next != Some(b':') => (b':', 1),
        // A command-line flag: `--password hunter2`.
        _ if (flag && !quoted_name && gap > 0) || netrc => (b' ', 0),
        _ if column => (COLUMN, 0),
        _ => return None,
    };
    at += width;
    if width > 0 {
        at += blank_len(&bytes[at..]);
    }

    let rest = &bytes[at..];
    if rest.is_empty() {
        return None;
    }
    let line_end = memo.line_end(bytes, at);

    // A quoted value ends at its closing quote, or at the end of the line when the quote is never closed.
    if rest.starts_with(b"\\\"") {
        let start = at + 2;
        return Hit::value(bytes, start, escaped_quote_end(bytes, start, line_end));
    }
    if let quote @ (b'"' | b'\'') = rest[0] {
        let start = at + 1;
        let mut end = start;
        while end < line_end && bytes[end] != quote {
            end += if bytes[end] == b'\\' { 2 } else { 1 };
        }
        return Hit::value(bytes, start, end.min(line_end));
    }
    if separator == b' ' && rest[0] == b'-' {
        return None;
    }

    // A quote in front of the name wraps the whole assignment (`set "API_KEY=a b"`, `curl -d 'token=a b'`): the
    // value runs to that quote's partner.
    let wrapping_quote = (separator == b'=' && !quoted_name && index > 0)
        .then(|| bytes[index - 1])
        .filter(|byte| matches!(byte, b'"' | b'\''));

    // An unquoted value: the rest of the line after `:` (headers, YAML), in a two-column listing and in an assignment
    // that starts its line (environment dumps); otherwise one word after `=` or a flag (shell command lines).
    let whole_line = separator == b':'
        || separator == COLUMN
        // (a single leading dash is the removed line of a diff, two are a command-line flag)
        || (separator == b'='
            && !bytes[index..].starts_with(b"--")
            && prefix.is_some_and(starts_line));
    let mut end = if let Some(quote) = wrapping_quote {
        at + bytes[at..line_end]
            .iter()
            .take_while(|byte| **byte != quote)
            .count()
    } else if whole_line {
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
        || matches!(value, b"{" | b"[")
        // The rule under a column heading.
        || (separator == COLUMN && value.iter().all(|byte| matches!(byte, b'-' | b' ' | b'\t')));
    if not_a_secret {
        return None;
    }
    Hit::value(bytes, at, end)
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
    let at = match authority.iter().rposition(|byte| *byte == b'@') {
        Some(at) => at,
        // `user:pa/ss@host`: an unencoded `/`, `?` or `#` in the password ended the authority early. What follows
        // the colon is not a port, so look a little further along the same word for the `@`.
        None => {
            let colon = authority.iter().position(|byte| *byte == b':')?;
            let after = &authority[colon + 1..];
            // An IPv6 literal (`[::1]:3000`) and a template (`localhost:${PORT}`) have colons of their own.
            let not_user_info = authority.first() == Some(&b'[')
                || authority.iter().any(|byte| matches!(byte, b'$' | b'{'));
            if after.is_empty() || after.iter().all(u8::is_ascii_digit) || not_user_info {
                return None;
            }
            let word_len = bytes[start..]
                .iter()
                .take(URL_LOOKAHEAD)
                .take_while(|byte| !byte.is_ascii_whitespace())
                .count();
            bytes[start..start + word_len]
                .iter()
                .rposition(|byte| *byte == b'@')?
        }
    };
    let user_info = &bytes[start..start + at];
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
                    '\u{1b}' => text.push_str("\\u001b"),
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
            "if (password === input) { ok() }",
            "token === expected",
            "the password is wrong",
            "password reset requested",
            "token  ",
            "http://[::1]:3000/@fs/x",
            "http://localhost:${PORT}/@vite/client",
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
                "run DB_PASSWORD=hunter2 npm test",
                "run DB_PASSWORD=[REDACTED] npm test",
            ),
            // An assignment that starts its line is an environment dump: the value is the rest of the line.
            ("DB_PASSWORD=hunter2 npm test", "DB_PASSWORD=[REDACTED]"),
            (
                "DB_PASSWORD=correct horse battery",
                "DB_PASSWORD=[REDACTED]",
            ),
            ("export COOKIE=a=1; b=2", "export COOKIE=[REDACTED]"),
            (
                ".env\nAPI_KEY=a b\nNEXT=1",
                ".env\nAPI_KEY=[REDACTED]\nNEXT=1",
            ),
            ("DB_PASSWORD=~Tr0ub4dor", "DB_PASSWORD=[REDACTED]"),
            (
                "+API_KEY=a b
-API_KEY=c d
 API_KEY=e f",
                "+API_KEY=[REDACTED]
-API_KEY=[REDACTED]
 API_KEY=[REDACTED]",
            ),
            ("--password=a b", "--password=[REDACTED] b"),
            ("x DB_PASSWORD==abc y", "x DB_PASSWORD=[REDACTED] y"),
            ("'password' => 'hunter2',", "'password' => '[REDACTED]',"),
            // The first word of a line may stand behind indentation, a list or diff marker, or a timestamp.
            (
                "  - DB_PASSWORD=my secret pass",
                "  - DB_PASSWORD=[REDACTED]",
            ),
            ("-    DB_PASSWORD=a b", "-    DB_PASSWORD=[REDACTED]"),
            (
                "                              DB_PASSWORD=a b",
                "                              DB_PASSWORD=[REDACTED]",
            ),
            (
                "2026-10-11T19:30:21.123Z   export DB_PASSWORD=a b",
                "2026-10-11T19:30:21.123Z   export DB_PASSWORD=[REDACTED]",
            ),
            ("12: [3/5] * API_KEY=a b", "12: [3/5] * API_KEY=[REDACTED]"),
            ("APP_KEY=base64:Zm9v YmFy", "APP_KEY=[REDACTED]"),
            // A two-column listing (PowerShell `Get-ChildItem Env:`) and a `.netrc` entry.
            (
                "Name                           Value\n----                           -----\nDB_PASSWORD                    hunter2 x\nPATH                           C:\\bin",
                "Name                           Value\n----                           -----\nDB_PASSWORD                    [REDACTED]\nPATH                           C:\\bin",
            ),
            ("API_TOKEN\tabc def", "API_TOKEN\t[REDACTED]"),
            (
                "machine example.com login ada password hunter2 port 21",
                "machine example.com login ada password [REDACTED] port 21",
            ),
            // An escaped quote inside an escaped-quote value does not end it.
            (
                "{\\\"password\\\":\\\"ab\\\\\\\"cd-rest\\\\\\\\\\\"} x",
                "{\\\"password\\\":\\\"[REDACTED]\\\"} x",
            ),
            (
                "x password => hunter2 next",
                "x password => [REDACTED] next",
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
            ("use auth=Bearer abc next", "use auth=[REDACTED] next"),
            (
                "Set-Cookie: sid=abc; Path=/; HttpOnly",
                "Set-Cookie: [REDACTED]",
            ),
            ("password='unterminated value", "password='[REDACTED]"),
            // A marker in front of a secret does not protect it.
            ("a password=[REDACTED]hunter2 x", "a password=[REDACTED] x"),
            ("token: [REDACTED] and more", "token: [REDACTED]"),
            ("?access_token=abc&page=2", "?access_token=[REDACTED]"),
            (
                "set \"API_KEY=a b c\" & build",
                "set \"API_KEY=[REDACTED]\" & build",
            ),
            (
                "curl -d 'password=a b&x=1' https://x",
                "curl -d 'password=[REDACTED]' https://x",
            ),
            (
                "with auth_tokens=abc api_keys: k1, k2",
                "with auth_tokens=[REDACTED] api_keys: [REDACTED]",
            ),
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
    fn tokens_are_found_after_escapes_and_inside_words() {
        let token = ["gh", "p_", "0123456789abcdefghijABCDEFGHIJ012345"].concat();
        let jwt = [
            "ey",
            "JhbGciOiJIUzI1NiJ9.ey",
            "JzdWIiOiIxMjM0NTYifQ.c2lnbmF0dXJl",
        ]
        .concat();
        for (text, expected) in [
            // A Debug-formatted string: the line break is a backslash and an `n`.
            (
                format!("stderr: \"remote: denied\\n{token}\\ncookie: sid=abc\""),
                "stderr: \"remote: denied\\n[REDACTED]\\ncookie: [REDACTED]".to_owned(),
            ),
            (
                format!("\\tapi_key=abc \\r{token}"),
                "\\tapi_key=[REDACTED] \\r[REDACTED]".to_owned(),
            ),
            // Terminal colours.
            (
                format!("\u{1b}[1m{token}\u{1b}[0m \u{1b}[38;5;9mpassword=abc"),
                "\u{1b}[1m[REDACTED]\u{1b}[0m \u{1b}[38;5;9mpassword=[REDACTED]".to_owned(),
            ),
            // Percent-encoded text.
            (
                format!("next=token%3D{token}%26x Bearer%20{jwt}"),
                "next=token%3D[REDACTED]%26x Bearer%20[REDACTED]".to_owned(),
            ),
            // In the middle of a word.
            (format!("prefix{token}"), "prefix[REDACTED]".to_owned()),
            (format!("x{jwt} y"), "x[REDACTED] y".to_owned()),
        ] {
            assert_eq!(redact(&text), expected, "{text}");
        }
        // The two short prefixes still need a word start.
        for text in [
            "ask-aaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "v1//aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ] {
            assert_eq!(redact(text), text);
        }
    }

    #[test]
    fn url_credentials_and_private_keys() {
        assert_eq!(
            redact("DATABASE_URL=postgres://app:pa/ss#w0rd@db.internal/prod"),
            "DATABASE_URL=postgres://[REDACTED]@db.internal/prod"
        );
        for text in [
            "http://localhost:3000/@fs/x",
            "https://example.com/@user",
            "ftp://h:21/a@b",
        ] {
            assert_eq!(redact(text), text);
        }
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
        // Paths and placeholders under credential-like names are not registered, and `PWD` is not one.
        let redactor = PatternRedactor::new().with_environment([
            ("PWD", "/c/Users/ada/worktrees/f07"),
            ("GOOGLE_APPLICATION_CREDENTIALS", "C:\\Users\\ada\\key.json"),
            ("SSH_KEY_PASSPHRASE", "~/secrets/passphrase"),
            ("API_TOKEN", "undefined"),
            ("AWS_SECRET_ACCESS_KEY", "/Abc+def/GhiJkl="),
            ("AWS_SESSION_TOKEN", "/aB3dEf9/Kq7MDENGbPxRfiCYzz"),
            ("APP_KEY", "base64:Zm9vYmFy"),
        ]);
        assert_eq!(
            redactor.redact("saw /aB3dEf9/Kq7MDENGbPxRfiCYzz and base64:Zm9vYmFy"),
            "saw [REDACTED] and [REDACTED]"
        );
        assert_eq!(
            redactor.redact("at /c/Users/ada/worktrees/f07/src C:\\Users\\ada\\key.json ~/secrets/passphrase undefined /Abc+def/GhiJkl="),
            "at /c/Users/ada/worktrees/f07/src C:\\Users\\ada\\key.json ~/secrets/passphrase undefined [REDACTED]"
        );
        let mut redactor = PatternRedactor::new();
        assert!(!redactor.add_secret("undefined"));
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

    /// Each input below made an earlier version look at the rest of the line once per occurrence. At these sizes
    /// that is tens of billions of steps, so the test cannot finish unless the scan is linear.
    #[test]
    fn hostile_input_stays_linear() {
        let redactor = PatternRedactor::new();
        for unit in [
            "-",
            "-.",
            "eyJ-",
            "a://b:c/",
            "x password=null ",
            "token == x ",
            "Bearer x ",
        ] {
            let text = unit.repeat(1_000_000 / unit.len());
            assert!(matches!(redactor.redact(&text), Cow::Borrowed(_)), "{unit}");
        }
        // A run of token-shaped segments is a run of tokens, each found once.
        let text = "eyJhbGciOiJIUzI1NiJ9.".repeat(50_000);
        assert!(!redactor.redact(&text).contains("eyJ"));
        // Only the last twenty bytes of this run have the shape of an access key id.
        let text = "AKIA".repeat(250_000);
        assert!(redactor.redact(&text).ends_with(REDACTED));
        let text = " token=x".repeat(100_000).replacen(' ', "x ", 1);
        assert_eq!(redactor.redact(&text).matches(REDACTED).count(), 100_000);
        let text = format!("x {}", "password: p, ".repeat(100_000));
        assert_eq!(redactor.redact(&text), "x password: [REDACTED], ");
    }

    #[test]
    fn redaction_stays_on_character_boundaries() {
        let redactor = PatternRedactor::new();
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
