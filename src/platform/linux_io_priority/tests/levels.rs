use super::*;

fn current() -> libc::c_long {
    // SAFETY: a system call on the calling thread.
    unsafe { libc::syscall(libc::SYS_ioprio_get, WHO_THREAD, 0) }
}

#[test]
fn each_mode_sets_the_threads_level_and_normal_goes_back() {
    std::thread::spawn(|| {
        let initial = current();
        set(ThreadPriority::Background);
        assert_eq!(current(), libc::c_long::from(CLASS_BEST_EFFORT << CLASS_SHIFT | 7));
        set(ThreadPriority::High);
        assert_eq!(current(), libc::c_long::from(CLASS_BEST_EFFORT << CLASS_SHIFT));
        set(ThreadPriority::Normal);
        assert_eq!(current(), initial);
    })
    .join()
    .unwrap();
}
