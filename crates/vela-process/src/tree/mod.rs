//! Process-tree tracking and termination (P02, FR-021, FR-009; spike S-PROC-JOB, `docs/research/SPIKE_S-PROC-JOB.md`).
//!
//! On Windows every spawned tree is assigned to a Job Object with `KILL_ON_JOB_CLOSE`, so terminating (or dropping) the job
//! ends the root and every descendant, including PowerShell children. A [`TreeRegistry`] maps a caller-assigned process id to
//! its tree so cancellation, Stop All and reconciliation can find every running tree.
//!
//! Known window: a descendant spawned between `spawn` and job assignment is not in the job. The runner assigns immediately
//! after spawn; the window is a few microseconds and is recorded in the spike result.

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A set of processes that can be ended together.
#[derive(Debug)]
pub struct JobTree {
    #[cfg(windows)]
    handle: win::JobHandle,
    #[cfg(not(windows))]
    _private: (),
    root_pid: Mutex<Option<u32>>,
}

impl JobTree {
    /// Creates an empty job that kills its processes when the last handle closes.
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            #[cfg(windows)]
            handle: win::JobHandle::create()?,
            #[cfg(not(windows))]
            _private: (),
            root_pid: Mutex::new(None),
        })
    }

    /// Puts a freshly spawned child (and so everything it spawns later) into the job.
    pub fn assign(&self, child: &tokio::process::Child) -> io::Result<()> {
        *self.root_pid.lock().expect("pid lock") = child.id();
        #[cfg(windows)]
        {
            let raw = child
                .raw_handle()
                .ok_or_else(|| io::Error::other("the child has already exited"))?;
            self.handle.assign(raw)?;
        }
        Ok(())
    }

    /// The root process id, if known.
    pub fn root_pid(&self) -> Option<u32> {
        *self.root_pid.lock().expect("pid lock")
    }

    /// Number of processes still alive in the job (always 0 where jobs do not exist).
    pub fn active_processes(&self) -> io::Result<u32> {
        #[cfg(windows)]
        {
            self.handle.active_processes()
        }
        #[cfg(not(windows))]
        {
            Ok(0)
        }
    }

    /// Ends every process in the tree immediately.
    pub fn terminate(&self) -> io::Result<()> {
        #[cfg(windows)]
        {
            self.handle.terminate()
        }
        #[cfg(not(windows))]
        {
            Ok(())
        }
    }

    /// Asks the root to exit, waits up to `grace` for the whole tree to end, then terminates what is left.
    pub async fn terminate_gracefully(&self, grace: Duration) -> io::Result<()> {
        if !grace.is_zero() {
            #[cfg(windows)]
            if let Some(pid) = self.root_pid() {
                // `taskkill /T` without `/F` asks the tree to close politely.
                let _ = tokio::process::Command::new("taskkill")
                    .args(["/T", "/PID", &pid.to_string()])
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .await;
            }
            let deadline = tokio::time::Instant::now() + grace;
            while tokio::time::Instant::now() < deadline {
                if self.active_processes().unwrap_or(0) == 0 {
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
        self.terminate()
    }
}

/// One tracked tree and whether it was cancelled on purpose.
#[derive(Debug)]
pub struct TrackedTree {
    /// The tree.
    pub tree: JobTree,
    cancelled: AtomicBool,
}

impl TrackedTree {
    /// True once [`TreeRegistry::cancel`] was called for it.
    pub fn was_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

/// The trees currently running, by caller-assigned process id.
#[derive(Debug, Default)]
pub struct TreeRegistry {
    trees: Mutex<HashMap<String, Arc<TrackedTree>>>,
}

impl TreeRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a tree under `id`, replacing nothing: a duplicate id is an error.
    pub fn register(&self, id: &str, tree: JobTree) -> io::Result<Arc<TrackedTree>> {
        let mut map = self.trees.lock().expect("registry lock");
        if map.contains_key(id) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "a process with this id is already running",
            ));
        }
        let tracked = Arc::new(TrackedTree {
            tree,
            cancelled: AtomicBool::new(false),
        });
        map.insert(id.to_owned(), tracked.clone());
        Ok(tracked)
    }

    /// Forgets a finished tree.
    pub fn remove(&self, id: &str) {
        self.trees.lock().expect("registry lock").remove(id);
    }

    /// Ids of every tree still registered (for reconciliation after a crash or Stop All).
    pub fn running_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .trees
            .lock()
            .expect("registry lock")
            .keys()
            .cloned()
            .collect();
        ids.sort();
        ids
    }

    /// Marks `id` cancelled and ends its tree (gracefully for `grace`, then forced). An unknown id succeeds.
    pub async fn cancel(&self, id: &str, grace: Duration) -> io::Result<()> {
        let tracked = self.trees.lock().expect("registry lock").get(id).cloned();
        if let Some(t) = tracked {
            t.cancelled.store(true, Ordering::SeqCst);
            t.tree.terminate_gracefully(grace).await?;
        }
        Ok(())
    }
}

#[cfg(windows)]
mod win {
    use std::io;

    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
        QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
    };

    fn err(e: windows::core::Error) -> io::Error {
        io::Error::other(e.to_string())
    }

    #[derive(Debug)]
    pub(super) struct JobHandle(HANDLE);

    // SAFETY: a job handle is a kernel object handle; the Win32 job APIs used here are thread-safe.
    unsafe impl Send for JobHandle {}
    unsafe impl Sync for JobHandle {}

    impl JobHandle {
        pub(super) fn create() -> io::Result<Self> {
            // SAFETY: plain FFI calls with valid pointers to stack data of the documented types.
            unsafe {
                let handle = CreateJobObjectW(None, None).map_err(err)?;
                let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if let Err(e) = SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    std::ptr::addr_of!(info).cast(),
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                ) {
                    let _ = CloseHandle(handle);
                    return Err(err(e));
                }
                Ok(Self(handle))
            }
        }

        pub(super) fn assign(&self, process: std::os::windows::io::RawHandle) -> io::Result<()> {
            // SAFETY: `process` is a live process handle owned by the tokio child for the duration of the call.
            unsafe { AssignProcessToJobObject(self.0, HANDLE(process)).map_err(err) }
        }

        pub(super) fn active_processes(&self) -> io::Result<u32> {
            let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
            // SAFETY: the buffer is a correctly sized, writable accounting structure.
            unsafe {
                QueryInformationJobObject(
                    Some(self.0),
                    JobObjectBasicAccountingInformation,
                    std::ptr::addr_of_mut!(info).cast(),
                    std::mem::size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                    None,
                )
                .map_err(err)?;
            }
            Ok(info.ActiveProcesses)
        }

        pub(super) fn terminate(&self) -> io::Result<()> {
            // SAFETY: the handle is a valid job object for the lifetime of `self`.
            unsafe { TerminateJobObject(self.0, 1).map_err(err) }
        }
    }

    impl Drop for JobHandle {
        fn drop(&mut self) {
            // SAFETY: closing the handle we own; KILL_ON_JOB_CLOSE ends any remaining processes.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

#[cfg(all(test, windows))]
mod tests;
