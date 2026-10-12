//! Command groups and the command registry (`SHARED_SURFACE_PROTOCOL.md` section 4, F04).
//!
//! One module per command group, each pre-declared here by F01. A group module exposes `COMMANDS` (the dotted
//! names it registers) and `handle` (runs a command if the group owns it). The registry and the dispatcher are
//! assembled from all fifteen groups in sorted order, so a ticket that adds a command group edits only that group's
//! own file (plus its contract wrapper and permission file), never this file or another group's.
//!
//! Tauri addresses an application command by a Rust function name, so a command's wire name is its dotted registry
//! name with `.` replaced by `_` (`diagnostics.ping` is `diagnostics_ping`), and its group is the first segment.

pub mod approval;
pub mod diagnostics;
pub mod evidence;
pub mod graph;
pub mod intervention;
pub mod policy;
pub mod preflight;
pub mod project;
pub mod run;
pub mod settings;
pub mod snapshot;
pub mod ticket;
pub mod timeline;
pub mod trust;
pub mod update;

use tauri::{Builder, Runtime, ipc::Invoke};
use vela_domain::events::command_registry::{CommandGroup, CommandRegistry};

/// The IPC (wire) name of a dotted command name.
pub fn wire_name(name: &str) -> String {
    name.replace('.', "_")
}

/// The group a wire command name belongs to (its first segment), if it names one.
pub fn group_of(wire: &str) -> Option<CommandGroup> {
    CommandGroup::from_name(wire.split('_').next()?)
}

/// The dotted command names a group registers.
pub fn group_commands(group: CommandGroup) -> &'static [&'static str] {
    match group {
        CommandGroup::Approval => approval::COMMANDS,
        CommandGroup::Diagnostics => diagnostics::COMMANDS,
        CommandGroup::Evidence => evidence::COMMANDS,
        CommandGroup::Graph => graph::COMMANDS,
        CommandGroup::Intervention => intervention::COMMANDS,
        CommandGroup::Policy => policy::COMMANDS,
        CommandGroup::Preflight => preflight::COMMANDS,
        CommandGroup::Project => project::COMMANDS,
        CommandGroup::Run => run::COMMANDS,
        CommandGroup::Settings => settings::COMMANDS,
        CommandGroup::Snapshot => snapshot::COMMANDS,
        CommandGroup::Ticket => ticket::COMMANDS,
        CommandGroup::Timeline => timeline::COMMANDS,
        CommandGroup::Trust => trust::COMMANDS,
        CommandGroup::Update => update::COMMANDS,
    }
}

/// The registry of every registered command, assembled from the group modules in sorted order.
///
/// # Panics
///
/// If a group registers an invalid or duplicate name: a programming error caught by the tests.
pub fn registry() -> CommandRegistry {
    let mut registry = CommandRegistry::new();
    for group in CommandGroup::ALL {
        for name in group_commands(group) {
            registry
                .register(group, name)
                .unwrap_or_else(|error| panic!("invalid command registration: {error}"));
        }
    }
    registry
}

/// Routes an invoke to the group that owns the command. Returns `false` for a command no group owns, which Tauri
/// reports to the renderer as "command not found".
pub fn dispatch<R: Runtime>(invoke: Invoke<R>) -> bool {
    let Some(group) = group_of(invoke.message.command()) else {
        return false;
    };
    match group {
        CommandGroup::Approval => approval::handle(invoke),
        CommandGroup::Diagnostics => diagnostics::handle(invoke),
        CommandGroup::Evidence => evidence::handle(invoke),
        CommandGroup::Graph => graph::handle(invoke),
        CommandGroup::Intervention => intervention::handle(invoke),
        CommandGroup::Policy => policy::handle(invoke),
        CommandGroup::Preflight => preflight::handle(invoke),
        CommandGroup::Project => project::handle(invoke),
        CommandGroup::Run => run::handle(invoke),
        CommandGroup::Settings => settings::handle(invoke),
        CommandGroup::Snapshot => snapshot::handle(invoke),
        CommandGroup::Ticket => ticket::handle(invoke),
        CommandGroup::Timeline => timeline::handle(invoke),
        CommandGroup::Trust => trust::handle(invoke),
        CommandGroup::Update => update::handle(invoke),
    }
}

