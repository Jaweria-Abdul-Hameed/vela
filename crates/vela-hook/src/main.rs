//! The `vela-hook` PreToolUse hook helper (A04, ADR-018).
//!
//! Antigravity runs this program before every tool call, writes one JSON object to its standard input, and reads one JSON
//! object from its standard output. The hook classifies the call with the same policy engine the core uses (from a generated
//! rule table), records a secret-free line in the spool, and answers `allow`, `ask` or `deny`.
//!
//! It fails closed on everything: empty or malformed input, an unreadable or invalid rule table, a spool that cannot be
//! written for an `allow`, a timeout, and a panic all produce `deny`. It depends on nothing but `serde`, `serde_json` and the
//! policy types in `vela-domain` (no graph, no SQLite, no Tauri, no async runtime).
//!
//! Usage: `vela-hook --table <rule-table.json> --spool <dir>` (or `VELA_HOOK_TABLE` and `VELA_HOOK_SPOOL`).

mod decide;
mod input;
mod spool;

use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use decide::{Verdict, VerdictKind};

/// The longest standard input accepted. A larger one is not a tool call.
const MAX_INPUT_BYTES: u64 = 1024 * 1024;
/// How long the hook waits for the input before it denies (the provider's own hook timeout is larger).
const INPUT_TIMEOUT: Duration = Duration::from_secs(8);

struct Config {
    table: PathBuf,
    spool: PathBuf,
}

fn parse_args(args: &[String], env: impl Fn(&str) -> Option<String>) -> Result<Config, String> {
    let mut table = env("VELA_HOOK_TABLE").map(PathBuf::from);
    let mut spool = env("VELA_HOOK_SPOOL").map(PathBuf::from);
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--table" => table = iter.next().map(PathBuf::from),
            "--spool" => spool = iter.next().map(PathBuf::from),
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    match (table, spool) {
        (Some(table), Some(spool)) => Ok(Config { table, spool }),
        _ => Err(
            "both --table and --spool (or VELA_HOOK_TABLE and VELA_HOOK_SPOOL) are required"
                .to_owned(),
        ),
    }
}

fn read_stdin_with_timeout() -> Result<String, String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let result = std::io::stdin()
            .take(MAX_INPUT_BYTES + 1)
            .read_to_string(&mut text)
            .map(|_| text);
        let _ = tx.send(result);
    });
    match rx.recv_timeout(INPUT_TIMEOUT) {
        Ok(Ok(text)) if text.len() as u64 > MAX_INPUT_BYTES => {
            Err("the input is too large".to_owned())
        }
        Ok(Ok(text)) => Ok(text),
        Ok(Err(e)) => Err(format!("could not read the input: {e}")),
        Err(_) => Err("timed out waiting for the input".to_owned()),
    }
}

fn run() -> Verdict {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let config = match parse_args(&args, |k| std::env::var(k).ok()) {
        Ok(c) => c,
        Err(why) => return Verdict::deny("vela.hook.config", &why),
    };
    let text = match read_stdin_with_timeout() {
        Ok(t) => t,
        Err(why) => return Verdict::deny("vela.hook.input", &why),
    };
    decide::run(&text, &config.table, &config.spool)
}

fn main() {
    // Any failure of any kind is a denial printed in the provider's protocol; the exit code stays 0 so the answer is read.
    let verdict = std::panic::catch_unwind(run)
        .unwrap_or_else(|_| Verdict::deny("vela.hook.panic", "the hook failed unexpectedly"));
    println!("{}", verdict.to_json());
    debug_assert!(matches!(
        verdict.kind,
        VerdictKind::Allow | VerdictKind::Ask | VerdictKind::Deny
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn arguments_and_environment_name_the_table_and_the_spool() {
        let args = ["--table", "t.json", "--spool", "s"].map(String::from);
        let c = parse_args(&args, no_env).unwrap();
        assert_eq!(
            (c.table, c.spool),
            (PathBuf::from("t.json"), PathBuf::from("s"))
        );
        let env = |k: &str| match k {
            "VELA_HOOK_TABLE" => Some("e.json".to_owned()),
            "VELA_HOOK_SPOOL" => Some("es".to_owned()),
            _ => None,
        };
        let c = parse_args(&[], env).unwrap();
        assert_eq!(c.table, PathBuf::from("e.json"));
        let overridden = parse_args(&["--table".to_owned(), "x.json".to_owned()], env).unwrap();
        assert_eq!(overridden.table, PathBuf::from("x.json"));
    }

    #[test]
    fn missing_or_unknown_arguments_are_errors() {
        assert!(parse_args(&[], no_env).is_err());
        assert!(parse_args(&["--table".to_owned(), "t".to_owned()], no_env).is_err());
        assert!(parse_args(&["--frobnicate".to_owned()], no_env).is_err());
        assert!(parse_args(&["--table".to_owned()], no_env).is_err());
    }
}
