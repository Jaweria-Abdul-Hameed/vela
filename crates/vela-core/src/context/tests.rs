use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;
use crate::orchestrator::test_support::env;

struct Repo(PathBuf);

impl Repo {
    fn new() -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "vela-docs-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(dir.join("docs/adr")).unwrap();
        std::fs::create_dir_all(dir.join("docs/specs")).unwrap();
        std::fs::write(dir.join("AGENTS.md"), "agents").unwrap();
        std::fs::write(dir.join("README.md"), "readme").unwrap();
        std::fs::write(dir.join("src.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.join("docs/adr/ADR-001.md"), "adr").unwrap();
        std::fs::write(dir.join("docs/specs/SPEC.md"), "spec").unwrap();
        std::fs::write(dir.join("docs/specs/notes.txt"), "not markdown").unwrap();
        std::fs::write(dir.join("extra.md"), "extra").unwrap();
        Self(dir)
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn discovery_finds_canonical_markdown_only() {
    let r = Repo::new();
    let m = discover(&r.0, &[]);
    let paths: Vec<_> = m.entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "AGENTS.md",
            "README.md",
            "docs/adr/ADR-001.md",
            "docs/specs/SPEC.md"
        ]
    );
    let kind = |p: &str| m.entries.iter().find(|e| e.path == p).unwrap().kind;
    assert_eq!(kind("AGENTS.md"), DocKind::Agents);
    assert_eq!(kind("README.md"), DocKind::Overview);
    assert_eq!(kind("docs/adr/ADR-001.md"), DocKind::Adr);
    assert_eq!(kind("docs/specs/SPEC.md"), DocKind::Spec);
}

#[test]
fn fingerprints_change_with_content() {
    let r = Repo::new();
    let before = discover(&r.0, &[]);
    std::fs::write(r.0.join("AGENTS.md"), "agents changed").unwrap();
    let after = discover(&r.0, &[]);
    assert_ne!(before.entries[0].fingerprint, after.entries[0].fingerprint);
    assert_eq!(before.entries[1], after.entries[1]);
}

#[test]
fn additions_must_exist_and_stay_inside_the_repository() {
    let r = Repo::new();
    let adds = vec![
        "extra.md".to_owned(),
        "../outside.md".to_owned(),
        "missing.md".to_owned(),
        "C:/Windows/win.ini".to_owned(),
        "/etc/passwd".to_owned(),
    ];
    let m = discover(&r.0, &adds);
    assert!(
        m.entries
            .iter()
            .any(|e| e.path == "extra.md" && e.kind == DocKind::Addition)
    );
    assert_eq!(m.skipped_additions.len(), 4);
}

#[test]
fn text_is_read_only_for_manifest_files_and_is_truncated() {
    let r = Repo::new();
    let m = discover(&r.0, &[]);
    let t = read_untrusted(&r.0, &m, "AGENTS.md", 3).unwrap();
    assert_eq!(t.text, "age");
    assert!(read_untrusted(&r.0, &m, "src.rs", 100).is_err());
    assert!(read_untrusted(&r.0, &m, "../x.md", 100).is_err());
}

#[test]
fn the_manifest_is_recorded_with_additions_from_vela_settings_not_the_repository() {
    let e = env("manifest");
    e.seed_project();
    let r = Repo::new();
    // A file in the repository that claims to configure Vela has no effect.
    std::fs::write(r.0.join("docs/specs/vela.md"), "docs.additions = src.rs").unwrap();
    let m = record(&e.services, "p1", &r.0).unwrap();
    assert!(!m.entries.iter().any(|x| x.path == "src.rs"));
    e.services
        .store
        .write(|tx| {
            vela_persistence::repositories::s02_settings::set_project_setting(
                tx.conn(),
                "p1",
                ADDITIONS_KEY,
                "extra.md, nope.md",
                1,
            )
        })
        .unwrap();
    let m = record(&e.services, "p1", &r.0).unwrap();
    assert!(m.entries.iter().any(|x| x.path == "extra.md"));
    assert_eq!(m.skipped_additions, ["nope.md"]);
    assert_eq!(recorded(&e.services, "p1").unwrap().unwrap(), m);
    assert!(recorded(&e.services, "other").unwrap().is_none());
}
