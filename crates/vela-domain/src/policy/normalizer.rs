//! Normalizer for one simple command in PowerShell or `cmd` syntax.
//!
//! The grammar is deliberately tiny (`COMPONENT_SPECIFICATIONS.md` C03): a bare executable word followed by arguments that
//! are bare words or fully quoted strings. Anything richer is **rejected** rather than interpreted, and a rejection is an
//! `ASK` upstream. Rejected: pipes, redirection, chaining, command separators, subexpressions and script blocks, variable
//! and environment expansion, escapes, line continuation, comments, wildcards, splatting, home expansion, control and
//! invisible characters, typographic quotes and dashes, and a quote that starts or ends in the middle of a word.
//!
//! PowerShell and `cmd` disagree on what a quote means and which characters are special. When the dialect is not known the
//! text must parse identically under both; otherwise it is [`Rejection::Ambiguous`].

use std::fmt;

/// Longest command text, in bytes, the normalizer accepts.
pub const MAX_COMMAND_LEN: usize = 8192;
/// Most tokens (executable included) the normalizer accepts.
pub const MAX_TOKENS: usize = 256;

/// The syntax a command text is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dialect {
    /// Windows PowerShell or PowerShell 7 (the Antigravity agent's shell).
    PowerShell,
    /// `cmd.exe`.
    Cmd,
    /// Unknown: the text must be unambiguous under both.
    Unspecified,
}

/// Why a command text is not a single simple command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rejection {
    /// Empty or only whitespace.
    Empty,
    /// Longer than [`MAX_COMMAND_LEN`] or more than [`MAX_TOKENS`] tokens.
    TooLong,
    /// A control, newline, invisible, typographic or other non-word character outside the permitted set.
    UnsupportedCharacter,
    /// `|`.
    Pipeline,
    /// `<` or `>`.
    Redirection,
    /// `&`, `;`, or `,` separating commands or values.
    Chaining,
    /// `(`, `)`, `{`, `}`, `[`, `]`: subexpression, script block, grouping, or type literal.
    Subexpression,
    /// `$`, `%`, `!`: variable, environment or delayed expansion.
    Expansion,
    /// Backtick or caret escape.
    Escape,
    /// `#` comment or `@` splat, array or echo-off.
    ScriptSyntax,
    /// `*` or `?`.
    Wildcard,
    /// A quote opened and not closed, or content that cannot be taken literally.
    BadQuoting,
    /// A quote begins or ends in the middle of a word.
    QuoteInsideWord,
    /// The executable is quoted or is a dot-source, call or label form.
    NonBareExecutable,
    /// The text parses differently, or only partly, under PowerShell and `cmd`.
    Ambiguous,
}

impl Rejection {
    /// A fixed, argument-free description for decisions and logs.
    pub fn describe(self) -> &'static str {
        match self {
            Rejection::Empty => "empty command",
            Rejection::TooLong => "command is too long",
            Rejection::UnsupportedCharacter => {
                "contains a control, invisible or non-word character"
            }
            Rejection::Pipeline => "contains a pipeline",
            Rejection::Redirection => "contains redirection",
            Rejection::Chaining => "contains command chaining or separators",
            Rejection::Subexpression => "contains a subexpression, script block or grouping",
            Rejection::Expansion => "contains variable or environment expansion",
            Rejection::Escape => "contains an escape character",
            Rejection::ScriptSyntax => "contains a comment, splat or script syntax",
            Rejection::Wildcard => "contains a wildcard",
            Rejection::BadQuoting => "has unbalanced or non-literal quoting",
            Rejection::QuoteInsideWord => "has a quote inside a word",
            Rejection::NonBareExecutable => "executable is not a bare word",
            Rejection::Ambiguous => "parses differently in PowerShell and cmd",
        }
    }
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.describe())
    }
}

/// A single simple command: an executable and its literal arguments.
///
/// The `Debug` form shows the executable and the argument count only; arguments can carry secrets.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct NormalizedCommand {
    executable: String,
    args: Vec<String>,
}

