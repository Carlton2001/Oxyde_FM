use super::*;
use std::{sync::mpsc, time::Duration};

#[test]
fn dropping_the_guard_ends_the_watch() {
    let mounts = std::fs::File::open("/proc/self/mountinfo").unwrap();
    let (stopped, stop) = pipe_with(PipeFlags::CLOEXEC).unwrap();
    let (finished, completion) = mpsc::channel();
    std::thread::spawn(move || {
        run(&mounts, &stopped, &AtomicU64::new(0));
        finished.send(()).unwrap();
    });
    assert!(matches!(completion.recv_timeout(Duration::from_millis(50)), Err(mpsc::RecvTimeoutError::Timeout)));
    drop(Watch { _stop: stop });
    completion.recv_timeout(Duration::from_secs(2)).unwrap();
}
