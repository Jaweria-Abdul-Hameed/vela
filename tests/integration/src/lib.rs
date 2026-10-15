//! Cross-crate Rust integration tests live in `tests/`. This library holds their shared support: the hostile-fixture loader (X01).
//!
//! Hostile fixtures are *data*. The loader reads text files listed in a manifest and refuses anything that could execute: unknown
//! extensions, a script or binary signature, control bytes, an oversized file, a file the manifest does not list. The suite then
//! passes the text through Vela's real components and never runs any command a fixture urges.

use std::path::{Path, PathBuf};

/// The largest fixture the loader accepts.
pub const MAX_FIXTURE_BYTES: u64 = 64 * 1024;

/// Extensions that are plain text.
const TEXT_EXTENSIONS: [&str; 3] = ["md", "txt", "json"];

/// One hostile text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hostile {
    /// The file name.
    pub file: String,
    /// `issue`, `agents` or `readme`.
    pub kind: String,
    /// The text.
    pub text: String,
    /// The commands the text urges someone to run (never run by the suite).
    pub commands: Vec<String>,
}

/// Where the shared hostile fixtures live.
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fixtures")
        .join("hostile")
}

fn refuse(file: &str, why: &str) -> String {
    format!("hostile fixture {file} refused: {why}")
}

/// Reads one fixture's text after checking it cannot execute.
pub fn read_text(dir: &Path, file: &str) -> Result<String, String> {
    let path = dir.join(file);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if !TEXT_EXTENSIONS.contains(&ext.as_str()) {
        return Err(refuse(file, "not a text extension (md, txt, json only)"));
    }
    let meta = std::fs::metadata(&path).map_err(|e| refuse(file, &e.to_string()))?;
    if meta.len() > MAX_FIXTURE_BYTES {
        return Err(refuse(file, "too large"));
    }
    let bytes = std::fs::read(&path).map_err(|e| refuse(file, &e.to_string()))?;
    if bytes.starts_with(b"#!") || bytes.starts_with(b"MZ") || bytes.starts_with(b"\x7fELF") {
        return Err(refuse(file, "starts like a script or an executable"));
    }
    if bytes.contains(&0) {
        return Err(refuse(file, "contains a NUL byte"));
    }
    String::from_utf8(bytes).map_err(|_| refuse(file, "not UTF-8 text"))
}

/// Loads every fixture in the manifest, and fails if the folder holds a file the manifest does not list.
pub fn load_all(dir: &Path) -> Result<Vec<Hostile>, String> {
    let manifest = read_text(dir, "manifest.json")?;
    let value: serde_json::Value =
        serde_json::from_str(&manifest).map_err(|e| format!("manifest: {e}"))?;
    let entries = value["fixtures"]
        .as_array()
        .ok_or("manifest has no fixtures list")?;
    let mut listed = vec!["manifest.json".to_owned()];
    let mut out = Vec::new();
    for e in entries {
        let file = e["file"].as_str().ok_or("fixture without a file name")?;
        listed.push(file.to_owned());
        out.push(Hostile {
            file: file.to_owned(),
            kind: e["kind"].as_str().unwrap_or("issue").to_owned(),
            text: read_text(dir, file)?,
            commands: e["commands"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|c| c.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
        });
    }
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !listed.contains(&name) {
            return Err(refuse(&name, "not listed in the manifest"));
        }
    }
    Ok(out)
}
