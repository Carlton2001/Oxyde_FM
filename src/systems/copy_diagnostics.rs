//! Timings for synchronous copy paths. Nested flush time is part of native time.
use std::cell::RefCell;
use std::path::Path;
use std::time::{Duration, Instant};

const TARGET: &str = "oxyde_lib::copy_timing";

#[derive(Clone, Copy)]
pub(crate) enum Phase {
    Native,
    Flush,
    Hash,
    Publish,
    Pause,
    Parents,
    StagingCreate,
    WriterRelease,
    OutputOpen,
    Checks,
    Cleanup,
}

#[derive(Default)]
struct Measurements {
    native: Duration,
    flush: Duration,
    hash: Duration,
    publish: Duration,
    pause: Duration,
    flush_calls: usize,
    parents: Duration,
    staging_create: Duration,
    writer_release: Duration,
    output_open: Duration,
    checks: Duration,
    cleanup: Duration,
}
impl Measurements {
    fn record(&mut self, phase: Phase, elapsed: Duration, pause_before: Duration) {
        let active = elapsed.saturating_sub(self.pause.saturating_sub(pause_before));
        match phase {
            Phase::Native => self.native += active,
            Phase::Flush => {
                self.flush += active;
                self.flush_calls += 1;
            }
            Phase::Hash => self.hash += active,
            Phase::Publish => self.publish += active,
            Phase::Pause => self.pause += elapsed,
            Phase::Parents => self.parents += active,
            Phase::StagingCreate => self.staging_create += active,
            Phase::WriterRelease => self.writer_release += active,
            Phase::OutputOpen => self.output_open += active,
            Phase::Checks => self.checks += active,
            Phase::Cleanup => self.cleanup += active,
        }
    }

    fn remaining(&self, wall: Duration) -> Duration {
        // Flush is nested in native: count it only once.
        wall.saturating_sub(
            self.pause
                + self.native
                + self.hash
                + self.publish
                + self.parents
                + self.staging_create
                + self.writer_release
                + self.output_open
                + self.checks
                + self.cleanup,
        )
    }
}

thread_local! {
    static FILES: RefCell<Vec<Measurements>> = const { RefCell::new(Vec::new()) };
    static WORKER: RefCell<Option<(String, usize)>> = const { RefCell::new(None) };
}

/// Copies and their native callbacks stay on the same OS thread.
pub(crate) struct WorkerScope(Option<(String, usize)>);
impl WorkerScope {
    pub fn enter(operation: String, worker: usize) -> Self {
        Self(WORKER.with(|current| current.replace(Some((operation, worker)))))
    }
}
impl Drop for WorkerScope {
    fn drop(&mut self) {
        WORKER.with(|current| current.replace(self.0.take()));
    }
}

pub(crate) struct Timer {
    phase: Phase,
    started: Instant,
    pause_before: Duration,
    depth: usize,
}
impl Timer {
    pub fn start(phase: Phase) -> Self {
        let (depth, pause_before) = FILES.with(|files| {
            let files = files.borrow();
            (
                files.len(),
                files.last().map_or(Duration::ZERO, |file| file.pause),
            )
        });
        Self {
            phase,
            started: Instant::now(),
            pause_before,
            depth,
        }
    }
}
impl Drop for Timer {
    fn drop(&mut self) {
        FILES.with(|files| {
            let mut files = files.borrow_mut();
            if self.depth > 0 && files.len() == self.depth {
                files.last_mut().unwrap().record(
                    self.phase,
                    self.started.elapsed(),
                    self.pause_before,
                );
            }
        });
    }
}

pub(crate) struct FileTiming {
    started: Instant,
    source: std::path::PathBuf,
    destination: std::path::PathBuf,
    pub bytes: u64,
    pub published: bool,
}
impl FileTiming {
    pub fn start(source: &Path, destination: &Path) -> Self {
        FILES.with(|files| files.borrow_mut().push(Measurements::default()));
        Self {
            started: Instant::now(),
            source: source.into(),
            destination: destination.into(),
            bytes: 0,
            published: false,
        }
    }
}
impl Drop for FileTiming {
    fn drop(&mut self) {
        let elapsed = self.started.elapsed();
        let metrics = FILES
            .with(|files| files.borrow_mut().pop())
            .unwrap_or_default();
        WORKER.with(|worker| {
            let worker = worker.borrow();
            let (operation, index) = worker.as_ref().map_or(("unattributed", 0), |(id, index)| (id.as_str(), *index));
            log::info!(target: TARGET,
                "copy_file op={} worker={} source={:?} destination={:?} bytes={} published={} wall_s={:.6} active_s={:.6} pause_s={:.6} native_s={:.6} flush_s={:.6} flush_calls={} hash_s={:.6} publish_s={:.6} parents_s={:.6} staging_create_s={:.6} writer_release_s={:.6} output_open_s={:.6} checks_s={:.6} cleanup_s={:.6} remaining_s={:.6}",
                operation, index, self.source, self.destination, self.bytes, self.published,
                elapsed.as_secs_f64(), elapsed.saturating_sub(metrics.pause).as_secs_f64(),
                metrics.pause.as_secs_f64(), metrics.native.as_secs_f64(), metrics.flush.as_secs_f64(),
                metrics.flush_calls, metrics.hash.as_secs_f64(), metrics.publish.as_secs_f64(),
                metrics.parents.as_secs_f64(), metrics.staging_create.as_secs_f64(),
                metrics.writer_release.as_secs_f64(), metrics.output_open.as_secs_f64(),
                metrics.checks.as_secs_f64(), metrics.cleanup.as_secs_f64(), metrics.remaining(elapsed).as_secs_f64());
        });
    }
}

#[cfg(test)]
#[path = "copy_diagnostics/tests/timings.rs"]
mod tests;
