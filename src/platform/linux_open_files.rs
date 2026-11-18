//! The open file limit. Planning a copy keeps a descriptor per source folder that holds a
//! planned file and per destination folder, so a tree of a thousand folders exceeds the
//! soft limit of 1024 most sessions start with. Oxyde raises it to the hard limit, as
//! systemd advises for programs that need it, and gives programs it starts the soft limit
//! it received: one that waits on `select` breaks past descriptor 1023.
use rustix::process::{getrlimit, setrlimit, Resource, Rlimit};
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

/// The kernel's default ceiling (`fs.nr_open`), for a hard limit reported as unlimited.
const UNLIMITED_CEILING: u64 = 1 << 20;
/// No raise happened: children keep the limit they inherit.
const NOT_RAISED: u64 = u64::MAX;

/// The soft limit Oxyde started with, when it raised it.
static RECEIVED: AtomicU64 = AtomicU64::new(NOT_RAISED);

/// Raises the soft limit to the hard one; returns what changed, for the log.
pub(super) fn raise() -> Option<String> {
    let limit = getrlimit(Resource::Nofile);
    let received = limit.current?;
    let target = limit.maximum.unwrap_or(UNLIMITED_CEILING);
    if target <= received {
        return None;
    }
    match setrlimit(Resource::Nofile, Rlimit { current: Some(target), maximum: limit.maximum }) {
        Ok(()) => {
            RECEIVED.store(received, Ordering::Relaxed);
            Some(format!("open file limit raised from {received} to {target}"))
        }
        Err(error) => Some(format!("open file limit kept at {received}: {error}")),
    }
}

/// Applications Oxyde opens receive the soft limit it received, as if opened by the desktop.
pub(super) fn leave_out_of(command: &mut Command) {
    let received = RECEIVED.load(Ordering::Relaxed);
    if received == NOT_RAISED {
        return;
    }
    // SAFETY: two raw system calls between fork and exec.
    unsafe {
        command.pre_exec(move || {
            restore(received);
            Ok(())
        });
    }
}

/// The same for a child GLib starts: a `GSpawnChildSetupFunc`, run between fork and exec.
pub(super) unsafe extern "C" fn restore_in_child(_data: *mut std::ffi::c_void) {
    let received = RECEIVED.load(Ordering::Relaxed);
    if received != NOT_RAISED {
        restore(received);
    }
}

/// A failure leaves the raised limit, which the child can live with.
fn restore(received: u64) {
    let maximum = getrlimit(Resource::Nofile).maximum;
    let _ = setrlimit(Resource::Nofile, Rlimit { current: Some(received), maximum });
}

#[cfg(test)]
#[path = "linux_open_files/tests/limit.rs"]
mod tests;
