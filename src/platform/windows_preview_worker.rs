//! A parser starts only after its resource job has been attached.
use std::io::{self, Read, Write};
use std::os::windows::{
    io::{AsRawHandle, FromRawHandle, OwnedHandle},
    process::CommandExt,
};
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{ERROR_BROKEN_PIPE, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::JobObjects::*;
use windows::Win32::System::Pipes::PeekNamedPipe;
use windows::Win32::System::Threading::WaitForSingleObject;

const MEMORY_BYTES: usize = 512 * 1024 * 1024;
// Codec allocations precede our 640px downsampling. The 512MiB still-image job
// stalled a real HEVC MOV, while 1GiB allowed successive frames to be decoded;
// a separate 1GiB video envelope restores decoding without removing the bound.
const VIDEO_MEMORY_BYTES: usize = 1024 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(15);
pub(super) fn initialize() -> io::Result<()> {
    use windows::Win32::UI::HiDpi::*;
    // The headless executable has not created any UI/DPI-dependent resource.
    // Match GPUI's native awareness instead of Windows virtualizing PDF sizes.
    match unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) } {
        Ok(()) => Ok(()),
        Err(_)
            if unsafe { GetAwarenessFromDpiAwarenessContext(GetThreadDpiAwarenessContext()) }
                == DPI_AWARENESS_PER_MONITOR_AWARE =>
        {
            Ok(())
        }
        Err(error) => Err(native(error)),
    }
}
// Covers all callers, including those outside the UI admission queue.
static SLOT: Mutex<()> = Mutex::new(());
static VIDEO_SLOT: Mutex<()> = Mutex::new(());

fn native(error: windows::core::Error) -> io::Error {
    io::Error::other(error.to_string())
}

struct Worker {
    child: Child,
    _job: OwnedHandle,
}
impl Worker {
    fn start(mut command: Command, memory_bytes: usize) -> io::Result<(Self, ChildStdout)> {
        let job = job(memory_bytes)?;
        let child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(0x08000000)
            .spawn()?;
        let mut worker = Self { child, _job: job };
        unsafe {
            AssignProcessToJobObject(
                HANDLE(worker._job.as_raw_handle()),
                HANDLE(worker.child.as_raw_handle()),
            )
        }
        .map_err(native)?;
        let mut start = worker
            .child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("Missing preview gate"))?;
        start.write_all(&[0xA7])?;
        drop(start);
        let output = worker
            .child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("Missing preview output"))?;
        Ok((worker, output))
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        // Reap on success, error, timeout and unwinding. The job also kills its
        // member if Oxyde terminates while a preview is still running.
        let _ = self.child.kill();
        // A faulty kernel driver can delay process termination. Never turn a
        // parser timeout into an unlimited wait in the explorer's worker thread.
        if unsafe { WaitForSingleObject(HANDLE(self.child.as_raw_handle()), 1000) } == WAIT_OBJECT_0
        {
            let _ = self.child.wait();
        } else {
            log::warn!("Preview worker termination has not completed; closing its resource job");
        }
    }
}

fn job(memory_bytes: usize) -> io::Result<OwnedHandle> {
    let raw = unsafe { CreateJobObjectW(None, None) }.map_err(native)?;
    let owned = unsafe { OwnedHandle::from_raw_handle(raw.0) };
    let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_PROCESS_MEMORY
        | JOB_OBJECT_LIMIT_JOB_MEMORY
        | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
        | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
    info.BasicLimitInformation.ActiveProcessLimit = 1;
    info.ProcessMemoryLimit = memory_bytes;
    info.JobMemoryLimit = memory_bytes;
    unsafe {
        SetInformationJobObject(
            raw,
            JobObjectExtendedLimitInformation,
            (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&info) as u32,
        )
    }
    .map_err(native)?;
    Ok(owned)
}

pub(super) fn run(mode: &str, path: &Path) -> io::Result<Vec<u8>> {
    let _slot = SLOT
        .lock()
        .map_err(|_| io::Error::other("Preview worker admission failed"))?;
    run_in(&std::env::current_exe()?, mode, path, DEADLINE)
}

