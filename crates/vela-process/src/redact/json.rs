//! Structure-preserving redaction of serialized JSON (journal payloads, JSON log records, stream-json output).
//!
//! Redacting JSON as plain text would either corrupt the document or stop at a quote that belongs to the secret. This
//! module walks the document instead: every string is unescaped, redacted as free text (or as a named value when it is
//! an object member) and written back escaped, so the output is still JSON with the same shape.
//!
//! Three things are known only from the structure, and are redacted because of it:
//!
//! - everything inside a container stored under a credential-like name (`"credentials": {"user": "..."}`);
//! - the `value` of a name and value pair whose name is credential-like (`{"name": "Cookie", "value": "..."}`),
//!   whichever of the two members comes first;
//! - the array element after a bare credential flag (`["mysql", "--password", "..."]`).
//!
//! The walk is a single pass with an explicit stack. It never copies a member name per element or per level, so its
//! cost is linear in the document whatever the document looks like.

use std::borrow::Cow;
use std::fmt::Write as _;
use std::rc::Rc;

use super::names::{MAX_NAME_LEN, is_credential_name};
use super::{REDACTED, Redactor};

/// Members that name the credential held by a sibling `value` member (`{"name": "DB_PASSWORD", "value": "..."}`).
const PAIR_NAMES: [&str; 4] = ["name", "key", "header", "variable"];
const PAIR_VALUES: [&str; 2] = ["value", "val"];

/// Documents nested deeper than this are not walked; they are redacted as plain text instead.
const MAX_DEPTH: usize = 128;

/// One open container.
struct Frame {
    /// An object (`{`) rather than an array (`[`).
    object: bool,
    /// The member name the container is stored under. The elements of an array are values of that name, and an array
    /// inside an array inherits it.
    name: Option<Rc<str>>,
    /// The container is stored, directly or through its ancestors, under a credential-like name: every string and
    /// number in it is a credential.
    credential: bool,
    /// Object only: a [`PAIR_NAMES`] member held a credential name, so the [`PAIR_VALUES`] member holds the credential.
    credential_pair: bool,
    /// Object only: a [`PAIR_VALUES`] member seen before any credential [`PAIR_NAMES`] member. It is redacted after
    /// the fact if such a name member follows.
    pair_value: Option<Held>,
    /// Array only: the previous element was a bare credential flag, so the next one is its value.
    flag_value: bool,
}

/// A value that may still have to be redacted: where it is in the text, and the edit already made to it, if any.
struct Held {
    start: usize,
    end: usize,
    edit: Option<usize>,
}

/// One replacement: the bytes `start..end` of the text become `text` (already JSON).
struct Edit {
    start: usize,
    end: usize,
    text: String,
}

/// Redacts every string and number of `text`, which must be one or more well-formed JSON containers.
///
/// Returns `None` when `text` is not JSON (truncated, nested too deeply, or not JSON at all); the caller then redacts
/// it as plain text.
pub(super) fn redact_document<'a>(text: &'a str, redactor: &dyn Redactor) -> Option<Cow<'a, str>> {
    let bytes = text.as_bytes();
    let mut edits: Vec<Edit> = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();
    // The name of the member whose value comes next.
    let mut key: Option<Rc<str>> = None;
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b' ' | b'\t' | b'\n' | b'\r' | b',' => index += 1,
            open @ (b'{' | b'[') => {
                if stack.len() >= MAX_DEPTH {
                    return None;
                }
                let name = value_name(&mut key, &stack);
                let credential = stack.last().is_some_and(|parent| parent.credential)
                    || name.as_deref().is_some_and(is_credential_name);
                if let Some(parent) = stack.last_mut() {
                    parent.flag_value = false;
                }
                stack.push(Frame {
                    object: open == b'{',
                    name,
                    credential,
                    credential_pair: false,
                    pair_value: None,
                    flag_value: false,
                });
                index += 1;
            }
            close @ (b'}' | b']') => {
                if stack.pop()?.object != (close == b'}') {
                    return None;
                }
                key = None;
                index += 1;
            }
            b'"' => {
                let (content, end) = parse_string(text, index)?;
                let after = end + blank_len(&bytes[end..]);
                let is_key = key.is_none()
                    && stack.last().is_some_and(|frame| frame.object)
                    && bytes.get(after) == Some(&b':');
                if is_key {
                    let shown = redactor.redact(&content);
                    if shown != content {
                        edits.push(Edit {
                            start: index,
                            end,
                            text: json_string(&shown),
                        });
                    }
                    key = Some(Rc::from(&*content));
                    index = after + 1;
                } else {
                    let name = value_name(&mut key, &stack);
                    let value = Value {
                        name: name.as_deref(),
                        content: &content,
                        start: index,
                        end,
                        is_string: true,
                    };
                    redact_value(redactor, &mut edits, stack.last_mut(), &value);
                    index = end;
                }
            }
            _ => {
                let end = index + scalar_len(&bytes[index..]);
                let token = text.get(index..end)?;
                let name = value_name(&mut key, &stack);
                if is_number(token) {
                    // A redacted number becomes a string so the document stays valid.
                    let value = Value {
                        name: name.as_deref(),
                        content: token,
                        start: index,
                        end,
                        is_string: false,
                    };
                    redact_value(redactor, &mut edits, stack.last_mut(), &value);
                } else if matches!(token, "true" | "false" | "null") {
                    if let Some(frame) = stack.last_mut() {
                        frame.flag_value = false;
                    }
                } else {
                    return None;
                }
                index = end;
            }
        }
    }

    if !stack.is_empty() {
        return None;
    }
    if edits.is_empty() {
        return Some(Cow::Borrowed(text));
    }
    // Edits are made in text order except for a held pair value, which is redacted when its name arrives.
    edits.sort_by_key(|edit| edit.start);
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    for edit in &edits {
        out.push_str(&text[copied..edit.start]);
        out.push_str(&edit.text);
        copied = edit.end;
    }
    out.push_str(&text[copied..]);
    Some(Cow::Owned(out))
}

