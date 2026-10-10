//! Headless codec dispatch: no native paths, user state, caches, or GPUI.
use crate::models::error::{ArchiveLimitFailure, ArchiveLimitKind, ArchiveSourceConflict};
use crate::utils::archive_io_protocol::{invalid, FailureKind, Frame, Message, Wire, MAX_ERROR};
use crate::utils::archive_io_stream::Exchange;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};

pub const FLAG: &str = "--oxyde-archive-io-worker-v1";

pub struct Client<R, W> {
    input: R,
    output: W,
    wire: Wire,
    turbo: bool,
}
impl<R: Read, W: Write> Client<R, W> {
    pub fn new(input: R, output: W, session: [u8; 16]) -> Self {
        Self {
            input,
            output,
            wire: Wire::new(session),
            turbo: false,
        }
    }
    fn perform(&mut self, frame: Frame) -> io::Result<Frame> {
        let checkpoint = frame.message == Message::Checkpoint;
        self.wire.write(&mut self.output, &frame)?;
        let reply = self.wire.read(&mut self.input)?;
        if let Message::Ack { pause: true, turbo } = reply.message {
            self.turbo = turbo;
            self.wire
                .write(&mut self.output, &Frame::new(Message::Paused))?;
            let resumed = self.wire.read(&mut self.input)?;
            let Message::Resume { turbo } = resumed.message else {
                return Err(invalid());
            };
            if !resumed.data.is_empty() {
                return Err(invalid());
            }
            self.turbo = turbo;
            if checkpoint {
                return Ok(Frame::new(Message::Ack {
                    pause: false,
                    turbo: false,
                }));
            }
            return self.wire.read(&mut self.input);
        }
        Ok(reply)
    }
}
impl<R: Read, W: Write> Exchange for Client<R, W> {
    fn exchange(&mut self, frame: Frame) -> io::Result<Frame> {
        // Rendezvous before every resource/content request, including codec
        // header reads and encoder flushes that bypass higher-level checkpoints.
        if frame.message != Message::Checkpoint {
            let reply = self.perform(Frame::new(Message::Checkpoint))?;
            let Message::Ack {
                pause: false,
                turbo,
            } = reply.message
            else {
                return Err(invalid());
            };
            if !reply.data.is_empty() {
                return Err(invalid());
            }
            self.turbo = turbo;
        }
        let reply = self.perform(frame)?;
        if let Message::Ack {
            pause: false,
            turbo,
        } = reply.message
        {
            self.turbo = turbo;
            return Ok(Frame::new(Message::Ack {
                pause: false,
                turbo: false,
            }));
        }
        Ok(reply)
    }
    fn turbo(&self) -> bool {
        self.turbo
    }
}

pub fn error_frame(error: &crate::models::CommandError) -> Frame {
    let detail = error.to_string();
    let (cause, limit) = if let Some(failure) = ArchiveLimitFailure::from_message(&detail) {
        (
            match failure.kind {
                ArchiveLimitKind::Bytes => FailureKind::Bytes,
                ArchiveLimitKind::Entries => FailureKind::Entries,
                ArchiveLimitKind::Depth => FailureKind::Depth,
                ArchiveLimitKind::Seconds => FailureKind::Seconds,
                ArchiveLimitKind::Decoded => FailureKind::Decoded,
            },
            failure.limit,
        )
    } else if ArchiveSourceConflict::from_message(&detail) {
        (FailureKind::SourceChanged, 0)
    } else {
        (FailureKind::Corrupt, 0)
    };
    let mut end = detail.len().min(MAX_ERROR);
    while !detail.is_char_boundary(end) {
        end -= 1;
    }
    Frame::new(Message::Failure {
        cause,
        limit,
        detail: detail[..end].to_owned(),
    })
}

pub fn run_if_requested() -> bool {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new(FLAG)) {
        return false;
    }
    let result = (|| -> io::Result<()> {
        let mut input = io::stdin();
        let mut gate = [0];
        input.read_exact(&mut gate)?;
        if gate != [0xA7] {
            return Err(invalid());
        }
        let session = arguments
            .next()
            .and_then(|arg| arg.into_string().ok())
            .ok_or_else(invalid)?;
        if arguments.next().is_some() {
            return Err(invalid());
        }
        let session = *uuid::Uuid::parse_str(&session)
            .map_err(|_| invalid())?
            .as_bytes();
        let mut client = Client::new(input, io::stdout(), session);
        let start = client.wire.read(&mut client.input)?;
        let Message::Start { operation } = start.message else {
            return Err(invalid());
        };
        if !start.data.is_empty() {
            return Err(invalid());
        }
        let connection = Arc::new(Mutex::new(client));
        if let Err(error) = super::archive_io_codec::run(connection.clone(), operation) {
            let mut client = connection.lock().map_err(|_| invalid())?;
            // The parent treats this as terminal and kills the session. Do not
            // demand another checkpoint/ack when reporting a failed conversation.
            let client = &mut *client;
            client
                .wire
                .write(&mut client.output, &error_frame(&error))?;
        }
        connection.lock().map_err(|_| invalid())?.output.flush()?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = writeln!(io::stderr(), "Archive IO worker failed: {error}");
    }
    let _ = crate::platform::platform().finish_process_shutdown();
    true
}
