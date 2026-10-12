//! Shared helpers for the vela-git integration tests.
#![allow(dead_code)]

use std::path::Path;
use std::sync::Arc;

use vela_domain::ids::{OperationId, WorktreePath};
use vela_domain::ports::{GitContext, RepoTrust};
use vela_git::read::GitCli;
use vela_process::redact::PatternRedactor;
use vela_process::runner::{TokioProcessRunner, baseline_env};
use vela_process::tree::TreeRegistry;

pub fn git_cli() -> GitCli {
    let runner = TokioProcessRunner::new(
        Arc::new(PatternRedactor::new()),
        Arc::new(TreeRegistry::new()),
    );
    GitCli::new(Arc::new(runner), baseline_env())
}

pub fn ctx(trust: RepoTrust) -> GitContext {
    GitContext {
        trust,
        operation: OperationId::new("op-test").unwrap(),
    }
}

pub fn wt(path: &Path) -> WorktreePath {
    WorktreePath::new(path.to_string_lossy().replace('\\', "/")).unwrap()
}