/// The name of the value that comes next: the pending member name, or the name the enclosing array is stored under.
fn value_name(key: &mut Option<Rc<str>>, stack: &[Frame]) -> Option<Rc<str>> {
    key.take().or_else(|| match stack.last() {
        Some(frame) if !frame.object => frame.name.clone(),
        _ => None,
    })
}

/// A command-line flag on its own that names a credential: `--password`, `-token`. (`--password=x` is a whole
/// assignment and is redacted as text; nothing follows it.)
fn is_bare_flag(content: &str) -> bool {
    content.starts_with('-')
        && content.len() <= MAX_NAME_LEN
        && content
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        && is_credential_name(content)
}

/// One string (`is_string`) or number of the document: its member name, its unescaped content and where it is.
struct Value<'v> {
    name: Option<&'v str>,
    content: &'v str,
    start: usize,
    end: usize,
    is_string: bool,
}

/// Redacts one value, using what its container says about it, and records the edit.
fn redact_value(
    redactor: &dyn Redactor,
    edits: &mut Vec<Edit>,
    mut frame: Option<&mut Frame>,
    value: &Value<'_>,
) {
    let Value {
        name,
        content,
        start,
        end,
        is_string,
    } = *value;
    let mut credential = false;
    let mut hold = false;
    if let Some(frame) = frame.as_deref_mut() {
        let follows_flag =
            std::mem::replace(&mut frame.flag_value, is_string && is_bare_flag(content));
        let pair_member = |members: &[&str]| {
            frame.object
                && name.is_some_and(|name| members.iter().any(|m| name.eq_ignore_ascii_case(m)))
        };
        let is_pair_name = pair_member(&PAIR_NAMES);
        let is_pair_value = pair_member(&PAIR_VALUES);
        if is_pair_name && is_string {
            frame.credential_pair = is_credential_name(content);
            // The value member came first: redact it now that its name is known.
            if frame.credential_pair
                && let Some(held) = frame.pair_value.take()
            {
                let text = json_string(REDACTED);
                match held.edit {
                    Some(edit) => edits[edit].text = text,
                    None => edits.push(Edit {
                        start: held.start,
                        end: held.end,
                        text,
                    }),
                }
            }
        }
        credential = frame.credential
            || (!frame.object && follows_flag)
            || (is_pair_value && frame.credential_pair);
        hold = is_pair_value && !credential && !content.is_empty();
    }

    let shown = if credential && !content.is_empty() {
        Cow::Borrowed(REDACTED)
    } else {
        match name {
            Some(name) => redactor.redact_value(name, content),
            None => redactor.redact(content),
        }
    };
    let edit = (shown != content).then(|| {
        edits.push(Edit {
            start,
            end,
            text: json_string(&shown),
        });
        edits.len() - 1
    });
    if hold && let Some(frame) = frame {
        frame.pair_value = Some(Held { start, end, edit });
    }
}

