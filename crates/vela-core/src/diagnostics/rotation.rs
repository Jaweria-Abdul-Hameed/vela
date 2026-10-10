//! Size-capped log files (component C17: "log disk growth (size-capped rotation)").
//!
//! A stream writes to `<stem>.log`. When that file is full it becomes `<stem>.1.log`, the previous `<stem>.1.log`
//! becomes `<stem>.2.log`, and so on; the oldest file is deleted. No file ever exceeds
//! [`RotationLimits::max_file_bytes`], and at most [`RotationLimits::max_files`] rotated files are kept, so the stream
//! never occupies more than [`RotationLimits::total_bytes`].
//!
//! The cap is the invariant; when it cannot be kept by renaming, the oldest history is given up first and the newest
//! records last. A rotated file that cannot be moved (another program holds it open) is overwritten by the next one.
//! An active file that cannot be renamed is copied to `<stem>.1.log` and then emptied; only when that copy fails too
//! is it emptied without one. Files left by an earlier, larger configuration are deleted when the stream is opened.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

/// How much disk one log stream may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RotationLimits {
    /// The largest size of one log file, in bytes (at least 1).
    pub max_file_bytes: u64,
    /// How many rotated files are kept beside the active one. Zero keeps only the active file.
    pub max_files: u32,
}

impl RotationLimits {
    /// The most disk space the stream can occupy: the active file plus every rotated file, all full.
    pub const fn total_bytes(&self) -> u64 {
        self.max_file_bytes
            .saturating_mul(self.max_files as u64 + 1)
    }
}

/// 10 MiB per file and four rotated files: at most 50 MiB per stream.
impl Default for RotationLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: 10 * 1024 * 1024,
            max_files: 4,
        }
    }
}

/// The separate logs of `OBSERVABILITY.md` ("Logs"). Each is its own stream with its own cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogStream {
    /// The Vela core log.
    Core,
    /// Worker and agent streams.
    Agent,
    /// Command stdout and stderr.
    Command,
    /// Git operations.
    Git,
    /// Tracker operations.
    Tracker,
}

impl LogStream {
    /// Every stream.
    pub const ALL: [Self; 5] = [
        Self::Core,
        Self::Agent,
        Self::Command,
        Self::Git,
        Self::Tracker,
    ];

    /// The file name stem of the stream (`vela-core` for `vela-core.log`).
    pub const fn stem(self) -> &'static str {
        match self {
            Self::Core => "vela-core",
            Self::Agent => "vela-agent",
            Self::Command => "vela-command",
            Self::Git => "vela-git",
            Self::Tracker => "vela-tracker",
        }
    }
}

/// The files of one stream and the bookkeeping of the active one.
struct Rotating {
    directory: PathBuf,
    stem: &'static str,
    limits: RotationLimits,
    /// The active file, opened for appending. `None` only while a rotation is in progress or after one failed.
    file: Option<File>,
    /// The size of the active file.
    written: u64,
}

