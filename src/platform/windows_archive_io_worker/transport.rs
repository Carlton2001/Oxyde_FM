//! Blocking pipe helpers wake the supervisor through one bounded event queue.
//! Neither helper calls application code, and the supervisor never waits on pipe I/O.
use crate::utils::archive_io_protocol::{Frame, Wire};
use std::{
    io::{self, Read, Write},
    sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError},
    thread::{self, JoinHandle},
    time::Duration,
};

pub(super) enum Event {
    Frame(Frame),
    Written(io::Result<()>),
    Closed(io::Result<()>),
}

pub(super) struct Pipe {
    commands: SyncSender<Vec<u8>>,
    reads: SyncSender<()>,
    events: Receiver<Event>,
    reader: JoinHandle<()>,
    writer: JoinHandle<()>,
}

// Distinguish EOF between frames from a truncated header/body. Wire validates the
// session, sequence and lengths before allocating the metadata and payload.
fn next_frame(reader: &mut impl Read, wire: &mut Wire) -> io::Result<Option<Frame>> {
    let mut first = [0];
    loop {
        match reader.read(&mut first) {
            Ok(0) => return Ok(None),
            Ok(_) => {
                return wire
                    .read(&mut io::Cursor::new(first).chain(reader))
                    .map(Some)
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
}

impl Pipe {
    pub fn start(
        mut output: impl Read + Send + 'static,
        mut input: impl Write + Send + 'static,
        session: [u8; 16],
    ) -> io::Result<Self> {
        let (commands, receive) = mpsc::sync_channel::<Vec<u8>>(1);
        let (reads, requested) = mpsc::sync_channel(1);
        reads.try_send(()).map_err(io::Error::other)?;
        let (send, events) = mpsc::sync_channel(1);
        let written = send.clone();
        let writer = thread::Builder::new()
            .name("archive-io-write".into())
            .spawn(move || {
                while let Ok(bytes) = receive.recv() {
                    let result = input.write_all(&bytes).and_then(|_| input.flush());
                    let failed = result.is_err();
                    if written.send(Event::Written(result)).is_err() || failed {
                        break;
                    }
                }
            })?;
        let reader = thread::Builder::new()
            .name("archive-io-read".into())
            .spawn(move || {
                let mut wire = Wire::new(session);
                while requested.recv().is_ok() {
                    let event = match next_frame(&mut output, &mut wire) {
                        Ok(Some(frame)) => Event::Frame(frame),
                        result => {
                            let _ = send.send(Event::Closed(result.map(|_| ())));
                            break;
                        }
                    };
                    if send.send(event).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            commands,
            reads,
            events,
            reader,
            writer,
        })
    }

    pub fn send(&self, bytes: Vec<u8>) -> Result<(), TrySendError<Vec<u8>>> {
        self.commands.try_send(bytes)
    }
    // A reader receives one credit only after the supervisor consumes the prior
    // frame. A flooded stdout cannot occupy the event queue indefinitely and
    // starve the completion that releases a blocked outgoing packet.
    pub fn read_next(&self) -> io::Result<()> {
        self.reads.try_send(()).map_err(io::Error::other)
    }
    pub fn wait(&self, timeout: Duration) -> Result<Event, RecvTimeoutError> {
        self.events.recv_timeout(timeout)
    }

    // Disconnect both bounded queues before killing the child. A helper blocked
    // on a queue is released; a helper in pipe I/O is released by child/job exit.
    // Return handles so the caller can join only those already finished.
    pub fn close(self) -> [JoinHandle<()>; 2] {
        let Self {
            commands,
            reads,
            events,
            reader,
            writer,
        } = self;
        drop(commands);
        drop(reads);
        drop(events);
        [reader, writer]
    }
}

#[cfg(test)]
#[path = "tests/transport.rs"]
mod tests;
