//! Self-workers for native decoders, as Nautilus runs its thumbnailers apart: a
//! hostile file can only end that process, bounded in private memory and in time.
use std::io::{self, Read, Write};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

/// The Windows parent sends it once the job is attached; the worker waits for it.
const START_GATE: u8 = 0xA7;

pub(super) struct Worker {
    child: Child,
    input: ChildStdin,
    output: ChildStdout,
}

impl Worker {
    /// `memory_bytes` bounds private writable memory; mapped code does not count.
    pub(super) fn start(executable: &Path, mode: &str, path: Option<&Path>, memory_bytes: u64) -> io::Result<Self> {
        let mut command = Command::new(executable);
        command.arg("--oxyde-preview-worker-v1").arg(mode);
        if let Some(path) = path {
            command.arg(path);
        }
        // glibc reserves 64 MiB per thread arena, which decoders with many threads would
        // spend on reservations rather than pixels.
        command.env("MALLOC_ARENA_MAX", "2").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
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
        use rustix::event::{poll, PollFd, PollFlags, Timespec};
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "Preview worker exceeded its deadline"));
            }
            let timeout = Timespec { tv_sec: remaining.as_secs() as i64, tv_nsec: remaining.subsec_nanos().into() };
            let mut descriptors = [PollFd::new(&self.output, PollFlags::IN)];
            match poll(&mut descriptors, Some(&timeout)) {
                // Readable, or closed: the read tells which.
                Ok(ready) if ready > 0 => return Ok(()),
                Ok(_) | Err(rustix::io::Errno::INTR) => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Worker side: end with Oxyde even while a decoder holds the main thread. The
/// parent's death signal would follow its spawning thread, which a pool may retire.
pub(super) fn exit_with_parent() {
    let parent = rustix::process::getppid();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(500));
        if rustix::process::getppid() != parent {
            let _ = rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL);
        }
    });
}
