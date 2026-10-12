//! The hardened Git invocation (G01, ADR-013, `EXTERNAL_VERIFICATION_2026-10-02.md` P10).
//!
//! A repository can carry configuration that makes Git run programs: `core.fsmonitor`, `diff.external`, textconv,
//! clean/smudge/process filters, and hooks. Vela never lets that happen during a read, and lets hooks run only for the
//! writes the repository's owner trusts (commit and merge in a trusted repository). The arguments below neutralize every
//! known execution path; the per-driver overrides are built from the driver names the repository's own configuration
//! declares, which are discovered with a config-only (`git config --list`) call that executes nothing.

use std::collections::BTreeSet;

use vela_domain::ports::RepoTrust;

/// The null device as Git spells it on this platform.
pub const fn null_device() -> &'static str {
    if cfg!(windows) { "NUL" } else { "/dev/null" }
}

/// What an invocation is allowed to do beyond reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Inspection only: no hooks, no filters, no external diff, no optional locks.
    Read,
    /// A write the repository owner may have hooks for (commit, merge). Hooks run only when the repository is trusted.
    Write,
    /// Talks to a remote: like [`Mode::Read`] for local execution paths, plus no ssh command override and no prompts.
    Remote,
}

/// Names of the filter and diff drivers a repository's configuration declares.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeclaredDrivers {
    /// `filter.<name>.*` names.
    pub filters: BTreeSet<String>,
    /// `diff.<name>.*` names (textconv, command).
    pub diffs: BTreeSet<String>,
}

/// Reads driver names out of `git config --list --name-only` output (one key per line). Keys are case-insensitive for the
/// section and variable and keep their case in the middle part; a driver name may itself contain dots.
pub fn parse_declared_drivers(config_names: &str) -> DeclaredDrivers {
    let mut drivers = DeclaredDrivers::default();
    for key in config_names
        .lines()
        .map(str::trim)
        .filter(|k| !k.is_empty())
    {
        let Some((section, rest)) = key.split_once('.') else {
            continue;
        };
        let Some((name, variable)) = rest.rsplit_once('.') else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        match (
            section.to_ascii_lowercase().as_str(),
            variable.to_ascii_lowercase().as_str(),
        ) {
            ("filter", "clean" | "smudge" | "process" | "required") => {
                drivers.filters.insert(name.to_owned());
            }
            ("diff", "textconv" | "command" | "cachetextconv" | "binary") => {
                drivers.diffs.insert(name.to_owned());
            }
            _ => {}
        }
    }
    drivers
}

/// The `-c` arguments for one invocation, in order. Every one is a single `-c key=value` pair.
pub fn hardened_config(mode: Mode, trust: RepoTrust, drivers: &DeclaredDrivers) -> Vec<String> {
    let mut settings: Vec<String> = vec![
        "core.fsmonitor=false".to_owned(),
        "core.pager=cat".to_owned(),
        "core.editor=true".to_owned(),
        "core.longpaths=true".to_owned(),
        "core.autocrlf=false".to_owned(),
        "core.safecrlf=false".to_owned(),
        "diff.external=".to_owned(),
        "protocol.ext.allow=never".to_owned(),
        "protocol.file.allow=user".to_owned(),
        "commit.gpgsign=false".to_owned(),
        "tag.gpgsign=false".to_owned(),
        "gc.auto=0".to_owned(),
        "maintenance.auto=false".to_owned(),
    ];
    // Hooks run only for a write in a repository the user has trusted.
    let hooks_allowed = mode == Mode::Write && trust == RepoTrust::Trusted;
    if !hooks_allowed {
        settings.push(format!("core.hooksPath={}", null_device()));
    }
    if mode == Mode::Remote {
        // A repository must not choose the program that carries its traffic.
        settings.push("core.sshCommand=ssh".to_owned());
    }
    // Reads and remote operations never run clean/smudge filters; a commit or merge in a trusted repository may need the
    // repository's own filters for content it stores through them.
    let filters_allowed = mode == Mode::Write && trust == RepoTrust::Trusted;
    if !filters_allowed {
        for name in &drivers.filters {
            for variable in ["clean", "smudge", "process"] {
                settings.push(format!("filter.{name}.{variable}="));
            }
            settings.push(format!("filter.{name}.required=false"));
        }
    }
    for name in &drivers.diffs {
        settings.push(format!("diff.{name}.textconv="));
        settings.push(format!("diff.{name}.command="));
    }
    settings
}

/// The full leading arguments: `--no-pager` and the `-c` pairs.
pub fn hardened_args(mode: Mode, trust: RepoTrust, drivers: &DeclaredDrivers) -> Vec<String> {
    let mut args = vec!["--no-pager".to_owned()];
    for setting in hardened_config(mode, trust, drivers) {
        args.push("-c".to_owned());
        args.push(setting);
    }
    args
}

