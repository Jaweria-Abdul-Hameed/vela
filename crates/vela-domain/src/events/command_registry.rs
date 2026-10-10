//! Command registry type and the Rust/TypeScript contract comparison (F02 scaffold;
//! `SHARED_SURFACE_PROTOCOL.md` section 4, `IMPLEMENTATION_ARCHITECTURE.md` section 6).
//!
//! Commands are registered one module per group: `apps/desktop/src-tauri/src/commands/<group>.rs` on the Rust side and
//! `packages/contracts/src/commands/<group>.ts` on the TypeScript side. The registry is assembled from the group modules
//! in sorted order, so adding a group edits no other group's file. This module defines the registry type and the pure
//! comparison; the real commands arrive with the tickets that own each group. A command's name is independent of the
//! group module that registers it (see [`CommandRegistry`]).
//!
//! TypeScript wrapper convention (read by [`parse_wrapper_names`]): each group file declares its commands in
//! `export const commandNames = ['name.action', ...] as const;`.

use std::collections::BTreeSet;
use std::fmt;

/// The command groups (`SHARED_SURFACE_PROTOCOL.md` section 4), in sorted order of their wire names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CommandGroup {
    /// The `approval` group.
    Approval,
    /// The `diagnostics` group.
    Diagnostics,
    /// The `evidence` group.
    Evidence,
    /// The `graph` group.
    Graph,
    /// The `intervention` group.
    Intervention,
    /// The `policy` group.
    Policy,
    /// The `preflight` group.
    Preflight,
    /// The `project` group.
    Project,
    /// The `run` group.
    Run,
    /// The `settings` group.
    Settings,
    /// The `snapshot` group.
    Snapshot,
    /// The `ticket` group.
    Ticket,
    /// The `timeline` group.
    Timeline,
    /// The `trust` group.
    Trust,
    /// The `update` group.
    Update,
}

impl CommandGroup {
    /// Every group, sorted by wire name.
    pub const ALL: [Self; 15] = [
        Self::Approval,
        Self::Diagnostics,
        Self::Evidence,
        Self::Graph,
        Self::Intervention,
        Self::Policy,
        Self::Preflight,
        Self::Project,
        Self::Run,
        Self::Settings,
        Self::Snapshot,
        Self::Ticket,
        Self::Timeline,
        Self::Trust,
        Self::Update,
    ];

    /// The group's wire name: the command-name prefix and the module and file stem.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Approval => "approval",
            Self::Diagnostics => "diagnostics",
            Self::Evidence => "evidence",
            Self::Graph => "graph",
            Self::Intervention => "intervention",
            Self::Policy => "policy",
            Self::Preflight => "preflight",
            Self::Project => "project",
            Self::Run => "run",
            Self::Settings => "settings",
            Self::Snapshot => "snapshot",
            Self::Ticket => "ticket",
            Self::Timeline => "timeline",
            Self::Trust => "trust",
            Self::Update => "update",
        }
    }

    /// Parses a wire name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|g| g.as_str() == name)
    }
}

/// One registered command: the group module that registers it and its full name (for example group `project`, name
/// `project.import`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CommandDescriptor {
    /// The group module that registers the command (the Rust module and TypeScript file stem).
    pub group: CommandGroup,
    /// The full command name: dotted lowercase segments, for example `project.trust.set`.
    pub name: String,
}

/// Why a command could not be registered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// The name is not two or more dot-separated lowercase segments (`[a-z][a-z0-9_]*`).
    InvalidName(String),
    /// The name is already registered.
    Duplicate(String),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(name) => {
                write!(f, "command name {name:?} is not <segment>.<segment>[...]")
            }
            Self::Duplicate(name) => write!(f, "command {name:?} is registered twice"),
        }
    }
}

impl std::error::Error for RegistryError {}

/// True for `segment(.segment)+` where each segment is `[a-z][a-z0-9_]*`.
pub fn is_valid_command_name(name: &str) -> bool {
    let mut segments = 0;
    for segment in name.split('.') {
        let mut chars = segment.chars();
        let valid = chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if !valid {
            return false;
        }
        segments += 1;
    }
    segments >= 2
}

/// The set of registered commands, always kept in sorted order.
///
/// A command's *name* (the IPC command, for example `onboarding.set` or `project.trust.set`) is independent of the
/// *group module* that registers it: the frozen specification lists command names (`IMPLEMENTATION_ARCHITECTURE.md`
/// section 6) and group modules (`SHARED_SURFACE_PROTOCOL.md` section 4) separately and does not map every command to a
/// group, so the registry does not invent that mapping. The owning ticket registers each command under its group.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandRegistry {
    commands: BTreeSet<CommandDescriptor>,
}

