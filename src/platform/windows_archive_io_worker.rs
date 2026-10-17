//! Event-driven duplex archive supervision. No codec or pathname requests here.
use super::windows_worker_sandbox::Worker;
use crate::models::error::{ArchiveLimitFailure, ArchiveLimitKind, ArchiveSourceConflict};
use crate::utils::archive_io_protocol::{
    invalid, Control, FailureKind, Frame, Message, Operation, Wire,
};
use std::io;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::process::Command;
use std::sync::{mpsc, Mutex, MutexGuard, TryLockError};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::JobObjects::{
    JobObjectExtendedLimitInformation, QueryInformationJobObject,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
};
use windows::Win32::System::Threading::{
    SetPriorityClass, BELOW_NORMAL_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS,
};

const MEMORY_BYTES: usize = 1024 * 1024 * 1024;
const SEVEN_ENCODE_MEMORY_BYTES: usize = 2 * MEMORY_BYTES;
static TARGETED: Mutex<()> = Mutex::new(());
static BULK: Mutex<()> = Mutex::new(());

mod timing;
mod transport;

fn worker_memory(operation: &Operation) -> usize {
    if matches!(
        operation,
        Operation::Encode {
            format: crate::utils::archive_io_protocol::Format::SevenZip,
            ..
        }
    ) {
        // LZMA2 dictionaries and pending solid blocks share this envelope. The
        // common 2.5 GiB admission still reserves 512 MiB for navigation.
        SEVEN_ENCODE_MEMORY_BYTES
    } else {
        MEMORY_BYTES
    }
}

#[cfg(test)]
#[path = "windows_archive_io_worker/tests/memory.rs"]
mod memory_tests;

