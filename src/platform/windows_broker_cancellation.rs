//! Cooperative native interruption, scoped to the thread that owns broker I/O.
//! Nested scopes share one monitor and keep independent deadlines/cancel flags.
use std::{
    cell::RefCell,
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    },
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::{DuplicateHandle, DUPLICATE_SAME_ACCESS, HANDLE},
    System::{
        Threading::{GetCurrentProcess, GetCurrentThread},
        IO::CancelSynchronousIo,
    },
};
const MAX_MONITORS: usize = 8;
static ACTIVE: AtomicUsize = AtomicUsize::new(0);
thread_local! { static CURRENT: RefCell<Weak<Monitor>> = const { RefCell::new(Weak::new()) }; }
struct Reservation;
impl Drop for Reservation {
    fn drop(&mut self) {
        ACTIVE.fetch_sub(1, Ordering::AcqRel);
    }
}
struct Spec {
    id: usize,
    limit: Duration,
    elapsed: Duration,
    previous: Instant,
    cancel: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
}
struct State {
    done: AtomicBool,
    scopes: Mutex<Vec<Spec>>,
    next: AtomicUsize,
}
struct Monitor {
    state: Arc<State>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Drop for Monitor {
    fn drop(&mut self) {
        self.state.done.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
struct Scope {
    monitor: Arc<Monitor>,
    id: usize,
}
impl Drop for Scope {
    fn drop(&mut self) {
        // Removal also waits for any cancellation request from this scope, so
        // it cannot interrupt the next read in a still-live outer scope.
        self.monitor
            .state
            .scopes
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .retain(|spec| spec.id != self.id);
    }
}
fn create(remaining: Duration, cancel: &AtomicBool) -> io::Result<Arc<Monitor>> {
    let admission = Instant::now();
    while ACTIVE
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
            (active < MAX_MONITORS).then_some(active + 1)
        })
        .is_err()
    {
        if cancel.load(Ordering::Acquire) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Operation cancelled",
            ));
        }
        if admission.elapsed() >= remaining.min(Duration::from_secs(30)) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Archive broker monitor budget exceeded",
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let reservation = Reservation;
    let mut handle = HANDLE::default();
    unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            GetCurrentThread(),
            GetCurrentProcess(),
            &mut handle,
            0,
            false,
            DUPLICATE_SAME_ACCESS,
        )
    }
    .map_err(|error| io::Error::other(error.to_string()))?;
    let target = unsafe { OwnedHandle::from_raw_handle(handle.0) };
    let state = Arc::new(State {
        done: AtomicBool::new(false),
        scopes: Mutex::new(Vec::new()),
        next: AtomicUsize::new(0),
    });
    let monitored = state.clone();
    let thread = std::thread::Builder::new()
        .name("oxyde-broker-cancel".into())
        .spawn(move || {
            let _reservation = reservation;
            while !monitored.done.load(Ordering::Acquire) {
                let now = Instant::now();
                let mut scopes = monitored
                    .scopes
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                let mut stop = false;
                for spec in &mut *scopes {
                    if !spec.pause.load(Ordering::Acquire) {
                        spec.elapsed = spec
                            .elapsed
                            .saturating_add(now.duration_since(spec.previous));
                    }
                    spec.previous = now;
                    stop |= spec.cancel.load(Ordering::Acquire) || spec.elapsed >= spec.limit;
                }
                if stop {
                    // Order requests with scope removal. Drivers can still refuse
                    // cancellation; guards remain alive until native I/O returns.
                    let _ = unsafe { CancelSynchronousIo(HANDLE(target.as_raw_handle())) };
                }
                drop(scopes);
                std::thread::sleep(Duration::from_millis(10));
            }
        })?;
    Ok(Arc::new(Monitor {
        state,
        thread: Some(thread),
    }))
}
pub(super) fn watch(
    remaining: Duration,
    cancel: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
) -> io::Result<super::IoCancellationGuard> {
    let monitor = match CURRENT.with(|current| current.borrow().upgrade()) {
        Some(monitor) => monitor,
        None => {
            let monitor = create(remaining, &cancel)?;
            CURRENT.with(|current| *current.borrow_mut() = Arc::downgrade(&monitor));
            monitor
        }
    };
    let id = monitor
        .state
        .next
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |id| id.checked_add(1))
        .map_err(|_| io::Error::other("Broker cancellation scope IDs exhausted"))?;
    {
        let mut scopes = monitor
            .state
            .scopes
            .lock()
            .map_err(|_| io::Error::other("Broker cancellation monitor poisoned"))?;
        if scopes.len() >= 16 {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Broker cancellation nesting exceeded",
            ));
        }
        scopes.push(Spec {
            id,
            limit: remaining,
            elapsed: Duration::ZERO,
            previous: Instant::now(),
            cancel,
            pause,
        });
    }
    Ok(super::IoCancellationGuard {
        _state: Box::new(Scope { monitor, id }),
    })
}
#[cfg(test)]
#[path = "windows_broker_cancellation/tests/native_io.rs"]
mod tests;
