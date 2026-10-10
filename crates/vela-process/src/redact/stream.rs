//! [`StreamRedactor`]: redaction of output that arrives in arbitrary chunks (process stdout/stderr, log writers).
//!
//! A secret can be split across two reads, so chunks are never redacted on their own. Output is held until its line is
//! complete and each line is redacted whole. The only state kept across lines is whether a private key block is open.
//!
//! Input is split at line feeds and decoded as UTF-8. Output in another encoding (UTF-16 from some Windows tools) must
//! be transcoded by the caller first; undecoded, its text matches no pattern.

use super::{REDACTED, SharedRedactor};

/// The longest line that is redacted whole. A longer line is cut here and the rest of it is dropped.
pub const DEFAULT_MAX_LINE_BYTES: usize = 1024 * 1024;

/// Written where the rest of an over-long line was dropped.
pub const TRUNCATED: &str = "[TRUNCATED]";

const PEM_BEGIN: &str = "-----BEGIN ";
const PEM_END: &str = "-----END ";
const PEM_DASHES: &str = "-----";
const PRIVATE_KEY: &str = "PRIVATE KEY";

/// The most lines swallowed after a private key's BEGIN line when no END line arrives (a 16384-bit RSA key is about
/// 200 lines). The bound keeps a stray BEGIN line from silencing the rest of the stream.
const MAX_KEY_LINES: usize = 256;

/// Redacts a stream line by line.
///
/// Feed chunks with [`push`](Self::push) or [`push_bytes`](Self::push_bytes) and write what they return; call
/// [`finish`](Self::finish) when the stream ends. Line breaks are preserved, so the output has exactly the line
/// structure of the input. Nothing is returned for an incomplete line until it is completed or the stream finishes.
pub struct StreamRedactor {
    redactor: SharedRedactor,
    /// The current, still incomplete line. Never longer than `max_line_bytes`.
    pending: Vec<u8>,
    max_line_bytes: usize,
    /// The current line was cut at `max_line_bytes`; its remaining bytes are dropped.
    truncating: bool,
    /// How many more lines are swallowed as the body of a private key whose BEGIN line was seen. Zero when no key is
    /// open.
    key_lines_left: usize,
}

impl StreamRedactor {
    /// A stream redactor with the default line limit.
    pub fn new(redactor: SharedRedactor) -> Self {
        Self {
            redactor,
            pending: Vec::new(),
            max_line_bytes: DEFAULT_MAX_LINE_BYTES,
            truncating: false,
            key_lines_left: 0,
        }
    }

    /// Sets the longest line that is redacted whole (at least one byte).
    #[must_use]
    pub fn with_max_line_bytes(mut self, max_line_bytes: usize) -> Self {
        self.max_line_bytes = max_line_bytes.max(1);
        self
    }

    /// Feeds text; returns the redacted form of every line it completed.
    pub fn push(&mut self, chunk: &str) -> String {
        self.push_bytes(chunk.as_bytes())
    }

    /// Feeds raw output; returns the redacted form of every line it completed. Bytes that are not UTF-8 become the
    /// replacement character, and a character split across two chunks is decoded once it is complete.
    pub fn push_bytes(&mut self, mut chunk: &[u8]) -> String {
        let mut out = String::new();
        while !chunk.is_empty() {
            let newline = chunk.iter().position(|byte| *byte == b'\n');
            let piece = &chunk[..newline.unwrap_or(chunk.len())];
            chunk = &chunk[piece.len() + usize::from(newline.is_some())..];

            if !self.truncating {
                let room = self.max_line_bytes - self.pending.len();
                if piece.len() > room {
                    self.pending.extend_from_slice(&piece[..room]);
                    // The word the cut falls in is dropped with the rest of the line: half a token or half a URL
                    // is not recognizable as a secret, and must not be written out for that reason.
                    let keep = self
                        .pending
                        .iter()
                        .rposition(u8::is_ascii_whitespace)
                        .map_or(0, |blank| blank + 1);
                    self.pending.truncate(keep);
                    self.flush_line(&mut out);
                    out.push_str(TRUNCATED);
                    self.truncating = true;
                } else {
                    self.pending.extend_from_slice(piece);
                }
            }
            if newline.is_some() {
                if !self.truncating {
                    self.flush_line(&mut out);
                }
                self.truncating = false;
                out.push('\n');
            }
        }
        out
    }