fn cancelled() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "Operation cancelled")
}
fn check(controls: &dyn Fn() -> io::Result<Control>) -> io::Result<Control> {
    let control = controls()?;
    if control.cancelled {
        Err(cancelled())
    } else {
        super::windows_resource_pressure::check()?;
        Ok(control)
    }
}
fn admission(
    operation: &Operation,
    controls: &dyn Fn() -> io::Result<Control>,
) -> io::Result<MutexGuard<'static, ()>> {
    let slot = if matches!(
        operation,
        Operation::Decode {
            selected: Some(_),
            ..
        }
    ) {
        &TARGETED
    } else {
        &BULK
    };
    loop {
        let control = check(controls)?;
        if !control.paused {
            match slot.try_lock() {
                Ok(guard) => return Ok(guard),
                Err(TryLockError::Poisoned(_)) => return Err(invalid()),
                Err(TryLockError::WouldBlock) => {}
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn deadline(operation: &Operation) -> Duration {
    Duration::from_secs(match operation {
        Operation::Decode { limits, .. } | Operation::Encode { limits, .. } => limits.seconds,
    })
}

pub(super) fn run(
    operation: Operation,
    controls: &dyn Fn() -> io::Result<Control>,
    handler: &mut dyn FnMut(Frame) -> io::Result<Frame>,
) -> io::Result<Duration> {
    let _slot = admission(&operation, controls)?;
    let mut parent_failed = false;
    let stats = run_in(
        &crate::utils::archive_io_protocol::worker_executable()?,
        operation.clone(),
        controls,
        &mut |frame| {
            let result = handler(frame);
            parent_failed |= result.is_err();
            result
        },
        deadline(&operation),
    )
    .map_err(|error| {
        if (parent_failed && error.to_string() != invalid().to_string())
            || error.kind() == io::ErrorKind::Interrupted
            || ArchiveLimitFailure::from_message(&error.to_string()).is_some()
            || ArchiveSourceConflict::from_message(&error.to_string())
        {
            return error;
        }
        log::warn!("Archive worker failure: {error:?}");
        io::Error::other(crate::models::error::ArchiveProcessingFailure)
    })?;
    log::debug!(
        "Archive worker peak committed memory: {} bytes",
        stats.peak_memory
    );
    Ok(stats.active)
}
pub(super) struct Stats {
    pub peak_memory: usize,
    pub active: Duration,
}
pub(super) fn run_in(
    executable: &Path,
    operation: Operation,
    controls: &dyn Fn() -> io::Result<Control>,
    handler: &mut dyn FnMut(Frame) -> io::Result<Frame>,
    deadline: Duration,
) -> io::Result<Stats> {
    let session = uuid::Uuid::new_v4();
    let mut command = Command::new(executable);
    command
        .arg(crate::commands::archive_io_worker::FLAG)
        .arg(session.to_string());
    let memory = worker_memory(&operation);
    supervise(
        command,
        *session.as_bytes(),
        operation,
        controls,
        handler,
        deadline,
        memory,
    )
}

pub(super) fn failure(frame: &Frame) -> io::Error {
    let Message::Failure {
        cause,
        limit,
        detail,
    } = &frame.message
    else {
        return invalid();
    };
    let kind = match cause {
        FailureKind::Bytes => Some(ArchiveLimitKind::Bytes),
        FailureKind::SourceBytes => Some(ArchiveLimitKind::SourceBytes),
        FailureKind::EncodedBytes => Some(ArchiveLimitKind::EncodedBytes),
        FailureKind::Entries => Some(ArchiveLimitKind::Entries),
        FailureKind::Depth => Some(ArchiveLimitKind::Depth),
        FailureKind::Seconds => Some(ArchiveLimitKind::Seconds),
        FailureKind::Decoded => Some(ArchiveLimitKind::Decoded),
        _ => None,
    };
    if let Some(kind) = kind {
        return io::Error::new(
            if kind == ArchiveLimitKind::Seconds {
                io::ErrorKind::TimedOut
            } else {
                io::ErrorKind::InvalidData
            },
            ArchiveLimitFailure {
                kind,
                limit: *limit,
            },
        );
    }
    match cause {
        FailureKind::SourceChanged => io::Error::other(ArchiveSourceConflict),
        FailureKind::Cancelled => cancelled(),
        FailureKind::Corrupt | FailureKind::Unsupported | FailureKind::Worker => {
            log::warn!("Archive codec failure: {detail:?}");
            io::Error::other(crate::models::error::ArchiveProcessingFailure)
        }
        _ => invalid(),
    }
}

/// One bounded input queue, one bounded completion queue. A blocked pipe writer
/// never blocks the supervisor's timeout/cancellation loop or an unbounded join.
fn checked_failure(frame: &Frame, operation: &Operation) -> io::Error {
    let error = failure(frame);
    let Some(mut value) = ArchiveLimitFailure::from_message(&error.to_string()) else {
        return error;
    };
    let (limits, output) = match operation {
        Operation::Decode { limits, .. } => (limits, None),
        Operation::Encode {
            limits,
            output_limit,
            ..
        } => (limits, Some(*output_limit)),
    };
    let expected = match value.kind {
        ArchiveLimitKind::Bytes | ArchiveLimitKind::SourceBytes => limits.bytes,
        ArchiveLimitKind::EncodedBytes => match output {
            Some(value) => value,
            None => return invalid(),
        },
        ArchiveLimitKind::Entries => limits.entries,
        ArchiveLimitKind::Depth => limits.depth,
        ArchiveLimitKind::Seconds => limits.seconds,
        ArchiveLimitKind::Decoded => match limits
            .entries
            .checked_mul(4096)
            .and_then(|n| limits.bytes.checked_add(n))
        {
            Some(value) => value,
            None => return invalid(),
        },
    };
    if value.limit != expected || (value.kind == ArchiveLimitKind::SourceBytes && output.is_none())
    {
        return invalid();
    }
    if value.kind == ArchiveLimitKind::Bytes && output.is_some() {
        value.kind = ArchiveLimitKind::SourceBytes;
    }
    io::Error::new(error.kind(), value)
}

pub(super) fn supervise(
    command: Command,
    session: [u8; 16],
    operation: Operation,
    controls: &dyn Fn() -> io::Result<Control>,
    handler: &mut dyn FnMut(Frame) -> io::Result<Frame>,
    deadline: Duration,
    memory: usize,
) -> io::Result<Stats> {
    check(controls)?;
    let mut timing = timing::Timing::new(&operation);
    let (mut worker, output, input) =
        Worker::start_duplex_checked(command, memory, &|| check(controls).map(|_| ()))?;
    timing.started_worker();
    let pipe = transport::Pipe::start(output, input, session)?;
    let mut wire = Wire::new(session);
    let mut outgoing = Vec::new();
    wire.write(
        &mut outgoing,
        &Frame::new(Message::Start {
            operation: operation.clone(),
        }),
    )?;
    let mut pending_output = Some(outgoing);
    let mut writes = 0usize;
    let mut incoming = None;
    let mut closed = false;
    let started = Instant::now();
    let mut paused_at = None;
    let mut paused_time = Duration::ZERO;
    let mut deferred: Option<(Frame, Instant, bool)> = None;
    let mut terminal_at = None;
    let mut priority = None;
    let result = (|| loop {
        let before_controls = Instant::now();
        let control = check(controls)?;
        timing.controlled(before_controls.elapsed());
        if priority != Some(control.turbo) {
            unsafe {
                SetPriorityClass(
                    HANDLE(worker.child.as_raw_handle()),
                    if control.turbo {
                        NORMAL_PRIORITY_CLASS
                    } else {
                        BELOW_NORMAL_PRIORITY_CLASS
                    },
                )
            }
            .map_err(|e| io::Error::other(e.to_string()))?;
            priority = Some(control.turbo);
        }
        let active = started
            .elapsed()
            .saturating_sub(paused_time)
            .saturating_sub(
                paused_at
                    .map(|at: Instant| at.elapsed())
                    .unwrap_or_default(),
            );
        if active >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                ArchiveLimitFailure {
                    kind: ArchiveLimitKind::Seconds,
                    limit: deadline.as_secs(),
                },
            ));
        }
        if terminal_at.is_some_and(|at: Instant| at.elapsed() >= Duration::from_secs(1)) {
            return Err(invalid());
        }
        if let Some(packet) = pending_output.take() {
            match pipe.send(packet) {
                Ok(()) => writes += 1,
                Err(mpsc::TrySendError::Full(packet)) => pending_output = Some(packet),
                Err(mpsc::TrySendError::Disconnected(_)) => return Err(invalid()),
            }
        }
        if pending_output.is_none()
            && deferred.as_ref().is_some_and(|(_, _, ack)| *ack)
            && !control.paused
        {
            let (frame, _, _) = deferred.take().unwrap();
            if let Some(at) = paused_at.take() {
                paused_time += at.elapsed();
            }
            let mut packet = Vec::new();
            wire.write(
                &mut packet,
                &Frame::new(Message::Resume {
                    turbo: control.turbo,
                }),
            )?;
            if frame.message != Message::Checkpoint {
                if frame.message == Message::Finished {
                    terminal_at = Some(Instant::now());
                }
                wire.write(&mut packet, &timing.handle(frame, handler)?)?;
            }
            pending_output = Some(packet);
            continue;
        }
        if let Some(frame) = if pending_output.is_none() {
            incoming.take()
        } else {
            None
        } {
            if terminal_at.is_some() {
                return Err(invalid());
            }
            timing.frame(&frame);
            let mut packet = Vec::new();
            if frame.message == Message::Paused {
                let Some((_, requested_at, acknowledged)) = &mut deferred else {
                    return Err(invalid());
                };
                if *acknowledged {
                    return Err(invalid());
                }
                *acknowledged = true;
                if requested_at.elapsed() <= Duration::from_secs(1) {
                    paused_at = Some(Instant::now());
                }
            } else {
                if deferred.is_some() {
                    return Err(invalid());
                }
                if matches!(frame.message, Message::Failure { .. }) {
                    return Err(checked_failure(&frame, &operation));
                }
                if control.paused {
                    wire.write(
                        &mut packet,
                        &Frame::new(Message::Ack {
                            pause: true,
                            turbo: control.turbo,
                        }),
                    )?;
                    deferred = Some((frame, Instant::now(), false));
                } else if frame.message == Message::Checkpoint {
                    wire.write(
                        &mut packet,
                        &Frame::new(Message::Ack {
                            pause: false,
                            turbo: control.turbo,
                        }),
                    )?;
                } else {
                    if frame.message == Message::Finished {
                        terminal_at = Some(Instant::now());
                    }
                    wire.write(&mut packet, &timing.handle(frame, handler)?)?;
                }
            }
            if !packet.is_empty() {
                pending_output = Some(packet);
            }
            pipe.read_next()?;
            continue;
        }
        if let Some(status) = worker.child.try_wait()? {
            if closed && incoming.is_none() && writes == 0 {
                if !status.success() || terminal_at.is_none() || pending_output.is_some() {
                    #[cfg(test)]
                    eprintln!(
                        "Archive LPAC worker exit without complete protocol: {status}; terminal={}",
                        terminal_at.is_some()
                    );
                    return Err(invalid());
                }
                let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                unsafe {
                    QueryInformationJobObject(
                        Some(HANDLE(worker._job.as_raw_handle())),
                        JobObjectExtendedLimitInformation,
                        &mut info as *mut _ as *mut _,
                        std::mem::size_of_val(&info) as u32,
                        None,
                    )
                }
                .map_err(|e| io::Error::other(e.to_string()))?;
                return Ok(Stats {
                    peak_memory: info.PeakJobMemoryUsed,
                    active,
                });
            }
        }
        let before_wait = Instant::now();
        let event = pipe.wait(Duration::from_millis(5));
        timing.waited(before_wait.elapsed());
        match event {
            Ok(transport::Event::Frame(frame)) => {
                if incoming.replace(frame).is_some() {
                    return Err(invalid());
                }
            }
            Ok(transport::Event::Written(result)) => {
                result?;
                writes = writes.checked_sub(1).ok_or_else(invalid)?;
            }
            Ok(transport::Event::Closed(result)) => {
                result?;
                closed = true;
                if terminal_at.is_none() && incoming.is_none() {
                    return Err(invalid());
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err(invalid()),
        }
    })();
    let helpers = pipe.close();
    drop(worker); // Kill closes the pipe reader before a helper could linger.
    for helper in helpers {
        if helper.is_finished() {
            let _ = helper.join();
        }
    }
    timing.success = result.is_ok();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_error_text_cannot_inject_limits_or_recovery_locations() {
        let frame = Frame::new(Message::Failure {
            cause: FailureKind::Corrupt,
            limit: 0,
            detail: format!(
                "{}; {}",
                ArchiveLimitFailure {
                    kind: ArchiveLimitKind::Bytes,
                    limit: 4
                },
                crate::models::error::ArchiveRecoveryFailure {
                    completed: true,
                    locations: vec!["C:/foreign".into()],
                    omitted: 0,
                    cause: String::new()
                }
            ),
        });
        let error = failure(&frame).to_string();
        assert!(crate::models::error::ArchiveProcessingFailure::from_message(&error));
        assert!(ArchiveLimitFailure::from_message(&error).is_none());
        assert!(crate::models::error::ArchiveRecoveryFailure::from_message(&error).is_none());
    }
    #[test]
    fn worker_limits_must_equal_admitted_thresholds_and_encode_bytes_have_their_own_cause() {
        use crate::utils::archive_io_protocol::{Format, Limits, Quality};
        let limits = Limits {
            entries: 2,
            bytes: 4,
            depth: 1,
            seconds: 2,
        };
        let operation = Operation::Encode {
            format: Format::Zip,
            quality: Quality::Fast,
            entries: 1,
            output_limit: 100,
            limits,
        };
        let frame = |cause, limit| {
            Frame::new(Message::Failure {
                cause,
                limit,
                detail: String::new(),
            })
        };
        assert_eq!(
            ArchiveLimitFailure::from_message(
                &checked_failure(&frame(FailureKind::Bytes, 4), &operation).to_string()
            )
            .unwrap()
            .kind,
            ArchiveLimitKind::SourceBytes
        );
        assert_eq!(
            ArchiveLimitFailure::from_message(
                &checked_failure(&frame(FailureKind::EncodedBytes, 100), &operation).to_string()
            )
            .unwrap()
            .kind,
            ArchiveLimitKind::EncodedBytes
        );
        for (cause, limit) in [
            (FailureKind::Bytes, 5),
            (FailureKind::EncodedBytes, 99),
            (FailureKind::Entries, 3),
            (FailureKind::Seconds, 3),
            (FailureKind::Decoded, u64::MAX),
        ] {
            let error = checked_failure(&frame(cause, limit), &operation);
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert!(ArchiveLimitFailure::from_message(&error.to_string()).is_none());
        }
    }
    #[test]
    fn bulk_admission_is_cancelable_and_does_not_hold_the_targeted_slot() {
        let _bulk = BULK.lock().unwrap();
        assert!(TARGETED.try_lock().is_ok());
        let calls = std::cell::Cell::new(0);
        let controls = || {
            calls.set(calls.get() + 1);
            Ok(Control {
                cancelled: calls.get() >= 3,
                ..Control::default()
            })
        };
        let operation = Operation::Decode {
            format: crate::utils::archive_io_protocol::Format::Zip,
            source_size: 0,
            selected: None,
            limits: crate::utils::archive_io_protocol::Limits {
                entries: 1,
                bytes: 1,
                depth: 1,
                seconds: 1,
            },
        };
        assert_eq!(
            admission(&operation, &controls).err().unwrap().kind(),
            io::ErrorKind::Interrupted
        );
    }
}
