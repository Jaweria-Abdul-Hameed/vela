//! Parsing of one local Markdown ticket (`ISSUE_AUTHORING.md`; the graph tickets use the same section layout).
//!
//! The parser is pure: it takes text, never touches the file system, and never fails. Everything in the text is
//! untrusted (ADR-013): it is read as data, it is never used as a path, a command or a policy, and problems are
//! reported as typed [`TicketWarning`]s instead of being repaired silently or panicking.

use std::fmt;

use vela_domain::ids::TicketId;
use vela_domain::ports::TrackerTicket;

/// Longest title kept; longer ones are truncated and flagged.
pub const MAX_TITLE_CHARS: usize = 200;
/// Longest fragment of untrusted text quoted inside a warning.
const MAX_QUOTE_CHARS: usize = 64;

/// What is wrong with a ticket file. Typed so callers can react without parsing messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WarningKind {
    /// The file is not valid UTF-8 and was skipped.
    NotUtf8,
    /// The file is larger than the size limit and was skipped.
    TooLarge {
        /// The limit in bytes.
        max_bytes: u64,
    },
    /// The file could not be read.
    Unreadable,
    /// No usable title heading; the ticket id is used as the title.
    MissingTitle,
    /// The title was longer than [`MAX_TITLE_CHARS`] and was truncated.
    TitleTruncated,
    /// No valid ticket id could be determined; the file is not a ticket.
    InvalidTicketId {
        /// The offending text (shortened, control characters replaced).
        found: String,
    },
    /// The id given by the symbolic key and the id in the title heading differ; the symbolic key wins.
    IdMismatch {
        /// The title heading's id.
        title_id: String,
        /// The symbolic key's id.
        key_id: String,
    },
    /// The file has no "Dependencies / blocked by" section; it is treated as having no blockers.
    MissingBlockersSection,
    /// The blockers section has text that is neither a bullet nor "None".
    UnrecognisedBlockersText,
    /// A blocker entry does not start with a valid ticket id and was ignored.
    InvalidBlocker {
        /// The offending text (shortened, control characters replaced).
        found: String,
    },
    /// A ticket lists itself as a blocker; the entry was ignored.
    SelfBlocker,
    /// The same blocker is listed twice; the duplicate was ignored.
    DuplicateBlocker {
        /// The repeated blocker.
        blocker: String,
    },
    /// Another file already defined this ticket id; this file was ignored.
    DuplicateTicketId {
        /// The repeated id.
        id: String,
    },
    /// A blocker names a ticket that is not in the corpus; no edge was created for it.
    UnknownBlocker {
        /// The unknown blocker.
        blocker: String,
    },
}

/// A problem found in one ticket file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TicketWarning {
    /// The file name (not a path) the warning is about.
    pub source: String,
    /// What is wrong.
    pub kind: WarningKind,
}

impl TicketWarning {
    pub(crate) fn new(source: &str, kind: WarningKind) -> Self {
        Self {
            source: source.to_owned(),
            kind,
        }
    }
}

impl fmt::Display for TicketWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let source = &self.source;
        match &self.kind {
            WarningKind::NotUtf8 => write!(f, "{source}: not valid UTF-8, skipped"),
            WarningKind::TooLarge { max_bytes } => {
                write!(f, "{source}: larger than {max_bytes} bytes, skipped")
            }
            WarningKind::Unreadable => write!(f, "{source}: could not be read, skipped"),
            WarningKind::MissingTitle => write!(f, "{source}: no title heading"),
            WarningKind::TitleTruncated => write!(f, "{source}: title truncated"),
            WarningKind::InvalidTicketId { found } => {
                write!(f, "{source}: no valid ticket id (found `{found}`)")
            }
            WarningKind::IdMismatch { title_id, key_id } => write!(
                f,
                "{source}: title id `{title_id}` differs from symbolic key `{key_id}`"
            ),
            WarningKind::MissingBlockersSection => {
                write!(f, "{source}: no dependencies / blocked by section")
            }
            WarningKind::UnrecognisedBlockersText => {
                write!(f, "{source}: unrecognised text in the blockers section")
            }
            WarningKind::InvalidBlocker { found } => {
                write!(f, "{source}: invalid blocker `{found}` ignored")
            }
            WarningKind::SelfBlocker => write!(f, "{source}: ticket lists itself as a blocker"),
            WarningKind::DuplicateBlocker { blocker } => {
                write!(f, "{source}: blocker `{blocker}` listed twice")
            }
            WarningKind::DuplicateTicketId { id } => {
                write!(f, "{source}: ticket id `{id}` already defined, ignored")
            }
            WarningKind::UnknownBlocker { blocker } => {
                write!(f, "{source}: blocker `{blocker}` is not in the corpus")
            }
        }
    }
}