fn blank_len(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .take_while(|byte| matches!(**byte, b' ' | b'\t' | b'\n' | b'\r'))
        .count()
}

/// The length of a number or literal: everything up to the next structural byte. Never zero, so the walk always
/// advances; a stray structural byte (`:`) is returned as its own token and rejected by the caller.
fn scalar_len(bytes: &[u8]) -> usize {
    let len = bytes
        .iter()
        .take_while(|byte| {
            !matches!(
                **byte,
                b' ' | b'\t' | b'\n' | b'\r' | b',' | b':' | b'"' | b'{' | b'}' | b'[' | b']'
            )
        })
        .count();
    len.max(1)
}

fn is_number(token: &str) -> bool {
    token.bytes().any(|byte| byte.is_ascii_digit())
        && token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'-' | b'+' | b'.' | b'e' | b'E'))
}

/// Parses the string literal that starts at `start` (a `"`). Returns the unescaped content and the index just past the
/// closing quote.
fn parse_string(text: &str, start: usize) -> Option<(Cow<'_, str>, usize)> {
    let bytes = text.as_bytes();
    let mut owned: Option<String> = None;
    let mut segment = start + 1;
    let mut index = segment;
    loop {
        match *bytes.get(index)? {
            b'"' => {
                let content = match owned {
                    None => Cow::Borrowed(&text[segment..index]),
                    Some(mut unescaped) => {
                        unescaped.push_str(&text[segment..index]);
                        Cow::Owned(unescaped)
                    }
                };
                return Some((content, index + 1));
            }
            b'\\' => {
                let unescaped = owned.get_or_insert_with(String::new);
                unescaped.push_str(&text[segment..index]);
                let escape = *bytes.get(index + 1)?;
                index += 2;
                match escape {
                    b'"' => unescaped.push('"'),
                    b'\\' => unescaped.push('\\'),
                    b'/' => unescaped.push('/'),
                    b'b' => unescaped.push('\u{8}'),
                    b'f' => unescaped.push('\u{c}'),
                    b'n' => unescaped.push('\n'),
                    b'r' => unescaped.push('\r'),
                    b't' => unescaped.push('\t'),
                    b'u' => {
                        let (character, next) = unicode_escape(bytes, index)?;
                        unescaped.push(character);
                        index = next;
                    }
                    _ => return None,
                }
                segment = index;
            }
            0x00..=0x1f => return None,
            _ => index += 1,
        }
    }
}

/// Decodes the four hex digits at `index` (just past `\u`), joining a surrogate pair. Returns the character and the
/// index past it. A surrogate without its partner is not text; it decodes to the replacement character, which only
/// shows if the string it is in is redacted and rewritten.
fn unicode_escape(bytes: &[u8], index: usize) -> Option<(char, usize)> {
    let first = hex4(bytes, index)?;
    if !(0xD800..0xE000).contains(&first) {
        return Some((char::from_u32(first)?, index + 4));
    }
    let lone = Some((char::REPLACEMENT_CHARACTER, index + 4));
    if first >= 0xDC00 || bytes.get(index + 4..index + 6) != Some(b"\\u".as_slice()) {
        return lone;
    }
    let Some(second) = hex4(bytes, index + 6) else {
        return lone;
    };
    if !(0xDC00..0xE000).contains(&second) {
        return lone;
    }
    let scalar = 0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00);
    Some((char::from_u32(scalar)?, index + 10))
}

fn hex4(bytes: &[u8], index: usize) -> Option<u32> {
    let digits = bytes.get(index..index + 4)?;
    let mut value = 0;
    for digit in digits {
        value = value * 16 + char::from(*digit).to_digit(16)?;
    }
    Some(value)
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    push_json_string(&mut out, value);
    out
}

