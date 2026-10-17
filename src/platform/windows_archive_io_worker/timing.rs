//! Observations only: event waits can overlap worker computation, not additive CPU time.
use crate::utils::archive_io_protocol::{Frame, Message, Operation};
use std::{
    io,
    time::{Duration, Instant},
};

pub(super) struct Timing {
    started: Instant,
    mode: &'static str,
    setup: Duration,
    frames: u64,
    reads: u64,
    writes: u64,
    checkpoints: u64,
    read_bytes: u64,
    write_bytes: u64,
    handler: Duration,
    controls: Duration,
    sleeps: u64,
    sleep: Duration,
    pub success: bool,
}

impl Timing {
    pub fn new(operation: &Operation) -> Self {
        Self {
            started: Instant::now(),
            mode: match operation {
                Operation::Encode { .. } => "encode",
                _ => "decode",
            },
            setup: Duration::ZERO,
            frames: 0,
            reads: 0,
            writes: 0,
            checkpoints: 0,
            read_bytes: 0,
            write_bytes: 0,
            handler: Duration::ZERO,
            controls: Duration::ZERO,
            sleeps: 0,
            sleep: Duration::ZERO,
            success: false,
        }
    }
    pub fn started_worker(&mut self) {
        self.setup = self.started.elapsed();
    }
    pub fn frame(&mut self, frame: &Frame) {
        self.frames += 1;
        match frame.message {
            Message::Read { length, .. } => {
                self.reads += 1;
                self.read_bytes += u64::from(length);
            }
            Message::Write { .. } => {
                self.writes += 1;
                self.write_bytes += frame.data.len() as u64;
            }
            Message::Checkpoint => self.checkpoints += 1,
            _ => {}
        }
    }
    pub fn handle(
        &mut self,
        frame: Frame,
        handler: &mut dyn FnMut(Frame) -> io::Result<Frame>,
    ) -> io::Result<Frame> {
        let started = Instant::now();
        let result = handler(frame);
        self.handler += started.elapsed();
        result
    }
    pub fn waited(&mut self, duration: Duration) {
        self.sleeps += 1;
        self.sleep += duration;
    }
    pub fn controlled(&mut self, duration: Duration) {
        self.controls += duration;
    }
}

impl Drop for Timing {
    fn drop(&mut self) {
        log::info!(target: "oxyde_lib::archive_timing",
            "archive_worker_timing mode={} success={} wall_s={:.6} setup_s={:.6} handler_s={:.6} controls_s={:.6} transport_wait_s={:.6} transport_waits={} frames={} reads={} read_bytes={} writes={} write_bytes={} checkpoints={}",
            self.mode, self.success, self.started.elapsed().as_secs_f64(), self.setup.as_secs_f64(),
            self.handler.as_secs_f64(), self.controls.as_secs_f64(), self.sleep.as_secs_f64(), self.sleeps, self.frames,
            self.reads, self.read_bytes, self.writes, self.write_bytes, self.checkpoints);
    }
}

#[cfg(test)]
#[path = "tests/timing.rs"]
mod tests;