/// The result of parsing one file: a ticket when an id could be determined, and every problem found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTicket {
    /// The ticket, absent when the file has no valid id.
    pub ticket: Option<TrackerTicket>,
    /// Problems found, in file order.
    pub warnings: Vec<TicketWarning>,
}

/// Shortens untrusted text for quoting in a warning and replaces control characters.
pub(crate) fn quote(text: &str) -> String {
    text.chars()
        .take(MAX_QUOTE_CHARS)
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

/// Each line with whether it is structure (true) or inside a fenced code block (false), so headings in examples are
/// not mistaken for sections.
fn structural_lines(text: &str) -> Vec<(&str, bool)> {
    let mut in_fence = false;
    let mut lines = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            lines.push((line, false));
        } else {
            lines.push((line, !in_fence));
        }
    }
    lines
}

/// `Some((level, text))` for a level-1 or level-2 heading.
fn heading(line: &str) -> Option<(usize, &str)> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 2 {
        return None;
    }
    let rest = line[hashes..].strip_prefix(' ')?;
    Some((hashes, rest.trim()))
}

/// Removes a leading section number such as `7. `.
fn strip_section_number(text: &str) -> &str {
    let digits = text.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0
        && let Some(rest) = text[digits..].strip_prefix('.')
    {
        return rest.trim_start();
    }
    text
}

/// Backtick-quoted spans on a line.
fn backticked(line: &str) -> Vec<&str> {
    line.split('`')
        .enumerate()
        .filter(|(index, part)| index % 2 == 1 && !part.trim().is_empty())
        .map(|(_, part)| part.trim())
        .collect()
}

/// The value of a `**Name:** value` line (an optional leading `>` is allowed).
fn label_value<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let trimmed = line.trim_start().trim_start_matches(['>', ' ']);
    let rest = trimmed.strip_prefix("**")?;
    let (found, value) = rest.split_once(":**")?;
    found.eq_ignore_ascii_case(name).then_some(value)
}

/// The id in a `> Symbolic key: `ID`` line.
fn symbolic_key(line: &str) -> Option<&str> {
    let trimmed = line.trim_start().trim_start_matches(['>', ' ']);
    let rest = trimmed.strip_prefix("Symbolic key:")?;
    backticked(rest).into_iter().next()
}

/// Blocker entries are the first word of each bullet; `None` states that there are none.
fn parse_blockers(
    body: &[&str],
    own_id: &TicketId,
    source: &str,
    warnings: &mut Vec<TicketWarning>,
) -> Vec<TicketId> {
    let mut blockers: Vec<TicketId> = Vec::new();
    let mut unrecognised = false;
    for line in body {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some(item) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        else {
            if !trimmed.to_ascii_lowercase().starts_with("none") {
                unrecognised = true;
            }
            continue;
        };
        let token = item
            .trim_start_matches(['`', '['])
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_end_matches(['`', ']', ':', ',', ';', '.']);
        if token.eq_ignore_ascii_case("none") {
            continue;
        }
        match TicketId::new(token) {
            Err(_) => warnings.push(TicketWarning::new(
                source,
                WarningKind::InvalidBlocker { found: quote(item) },
            )),
            Ok(id) if &id == own_id => {
                warnings.push(TicketWarning::new(source, WarningKind::SelfBlocker));
            }
            Ok(id) if blockers.contains(&id) => warnings.push(TicketWarning::new(
                source,
                WarningKind::DuplicateBlocker {
                    blocker: id.to_string(),
                },
            )),
            Ok(id) => blockers.push(id),
        }
    }
    if unrecognised {
        warnings.push(TicketWarning::new(
            source,
            WarningKind::UnrecognisedBlockersText,
        ));
    }
    blockers
}

