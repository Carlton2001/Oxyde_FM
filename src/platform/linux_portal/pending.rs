//! The calls Oxyde has not answered yet, which decide when the process may end: an
//! application would otherwise never learn the file chosen as the last window closed.
//! A call handed to the portal after Oxyde's waits for the user there, as long as it
//! takes, and keeps the process too, since the answer goes back on Oxyde's connection.
use futures::channel::oneshot;
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

struct Counts {
    /// Calls not answered yet, those forwarded included.
    waiting: usize,
    /// Calls the portal after Oxyde's answers.
    forwarded: usize,
}

static PENDING: (Mutex<Counts>, Condvar) = (Mutex::new(Counts { waiting: 0, forwarded: 0 }), Condvar::new());

/// Oxyde's interface can no longer show or answer a window, and the calls waiting for
/// one, the next ones too, go to the portal after Oxyde's.
static INTERFACE_FAILED: Mutex<(bool, Vec<oneshot::Sender<()>>)> = Mutex::new((false, Vec::new()));

fn counts() -> MutexGuard<'static, Counts> {
    PENDING.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// One call not answered yet, until it drops.
pub(super) struct Pending {
    forwarded: bool,
}

impl Pending {
    pub(super) fn start() -> Self {
        counts().waiting += 1;
        Pending { forwarded: false }
    }

    /// The portal after Oxyde's answers this call.
    pub(super) fn forward(&mut self) {
        if !std::mem::replace(&mut self.forwarded, true) {
            counts().forwarded += 1;
        }
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        let mut counts = counts();
        counts.waiting = counts.waiting.saturating_sub(1);
        if self.forwarded {
            counts.forwarded = counts.forwarded.saturating_sub(1);
        }
        PENDING.1.notify_all();
    }
}

/// Whether an application waits for a file from Oxyde, or a portal Oxyde asked for it.
pub(in crate::platform) fn answering() -> bool {
    counts().waiting > 0
}

/// At exit: the last answers get `limit` to reach their application, and calls the
/// portal after Oxyde's answers get the time the user takes there.
pub(super) fn wait_for_replies(limit: Duration) {
    let deadline = Instant::now() + limit;
    let mut counts = counts();
    while counts.waiting > 0 {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() && counts.forwarded == 0 {
            log::warn!("{} file chooser requests were still unanswered at exit", counts.waiting);
            return;
        }
        let wait = if left.is_zero() { Duration::from_secs(60) } else { left };
        counts = PENDING.1.wait_timeout(counts, wait).unwrap_or_else(|poisoned| poisoned.into_inner()).0;
    }
}

/// Without a window: returns once no call has waited for `quiet`.
pub(super) fn wait_until_idle(quiet: Duration) {
    let mut counts = counts();
    loop {
        counts = PENDING.1.wait_while(counts, |counts| counts.waiting > 0).unwrap_or_else(|poisoned| poisoned.into_inner());
        let (next, idle) = PENDING.1.wait_timeout_while(counts, quiet, |counts| counts.waiting == 0)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if idle.timed_out() {
            return;
        }
        counts = next;
    }
}

/// Resolves once Oxyde's interface has failed; at once when it already has.
pub(super) fn interface_failure() -> oneshot::Receiver<()> {
    let (sender, receiver) = oneshot::channel();
    let mut failed = INTERFACE_FAILED.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if failed.0 {
        let _ = sender.send(());
    } else {
        failed.1.retain(|waiting| !waiting.is_canceled());
        failed.1.push(sender);
    }
    receiver
}

/// The interface thread is failing: the calls waiting for its windows go to the portal
/// after Oxyde's, and this returns once all are answered.
pub(super) fn fail_interface() {
    {
        let mut failed = INTERFACE_FAILED.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        failed.0 = true;
        for waiting in failed.1.drain(..) {
            let _ = waiting.send(());
        }
    }
    let counts = counts();
    drop(PENDING.1.wait_while(counts, |counts| counts.waiting > 0));
}
