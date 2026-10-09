//! Admission for UI previews. Waiting jobs do not occupy a worker thread.
//! Permits stay inside blocking work until it really exits, even if its caller drops.
use std::fmt::Display;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::runtime::Handle;
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};

const MAX_STILL_JOBS: usize = 4;
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

    async fn cancelled(&self) {
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

/// Construct on the UI thread where main's Tokio handle is entered. The returned
/// future is lazy and Send, and can be polled by GPUI without a Tokio thread context.
pub(crate) fn load<T, E>(
    work: impl FnOnce() -> Result<T, E> + Send + 'static,
) -> impl Future<Output = Result<T, String>> + Send
where
    T: Send + 'static,
    E: Display + Send + 'static,
{
    let runtime =
        Handle::try_current().map_err(|error| format!("Preview runtime unavailable: {error}"));
    async move {
        still_queue()
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
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::sync::atomic::AtomicUsize;
    use std::sync::{mpsc, Condvar, Mutex};
    use std::task::{Context, Poll};
    use std::time::Duration;

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .max_blocking_threads(16)
            .enable_all()
            .build()
            .unwrap()
    }

    fn poll_once<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
        future.poll(&mut Context::from_waker(futures::task::noop_waker_ref()))
    }

    #[derive(Clone, Default)]
    struct Blocker(Arc<(Mutex<bool>, Condvar)>);
    impl Blocker {
        fn wait(&self) {
            let (lock, changed) = &*self.0;
            let mut released = lock.lock().unwrap();
            while !*released {
                released = changed.wait(released).unwrap();
            }
        }
        fn release(&self) {
            let (lock, changed) = &*self.0;
            *lock.lock().unwrap() = true;
            changed.notify_all();
        }
    }
    struct ReleaseOnDrop(Blocker);
    impl Drop for ReleaseOnDrop {
        fn drop(&mut self) {
            self.0.release();
        }
    }
    struct Active(Arc<AtomicUsize>);
    impl Drop for Active {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn preview_jobs_bound_twelve_requests_and_leave_async_worker_available() {
        let runtime = runtime();
        let queue = PreviewQueue::new(MAX_STILL_JOBS);
        let blocker = Blocker::default();
        let _release = ReleaseOnDrop(blocker.clone());
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let (started, received) = mpsc::channel();
        let mut futures = Vec::new();
        for index in 0..12 {
            let blocker = blocker.clone();
            let active = active.clone();
            let peak = peak.clone();
            let started = started.clone();
            let mut future = Box::pin(queue.run(runtime.handle().clone(), None, move || {
                let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                let _active = Active(active);
                peak.fetch_max(count, Ordering::SeqCst);
                started.send(index).unwrap();
                blocker.wait();
                Ok::<_, String>(index)
            }));
            assert!(poll_once(future.as_mut()).is_pending());
            futures.push(future);
        }
        for _ in 0..MAX_STILL_JOBS {
            received.recv_timeout(Duration::from_secs(2)).unwrap();
        }
        assert_eq!(active.load(Ordering::SeqCst), MAX_STILL_JOBS);
        assert!(matches!(
            received.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        // A single async runtime worker still services unrelated work while all
        // preview slots are occupied by synchronous decoders.
        let (heartbeat, beat) = mpsc::channel();
        runtime.spawn(async move {
            heartbeat.send(()).unwrap();
        });
        beat.recv_timeout(Duration::from_secs(2)).unwrap();
        blocker.release();
        let outputs = runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(2), futures::future::join_all(futures))
                .await
                .unwrap()
        });
        for (index, output) in outputs.into_iter().enumerate() {
            assert_eq!(output.unwrap(), Some(index));
        }
        assert_eq!(active.load(Ordering::SeqCst), 0);
        assert!(peak.load(Ordering::SeqCst) <= MAX_STILL_JOBS);
    }

    #[test]
    fn preview_jobs_drop_waiter_never_runs_it_or_leaks_capacity() {
        let runtime = runtime();
        let queue = PreviewQueue::new(1);
        let occupied = queue.slots.clone().try_acquire_owned().unwrap();
        let called = Arc::new(AtomicBool::new(false));
        let worker_called = called.clone();
        let mut waiting = Box::pin(queue.run(runtime.handle().clone(), None, move || {
            worker_called.store(true, Ordering::SeqCst);
            Ok::<_, String>(())
        }));
        assert!(poll_once(waiting.as_mut()).is_pending());
        drop(waiting);
        drop(occupied);
        assert_eq!(
            runtime
                .block_on(queue.run(runtime.handle().clone(), None, || Ok::<_, String>(7)))
                .unwrap(),
            Some(7)
        );
        assert!(!called.load(Ordering::SeqCst));
    }

    #[test]
    fn preview_jobs_cancel_waiting_video_closes_channel_without_free_slot() {
        let runtime = runtime();
        let queue = PreviewQueue::new(MAX_VIDEO_JOBS);
        let occupied = queue.slots.clone().try_acquire_owned().unwrap();
        let cancellation = Cancellation::default();
        let called = Arc::new(AtomicBool::new(false));
        let worker_called = called.clone();
        let (sender, receiver) = mpsc::channel::<()>();
        let mut waiting = Box::pin(queue.run(
            runtime.handle().clone(),
            Some(cancellation.clone()),
            move || {
                worker_called.store(true, Ordering::SeqCst);
                sender.send(()).unwrap();
                Ok::<_, String>(())
            },
        ));
        assert!(poll_once(waiting.as_mut()).is_pending());
        cancellation.cancel();
        let result = runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(2), waiting)
                .await
                .unwrap()
        });
        assert_eq!(result.unwrap(), None);
        assert!(!called.load(Ordering::SeqCst));
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(2)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
        assert!(queue.slots.clone().try_acquire_owned().is_err());
        drop(occupied);
    }

    #[test]
    fn preview_jobs_cancel_before_poll_does_not_start_work() {
        let runtime = runtime();
        let queue = PreviewQueue::new(1);
        let cancellation = Cancellation::default();
        cancellation.cancel();
        let result = runtime.block_on(queue.run(
            runtime.handle().clone(),
            Some(cancellation),
            || -> Result<(), String> {
                panic!("Cancelled work must never execute");
            },
        ));
        assert_eq!(result.unwrap(), None);
        assert_eq!(queue.slots.available_permits(), 1);
    }

    #[test]
    fn preview_jobs_dropped_active_caller_keeps_slot_until_work_returns() {
        let runtime = runtime();
        let queue = PreviewQueue::new(1);
        let blocker = Blocker::default();
        let _release = ReleaseOnDrop(blocker.clone());
        let work_blocker = blocker.clone();
        let (started, received) = mpsc::channel();
        let mut active = Box::pin(queue.run(runtime.handle().clone(), None, move || {
            started.send(()).unwrap();
            work_blocker.wait();
            Ok::<_, String>(())
        }));
        assert!(poll_once(active.as_mut()).is_pending());
        received.recv_timeout(Duration::from_secs(2)).unwrap();
        drop(active);
        assert!(queue.slots.clone().try_acquire_owned().is_err());
        blocker.release();
        let result = runtime.block_on(async {
            tokio::time::timeout(
                Duration::from_secs(2),
                queue.run(runtime.handle().clone(), None, || Ok::<_, String>(9)),
            )
            .await
            .unwrap()
        });
        assert_eq!(result.unwrap(), Some(9));
    }

    #[test]
    fn preview_jobs_error_and_worker_panic_release_capacity() {
        let runtime = runtime();
        let queue = PreviewQueue::new(1);
        assert_eq!(
            runtime
                .block_on(queue.run(runtime.handle().clone(), None, || Err::<(), _>(
                    "fixture decode error"
                )))
                .unwrap_err(),
            "fixture decode error"
        );
        let result = runtime.block_on(queue.run(
            runtime.handle().clone(),
            None,
            || -> Result<(), String> {
                panic!("injected preview worker panic");
            },
        ));
        assert!(result.unwrap_err().contains("Preview worker failed"));
        assert_eq!(
            runtime
                .block_on(queue.run(runtime.handle().clone(), None, || Ok::<_, String>(1)))
                .unwrap(),
            Some(1)
        );
    }

    #[test]
    fn preview_jobs_local_loader_future_is_not_polled_before_admission() {
        let runtime = runtime();
        let queue = PreviewQueue::new(1);
        let occupied = queue.slots.clone().try_acquire_owned().unwrap();
        let polled = Arc::new(AtomicBool::new(false));
        let loader_polled = polled.clone();
        let loader = async move {
            loader_polled.store(true, Ordering::SeqCst);
            Ok::<_, String>(vec![1, 2, 3])
        };
        let mut job = Box::pin(queue.run(runtime.handle().clone(), None, move || {
            futures::executor::block_on(loader)
        }));
        assert!(poll_once(job.as_mut()).is_pending());
        assert!(!polled.load(Ordering::SeqCst));
        drop(occupied);
        assert_eq!(runtime.block_on(job).unwrap(), Some(vec![1, 2, 3]));
        assert!(polled.load(Ordering::SeqCst));
    }

    #[test]
    fn preview_jobs_capture_runtime_before_gpui_polls_future() {
        let runtime = runtime();
        let job = {
            let _entered = runtime.enter();
            load(|| Ok::<_, String>(42))
        };
        // Asset futures may be polled on a GPUI worker outside Tokio's context.
        assert!(Handle::try_current().is_err());
        assert_eq!(futures::executor::block_on(job).unwrap(), 42);
    }

    #[test]
    fn preview_jobs_missing_runtime_returns_error_without_calling_decoder() {
        assert!(Handle::try_current().is_err());
        let result = futures::executor::block_on(load(|| -> Result<(), String> {
            panic!("A missing runtime must not invoke the decoder");
        }));
        assert!(result.unwrap_err().contains("Preview runtime unavailable"));
    }
    #[test]
    fn preview_jobs_cancel_after_admission_before_blocking_worker_starts() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .max_blocking_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let queue = PreviewQueue::new(1);
        let blocker = Blocker::default();
        let _release = ReleaseOnDrop(blocker.clone());
        let work_blocker = blocker.clone();
        let (started, received) = mpsc::channel();
        let occupied_worker = runtime.spawn_blocking(move || {
            started.send(()).unwrap();
            work_blocker.wait();
        });
        received.recv_timeout(Duration::from_secs(2)).unwrap();
        let cancellation = Cancellation::default();
        let called = Arc::new(AtomicBool::new(false));
        let worker_called = called.clone();
        let mut job = Box::pin(queue.run(
            runtime.handle().clone(),
            Some(cancellation.clone()),
            move || {
                worker_called.store(true, Ordering::SeqCst);
                Ok::<_, String>(())
            },
        ));
        assert!(poll_once(job.as_mut()).is_pending());
        assert_eq!(queue.slots.available_permits(), 0);
        cancellation.cancel();
        blocker.release();
        assert_eq!(runtime.block_on(job).unwrap(), None);
        runtime.block_on(occupied_worker).unwrap();
        assert!(!called.load(Ordering::SeqCst));
        assert_eq!(queue.slots.available_permits(), 1);
    }

    #[test]
    fn preview_jobs_active_video_does_not_occupy_still_admission() {
        let runtime = runtime();
        let blocker = Blocker::default();
        let _release = ReleaseOnDrop(blocker.clone());
        let work_blocker = blocker.clone();
        let (started, received) = mpsc::channel();
        let cancellation = Cancellation::default();
        let (video, still) = {
            let _entered = runtime.enter();
            (
                load_video(cancellation, move || {
                    started.send(()).unwrap();
                    work_blocker.wait();
                    Ok::<_, String>(())
                }),
                load(|| Ok::<_, String>(8)),
            )
        };
        let mut video = Box::pin(video);
        assert!(poll_once(video.as_mut()).is_pending());
        received.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(
            runtime
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(2), still)
                        .await
                        .unwrap()
                })
                .unwrap(),
            8
        );
        blocker.release();
        assert_eq!(runtime.block_on(video).unwrap(), Some(()));
    }
}