pub(super) fn run_in(
    executable: &Path,
    mode: &str,
    path: &Path,
    deadline: Duration,
) -> io::Result<Vec<u8>> {
    let mut command = Command::new(executable);
    command.arg("--oxyde-preview-worker-v1").arg(mode).arg(path);
    supervise(command, deadline)
}

pub(super) fn supervise(command: Command, deadline: Duration) -> io::Result<Vec<u8>> {
    let started = Instant::now();
    let (mut worker, mut output) = Worker::start(command, MEMORY_BYTES)?;
    let mut bytes = Vec::new();
    let mut chunk = [0; 64 * 1024];
    let mut exited = None;
    loop {
        if started.elapsed() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "PreviewWorkerTimeout: Preview exceeded 15 seconds",
            ));
        }
        let mut available = 0;
        let peek = unsafe {
            PeekNamedPipe(
                HANDLE(output.as_raw_handle()),
                None,
                0,
                None,
                Some(&mut available),
                None,
            )
        };
        if let Err(error) = peek {
            if error.code() != windows::core::HRESULT::from_win32(ERROR_BROKEN_PIPE.0) {
                return Err(native(error));
            }
        }
        if available > 0 {
            let requested = (available as usize).min(chunk.len());
            let count = output.read(&mut chunk[..requested])?;
            if count == 0 {
                return Err(io::Error::other(
                    "Preview worker output closed unexpectedly",
                ));
            }
            if bytes.len() + count > crate::utils::preview_protocol::MAX_REPLY {
                return Err(io::Error::other("Preview worker output exceeds its limit"));
            }
            let needed = bytes.len() + count;
            if needed > bytes.capacity() {
                let capacity = needed
                    .max(bytes.capacity().saturating_mul(2))
                    .min(crate::utils::preview_protocol::MAX_REPLY);
                bytes
                    .try_reserve_exact(capacity - bytes.len())
                    .map_err(io::Error::other)?;
            }
            bytes.extend_from_slice(&chunk[..count]);
        } else if let Some(status) = exited {
            if !std::process::ExitStatus::success(&status) {
                return Err(io::Error::other(format!("Preview worker failed: {status}")));
            }
            return Ok(bytes);
        } else {
            std::thread::sleep(Duration::from_millis(5));
        }
        exited = worker.child.try_wait()?;
    }
}

pub(super) fn stream(
    path: &Path,
    on_frame: &mut dyn FnMut(image::RgbaImage, Duration) -> bool,
    cancelled: &dyn Fn() -> bool,
) -> io::Result<()> {
    if cancelled() {
        return Ok(());
    }
    let _slot = VIDEO_SLOT
        .lock()
        .map_err(|_| io::Error::other("Video worker admission failed"))?;
    if cancelled() {
        return Ok(());
    }
    stream_in(
        &std::env::current_exe()?,
        path,
        on_frame,
        cancelled,
        DEADLINE,
    )
}

pub(super) fn stream_in(
    executable: &Path,
    path: &Path,
    on_frame: &mut dyn FnMut(image::RgbaImage, Duration) -> bool,
    cancelled: &dyn Fn() -> bool,
    deadline: Duration,
) -> io::Result<()> {
    let mut command = Command::new(executable);
    command
        .arg("--oxyde-preview-worker-v1")
        .arg("video")
        .arg(path);
    supervise_stream(command, on_frame, cancelled, deadline)
}