impl CommandRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `name` under the module of `group`. The name must be a valid command name; duplicates (in any group)
    /// are rejected.
    pub fn register(&mut self, group: CommandGroup, name: &str) -> Result<(), RegistryError> {
        if !is_valid_command_name(name) {
            return Err(RegistryError::InvalidName(name.to_owned()));
        }
        if self.commands.iter().any(|c| c.name == name) {
            return Err(RegistryError::Duplicate(name.to_owned()));
        }
        self.commands.insert(CommandDescriptor {
            group,
            name: name.to_owned(),
        });
        Ok(())
    }

    /// The registered commands, sorted by group then name.
    pub fn commands(&self) -> impl Iterator<Item = &CommandDescriptor> {
        self.commands.iter()
    }

    /// Number of registered commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// True when nothing is registered.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

/// A difference between the Rust registry and the TypeScript wrappers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractMismatch {
    /// Registered in Rust but without a TypeScript wrapper.
    pub missing_in_typescript: Vec<String>,
    /// Wrapped in TypeScript but not registered in Rust.
    pub missing_in_rust: Vec<String>,
}

impl fmt::Display for ContractMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "command contract mismatch: Rust commands without a TypeScript wrapper: {:?}; \
             TypeScript wrappers without a Rust command: {:?}",
            self.missing_in_typescript, self.missing_in_rust
        )
    }
}

impl std::error::Error for ContractMismatch {}

/// Compares the Rust registry with the command names found in the TypeScript wrappers.
///
/// Fails when a Rust command lacks a TypeScript wrapper or the reverse. Both lists are sorted and de-duplicated. The
/// crate that assembles the real registry (the desktop shell) calls this from its contract test with the real registry
/// and the names read from `packages/contracts/src/commands/<group>.ts`.
pub fn compare_contract(
    registry: &CommandRegistry,
    typescript_names: &[String],
) -> Result<(), ContractMismatch> {
    let rust: BTreeSet<&str> = registry.commands().map(|c| c.name.as_str()).collect();
    let ts: BTreeSet<&str> = typescript_names.iter().map(String::as_str).collect();
    let missing_in_typescript: Vec<String> =
        rust.difference(&ts).map(|s| (*s).to_owned()).collect();
    let missing_in_rust: Vec<String> = ts.difference(&rust).map(|s| (*s).to_owned()).collect();
    if missing_in_typescript.is_empty() && missing_in_rust.is_empty() {
        Ok(())
    } else {
        Err(ContractMismatch {
            missing_in_typescript,
            missing_in_rust,
        })
    }
}

