//! A parser starts only after its resource job has been attached.
use std::io::{self, Read, Write};
use std::os::windows::{
    io::{AsRawHandle, FromRawHandle, OwnedHandle},
    process::CommandExt,
};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{ERROR_BROKEN_PIPE, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::JobObjects::*;
use windows::Win32::System::Pipes::{CreatePipe, PeekNamedPipe};
use windows::Win32::System::Threading::WaitForSingleObject;

const MEMORY_BYTES: usize = 512 * 1024 * 1024;
// Codec allocations precede our 640px downsampling. The 512MiB still-image job
// stalled a real HEVC MOV, while 1GiB allowed successive frames to be decoded;
// a separate 1GiB video envelope restores decoding without removing the bound.
const VIDEO_MEMORY_BYTES: usize = 1024 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(15);
mod image_session;
mod icon_session;
pub(super) fn shutdown_image_session() { image_session::shutdown(); }
pub(super) fn warm_viewer_images() { image_session::warm_viewer(); }
pub(super) fn release_viewer_images() { image_session::release_viewer(); }
pub(super) fn shutdown_icon_session() { icon_session::shutdown(); }
pub(super) fn reset_icon_session() { icon_session::reset(); }
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

pub(super) struct Worker {
    pub(super) child: Child,
    pub(super) _job: OwnedHandle,
    budget: Option<super::windows_worker_budget::Permit>,
}
impl Worker {
    fn start(
        command: Command,
        memory_bytes: usize,
        checkpoint: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<(Self, std::fs::File)> {
        let (worker, output, input) =
            Self::start_duplex_checked(command, memory_bytes, checkpoint)?;
        drop(input);
        Ok((worker, output))
    }
    pub(super) fn start_duplex_checked(
        mut command: Command,
        memory_bytes: usize,
        checkpoint: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<(Self, std::fs::File, std::process::ChildStdin)> {
        let budget = super::windows_worker_budget::acquire(memory_bytes, checkpoint)?;
        let job = job(memory_bytes)?;
        // Suggest enough capacity for one bounded video packet to reduce small
        // transfers through the polled pipe. Windows may choose another size.
        let mut read = HANDLE::default();
        let mut write = HANDLE::default();
        unsafe { CreatePipe(&mut read, &mut write, None, 2 * 1024 * 1024) }.map_err(native)?;
        let output = std::fs::File::from(unsafe { OwnedHandle::from_raw_handle(read.0) });
        let input = std::fs::File::from(unsafe { OwnedHandle::from_raw_handle(write.0) });
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::from(input))
            .stderr(Stdio::null())
            .creation_flags(0x08000000);
        let child = super::windows_process_launch::spawn(command)?;
        let mut worker = Self {
            child,
            _job: job,
            budget: Some(budget),
        };
        super::windows_worker_budget::attach(HANDLE(worker.child.as_raw_handle()))?;
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
        Ok((worker, output, start))
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
            // Do not admit replacement work while an unconfirmed process still
            // owns its envelope. The native aggregate job remains a second bound.
            if let Some(budget) = self.budget.take() {
                super::windows_worker_budget::retain_until_exit(
                    HANDLE(self.child.as_raw_handle()),
                    budget,
                    || {},
                );
            }
        }
    }
}

pub(super) fn job(memory_bytes: usize) -> io::Result<OwnedHandle> {
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
    if mode == "audio-cover" || mode == "audio-cover-thumbnail" {
        return image_session::run(path, if mode == "audio-cover-thumbnail" {
            image_session::AUDIO_THUMBNAIL_KIND
        } else { image_session::AUDIO_KIND });
    }
    if mode == "audio-metadata" {
        return run_audio_metadata_in(&std::env::current_exe()?.with_file_name("oxyde-image-worker.exe"), path);
    }
    if (mode == "icon-small" || mode == "icon-large") && path.to_str().is_some_and(|path| path.len() + 5 <= crate::commands::icon_session_worker::MAX_INPUT) {
        return icon_session::run(path, mode == "icon-large");
    }
    if mode == "image-viewer" {
        return image_session::run(path, image_session::VIEWER_KIND);
    }
    let _slot = SLOT
        .lock()
        .map_err(|_| io::Error::other("Preview worker admission failed"))?;
    if mode == "image" {
        image_session::run(path, 0)
    } else if mode == "image-thumbnail" {
        image_session::run(path, 1)
    } else if mode == "office-text" || mode == "office-document" {
        run_office_text_in(
            &std::env::current_exe()?.with_file_name("oxyde-image-worker.exe"),
            path,
            DEADLINE,
            mode == "office-document",
        )
    } else {
        run_in(&std::env::current_exe()?, mode, path, DEADLINE)
    }
}

fn run_audio_metadata_in(executable: &Path, path: &Path) -> io::Result<Vec<u8>> {
    let source = super::platform().retain_archive_source(path)?;
    let mut command = Command::new(executable);
    command.arg(crate::commands::audio_handle_worker::FLAG);
    supervise_source(command, &source.file, DEADLINE, MEMORY_BYTES,
        crate::utils::audio_metadata::MAX_REPLY, &|| Ok(()))
}

#[cfg(test)]
#[path = "windows_preview_worker/tests/audio_metadata.rs"]
mod audio_metadata_tests;

pub(super) fn run_image_mode_in(executable: &Path, path: &Path, deadline: Duration, thumbnail: bool) -> io::Result<Vec<u8>> {
    let source = crate::platform::platform().retain_archive_source(path)?;
    if source.file.metadata()?.len()
        > crate::utils::image_preview_decode::POLICY.encoded_bytes as u64
    {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "Image source exceeds its limit",
        ));
    }
    let mut command = Command::new(executable);
    command.arg(if thumbnail { crate::commands::image_handle_worker::THUMBNAIL_FLAG } else { crate::commands::image_handle_worker::FLAG });
    supervise_source(
        command,
        &source.file,
        deadline,
        MEMORY_BYTES,
        crate::utils::preview_protocol::MAX_REPLY,
        &|| Ok(()),
    )
}

