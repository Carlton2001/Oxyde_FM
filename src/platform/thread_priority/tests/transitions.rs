use super::*;

#[cfg(target_os = "windows")]
fn native_priority() -> i32 {
    unsafe { windows::Win32::System::Threading::GetThreadPriority(windows::Win32::System::Threading::GetCurrentThread()) }
}

#[cfg(target_os = "windows")]
fn expect_native(priority: ThreadPriority) {
    use windows::Win32::System::Threading::{THREAD_PRIORITY_HIGHEST, THREAD_PRIORITY_NORMAL};
    let native = native_priority();
    match priority {
        ThreadPriority::High => assert_eq!(native, THREAD_PRIORITY_HIGHEST.0),
        ThreadPriority::Normal => assert_eq!(native, THREAD_PRIORITY_NORMAL.0),
        // Background mode lowers the reported priority below normal.
        ThreadPriority::Background => assert!(native < THREAD_PRIORITY_NORMAL.0, "{native}"),
    }
}

#[cfg(not(target_os = "windows"))]
fn expect_native(_: ThreadPriority) {}

#[test]
fn turbo_start_applies_the_requested_priority_without_a_prior_background_phase() {
    std::thread::spawn(|| {
        for turbo_priority in [ThreadPriority::High, ThreadPriority::Normal] {
            std::thread::spawn(move || {
                follow_turbo_priority(true, turbo_priority);
                assert_eq!(applied_thread_priority(), Some(turbo_priority));
                expect_native(turbo_priority);
            })
            .join()
            .unwrap();
        }
    })
    .join()
    .unwrap();
}

#[test]
fn repeated_switches_return_to_absolute_normal_priority() {
    std::thread::spawn(|| {
        {
            let _restore = RestoreNormalPriority;
            for _ in 0..3 {
                follow_turbo_priority(false, ThreadPriority::High);
                expect_native(ThreadPriority::Background);
                follow_turbo_priority(false, ThreadPriority::High);
                expect_native(ThreadPriority::Background);
                follow_turbo_priority(true, ThreadPriority::High);
                expect_native(ThreadPriority::High);
            }
        }
        assert_eq!(applied_thread_priority(), Some(ThreadPriority::Normal));
        expect_native(ThreadPriority::Normal);
    })
    .join()
    .unwrap();
}

#[test]
fn restore_guard_leaves_an_untouched_thread_alone() {
    std::thread::spawn(|| {
        drop(RestoreNormalPriority);
        assert_eq!(applied_thread_priority(), None);
    })
    .join()
    .unwrap();
}
