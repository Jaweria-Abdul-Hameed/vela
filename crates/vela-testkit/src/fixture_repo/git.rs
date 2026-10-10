//! Deterministic, neutralised `git` invocation for building and inspecting fixture repositories.

use super::FixtureError;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Unix time of the first fixture commit: 2026-01-01T00:00:00Z.
pub const BASE_EPOCH: i64 = 1_767_225_600;

/// Seconds between consecutive fixture commits.
pub const COMMIT_SPACING_SECONDS: i64 = 60;

/// Drivers the hostile-config fixture defines (clean/smudge filters and a diff textconv with this name).
pub const HOSTILE_DRIVER: &str = "hostile";

/// The null device, as Git spells it on this platform.
pub const fn null_device() -> &'static str {
    if cfg!(windows) { "NUL" } else { "/dev/null" }
}

/// The verified P10 neutralising configuration (`EXTERNAL_VERIFICATION_2026-10-02.md`), as `git` arguments.
///
/// It disables the fsmonitor hook, points the hooks path at the null device, forces the `cat` pager, empties the
/// external diff program, and empties the clean/smudge/textconv commands of the hostile fixture's driver. This is the
/// test kit's own copy used to inspect fixtures safely; the production hardened invocation belongs to `vela-git`.
///
/// It is not sufficient on its own for diff-producing commands: those must also pass `--no-ext-diff --no-textconv`
/// (an empty `diff.external` makes Git try to spawn an empty program and fail, which CI demonstrated).
pub fn neutralizing_args() -> Vec<String> {
    let mut args: Vec<String> = vec!["--no-pager".to_owned()];
    for setting in [
        "core.fsmonitor=false".to_owned(),
        format!("core.hooksPath={}", null_device()),
        "core.pager=cat".to_owned(),
        "diff.external=".to_owned(),
        format!("filter.{HOSTILE_DRIVER}.clean="),
        format!("filter.{HOSTILE_DRIVER}.smudge="),
        format!("diff.{HOSTILE_DRIVER}.textconv="),
    ] {
        args.push("-c".to_owned());
        args.push(setting);
    }
    args
}

/// A Git repository directory plus the configuration needed to drive it reproducibly.
#[derive(Debug, Clone)]
pub(crate) struct GitRepo {
    /// The working tree.
    pub dir: PathBuf,
    /// An empty file used as the global Git configuration, so the host's own configuration never leaks in.
    pub empty_global_config: PathBuf,
    /// Number of commits made so far; the next commit is dated `BASE_EPOCH + seq * COMMIT_SPACING_SECONDS`.
    pub seq: u32,
}

impl GitRepo {
    pub fn new(dir: PathBuf, empty_global_config: PathBuf) -> Self {
        Self {
            dir,
            empty_global_config,
            seq: 0,
        }
    }

    fn command(&self, neutralize: bool) -> Command {
        let mut command = Command::new("git");
        command.arg("-C").arg(&self.dir);
        if neutralize {
            command.args(neutralizing_args());
        }
        command.args([
            "-c",
            "core.autocrlf=false",
            "-c",
            "core.safecrlf=false",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "tag.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
            "-c",
            "core.quotepath=false",
            "-c",
            format!("core.attributesFile={}", null_device()).as_str(),
            "-c",
            format!("core.excludesFile={}", null_device()).as_str(),
        ]);
        for var in [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
            "GIT_OBJECT_DIRECTORY",
            "GIT_COMMON_DIR",
            "GIT_NAMESPACE",
            "GIT_PREFIX",
            "GIT_TEMPLATE_DIR",
            "GIT_EXTERNAL_DIFF",
            "GIT_CONFIG",
            "GIT_CONFIG_PARAMETERS",
            "GIT_CONFIG_COUNT",
            "GIT_ATTR_SOURCE",
            "GIT_CEILING_DIRECTORIES",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            "GIT_PAGER",
            "GIT_DEFAULT_HASH",
            "GIT_DEFAULT_REF_FORMAT",
            "GIT_EDITOR",
            "GIT_SEQUENCE_EDITOR",
            "GIT_ASKPASS",
            "GIT_SSH_COMMAND",
        ] {
            command.env_remove(var);
        }
        let date = format!(
            "{} +0000",
            BASE_EPOCH + i64::from(self.seq) * COMMIT_SPACING_SECONDS
        );
        // Nothing from the host's home may leak in: no global attributes or ignore file, no XDG configuration.
        let isolated_home = self
            .empty_global_config
            .parent()
            .unwrap_or(&self.empty_global_config);
        command
            .env("HOME", isolated_home)
            .env("USERPROFILE", isolated_home)
            .env("XDG_CONFIG_HOME", isolated_home)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_ATTR_NOSYSTEM", "1")
            .env("LC_ALL", "C")
            .env("GIT_CONFIG_GLOBAL", &self.empty_global_config)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_AUTHOR_NAME", "Vela Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@vela.invalid")
            .env("GIT_COMMITTER_NAME", "Vela Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@vela.invalid")
            .env("GIT_AUTHOR_DATE", &date)
            .env("GIT_COMMITTER_DATE", &date);
        command
    }

