//! Self-workers for native decoders, as Nautilus runs its thumbnailers apart: a
//! hostile file only reaches a confined process, bounded in private memory and in time.
use super::linux_sandbox::{self, Profile};
use std::io::{self, Read, Write};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Stdio};
use std::time::{Duration, Instant};

/// The Windows parent sends it once the job is attached; the worker waits for it.
const START_GATE: u8 = 0xA7;

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
        let source = path.map(|path| std::fs::File::open(path).map(|file| (path, file))).transpose()?;
        // glibc reserves 64 MiB per thread arena, which decoders with many threads would
        // spend on reservations rather than pixels: the launch limits arenas to two.
        let mut launch = linux_sandbox::command(executable, profile, source)?;
        let command = &mut launch.command;
        command.arg("--oxyde-preview-worker-v1").arg(mode);
        if let Some(source) = &launch.source {
            command.arg(source);
        }
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
        let mut bytes = Vec::new();
        let mut chunk = [0; 64 * 1024];
        loop {
            self.wait_readable(deadline)?;
            match self.output.read(&mut chunk)? {
                0 => return Ok(bytes),
                read if bytes.len() + read > limit => return Err(io::Error::other("Preview worker reply exceeds its limit")),
                read => bytes.extend_from_slice(&chunk[..read]),
            }
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
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "Preview worker exceeded its deadline"));
            }
            if self.readable(remaining)? {
                return Ok(());
            }
        }
    }

    /// Output to read, or closed (the read tells which), within `timeout`.
    pub(super) fn readable(&self, timeout: Duration) -> io::Result<bool> {
        use rustix::event::{poll, PollFd, PollFlags, Timespec};
        let timeout = Timespec { tv_sec: timeout.as_secs() as i64, tv_nsec: timeout.subsec_nanos().into() };
        let mut descriptors = [PollFd::new(&self.output, PollFlags::IN)];
        match poll(&mut descriptors, Some(&timeout)) {
            Ok(ready) => Ok(ready > 0),
            Err(rustix::io::Errno::INTR) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
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