#[cfg(test)]
#[path = "windows_preview_worker/tests/thumbnail.rs"]
mod thumbnail_tests;
pub(super) fn run_office_text_in(
    executable: &Path,
    path: &Path,
    deadline: Duration,
    document: bool,
) -> io::Result<Vec<u8>> {
    let source = crate::platform::platform().retain_archive_source(path)?;
    let mut command = Command::new(executable);
    let extension = path.extension().and_then(|value| value.to_str()).unwrap_or("");
    // The hint selects the complete Viewer document; it grants no extra authority.
    command
        .arg("--oxyde-office-text-handle-worker-v1")
        .arg(if document { format!("document:{extension}") } else { extension.to_owned() });
    supervise_source(
        command,
        &source.file,
        deadline,
        MEMORY_BYTES,
        crate::utils::preview_protocol::MAX_REPLY,
        &|| Ok(()),
    )
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
    supervise_bounded(
        command,
        deadline,
        MEMORY_BYTES,
        crate::utils::preview_protocol::MAX_REPLY,
        "PreviewWorkerTimeout: Preview exceeded 15 seconds",
    )
}

/// Shared native resource supervision; admission and response validation belong
/// to each caller. A parser is never run locally on transport failure.
pub(super) fn supervise_bounded(
    command: Command,
    deadline: Duration,
    memory_bytes: usize,
    max_reply: usize,
    timeout_detail: &str,
) -> io::Result<Vec<u8>> {
    supervise_checked(
        command,
        deadline,
        memory_bytes,
        max_reply,
        timeout_detail,
        &|| Ok(()),
    )
}

pub(super) fn supervise_checked(
    command: Command,
    deadline: Duration,
    memory_bytes: usize,
    max_reply: usize,
    timeout_detail: &str,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> io::Result<Vec<u8>> {
    checkpoint()?;
    let started = Instant::now();
    let checked = || {
        checkpoint()?;
        if started.elapsed() >= deadline {
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                timeout_detail.to_owned(),
            ))
        } else {
            Ok(())
        }
    };
    let (mut worker, output) = Worker::start(command, memory_bytes, &checked)?;
    collect_reply(
        output,
        started,
        deadline,
        max_reply,
        timeout_detail,
        checkpoint,
        &mut || {
            super::windows_resource_pressure::check_worker(HANDLE(worker.child.as_raw_handle()))?;
            worker.child.try_wait()
        },
    )
}

pub(super) fn supervise_source(
    command: Command,
    source: &std::fs::File,
    deadline: Duration,
    memory_bytes: usize,
    max_reply: usize,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> io::Result<Vec<u8>> {
    let started = Instant::now();
    let timeout_detail = if command
        .get_args()
        .next()
        .is_some_and(|flag| flag == "--oxyde-archive-handle-worker-v1")
    {
        "Archive metadata worker timed out"
    } else {
        "PreviewWorkerTimeout: Preview exceeded its deadline"
    };
    let checked = || {
        checkpoint()?;
        if started.elapsed() >= deadline {
            Err(io::Error::new(io::ErrorKind::TimedOut, timeout_detail))
        } else {
            Ok(())
        }
    };
    let (mut worker, output, input) = super::windows_worker_sandbox::Worker::start_duplex_source(
        command,
        memory_bytes,
        &checked,
        source,
    )?;
    drop(input);
    collect_reply(
        output,
        started,
        deadline,
        max_reply,
        timeout_detail,
        checkpoint,
        &mut || worker.child.try_wait(),
    )
}

pub(super) fn collect_reply(
    mut output: std::fs::File,
    started: Instant,
    deadline: Duration,
    max_reply: usize,
    timeout_detail: &str,
    checkpoint: &dyn Fn() -> io::Result<()>,
    wait: &mut dyn FnMut() -> io::Result<Option<std::process::ExitStatus>>,
) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut chunk = [0; 64 * 1024];
    let mut exited = None;
    loop {
        checkpoint()?;
        super::windows_resource_pressure::check()?;
        if started.elapsed() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                timeout_detail.to_owned(),
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
            if bytes.len() + count > max_reply {
                return Err(io::Error::new(
                    io::ErrorKind::FileTooLarge,
                    "Worker output exceeds its limit",
                ));
            }
            let needed = bytes.len() + count;
            if needed > bytes.capacity() {
                let capacity = needed
                    .max(bytes.capacity().saturating_mul(2))
                    .min(max_reply);
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
        exited = wait()?;
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
    let started = Worker::start(command, VIDEO_MEMORY_BYTES, &|| {
        if cancelled() {
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Operation cancelled",
            ))
        } else {
            Ok(())
        }
    });
    let (mut worker, mut output) = match started {
        Err(error) if error.kind() == io::ErrorKind::Interrupted && cancelled() => return Ok(()),
        result => result?,
    };
    let mut pending = Vec::new();
    let mut chunk = [0; 64 * 1024];
    let mut exited = None;
    let mut received = false;
    loop {
        if cancelled() {
            return Ok(());
        }
        super::windows_resource_pressure::check()?;
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
                        ..
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
        super::windows_resource_pressure::check_worker(HANDLE(worker.child.as_raw_handle()))?;
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
