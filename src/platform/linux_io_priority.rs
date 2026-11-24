//! Copy and deletion priority on Linux, through the I/O scheduler of the calling thread:
//! Background takes the lowest best-effort level, High the highest, Normal the level the
//! CPU priority gives. The CPU priority stays: an unprivileged thread cannot lower its
//! nice value again, and pool threads move between modes. Schedulers without levels
//! (`none`, `mq-deadline`) ignore them; BFQ, the desktop default of several
//! distributions, follows them.
use super::ThreadPriority;

const WHO_THREAD: libc::c_int = 1; // IOPRIO_WHO_PROCESS, `who` 0 being the calling thread.
const CLASS_SHIFT: libc::c_int = 13;
const CLASS_NONE: libc::c_int = 0;
const CLASS_BEST_EFFORT: libc::c_int = 2;

fn value(priority: ThreadPriority) -> libc::c_int {
    match priority {
        ThreadPriority::Background => CLASS_BEST_EFFORT << CLASS_SHIFT | 7,
        ThreadPriority::Normal => CLASS_NONE << CLASS_SHIFT,
        ThreadPriority::High => CLASS_BEST_EFFORT << CLASS_SHIFT,
    }
}

pub(super) fn set(priority: ThreadPriority) {
    // SAFETY: a system call on the calling thread, with integer arguments.
    let result = unsafe { libc::syscall(libc::SYS_ioprio_set, WHO_THREAD, 0, value(priority)) };
    if result != 0 {
        log::debug!("I/O priority {priority:?} not applied: {}", std::io::Error::last_os_error());
    }
}

#[cfg(test)]
#[path = "linux_io_priority/tests/levels.rs"]
mod tests;
