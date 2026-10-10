//! Scratch directories: uniquely named temporary directories that are guard-checked and removed on drop.

use super::{FixtureError, guard};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A uniquely named directory under the OS temp directory (or an explicit parent) that is deleted when dropped.
///
/// Creation refuses synced folders and locations inside a Git repository ([`guard::ensure_fixture_location`]).
/// Removal is best effort and tolerates the read-only Git object files Windows leaves behind.
#[derive(Debug)]
pub struct ScratchDir {
    path: PathBuf,
}

impl ScratchDir {
    /// Creates a scratch directory under the OS temp directory. `label` is sanitised into the directory name.
    pub fn create(label: &str) -> Result<Self, FixtureError> {
        Self::create_in(&std::env::temp_dir(), label)
    }

    /// Creates a scratch directory inside `parent`, which must already exist.
    ///
    /// The guard runs first, so a refused location is never touched.
    pub fn create_in(parent: &Path, label: &str) -> Result<Self, FixtureError> {
        guard::ensure_fixture_location(parent)?;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let name = format!(
            "vela-{}-{}-{}-{}",
            sanitize_label(label),
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
            nanos
        );
        let path = parent.join(name);
        std::fs::create_dir(&path)
            .map_err(|source| FixtureError::io(format!("create {}", path.display()), source))?;
        Ok(Self { path })
    }

    /// The directory.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        remove_tree(&self.path);
    }
}

fn sanitize_label(label: &str) -> String {
    let cleaned: String = label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(40)
        .collect();
    if cleaned.is_empty() {
        "scratch".to_owned()
    } else {
        cleaned
    }
}

/// Removes a directory tree, retrying once after clearing read-only attributes. Never panics.
pub(crate) fn remove_tree(path: &Path) {
    if std::fs::remove_dir_all(path).is_ok() {
        return;
    }
    clear_readonly(path);
    let _ = std::fs::remove_dir_all(path);
}

// Making an entry writable is exactly the point here: the tree is about to be deleted.
#[allow(clippy::permissions_set_readonly_false)]
fn clear_readonly(path: &Path) {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return;
    };
    if meta.is_dir()
        && let Ok(entries) = std::fs::read_dir(path)
    {
        for entry in entries.flatten() {
            clear_readonly(&entry.path());
        }
    }
    let mut perms = meta.permissions();
    if perms.readonly() {
        perms.set_readonly(false);
        let _ = std::fs::set_permissions(path, perms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scratch_directory_is_unique_and_removed_on_drop() {
        let a = ScratchDir::create("unit").unwrap();
        let b = ScratchDir::create("unit").unwrap();
        assert_ne!(a.path(), b.path());
        let path = a.path().to_path_buf();
        assert!(path.is_dir());
        drop(a);
        assert!(!path.exists());
        assert!(b.path().is_dir());
    }

    #[test]
    fn removal_handles_read_only_files() {
        let scratch = ScratchDir::create("readonly").unwrap();
        let nested = scratch.path().join("objects").join("ab");
        std::fs::create_dir_all(&nested).unwrap();
        let file = nested.join("cdef");
        std::fs::write(&file, b"x").unwrap();
        let mut perms = std::fs::metadata(&file).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&file, perms).unwrap();
        let root = scratch.path().to_path_buf();
        drop(scratch);
        assert!(!root.exists());
    }

    #[test]
    fn labels_are_sanitised() {
        assert_eq!(sanitize_label("a b/c\\d"), "a_b_c_d");
        assert_eq!(sanitize_label(""), "scratch");
        assert_eq!(sanitize_label(&"x".repeat(100)).len(), 40);
    }

    #[test]
    fn a_synced_parent_is_refused_before_anything_is_created() {
        let parent = Path::new(r"C:\Users\Nobody\OneDrive\Documents");
        assert!(matches!(
            ScratchDir::create_in(parent, "x"),
            Err(FixtureError::Guard(_))
        ));
    }
}
