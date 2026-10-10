//! Synced-folder guard: refuses to place fixtures where a cloud-sync client would copy them.
//!
//! Fixture repositories are written and deleted in bulk, contain `.git` object stores, and (for the hostile-config
//! fixture) executable hooks. A sync client (OneDrive, Dropbox, Google Drive, iCloud, Box) would upload, lock, or
//! resurrect those files, which makes tests flaky and can leak fixture content. The guard classifies a path by its
//! components (case-insensitive, either separator, Windows 8.3 short names included) and also by the actual sync roots
//! the OS advertises through environment variables. It also refuses locations inside any Git repository, because
//! `DIRECTORY_STRUCTURE.md` forbids test repositories inside the Vela repository.

use std::fmt;
use std::path::{Path, PathBuf};

/// A cloud-sync product whose folders must never hold fixtures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyncProvider {
    /// Microsoft OneDrive (personal, business, and `OneDrive - <tenant>` folders).
    OneDrive,
    /// Dropbox.
    Dropbox,
    /// Google Drive (Drive for desktop `My Drive`, `Google Drive`, `GoogleDrive`).
    GoogleDrive,
    /// Apple iCloud Drive.
    ICloud,
    /// Box Sync.
    Box,
}

impl SyncProvider {
    /// Every provider the guard knows.
    pub const ALL: &'static [SyncProvider] = &[
        SyncProvider::OneDrive,
        SyncProvider::Dropbox,
        SyncProvider::GoogleDrive,
        SyncProvider::ICloud,
        SyncProvider::Box,
    ];

    /// Human-readable product name.
    pub const fn name(self) -> &'static str {
        match self {
            SyncProvider::OneDrive => "OneDrive",
            SyncProvider::Dropbox => "Dropbox",
            SyncProvider::GoogleDrive => "Google Drive",
            SyncProvider::ICloud => "iCloud Drive",
            SyncProvider::Box => "Box",
        }
    }
}

/// Why a fixture location was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardError {
    /// The location is inside a cloud-synced folder.
    Synced {
        /// The location that was offered.
        path: PathBuf,
        /// The sync product recognised.
        provider: SyncProvider,
    },
    /// The location is inside a Git repository (a `.git` entry exists in an ancestor).
    InsideRepository {
        /// The location that was offered.
        path: PathBuf,
        /// The ancestor that holds the `.git` entry.
        repository: PathBuf,
    },
}

impl fmt::Display for GuardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GuardError::Synced { path, provider } => write!(
                f,
                "refusing to create fixtures under {}: it is inside a {} synced folder",
                path.display(),
                provider.name()
            ),
            GuardError::InsideRepository { path, repository } => write!(
                f,
                "refusing to create fixtures under {}: it is inside the Git repository at {}",
                path.display(),
                repository.display()
            ),
        }
    }
}

impl std::error::Error for GuardError {}

/// Classifies one path component (a single directory or file name, not a path).
pub fn classify_component(component: &str) -> Option<SyncProvider> {
    let c = component.trim().to_lowercase();
    if c.is_empty() {
        return None;
    }
    // `OneDrive`, `OneDrive - Contoso`, `OneDriveConsumer`, `OneDriveTemp`; `ONEDRI~1` is the 8.3 short name.
    if c.starts_with("onedrive") || c.starts_with("onedri~") {
        return Some(SyncProvider::OneDrive);
    }
    if c.starts_with("dropbox") || c.starts_with("dropbo~") {
        return Some(SyncProvider::Dropbox);
    }
    if c.starts_with("google drive")
        || c.starts_with("googledrive")
        || c == "google-drive"
        || c == "my drive"
    {
        return Some(SyncProvider::GoogleDrive);
    }
    // iCloud Drive on Windows is `iCloudDrive`; on macOS the container is `Mobile Documents/com~apple~CloudDocs`.
    if c.starts_with("icloud") || c == "mobile documents" || c == "com~apple~clouddocs" {
        return Some(SyncProvider::ICloud);
    }
    if c.starts_with("box sync") {
        return Some(SyncProvider::Box);
    }
    None
}

