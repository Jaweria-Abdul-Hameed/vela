//! Documentation manifest (S05): finds the canonical Markdown of a repository and records what it found.
//!
//! The manifest carries paths and fingerprints only. Anything read from those files is [`UntrustedText`]: it is data for
//! the worker's context and has no way to become a setting, a trust decision or an approval rule (ADR-013). Project-specific
//! additions come from Vela's own project settings, never from the repository.

use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use vela_domain::errors::{ErrorClass, VelaError};
use vela_persistence::repositories::s02_settings;

use crate::orchestrator::Services;

/// The project-setting key the manifest is recorded under (Vela writes it; a repository cannot).
pub const MANIFEST_KEY: &str = "docs.manifest";
/// The project-setting key holding the user's additional documentation paths, comma separated.
pub const ADDITIONS_KEY: &str = "docs.additions";

const ROOT_FILES: &[&str] = &[
    "AGENTS.md",
    "CLAUDE.md",
    "CODEX.md",
    "GEMINI.md",
    "CONTEXT.md",
    "README.md",
];
const MAX_ENTRIES: usize = 400;
const MAX_DEPTH: usize = 6;

/// What a document is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocKind {
    /// Agent instructions.
    Agents,
    /// A decision record.
    Adr,
    /// A specification, constraint or other document under `docs`.
    Spec,
    /// A readme or context file.
    Overview,
    /// Added by the user for this project.
    Addition,
}

/// One document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocEntry {
    /// Path relative to the repository, with forward slashes.
    pub path: String,
    /// What it is.
    pub kind: DocKind,
    /// Size in bytes.
    pub bytes: u64,
    /// A fingerprint of the content (FNV-1a, 64 bit), to notice changes.
    pub fingerprint: String,
}

/// The documentation manifest of a repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Documents, sorted by path.
    pub entries: Vec<DocEntry>,
    /// Additions the user named that do not exist or are not allowed.
    pub skipped_additions: Vec<String>,
}

/// Text read from a repository. It is never a command, a setting or a policy input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UntrustedText {
    /// Where it came from.
    pub path: String,
    /// The text (lossily decoded, truncated to the requested size).
    pub text: String,
}

fn fnv1a(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn kind_of(rel: &str) -> DocKind {
    let lower = rel.to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or("");
    if matches!(name, "agents.md" | "claude.md" | "codex.md" | "gemini.md") {
        DocKind::Agents
    } else if lower.contains("/adr") || name.starts_with("adr-") {
        DocKind::Adr
    } else if lower.starts_with("docs/") {
        DocKind::Spec
    } else {
        DocKind::Overview
    }
}

fn entry(root: &Path, rel: &str, kind: Option<DocKind>) -> Option<DocEntry> {
    let data = std::fs::read(root.join(rel)).ok()?;
    Some(DocEntry {
        path: rel.to_owned(),
        kind: kind.unwrap_or_else(|| kind_of(rel)),
        bytes: data.len() as u64,
        fingerprint: fnv1a(&data),
    })
}

fn walk(root: &Path, dir: &str, depth: usize, out: &mut Vec<DocEntry>) {
    if depth > MAX_DEPTH || out.len() >= MAX_ENTRIES {
        return;
    }
    let Ok(read) = std::fs::read_dir(root.join(dir)) else {
        return;
    };
    let mut names: Vec<_> = read.filter_map(Result::ok).collect();
    names.sort_by_key(|e| e.file_name());
    for item in names {
        let name = item.file_name().to_string_lossy().into_owned();
        let rel = format!("{dir}/{name}");
        let Ok(kind) = item.file_type() else { continue };
        // Symbolic links are skipped: they could point outside the repository.
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            walk(root, &rel, depth + 1, out);
        } else if name.to_ascii_lowercase().ends_with(".md")
            && let Some(e) = entry(root, &rel, None)
        {
            out.push(e);
        }
    }
}

/// An addition is allowed when it is a relative path that stays inside the repository.
fn allowed_addition(path: &str) -> bool {
    let p = Path::new(path);
    !path.is_empty() && p.is_relative() && p.components().all(|c| matches!(c, Component::Normal(_)))
}

/// Discovers the manifest of the repository at `root`, plus the user's additions.
pub fn discover(root: &Path, additions: &[String]) -> Manifest {
    let mut entries = Vec::new();
    for name in ROOT_FILES {
        if let Some(e) = entry(root, name, None) {
            entries.push(e);
        }
    }
    walk(root, "docs", 0, &mut entries);
    let mut skipped = Vec::new();
    for add in additions {
        let rel = add.replace('\\', "/");
        let linked = root
            .join(&rel)
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);
        match (
            allowed_addition(&rel) && !linked,
            entry(root, &rel, Some(DocKind::Addition)),
        ) {
            (true, Some(e)) => {
                if let Some(existing) = entries.iter_mut().find(|x| x.path == e.path) {
                    existing.kind = DocKind::Addition;
                } else {
                    entries.push(e);
                }
            }
            _ => skipped.push(add.clone()),
        }
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries.dedup_by(|a, b| a.path == b.path);
    Manifest {
        entries,
        skipped_additions: skipped,
    }
}

/// Reads one manifest document as untrusted text, at most `max_bytes` of it. Paths outside the manifest are refused.
pub fn read_untrusted(
    root: &Path,
    manifest: &Manifest,
    path: &str,
    max_bytes: usize,
) -> Result<UntrustedText, VelaError> {
    if !manifest.entries.iter().any(|e| e.path == path) {
        return Err(VelaError::of_class(
            ErrorClass::ValidationFailure,
            "that file is not in the documentation manifest",
        ));
    }
    let data = std::fs::read(root.join(path))
        .map_err(|e| VelaError::internal(format!("cannot read {path}: {e}")))?;
    let cut = &data[..data.len().min(max_bytes)];
    Ok(UntrustedText {
        path: path.to_owned(),
        text: String::from_utf8_lossy(cut).into_owned(),
    })
}

/// Discovers and records the manifest for `project_id` (additions come from Vela's project settings).
pub fn record(services: &Services, project_id: &str, root: &Path) -> Result<Manifest, VelaError> {
    let pid = project_id.to_owned();
    let additions: Vec<String> = services
        .store
        .read(|c| s02_settings::get_project_setting(c, &pid, ADDITIONS_KEY))?
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let manifest = discover(root, &additions);
    let json = serde_json::to_string(&manifest).map_err(|e| VelaError::internal(e.to_string()))?;
    let (pid, now) = (project_id.to_owned(), services.now_ms());
    services.store.write(move |tx| {
        s02_settings::set_project_setting(tx.conn(), &pid, MANIFEST_KEY, &json, now)
    })?;
    Ok(manifest)
}

/// The manifest last recorded for `project_id`.
pub fn recorded(services: &Services, project_id: &str) -> Result<Option<Manifest>, VelaError> {
    let pid = project_id.to_owned();
    let text = services
        .store
        .read(|c| s02_settings::get_project_setting(c, &pid, MANIFEST_KEY))?;
    text.map(|t| {
        serde_json::from_str(&t)
            .map_err(|e| VelaError::internal(format!("manifest unreadable: {e}")))
    })
    .transpose()
}

#[cfg(test)]
mod tests;