fn push_json_string(out: &mut String, value: &str) {
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if u32::from(control) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(control));
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::redact::PatternRedactor;

    fn redact(text: &str) -> Option<String> {
        redact_document(text, &PatternRedactor::new()).map(Cow::into_owned)
    }

    #[test]
    fn credential_members_are_redacted_and_the_shape_is_kept() {
        let text = r#"{"user":"ada","password":"hunter2","nested":{"api_key":"k-123","n":1},"list":[true,null,2.5e3]}"#;
        assert_eq!(
            redact(text).unwrap(),
            r#"{"user":"ada","password":"[REDACTED]","nested":{"api_key":"[REDACTED]","n":1},"list":[true,null,2.5e3]}"#
        );
    }

    #[test]
    fn untouched_documents_are_returned_byte_for_byte() {
        let text =
            "{ \"a\" : \"caf\\u00e9 \\/ \\\"quoted\\\"\",\n  \"b\": [1, 2, {\"c\": null}] }\n";
        assert!(matches!(
            redact_document(text, &PatternRedactor::new()),
            Some(Cow::Borrowed(_))
        ));
    }

    #[test]
    fn numbers_arrays_and_literals_under_a_credential_name() {
        let text = r#"{"pin_token":123456,"tokens":[1,2],"secret":["a","b"],"password":null,"token":true}"#;
        assert_eq!(
            redact(text).unwrap(),
            r#"{"pin_token":"[REDACTED]","tokens":[1,2],"secret":["[REDACTED]","[REDACTED]"],"password":null,"token":true}"#
        );
    }

    #[test]
    fn secrets_inside_strings_are_redacted_after_unescaping() {
        let text =
            r#"{"message":"curl -H \"Authorization: Bearer abcdefghijklmnop\" x","ok":true}"#;
        assert_eq!(
            redact(text).unwrap(),
            r#"{"message":"curl -H \"Authorization: [REDACTED]","ok":true}"#
        );
        // A quote inside the secret does not end the redaction early.
        let cookie = r#"{"line":"Cookie: sid=\"abc def\"; theme=dark"}"#;
        assert_eq!(redact(cookie).unwrap(), r#"{"line":"Cookie: [REDACTED]"}"#);
    }

    #[test]
    fn json_nested_in_a_string_is_redacted_as_json() {
        let text = r#"{"payload":"{\"password\":\"hunter2\",\"n\":1}"}"#;
        assert_eq!(
            redact(text).unwrap(),
            r#"{"payload":"{\"password\":\"[REDACTED]\",\"n\":1}"}"#
        );
    }

    #[test]
    fn name_value_pairs_use_the_name_member() {
        let text =
            r#"[{"name":"DB_PASSWORD","value":"hunter2"},{"name":"PATH","value":"/usr/bin"}]"#;
        assert_eq!(
            redact(text).unwrap(),
            r#"[{"name":"DB_PASSWORD","value":"[REDACTED]"},{"name":"PATH","value":"/usr/bin"}]"#
        );
    }

    #[test]
    fn name_value_pairs_with_numbers_and_in_either_case() {
        let text = r#"{"Name":"PIN_TOKEN","Value":1234,"other":5}"#;
        assert_eq!(
            redact(text).unwrap(),
            r#"{"Name":"PIN_TOKEN","Value":"[REDACTED]","other":5}"#
        );
    }

    #[test]
    fn the_element_after_a_bare_credential_flag_is_its_value() {
        let text = r#"{"args":["mysql","--password","tr0ub4dor","--host","db","-token",42,"--password"],"n":"x"}"#;
        assert_eq!(
            redact(text).unwrap(),
            r#"{"args":["mysql","--password","[REDACTED]","--host","db","-token","[REDACTED]","--password"],"n":"x"}"#
        );
        // A flag is followed by its value only: a nested container or a literal resets it.
        let text = r#"["--password",null,"plain",["--token"],"plain"]"#;
        assert!(matches!(
            redact_document(text, &PatternRedactor::new()),
            Some(Cow::Borrowed(_))
        ));
    }

    #[test]
    fn everything_inside_a_credential_container_is_redacted() {
        let text = r#"{"credentials":{"user":"ada","pass":"hunter2","port":5432,"tls":true,"extra":[{"k":"v"}]},"auth":{"value":"abc"},"user":"ada"}"#;
        assert_eq!(
            redact(text).unwrap(),
            r#"{"credentials":{"user":"[REDACTED]","pass":"[REDACTED]","port":"[REDACTED]","tls":true,"extra":[{"k":"[REDACTED]"}]},"auth":{"value":"[REDACTED]"},"user":"ada"}"#
        );
    }

    #[test]
    fn hostile_nesting_is_cheap_and_bounded() {
        // A long member name above a deep stack of arrays: the name is shared, never copied per level.
        let name = "k".repeat(200_000);
        let deep = format!("{{\"{name}\":{}", "[".repeat(MAX_DEPTH + 50));
        assert!(redact(&deep).is_none());
        // Within the depth limit the document is walked; unbalanced, it is still rejected.
        let shallow = format!("{{\"{name}\":{}", "[".repeat(MAX_DEPTH - 2));
        assert!(redact(&shallow).is_none());
        // A long name above a very wide array: one shared name for 200 000 elements.
        let wide = format!("{{\"{name}\":[{}1]}}", "1,".repeat(200_000));
        assert!(matches!(
            redact_document(&wide, &PatternRedactor::new()),
            Some(Cow::Borrowed(_))
        ));
        // The plain-text fallback handles what the walk refuses.
        let fallback = format!("{}{{\"password\":\"hunter2\"}}", "[".repeat(MAX_DEPTH + 1));
        let shown = PatternRedactor::new().redact(&fallback).into_owned();
        assert!(!shown.contains("hunter2"), "{}", &shown[shown.len() - 40..]);
    }

    #[test]
    fn a_pair_is_recognized_whichever_member_comes_first() {
        // Serializers that sort member names put `value` before `variable`.
        let text = r#"[{"value":"hunter2","variable":"DB_PASSWORD"},{"val":7,"key":"API_TOKEN","x":1},{"value":"/usr/bin","name":"PATH"}]"#;
        assert_eq!(
            redact(text).unwrap(),
            r#"[{"value":"[REDACTED]","variable":"DB_PASSWORD"},{"val":"[REDACTED]","key":"API_TOKEN","x":1},{"value":"/usr/bin","name":"PATH"}]"#
        );
        // A value that was already partly redacted as text is replaced, not redacted twice.
        let text = r#"{"value":"password=abc and more","name":"Cookie","n":1}"#;
        assert_eq!(
            redact(text).unwrap(),
            r#"{"value":"[REDACTED]","name":"Cookie","n":1}"#
        );
        // The held value belongs to its own object only.
        let text = r#"{"value":"keep","inner":{"name":"Cookie"},"list":[{"name":"Cookie"}]}"#;
        assert!(matches!(
            redact_document(text, &PatternRedactor::new()),
            Some(Cow::Borrowed(_))
        ));
    }

    #[test]
    fn a_flag_with_its_value_attached_does_not_claim_the_next_element() {
        let text = r#"["mysql","--password=hunter2","--host","db"]"#;
        assert_eq!(
            redact(text).unwrap(),
            r#"["mysql","--password=[REDACTED]","--host","db"]"#
        );
    }

    #[test]
    fn a_lone_surrogate_does_not_stop_the_walk() {
        let text =
            r#"{"note":"\ud83d ok","password":"hunter2","tail":"\udc00","pair":"\ud83d\u0041"}"#;
        assert_eq!(
            redact(text).unwrap(),
            r#"{"note":"\ud83d ok","password":"[REDACTED]","tail":"\udc00","pair":"\ud83d\u0041"}"#
        );
        assert_eq!(parse_string(r#""a\ud83db""#, 0).unwrap().0, "a\u{fffd}b");
    }

    #[test]
    fn a_secret_used_as_a_member_name_is_redacted() {
        let key = ["gh", "p_", "0123456789abcdefghijABCDEFGHIJ012345"].concat();
        let text = format!(r#"{{"{key}":1}}"#);
        assert_eq!(redact(&text).unwrap(), r#"{"[REDACTED]":1}"#);
    }

    #[test]
    fn json_lines_and_control_characters() {
        let text = "{\"password\":\"a\"}\n{\"note\":\"tab\\there\",\"secret\":\"line\\nbreak\"}\n";
        assert_eq!(
            redact(text).unwrap(),
            "{\"password\":\"[REDACTED]\"}\n{\"note\":\"tab\\there\",\"secret\":\"[REDACTED]\"}\n"
        );
        let mut out = String::new();
        push_json_string(&mut out, "a\u{1}\"\\\n\r\t");
        assert_eq!(out, "\"a\\u0001\\\"\\\\\\n\\r\\t\"");
    }

    #[test]
    fn surrogate_pairs_are_decoded() {
        let text = r#"{"password":"😀"}"#;
        assert_eq!(redact(text).unwrap(), r#"{"password":"[REDACTED]"}"#);
        assert_eq!(parse_string(r#""😀""#, 0).unwrap().0, "\u{1F600}");
    }

    #[test]
    fn text_that_is_not_json_is_rejected() {
        for text in [
            r#"{"password":"hunter2""#,
            r#"{"password":"hunter2"]"#,
            r#"{"a":bare}"#,
            r#"{"a":"\x"}"#,
            r#"{"a":"\u12"}"#,
            "{\"a\":\"raw\u{1}control\"}",
            "[REDACTED]",
            r#"{"a"::1}"#,
            "]",
        ] {
            assert!(redact(text).is_none(), "{text} should not parse");
        }
    }
}
