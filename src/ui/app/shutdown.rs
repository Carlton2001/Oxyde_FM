//! Retain physical cleanup beyond GPUI's short quit-hook grace period.
use futures::{future::{BoxFuture, Shared}, FutureExt};
use oxyde_lib::models::HistoryManager;
use std::sync::{Arc, Mutex};

type Cleanup = Shared<BoxFuture<'static, ()>>;

#[derive(Default)]
struct PendingCleanup(Mutex<Option<Cleanup>>);

static HISTORY: PendingCleanup = PendingCleanup(Mutex::new(None));

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

/// Call after GPUI returns, while Tokio still owns its blocking worker pool.
pub(crate) fn finish_pending_cleanup() {
    HISTORY.finish();
}

#[cfg(test)]
#[path = "shutdown/tests/cleanup.rs"]
mod tests;
