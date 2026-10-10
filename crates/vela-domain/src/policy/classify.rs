//! Operation classification: from a normalized operation and its context to an [`OperationClass`] and the facts rules
//! match on. Pure, table-driven, and conservative: when a command cannot be placed in a known class it is
//! [`OperationClass::UnknownExecutable`], which is `ASK`.

use serde::{Deserialize, Serialize};

use super::normalizer::{Dialect, NormalizedCommand, Rejection, normalize};
use super::operation::{Operation, PolicyContext, TrustState};
use super::path::{PathIssue, Placement, Relation, ResolvedPath};

/// What kind of thing an operation is, for rule matching and explanation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationClass {
    /// Read a file or directory.
    FileRead,
    /// Write a file or directory.
    FileWrite,
    /// Delete one filesystem entry, or something that cannot be placed.
    Delete,
    /// Recursive delete wholly below the worktree root.
    RecursiveDelete,
    /// Delete outside the worktree root, the root itself or an ancestor, or an unrecoverable disk operation.
    DestructiveDelete,
    /// Write or delete Git internals or agent and Vela policy locations (`.git`, `.agents`, `.gemini`, `.vela`).
    ProtectedPathChange,
    /// A command that creates, copies, moves or modifies files.
    FileMutate,
    /// Git read-only inspection.
    GitInspect,
    /// Git operation that changes local state.
    GitMutate,
    /// Normal `git push`.
    GitPush,
    /// Force push, remote deletion or mirror push.
    GitForcePush,
    /// Git operation that discards uncommitted work or history.
    GitDestructive,
    /// Git invoked with a configuration or location override.
    GitConfigOverride,
    /// Reading credentials, tokens, key material, or the environment.
    CredentialAccess,
    /// Disabling or weakening a security control.
    SecurityControlChange,
    /// System-wide configuration, installers, services, elevation.
    SystemConfiguration,
    /// Network or remote access.
    NetworkAccess,
    /// A shell, interpreter host or launcher that runs further commands.
    NestedShell,
    /// Destructive database, cloud or cluster operation.
    InfrastructureDestructive,
    /// A well-formed command whose executable is not otherwise classified.
    UnknownExecutable,
    /// Not a single simple command; see the rejection.
    Unparseable,
}

/// The facts a rule can match on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Facts {
    pub class: OperationClass,
    pub placement: Placement,
    /// Lookup name of the executable (basename without extension), commands only.
    pub executable: Option<String>,
    /// The command equals a confirmed profile entry in a permitted directory.
    pub profile_match: bool,
    pub rejection: Option<Rejection>,
}

impl Facts {
    fn of(class: OperationClass, placement: Placement) -> Facts {
        Facts {
            class,
            placement,
            executable: None,
            profile_match: false,
            rejection: None,
        }
    }
}

/// Analyzes `op` in `ctx`.
pub(crate) fn analyze(op: &Operation, ctx: &PolicyContext) -> Facts {
    match op {
        Operation::FileRead { path } => file_op(path, ctx, FileKind::Read),
        Operation::FileWrite { path } => file_op(path, ctx, FileKind::Write),
        Operation::FileDelete { path, recursive } => {
            file_op(path, ctx, FileKind::Delete(*recursive))
        }
        Operation::Command { dialect, text, cwd } => {
            let command = match normalize(text, *dialect) {
                Ok(c) => c,
                Err(rejection) => {
                    return Facts {
                        rejection: Some(rejection),
                        ..Facts::of(OperationClass::Unparseable, Placement::Unresolvable)
                    };
                }
            };
            let cwd = match cwd {
                Some(raw) => ResolvedPath::resolve(Some(ctx.root()), raw),
                None => Ok(ctx.root().clone()),
            };
            command_facts(&command, cwd, ctx, *dialect)
        }
    }
}

enum FileKind {
    Read,
    Write,
    Delete(bool),
}

