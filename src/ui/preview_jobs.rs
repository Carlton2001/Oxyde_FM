//! Admission for UI previews. Waiting jobs do not occupy a worker thread.
//! Permits stay inside blocking work until it really exits, even if its caller drops.
use std::fmt::Display;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::runtime::Handle;
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};

const MAX_STILL_JOBS: usize = 4;
const MAX_ICON_JOBS: usize = 2;
const MAX_VIDEO_JOBS: usize = 1;

#[derive(Clone, Default)]
pub(crate) struct Cancellation {
    cancelled: Arc<AtomicBool>,
    changed: Arc<Notify>,
}

impl Cancellation {
    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        self.changed.notify_waiters();
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub(crate) async fn cancelled(&self) {
        let mut notified = Box::pin(self.changed.notified());
        // Register before inspecting the flag: cancel must not be lost between them.
        notified.as_mut().enable();
        if !self.is_cancelled() {
            notified.await;
        }
    }
}

#[derive(Clone)]
struct PreviewQueue {
    slots: Arc<Semaphore>,
}

impl PreviewQueue {
    fn new(limit: usize) -> Self {
        Self {
            slots: Arc::new(Semaphore::new(limit)),
        }
    }

    async fn admit(
        &self,
        cancellation: Option<&Cancellation>,
    ) -> Result<Option<OwnedSemaphorePermit>, String> {
        let acquire = Box::pin(self.slots.clone().acquire_owned());
        let permit = if let Some(cancellation) = cancellation {
            match futures::future::select(acquire, Box::pin(cancellation.cancelled())).await {
                futures::future::Either::Left((permit, _)) => permit,
                futures::future::Either::Right(_) => return Ok(None),
            }
        } else {
            acquire.await
        }
        .map_err(|error| format!("Preview admission failed: {error}"))?;
        if cancellation.is_some_and(Cancellation::is_cancelled) {
            return Ok(None);
        }
        Ok(Some(permit))
    }

    async fn run<T, E>(
        &self,
        runtime: Handle,
        cancellation: Option<Cancellation>,
        work: impl FnOnce() -> Result<T, E> + Send + 'static,
    ) -> Result<Option<T>, String>
    where
        T: Send + 'static,
        E: Display + Send + 'static,
    {
        let Some(permit) = self.admit(cancellation.as_ref()).await? else {
            return Ok(None);
        };
        runtime
            .spawn_blocking(move || {
                // Do not hold this permit in the awaiting future: dropping a join handle
                // cannot stop native work that has started.
                let _permit = permit;
                if cancellation
                    .as_ref()
                    .is_some_and(Cancellation::is_cancelled)
                {
                    return Ok(None);
                }
                work().map(Some).map_err(|error| error.to_string())
            })
            .await
            .map_err(|error| format!("Preview worker failed: {error}"))?
    }
}

fn still_queue() -> &'static PreviewQueue {
    static QUEUE: OnceLock<PreviewQueue> = OnceLock::new();
    QUEUE.get_or_init(|| PreviewQueue::new(MAX_STILL_JOBS))
}

fn video_queue() -> &'static PreviewQueue {
    static QUEUE: OnceLock<PreviewQueue> = OnceLock::new();
    QUEUE.get_or_init(|| PreviewQueue::new(MAX_VIDEO_JOBS))
}

fn media_queue() -> &'static PreviewQueue {
    static QUEUE: OnceLock<PreviewQueue> = OnceLock::new();
    QUEUE.get_or_init(|| PreviewQueue::new(2))
}

/// Long-lived Viewer playback must not occupy still-preview or hover-video slots.
pub(crate) fn load_media<T, E>(
    cancellation: Cancellation,
    work: impl FnOnce() -> Result<T, E> + Send + 'static,
) -> impl Future<Output = Result<Option<T>, String>> + Send
where T: Send + 'static, E: Display + Send + 'static {
    let runtime = Handle::try_current().map_err(|error| format!("Preview runtime unavailable: {error}"));
    async move { media_queue().run(runtime?, Some(cancellation), work).await }
}

fn icon_queue() -> &'static PreviewQueue {
    static QUEUE: OnceLock<PreviewQueue> = OnceLock::new();
    QUEUE.get_or_init(|| PreviewQueue::new(MAX_ICON_JOBS))
}

/// Construct on the UI thread where main's Tokio handle is entered. The returned
/// future is lazy and Send, and can be polled by GPUI without a Tokio thread context.
pub(crate) fn load<T, E>(
    work: impl FnOnce() -> Result<T, E> + Send + 'static,
) -> impl Future<Output = Result<T, String>> + Send
where
    T: Send + 'static,
    E: Display + Send + 'static,
{
    load_on(still_queue(), work)
}

/// Native icons have their own bounded admission so document previews cannot
/// delay even memory/disk icon hits. Shell requests remain sequential downstream.
pub(crate) fn load_icon<T, E>(
    work: impl FnOnce() -> Result<T, E> + Send + 'static,
) -> impl Future<Output = Result<T, String>> + Send
where
    T: Send + 'static,
    E: Display + Send + 'static,
{
    load_on(icon_queue(), work)
}

fn load_on<T, E>(
    queue: &'static PreviewQueue,
    work: impl FnOnce() -> Result<T, E> + Send + 'static,
) -> impl Future<Output = Result<T, String>> + Send
where
    T: Send + 'static,
    E: Display + Send + 'static,
{
    let runtime =
        Handle::try_current().map_err(|error| format!("Preview runtime unavailable: {error}"));
    async move {
        queue
            .run(runtime?, None, work)
            .await?
            .ok_or_else(|| "Preview job cancelled".into())
    }
}

pub(crate) fn load_video<T, E>(
    cancellation: Cancellation,
    work: impl FnOnce() -> Result<T, E> + Send + 'static,
) -> impl Future<Output = Result<Option<T>, String>> + Send
where
    T: Send + 'static,
    E: Display + Send + 'static,
{
    let runtime =
        Handle::try_current().map_err(|error| format!("Preview runtime unavailable: {error}"));
    async move { video_queue().run(runtime?, Some(cancellation), work).await }
}

#[cfg(test)]
#[path = "preview_jobs/tests/jobs.rs"]
mod tests;
