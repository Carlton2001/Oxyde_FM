//! Measured high-water admission/stop policy. These are not hard process limits.
use std::{
    io,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
use windows::Win32::System::{
    ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX},
    SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
    Threading::{GetCurrentProcess, GetProcessHandleCount},
};

const PARENT_COMMIT: u64 = 2 * 1024 * 1024 * 1024;
const PARENT_HANDLES: u32 = 8192;
const WORKER_HANDLES: u32 = 4096;
const MIN_AVAILABLE: u64 = 512 * 1024 * 1024;
const SAMPLE_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug)]
struct Sample {
    parent_commit: u64,
    parent_handles: u32,
    available_physical: u64,
    available_commit: u64,
}
impl Sample {
    fn read() -> io::Result<Self> {
        let process = unsafe { GetCurrentProcess() };
        let mut counters = PROCESS_MEMORY_COUNTERS_EX::default();
        counters.cb = std::mem::size_of_val(&counters) as u32;
        let mut handles = 0;
        let mut memory = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        unsafe {
            GetProcessMemoryInfo(
                process,
                (&mut counters as *mut PROCESS_MEMORY_COUNTERS_EX).cast(),
                counters.cb,
            )?;
            GetProcessHandleCount(process, &mut handles)?;
            GlobalMemoryStatusEx(&mut memory)?;
        }
        Ok(Self {
            parent_commit: counters.PrivateUsage as u64,
            parent_handles: handles,
            available_physical: memory.ullAvailPhys,
            available_commit: memory.ullAvailPageFile,
        })
    }
    fn admitted(self) -> bool {
        self.parent_commit < PARENT_COMMIT
            && self.parent_handles < PARENT_HANDLES
            && self.available_physical >= MIN_AVAILABLE
            && self.available_commit >= MIN_AVAILABLE
    }
}
pub(super) fn check() -> io::Result<()> {
    type Cached = Option<(Instant, Result<Sample, String>)>;
    static SAMPLE: OnceLock<Mutex<Cached>> = OnceLock::new();
    let mut cached = SAMPLE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| io::Error::other("Resource pressure monitor unavailable"))?;
    if cached
        .as_ref()
        .is_none_or(|(time, _)| time.elapsed() >= SAMPLE_INTERVAL)
    {
        let sample = Sample::read().map_err(|error| error.to_string());
        if sample.as_ref().is_ok_and(|sample| !sample.admitted()) {
            log::warn!("Resource pressure suspends parser admission: {sample:?}");
        }
        *cached = Some((Instant::now(), sample));
    }
    match &cached.as_ref().expect("sample initialized").1 {
        Ok(sample) if sample.admitted() => Ok(()),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "System resource pressure stopped parser work",
        )),
        Err(error) => Err(io::Error::other(format!(
            "Resource pressure measurement failed: {error}"
        ))),
    }
}

pub(super) fn check_worker(process: windows::Win32::Foundation::HANDLE) -> io::Result<()> {
    use windows::Win32::{Foundation::WAIT_OBJECT_0, System::Threading::WaitForSingleObject};
    let mut handles = 0;
    if let Err(error) = unsafe { GetProcessHandleCount(process, &mut handles) } {
        // Exit can race the counter query; a confirmed exit owns no live budget.
        if unsafe { WaitForSingleObject(process, 0) } == WAIT_OBJECT_0 {
            return Ok(());
        }
        return Err(io::Error::other(format!(
            "Worker handle measurement failed: {error}"
        )));
    }
    if handles >= WORKER_HANDLES {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "Worker handle pressure stopped parser work",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "windows_resource_pressure/tests/policy.rs"]
mod tests;