impl Rotating {
    fn open(directory: PathBuf, stem: &'static str, limits: RotationLimits) -> io::Result<Self> {
        if limits.max_file_bytes == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "max_file_bytes must be at least 1",
            ));
        }
        fs::create_dir_all(&directory)?;
        let mut rotating = Self {
            directory,
            stem,
            limits,
            file: None,
            written: 0,
        };
        rotating.enforce_limits_on_existing_files()?;
        rotating.open_active()?;
        Ok(rotating)
    }

    fn active_path(&self) -> PathBuf {
        self.directory.join(format!("{}.log", self.stem))
    }

    fn rotated_path(&self, index: u32) -> PathBuf {
        self.directory.join(format!("{}.{index}.log", self.stem))
    }

    /// The rotation index of `path` when it is one of this stream's rotated files.
    fn rotated_index(&self, path: &Path) -> Option<u32> {
        let name = path.file_name()?.to_str()?;
        let index = name
            .strip_prefix(self.stem)?
            .strip_prefix('.')?
            .strip_suffix(".log")?;
        if index.is_empty() || !index.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        index.parse().ok()
    }

    /// Deletes what an earlier configuration left behind and the current limits do not allow: rotated files beyond
    /// `max_files`, and any file larger than `max_file_bytes`.
    fn enforce_limits_on_existing_files(&self) -> io::Result<()> {
        let active = self.active_path();
        for entry in fs::read_dir(&self.directory)? {
            let entry = entry?;
            let path = entry.path();
            let index = self.rotated_index(&path);
            if index.is_none() && path != active {
                continue;
            }
            let too_many = index.is_some_and(|index| index == 0 || index > self.limits.max_files);
            let too_large = entry.metadata()?.len() > self.limits.max_file_bytes;
            if too_many || too_large {
                remove_if_present(&path)?;
            }
        }
        Ok(())
    }

    fn open_active(&mut self) -> io::Result<()> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.active_path())?;
        self.written = file.metadata()?.len();
        self.file = Some(file);
        Ok(())
    }

    /// Makes the active file the newest rotated file and starts an empty one.
    fn rotate(&mut self) -> io::Result<()> {
        // Close the active file first: Windows does not rename an open file.
        self.file = None;
        self.shift_files()?;
        self.open_active()
    }

    /// Moves every file one place along. Afterwards the active file is missing or empty, whatever failed on the way.
    fn shift_files(&self) -> io::Result<()> {
        let active = self.active_path();
        let keep = self.limits.max_files;
        if keep == 0 {
            return remove_if_present(&active).or_else(|_| empty(&active));
        }
        // Best effort from the oldest file down: a file that cannot be deleted or moved stays where it is and is
        // overwritten by the file that follows it, so a failure here costs old history, not new.
        let _ = remove_if_present(&self.rotated_path(keep));
        for index in (1..keep).rev() {
            let _ = rename_if_present(&self.rotated_path(index), &self.rotated_path(index + 1));
        }
        let newest = self.rotated_path(1);
        if rename_if_present(&active, &newest).is_ok() {
            return Ok(());
        }
        // The active file cannot be renamed (on Windows: another program has it open without sharing deletion). Keep
        // its records by copying them, then empty it in place; the copy is no larger than one file.
        let _ = fs::copy(&active, &newest);
        empty(&active)
    }

    fn write_record(&mut self, record: &[u8]) -> io::Result<()> {
        let max = self.limits.max_file_bytes;
        let mut rest = record;
        while !rest.is_empty() {
            if self.file.is_none() {
                self.open_active()?;
            }
            let room = max.saturating_sub(self.written);
            // A record that fits in one file is never split across two; only a record larger than a whole file is.
            let fits_in_an_empty_file = u64::try_from(rest.len()).is_ok_and(|len| len <= max);
            if room == 0 || (self.written > 0 && fits_in_an_empty_file && rest.len() as u64 > room)
            {
                self.rotate()?;
                continue;
            }
            let take = usize::try_from(room).map_or(rest.len(), |room| room.min(rest.len()));
            let Some(file) = self.file.as_mut() else {
                return Err(io::Error::other("the log file is not open"));
            };
            file.write_all(&rest[..take])?;
            self.written += take as u64;
            rest = &rest[take..];
        }
        Ok(())
    }

    fn total_bytes(&self) -> io::Result<u64> {
        let mut total = 0;
        for path in std::iter::once(self.active_path())
            .chain((1..=self.limits.max_files).map(|index| self.rotated_path(index)))
        {
            match fs::metadata(&path) {
                Ok(metadata) => total += metadata.len(),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        Ok(total)
    }
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// Truncates `path` in place, which works where renaming and deleting do not.
fn empty(path: &Path) -> io::Result<()> {
    OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .map(drop)
}

fn rename_if_present(from: &Path, to: &Path) -> io::Result<()> {
    match fs::rename(from, to) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// A size-capped, rotating log file. Clones share the file, so one handle can be given to every writer of a stream;
/// each `write` call is one record and is never interleaved with another.
#[derive(Clone)]
pub struct LogFile {
    shared: Arc<Mutex<Rotating>>,
}

impl LogFile {
    /// Opens (creating the directory and the file when missing) the log of `stream` inside `directory`.
    ///
    /// Existing files that the limits no longer allow are deleted, so the cap holds from the first write.
    pub fn open(
        directory: impl Into<PathBuf>,
        stream: LogStream,
        limits: RotationLimits,
    ) -> io::Result<Self> {
        let rotating = Rotating::open(directory.into(), stream.stem(), limits)?;
        Ok(Self {
            shared: Arc::new(Mutex::new(rotating)),
        })
    }

    /// The path of the active file.
    pub fn path(&self) -> PathBuf {
        self.lock().active_path()
    }

    /// The limits the file was opened with.
    pub fn limits(&self) -> RotationLimits {
        self.lock().limits
    }

    /// The bytes currently on disk for this stream: the active file plus its rotated files.
    pub fn total_bytes(&self) -> io::Result<u64> {
        self.lock().total_bytes()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Rotating> {
        // A writer that panicked mid-record leaves at worst a partial line; logging must keep working.
        self.shared.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Write for LogFile {
    fn write(&mut self, record: &[u8]) -> io::Result<usize> {
        (&*self).write(record)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&*self).flush()
    }
}

impl Write for &LogFile {
    fn write(&mut self, record: &[u8]) -> io::Result<usize> {
        self.lock().write_record(record)?;
        Ok(record.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.lock().file.as_mut() {
            Some(file) => file.flush(),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// A uniquely named directory under the OS temp directory, removed when dropped.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let name = format!(
                "vela-f07-{label}-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            );
            let path = std::env::temp_dir().join(name);
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn files(&self) -> Vec<(String, u64)> {
            let mut files: Vec<(String, u64)> = fs::read_dir(&self.0)
                .unwrap()
                .map(|entry| {
                    let entry = entry.unwrap();
                    (
                        entry.file_name().into_string().unwrap(),
                        entry.metadata().unwrap().len(),
                    )
                })
                .collect();
            files.sort();
            files
        }

        fn read(&self, name: &str) -> String {
            fs::read_to_string(self.0.join(name)).unwrap()
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn limits(max_file_bytes: u64, max_files: u32) -> RotationLimits {
        RotationLimits {
            max_file_bytes,
            max_files,
        }
    }

    #[test]
    fn rotation_caps_the_total_size() {
        let dir = TempDir::new("cap");
        let limits = limits(1_000, 3);
        let mut log = LogFile::open(&dir.0, LogStream::Core, limits).unwrap();
        assert_eq!(limits.total_bytes(), 4_000);

        let mut written = 0;
        for record in 0..2_000 {
            let line = format!(
                "{{\"record\":{record},\"padding\":\"{}\"}}\n",
                "x".repeat(record % 97)
            );
            log.write_all(line.as_bytes()).unwrap();
            written += line.len() as u64;

            let on_disk = log.total_bytes().unwrap();
            assert!(
                on_disk <= limits.total_bytes(),
                "{on_disk} bytes after record {record}"
            );
        }
        assert!(
            written > 20 * limits.total_bytes(),
            "the test must write far more than the cap"
        );

        let files = dir.files();
        let names: Vec<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(
            names,
            [
                "vela-core.1.log",
                "vela-core.2.log",
                "vela-core.3.log",
                "vela-core.log"
            ]
        );
        for (name, size) in &files {
            assert!(*size <= limits.max_file_bytes, "{name} is {size} bytes");
        }
        assert_eq!(
            files.iter().map(|(_, size)| size).sum::<u64>(),
            log.total_bytes().unwrap()
        );
        // The newest records survive, the oldest are gone, and no record was split across files.
        let newest = format!(
            "{{\"record\":1999,\"padding\":\"{}\"}}\n",
            "x".repeat(1999 % 97)
        );
        assert!(dir.read("vela-core.log").ends_with(&newest));
        for (name, _) in &files {
            let text = dir.read(name);
            assert!(
                text.starts_with("{\"record\":") && text.ends_with("}\n"),
                "{name}"
            );
            assert!(
                !text.contains("{\"record\":0,"),
                "{name} still holds the oldest record"
            );
        }
    }

    #[test]
    fn files_rotate_in_order() {
        let dir = TempDir::new("order");
        let mut log = LogFile::open(&dir.0, LogStream::Git, limits(10, 2)).unwrap();
        for record in ["aaaaaaa\n", "bbbbbbb\n", "ccccccc\n", "ddddddd\n"] {
            log.write_all(record.as_bytes()).unwrap();
        }
        assert_eq!(dir.read("vela-git.log"), "ddddddd\n");
        assert_eq!(dir.read("vela-git.1.log"), "ccccccc\n");
        assert_eq!(dir.read("vela-git.2.log"), "bbbbbbb\n");
        assert_eq!(dir.files().len(), 3);
        assert_eq!(log.path(), dir.0.join("vela-git.log"));
        assert_eq!(log.limits(), limits(10, 2));
    }

    #[test]
    fn a_record_larger_than_a_file_is_split_and_still_capped() {
        let dir = TempDir::new("huge");
        let limits = limits(100, 2);
        let mut log = LogFile::open(&dir.0, LogStream::Command, limits).unwrap();
        log.write_all(b"small\n").unwrap();
        let huge = "y".repeat(1_050);
        log.write_all(huge.as_bytes()).unwrap();
        assert!(log.total_bytes().unwrap() <= limits.total_bytes());
        for (name, size) in dir.files() {
            assert!(size <= limits.max_file_bytes, "{name} is {size} bytes");
        }
        // The tail of the record is what remains: two full rotated files and the last 56 bytes.
        assert_eq!(dir.read("vela-command.log"), "y".repeat(56));
        assert_eq!(dir.read("vela-command.1.log").len(), 100);
        assert_eq!(dir.read("vela-command.2.log").len(), 100);
    }

    #[test]
    fn keeping_no_rotated_files_keeps_only_the_active_file() {
        let dir = TempDir::new("none");
        let mut log = LogFile::open(&dir.0, LogStream::Agent, limits(16, 0)).unwrap();
        for record in ["0123456789\n", "abcdefghij\n", "klmnopqrst\n"] {
            log.write_all(record.as_bytes()).unwrap();
            assert!(log.total_bytes().unwrap() <= 16);
        }
        assert_eq!(dir.files(), [("vela-agent.log".to_owned(), 11)]);
        assert_eq!(dir.read("vela-agent.log"), "klmnopqrst\n");
    }

    #[test]
    fn reopening_appends_and_enforces_smaller_limits() {
        let dir = TempDir::new("reopen");
        {
            let mut log = LogFile::open(&dir.0, LogStream::Core, limits(20, 4)).unwrap();
            for record in [
                "one 3456789\n",
                "two 3456789\n",
                "three 56789\n",
                "four 456789\n",
                "five 456789\n",
            ] {
                log.write_all(record.as_bytes()).unwrap();
            }
            log.flush().unwrap();
        }
        assert_eq!(dir.files().len(), 5);
        fs::write(dir.0.join("vela-core.9.log"), "stale").unwrap();
        fs::write(dir.0.join("vela-core.0.log"), "stale").unwrap();
        fs::write(dir.0.join("vela-core.x.log"), "not ours").unwrap();
        fs::write(
            dir.0.join("vela-git.log"),
            "another stream, far larger than the limit",
        )
        .unwrap();

        // Same limits: the active file is appended to.
        let mut log = LogFile::open(&dir.0, LogStream::Core, limits(20, 4)).unwrap();
        log.write_all(b"six\n").unwrap();
        assert_eq!(dir.read("vela-core.log"), "five 456789\nsix\n");
        assert!(!dir.0.join("vela-core.9.log").exists());
        assert!(!dir.0.join("vela-core.0.log").exists());
        drop(log);

        // Smaller limits: files beyond the new count and files larger than the new size are deleted.
        let log = LogFile::open(&dir.0, LogStream::Core, limits(14, 1)).unwrap();
        assert!(log.total_bytes().unwrap() <= log.limits().total_bytes());
        let names: Vec<String> = dir.files().into_iter().map(|(name, _)| name).collect();
        assert_eq!(
            names,
            [
                "vela-core.1.log",
                "vela-core.log",
                "vela-core.x.log",
                "vela-git.log"
            ]
        );
        assert_eq!(dir.read("vela-core.log"), "");
        assert_eq!(dir.read("vela-core.x.log"), "not ours");
    }

    /// Another program holds the active file open without sharing deletion, so it cannot be renamed: the newest
    /// records must survive (copied to the first rotated file) and the cap must hold.
    #[cfg(windows)]
    #[test]
    fn an_active_file_that_cannot_be_renamed_is_copied_and_emptied() {
        use std::os::windows::fs::OpenOptionsExt;

        const FILE_SHARE_READ_WRITE: u32 = 0x1 | 0x2;

        let dir = TempDir::new("held");
        let limits = limits(20, 2);
        let mut log = LogFile::open(&dir.0, LogStream::Core, limits).unwrap();
        log.write_all(b"one 3456789\n").unwrap();
        log.write_all(b"two 3456789\n").unwrap();
        assert_eq!(dir.read("vela-core.1.log"), "one 3456789\n");

        let holder = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ_WRITE)
            .open(log.path())
            .unwrap();
        assert!(fs::rename(log.path(), dir.0.join("elsewhere.log")).is_err());

        log.write_all(b"three 56789\n").unwrap();
        log.write_all(b"four 456789\n").unwrap();
        drop(holder);

        assert_eq!(dir.read("vela-core.log"), "four 456789\n");
        assert_eq!(dir.read("vela-core.1.log"), "three 56789\n");
        assert_eq!(dir.read("vela-core.2.log"), "two 3456789\n");
        assert!(log.total_bytes().unwrap() <= limits.total_bytes());
        assert_eq!(dir.files().len(), 3);
    }

    /// A rotated file that cannot be moved costs its own (old) records only.
    #[cfg(windows)]
    #[test]
    fn a_rotated_file_that_cannot_be_moved_costs_only_old_history() {
        use std::os::windows::fs::OpenOptionsExt;

        const FILE_SHARE_READ_WRITE: u32 = 0x1 | 0x2;

        let dir = TempDir::new("stuck");
        let limits = limits(20, 2);
        let mut log = LogFile::open(&dir.0, LogStream::Core, limits).unwrap();
        for record in ["one 3456789\n", "two 3456789\n", "three 56789\n"] {
            log.write_all(record.as_bytes()).unwrap();
        }
        let holder = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ_WRITE)
            .open(dir.0.join("vela-core.2.log"))
            .unwrap();
        log.write_all(b"four 456789\n").unwrap();
        drop(holder);

        assert_eq!(dir.read("vela-core.log"), "four 456789\n");
        assert_eq!(dir.read("vela-core.1.log"), "three 56789\n");
        assert!(log.total_bytes().unwrap() <= limits.total_bytes());
        assert_eq!(dir.files().len(), 3);
    }

    #[test]
    fn clones_share_one_file_and_records_are_not_interleaved() {
        let dir = TempDir::new("shared");
        let log = LogFile::open(&dir.0, LogStream::Tracker, limits(1_000_000, 1)).unwrap();
        let threads: Vec<_> = (0..4)
            .map(|writer| {
                let mut log = log.clone();
                std::thread::spawn(move || {
                    for _ in 0..200 {
                        log.write_all(format!("{}\n", writer.to_string().repeat(40)).as_bytes())
                            .unwrap();
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        let text = dir.read("vela-tracker.log");
        assert_eq!(text.lines().count(), 800);
        for line in text.lines() {
            assert!(
                line.len() == 40 && line.bytes().all(|byte| byte == line.as_bytes()[0]),
                "{line}"
            );
        }
    }

    #[test]
    fn invalid_limits_and_stream_names() {
        let dir = TempDir::new("invalid");
        let error = LogFile::open(&dir.0, LogStream::Core, limits(0, 1))
            .err()
            .unwrap();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(limits(u64::MAX, 3).total_bytes(), u64::MAX);
        assert_eq!(RotationLimits::default().total_bytes(), 50 * 1024 * 1024);
        let mut stems: Vec<&str> = LogStream::ALL.iter().map(|stream| stream.stem()).collect();
        stems.dedup();
        assert_eq!(stems.len(), 5);
    }
}
