//! Self-workers for native decoders, as Nautilus runs its thumbnailers apart: a
//! hostile file only reaches a confined process, bounded in private memory and in time.
use super::linux_sandbox::{self, Profile};
use std::ffi::OsStr;
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

/// The Windows parent sends it once the job is attached; the worker waits for it.
const START_GATE: u8 = 0xA7;
const CHECKPOINT_INTERVAL: Duration = Duration::from_millis(50);

pub(super) struct Worker {
    child: Child,
    input: ChildStdin,
    output: ChildStdout,
}

impl Worker {
    /// `memory_bytes` bounds private writable memory; mapped code does not count. The
    /// worker receives `path` already open, so it needs no access to the user's files.
    pub(super) fn start(executable: &Path, mode: &str, path: Option<&Path>, memory_bytes: u64) -> io::Result<Self> {
        let profile = Profile::of(mode);
        if profile.reads_media() {
            refresh_media_registry(executable);
        }
        Self::launch(executable, profile, path, memory_bytes, |command, source| {
            command.arg("--oxyde-preview-worker-v1").arg(mode);
            if let Some(source) = source {
                command.arg(source);
            }
        })
    }

    /// Any confined worker: `arguments` receive the document's path as it sees it.
    pub(super) fn launch(executable: &Path, profile: Profile, path: Option<&Path>, memory_bytes: u64,
        arguments: impl FnOnce(&mut Command, Option<&OsStr>)) -> io::Result<Self> {
        let source = path.map(|path| open_document(path).map(|file| (path, file))).transpose()?;
        // glibc reserves 64 MiB per thread arena, which decoders with many threads would
        // spend on reservations rather than pixels: the launch limits arenas to two.
        let mut launch = linux_sandbox::command(executable, profile, source)?;
        let command = &mut launch.command;
        arguments(command, launch.source.as_deref());
        command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        // SAFETY: a single raw system call between fork and exec.
        unsafe {
            command.pre_exec(move || {
                use rustix::process::{setrlimit, Resource, Rlimit};
                let limit = Some(memory_bytes);
                Ok(setrlimit(Resource::Data, Rlimit { current: limit, maximum: limit })?)
            });
        }
        let mut child = command.spawn()?;
        let (Some(input), Some(output)) = (child.stdin.take(), child.stdout.take()) else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other("Preview worker pipes are missing"));
        };
        let mut worker = Self { child, input, output };
        worker.send(&[START_GATE])?;
        Ok(worker)
    }

    pub(super) fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.input.write_all(bytes)?;
        self.input.flush()
    }

    pub(super) fn read_exact(&mut self, mut buffer: &mut [u8], deadline: Instant) -> io::Result<()> {
        while !buffer.is_empty() {
            self.wait_readable(deadline)?;
            match self.output.read(buffer)? {
                0 => return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Preview worker exited")),
                read => buffer = &mut buffer[read..],
            }
        }
        Ok(())
    }

    /// The whole output of a one-request worker, refused beyond `limit` bytes.
    pub(super) fn read_to_end(&mut self, limit: usize, deadline: Instant) -> io::Result<Vec<u8>> {
        self.read_to_end_checked(limit, deadline, &|| Ok(()))
    }

    /// As `read_to_end`, and `checkpoint` can stop the wait for a cancelled request.
    pub(super) fn read_to_end_checked(&mut self, limit: usize, deadline: Instant, checkpoint: &dyn Fn() -> io::Result<()>) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        let mut chunk = [0; 64 * 1024];
        loop {
            while !self.readable(CHECKPOINT_INTERVAL.min(remaining(deadline)?))? {
                checkpoint()?;
            }
            match self.output.read(&mut chunk)? {
                0 => return Ok(bytes),
                read if bytes.len() + read > limit => return Err(io::Error::new(io::ErrorKind::FileTooLarge, "Preview worker reply exceeds its limit")),
                read => bytes.extend_from_slice(&chunk[..read]),
            }
        }
    }

    /// For a conversation in both directions, where writes must not block the parent.
    pub(super) fn prepare_duplex(&mut self, pipe_bytes: usize) -> io::Result<()> {
        use rustix::pipe::fcntl_setpipe_size;
        // Refused beyond the system's limit: only slower, then.
        let _ = fcntl_setpipe_size(&self.input, pipe_bytes);
        let _ = fcntl_setpipe_size(&self.output, pipe_bytes);
        rustix::fs::fcntl_setfl(&self.input, rustix::fs::OFlags::NONBLOCK)?;
        Ok(())
    }

    pub(super) fn read_available(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.output.read(buffer)
    }

    pub(super) fn write_available(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.input.write(bytes)
    }

    pub(super) fn writable(&self, timeout: Duration) -> io::Result<bool> {
        poll_one(&self.input, rustix::event::PollFlags::OUT, timeout)
    }

    /// The worker's exit status, once it has ended by itself before `limit`.
    pub(super) fn wait_exit(&mut self, limit: Instant) -> io::Result<std::process::ExitStatus> {
        loop {
            if let Some(status) = self.child.try_wait()? {
                return Ok(status);
            }
            if Instant::now() >= limit {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "Worker did not exit"));
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Lets the worker end on its own after a close request, then reaps it.
    pub(super) fn finish(mut self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if !matches!(self.child.try_wait(), Ok(None)) {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn wait_readable(&self, deadline: Instant) -> io::Result<()> {
        while !self.readable(remaining(deadline)?)? {}
        Ok(())
    }

    /// Output to read, or closed (the read tells which), within `timeout`.
    pub(super) fn readable(&self, timeout: Duration) -> io::Result<bool> {
        poll_one(&self.output, rustix::event::PollFlags::IN, timeout)
    }
}

fn poll_one(descriptor: &impl std::os::fd::AsFd, flags: rustix::event::PollFlags, timeout: Duration) -> io::Result<bool> {
    use rustix::event::{poll, PollFd, Timespec};
    let timeout = Timespec { tv_sec: timeout.as_secs() as i64, tv_nsec: timeout.subsec_nanos().into() };
    let mut descriptors = [PollFd::new(descriptor, flags)];
    match poll(&mut descriptors, Some(&timeout)) {
        Ok(ready) => Ok(ready > 0),
        Err(rustix::io::Errno::INTR) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(io::Error::new(io::ErrorKind::TimedOut, "Preview worker exceeded its deadline"));
    }
    Ok(remaining)
}

/// The document as one descriptor. A named pipe would block its opening, and only
/// regular files are documents.
fn open_document(path: &Path) -> io::Result<std::fs::File> {
    let file = std::fs::OpenOptions::new().read(true)
        .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32).open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Preview source is not a regular file"));
    }
    Ok(file)
}