/// Removes `//` and `/* */` comments, keeping string and template literals intact (a comment marker inside a literal is
/// not a comment).
fn strip_ts_comments(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            quote @ ('\'' | '"' | '`') => {
                out.push(quote);
                i += 1;
                while i < chars.len() && chars[i] != quote {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        out.push(chars[i]);
                        i += 1;
                    }
                    out.push(chars[i]);
                    i += 1;
                }
                if i < chars.len() {
                    out.push(quote);
                    i += 1;
                }
            }
            '/' if chars.get(i + 1) == Some(&'/') => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '/' if chars.get(i + 1) == Some(&'*') => {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                i = (i + 2).min(chars.len());
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// Extracts the command names from a TypeScript group file: the string literals (single, double or backtick without
/// substitutions) of the array assigned in `export const commandNames ... = [ ... ]`. Comments are ignored, and an
/// optional type annotation (`: readonly string[]`) before the `=` is skipped. A file without that declaration declares
/// no commands.
pub fn parse_wrapper_names(source: &str) -> Vec<String> {
    const MARKER: &str = "export const commandNames";
    let code = strip_ts_comments(source);
    // The marker must be the whole identifier: `commandNamesByGroup` is a different declaration.
    let Some(start) = code.match_indices(MARKER).map(|(at, _)| at).find(|at| {
        !code[at + MARKER.len()..]
            .starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$')
    }) else {
        return Vec::new();
    };
    let rest = &code[start + MARKER.len()..];
    let Some(equals) = rest.find('=') else {
        return Vec::new();
    };
    let Some(body) = rest[equals + 1..].trim_start().strip_prefix('[') else {
        return Vec::new();
    };
    let mut names = Vec::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        match c {
            ']' => return names,
            '\'' | '"' | '`' => {
                let mut literal = String::new();
                for d in chars.by_ref() {
                    if d == c {
                        break;
                    }
                    literal.push(d);
                }
                names.push(literal);
            }
            _ => {}
        }
    }
    // No closing bracket: the declaration is malformed, so it declares nothing.
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Reads `packages/contracts/src/commands/<group>.ts`. A missing directory means no wrappers yet. Only files named
    /// after a group count; helper files with a dotted stem (`run.test.ts`, `run.d.ts`) are ignored, and a file named
    /// after no group is itself a mismatch (it would be a typo or a stray barrel).
    fn typescript_wrappers(dir: &Path) -> Result<Vec<String>, String> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Ok(Vec::new());
        };
        let mut names = Vec::new();
        for entry in entries {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ts") {
                continue;
            }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if stem.contains('.') {
                continue;
            }
            if CommandGroup::from_name(stem).is_none() {
                return Err(format!(
                    "wrapper file {stem}.ts is not a known command group"
                ));
            }
            let source = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            for name in parse_wrapper_names(&source) {
                if !is_valid_command_name(&name) {
                    return Err(format!("{name:?} in {stem}.ts is not a valid command name"));
                }
                names.push(name);
            }
        }
        Ok(names)
    }

    #[test]
    fn groups_are_sorted_unique_and_round_trip_their_names() {
        let names: Vec<&str> = CommandGroup::ALL.iter().map(|g| g.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(names, sorted);
        assert_eq!(names.len(), 15);
        let mut by_ord = CommandGroup::ALL;
        by_ord.sort();
        assert_eq!(
            by_ord,
            CommandGroup::ALL,
            "derive order must match wire order"
        );
        for group in CommandGroup::ALL {
            assert_eq!(CommandGroup::from_name(group.as_str()), Some(group));
        }
        assert_eq!(CommandGroup::from_name("nope"), None);
    }

    #[test]
    fn command_names_are_dotted_lowercase_segments() {
        for ok in [
            "project.import",
            "project.trust.set",
            "onboarding.set",
            "run.stop_all",
            "a1.b2",
        ] {
            assert!(is_valid_command_name(ok), "{ok}");
        }
        for bad in [
            "",
            "project",
            "project.",
            ".set",
            "Project.import",
            "a..b",
            "a.1b",
            "a.b-c",
            "a b.c",
            "a.b ",
        ] {
            assert!(!is_valid_command_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn registry_rejects_invalid_names_and_duplicates_in_any_group() {
        let mut registry = CommandRegistry::new();
        assert!(registry.is_empty());
        registry
            .register(CommandGroup::Project, "project.import")
            .unwrap();
        registry
            .register(CommandGroup::Project, "project.trust.set")
            .unwrap();
        assert_eq!(
            registry.register(CommandGroup::Project, "Project.Import"),
            Err(RegistryError::InvalidName("Project.Import".into()))
        );
        assert!(registry.register(CommandGroup::Project, "project").is_err());
        assert_eq!(
            registry.register(CommandGroup::Trust, "project.import"),
            Err(RegistryError::Duplicate("project.import".into()))
        );
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn a_command_name_does_not_have_to_start_with_its_group() {
        // `onboarding.set` appears in the IPC command list but no `onboarding` group is specified; which group
        // registers it is the owning ticket's decision (see docs/project/F02_DECISION_REQUESTS.md), so the registry
        // must not reject it under any group.
        let mut registry = CommandRegistry::new();
        registry
            .register(CommandGroup::Settings, "onboarding.set")
            .unwrap();
        registry
            .register(CommandGroup::Trust, "project.trust.set")
            .unwrap();
        let names: Vec<&str> = registry.commands().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["onboarding.set", "project.trust.set"]);
    }

    #[test]
    fn the_wrapper_directory_scan_is_well_formed_for_the_real_package() {
        // Equality with the real Rust registry is asserted by the contract test of the crate that assembles that
        // registry (it calls `compare_contract`); here only the scan of the real wrapper directory is checked, so this
        // test never needs editing when a command ticket adds a wrapper.
        let dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/contracts/src/commands");
        let wrappers = typescript_wrappers(&dir).expect("wrapper files are well formed");
        let unique: BTreeSet<&String> = wrappers.iter().collect();
        assert_eq!(unique.len(), wrappers.len(), "a command is declared twice");
    }

    #[test]
    fn contract_test_passes_with_an_empty_registry_and_no_wrappers() {
        assert_eq!(compare_contract(&CommandRegistry::new(), &[]), Ok(()));
    }

    #[test]
    fn contract_test_fails_with_a_seeded_mismatch_in_either_direction() {
        let mut registry = CommandRegistry::new();
        registry.register(CommandGroup::Run, "run.create").unwrap();
        registry.register(CommandGroup::Run, "run.start").unwrap();

        // Rust command without a wrapper.
        let err = compare_contract(&registry, &["run.create".to_owned()]).unwrap_err();
        assert_eq!(err.missing_in_typescript, ["run.start"]);
        assert!(err.missing_in_rust.is_empty());

        // Wrapper without a Rust command.
        let all = ["run.create".to_owned(), "run.start".to_owned()];
        assert_eq!(compare_contract(&registry, &all), Ok(()));
        let extra = [
            "run.create".to_owned(),
            "run.start".to_owned(),
            "run.cancel".to_owned(),
        ];
        let err = compare_contract(&registry, &extra).unwrap_err();
        assert_eq!(err.missing_in_rust, ["run.cancel"]);
        assert!(err.to_string().contains("run.cancel"));
    }

    #[test]
    fn wrapper_names_are_read_from_the_command_names_array() {
        let source = "import x from 'y';\n\
            export const commandNames = [\n  'project.import',\n  \"project.list\",\n] as const;\n\
            export const other = ['not.a.command'];";
        assert_eq!(
            parse_wrapper_names(source),
            ["project.import", "project.list"]
        );
        assert!(parse_wrapper_names("export const other = [];").is_empty());
        assert!(parse_wrapper_names("export const commandNames = [").is_empty());
    }

    #[test]
    fn wrapper_parsing_handles_annotations_comments_and_template_literals() {
        let annotated = "export const commandNames: readonly string[] = ['run.create'] as const;";
        assert_eq!(parse_wrapper_names(annotated), ["run.create"]);

        let commented = "export const commandNames = [\n  // don't rename: it's stable\n  'run.create',\n  /* 'run.old', ] */\n  'run.start', // trailing ]\n] as const;";
        assert_eq!(parse_wrapper_names(commented), ["run.create", "run.start"]);

        let template = "export const commandNames = [`run.create`, 'run.start'] as const;";
        assert_eq!(parse_wrapper_names(template), ["run.create", "run.start"]);

        // A marker inside a comment does not declare commands; a comment marker inside a string is not a comment.
        assert!(parse_wrapper_names("// export const commandNames = ['a.b'];").is_empty());
        // A longer identifier that merely starts with the marker is a different declaration.
        assert!(
            parse_wrapper_names("export const commandNamesByGroup = ['a.b'] as const;").is_empty()
        );
        let both =
            "export const commandNamesByGroup = ['x.y'];\nexport const commandNames = ['a.b'];";
        assert_eq!(parse_wrapper_names(both), ["a.b"]);
        let urlish = "export const commandNames = ['a.b//c'] as const;";
        assert_eq!(parse_wrapper_names(urlish), ["a.b//c"]);
    }

    #[test]
    fn wrapper_scan_ignores_helper_files_and_rejects_unknown_groups_and_bad_names() {
        let dir = std::env::temp_dir().join(format!("vela-domain-wrappers-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("run.ts"),
            "export const commandNames = ['run.create'] as const;",
        )
        .unwrap();
        // Helper files next to a group file are not group files.
        std::fs::write(
            dir.join("run.test.ts"),
            "export const commandNames = ['x.y'];",
        )
        .unwrap();
        std::fs::write(dir.join("run.d.ts"), "export const commandNames = ['x.z'];").unwrap();
        std::fs::write(dir.join("notes.md"), "ignored").unwrap();
        assert_eq!(typescript_wrappers(&dir), Ok(vec!["run.create".to_owned()]));

        // A wrapper may declare a command whose name does not start with its group (`onboarding.set` in `settings`).
        std::fs::write(
            dir.join("settings.ts"),
            "export const commandNames = ['onboarding.set'] as const;",
        )
        .unwrap();
        let names = typescript_wrappers(&dir).unwrap();
        assert!(names.contains(&"onboarding.set".to_owned()));

        // An invalid command name is an error.
        std::fs::write(
            dir.join("settings.ts"),
            "export const commandNames = ['Bad Name'] as const;",
        )
        .unwrap();
        assert!(typescript_wrappers(&dir).is_err());

        // A file named after no group (a typo, or a stray barrel) is an error.
        std::fs::write(dir.join("settings.ts"), "").unwrap();
        std::fs::write(dir.join("index.ts"), "").unwrap();
        assert!(typescript_wrappers(&dir).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
