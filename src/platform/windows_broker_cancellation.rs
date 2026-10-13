//! Cooperative native interruption of synchronous archive broker calls.
//! Never kill a parent thread or release its object guards while I/O is pending.
use std::{
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
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
struct Reservation;
impl Drop for Reservation {
    fn drop(&mut self) {
        ACTIVE.fetch_sub(1, Ordering::AcqRel);
    }
}
struct Monitor {
    done: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Drop for Monitor {
    fn drop(&mut self) {
        self.done.store(true, Ordering::Release);
        // Join before the caller can reuse its thread for unrelated I/O.
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
pub(super) fn watch(
    remaining: Duration,
    cancel: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
) -> io::Result<super::IoCancellationGuard> {
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
    let done = Arc::new(AtomicBool::new(false));
    let stopped = done.clone();
    let thread = std::thread::Builder::new()
        .name("oxyde-broker-cancel".into())
        .spawn(move || {
            let _reservation = reservation;
            let mut previous = Instant::now();
            let mut elapsed = Duration::ZERO;
            while !stopped.load(Ordering::Acquire) {
                let now = Instant::now();
                if !pause.load(Ordering::Acquire) {
                    elapsed = elapsed.saturating_add(now.duration_since(previous));
                }
                previous = now;
                if cancel.load(Ordering::Acquire) || elapsed >= remaining {
                    // Reissue to cover a cancellation arriving between two native
                    // calls. The monitor belongs to this broker scope only.
                    let _ = unsafe { CancelSynchronousIo(HANDLE(target.as_raw_handle())) };
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        })?;
    Ok(super::IoCancellationGuard {
        _state: Box::new(Monitor {
            done,
            thread: Some(thread),
        }),
    })
}

#[cfg(test)]
#[path = "windows_broker_cancellation/tests/native_io.rs"]
mod tests;
