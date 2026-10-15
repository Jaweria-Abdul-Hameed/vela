use std::collections::HashSet;
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use vela_domain::ids::{WorkerId, WorktreePath};
use vela_domain::policy::{
    CommandProfile, Dialect, HookRuleTable, PolicyContext, PolicyEngine, ProfileEntry, RunMode,
    TrustState,
};
use vela_domain::ports::{AgentEvent, AgentEventSink, AgentSession, TurnRequest};
use vela_testkit::fake_agy::FakeAgy;

use super::*;
use crate::antigravity::session::AgyDriver;

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "vela-profile-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn engine(trust: TrustState, commands: &[&str]) -> PolicyEngine {
    let root = "C:/Vela/worktrees/run-1/t1";
    let entries = commands
        .iter()
        .map(|c| ProfileEntry::parse(c, Dialect::PowerShell, None, root).unwrap())
        .collect();
    let ctx = PolicyContext::new(
        root,
        trust,
        RunMode::Autonomous,
        CommandProfile::new(entries).unwrap(),
    )
    .unwrap();
    PolicyEngine::new(ctx)
}

fn fake_hook_exe(dir: &Dir) -> PathBuf {
    let exe = dir.0.join("vela-hook-built.exe");
    std::fs::write(&exe, b"MZ fake hook binary").unwrap();
    exe
}

fn manager(dir: &Dir) -> ProfileManager {
    ProfileManager::new(dir.0.join("profiles"), fake_hook_exe(dir))
}

fn worker(n: &str) -> WorkerId {
    WorkerId::new(n).unwrap()
}