impl fmt::Debug for NormalizedCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NormalizedCommand({} +{} args)",
            self.executable,
            self.args.len()
        )
    }
}

impl NormalizedCommand {
    /// The executable: ASCII-lowercased, with a trailing `.exe` removed when it is a bare name (no path separator).
    pub fn executable(&self) -> &str {
        &self.executable
    }

    /// The literal arguments, quotes removed, original case.
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// True when the executable names a path rather than a bare command.
    pub fn is_path_qualified(&self) -> bool {
        self.executable.contains(['\\', '/'])
    }

    /// The executable name used for table lookups: basename, without a script or executable extension.
    pub(crate) fn lookup_name(&self) -> String {
        // Windows drops trailing dots and spaces from a file name, so `format.` runs `format`.
        let base = self.executable.rsplit(['\\', '/']).next().unwrap_or("");
        let base = base.trim_end_matches(['.', ' ']);
        for ext in [".exe", ".com", ".cmd", ".bat"] {
            if let Some(stem) = base.strip_suffix(ext) {
                return stem.to_string();
            }
        }
        base.to_string()
    }

    /// A canonical one-line form for display, profile storage and native rules. Arguments that are not plain words
    /// (whitespace, shell-significant characters, or none at all) are double-quoted; the grammar guarantees no argument
    /// contains a double quote, so the form re-parses to the same command under `cmd` or PowerShell (`cmd` splits a
    /// first word at a slash, PowerShell does not). A command that neither reproduces is refused when it is stored (see
    /// `ProfileEntry::parse`).
    pub fn canonical_text(&self) -> String {
        let mut out = self.executable.clone();
        for arg in &self.args {
            out.push(' ');
            let plain = !arg.is_empty()
                && arg
                    .chars()
                    .all(|c| c.is_alphanumeric() || "-_./\\:=+~".contains(c));
            if plain {
                out.push_str(arg);
            } else {
                out.push('"');
                out.push_str(arg);
                out.push('"');
            }
        }
        out
    }

    pub(crate) fn from_parts(executable: String, args: Vec<String>) -> Self {
        NormalizedCommand { executable, args }
    }
}

