use super::ChangeSignal;
use std::sync::Arc;
use std::time::Duration;

#[test]
fn a_change_before_the_wait_is_not_lost() {
    let signal = ChangeSignal::new();
    signal.notify();
    assert_eq!(signal.value(), 1);
    futures::executor::block_on(signal.changed());
}

#[test]
fn a_change_from_another_thread_wakes_the_waiter() {
    let signal = Arc::new(ChangeSignal::new());
    let notifier = signal.clone();
    let thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        notifier.notify();
    });
    futures::executor::block_on(signal.changed());
    assert_eq!(signal.value(), 1);
    thread.join().unwrap();
}