pub(super) fn supervise_stream(
    command: Command,
    on_frame: &mut dyn FnMut(image::RgbaImage, Duration) -> bool,
    cancelled: &dyn Fn() -> bool,
    deadline: Duration,
) -> io::Result<()> {
    use crate::utils::preview_protocol::{self, Reply};
    const MAX_PACKET: usize = 2 * 1024 * 1024;
    if cancelled() {
        return Ok(());
    }
    let mut progress = Instant::now();
    let (mut worker, mut output) = Worker::start(command, VIDEO_MEMORY_BYTES)?;
    let mut pending = Vec::new();
    let mut chunk = [0; 64 * 1024];
    let mut exited = None;
    let mut received = false;
    loop {
        if cancelled() {
            return Ok(());
        }
        if progress.elapsed() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "PreviewWorkerTimeout: Video produced no frame for 15 seconds",
            ));
        }
        let mut available = 0;
        if let Err(error) = unsafe {
            PeekNamedPipe(
                HANDLE(output.as_raw_handle()),
                None,
                0,
                None,
                Some(&mut available),
                None,
            )
        } {
            if error.code() != windows::core::HRESULT::from_win32(ERROR_BROKEN_PIPE.0) {
                return Err(native(error));
            }
        }
        if available > 0 {
            let requested = (available as usize).min(chunk.len());
            let count = output.read(&mut chunk[..requested])?;
            if count == 0 {
                return Err(io::Error::other("Video worker output closed unexpectedly"));
            }
            if pending.len() + count > MAX_PACKET + 4 + chunk.len() {
                return Err(io::Error::other("Video worker buffer exceeds its limit"));
            }
            let needed = pending.len() + count;
            if needed > pending.capacity() {
                let capacity = needed
                    .max(pending.capacity().saturating_mul(2))
                    .min(MAX_PACKET + 4 + chunk.len());
                pending
                    .try_reserve_exact(capacity - pending.len())
                    .map_err(io::Error::other)?;
            }
            pending.extend_from_slice(&chunk[..count]);
            while pending.len() >= 4 {
                let length = u32::from_le_bytes(pending[..4].try_into().unwrap()) as usize;
                if !(12..=MAX_PACKET).contains(&length) {
                    return Err(io::Error::other("Invalid video worker packet size"));
                }
                if pending.len() < 4 + length {
                    break;
                }
                let packet = &pending[4..4 + length];
                let seconds = u64::from_le_bytes(packet[..8].try_into().unwrap());
                let nanos = u32::from_le_bytes(packet[8..12].try_into().unwrap());
                if nanos >= 1_000_000_000 || seconds > i64::MAX as u64 / 10_000_000 {
                    return Err(io::Error::other("Invalid video worker timestamp"));
                }
                let reply = preview_protocol::decode(&packet[12..])?;
                pending.drain(..4 + length);
                match reply {
                    Reply::Images {
                        mut frames,
                        limited: false,
                    } if frames.len() == 1 => {
                        let image = frames.remove(0).into_buffer();
                        if image.width() > 640 || image.height() > 640 {
                            return Err(io::Error::other(
                                "Video worker dimensions exceed their limit",
                            ));
                        }
                        received = true;
                        if !on_frame(image, Duration::new(seconds, nanos)) {
                            return Ok(());
                        }
                        progress = Instant::now();
                    }
                    Reply::Error { detail, .. } => return Err(io::Error::other(detail)),
                    _ => return Err(io::Error::other("Unexpected video worker reply")),
                }
                if cancelled() {
                    return Ok(());
                }
            }
        } else if let Some(status) = exited {
            if !std::process::ExitStatus::success(&status) || !pending.is_empty() || !received {
                return Err(io::Error::other(
                    "Video worker ended without a complete stream",
                ));
            }
            return Ok(());
        } else {
            std::thread::sleep(Duration::from_millis(5));
        }
        exited = worker.child.try_wait()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resource_job_has_real_kernel_limits_before_admission() {
        for memory_bytes in [MEMORY_BYTES, VIDEO_MEMORY_BYTES] {
            let job = job(memory_bytes).unwrap();
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            unsafe {
                QueryInformationJobObject(
                    Some(HANDLE(job.as_raw_handle())),
                    JobObjectExtendedLimitInformation,
                    (&mut info as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    std::mem::size_of_val(&info) as u32,
                    None,
                )
            }
            .unwrap();
            assert_eq!(info.ProcessMemoryLimit, memory_bytes);
            assert_eq!(info.JobMemoryLimit, memory_bytes);
            assert_eq!(info.BasicLimitInformation.ActiveProcessLimit, 1);
            assert_eq!(
                info.BasicLimitInformation.LimitFlags & JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            );
        }
    }
}
