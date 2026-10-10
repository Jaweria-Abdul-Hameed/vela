//! Structure-preserving redaction of serialized JSON (journal payloads, JSON log records, stream-json output).
//!
//! Redacting JSON as plain text would either corrupt the document or stop at a quote that belongs to the secret. This
//! module walks the document instead: every string is unescaped, redacted as free text (or as a named value when it is
//! an object member) and written back escaped, so the output is still JSON with the same shape.

use std::borrow::Cow;
use std::fmt::Write as _;

use super::names::is_credential_name;
use super::{REDACTED, Redactor};

/// Members that name the credential held by a sibling `value` member (`{"name": "DB_PASSWORD", "value": "..."}`).
const PAIR_NAMES: [&str; 4] = ["name", "key", "header", "variable"];
const PAIR_VALUES: [&str; 2] = ["value", "val"];

enum Frame {
    /// `credential_pair` is set once a [`PAIR_NAMES`] member of this object held a credential name.
    Object { credential_pair: bool },
    /// The member name the array is stored under; its elements are values of that name.
    Array { key: Option<String> },
}

/// Redacts every string and number of `text`, which must be one or more well-formed JSON containers.
///
/// Returns `None` when `text` is not JSON (truncated, or not JSON at all); the caller then redacts it as plain text.
pub(super) fn redact_document<'a>(text: &'a str, redactor: &dyn Redactor) -> Option<Cow<'a, str>> {
    let bytes = text.as_bytes();
    let mut out = String::new();
    let mut copied = 0;
    let mut stack: Vec<Frame> = Vec::new();
    // The name of the member whose value comes next.
    let mut key: Option<String> = None;
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b' ' | b'\t' | b'\n' | b'\r' | b',' => index += 1,
            b'{' => {
                key = None;
                stack.push(Frame::Object {
                    credential_pair: false,
                });
                index += 1;
            }
            b'[' => {
                let key = key.take().or_else(|| inherited_key(&stack));
                stack.push(Frame::Array { key });
                index += 1;
            }
            b'}' => {
                if !matches!(stack.pop(), Some(Frame::Object { .. })) {
                    return None;
                }
                key = None;
                index += 1;
            }
            b']' => {
                if !matches!(stack.pop(), Some(Frame::Array { .. })) {
                    return None;
                }
                index += 1;
            }
            b'"' => {
                let (content, end) = parse_string(text, index)?;
                let after = end + blank_len(&bytes[end..]);
                let is_key = key.is_none()
                    && matches!(stack.last(), Some(Frame::Object { .. }))
                    && bytes.get(after) == Some(&b':');
                let replacement = if is_key {
                    changed(redactor.redact(&content), &content)
                } else {
                    let name = key.take().or_else(|| inherited_key(&stack));
                    changed(
                        leaf(redactor, stack.last_mut(), name.as_deref(), &content),
                        &content,
                    )
                };
                if let Some(replacement) = replacement {
                    out.push_str(&text[copied..index]);
                    push_json_string(&mut out, &replacement);
                    copied = end;
                }
                if is_key {
                    key = Some(content.into_owned());
                    index = after + 1;
                } else {
                    index = end;
                }
            }
            _ => {
                let end = index + scalar_len(&bytes[index..]);
                let token = text.get(index..end)?;
                let name = key.take().or_else(|| inherited_key(&stack));
                if is_number(token) {
                    // A redacted number becomes a string so the document stays valid.
                    let shown = match &name {
                        Some(name) => redactor.redact_value(name, token),
                        None => redactor.redact(token),
                    };
                    if shown != token {
                        out.push_str(&text[copied..index]);
                        push_json_string(&mut out, &shown);
                        copied = end;
                    }
                } else if !matches!(token, "true" | "false" | "null") {
                    return None;
                }
                index = end;
            }
        }
    }

    if !stack.is_empty() {
        return None;
    }
    if copied == 0 {
        return Some(Cow::Borrowed(text));
    }
    out.push_str(&text[copied..]);
    Some(Cow::Owned(out))
}

fn changed(shown: Cow<'_, str>, original: &str) -> Option<String> {
    (shown != original).then(|| shown.into_owned())
}

fn inherited_key(stack: &[Frame]) -> Option<String> {
    match stack.last() {
        Some(Frame::Array { key }) => key.clone(),
        _ => None,
    }
}

fn leaf<'c>(
    redactor: &dyn Redactor,
    frame: Option<&mut Frame>,
    name: Option<&str>,
    content: &'c str,
) -> Cow<'c, str> {
    if let (Some(Frame::Object { credential_pair }), Some(name)) = (frame, name) {
        if PAIR_NAMES
            .iter()
            .any(|pair| name.eq_ignore_ascii_case(pair))
        {
            *credential_pair = is_credential_name(content);
        } else if *credential_pair
            && !content.is_empty()
            && PAIR_VALUES
                .iter()
                .any(|pair| name.eq_ignore_ascii_case(pair))
        {
            return Cow::Borrowed(REDACTED);
        }
    }
    match name {
        Some(name) => redactor.redact_value(name, content),
        None => redactor.redact(content),
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
/// index past it. A lone surrogate is not text, so the document is treated as unparseable.
fn unicode_escape(bytes: &[u8], index: usize) -> Option<(char, usize)> {
    let first = hex4(bytes, index)?;
    if !(0xD800..0xE000).contains(&first) {
        return Some((char::from_u32(first)?, index + 4));
    }
    if first >= 0xDC00 || bytes.get(index + 4..index + 6) != Some(b"\\u".as_slice()) {
        return None;
    }
    let second = hex4(bytes, index + 6)?;
    if !(0xDC00..0xE000).contains(&second) {
        return None;
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
            r#"{"a":"\ud83d"}"#,
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
