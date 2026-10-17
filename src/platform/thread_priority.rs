use std::cell::Cell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadPriority {
    Background,
    Normal,
    High,
}

thread_local! {
    // Last priority applied through this module; `None` means untouched.
    static APPLIED: Cell<Option<ThreadPriority>> = const { Cell::new(None) };
}

/// Applies an absolute priority to the current thread, skipping a native call
/// when the thread already holds it.
pub fn apply_thread_priority(priority: ThreadPriority) {
    APPLIED.with(|applied| {
        if applied.get() != Some(priority) {
            super::platform().set_thread_priority(priority);
            applied.set(Some(priority));
        }
    });
}

/// Normal mode runs in background mode; Turbo leaves it for `turbo_priority`.
pub fn follow_turbo_priority(turbo: bool, turbo_priority: ThreadPriority) {
    apply_thread_priority(if turbo { turbo_priority } else { ThreadPriority::Background });
}

/// Restores normal priority when a worker scope ends, including on early return.
pub struct RestoreNormalPriority;

impl Drop for RestoreNormalPriority {
    fn drop(&mut self) {
        // Pool threads are reused: never leave them in background or high mode.
        if APPLIED.with(Cell::get).is_some_and(|priority| priority != ThreadPriority::Normal) {
            apply_thread_priority(ThreadPriority::Normal);
        }
    }
}

#[cfg(test)]
pub(super) fn applied_thread_priority() -> Option<ThreadPriority> {
    APPLIED.with(Cell::get)
}

#[cfg(test)]
#[path = "thread_priority/tests/transitions.rs"]
mod tests;