/// Parses one ticket file. `file_name` is the fallback for the id and the name quoted in warnings; it is never
/// used to open anything.
pub fn parse_ticket(file_name: &str, text: &str) -> ParsedTicket {
    let source = quote(file_name);
    let mut warnings = Vec::new();
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);

    let mut raw_title: Option<&str> = None;
    let mut key: Option<&str> = None;
    let mut labels: Vec<String> = Vec::new();
    let mut blockers_body: Option<Vec<&str>> = None;
    let mut collecting = false;

    for (line, structural) in structural_lines(text) {
        if structural && let Some((level, title)) = heading(line) {
            collecting = false;
            if level == 1 {
                raw_title = raw_title.or(Some(title));
            } else if blockers_body.is_none()
                && strip_section_number(title).eq_ignore_ascii_case("dependencies / blocked by")
            {
                collecting = true;
                blockers_body = Some(Vec::new());
            }
            continue;
        }
        if collecting {
            if let Some(body) = blockers_body.as_mut() {
                body.push(line);
            }
            continue;
        }
        if !structural {
            continue;
        }
        if key.is_none() {
            key = symbolic_key(line);
        }
        if labels.is_empty()
            && let Some(value) =
                label_value(line, "Planned labels").or_else(|| label_value(line, "Labels"))
        {
            labels = backticked(value).into_iter().map(quote).collect();
        }
    }

    // Title and id: `# ID: Title` (graph tickets) or `# Title` (template); the symbolic key is authoritative.
    let (title_id, title_text) = match raw_title {
        Some(title) => match title.split_once(": ") {
            Some((head, rest))
                if head.chars().any(|c| c.is_ascii_digit()) && TicketId::new(head).is_ok() =>
            {
                (Some(head), rest.trim())
            }
            _ => (None, title),
        },
        None => (None, ""),
    };
    let stem = file_name
        .rsplit_once('.')
        .map_or(file_name, |(stem, _)| stem);
    if let (Some(key), Some(title_id)) = (key, title_id)
        && key != title_id
    {
        warnings.push(TicketWarning::new(
            &source,
            WarningKind::IdMismatch {
                title_id: quote(title_id),
                key_id: quote(key),
            },
        ));
    }
    let chosen = key.or(title_id).unwrap_or(stem);
    let Ok(id) = TicketId::new(chosen) else {
        warnings.push(TicketWarning::new(
            &source,
            WarningKind::InvalidTicketId {
                found: quote(chosen),
            },
        ));
        return ParsedTicket {
            ticket: None,
            warnings,
        };
    };

    let title = if title_text.is_empty() {
        warnings.push(TicketWarning::new(&source, WarningKind::MissingTitle));
        id.to_string()
    } else if title_text.chars().count() > MAX_TITLE_CHARS {
        warnings.push(TicketWarning::new(&source, WarningKind::TitleTruncated));
        title_text.chars().take(MAX_TITLE_CHARS).collect()
    } else {
        title_text.to_owned()
    };

    let blockers = match blockers_body {
        Some(body) => parse_blockers(&body, &id, &source, &mut warnings),
        None => {
            warnings.push(TicketWarning::new(
                &source,
                WarningKind::MissingBlockersSection,
            ));
            Vec::new()
        }
    };

    ParsedTicket {
        ticket: Some(TrackerTicket {
            id,
            title,
            body: text.to_owned(),
            blockers,
            labels,
        }),
        warnings,
    }
}

#[cfg(test)]
mod tests;
