//! Archive codecs in a confined worker that sees no file at all: it asks the parent
//! for each block to read and hands it each block to write, in turn. The parent keeps
//! sources, destinations and every check. A failure is never retried locally.
use super::linux_sandbox::Profile;
use super::linux_worker::Worker;
use crate::models::error::{ArchiveLimitFailure, ArchiveLimitKind, ArchiveSourceConflict};
use crate::utils::archive_io_failure::{cancelled, checked_failure};
use crate::utils::archive_io_protocol::{invalid, Control, Format, Frame, Message, Operation, Wire};
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::{Mutex, MutexGuard, TryLockError};
use std::time::{Duration, Instant};

const FLAG: &str = crate::commands::archive_io_worker::FLAG;
const MEMORY_BYTES: u64 = 1024 * 1024 * 1024;
/// LZMA2 dictionaries and pending solid blocks share this envelope.
const SEVEN_ENCODE_MEMORY_BYTES: u64 = 2 * MEMORY_BYTES;
/// A frame fits whole, so that block transfers do not cost a context switch per 64 KiB.
const PIPE_BYTES: usize = crate::utils::archive_io_protocol::MAX_FRAME;
const POLL_INTERVAL: Duration = Duration::from_millis(20);
const EXIT_GRACE: Duration = Duration::from_secs(1);
static TARGETED: Mutex<()> = Mutex::new(());
static BULK: Mutex<()> = Mutex::new(());

fn worker_memory(operation: &Operation) -> u64 {
    if matches!(operation, Operation::Encode { format: Format::SevenZip, .. }) { SEVEN_ENCODE_MEMORY_BYTES } else { MEMORY_BYTES }
}

fn check(controls: &dyn Fn() -> io::Result<Control>) -> io::Result<Control> {
    let control = controls()?;
    if control.cancelled { Err(cancelled()) } else { Ok(control) }
}

fn admission(operation: &Operation, controls: &dyn Fn() -> io::Result<Control>) -> io::Result<MutexGuard<'static, ()>> {
    let slot = if matches!(operation, Operation::Decode { selected: Some(_), .. }) { &TARGETED } else { &BULK };
    loop {
        if !check(controls)?.paused {
            match slot.try_lock() {
                Ok(guard) => return Ok(guard),
                Err(TryLockError::Poisoned(_)) => return Err(invalid()),
                Err(TryLockError::WouldBlock) => {}
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub(super) fn run(operation: Operation, controls: &dyn Fn() -> io::Result<Control>,
    handler: &mut dyn FnMut(Frame) -> io::Result<Frame>) -> io::Result<Duration> {
    let _slot = admission(&operation, controls)?;
    let deadline = Duration::from_secs(match &operation {
        Operation::Decode { limits, .. } | Operation::Encode { limits, .. } => limits.seconds,
    });
    let mut parent_failed = false;
    run_in(&crate::utils::archive_io_protocol::worker_executable()?, operation, controls, &mut |frame| {
        let result = handler(frame);
        parent_failed |= result.is_err();
        result
    }, deadline).map_err(|error| {
        if (parent_failed && error.to_string() != invalid().to_string())
            || error.kind() == io::ErrorKind::Interrupted
            || ArchiveLimitFailure::from_message(&error.to_string()).is_some()
            || ArchiveSourceConflict::from_message(&error.to_string()) {
            return error;
        }
        log::warn!("Archive worker failure: {error:?}");
        io::Error::other(crate::models::error::ArchiveProcessingFailure)
    })
}

/// The pipes as blocking streams that stay under control: cancellation and the deadline
/// are checked while waiting, whatever the worker does.
struct Conversation<'a> {
    worker: Worker,
    controls: &'a dyn Fn() -> io::Result<Control>,
    started: Instant,
    paused: Duration,
    deadline: Duration,
    cancelled: bool,
}

impl Conversation<'_> {
    /// For the streams: `read_exact` and `write_all` would retry an `Interrupted`
    /// forever, so cancellation leaves them as another error, restored by `run_in`.
    fn stream_live(&mut self) -> io::Result<()> {
        match self.live() {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                self.cancelled = true;
                Err(io::Error::other(error))
            }
            result => result.map(|_| ()),
        }
    }

    fn active(&self) -> Duration {
        self.started.elapsed().saturating_sub(self.paused)
    }

    fn live(&self) -> io::Result<Control> {
        let control = check(self.controls)?;
        if self.active() >= self.deadline {
            return Err(io::Error::new(io::ErrorKind::TimedOut, ArchiveLimitFailure {
                kind: ArchiveLimitKind::Seconds,
                limit: self.deadline.as_secs(),
            }));
        }
        Ok(control)
    }
}