#[test]
fn generated_rules_allow_only_the_confirmed_commands_and_the_worktree_path() {
    let rules = native_allow_rules(&engine(
        TrustState::Trusted,
        &["git status --short", "npm test"],
    ));
    let settings = settings_json(&rules);
    let allow: Vec<&str> = settings["permissions"]["allow"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(allow.contains(&"command(git status --short)"), "{allow:?}");
    assert!(allow.contains(&"command(npm test)"), "{allow:?}");
    assert!(
        allow.contains(&"write_file(C:\\Vela\\worktrees\\run-1\\t1)"),
        "{allow:?}"
    );
    assert_eq!(
        allow.len(),
        3,
        "nothing else is allowed natively: {allow:?}"
    );
    assert!(
        allow
            .iter()
            .all(|r| !r.contains("regex:") && !r.contains('*')),
        "no broad patterns: {allow:?}"
    );
}

#[test]
fn an_untrusted_repository_gets_no_native_allow_at_all() {
    let settings = settings_json(&native_allow_rules(&engine(
        TrustState::Untrusted,
        &["git status"],
    )));
    assert_eq!(
        settings["permissions"]["allow"].as_array().unwrap().len(),
        0
    );
}

#[test]
fn entries_the_native_syntax_cannot_express_are_skipped_not_guessed() {
    let ambiguous = NativeAllowRule::Command {
        executable: "git".into(),
        args: vec!["commit".into(), "-m".into(), "two words".into()],
        cwd: "C:/x".into(),
    };
    assert_eq!(render_rule(&ambiguous), None);
    for bad in ["regex:evil", ""] {
        let rule = NativeAllowRule::Command {
            executable: bad.into(),
            args: vec![],
            cwd: "C:/x".into(),
        };
        assert_eq!(render_rule(&rule), None, "{bad:?}");
    }
    let paren = NativeAllowRule::Command {
        executable: "git".into(),
        args: vec!["log".into(), "(x)".into()],
        cwd: "C:/x".into(),
    };
    assert_eq!(render_rule(&paren), None);
}

#[test]
fn the_hook_command_is_relative_and_free_of_spaces_in_paths_and_quotes() {
    let hooks = hooks_json();
    let command = hooks["vela"]["PreToolUse"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    assert_eq!(
        command,
        "vela-hook.exe --table vela-table.json --spool spool"
    );
    assert!(!command.contains('"') && !command.contains('\\') && !command.contains(':'));
    assert_eq!(
        hooks["vela"]["PreToolUse"][0]["hooks"][0]["type"],
        "command"
    );
    assert_eq!(
        hooks["vela"]["PreToolUse"][0]["hooks"][0]["timeout"],
        HOOK_TIMEOUT_SECONDS
    );
}

#[test]
fn a_profile_holds_exactly_the_generated_files_and_points_the_child_at_itself() {
    let dir = Dir::new("create");
    let m = manager(&dir);
    let engine = engine(TrustState::Trusted, &["git status --short"]);
    let profile = m.create("run-1", &worker("w-1"), &engine).unwrap();

    let env = profile.env();
    assert_eq!(env.len(), 2);
    assert!(
        env.iter()
            .all(|(_, v)| profile.home == std::path::Path::new(v))
    );
    assert!(profile.home.starts_with(dir.0.join("profiles")));

    let mut files: Vec<String> = Vec::new();
    fn walk(base: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            let rel = p
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if p.is_dir() {
                out.push(format!("{rel}/"));
                walk(base, &p, out);
            } else {
                out.push(rel);
            }
        }
    }
    walk(&profile.home, &profile.home, &mut files);
    files.sort();
    assert_eq!(
        files,
        [
            ".gemini/",
            ".gemini/antigravity-cli/",
            ".gemini/antigravity-cli/settings.json",
            ".gemini/config/",
            ".gemini/config/hooks.json",
            ".gemini/config/spool/",
            ".gemini/config/vela-hook.exe",
            ".gemini/config/vela-table.json",
        ]
    );
    // The table the hook reads is the engine's table.
    let table: HookRuleTable =
        serde_json::from_slice(&std::fs::read(&profile.table_path).unwrap()).unwrap();
    assert_eq!(table, HookRuleTable::from_engine(&engine));
    // A policy file never sits inside the worktree, where a write-scoped allow would cover it.
    assert!(!profile.home.starts_with("C:/Vela/worktrees"));
}

#[test]
fn the_profile_never_copies_reads_or_references_credentials() {
    let dir = Dir::new("creds");
    // A stand-in for the user's real Antigravity store, with credential-looking files.
    let real_home = dir.0.join("real-home");
    let real_store = real_home.join(".gemini").join("antigravity-cli");
    std::fs::create_dir_all(&real_store).unwrap();
    std::fs::write(
        real_store.join("oauth_creds.json"),
        "{\"refresh_token\":\"SECRET-REFRESH\"}",
    )
    .unwrap();
    std::fs::write(real_home.join(".gemini").join("credentials"), "SECRET-CRED").unwrap();

    let m = manager(&dir);
    let profile = m
        .create(
            "run-1",
            &worker("w-1"),
            &engine(TrustState::Trusted, &["git status"]),
        )
        .unwrap();
    let mut all = String::new();
    for f in [
        &profile.settings_path,
        &profile.hooks_path,
        &profile.table_path,
    ] {
        all.push_str(&std::fs::read_to_string(f).unwrap());
    }
    assert!(
        !all.contains("SECRET") && !all.contains("refresh_token") && !all.contains("real-home")
    );
    assert!(
        !profile
            .home
            .join(".gemini")
            .join("antigravity-cli")
            .join("oauth_creds.json")
            .exists()
    );

    // The module itself has no way to find the real home directory.
    let source = include_str!("mod.rs");
    let production: String = source
        .split("#[cfg(test)]")
        .next()
        .unwrap()
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join(
            "
",
        );
    for banned in [
        "std::env::var",
        "home_dir",
        "env!(\"USERPROFILE\")",
        "oauth",
        "credential",
        "cookies",
    ] {
        assert!(
            !production.contains(banned),
            "the profile module must not touch {banned}"
        );
    }
}

#[test]
fn a_profile_is_removed_when_its_worker_ends_and_never_outlives_the_run() {
    let dir = Dir::new("lifecycle");
    let m = manager(&dir);
    let e = engine(TrustState::Trusted, &["git status"]);
    let a = m.create("run-1", &worker("w-1"), &e).unwrap();
    let b = m.create("run-1", &worker("w-2"), &e).unwrap();
    let c = m.create("run-2", &worker("w-3"), &e).unwrap();
    m.remove("run-1", &worker("w-1")).unwrap();
    assert!(!a.home.exists() && b.home.exists());
    m.remove("run-1", &worker("w-1")).unwrap();
    m.remove_run("run-1").unwrap();
    assert!(!b.home.exists());
    assert!(c.home.exists());
    // Recovery sweep: a run that is no longer active loses its profiles.
    let d = m.create("run-9", &worker("w-9"), &e).unwrap();
    let removed = m.sweep(&HashSet::from(["run-2".to_owned()])).unwrap();
    assert_eq!(removed, vec!["run-9".to_owned()]);
    assert!(!d.home.exists() && c.home.exists());
    // Creating again replaces an old profile.
    std::fs::write(c.home.join("stale.txt"), "x").unwrap();
    let again = m.create("run-2", &worker("w-3"), &e).unwrap();
    assert!(!again.home.join("stale.txt").exists());
}

#[test]
fn run_and_worker_ids_cannot_escape_the_profile_root() {
    let dir = Dir::new("escape");
    let m = manager(&dir);
    let e = engine(TrustState::Trusted, &["git status"]);
    for bad in ["", "..", ".hidden", "a/b", "a\\b", "c:", "x y"] {
        assert!(m.create(bad, &worker("w-1"), &e).is_err(), "{bad:?}");
    }
    assert!(m.remove_run("../outside").is_err());
}

#[derive(Default)]
struct Events(Mutex<Vec<AgentEvent>>);

impl AgentEventSink for Events {
    fn on_event(&self, event: AgentEvent) {
        self.0.lock().unwrap().push(event);
    }
}

#[tokio::test]
async fn the_driver_runs_each_workers_turn_inside_its_own_profile() {
    let dir = Dir::new("driver");
    let m = manager(&dir);
    let e = engine(TrustState::Trusted, &["git status"]);
    let fake = Arc::new(FakeAgy::new());
    let driver = AgyDriver::new(
        fake.clone(),
        vec![
            ("USERPROFILE".into(), "C:/real".into()),
            ("PATH".into(), "x".into()),
        ],
    );
    let w1 = worker("w-1");
    let w2 = worker("w-2");
    let p1 = m.create("run-1", &w1, &e).unwrap();
    driver.set_worker_env(w1.clone(), p1.env());
    let turn = |w: &WorkerId| TurnRequest {
        worker: w.clone(),
        worktree: WorktreePath::new("C:/Vela/worktrees/run-1/t1").unwrap(),
        prompt: "hi".to_owned(),
        conversation: None,
        timeout_ms: NonZeroU64::new(60_000).unwrap(),
    };
    driver
        .run_turn(&turn(&w1), &Events::default())
        .await
        .unwrap();
    driver
        .run_turn(&turn(&w2), &Events::default())
        .await
        .unwrap();
    let calls = fake.calls();
    assert_eq!(
        calls[0].userprofile.as_deref(),
        Some(p1.home.to_string_lossy().as_ref()),
        "w-1 runs in its profile"
    );
    assert_eq!(
        calls[1].userprofile.as_deref(),
        Some("C:/real"),
        "w-2 has no profile override"
    );
    driver.clear_worker_env(&w1);
    driver
        .run_turn(&turn(&w1), &Events::default())
        .await
        .unwrap();
    assert_eq!(fake.calls()[2].userprofile.as_deref(), Some("C:/real"));
}

#[test]
fn locate_names_the_same_files_create_writes_without_writing_anything() {
    let dir = Dir::new("locate");
    let m = manager(&dir);
    let w = WorkerId::new("w-1").unwrap();
    let located = m.locate("run-1", &w).unwrap();
    assert!(!located.home.exists(), "locating writes nothing");
    let created = m
        .create("run-1", &w, &engine(TrustState::Trusted, &["npm test"]))
        .unwrap();
    assert_eq!(located, created);
}
