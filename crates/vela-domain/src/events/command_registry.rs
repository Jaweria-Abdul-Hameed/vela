//! Command registry type and the Rust/TypeScript contract comparison (F02 scaffold;
//! `SHARED_SURFACE_PROTOCOL.md` section 4, `IMPLEMENTATION_ARCHITECTURE.md` section 6).
//!
//! Commands are registered one module per group: `apps/desktop/src-tauri/src/commands/<group>.rs` on the Rust side and
//! `packages/contracts/src/commands/<group>.ts` on the TypeScript side. The registry is assembled from the group modules
//! in sorted order, so adding a group edits no other group's file. This module defines the registry type and the pure
//! comparison; the real commands arrive with the tickets that own each group.
//!
//! TypeScript wrapper convention (read by [`parse_wrapper_names`]): each group file declares its commands in
//! `export const commandNames = ['group.name', ...] as const;`.

use std::collections::BTreeSet;
use std::fmt;

/// The command groups (`SHARED_SURFACE_PROTOCOL.md` section 4), in sorted order of their wire names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CommandGroup {
    /// `approval.*`
    Approval,
    /// `diagnostics.*`
    Diagnostics,
    /// `evidence.*`
    Evidence,
    /// `graph.*`
    Graph,
    /// `intervention.*`
    Intervention,
    /// `policy.*`
    Policy,
    /// `preflight.*`
    Preflight,
    /// `project.*`
    Project,
    /// `run.*`
    Run,
    /// `settings.*`
    Settings,
    /// `snapshot.*`
    Snapshot,
    /// `ticket.*`
    Ticket,
    /// `timeline.*`
    Timeline,
    /// `trust.*`
    Trust,
    /// `update.*`
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

/// One registered command, for example group `project`, name `project.import`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CommandDescriptor {
    /// The group the command belongs to.
    pub group: CommandGroup,
    /// The full command name, `<group>.<action>` (for example `project.trust.set`).
    pub name: String,
}

/// Why a command could not be registered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// The name does not start with `<group>.` followed by an action.
    NotInGroup {
        /// The offending name.
        name: String,
        /// The group it was registered under.
        group: CommandGroup,
    },
    /// The name is already registered.
    Duplicate(String),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInGroup { name, group } => {
                write!(f, "command {name:?} is not in group {:?}", group.as_str())
            }
            Self::Duplicate(name) => write!(f, "command {name:?} is registered twice"),
        }
    }
}

impl std::error::Error for RegistryError {}

/// The set of registered commands, always kept in sorted order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandRegistry {
    commands: BTreeSet<CommandDescriptor>,
}

impl CommandRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `name` under `group`. The name must be `<group>.<action>`; duplicates are rejected.
    pub fn register(&mut self, group: CommandGroup, name: &str) -> Result<(), RegistryError> {
        let in_group = name
            .strip_prefix(group.as_str())
            .and_then(|rest| rest.strip_prefix('.'))
            .is_some_and(|action| !action.is_empty());
        if !in_group {
            return Err(RegistryError::NotInGroup {
                name: name.to_owned(),
                group,
            });
        }
        let descriptor = CommandDescriptor {
            group,
            name: name.to_owned(),
        };
        if self.commands.iter().any(|c| c.name == name) {
            return Err(RegistryError::Duplicate(name.to_owned()));
        }
        self.commands.insert(descriptor);
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
/// Fails when a Rust command lacks a TypeScript wrapper or the reverse. Both lists are sorted and de-duplicated.
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

/// Extracts the command names from a TypeScript group file: the string literals of the
/// `export const commandNames = [...]` array. A file without that declaration declares no commands.
pub fn parse_wrapper_names(source: &str) -> Vec<String> {
    const MARKER: &str = "export const commandNames";
    let Some(start) = source.find(MARKER) else {
        return Vec::new();
    };
    let rest = &source[start + MARKER.len()..];
    let Some(open) = rest.find('[') else {
        return Vec::new();
    };
    let Some(close) = rest[open..].find(']') else {
        return Vec::new();
    };
    let body = &rest[open + 1..open + close];
    let mut names = Vec::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        if c == '\'' || c == '"' {
            let literal: String = chars.by_ref().take_while(|&d| d != c).collect();
            names.push(literal);
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Reads `packages/contracts/src/commands/*.ts`. A missing directory means no wrappers yet. A wrapper file whose
    /// stem is not a known group, or that declares a command outside its own group, is itself a mismatch.
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
            let Some(group) = CommandGroup::from_name(stem) else {
                return Err(format!(
                    "wrapper file {stem}.ts is not a known command group"
                ));
            };
            let source = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            for name in parse_wrapper_names(&source) {
                if !name.starts_with(&format!("{}.", group.as_str())) {
                    return Err(format!("{name} is declared outside group {stem}"));
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
    fn registry_requires_the_group_prefix_and_rejects_duplicates() {
        let mut registry = CommandRegistry::new();
        assert!(registry.is_empty());
        registry
            .register(CommandGroup::Project, "project.import")
            .unwrap();
        registry
            .register(CommandGroup::Project, "project.trust.set")
            .unwrap();
        assert_eq!(
            registry.register(CommandGroup::Project, "settings.get"),
            Err(RegistryError::NotInGroup {
                name: "settings.get".into(),
                group: CommandGroup::Project
            })
        );
        assert!(
            registry
                .register(CommandGroup::Project, "project.")
                .is_err()
        );
        assert!(
            registry
                .register(CommandGroup::Project, "projectx.a")
                .is_err()
        );
        assert_eq!(
            registry.register(CommandGroup::Project, "project.import"),
            Err(RegistryError::Duplicate("project.import".into()))
        );
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn contract_test_passes_with_an_empty_registry_against_the_real_wrapper_directory() {
        let dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/contracts/src/commands");
        let wrappers = typescript_wrappers(&dir).expect("wrapper files are well formed");
        let registry = CommandRegistry::new();
        // The registry is empty until the owning tickets add commands, and so are the wrappers.
        assert_eq!(compare_contract(&registry, &wrappers), Ok(()));
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
    fn wrapper_scan_rejects_unknown_groups_and_cross_group_names() {
        let dir = std::env::temp_dir().join(format!("vela-domain-wrappers-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("run.ts"),
            "export const commandNames = ['run.create'] as const;",
        )
        .unwrap();
        assert_eq!(typescript_wrappers(&dir), Ok(vec!["run.create".to_owned()]));

        std::fs::write(
            dir.join("run.ts"),
            "export const commandNames = ['project.import'] as const;",
        )
        .unwrap();
        assert!(typescript_wrappers(&dir).is_err());

        std::fs::write(dir.join("run.ts"), "").unwrap();
        std::fs::write(dir.join("bogus.ts"), "").unwrap();
        assert!(typescript_wrappers(&dir).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
