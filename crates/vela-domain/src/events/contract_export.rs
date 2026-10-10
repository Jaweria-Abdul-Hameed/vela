//! Generates the TypeScript contracts with `ts-rs` and checks them against the committed files.
//!
//! The generated files live in `packages/contracts/src/generated` and are committed (`SHARED_SURFACE_PROTOCOL.md`
//! section 4). `cargo test` runs the real `ts-rs` export for every contract type into a scratch directory and compares
//! the result with the committed files, so a Rust change that is not reflected in TypeScript fails the build, and the
//! TypeScript build (`tsc` in `packages/contracts`) type-checks the committed output.
//!
//! To regenerate after changing a contract type (also how a merge conflict in generated files is resolved, never by hand):
//!
//! ```text
//! VELA_UPDATE_CONTRACTS=1 cargo test -p vela-domain generated_typescript_contracts
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ts_rs::{Config, TS};

use super::{Event, EventEnvelope, EventKind};
use crate::errors::{ErrorClass, ErrorCode, VelaError};
use crate::ids::{DependencyEdge, GateResult, ReviewSeverity, RiskClass};

const UPDATE_ENV: &str = "VELA_UPDATE_CONTRACTS";

fn committed_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/contracts/src/generated")
}

/// Exports every contract type (and, recursively, everything they depend on) into `dir`.
fn export_all_into(dir: &Path) {
    let cfg = Config::new().with_out_dir(dir);
    // Reaches every payload, identifier, error and value-object type through the roots below.
    EventEnvelope::export_all(&cfg).expect("export EventEnvelope");
    Event::export_all(&cfg).expect("export Event");
    EventKind::export_all(&cfg).expect("export EventKind");
    VelaError::export_all(&cfg).expect("export VelaError");
    ErrorCode::export_all(&cfg).expect("export ErrorCode");
    ErrorClass::export_all(&cfg).expect("export ErrorClass");
    DependencyEdge::export_all(&cfg).expect("export DependencyEdge");
    GateResult::export_all(&cfg).expect("export GateResult");
    ReviewSeverity::export_all(&cfg).expect("export ReviewSeverity");
    RiskClass::export_all(&cfg).expect("export RiskClass");
}

/// Reads every `.ts` file of `dir` as `name -> contents` with line endings normalized to LF.
fn read_dir_normalized(dir: &Path) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return files;
    };
    for entry in entries {
        let path = entry.expect("read directory entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("ts") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&path).expect("read generated file");
        files.insert(name, text.replace("\r\n", "\n"));
    }
    files
}

#[test]
fn generated_typescript_contracts_match_the_committed_files() {
    let scratch =
        std::env::temp_dir().join(format!("vela-domain-contracts-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    export_all_into(&scratch);
    let generated = read_dir_normalized(&scratch);
    std::fs::remove_dir_all(&scratch).expect("remove the scratch directory");
    assert!(
        generated.len() > 50,
        "ts-rs exported only {} files; expected the whole contract",
        generated.len()
    );
    for kind in EventKind::ALL {
        assert!(
            generated.contains_key(&format!("{}Payload.ts", kind.as_str())),
            "no TypeScript payload type for {kind:?}"
        );
    }

    let committed_dir = committed_dir();
    if std::env::var_os(UPDATE_ENV).is_some() {
        std::fs::create_dir_all(&committed_dir).expect("create the generated directory");
        for stale in read_dir_normalized(&committed_dir).keys() {
            if !generated.contains_key(stale) {
                std::fs::remove_file(committed_dir.join(stale)).expect("remove a stale file");
            }
        }
        for (name, text) in &generated {
            std::fs::write(committed_dir.join(name), text).expect("write a generated file");
        }
        return;
    }

    let committed = read_dir_normalized(&committed_dir);
    let mut problems = Vec::new();
    let mut dump = String::new();
    for (name, text) in &generated {
        match committed.get(name) {
            None => problems.push(format!("missing: {name}")),
            Some(existing) if existing != text => problems.push(format!("out of date: {name}")),
            Some(_) => continue,
        }
        dump.push_str(&format!("@@@FILE {name}@@@\n{text}@@@END@@@\n"));
    }
    for name in committed.keys() {
        if !generated.contains_key(name) {
            problems.push(format!("stale (no longer generated): {name}"));
        }
    }
    assert!(
        problems.is_empty(),
        "packages/contracts/src/generated is not up to date with the Rust contract types:\n  {}\n\
         Regenerate with: {UPDATE_ENV}=1 cargo test -p vela-domain generated_typescript_contracts\n{dump}",
        problems.join("\n  ")
    );
}
