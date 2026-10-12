use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use vela_testkit::fakes::FakeGit;

use super::*;
use crate::orchestrator::test_support::env;

struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir()
            .join(format!(
                "vela-proj-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::SeqCst)
            ))
            .join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("README.md"), "keep me").unwrap();
        Self(dir)
    }

    fn text(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
    }
}

fn service(e: &crate::orchestrator::test_support::TestEnv) -> ProjectService {
    ProjectService::new(e.services.clone(), std::sync::Arc::new(FakeGit::new()))
}

#[tokio::test]
async fn importing_lists_reopens_and_survives_a_restart() {
    let e = env("import");
    let folder = Folder::new("demo-app");
    let svc = service(&e);
    let imported = svc.import(&folder.text()).await.unwrap();
    assert!(!imported.already_known);
    assert_eq!(imported.project.name, "demo-app");
    assert_eq!(imported.project.default_branch.as_deref(), Some("main"));
    assert_eq!(
        imported.project.head_at_import.as_deref().map(str::len),
        Some(40)
    );
    assert_eq!(svc.list().unwrap().len(), 1);

    // After a restart the project is still there, and opening it moves it to the front.
    let reopened = ProjectService::new(e.restart(), std::sync::Arc::new(FakeGit::new()));
    let list = reopened.list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, imported.project.id);
    e.clock.advance(1000);
    assert!(
        reopened.open(&imported.project.id).unwrap().last_opened_ms
            > imported.project.last_opened_ms
    );
}

#[tokio::test]
async fn importing_the_same_repository_twice_keeps_one_project() {
    let e = env("twice");
    let folder = Folder::new("same");
    let svc = service(&e);
    let a = svc.import(&folder.text()).await.unwrap();
    let b = svc
        .import(&format!("{}/", folder.text().replace('\\', "/")))
        .await
        .unwrap();
    assert!(b.already_known);
    assert_eq!(a.project.id, b.project.id);
    assert_eq!(svc.list().unwrap().len(), 1);
}

#[tokio::test]
async fn removing_never_touches_the_repository_and_a_removed_project_can_be_imported_again() {
    let e = env("remove");
    let folder = Folder::new("keepme");
    let svc = service(&e);
    let p = svc.import(&folder.text()).await.unwrap().project;
    svc.remove(&p.id).unwrap();
    assert!(svc.list().unwrap().is_empty());
    assert_eq!(
        std::fs::read_to_string(folder.0.join("README.md")).unwrap(),
        "keep me",
        "the repository is untouched"
    );
    assert!(svc.remove(&p.id).is_err(), "removing twice is an error");
    assert!(
        svc.open(&p.id).is_err(),
        "a removed project cannot be opened"
    );
    let again = svc.import(&folder.text()).await.unwrap();
    assert!(again.already_known);
    assert_eq!(again.project.id, p.id);
    assert_eq!(svc.list().unwrap().len(), 1);
}

#[tokio::test]
async fn an_invalid_folder_is_a_clear_typed_error() {
    let e = env("invalid");
    let svc = service(&e);
    for bad in ["", "   ", "/", "C:/definitely/not/a/folder/xyz"] {
        let err = svc.import(bad).await.unwrap_err();
        assert_eq!(err.class(), ErrorClass::UserActionRequired, "{bad:?}");
        assert!(!err.what_failed().is_empty());
    }
    // A file is not a folder.
    let folder = Folder::new("file-holder");
    let file = folder.0.join("README.md");
    assert!(svc.import(&file.to_string_lossy()).await.is_err());
    assert!(svc.list().unwrap().is_empty());
}

#[tokio::test]
async fn a_project_whose_folder_vanished_is_reported_when_opened() {
    let e = env("vanished");
    let folder = Folder::new("gone");
    let svc = service(&e);
    let p = svc.import(&folder.text()).await.unwrap().project;
    std::fs::remove_dir_all(&folder.0).unwrap();
    let err = svc.open(&p.id).unwrap_err();
    assert!(err.what_failed().contains("no longer exists"));
}