/// Normalizes `text` under `dialect`.
pub fn normalize(text: &str, dialect: Dialect) -> Result<NormalizedCommand, Rejection> {
    if text.len() > MAX_COMMAND_LEN {
        return Err(Rejection::TooLong);
    }
    match dialect {
        Dialect::PowerShell => parse(text, Syntax::PowerShell),
        Dialect::Cmd => parse(text, Syntax::Cmd),
        Dialect::Unspecified => match (parse(text, Syntax::PowerShell), parse(text, Syntax::Cmd)) {
            (Ok(a), Ok(b)) if a == b => Ok(a),
            (Ok(_), Ok(_)) | (Ok(_), Err(_)) | (Err(_), Ok(_)) => Err(Rejection::Ambiguous),
            (Err(e), Err(_)) => Err(e),
        },
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Syntax {
    PowerShell,
    Cmd,
}

/// Characters allowed in a word beyond ASCII alphanumerics and non-ASCII alphanumerics.
const WORD_PUNCTUATION: &str = "-_./\\:=+~'";

fn parse(text: &str, syntax: Syntax) -> Result<NormalizedCommand, Rejection> {
    if text.trim().is_empty() {
        return Err(Rejection::Empty);
    }
    // Character screening first, outside and inside quotes alike, so a quoted payload cannot smuggle control or look-alike
    // characters.
    for c in text.chars() {
        if c == ' ' || c == '\t' {
            continue;
        }
        if c.is_control() {
            return Err(Rejection::UnsupportedCharacter);
        }
        if !c.is_ascii() && !c.is_alphanumeric() {
            return Err(Rejection::UnsupportedCharacter);
        }
    }

    let mut tokens: Vec<Token> = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == ' ' || c == '\t' {
            i += 1;
            continue;
        }
        if tokens.len() >= MAX_TOKENS {
            return Err(Rejection::TooLong);
        }
        let quote = match (c, syntax) {
            ('"', _) => Some('"'),
            ('\'', Syntax::PowerShell) => Some('\''),
            _ => None,
        };
        if let Some(q) = quote {
            let start = i + 1;
            let mut end = start;
            while end < chars.len() && chars[end] != q {
                end += 1;
            }
            if end >= chars.len() {
                return Err(Rejection::BadQuoting);
            }
            let content: String = chars[start..end].iter().collect();
            check_quoted(&content, q, syntax)?;
            // The closing quote must end the word.
            if let Some(&next) = chars.get(end + 1)
                && next != ' '
                && next != '\t'
            {
                return Err(Rejection::QuoteInsideWord);
            }
            tokens.push(Token {
                text: content,
                quoted: true,
            });
            i = end + 1;
        } else {
            let start = i;
            while i < chars.len() && chars[i] != ' ' && chars[i] != '\t' {
                let ch = chars[i];
                if ch == '"' || (ch == '\'' && syntax == Syntax::PowerShell) {
                    return Err(Rejection::QuoteInsideWord);
                }
                check_bare(ch, syntax)?;
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            tokens.push(Token {
                text: word,
                quoted: false,
            });
        }
    }

    let mut iter = tokens.into_iter();
    let first = iter.next().ok_or(Rejection::Empty)?;
    if first.quoted
        || first.text.starts_with(['-', '/', ':'])
        || first.text == "."
        || first.text == ".."
        || first.text.starts_with("--")
    {
        return Err(Rejection::NonBareExecutable);
    }
    // An assignment (`FOO=bar cmd`) is not a command word in either syntax.
    if first.text.contains('=') {
        return Err(Rejection::NonBareExecutable);
    }
    let mut glued: Vec<String> = Vec::new();
    let mut head = first.text.as_str();
    if syntax == Syntax::Cmd
        && let Some((name, switches)) = head.split_once('/')
        && !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        // In `cmd` a slash ends a command name: `rd/s/q` is `rd /s /q`.
        glued = switches.split('/').map(|s| format!("/{s}")).collect();
        head = name;
    }
    let executable = normalize_executable(head);
    let args: Vec<String> = glued.into_iter().chain(iter.map(|t| t.text)).collect();
    Ok(NormalizedCommand::from_parts(executable, args))
}

struct Token {
    text: String,
    quoted: bool,
}

/// Maps a bare character that is special in the syntax to the reason it is rejected.
fn check_bare(c: char, syntax: Syntax) -> Result<(), Rejection> {
    match c {
        '|' => Err(Rejection::Pipeline),
        '<' | '>' => Err(Rejection::Redirection),
        '&' | ';' | ',' => Err(Rejection::Chaining),
        '(' | ')' | '{' | '}' | '[' | ']' => match syntax {
            Syntax::PowerShell => Err(Rejection::Subexpression),
            // `)` and `(` are grouping in cmd; braces and brackets are plain characters there.
            Syntax::Cmd if c == '(' || c == ')' => Err(Rejection::Subexpression),
            Syntax::Cmd => Ok(()),
        },
        '%' | '!' => Err(Rejection::Expansion),
        '$' if syntax == Syntax::PowerShell => Err(Rejection::Expansion),
        '`' => Err(Rejection::Escape),
        '^' => Err(Rejection::Escape),
        '#' if syntax == Syntax::PowerShell => Err(Rejection::ScriptSyntax),
        '@' => Err(Rejection::ScriptSyntax),
        '*' | '?' => Err(Rejection::Wildcard),
        c if c.is_ascii_alphanumeric() || !c.is_ascii() => Ok(()),
        c if WORD_PUNCTUATION.contains(c) => Ok(()),
        // `$` and `#` in cmd, and any other ASCII punctuation, are not part of the supported word grammar.
        '$' | '#' => Ok(()),
        _ => Err(Rejection::UnsupportedCharacter),
    }
}

/// A quoted string must be taken literally by both the shell and a native program.
fn check_quoted(content: &str, quote: char, syntax: Syntax) -> Result<(), Rejection> {
    // A trailing backslash before the closing quote reads as an escaped quote to a native program's argument parser, which
    // would merge this argument with the next.
    if content.ends_with('\\') {
        return Err(Rejection::BadQuoting);
    }
    for c in content.chars() {
        match c {
            '"' | '\'' => return Err(Rejection::BadQuoting),
            '`' | '^' => return Err(Rejection::Escape),
            // Expansion happens inside double quotes in PowerShell (`$`) and cmd (`%`, `!`), and inside cmd quotes `%`.
            '$' if quote == '"' && syntax == Syntax::PowerShell => {
                return Err(Rejection::Expansion);
            }
            '%' | '!' if syntax == Syntax::Cmd || quote == '"' => return Err(Rejection::Expansion),
            _ => {}
        }
    }
    Ok(())
}

fn normalize_executable(word: &str) -> String {
    let lower = word.to_lowercase();
    if !lower.contains(['\\', '/'])
        && let Some(stem) = lower.strip_suffix(".exe")
        && !stem.is_empty()
    {
        return stem.to_string();
    }
    lower
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(text: &str, dialect: Dialect) -> NormalizedCommand {
        normalize(text, dialect).unwrap_or_else(|e| panic!("{text:?} rejected: {e}"))
    }

    #[test]
    fn parses_simple_commands_in_every_dialect() {
        for dialect in [Dialect::PowerShell, Dialect::Cmd, Dialect::Unspecified] {
            let c = ok("  Git.EXE   status --short  ", dialect);
            assert_eq!(c.executable(), "git");
            assert_eq!(c.args(), ["status", "--short"]);
            assert_eq!(c.canonical_text(), "git status --short");
        }
        let c = ok(r#"npm run "my script" -- --flag=1"#, Dialect::Unspecified);
        assert_eq!(c.args(), ["run", "my script", "--", "--flag=1"]);
        assert_eq!(c.canonical_text(), r#"npm run "my script" -- --flag=1"#);
    }

    #[test]
    fn path_qualified_executables_keep_their_path() {
        let c = ok(r".\Tools\Build.exe go", Dialect::PowerShell);
        assert_eq!(c.executable(), r".\tools\build.exe");
        assert!(c.is_path_qualified());
        assert_eq!(c.lookup_name(), "build");
    }

    #[test]
    fn single_quotes_quote_only_in_powershell() {
        let c = ok("echo 'a b'", Dialect::PowerShell);
        assert_eq!(c.args(), ["a b"]);
        let c = ok("echo 'a b'", Dialect::Cmd);
        assert_eq!(c.args(), ["'a", "b'"]);
        assert_eq!(
            normalize("echo 'a b'", Dialect::Unspecified),
            Err(Rejection::Ambiguous)
        );
        assert_eq!(
            normalize("echo 'a'", Dialect::Unspecified),
            Err(Rejection::Ambiguous)
        );
    }

    #[test]
    fn rejects_each_metacharacter_class_with_its_reason() {
        let cases = [
            ("npm test | tee x", Rejection::Pipeline),
            ("npm test > out.txt", Rejection::Redirection),
            ("npm test < in.txt", Rejection::Redirection),
            ("npm test & calc", Rejection::Chaining),
            ("npm test ; calc", Rejection::Chaining),
            ("npm test && calc", Rejection::Chaining),
            ("npm test || calc", Rejection::Pipeline),
            ("echo a,b", Rejection::Chaining),
            ("echo $(whoami)", Rejection::Expansion),
            ("echo (1)", Rejection::Subexpression),
            ("echo {x}", Rejection::Subexpression),
            ("echo [x]", Rejection::Subexpression),
            ("echo $env:TEMP", Rejection::Expansion),
            ("echo %TEMP%", Rejection::Expansion),
            ("echo !x", Rejection::Expansion),
            ("echo a`b", Rejection::Escape),
            ("echo a^b", Rejection::Escape),
            ("echo a # c", Rejection::ScriptSyntax),
            ("echo @args", Rejection::ScriptSyntax),
            ("del *.txt", Rejection::Wildcard),
            ("dir ?", Rejection::Wildcard),
            ("echo \"open", Rejection::BadQuoting),
            ("echo a\"b\"", Rejection::QuoteInsideWord),
            ("echo \"a\"b", Rejection::QuoteInsideWord),
            ("echo 'a'b", Rejection::QuoteInsideWord),
            ("echo \"a$b\"", Rejection::Expansion),
            ("echo \"a`b\"", Rejection::Escape),
            ("echo 'a\"b'", Rejection::BadQuoting),
            ("echo \"%x%\"", Rejection::Expansion),
            ("", Rejection::Empty),
            ("   ", Rejection::Empty),
        ];
        for (text, reason) in cases {
            assert_eq!(
                normalize(text, Dialect::PowerShell),
                Err(reason),
                "powershell: {text:?}"
            );
        }
    }

    #[test]
    fn rejects_control_invisible_and_lookalike_characters() {
        for text in [
            "npm test\nrm -rf x",
            "npm test\r",
            "npm\u{0}test",
            "npm test \u{2018}x\u{2019}",
            "npm test \u{201c}x\u{201d}",
            "npm test \u{2013}flag",
            "npm test \u{202e}",
            "npm test\u{200b}",
            "npm\u{a0}test",
            "npm test \u{ff1b} calc",
        ] {
            for d in [Dialect::PowerShell, Dialect::Cmd, Dialect::Unspecified] {
                assert_eq!(
                    normalize(text, d),
                    Err(Rejection::UnsupportedCharacter),
                    "{d:?} {text:?}"
                );
            }
        }
        assert!(normalize("echo ünï", Dialect::PowerShell).is_ok());
    }

    #[test]
    fn executable_must_be_a_bare_word() {
        for text in [
            "\"git\" status",
            "'git' status",
            "& git status",
            ". git",
            "-git x",
            "/c dir",
            ":label",
        ] {
            assert!(normalize(text, Dialect::PowerShell).is_err(), "{text:?}");
        }
        assert_eq!(
            normalize("\"git\" status", Dialect::Cmd),
            Err(Rejection::NonBareExecutable)
        );
    }

    #[test]
    fn cmd_and_powershell_differ_on_dollar_and_braces() {
        assert_eq!(
            normalize("echo $x", Dialect::PowerShell),
            Err(Rejection::Expansion)
        );
        assert!(normalize("echo $x", Dialect::Cmd).is_ok());
        assert_eq!(
            normalize("echo $x", Dialect::Unspecified),
            Err(Rejection::Ambiguous)
        );
        assert_eq!(
            normalize("echo {x}", Dialect::Unspecified),
            Err(Rejection::Ambiguous)
        );
        assert_eq!(
            normalize("echo a|b", Dialect::Unspecified),
            Err(Rejection::Pipeline)
        );
    }

    #[test]
    fn length_and_token_limits_apply() {
        let long = format!("echo {}", "a".repeat(MAX_COMMAND_LEN));
        assert_eq!(
            normalize(&long, Dialect::PowerShell),
            Err(Rejection::TooLong)
        );
        let many = format!("echo {}", "a ".repeat(MAX_TOKENS + 1));
        assert_eq!(
            normalize(&many, Dialect::PowerShell),
            Err(Rejection::TooLong)
        );
    }

    #[test]
    fn debug_output_never_shows_arguments() {
        let c = ok("tool --token sk-live-SECRET", Dialect::PowerShell);
        let shown = format!("{c:?}");
        assert!(
            !shown.contains("SECRET") && !shown.contains("sk-live"),
            "{shown}"
        );
    }
}
