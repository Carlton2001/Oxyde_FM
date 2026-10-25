use super::*;
use std::{sync::mpsc, time::Duration};

#[test]
fn blocked_cleanup_leaves_the_caller_free_and_survives_a_dropped_quit_hook() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _entered = runtime.enter();
    let pending = Arc::new(PendingCleanup::default());
    let caller = std::thread::current().id();
    let (started, starts) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let hook = pending.schedule(move || {
        started.send(std::thread::current().id()).unwrap();
        wait.recv().unwrap();
    });
    assert_ne!(starts.recv_timeout(Duration::from_secs(2)).unwrap(), caller);
    assert!(hook.clone().now_or_never().is_none());
    drop(hook); // GPUI may drop its future after its 200 ms grace period.
    let (finished, completion) = mpsc::channel();
    let barrier = pending.clone();
    let join = std::thread::spawn(move || {
        barrier.finish();
        finished.send(()).unwrap();
    });
    let early = completion.recv_timeout(Duration::from_millis(30));
    // Always release the worker, including when the assertion below fails.
    release.send(()).unwrap();
    completion.recv_timeout(Duration::from_secs(2)).unwrap();
    join.join().unwrap();
    assert!(matches!(early, Err(mpsc::RecvTimeoutError::Timeout)));
    pending.finish(); // The final barrier is idempotent.
}