    /// Runs `git` with the neutralising configuration and returns the raw output (a non-zero exit is not an error).
    pub fn output(&self, args: &[&str]) -> Result<Output, FixtureError> {
        let mut command = self.command(true);
        // Diff-producing subcommands also need `--no-ext-diff --no-textconv` (an empty `diff.external` alone makes Git
        // try to spawn an empty program); they go right after the subcommand so a trailing `--` pathspec still works.
        match args.split_first() {
            Some((sub, rest)) if matches!(*sub, "diff" | "log" | "show") => {
                command.arg(sub).args(["--no-ext-diff", "--no-textconv"]).args(rest);
            }
            _ => {
                command.args(args);
            }
        }
        command
            .output()
            .map_err(|source| FixtureError::io(format!("spawn git {}", args.join(" ")), source))
    }

    /// Runs `git` with only the determinism configuration, NOT the neutralising set. Used by the hostile fixture's
    /// control test to prove the repository really fires its payloads; never use it on an untrusted repository
    /// outside a test that expects the payloads to run.
    #[cfg(test)]
    pub fn output_unneutralized(&self, args: &[&str]) -> Result<Output, FixtureError> {
        let mut command = self.command(false);
        command.args(args);
        command
            .output()
            .map_err(|source| FixtureError::io(format!("spawn git {}", args.join(" ")), source))
    }

    /// Runs `git` (neutralised) and returns stdout with trailing whitespace removed; a non-zero exit is an error.
    pub fn run(&self, args: &[&str]) -> Result<String, FixtureError> {
        let output = self.output(args)?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout)
                .trim_end()
                .to_owned())
        } else {
            Err(FixtureError::Git {
                command: args.join(" "),
                exit_code: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            })
        }
    }

    /// Writes a UTF-8 file at `relative` (forward slashes), creating parent directories. Content is written as given
    /// (LF only), so the bytes do not depend on the host.
    pub fn write(&self, relative: &str, content: &str) -> Result<(), FixtureError> {
        let path = self.dir.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| {
                FixtureError::io(format!("create {}", parent.display()), source)
            })?;
        }
        std::fs::write(&path, content.as_bytes())
            .map_err(|source| FixtureError::io(format!("write {}", path.display()), source))
    }

    /// Stages everything and commits it, advancing the deterministic clock after the commit.
    pub fn commit_all(&mut self, message: &str) -> Result<(), FixtureError> {
        self.run(&["add", "-A"])?;
        self.run(&["commit", "--no-verify", "--allow-empty", "-m", message])?;
        self.seq += 1;
        Ok(())
    }
}

/// Where a test may put the empty global configuration for `dir`: a sibling file.
pub(crate) fn empty_config_path(root: &Path) -> PathBuf {
    root.join("gitconfig-empty")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_git_environment_is_isolated_from_the_host() {
        let repo = GitRepo::new(PathBuf::from("repo"), PathBuf::from("scratch").join("cfg"));
        let command = repo.command(true);
        let envs: std::collections::HashMap<_, _> = command
            .get_envs()
            .map(|(k, v)| (k.to_string_lossy().into_owned(), v.map(|v| v.to_owned())))
            .collect();
        // Removed (present with no value) so a host value cannot reach Git.
        for var in [
            "GIT_TEMPLATE_DIR",
            "GIT_EXTERNAL_DIFF",
            "GIT_CONFIG_PARAMETERS",
            "GIT_CONFIG_COUNT",
            "GIT_DIR",
            "GIT_PAGER",
        ] {
            assert_eq!(envs.get(var), Some(&None), "{var} must be removed");
        }
        // Redirected into the scratch directory.
        for var in ["HOME", "USERPROFILE", "XDG_CONFIG_HOME"] {
            assert_eq!(
                envs.get(var).and_then(|v| v.as_deref()),
                Some(std::ffi::OsStr::new("scratch")),
                "{var}"
            );
        }
        let args: Vec<String> = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.iter().any(|a| a.starts_with("core.attributesFile=")));
        assert!(args.iter().any(|a| a.starts_with("core.excludesFile=")));
    }
}
