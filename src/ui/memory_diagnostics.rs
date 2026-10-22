//! Opt-in, bounded weak pixel tracking. Samples run off the UI thread.
//! Logical pixel/cache totals overlap and must never be added to Private Bytes.
use gpui::{App, Global, RenderImage, Task};
use std::{
    collections::HashMap,
    sync::{atomic::{AtomicU64, Ordering}, Arc, Mutex, OnceLock, Weak},
    time::Duration,
};

const MAX_TRACKED: usize = 4096;
static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
static REVISION: AtomicU64 = AtomicU64::new(0);
static MISSED: AtomicU64 = AtomicU64::new(0);
static EVENT: Mutex<&'static str> = Mutex::new("startup");

pub(crate) fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("OXYDE_MEMORY_DIAGNOSTICS").is_some_and(|value| value == "1"))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum State { Decoded, Shown, Retired }

struct Entry {
    image: Weak<RenderImage>,
    bytes: usize,
    state: State,
}

#[derive(Default)]
struct Registry {
    entries: HashMap<usize, Entry>,
    dropped: usize,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Pixels {
    live_images: usize,
    live_bytes: usize,
    pending_bytes: usize,
    retired_images: usize,
    retired_bytes: usize,
    untracked_registrations: usize,
}

impl Registry {
    fn track(&mut self, image: &Arc<RenderImage>, state: State) {
        self.entries.retain(|_, entry| entry.image.strong_count() > 0);
        if let Some(entry) = self.entries.get_mut(&image.id.0) {
            entry.state = state;
            return;
        }
        if self.entries.len() >= MAX_TRACKED {
            self.dropped += 1;
            return;
        }
        let bytes = (0..image.frame_count()).filter_map(|frame| image.as_bytes(frame)).map(<[u8]>::len).sum();
        self.entries.insert(image.id.0, Entry { image: Arc::downgrade(image), bytes, state });
    }

    fn snapshot(&mut self) -> Pixels {
        self.entries.retain(|_, entry| entry.image.strong_count() > 0);
        let mut pixels = Pixels { untracked_registrations: self.dropped, ..Default::default() };
        for entry in self.entries.values() {
            pixels.live_images += 1;
            pixels.live_bytes += entry.bytes;
            if entry.state == State::Decoded { pixels.pending_bytes += entry.bytes; }
            if entry.state == State::Retired {
                pixels.retired_images += 1;
                pixels.retired_bytes += entry.bytes;
            }
        }
        pixels
    }
}

pub(crate) fn track(image: &Arc<RenderImage>, state: State) {
    if !enabled() { return; }
    if let Ok(mut registry) = REGISTRY.get_or_init(Default::default).try_lock() {
        registry.track(image, state);
    } else {
        MISSED.fetch_add(1, Ordering::Relaxed);
    }
    REVISION.fetch_add(1, Ordering::Relaxed);
}

fn pixels() -> Option<Pixels> {
    let mut snapshot = match REGISTRY.get() {
        None => Some(Pixels::default()),
        Some(registry) => registry.try_lock().ok().map(|mut registry| registry.snapshot()),
    };
    if let Some(pixels) = snapshot.as_mut() {
        pixels.untracked_registrations += MISSED.load(Ordering::Relaxed) as usize;
    }
    snapshot
}

pub(crate) fn checkpoint(event: &'static str) {
    if enabled() {
        REVISION.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut current) = EVENT.try_lock() { *current = event; }
    }
}

fn sample(reason: &str) {
    let event = EVENT.try_lock().ok().map(|event| *event);
    let process = oxyde_lib::platform::platform().process_memory_snapshot();
    let viewer = pixels();
    let preview_budget = crate::ui::preview_budget::diagnostic_snapshot();
    let tooltip_caches = crate::ui::components::tooltip::diagnostic_cache_snapshot();
    let audio_cache = crate::ui::audio_covers::diagnostic_snapshot();
    log::info!("memory-sample: reason={reason}; last_event={event:?}; process_bytes={process:?}; viewer_pixels={viewer:?}; preview_budget_entries_bytes={preview_budget:?}; tooltip_cache_entries_bytes={tooltip_caches:?}; audio_cache_unique_entries_bytes={audio_cache:?}");
}

struct Monitor { _task: Task<()> }
impl Global for Monitor {}

pub(crate) fn start(cx: &mut App) {
    if !enabled() || cx.has_global::<Monitor>() { return; }
    log::info!("memory diagnostics enabled; byte totals overlap; None means busy/unavailable; GPU atlas and heap allocations are not measured individually");
    let executor = cx.background_executor().clone();
    let task = executor.clone().spawn(async move {
        sample("startup");
        let mut previous = REVISION.load(Ordering::Relaxed);
        let mut idle = 0u64;
        loop {
            executor.timer(Duration::from_secs(1)).await;
            let revision = REVISION.load(Ordering::Relaxed);
            if revision != previous {
                previous = revision;
                idle = 0;
                sample("activity");
            } else {
                idle += 1;
                if idle == 5 || idle == 30 || idle.is_multiple_of(60) {
                    sample("idle");
                }
            }
        }
    });
    cx.set_global(Monitor { _task: task });
}

#[cfg(test)]
#[path = "memory_diagnostics/tests/pixels.rs"]
mod tests;
