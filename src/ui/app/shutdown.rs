//! Retain physical cleanup beyond GPUI's short quit-hook grace period.
use futures::{future::{BoxFuture, Shared}, FutureExt};
use oxyde_lib::models::HistoryManager;
use std::sync::{Arc, Mutex};

pub(super) type Cleanup = Shared<BoxFuture<'static, ()>>;

#[derive(Default)]
struct PendingCleanup(Mutex<Option<Cleanup>>);

static HISTORY: PendingCleanup = PendingCleanup(Mutex::new(None));
static PREFERENCES: PendingCleanup = PendingCleanup(Mutex::new(None));

impl PendingCleanup {
    fn schedule(&self, work: impl FnOnce() + Send + 'static) -> Cleanup {
        let worker = tokio::task::spawn_blocking(work);
        let cleanup = async move {
            if let Err(error) = worker.await {
                log::error!("Shutdown cleanup failed: {error}");
            }
        }.boxed().shared();
        *self.0.lock().unwrap_or_else(|error| error.into_inner()) = Some(cleanup.clone());
        cleanup
    }

    fn finish(&self) {
        let cleanup = self.0.lock().unwrap_or_else(|error| error.into_inner()).take();
        if let Some(cleanup) = cleanup {
            futures::executor::block_on(cleanup);
        }
    }
}

pub(super) fn begin_history_cleanup(history: Arc<HistoryManager>) -> Cleanup {
    HISTORY.schedule(move || {
        crate::trace_shutdown_stage("history cleanup started");
        if history.clear() {
            crate::trace_shutdown_stage("history cleanup completed");
        } else {
            crate::trace_shutdown_stage("history cleanup deferred to active worker");
        }
    })
}

/// GPUI runs no foreground task while it waits for quit hooks, so the last
/// preferences are written here rather than by the queued save chain.
pub(super) fn begin_preferences_write(write: impl FnOnce() + Send + 'static) -> Cleanup {
    PREFERENCES.schedule(write)
}

/// Call after GPUI returns, while Tokio still owns its blocking worker pool.
pub(crate) fn finish_pending_cleanup() {
    PREFERENCES.finish();
    HISTORY.finish();
}

impl super::OxydeApp {
    /// At start, removes the partial output an ended Oxyde left in its staging
    /// folders (`staging_journal`), then says where.
    pub(crate) fn recover_interrupted_operations(&mut self, cx: &mut gpui::Context<Self>) {
        cx.spawn(async move |this, cx| {
            let recovered = cx.background_executor().spawn(async { oxyde_lib::systems::staging_journal::recover() }).await;
            if recovered.removed.is_empty() {
                return;
            }
            log::info!("Interrupted staging removed: {:?}", recovered.removed);
            let mut folders: Vec<String> = Vec::new();
            for parent in recovered.removed.iter().filter_map(|path| path.parent()) {
                let parent = parent.to_string_lossy().into_owned();
                if !folders.contains(&parent) {
                    folders.push(parent);
                }
            }
            let _ = this.update(cx, |this, cx| {
                this.toast(
                    super::toasts::ToastKind::Success,
                    crate::ui::i18n::Message::InterruptedOutputRemoved,
                    folders.join(", "),
                    recovered.removed.len(),
                    cx,
                );
            });
        })
        .detach();
    }
}

#[cfg(test)]
#[path = "shutdown/tests/cleanup.rs"]
mod tests;