impl Read for Conversation<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        loop {
            self.stream_live()?;
            if self.worker.readable(POLL_INTERVAL)? {
                return self.worker.read_available(buffer);
            }
        }
    }
}

impl Write for Conversation<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        loop {
            self.stream_live()?;
            if self.worker.writable(POLL_INTERVAL)? {
                match self.worker.write_available(bytes) {
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                    written => return written,
                }
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn run_in(executable: &Path, operation: Operation, controls: &dyn Fn() -> io::Result<Control>,
    handler: &mut dyn FnMut(Frame) -> io::Result<Frame>, deadline: Duration) -> io::Result<Duration> {
    check(controls)?;
    let session = uuid::Uuid::new_v4();
    let memory = worker_memory(&operation);
    let mut worker = Worker::launch(executable, Profile::Codec, None, memory, |command, _| {
        command.arg(FLAG).arg(session.to_string());
    })?;
    worker.prepare_duplex(PIPE_BYTES)?;
    let mut conversation = Conversation { worker, controls, started: Instant::now(), paused: Duration::ZERO, deadline, cancelled: false };
    let result = converse(&mut conversation, *session.as_bytes(), &operation, handler);
    if conversation.cancelled {
        return Err(cancelled());
    }
    result
}

fn converse(conversation: &mut Conversation<'_>, session: [u8; 16], operation: &Operation,
    handler: &mut dyn FnMut(Frame) -> io::Result<Frame>) -> io::Result<Duration> {
    let mut wire = Wire::new(session);
    wire.write(&mut *conversation, &Frame::new(Message::Start { operation: operation.clone() }))?;
    loop {
        let frame = wire.read(&mut *conversation)?;
        if matches!(frame.message, Message::Failure { .. }) {
            return Err(checked_failure(&frame, operation));
        }
        if frame.message == Message::Paused {
            return Err(invalid());
        }
        let control = conversation.live()?;
        let turbo = if control.paused { pause(conversation, &mut wire, control.turbo)? } else { control.turbo };
        let finished = frame.message == Message::Finished;
        match frame.message {
            // The pause already answered a checkpoint.
            Message::Checkpoint if control.paused => {}
            Message::Checkpoint => wire.write(&mut *conversation, &Frame::new(Message::Ack { pause: false, turbo }))?,
            _ => {
                let reply = handler(frame)?;
                wire.write(&mut *conversation, &reply)?;
            }
        }
        if finished {
            return conclude(conversation);
        }
    }
}

/// Asks the worker to stop before its next effect. Only a confirmed pause suspends the
/// deadline: a worker that never confirms it still runs out of time.
fn pause(conversation: &mut Conversation<'_>, wire: &mut Wire, turbo: bool) -> io::Result<bool> {
    let requested = Instant::now();
    wire.write(&mut *conversation, &Frame::new(Message::Ack { pause: true, turbo }))?;
    if wire.read(&mut *conversation)?.message != Message::Paused {
        return Err(invalid());
    }
    let confirmed = Instant::now();
    let suspended = requested.elapsed() <= Duration::from_secs(1);
    let turbo = loop {
        let control = check(conversation.controls)?;
        if !control.paused {
            break control.turbo;
        }
        if !suspended {
            conversation.live()?;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    if suspended {
        conversation.paused += confirmed.elapsed();
    }
    wire.write(&mut *conversation, &Frame::new(Message::Resume { turbo }))?;
    Ok(turbo)
}

/// After its result, the worker owes only its clean exit.
fn conclude(conversation: &mut Conversation<'_>) -> io::Result<Duration> {
    let limit = Instant::now() + EXIT_GRACE;
    let mut extra = [0];
    loop {
        if Instant::now() >= limit {
            return Err(invalid());
        }
        if conversation.worker.readable(POLL_INTERVAL)? {
            if conversation.worker.read_available(&mut extra)? != 0 {
                return Err(invalid());
            }
            break;
        }
    }
    if !conversation.worker.wait_exit(limit)?.success() {
        return Err(invalid());
    }
    Ok(conversation.active())
}

#[cfg(test)]
#[path = "linux_archive_io/tests/supervision.rs"]
mod tests;