    /// Releases the current, still incomplete line (a prompt that waits for input has no line break). What follows
    /// continues the same line. Use it only when the stream has gone quiet: a secret that is still arriving is
    /// redacted in two halves, and neither half may be recognizable.
    pub fn flush_partial(&mut self) -> String {
        let mut out = String::new();
        if !self.pending.is_empty() && !self.truncating {
            self.flush_line(&mut out);
        }
        out
    }

    /// Ends the stream; returns the redacted form of the last, unterminated line.
    pub fn finish(&mut self) -> String {
        let out = self.flush_partial();
        self.truncating = false;
        self.key_lines_left = 0;
        out
    }

    fn flush_line(&mut self, out: &mut String) {
        let mut raw = std::mem::take(&mut self.pending);
        self.redact_line(&String::from_utf8_lossy(&raw), out);
        raw.clear();
        self.pending = raw;
    }

    fn redact_line(&mut self, line: &str, out: &mut String) {
        let mut line = line;
        if self.key_lines_left > 0 {
            match after_key_end(line) {
                // The key ends on this line; whatever follows it is ordinary output.
                Some(after) => {
                    self.key_lines_left = 0;
                    line = after;
                }
                // Every line of an open key is swallowed whatever it looks like: key lines arrive prefixed by
                // `git diff`, `grep` and log collectors, and one unrecognized line must not release the rest.
                None => {
                    self.key_lines_left -= 1;
                    return;
                }
            }
        }
        out.push_str(&self.redactor.redact(line));
        if opens_private_key(line) {
            // The redactor replaced the key from its `BEGIN` line to the end of this line; make sure of it even for a
            // redactor that does not know private keys, then swallow the lines that follow.
            if !out.ends_with(REDACTED) {
                out.push_str(REDACTED);
            }
            self.key_lines_left = MAX_KEY_LINES;
        }
    }
}

/// Whether `line` ends with the `BEGIN` marker of a private key, so the key itself is on the lines that follow.
///
/// A key that is written on one line (inside a JSON string, with escaped line breaks) continues after the marker and
/// is redacted with its line; it opens nothing.
fn opens_private_key(line: &str) -> bool {
    let Some(begin) = line.rfind(PEM_BEGIN) else {
        return false;
    };
    let label = &line[begin + PEM_BEGIN.len()..];
    let Some(label_end) = label.find(PEM_DASHES) else {
        return false;
    };
    label[..label_end].contains(PRIVATE_KEY)
        && label[label_end + PEM_DASHES.len()..].trim().is_empty()
}