/// GStreamer scans every plugin when its registry is missing or outdated, which costs
/// seconds: a worker that opens no document brings Oxyde's registry up to date once per
/// session, before the first media worker. Other workers only read it.
fn refresh_media_registry(executable: &Path) {
    static REFRESHED: std::sync::Mutex<bool> = std::sync::Mutex::new(false);
    let mut refreshed = REFRESHED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if std::mem::replace(&mut *refreshed, true) {
        return;
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    if let Err(error) = Worker::start(executable, "media-registry", None, 1024 * 1024 * 1024)
        .and_then(|mut worker| worker.read_to_end(0, deadline)) {
        log::warn!("GStreamer registry not refreshed: {error}");
    }
}

/// Worker side: end with Oxyde even while a decoder holds the main thread. Oxyde holds
/// the other end of standard input until it lets the worker go; neither the parent's
/// death signal, tied to its spawning thread, nor the parent process, hidden by the
/// sandbox's own, can tell.
pub(super) fn exit_with_parent() {
    use rustix::event::{poll, PollFd, PollFlags};
    use std::os::fd::AsFd;
    std::thread::spawn(|| {
        let input = io::stdin();
        let input = input.as_fd();
        // Only the end of the pipe wakes it: the data is left to the worker.
        let mut descriptors = [PollFd::new(&input, PollFlags::empty())];
        while matches!(poll(&mut descriptors, None), Ok(0) | Err(rustix::io::Errno::INTR)) {}
        let _ = rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL);
    });
}