fn file_op(raw: &str, ctx: &PolicyContext, kind: FileKind) -> Facts {
    let path = match ResolvedPath::resolve(Some(ctx.root()), raw) {
        Ok(p) => p,
        Err(issue) => {
            // Judge what cannot be resolved by its text, as for command targets, so the structured and command entry
            // points agree.
            let class = if arg_mentions_credentials(raw, ctx.root(), ctx.root()) {
                OperationClass::CredentialAccess
            } else if !matches!(kind, FileKind::Read) && raw_mentions_protected(raw, ctx.root()) {
                OperationClass::ProtectedPathChange
            } else {
                match kind {
                    FileKind::Read => OperationClass::FileRead,
                    FileKind::Write => OperationClass::FileWrite,
                    FileKind::Delete(recursive) => delete_class(&[Err(issue)], recursive, ctx),
                }
            };
            return Facts::of(class, Placement::Unresolvable);
        }
    };
    let placement = path.placement(ctx.root());
    if is_credential_path(&path, ctx.root()) {
        return Facts::of(OperationClass::CredentialAccess, placement);
    }
    let class = match kind {
        FileKind::Read => OperationClass::FileRead,
        FileKind::Write | FileKind::Delete(_) if is_protected(&path, ctx.root()) => {
            OperationClass::ProtectedPathChange
        }
        FileKind::Write => OperationClass::FileWrite,
        FileKind::Delete(recursive) => delete_class(&[Ok(path.clone())], recursive, ctx),
    };
    Facts::of(class, placement)
}

/// Facts for an already-normalized command, as stored in a profile entry, classified as if issued in `dialect`.
pub(crate) fn command_facts_for_parts(
    executable: &str,
    args: &[String],
    cwd: &str,
    ctx: &PolicyContext,
    dialect: Dialect,
) -> Facts {
    let command = NormalizedCommand::from_parts(executable.to_string(), args.to_vec());
    command_facts(&command, ResolvedPath::resolve(None, cwd), ctx, dialect)
}

fn command_facts(
    command: &NormalizedCommand,
    cwd: Result<ResolvedPath, PathIssue>,
    ctx: &PolicyContext,
    dialect: Dialect,
) -> Facts {
    let name = command.lookup_name();
    let cwd_placement = match &cwd {
        Ok(p) => p.placement(ctx.root()),
        Err(_) => Placement::Unresolvable,
    };
    let base = cwd.clone().unwrap_or_else(|_| ctx.root().clone());
    let (class, targets_placement) = classify_command(command, &name, &base, ctx, dialect);
    let placement = worst(cwd_placement, targets_placement);
    let profile_match = ctx.trust() == TrustState::Trusted
        && cwd.as_ref().is_ok_and(|dir| {
            dir.placement(ctx.root()) == Placement::InsideRoot
                && ctx.profile().matches(command, dir)
        });
    Facts {
        class,
        placement,
        executable: Some(name),
        profile_match,
        rejection: None,
    }
}

