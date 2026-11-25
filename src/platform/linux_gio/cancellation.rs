//! A GCancellable that another thread cancels: GIO's loop and its synchronous calls
//! block the thread that runs them, so a flag or a deadline cannot be checked there.
use super::library::{Gio, Object};
use super::Owned;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

/// A GObject handed to the cancellation watcher.
#[derive(Clone, Copy)]
struct SendObject(Object);
// SAFETY: only `g_cancellable_cancel`, which GIO makes thread-safe, uses it there.
unsafe impl Send for SendObject {}

/// How a cancellable call ended.
pub(super) struct Watched<R> {
    pub(super) result: R,
    /// The limit passed and cancelled the call.
    pub(super) expired: bool,
}

/// Runs `run` with a cancellable, cancelled when `cancel` is set or once `limit` has
/// passed. `None` when GIO cannot create one.
pub(super) fn watched<R>(
    gio: &'static Gio, cancel: &AtomicBool, limit: Option<Duration>, run: impl FnOnce(Object) -> R,
) -> Option<Watched<R>> {
    // SAFETY: a new object, released when this call returns.
    let cancellable = Owned::new(gio, unsafe { (gio.g_cancellable_new)() })?;
    let deadline = limit.map(|limit| Instant::now() + limit);
    let expired = AtomicBool::new(false);
    let target = SendObject(cancellable.1);
    let (finished, over) = mpsc::channel::<()>();
    let result = std::thread::scope(|scope| {
        let expired = &expired;
        scope.spawn(move || {
            let target = target;
            // The call's end disconnects the channel and wakes the watcher at once.
            while let Err(RecvTimeoutError::Timeout) = over.recv_timeout(Duration::from_millis(100)) {
                let late = deadline.is_some_and(|deadline| Instant::now() >= deadline);
                if cancel.load(Ordering::Acquire) || late {
                    expired.store(late, Ordering::Release);
                    // SAFETY: the cancellable lives until this scope ends.
                    unsafe { (gio.g_cancellable_cancel)(target.0) };
                    return;
                }
            }
        });
        let result = run(cancellable.1);
        drop(finished);
        result
    });
    Some(Watched { result, expired: expired.load(Ordering::Acquire) })
}