/// Splits a path into lower-cased components, tolerating both separators and the `\\?\` verbatim prefix.
fn normalized_components(path: &Path) -> Vec<String> {
    let text = path.to_string_lossy();
    let text = text
        .strip_prefix(r"\\?\")
        .or_else(|| text.strip_prefix("//?/"))
        .unwrap_or(&text);
    text.split(['/', '\\'])
        .filter(|c| !c.is_empty() && *c != ".")
        .map(str::to_lowercase)
        .collect()
}

/// Classifies a path by its components alone (no filesystem access).
pub fn classify_path(path: &Path) -> Option<SyncProvider> {
    normalized_components(path)
        .iter()
        .find_map(|c| classify_component(c))
}

/// Classifies a path by its components and by explicit sync roots (a root and the provider it belongs to).
pub fn classify_path_with_roots(
    path: &Path,
    roots: &[(SyncProvider, PathBuf)],
) -> Option<SyncProvider> {
    if let Some(provider) = classify_path(path) {
        return Some(provider);
    }
    let comps = normalized_components(path);
    roots.iter().find_map(|(provider, root)| {
        let root = normalized_components(root);
        (!root.is_empty() && comps.starts_with(&root)).then_some(*provider)
    })
}

/// The sync roots the operating system advertises through environment variables (`OneDrive`, `OneDriveConsumer`,
/// `OneDriveCommercial`). A user may have renamed or relocated these folders, so the component check alone is not
/// enough.
pub fn sync_roots_from_env() -> Vec<(SyncProvider, PathBuf)> {
    ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"]
        .iter()
        .filter_map(|name| std::env::var_os(name))
        .filter(|value| !value.is_empty())
        .map(|value| (SyncProvider::OneDrive, PathBuf::from(value)))
        .collect()
}

/// The nearest ancestor of `path` (or `path` itself) that exists.
fn nearest_existing(path: &Path) -> Option<PathBuf> {
    path.ancestors().find(|p| p.exists()).map(Path::to_path_buf)
}

/// Checks a candidate parent directory for fixtures against the sync roots in `roots`.
///
/// `path` need not exist yet. A path that does exist is also resolved (symlinks and junctions followed) and
/// re-checked, so a link into a synced folder is refused too. The check is purely a refusal: nothing is created.
pub fn ensure_fixture_location_with_roots(
    path: &Path,
    roots: &[(SyncProvider, PathBuf)],
) -> Result<(), GuardError> {
    if let Some(provider) = classify_path_with_roots(path, roots) {
        return Err(GuardError::Synced {
            path: path.to_path_buf(),
            provider,
        });
    }
    let existing = nearest_existing(path);
    let resolved = existing.as_ref().and_then(|p| p.canonicalize().ok());
    if let Some(resolved) = &resolved
        && let Some(provider) = classify_path_with_roots(resolved, roots)
    {
        return Err(GuardError::Synced {
            path: path.to_path_buf(),
            provider,
        });
    }
    let start = resolved.or(existing);
    if let Some(start) = start
        && let Some(repository) = start.ancestors().find(|a| a.join(".git").exists())
    {
        return Err(GuardError::InsideRepository {
            path: path.to_path_buf(),
            repository: repository.to_path_buf(),
        });
    }
    Ok(())
}

/// [`ensure_fixture_location_with_roots`] with the sync roots taken from the environment.
pub fn ensure_fixture_location(path: &Path) -> Result<(), GuardError> {
    ensure_fixture_location_with_roots(path, &sync_roots_from_env())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(path: &str) -> Option<SyncProvider> {
        classify_path(Path::new(path))
    }

    #[test]
    fn onedrive_paths_are_classified() {
        for path in [
            r"C:\Users\Ada\OneDrive\Documents\repo",
            r"C:\Users\Ada\OneDrive - Contoso Ltd\Desktop",
            r"C:\Users\Ada\onedrive\x",
            r"C:\Users\Ada\OneDriveConsumer",
            r"C:\Users\Ada\ONEDRI~1\x",
            r"\\?\C:\Users\Ada\OneDrive\x",
            "C:/Users/Ada/OneDrive/Documents",
        ] {
            assert_eq!(provider(path), Some(SyncProvider::OneDrive), "{path}");
        }
    }

    #[test]
    fn dropbox_google_drive_icloud_and_box_paths_are_classified() {
        for path in [
            r"C:\Users\Ada\Dropbox\work",
            r"C:\Users\Ada\Dropbox (Personal)\work",
            "/home/ada/dropbox/work",
        ] {
            assert_eq!(provider(path), Some(SyncProvider::Dropbox), "{path}");
        }
        for path in [
            r"G:\My Drive\repos",
            r"C:\Users\Ada\Google Drive\repos",
            r"C:\Users\Ada\GoogleDrive\repos",
            "/Users/ada/Google Drive/My Drive/x",
        ] {
            assert_eq!(provider(path), Some(SyncProvider::GoogleDrive), "{path}");
        }
        for path in [
            r"C:\Users\Ada\iCloudDrive\repos",
            r"C:\Users\Ada\iCloud Drive\repos",
            "/Users/ada/Library/Mobile Documents/com~apple~CloudDocs/x",
        ] {
            assert_eq!(provider(path), Some(SyncProvider::ICloud), "{path}");
        }
        assert_eq!(
            provider(r"C:\Users\Ada\Box Sync\repos"),
            Some(SyncProvider::Box)
        );
    }

    #[test]
    fn ordinary_paths_are_not_classified() {
        for path in [
            r"C:\Users\Ada\AppData\Local\Temp\vela-fixture-1",
            r"C:\Temp\drive",
            r"D:\Projects\box",
            "/tmp/fixtures",
            "/var/folders/xx/T/one-drive-notes",
            "",
        ] {
            assert_eq!(provider(path), None, "{path}");
        }
    }

    #[test]
    fn a_component_match_needs_a_whole_component_not_a_substring() {
        assert_eq!(provider(r"C:\work\not-a-dropbox\x"), None);
        assert_eq!(provider(r"C:\work\my-onedrive-notes\x"), None);
    }

    #[test]
    fn every_provider_has_a_name_and_is_reachable() {
        for p in SyncProvider::ALL {
            assert!(!p.name().is_empty());
        }
        assert_eq!(SyncProvider::ALL.len(), 5);
    }

    #[test]
    fn explicit_roots_catch_a_renamed_sync_folder() {
        let roots = vec![(
            SyncProvider::OneDrive,
            PathBuf::from(r"D:\Cloud\Personal Files"),
        )];
        let inside = Path::new(r"d:\cloud\personal files\repos\x");
        assert_eq!(
            classify_path_with_roots(inside, &roots),
            Some(SyncProvider::OneDrive)
        );
        let sibling = Path::new(r"D:\Cloud\Personal Files Backup\x");
        assert_eq!(classify_path_with_roots(sibling, &roots), None);
        assert_eq!(
            classify_path_with_roots(Path::new(r"D:\Cloud"), &roots),
            None
        );
    }

    #[test]
    fn a_onedrive_location_is_rejected_without_touching_the_disk() {
        let path = Path::new(r"C:\Users\Nobody\OneDrive\Documents\vela-fixtures");
        match ensure_fixture_location_with_roots(path, &[]) {
            Err(GuardError::Synced { provider, .. }) => {
                assert_eq!(provider, SyncProvider::OneDrive)
            }
            other => panic!("expected a synced-folder refusal, got {other:?}"),
        }
        let message = ensure_fixture_location(path).unwrap_err().to_string();
        assert!(message.contains("OneDrive"), "{message}");
    }

    #[test]
    fn an_ordinary_temp_location_is_accepted() {
        let scratch = crate::fixture_repo::ScratchDir::create("guard-ok").unwrap();
        assert_eq!(
            ensure_fixture_location_with_roots(scratch.path(), &[]),
            Ok(())
        );
        // A not-yet-existing child of an acceptable directory is accepted too.
        assert_eq!(
            ensure_fixture_location_with_roots(&scratch.path().join("later").join("deeper"), &[]),
            Ok(())
        );
    }

    #[test]
    fn a_location_inside_a_git_repository_is_rejected() {
        let scratch = crate::fixture_repo::ScratchDir::create("guard-in-repo").unwrap();
        std::fs::create_dir(scratch.path().join(".git")).unwrap();
        let nested = scratch.path().join("sub");
        std::fs::create_dir(&nested).unwrap();
        assert!(matches!(
            ensure_fixture_location_with_roots(&nested, &[]),
            Err(GuardError::InsideRepository { .. })
        ));
    }
}
