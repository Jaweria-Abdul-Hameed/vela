//! Projects (S01): import a repository folder, list recent projects, remove one from Vela, reopen it after a restart.
//!
//! Removing a project only marks Vela's own record: the repository, its files and its Git data are never touched. An invalid
//! folder is a typed error. Project identity (the head at import) comes from the Git port, so nothing here shells out.

use std::path::Path;
use std::sync::Arc;

use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::ids::{OperationId, WorktreePath};
use vela_domain::ports::{GitContext, GitRead, RepoTrust};
use vela_persistence::repositories::s01_projects::{self, Project};

use crate::orchestrator::Services;

/// A project as the UI sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectView {
    /// Identifier.
    pub id: String,
    /// Display name (the folder name).
    pub name: String,
    /// The repository path.
    pub repo_path: String,
    /// The branch checked out when it was imported.
    pub default_branch: Option<String>,
    /// The commit `HEAD` pointed at when it was imported.
    pub head_at_import: Option<String>,
    /// Last opened, epoch milliseconds.
    pub last_opened_ms: Option<i64>,
}

impl From<Project> for ProjectView {
    fn from(p: Project) -> Self {
        Self {
            id: p.id,
            name: p.name,
            repo_path: p.repo_path,
            default_branch: p.default_branch,
            head_at_import: p.head_at_import,
            last_opened_ms: p.last_opened_ms,
        }
    }
}

/// What an import returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportResult {
    /// The project.
    pub project: ProjectView,
    /// True when the repository was already known.
    pub already_known: bool,
}

/// The project service.
pub struct ProjectService {
    services: Arc<Services>,
    git: Arc<dyn GitRead>,
}

fn invalid(msg: &str) -> VelaError {
    VelaError::of_class(ErrorClass::UserActionRequired, msg)
        .with_state_preserved("nothing was imported")
        .with_user_action(
            "choose the folder that contains the repository (the one with a .git folder)",
        )
}

/// A path as a forward-slash string with no trailing separator.
fn normalized(path: &str) -> String {
    let p = path.replace('\\', "/");
    let trimmed = p.trim_end_matches('/');
    if trimmed.is_empty() {
        "/".to_owned()
    } else {
        trimmed.to_owned()
    }
}

impl ProjectService {
    /// A service over `services` that asks `git` about repositories.
    pub fn new(services: Arc<Services>, git: Arc<dyn GitRead>) -> Self {
        Self { services, git }
    }

    fn ctx(&self) -> GitContext {
        GitContext {
            // A repository is untrusted until the user trusts it; reading its status runs nothing of its own.
            trust: RepoTrust::Untrusted,
            operation: OperationId::new(format!("op-import-{}", self.services.ids.new_token()))
                .unwrap_or_else(|_| self.services.ids.new_operation_id()),
        }
    }

    /// Imports the repository at `folder`.
    pub async fn import(&self, folder: &str) -> Result<ImportResult, VelaError> {
        let path = normalized(folder.trim());
        if path.is_empty() || path == "/" {
            return Err(invalid("the folder path is empty"));
        }
        let fs_path = Path::new(&path);
        if !fs_path.is_dir() {
            return Err(invalid("that folder does not exist"));
        }
        let wt = WorktreePath::new(path.clone())
            .map_err(|_| invalid("that folder path is not usable"))?;
        let status = self
            .git
            .status(&self.ctx(), &wt)
            .await
            .map_err(|_| invalid("that folder is not a Git repository"))?;
        let name = fs_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project")
            .to_owned();
        let id = format!("proj-{}", self.services.ids.new_token());
        let (branch, head) = (
            status.branch.as_ref().map(|b| b.as_str().to_owned()),
            status.head.as_ref().map(|h| h.as_str().to_owned()),
        );
        let now = self.services.now_ms();
        let imported = self.services.store.write(move |tx| {
            s01_projects::import(
                tx.conn(),
                &id,
                &name,
                &path,
                None,
                branch.as_deref(),
                head.as_deref(),
                now,
            )
        })?;
        Ok(ImportResult {
            project: imported.project.into(),
            already_known: imported.existing,
        })
    }

    /// Recent projects, most recently opened first.
    pub fn list(&self) -> Result<Vec<ProjectView>, VelaError> {
        let rows = self
            .services
            .store
            .read(|c| s01_projects::list_active(c, 50))?;
        Ok(rows.into_iter().map(ProjectView::from).collect())
    }

    /// Removes the project from Vela. The repository is untouched. Removing an unknown project is an error.
    pub fn remove(&self, id: &str) -> Result<(), VelaError> {
        let (id_text, now) = (id.to_owned(), self.services.now_ms());
        let removed = self
            .services
            .store
            .write(move |tx| s01_projects::remove(tx.conn(), &id_text, now))?;
        if removed {
            Ok(())
        } else {
            Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "there is no such project in Vela",
            ))
        }
    }

    /// Opens a project (marks it the most recent). A project that is gone from the disk is reported, not hidden.
    pub fn open(&self, id: &str) -> Result<ProjectView, VelaError> {
        let (id_text, now) = (id.to_owned(), self.services.now_ms());
        let project = self
            .services
            .store
            .write(move |tx| {
                if !s01_projects::touch(tx.conn(), &id_text, now)? {
                    return Ok(None);
                }
                s01_projects::get(tx.conn(), &id_text)
            })?
            .ok_or_else(|| {
                VelaError::of_class(
                    ErrorClass::ValidationFailure,
                    "there is no such project in Vela",
                )
            })?;
        if !Path::new(&project.repo_path).is_dir() {
            return Err(invalid("the project folder no longer exists on disk"));
        }
        Ok(project.into())
    }
}

#[cfg(test)]
mod tests;