/// The text after the `-----END ... -----` marker of `line`, if it has one.
fn after_key_end(line: &str) -> Option<&str> {
    let label = &line[line.find(PEM_END)? + PEM_END.len()..];
    Some(
        label
            .find(PEM_DASHES)
            .map_or("", |close| &label[close + PEM_DASHES.len()..]),
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::redact::PatternRedactor;
    use crate::redact::corpus::corpus;

    fn stream() -> StreamRedactor {
        StreamRedactor::new(Arc::new(PatternRedactor::new()))
    }

    /// Feeds `text` in chunks of `size` bytes and returns everything the stream produced.
    fn feed(stream: &mut StreamRedactor, text: &[u8], size: usize) -> String {
        let mut out = String::new();
        for chunk in text.chunks(size) {
            out.push_str(&stream.push_bytes(chunk));
        }
        out.push_str(&stream.finish());
        out
    }

    #[test]
    fn the_corpus_is_redacted_however_the_output_is_chunked() {
        for case in corpus() {
            let whole = feed(&mut stream(), case.text.as_bytes(), usize::MAX);
            for size in [1, 2, 3, 5, 7, 16, 64] {
                let shown = feed(&mut stream(), case.text.as_bytes(), size);
                for secret in &case.secrets {
                    assert!(
                        !shown.contains(secret.as_str()),
                        "{} in chunks of {size}: `{secret}` survived in `{shown}`",
                        case.name
                    );
                }
                assert_eq!(shown, whole, "{} in chunks of {size}", case.name);
            }
        }
    }

    #[test]
    fn lines_are_released_only_when_complete() {
        let mut stream = stream();
        assert_eq!(stream.push("building\nDB_PASS"), "building\n");
        assert_eq!(stream.push("WORD=hun"), "");
        assert_eq!(stream.push("ter2\r\nnext"), "DB_PASSWORD=[REDACTED]\r\n");
        assert_eq!(stream.finish(), "next");
        assert_eq!(stream.finish(), "");
    }

    #[test]
    fn line_structure_is_preserved() {
        let text = "one\n\ntoken: abc\nlast";
        assert_eq!(
            feed(&mut stream(), text.as_bytes(), 4),
            "one\n\ntoken: [REDACTED]\nlast"
        );
    }

    #[test]
    fn a_private_key_spread_over_many_lines_is_swallowed() {
        let text = "start\n-----BEGIN RSA PRIVATE KEY-----\nProc-Type: 4,ENCRYPTED\n\nMIIEow+/AB==\nIBAAKCAQ\n-----END RSA PRIVATE KEY----- tail\nDB_PASSWORD=x\n";
        for size in [1, 9, 1000] {
            assert_eq!(
                feed(&mut stream(), text.as_bytes(), size),
                "start\n[REDACTED]\n\n\n\n\n tail\nDB_PASSWORD=[REDACTED]\n",
                "chunks of {size}"
            );
        }
    }

    #[test]
    fn prefixed_key_lines_are_swallowed_until_the_end_marker() {
        // `git diff` of a removed key, and `grep -n` output: no line is plain base64.
        let diff = "------BEGIN RSA PRIVATE KEY-----\n-MIIEow\n-IBAAKCAQ\n------END RSA PRIVATE KEY-----\n+ok\n";
        assert_eq!(
            feed(&mut stream(), diff.as_bytes(), 7),
            "-[REDACTED]\n\n\n\n+ok\n"
        );
        let grep = "id_rsa:1:-----BEGIN OPENSSH PRIVATE KEY-----\nid_rsa:2:b3BlbnNz\nid_rsa:3:-----END OPENSSH PRIVATE KEY-----\n";
        assert_eq!(
            feed(&mut stream(), grep.as_bytes(), 5),
            "id_rsa:1:[REDACTED]\n\n\n"
        );
    }

    #[test]
    fn an_unclosed_private_key_swallows_a_bounded_number_of_lines() {
        let mut text = String::from("-----BEGIN PRIVATE KEY-----\n");
        for line in 0..MAX_KEY_LINES + 2 {
            text.push_str(&format!("line {line}\n"));
        }
        let shown = feed(&mut stream(), text.as_bytes(), 64);
        let expected = format!(
            "[REDACTED]\n{}line {}\nline {}\n",
            "\n".repeat(MAX_KEY_LINES),
            MAX_KEY_LINES,
            MAX_KEY_LINES + 1
        );
        assert_eq!(shown, expected);
    }

    #[test]
    fn a_partial_line_can_be_released_on_request() {
        let mut stream = stream();
        assert_eq!(stream.push("Continue? [y/N] "), "");
        assert_eq!(stream.flush_partial(), "Continue? [y/N] ");
        assert_eq!(stream.flush_partial(), "");
        assert_eq!(
            stream.push("y\nDB_PASSWORD=x\n"),
            "y\nDB_PASSWORD=[REDACTED]\n"
        );
    }

    #[test]
    fn a_private_key_inside_one_json_line_does_not_swallow_the_next_lines() {
        let text = "{\"text\":\"-----BEGIN RSA PRIVATE KEY-----\\nMIIE\\nAAAA\",\"n\":1}\nMIIE\n";
        assert_eq!(
            feed(&mut stream(), text.as_bytes(), 8),
            "{\"text\":\"[REDACTED]\",\"n\":1}\nMIIE\n"
        );
    }

    #[test]
    fn an_over_long_line_is_cut_and_the_rest_dropped() {
        let mut stream = stream().with_max_line_bytes(16);
        let text = "0123456789 token=abcdefghijklmnopqrstuvwxyz\nnext token=abc\n";
        assert_eq!(
            feed(&mut stream, text.as_bytes(), 5),
            "0123456789 [TRUNCATED]\nnext token=[REDACTED]\n"
        );
        // Neither the tail of a value nor the head of an unfinished one is exposed by the cut.
        let mut stream = self::stream().with_max_line_bytes(12);
        assert_eq!(
            feed(&mut stream, b"password=abcdefghijklmnop\n", 3),
            "[TRUNCATED]\n"
        );
        let mut stream = self::stream().with_max_line_bytes(24);
        assert_eq!(
            feed(
                &mut stream,
                b"see https://ada:s3cr3t@host/path and more\n",
                4
            ),
            "see [TRUNCATED]\n"
        );
    }

    #[test]
    fn invalid_and_split_utf8() {
        let mut stream = stream();
        let text = "é password=sécret✓ ok\n".as_bytes();
        assert_eq!(feed(&mut stream, text, 1), "é password=[REDACTED] ok\n");
        assert_eq!(
            feed(&mut stream, b"bad \xff byte\n", 2),
            "bad \u{fffd} byte\n"
        );
    }
}
