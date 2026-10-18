//! Best-effort icon persistence owned by the parent, after pixel validation.
use image::RgbaImage;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

const MAX_PENDING: usize = 32;
const MAX_PENDING_BYTES: usize = 4 * 1024 * 1024;
const SHUTDOWN_WAIT: Duration = Duration::from_secs(2);
static SERVICE: OnceLock<Result<Writer, String>> = OnceLock::new();
static STOPPING: AtomicBool = AtomicBool::new(false);

struct Job {
    directory: PathBuf,
    key: String,
    image: RgbaImage,
}

#[derive(Default)]
struct State {
    pending: VecDeque<Job>,
    bytes: usize,
    revision: u64,
    pauses: usize,
    active: bool,
    stopped: bool,
    done: bool,
}

struct Shared {
    state: Mutex<State>,
    changed: Condvar,
    entries: usize,
    bytes: usize,
}

#[derive(Clone)]
struct Writer(Arc<Shared>);

impl Writer {
    fn start(
        entries: usize,
        bytes: usize,
        mut publish: impl FnMut(Job) + Send + 'static,
    ) -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
            entries,
            bytes,
        });
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("icon-cache-writer".into())
            .spawn(move || {
                loop {
                    let job = {
                        let mut state = worker
                            .state
                            .lock()
                            .unwrap_or_else(|error| error.into_inner());
                        while !state.stopped && (state.pauses != 0 || state.pending.is_empty()) {
                            state = worker
                                .changed
                                .wait(state)
                                .unwrap_or_else(|error| error.into_inner());
                        }
                        if state.stopped {
                            state.done = true;
                            worker.changed.notify_all();
                            break;
                        }
                        let job = state.pending.pop_front().unwrap();
                        state.bytes -= job.image.as_raw().len();
                        state.active = true;
                        job
                    };
                    // A failed optional write must not strand clear/shutdown waiters.
                    let result =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| publish(job)));
                    let mut state = worker
                        .state
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    state.active = false;
                    if result.is_err() {
                        state.stopped = true;
                        state.pending.clear();
                        state.bytes = 0;
                        log::warn!("Icon cache writer panicked; persistence disabled");
                    }
                    worker.changed.notify_all();
                }
            })?;
        Ok(Self(shared))
    }

    fn revision(&self) -> Option<u64> {
        let state = self
            .0
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        (!state.stopped && state.pauses == 0).then_some(state.revision)
    }

    fn enqueue(&self, revision: u64, directory: &Path, key: &str, image: &RgbaImage) -> bool {
        let length = image.as_raw().len();
        if image.width() == 0
            || image.height() == 0
            || image.width() > 256
            || image.height() > 256
            || length > self.0.bytes
        {
            return false;
        }
        let mut state = self
            .0
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if state.stopped
            || state.pauses != 0
            || state.revision != revision
            || state.pending.len() >= self.0.entries
            || state.bytes > self.0.bytes - length
        {
            return false;
        }
        if state
            .pending
            .iter()
            .any(|job| job.directory == directory && job.key == key)
        {
            return true;
        }
        state.bytes += length;
        state.pending.push_back(Job {
            directory: directory.into(),
            key: key.into(),
            image: image.clone(),
        });
        self.0.changed.notify_all();
        true
    }

    fn pause(&self) -> Pause {
        let mut state = self
            .0
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.pauses += 1;
        state.revision = state.revision.wrapping_add(1);
        state.pending.clear();
        state.bytes = 0;
        self.0.changed.notify_all();
        // Runs on the cache-cleanup background thread, never the UI thread.
        while state.active {
            state = self
                .0
                .changed
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
        Pause(Some(self.0.clone()))
    }

    fn shutdown(&self, timeout: Duration) -> bool {
        let mut state = self
            .0
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.stopped = true;
        state.revision = state.revision.wrapping_add(1);
        state.pending.clear();
        state.bytes = 0;
        self.0.changed.notify_all();
        let (state, _) = self
            .0
            .changed
            .wait_timeout_while(state, timeout, |state| !state.done)
            .unwrap_or_else(|error| error.into_inner());
        state.done
    }
}

pub(super) struct Pause(Option<Arc<Shared>>);

impl Drop for Pause {
    fn drop(&mut self) {
        if let Some(shared) = &self.0 {
            let mut state = shared
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            state.pauses -= 1;
            shared.changed.notify_all();
        }
    }
}

fn service() -> Option<&'static Writer> {
    if STOPPING.load(Ordering::Acquire) {
        return None;
    }
    let writer = SERVICE
        .get_or_init(|| {
            Writer::start(MAX_PENDING, MAX_PENDING_BYTES, |job| {
                let _ = super::windows_icon_cache::write_cached_png(
                    &job.directory,
                    &job.key,
                    &job.image,
                );
            })
            .map_err(|error| {
                log::warn!("Cannot start icon cache writer: {error}");
                error.to_string()
            })
        })
        .as_ref()
        .ok()?;
    // A first request may race final shutdown while OnceLock is initializing.
    if STOPPING.load(Ordering::Acquire) {
        writer.shutdown(Duration::ZERO);
        None
    } else {
        Some(writer)
    }
}

pub(super) fn revision() -> Option<u64> {
    service().and_then(Writer::revision)
}

pub(super) fn enqueue(revision: u64, directory: &Path, key: &str, image: &RgbaImage) {
    if let Some(writer) = service() {
        writer.enqueue(revision, directory, key, image);
    }
}

pub(super) fn pause() -> Pause {
    service().map(Writer::pause).unwrap_or(Pause(None))
}

pub(super) fn shutdown() {
    STOPPING.store(true, Ordering::Release);
    if let Some(Ok(writer)) = SERVICE.get() {
        if !writer.shutdown(SHUTDOWN_WAIT) {
            log::warn!("Icon cache writer shutdown exceeded deadline; final process exit stops it");
        }
    }
}

#[cfg(test)]
pub(super) fn wait_idle(timeout: Duration) -> bool {
    let Some(writer) = service() else {
        return false;
    };
    let state = writer
        .0
        .state
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let (state, _) = writer
        .0
        .changed
        .wait_timeout_while(state, timeout, |state| {
            state.active || !state.pending.is_empty()
        })
        .unwrap_or_else(|error| error.into_inner());
    !state.active && state.pending.is_empty()
}

#[cfg(test)]
#[path = "windows_icon_persistence/tests/writer.rs"]
mod tests;