/// Installs the dispatcher as the application's only invoke handler.
pub fn install<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    builder.invoke_handler(dispatch::<R>)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};
    use vela_domain::events::command_registry::{
        compare_contract, is_valid_command_name, parse_wrapper_names,
    };

    fn manifest_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    /// The command names declared by `packages/contracts/src/commands/<group>.ts`. Only files named after a group
    /// count; a dotted stem (`diagnostics.test.ts`) is a helper file.
    fn typescript_command_names(dir: &Path) -> Vec<String> {
        let mut names = Vec::new();
        for entry in std::fs::read_dir(dir).expect("packages/contracts/src/commands exists") {
            let path = entry.expect("directory entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("ts") {
                continue;
            }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if stem.contains('.') {
                continue;
            }
            assert!(
                CommandGroup::from_name(stem).is_some(),
                "{stem}.ts is not a known command group"
            );
            let source = std::fs::read_to_string(&path).expect("read the wrapper");
            names.extend(parse_wrapper_names(&source));
        }
        names
    }

    #[test]
    fn the_registry_holds_the_registered_commands_in_group_order() {
        let registry = registry();
        let commands: Vec<(CommandGroup, &str)> = registry
            .commands()
            .map(|c| (c.group, c.name.as_str()))
            .collect();
        assert_eq!(
            commands,
            [
                (CommandGroup::Diagnostics, "diagnostics.ping"),
                (CommandGroup::Snapshot, "snapshot.get"),
            ]
        );
    }

    #[test]
    fn every_command_is_valid_unique_and_filed_under_its_own_group() {
        let mut wires = BTreeSet::new();
        for group in CommandGroup::ALL {
            for name in group_commands(group) {
                assert!(is_valid_command_name(name), "{name}");
                assert_eq!(
                    name.split('.').next(),
                    Some(group.as_str()),
                    "{name} is registered by the wrong group"
                );
                assert!(
                    wires.insert(wire_name(name)),
                    "{name} has a duplicate wire name"
                );
                assert_eq!(group_of(&wire_name(name)), Some(group));
            }
        }
    }

    #[test]
    fn wire_names_and_groups_follow_the_naming_rule() {
        assert_eq!(wire_name("diagnostics.ping"), "diagnostics_ping");
        assert_eq!(wire_name("project.trust.set"), "project_trust_set");
        assert_eq!(group_of("run_stop_all"), Some(CommandGroup::Run));
        assert_eq!(group_of("update_can_apply"), Some(CommandGroup::Update));
        assert_eq!(group_of("ping"), None);
        assert_eq!(group_of("plugin:event|listen"), None);
        assert_eq!(group_of(""), None);
    }

    #[test]
    fn the_rust_registry_matches_the_typescript_wrappers() {
        let dir = manifest_dir().join("../../../packages/contracts/src/commands");
        let typescript = typescript_command_names(&dir);
        assert!(
            !typescript.is_empty(),
            "no TypeScript command wrappers found"
        );
        compare_contract(&registry(), &typescript).expect("the contract test passes");
    }

    #[test]
    fn the_contract_test_detects_drift_in_either_direction() {
        let registry = registry();
        let extra = vec![
            "diagnostics.ping".to_owned(),
            "diagnostics.export".to_owned(),
        ];
        let mismatch = compare_contract(&registry, &extra).unwrap_err();
        assert_eq!(mismatch.missing_in_rust, ["diagnostics.export"]);
        let mismatch = compare_contract(&registry, &[]).unwrap_err();
        assert_eq!(mismatch.missing_in_typescript, ["diagnostics.ping"]);
    }

    /// The capability files grant exactly the registered commands (through their generated permission identifiers)
    /// plus event listening: a command that is not registered cannot be granted, and a registered command that is
    /// not granted cannot be called.
    #[test]
    fn capabilities_grant_exactly_the_registered_commands_and_event_listening() {
        let dir = manifest_dir().join("capabilities");
        let mut granted = BTreeSet::new();
        let mut files = 0;
        for entry in std::fs::read_dir(&dir).expect("capabilities directory") {
            let path = entry.expect("directory entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            files += 1;
            let capability: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            assert_eq!(capability["windows"], serde_json::json!(["main"]));
            assert!(
                capability.get("remote").is_none(),
                "no remote origin may be granted"
            );
            for permission in capability["permissions"].as_array().unwrap() {
                granted.insert(permission.as_str().unwrap().to_owned());
            }
        }
        assert_eq!(files, 1, "one capability file per surface");

        let mut expected: BTreeSet<String> = BTreeSet::from([
            "core:event:allow-listen".to_owned(),
            "core:event:allow-unlisten".to_owned(),
        ]);
        for command in registry().commands() {
            expected.insert(format!(
                "allow-{}",
                wire_name(&command.name).replace('_', "-")
            ));
        }
        assert_eq!(granted, expected);
    }

    #[test]
    fn the_content_security_policy_is_deny_by_default_with_no_remote_source() {
        let path = manifest_dir().join("tauri.conf.json");
        let config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let csp = config["app"]["security"]["csp"]
            .as_object()
            .expect("a CSP object");
        assert_eq!(csp["default-src"], "'none'");
        for (directive, value) in csp {
            let value = value.as_str().unwrap();
            assert!(!value.contains('*'), "{directive}: wildcard");
            assert!(!value.contains("unsafe-"), "{directive}: unsafe source");
            for source in value.split_whitespace() {
                assert!(
                    !(source.starts_with("http") && source != "http://ipc.localhost"),
                    "{directive}: remote source {source}"
                );
            }
        }
        assert_eq!(config["bundle"], serde_json::json!({}));
        assert_eq!(config["plugins"], serde_json::json!({}));
    }
}
