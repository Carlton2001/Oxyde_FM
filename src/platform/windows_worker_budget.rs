//! Common admission for all Oxyde parser workers, including runtime preparation.
//! Reservations are logical committed-memory envelopes, not measurements of RSS.
use std::io;
use std::os::windows::io::{AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::JobObjects::*;

pub(super) const MEMORY_BYTES: usize = 5 * 512 * 1024 * 1024;
const INTERACTIVE_RESERVE: usize = 512 * 1024 * 1024;
const MAX_WORKERS: usize = 4;
const ADMISSION_TIME: Duration = Duration::from_secs(30);

#[derive(Default)]
struct Usage {
    bytes: usize,
    workers: usize,
    large_bytes: usize,
    large_workers: usize,
}
struct Budget {
    usage: Mutex<Usage>,
    changed: Condvar,
    bytes: usize,
    workers: usize,
    reserve: usize,
}
impl Budget {
    #[cfg(test)]
    fn new(bytes: usize, workers: usize) -> Arc<Self> {
        Self::with_reserve(bytes, workers, 0)
    }
    fn with_reserve(bytes: usize, workers: usize, reserve: usize) -> Arc<Self> {
        Arc::new(Self {
            usage: Mutex::new(Usage::default()),
            changed: Condvar::new(),
            bytes,
            workers,
            reserve,
        })
    }
    fn acquire(
        self: &Arc<Self>,
        bytes: usize,
        checkpoint: &dyn Fn() -> io::Result<()>,
        timeout: Duration,
    ) -> io::Result<Permit> {
        if bytes == 0 || bytes > self.bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid worker reservation",
            ));
        }
        let started = Instant::now();
        let large = self.reserve != 0 && bytes > self.reserve;
        loop {
            // Never call arbitrary caller code under the accounting lock.
            checkpoint()?;
            let mut usage = self
                .usage
                .lock()
                .map_err(|_| io::Error::other("Worker budget poisoned"))?;
            let leaves_interactive_capacity = !large
                || (usage.large_workers < self.workers - 1
                    && bytes <= (self.bytes - self.reserve).saturating_sub(usage.large_bytes));
            if usage.workers < self.workers
                && bytes <= self.bytes - usage.bytes
                && leaves_interactive_capacity
            {
                usage.workers += 1;
                usage.bytes += bytes;
                if large {
                    usage.large_bytes += bytes;
                    usage.large_workers += 1;
                }
                return Ok(Permit {
                    budget: self.clone(),
                    bytes,
                    large,
                });
            }
            if started.elapsed() >= timeout {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "Global worker budget is busy",
                ));
            }
            let _ = self
                .changed
                .wait_timeout(usage, Duration::from_millis(10))
                .map_err(|_| io::Error::other("Worker budget poisoned"))?;
        }
    }
}
pub(super) struct Permit {
    budget: Arc<Budget>,
    bytes: usize,
    large: bool,
}
impl Drop for Permit {
    fn drop(&mut self) {
        let mut usage = self.budget.usage.lock().unwrap_or_else(|p| p.into_inner());
        usage.bytes -= self.bytes;
        usage.workers -= 1;
        if self.large {
            usage.large_bytes -= self.bytes;
            usage.large_workers -= 1;
        }
        self.budget.changed.notify_all();
    }
}

pub(super) fn acquire(bytes: usize, checkpoint: &dyn Fn() -> io::Result<()>) -> io::Result<Permit> {
    static BUDGET: OnceLock<Arc<Budget>> = OnceLock::new();
    BUDGET
        .get_or_init(|| Budget::with_reserve(MEMORY_BYTES, MAX_WORKERS, INTERACTIVE_RESERVE))
        .acquire(
            bytes,
            &|| {
                checkpoint()?;
                super::windows_resource_pressure::check()
            },
            ADMISSION_TIME,
        )
}

fn create_job() -> io::Result<OwnedHandle> {
    create_job_with_limits(MEMORY_BYTES, MAX_WORKERS)
}
fn create_job_with_limits(bytes: usize, workers: usize) -> io::Result<OwnedHandle> {
    let raw = unsafe { CreateJobObjectW(None, None) }.map_err(native)?;
    let job = unsafe { OwnedHandle::from_raw_handle(raw.0) };
    let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_JOB_MEMORY
        | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
        | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    info.BasicLimitInformation.ActiveProcessLimit = workers as u32;
    info.JobMemoryLimit = bytes;
    unsafe {
        SetInformationJobObject(
            raw,
            JobObjectExtendedLimitInformation,
            (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&info) as u32,
        )
    }
    .map_err(native)?;
    let cpu = JOBOBJECT_CPU_RATE_CONTROL_INFORMATION {
        ControlFlags: JOB_OBJECT_CPU_RATE_CONTROL_ENABLE | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP,
        Anonymous: JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0 { CpuRate: 7000 },
    };
    unsafe {
        SetInformationJobObject(
            raw,
            JobObjectCpuRateControlInformation,
            (&cpu as *const JOBOBJECT_CPU_RATE_CONTROL_INFORMATION).cast(),
            std::mem::size_of_val(&cpu) as u32,
        )
    }
    .map_err(native)?;
    Ok(job)
}
fn native(error: windows::core::Error) -> io::Error {
    io::Error::other(error.to_string())
}

/// Attach the shared root FIRST, then the per-worker job. Failure is terminal.
/// The caller must still hold its start gate (or suspended initial thread).
pub(super) fn attach(process: HANDLE) -> io::Result<()> {
    static JOB: OnceLock<Result<OwnedHandle, String>> = OnceLock::new();
    let job = JOB
        .get_or_init(|| create_job().map_err(|error| error.to_string()))
        .as_ref()
        .map_err(|error| io::Error::other(error.clone()))?;
    unsafe { AssignProcessToJobObject(HANDLE(job.as_raw_handle()), process) }.map_err(native)
}

/// At most MAX_WORKERS reapers can exist: each retains its admission slot until
/// Windows confirms exit. Failure to start a reaper conserves the reservation.
pub(super) fn retain_until_exit(
    process: HANDLE,
    permit: Permit,
    cleanup: impl FnOnce() + Send + 'static,
) {
    use windows::Win32::Foundation::WAIT_OBJECT_0;
    use windows::Win32::System::Threading::WaitForSingleObject;
    let handle = unsafe { BorrowedHandle::borrow_raw(process.0) }.try_clone_to_owned();
    let Ok(handle) = handle else {
        log::warn!("Cannot retain delayed worker handle; its budget remains reserved");
        std::mem::forget(permit);
        std::mem::forget(cleanup);
        return;
    };
    let pending = Arc::new(Mutex::new(Some((handle, permit, cleanup))));
    let retained = pending.clone();
    if let Err(error) = std::thread::Builder::new()
        .name("worker-budget-reaper".into())
        .spawn(move || {
            let (handle, _permit, cleanup) = retained
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .take()
                .unwrap();
            loop {
                if unsafe { WaitForSingleObject(HANDLE(handle.as_raw_handle()), 1000) }
                    == WAIT_OBJECT_0
                {
                    break;
                }
            }
            cleanup();
        })
    {
        log::warn!("Cannot supervise delayed worker: {error}; its budget remains reserved");
        std::mem::forget(pending);
    }
}

#[cfg(test)]
#[path = "windows_worker_budget/tests/admission.rs"]
mod tests;
