//! Helper tools run under a total deadline. A tool that stops answering (a frozen
//! clipboard owner, a stalled server, an unreachable directory service) is killed with
//! its process group instead of holding the thread that waits for it.
use rustix::process::{kill_process_group, Pid, Signal};
use std::io::{self, Read};
use std::os::unix::process::CommandExt;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Standard error is only read for a message.
const STDERR_LIMIT: u64 = 64 * 1024;

pub(super) struct Output {
    pub(super) status: ExitStatus,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
}

/// Runs `command` without input. Past `timeout`, its process group is killed and the
/// result is `TimedOut`; past `limit` bytes of output, `FileTooLarge`.
pub(super) fn output_within(mut command: Command, timeout: Duration, limit: usize) -> io::Result<Output> {
    let deadline = Instant::now() + timeout;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;
    let group = Pid::from_child(&child);
    let kill = || {
        let _ = kill_process_group(group, Signal::KILL);
    };

    let stderr = child.stderr.take().expect("piped stderr");
    let (errors_sent, errors) = mpsc::channel();
    std::thread::spawn(move || {
        let mut text = Vec::new();
        let _ = stderr.take(STDERR_LIMIT).read_to_end(&mut text);
        let _ = errors_sent.send(text);
    });

    // The child is reaped only after the watchdog stops, so the group it kills is still
    // this child's and not a reused id.
    let (finished, done) = mpsc::channel::<()>();
    let watchdog = std::thread::spawn(move || {
        let expired = matches!(
            done.recv_timeout(deadline.saturating_duration_since(Instant::now())),
            Err(mpsc::RecvTimeoutError::Timeout)
        );
        if expired {
            let _ = kill_process_group(group, Signal::KILL);
        }
        expired
    });
    let mut stdout = Vec::new();
    let read = child.stdout.take().expect("piped stdout").take(limit as u64 + 1).read_to_end(&mut stdout);
    let too_large = stdout.len() > limit;
    if too_large {
        kill();
    }
    let _ = finished.send(());
    let mut expired = watchdog.join().unwrap_or(true);

    // A tool can close its output and keep running.
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            kill();
            expired = true;
            break child.wait()?;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let wait = deadline.saturating_duration_since(Instant::now()).max(Duration::from_millis(100));
    let stderr = errors.recv_timeout(wait).unwrap_or_default();

    let tool = command.get_program().to_string_lossy().into_owned();
    if expired {
        return Err(io::Error::new(io::ErrorKind::TimedOut, format!("{tool} did not finish in time")));
    }
    if too_large {
        return Err(io::Error::new(io::ErrorKind::FileTooLarge, format!("{tool} output exceeds its size limit")));
    }
    read?;
    Ok(Output { status, stdout, stderr })
}

#[cfg(test)]
#[path = "linux_process/tests/deadline.rs"]
mod tests;
