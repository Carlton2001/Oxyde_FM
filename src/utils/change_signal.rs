//! A counter that wakes whoever waits on it: system notifications (volumes, quick access)
//! bump it from their own threads, and the interface waits for it instead of polling.

use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Notify;

#[derive(Default)]
pub struct ChangeSignal {
    value: AtomicU64,
    changed: Notify,
}

impl ChangeSignal {
    pub fn new() -> Self {
        Self::default()
    }

    /// No allocation or blocking: callable from a system callback.
    pub fn notify(&self) {
        self.value.fetch_add(1, Ordering::Release);
        self.changed.notify_one();
    }

    pub fn value(&self) -> u64 {
        self.value.load(Ordering::Acquire)
    }

    /// Returns at once if a change came since the previous wait, so that one landing
    /// between reading `value` and waiting is not lost. Meant for a single waiter.
    pub async fn changed(&self) {
        self.changed.notified().await;
    }
}

#[cfg(test)]
#[path = "change_signal/tests/signal.rs"]
mod tests;