fn worst(a: Placement, b: Placement) -> Placement {
    use Placement::*;
    match (a, b) {
        (Unresolvable, _) | (_, Unresolvable) => Unresolvable,
        (OutsideRoot, _) | (_, OutsideRoot) => OutsideRoot,
        _ => InsideRoot,
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Command classification
// ---------------------------------------------------------------------------------------------------------------------

fn classify_command(
    cmd: &NormalizedCommand,
    name: &str,
    cwd: &ResolvedPath,
    ctx: &PolicyContext,
    dialect: Dialect,
) -> (OperationClass, Placement) {
    use OperationClass::*;
    let lower_args: Vec<String> = cmd.args().iter().map(|a| a.to_lowercase()).collect();
    let none = Placement::InsideRoot;

    if is_credential_command(cmd, name, &lower_args, cwd, ctx.root()) {
        return (CredentialAccess, none);
    }
    if is_security_control(name, &lower_args) {
        return (SecurityControlChange, none);
    }
    if DISK_DESTROYERS.contains(&name) || (name == "cipher" && lower_args.iter().any(|a| a == "/w"))
    {
        return (DestructiveDelete, none);
    }
    if name == "git" {
        return (git_class(cmd.args()), none);
    }

    let targets = target_paths(cmd.args(), cwd, dialect);
    let target_placement = targets
        .iter()
        .fold(Placement::InsideRoot, |acc, t| match &t.path {
            Ok(p) => worst(acc, p.placement(ctx.root())),
            Err(_) => worst(acc, Placement::Unresolvable),
        });

    if DELETERS.contains(&name) || FILE_MUTATORS.contains(&name) {
        if targets.iter().any(|t| t.is_protected(ctx.root())) {
            return (ProtectedPathChange, target_placement);
        }
        if FILE_MUTATORS.contains(&name) {
            return (FileMutate, target_placement);
        }
        let paths: Vec<_> = targets.iter().map(|t| t.path.clone()).collect();
        return (
            delete_class(&paths, is_recursive(&lower_args), ctx),
            target_placement,
        );
    }
    if NESTED_SHELLS.contains(&name) {
        return (NestedShell, none);
    }
    if INFRA_TOOLS
        .iter()
        .any(|(tool, words)| *tool == name && has_any_word(&lower_args, words))
    {
        return (InfrastructureDestructive, none);
    }
    if is_database_destructive(name, &lower_args) {
        return (InfrastructureDestructive, none);
    }
    if NETWORK_TOOLS.contains(&name) {
        return (NetworkAccess, none);
    }
    if SYSTEM_TOOLS.contains(&name) || is_global_install(name, &lower_args) {
        return (SystemConfiguration, none);
    }
    (UnknownExecutable, none)
}

const DISK_DESTROYERS: &[&str] = &[
    "format",
    "diskpart",
    "format-volume",
    "clear-disk",
    "remove-partition",
    "initialize-disk",
    "sdelete",
    "vssadmin",
    "wbadmin",
    "fsutil",
    "dd",
    "mkfs",
];
const DELETERS: &[&str] = &["rm", "del", "erase", "rd", "rmdir", "remove-item", "ri"];
const FILE_MUTATORS: &[&str] = &[
    "copy",
    "cp",
    "copy-item",
    "cpi",
    "xcopy",
    "robocopy",
    "move",
    "mv",
    "move-item",
    "mi",
    "ren",
    "rename",
    "rename-item",
    "rni",
    "mkdir",
    "md",
    "new-item",
    "ni",
    "set-content",
    "add-content",
    "ac",
    "clear-content",
    "clc",
    "out-file",
    "tee",
    "tee-object",
    "touch",
    "set-itemproperty",
    "sp",
    "new-itemproperty",
    "attrib",
    "expand-archive",
    "compress-archive",
    "tar",
    "unzip",
    "7z",
];
const NESTED_SHELLS: &[&str] = &[
    "cmd",
    "powershell",
    "pwsh",
    "bash",
    "sh",
    "zsh",
    "wsl",
    "wscript",
    "cscript",
    "mshta",
    "rundll32",
    "regsvr32",
    "invoke-expression",
    "iex",
    "invoke-command",
    "icm",
    "start-process",
    "start",
    "saps",
    "invoke-item",
    "ii",
    "start-job",
    "start-threadjob",
    "invoke-wmimethod",
    "invoke-cimmethod",
];
const NETWORK_TOOLS: &[&str] = &[
    "curl",
    "wget",
    "iwr",
    "irm",
    "invoke-webrequest",
    "invoke-restmethod",
    "start-bitstransfer",
    "bitsadmin",
    "certutil",
    "ftp",
    "tftp",
    "scp",
    "sftp",
    "ssh",
    "nc",
    "ncat",
    "telnet",
    "test-netconnection",
    "tnc",
];
const SYSTEM_TOOLS: &[&str] = &[
    "reg",
    "regedit",
    "setx",
    "schtasks",
    "sc",
    "net",
    "net1",
    "netsh",
    "icacls",
    "takeown",
    "winget",
    "choco",
    "scoop",
    "msiexec",
    "dism",
    "sfc",
    "pnputil",
    "wmic",
    "new-service",
    "set-service",
    "remove-service",
    "restart-service",
    "restart-computer",
    "stop-computer",
    "shutdown",
    "runas",
    "sudo",
    "gsudo",
    "assoc",
    "ftype",
    "mklink",
    "set-alias",
    "new-alias",
    "install-module",
    "install-package",
    "powercfg",
    "taskkill",
    "stop-process",
    "kill",
];
const INFRA_TOOLS: &[(&str, &[&str])] = &[
    ("terraform", &["destroy"]),
    ("tofu", &["destroy"]),
    ("pulumi", &["destroy"]),
    ("kubectl", &["delete", "drain", "replace"]),
    ("helm", &["uninstall", "delete"]),
    ("docker", &["prune", "rm", "rmi", "kill"]),
    ("podman", &["prune", "rm", "rmi", "kill"]),
    (
        "aws",
        &["delete", "terminate", "rm", "remove", "destroy", "purge"],
    ),
    ("az", &["delete", "purge"]),
    ("gcloud", &["delete", "destroy"]),
    ("gsutil", &["rm", "rb"]),
    ("dropdb", &[]),
    ("dropuser", &[]),
];

fn has_any_word(args: &[String], words: &[&str]) -> bool {
    words.is_empty() || words.iter().any(|w| args.iter().any(|a| a == w))
}

fn is_database_destructive(name: &str, lower_args: &[String]) -> bool {
    const CLIENTS: &[&str] = &[
        "psql",
        "mysql",
        "mariadb",
        "sqlcmd",
        "sqlite3",
        "mongosh",
        "redis-cli",
    ];
    const WORDS: &[&str] = &[
        "drop",
        "truncate",
        "delete",
        "flushall",
        "flushdb",
        "dropdatabase",
    ];
    CLIENTS.contains(&name)
        && lower_args.iter().any(|a| {
            a.split(|c: char| !c.is_ascii_alphanumeric())
                .any(|w| WORDS.contains(&w))
        })
}

fn is_global_install(name: &str, lower_args: &[String]) -> bool {
    let global = lower_args
        .iter()
        .any(|a| a == "-g" || a == "--global" || a == "global" || a == "--user");
    match name {
        "npm" | "pnpm" | "yarn" | "gem" | "pip" | "pip3" => {
            global || lower_args.iter().any(|a| a == "--break-system-packages")
        }
        "cargo" => lower_args.first().is_some_and(|a| a == "install"),
        "go" => lower_args.first().is_some_and(|a| a == "install"),
        _ => false,
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Credentials and security controls
// ---------------------------------------------------------------------------------------------------------------------

const CREDENTIAL_TOOLS: &[&str] = &[
    "cmdkey",
    "vaultcmd",
    "mimikatz",
    "procdump",
    "ssh-add",
    "get-credential",
    "convertfrom-securestring",
    "credwiz",
    "printenv",
    "env",
    "ntdsutil",
    "lsadump",
    "klist",
    "get-storedcredential",
];
const TOKEN_WORDS: &[&str] = &[
    "print-access-token",
    "print-identity-token",
    "get-access-token",
    "get-session-token",
    "get-login-password",
    "--show-token",
];
/// Directories whose contents are credential or key material.
const CREDENTIAL_DIRS: &[&str] = &[
    ".ssh", ".gnupg", ".aws", ".azure", ".kube", ".gemini", ".docker",
];
/// File names that are credential material wherever they appear.
const CREDENTIAL_FILES: &[&str] = &[
    ".git-credentials",
    ".netrc",
    "_netrc",
    ".npmrc",
    ".pypirc",
    "id_rsa",
    "id_dsa",
    "id_ecdsa",
    "id_ed25519",
    "authorized_keys",
    ".env",
];
const CREDENTIAL_EXTENSIONS: &[&str] = &[
    ".pem",
    ".pfx",
    ".p12",
    ".key",
    ".kdbx",
    ".ppk",
    ".keystore",
    ".jks",
];
/// Generic names that count only when the argument is path-like.
const WEAK_CREDENTIAL_NAMES: &[&str] = &[
    "credentials",
    "credentials.json",
    "secrets.json",
    "secret.json",
    "token",
    "tokens.json",
];
const SAFE_ENV_SUFFIXES: &[&str] = &["example", "sample", "template", "dist"];
const CREDENTIAL_SEQUENCES: &[&[&str]] = &[
    &["microsoft", "credentials"],
    &["microsoft", "protect"],
    &["microsoft", "vault"],
    &["system32", "config"],
    &["login data"],
    &["gh", "hosts.yml"],
];

fn is_credential_command(
    cmd: &NormalizedCommand,
    name: &str,
    lower_args: &[String],
    cwd: &ResolvedPath,
    root: &ResolvedPath,
) -> bool {
    if CREDENTIAL_TOOLS.contains(&name) {
        return true;
    }
    // `set` with no arguments lists the environment in cmd.
    if name == "set" && lower_args.is_empty() {
        return true;
    }
    if name == "gh"
        && lower_args.first().is_some_and(|a| a == "auth")
        && lower_args.iter().any(|a| a == "token" || a == "-t")
    {
        return true;
    }
    if name == "reg"
        && lower_args.iter().any(|a| {
            a.ends_with("\\sam")
                || a.ends_with("\\security")
                || a.contains("hklm\\sam")
                || a.contains("hklm\\security")
        })
    {
        return true;
    }
    let git_credential = name == "git"
        && (cmd
            .args()
            .iter()
            .any(|a| a == "credential" || a.starts_with("credential-"))
            || (lower_args.iter().any(|a| a == "config")
                && lower_args.iter().any(|a| a.contains("credential"))));
    git_credential
        || lower_args.iter().any(|a| TOKEN_WORDS.contains(&a.as_str()))
        || lower_args
            .iter()
            .any(|a| a.starts_with("env:") || a.contains("\\env:") || a.contains("/env:"))
        || cmd
            .args()
            .iter()
            .any(|a| arg_mentions_credentials(a, cwd, root))
}

fn arg_mentions_credentials(arg: &str, cwd: &ResolvedPath, root: &ResolvedPath) -> bool {
    let lower = arg.to_lowercase();
    let pathlike = lower.contains(['\\', '/']);
    // A plain path argument is judged below the worktree root, so the worktree's own location never counts.
    if pathlike && let Ok(path) = ResolvedPath::resolve(Some(cwd), arg) {
        return segments_are_credentials(&path.components_below(root), true);
    }
    let segments: Vec<&str> = lower
        .split(['\\', '/', '='])
        .filter(|s| !s.is_empty())
        .collect();
    // A bare word such as `token` in `cargo test token` is a name, not a path.
    segments_are_credentials(&segments, pathlike)
}

fn is_credential_path(path: &ResolvedPath, root: &ResolvedPath) -> bool {
    segments_are_credentials(&path.components_below(root), true)
}

/// True when a path, given as case-folded components, names credential or key material. Generic names (`token`,
/// `credentials`) count only as the final component of a path, so a source directory with that name, or a bare word that is
/// not a path (`weak_names` false), is not credential material.
fn segments_are_credentials(segments: &[&str], weak_names: bool) -> bool {
    for seg in segments {
        if CREDENTIAL_DIRS.contains(seg) || CREDENTIAL_FILES.contains(seg) {
            return true;
        }
        if let Some(suffix) = seg.strip_prefix(".env.")
            && !SAFE_ENV_SUFFIXES.contains(&suffix)
        {
            return true;
        }
        if CREDENTIAL_EXTENSIONS.iter().any(|ext| seg.ends_with(ext)) && seg.len() > 4 {
            return true;
        }
    }
    if weak_names
        && segments
            .last()
            .is_some_and(|last| WEAK_CREDENTIAL_NAMES.contains(last))
    {
        return true;
    }
    CREDENTIAL_SEQUENCES
        .iter()
        .any(|seq| segments.windows(seq.len()).any(|w| w == *seq))
}

fn is_security_control(name: &str, lower_args: &[String]) -> bool {
    const TOOLS: &[&str] = &[
        "set-mppreference",
        "add-mppreference",
        "set-executionpolicy",
        "bcdedit",
        "auditpol",
        "clear-eventlog",
        "set-netfirewallprofile",
        "new-netfirewallrule",
        "set-netfirewallrule",
        "disable-windowsoptionalfeature",
    ];
    const SERVICES: &[&str] = &[
        "windefend",
        "mpssvc",
        "wscsvc",
        "sense",
        "eventlog",
        "wdnissvc",
        "securityhealthservice",
    ];
    if TOOLS.contains(&name) || name.starts_with("disable-") {
        return true;
    }
    if name == "netsh"
        && lower_args
            .iter()
            .any(|a| a == "advfirewall" || a == "firewall")
    {
        return true;
    }
    if name == "wevtutil" && lower_args.iter().any(|a| a == "cl" || a == "clear-log") {
        return true;
    }
    let touches_service = lower_args.iter().any(|a| SERVICES.contains(&a.as_str()));
    touches_service
        && matches!(
            name,
            "sc" | "net" | "net1" | "stop-service" | "set-service" | "spsv" | "taskkill"
        )
}

// ---------------------------------------------------------------------------------------------------------------------
// Files: targets, deletes, protected locations
// ---------------------------------------------------------------------------------------------------------------------

/// A command argument that names a path, with its resolution.
struct Target {
    raw: String,
    path: Result<ResolvedPath, PathIssue>,
}

impl Target {
    /// True when the target is at a protected location. A target the engine could not resolve is judged by its text, so
    /// a wildcard, trailing dot or short name cannot hide a protected location.
    fn is_protected(&self, root: &ResolvedPath) -> bool {
        match &self.path {
            Ok(p) => is_protected(p, root),
            Err(_) => raw_mentions_protected(&self.raw, root),
        }
    }
}

/// True for a switch rather than a path. A single-slash word of at most two characters is a switch in every dialect
/// (`/s`, `/q`); longer ones only in `cmd`, where `/MIR` and `/purge` are switches and `/Windows` is not valid anyway. In
/// PowerShell a longer `/word` is a rooted path.
fn is_flag(arg: &str, dialect: Dialect) -> bool {
    if arg == "-" || arg == "--" {
        return true;
    }
    if arg.starts_with('-') {
        return !arg.contains(['\\', '/']) || arg.contains(':');
    }
    let tail = arg
        .strip_prefix('/')
        .filter(|t| !t.is_empty() && !t.contains(['/', '\\']));
    match tail {
        Some(t) => t.len() <= 2 || (dialect == Dialect::Cmd && t.len() <= 16),
        None => false,
    }
}

/// Paths a command argument list names: bare arguments plus values of `-Param:value` and `key=value` forms.
fn target_paths(args: &[String], cwd: &ResolvedPath, dialect: Dialect) -> Vec<Target> {
    let mut out = Vec::new();
    for arg in args {
        let candidate: Option<&str> = if is_flag(arg, dialect) {
            arg.split_once(':')
                .filter(|_| arg.starts_with('-'))
                .map(|(_, v)| v)
                .filter(|v| !v.is_empty())
        } else if !arg.starts_with('-') && arg.contains('=') && !arg.contains(['\\', '/']) {
            arg.split_once('=')
                .map(|(_, v)| v)
                .filter(|v| !v.is_empty())
        } else {
            Some(arg.as_str())
        };
        if let Some(c) = candidate {
            out.push(Target {
                raw: c.to_string(),
                path: ResolvedPath::resolve(Some(cwd), c),
            });
        }
    }
    out
}

fn is_recursive(lower_args: &[String]) -> bool {
    lower_args.iter().any(|a| {
        if a == "/s" {
            return true;
        }
        let Some(rest) = a.strip_prefix('-') else {
            return false;
        };
        if let Some(long) = rest.strip_prefix('-') {
            return long
                .split(':')
                .next()
                .is_some_and(|w| !w.is_empty() && (w == "recursive" || "recurse".starts_with(w)));
        }
        let flag = rest.split(':').next().unwrap_or("");
        // PowerShell parameter abbreviation (`-r`, `-rec`) or a unix-style cluster (`-rf`).
        !flag.is_empty()
            && (flag == "r"
                || "recurse".starts_with(flag)
                || (flag.len() <= 3 && flag.contains('r')))
    })
}

fn delete_class(
    targets: &[Result<ResolvedPath, PathIssue>],
    recursive: bool,
    ctx: &PolicyContext,
) -> OperationClass {
    let mut any_inside = false;
    let mut unresolved = false;
    for t in targets {
        match t {
            // A target that names an absolute location the engine cannot place (a rooted path, home, a bare drive, a
            // device namespace, a short name, an escape above the prefix) is treated as unrecoverable; a recursive
            // delete of anything unplaceable is too.
            Err(
                PathIssue::NotAbsolute
                | PathIssue::HomeOrVariable
                | PathIssue::DriveRelative
                | PathIssue::DeviceNamespace
                | PathIssue::ShortNameOrAlias
                | PathIssue::EscapesPrefix,
            ) => return OperationClass::DestructiveDelete,
            Err(_) if recursive => return OperationClass::DestructiveDelete,
            Err(_) => unresolved = true,
            Ok(p) => {
                if p.is_prefix_root() {
                    return OperationClass::DestructiveDelete;
                }
                match p.relation_to(ctx.root()) {
                    Relation::Below => any_inside = true,
                    Relation::Equal | Relation::Above | Relation::Disjoint => {
                        return OperationClass::DestructiveDelete;
                    }
                }
            }
        }
    }
    if unresolved || targets.is_empty() {
        return OperationClass::Delete;
    }
    if recursive && any_inside {
        OperationClass::RecursiveDelete
    } else {
        OperationClass::Delete
    }
}

/// Locations an agent must never write or delete: Git internals and the agent, hook and Vela policy directories
/// (ADR-013, ADR-018). Judged below the worktree root.
fn is_protected(path: &ResolvedPath, root: &ResolvedPath) -> bool {
    path.components_below(root)
        .iter()
        .any(|c| PROTECTED_COMPONENTS.contains(c))
}

/// The text-level check for a target that could not be resolved: any protected directory name in it, ignoring the
/// trailing dots and spaces Windows would drop, after removing the worktree root prefix when present.
fn raw_mentions_protected(raw: &str, root: &ResolvedPath) -> bool {
    let text = raw.to_lowercase().replace('/', "\\");
    let root_prefix = root.display().to_lowercase();
    let rest = text.strip_prefix(&root_prefix).unwrap_or(&text);
    rest.split('\\')
        .map(|seg| {
            seg.trim_start_matches(' ')
                .trim_end_matches([' ', '.'])
                .to_string()
        })
        .any(|seg| PROTECTED_COMPONENTS.contains(&seg.as_str()))
}

/// Directory names (case-folded) that mark a protected location below the worktree root.
pub const PROTECTED_COMPONENTS: &[&str] = &[".git", ".agents", ".gemini", ".vela", ".antigravity"];

/// What the native worktree write allow must exclude so it stays within what policy allows (ADR-009 NPP-2): the protected
/// directories and the credential and key material that policy denies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteExclusions {
    /// Directory names that are never writable below the root (protected locations and credential directories).
    pub components: Vec<String>,
    /// File names that are never writable.
    pub file_names: Vec<String>,
    /// File-name prefixes that are never writable (`.env.` except the documented example suffixes).
    pub file_prefixes: Vec<String>,
    /// File extensions that are never writable.
    pub file_extensions: Vec<String>,
}

/// The exclusions, derived from the same tables the classifier uses.
pub fn write_exclusions() -> WriteExclusions {
    let owned = |items: &[&str]| items.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
    let mut components = owned(PROTECTED_COMPONENTS);
    components.extend(owned(CREDENTIAL_DIRS));
    WriteExclusions {
        components,
        file_names: owned(CREDENTIAL_FILES),
        file_prefixes: vec![".env.".to_string()],
        file_extensions: owned(CREDENTIAL_EXTENSIONS),
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Git
// ---------------------------------------------------------------------------------------------------------------------

fn git_class(args: &[String]) -> OperationClass {
    use OperationClass::*;
    let mut i = 0;
    let mut overridden = false;
    while i < args.len() && args[i].starts_with('-') {
        match args[i].as_str() {
            "-c" | "-C" | "--git-dir" | "--work-tree" | "--namespace" | "--exec-path"
            | "--config-env" | "--super-prefix" | "--attr-source" => {
                overridden = true;
                i += 2;
            }
            "--no-pager"
            | "-p"
            | "-P"
            | "--paginate"
            | "--no-optional-locks"
            | "--no-replace-objects"
            | "--literal-pathspecs"
            | "--glob-pathspecs"
            | "--noglob-pathspecs"
            | "--icase-pathspecs" => i += 1,
            _ => {
                overridden = true;
                i += 1;
            }
        }
    }
    let Some(sub) = args.get(i).map(String::as_str) else {
        return GitInspect;
    };
    let rest = &args[i + 1..];
    let class = git_subcommand_class(sub, rest);
    match class {
        GitInspect | GitMutate | GitPush if overridden => GitConfigOverride,
        other => other,
    }
}

fn long(rest: &[String], names: &[&str]) -> bool {
    rest.iter().any(|a| {
        a.strip_prefix("--").is_some_and(|body| {
            let body = body.split('=').next().unwrap_or("");
            names.contains(&body)
        })
    })
}

/// Like [`long`], but also true for an abbreviation of a name: Git accepts any unambiguous prefix of a long option, so
/// `--forc` is `--force`. Used for options that make a command more dangerous, where over-matching only errs toward the
/// stricter class.
fn long_abbrev(rest: &[String], names: &[&str]) -> bool {
    rest.iter().any(|a| {
        a.strip_prefix("--").is_some_and(|body| {
            let body = body.split('=').next().unwrap_or("");
            !body.is_empty() && names.iter().any(|n| n.starts_with(body))
        })
    })
}

fn short(rest: &[String], flag: char) -> bool {
    rest.iter()
        .any(|a| a.starts_with('-') && !a.starts_with("--") && a[1..].contains(flag))
}

fn git_subcommand_class(sub: &str, rest: &[String]) -> OperationClass {
    use OperationClass::*;
    let positional: Vec<&String> = rest.iter().filter(|a| !a.starts_with('-')).collect();
    match sub {
        "push" => {
            let force = long_abbrev(
                rest,
                &[
                    "force",
                    "force-with-lease",
                    "force-if-includes",
                    "mirror",
                    "prune",
                    "delete",
                ],
            ) || short(rest, 'f')
                || short(rest, 'd')
                || rest
                    .iter()
                    .any(|a| a.starts_with('+') || a.starts_with(':') || a.contains(":+"));
            if force { GitForcePush } else { GitPush }
        }
        "reset" => {
            if long_abbrev(rest, &["hard", "merge"]) {
                GitDestructive
            } else {
                GitMutate
            }
        }
        "clean" => {
            if long(rest, &["dry-run"]) || short(rest, 'n') {
                GitInspect
            } else {
                GitDestructive
            }
        }
        "checkout" => {
            // `git checkout <path>` overwrites local edits with no flag at all, and a branch name cannot be told from a
            // path lexically, so only branch creation and a single plain branch-like name are a plain state change.
            let creates = short(rest, 'b') || short(rest, 'B') || long_abbrev(rest, &["orphan"]);
            let discards = rest.iter().any(|a| a == "--" || a == ".")
                || short(rest, 'f')
                || long_abbrev(rest, &["force", "ours", "theirs"]);
            let branch_like = |name: &str| !name.contains(['/', '\\', '.']);
            if discards {
                GitDestructive
            } else if creates
                || positional.is_empty()
                || (positional.len() == 1 && branch_like(positional[0]))
            {
                GitMutate
            } else {
                GitDestructive
            }
        }
        "rm" => {
            if short(rest, 'f') || long_abbrev(rest, &["force"]) {
                GitDestructive
            } else {
                GitMutate
            }
        }
        "restore" => {
            if long(rest, &["staged"]) && !long_abbrev(rest, &["worktree"]) {
                GitMutate
            } else {
                GitDestructive
            }
        }
        "switch" => {
            if long_abbrev(rest, &["force", "discard-changes"]) || short(rest, 'f') {
                GitDestructive
            } else {
                GitMutate
            }
        }
        "stash" => match positional.first().map(|s| s.as_str()) {
            Some("drop") | Some("clear") => GitDestructive,
            Some("list") | Some("show") => GitInspect,
            _ => GitMutate,
        },
        "branch" => {
            if short(rest, 'D') {
                GitDestructive
            } else if short(rest, 'd')
                || short(rest, 'm')
                || short(rest, 'M')
                || short(rest, 'c')
                || short(rest, 'C')
                || long(
                    rest,
                    &[
                        "delete",
                        "move",
                        "copy",
                        "set-upstream-to",
                        "unset-upstream",
                        "edit-description",
                    ],
                )
            {
                GitMutate
            } else if positional.is_empty()
                || long(
                    rest,
                    &["list", "show-current", "contains", "merged", "no-merged"],
                )
            {
                GitInspect
            } else {
                GitMutate
            }
        }
        "tag" => {
            if short(rest, 'd') || long(rest, &["delete"]) {
                GitMutate
            } else if positional.is_empty() || short(rest, 'l') || long(rest, &["list"]) {
                GitInspect
            } else {
                GitMutate
            }
        }
        "reflog" => match positional.first().map(|s| s.as_str()) {
            Some("expire") | Some("delete") => GitDestructive,
            _ => GitInspect,
        },
        "gc" => {
            if long_abbrev(rest, &["prune", "aggressive"]) {
                GitDestructive
            } else {
                GitMutate
            }
        }
        "prune" | "filter-branch" | "filter-repo" | "replace" => GitDestructive,
        "update-ref" => {
            if short(rest, 'd') {
                GitDestructive
            } else {
                GitMutate
            }
        }
        "worktree" => match positional.first().map(|s| s.as_str()) {
            Some("list") => GitInspect,
            Some("remove") | Some("prune") if long_abbrev(rest, &["force"]) || short(rest, 'f') => {
                GitDestructive
            }
            _ => GitMutate,
        },
        "remote" => {
            if positional.is_empty()
                || matches!(
                    positional.first().map(|s| s.as_str()),
                    Some("show") | Some("get-url")
                )
            {
                GitInspect
            } else {
                GitMutate
            }
        }
        "credential" | "credential-store" | "credential-cache" | "credential-manager" => {
            CredentialAccess
        }
        "status" | "log" | "show" | "rev-parse" | "rev-list" | "ls-files" | "ls-tree"
        | "cat-file" | "blame" | "describe" | "shortlog" | "grep" | "show-ref" | "name-rev"
        | "diff" | "diff-tree" | "diff-index" | "merge-base" | "version" | "help"
        | "check-ignore" | "count-objects" | "for-each-ref" | "whatchanged" | "symbolic-ref" => {
            const EXEC_FLAGS: &[&str] = &[
                "output",
                "ext-diff",
                "textconv",
                "open-files-in-pager",
                "exec",
                "paginate",
            ];
            if long_abbrev(rest, EXEC_FLAGS) || rest.iter().any(|a| a.starts_with("-O")) {
                GitMutate
            } else {
                GitInspect
            }
        }
        _ => GitMutate,
    }
}