/// Extra flags for any command that can produce a patch or run a diff driver.
pub const NO_DIFF_DRIVERS: [&str; 2] = ["--no-ext-diff", "--no-textconv"];

/// Environment variables added to the allowlist for every Git invocation.
pub fn hardened_env(mode: Mode) -> Vec<(String, String)> {
    let mut env = vec![
        ("GIT_TERMINAL_PROMPT".to_owned(), "0".to_owned()),
        ("GCM_INTERACTIVE".to_owned(), "never".to_owned()),
        ("GIT_PAGER".to_owned(), "cat".to_owned()),
        ("GIT_EDITOR".to_owned(), "true".to_owned()),
        ("GIT_ASKPASS".to_owned(), String::new()),
        ("LC_ALL".to_owned(), "C".to_owned()),
        ("LANG".to_owned(), "C".to_owned()),
    ];
    if mode == Mode::Read {
        // A read must not take the index lock (it would race a running writer).
        env.push(("GIT_OPTIONAL_LOCKS".to_owned(), "0".to_owned()));
    }
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(args: &[String]) -> Vec<&str> {
        args.windows(2)
            .filter(|w| w[0] == "-c")
            .map(|w| w[1].as_str())
            .collect()
    }

    #[test]
    fn config_keys_are_parsed_into_driver_names_including_dotted_names() {
        let d = parse_declared_drivers(
            "core.fsmonitor\nfilter.hostile.clean\nfilter.hostile.smudge\nfilter.my.lfs.process\nfilter.my.lfs.required\ndiff.hostile.textconv\ndiff.x.command\nuser.name\nremote.origin.url\nfilter.broken\n",
        );
        assert_eq!(
            d.filters.iter().map(String::as_str).collect::<Vec<_>>(),
            ["hostile", "my.lfs"]
        );
        assert_eq!(
            d.diffs.iter().map(String::as_str).collect::<Vec<_>>(),
            ["hostile", "x"]
        );
    }

    #[test]
    fn reads_and_untrusted_writes_never_allow_hooks_or_filters() {
        let drivers = parse_declared_drivers("filter.hostile.clean\ndiff.hostile.textconv\n");
        for (mode, trust) in [
            (Mode::Read, RepoTrust::Trusted),
            (Mode::Read, RepoTrust::Untrusted),
            (Mode::Write, RepoTrust::Untrusted),
            (Mode::Remote, RepoTrust::Trusted),
        ] {
            let args = hardened_args(mode, trust, &drivers);
            let p = pairs(&args);
            assert!(p.contains(&"core.fsmonitor=false"), "{mode:?} {trust:?}");
            assert!(
                p.iter().any(|s| s.starts_with("core.hooksPath=")),
                "{mode:?} {trust:?}"
            );
            assert!(p.contains(&"filter.hostile.clean="), "{mode:?} {trust:?}");
            assert!(p.contains(&"diff.hostile.textconv="));
            assert!(p.contains(&"diff.external="));
        }
    }

    #[test]
    fn a_trusted_write_keeps_the_repositorys_hooks_and_filters_but_never_its_diff_programs() {
        let drivers = parse_declared_drivers("filter.lfs.clean\ndiff.hostile.textconv\n");
        let args = hardened_args(Mode::Write, RepoTrust::Trusted, &drivers);
        let p = pairs(&args);
        assert!(!p.iter().any(|s| s.starts_with("core.hooksPath=")));
        assert!(!p.contains(&"filter.lfs.clean="));
        assert!(p.contains(&"diff.hostile.textconv="));
        assert!(p.contains(&"core.fsmonitor=false"));
    }

    #[test]
    fn every_setting_is_a_dash_c_pair_and_the_pager_is_off() {
        let args = hardened_args(
            Mode::Read,
            RepoTrust::Untrusted,
            &DeclaredDrivers::default(),
        );
        assert_eq!(args[0], "--no-pager");
        assert_eq!((args.len() - 1) % 2, 0);
        assert!(
            args[1..]
                .chunks(2)
                .all(|c| c[0] == "-c" && c[1].contains('='))
        );
    }

    #[test]
    fn the_environment_blocks_prompts_and_reads_avoid_the_index_lock() {
        let read = hardened_env(Mode::Read);
        assert!(
            read.iter()
                .any(|(k, v)| k == "GIT_TERMINAL_PROMPT" && v == "0")
        );
        assert!(
            read.iter()
                .any(|(k, v)| k == "GIT_OPTIONAL_LOCKS" && v == "0")
        );
        assert!(
            !hardened_env(Mode::Write)
                .iter()
                .any(|(k, _)| k == "GIT_OPTIONAL_LOCKS")
        );
    }
}
