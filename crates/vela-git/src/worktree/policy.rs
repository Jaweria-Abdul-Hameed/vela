//! The managed-root policy (G02): worktrees live only under one Vela-managed root, and that root must not be inside a
//! cloud-synced folder (a sync client rewriting files under a worktree corrupts Git state).

use std::path::{Component, Path, PathBuf};

use vela_domain::errors::{ErrorClass, VelaError};

/// Folder names that mark a synced location (matched as whole path components, case-insensitively).
const SYNC_COMPONENTS: &[(&str, &str)] = &[
    ("onedrive", "OneDrive"),
    ("dropbox", "Dropbox"),
    ("google drive", "Google Drive"),
    ("googledrive", "Google Drive"),
    ("icloud drive", "iCloud Drive"),
    ("iclouddrive", "iCloud Drive"),
    ("box", "Box"),
    ("box sync", "Box"),
    ("pcloud", "pCloud"),
    ("sync.com", "Sync.com"),
];

fn normalized(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|c| match c {
            Component::Normal(part) => Some(part.to_string_lossy().to_ascii_lowercase()),
            Component::Prefix(prefix) => {
                Some(prefix.as_os_str().to_string_lossy().to_ascii_lowercase())
            }
            _ => None,
        })
        .collect()
}

/// The sync provider whose folder `path` is inside, judged from its components (a component such as `OneDrive - Contoso`
/// counts).
pub fn synced_provider(path: &Path) -> Option<&'static str> {
    for component in normalized(path) {
        for (marker, name) in SYNC_COMPONENTS {
            if component == *marker || (marker == &"onedrive" && component.starts_with("onedrive"))
            {
                return Some(name);
            }
        }
    }
    None
}

/// Checks that `root` may host worktrees: absolute and not inside a synced folder.
pub fn check_root(root: &Path) -> Result<(), VelaError> {
    if !root.is_absolute() {
        return Err(VelaError::of_class(
            ErrorClass::ValidationFailure,
            "the worktree root must be an absolute path",
        ));
    }
    if let Some(provider) = synced_provider(root) {
        return Err(VelaError::of_class(
            ErrorClass::UserActionRequired,
            format!(
                "the worktree root is inside a {provider} folder; choose a folder that is not synced"
            ),
        ));
    }
    Ok(())
}

/// True when `path` is strictly below `root` (case-insensitive, separators ignored).
pub fn is_under(root: &Path, path: &Path) -> bool {
    let (r, p) = (normalized(root), normalized(path));
    p.len() > r.len() && p[..r.len()] == r[..]
}

/// The path as a forward-slash string, which Git accepts on Windows.
pub fn git_path(path: &Path) -> String {
    PathBuf::from(path).to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synced_folders_are_recognized_by_component() {
        for p in [
            "C:/Users/me/OneDrive/work",
            "C:/Users/me/OneDrive - Contoso/work",
            "D:/Dropbox/x",
            "C:/Users/me/Google Drive/x",
            "C:/Users/me/iCloudDrive/x",
        ] {
            assert!(synced_provider(Path::new(p)).is_some(), "{p}");
        }
        for p in [
            "C:/Users/me/work",
            "C:/Users/me/dropboxer/x",
            "C:/boxes/x",
            "C:/Vela/worktrees",
        ] {
            assert!(synced_provider(Path::new(p)).is_none(), "{p}");
        }
    }

    #[test]
    fn a_root_must_be_absolute_and_unsynced() {
        assert!(check_root(Path::new("C:/Vela/worktrees")).is_ok());
        assert!(check_root(Path::new("relative/dir")).is_err());
        let err = check_root(Path::new("C:/Users/me/OneDrive/worktrees")).unwrap_err();
        assert!(err.what_failed().contains("OneDrive"));
    }

    #[test]
    fn containment_ignores_case_and_separators_and_excludes_the_root_itself() {
        let root = Path::new("C:/Vela/Worktrees");
        assert!(is_under(root, Path::new("c:\\vela\\worktrees\\repo\\a")));
        assert!(!is_under(root, Path::new("C:/Vela/Worktrees")));
        assert!(!is_under(root, Path::new("C:/Vela/WorktreesEvil/a")));
        assert!(!is_under(root, Path::new("C:/Other/Worktrees/a")));
    }
}
