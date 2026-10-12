//! Serialize the short interval in which process-specific handles are
//! inheritable. Rust's internal spawn mutex does not cover CreateProcessW.
use std::io;
use std::os::windows::io::{AsRawHandle, OwnedHandle};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{Mutex, MutexGuard};
use windows::Win32::Foundation::{SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT};

static LAUNCH: Mutex<()> = Mutex::new(());

fn lock() -> io::Result<MutexGuard<'static, ()>> {
    LAUNCH
        .lock()
        .map_err(|_| io::Error::other("Windows process launch lock poisoned"))
}

/// Field order closes every inheritable handle BEFORE releasing the lock,
/// including creation failures and unwinding. Parent endpoints never inherit.
pub(super) struct InheritedHandles {
    handles: [OwnedHandle; 3],
    _guard: MutexGuard<'static, ()>,
}

impl InheritedHandles {
    pub(super) fn new(handles: [OwnedHandle; 3]) -> io::Result<Self> {
        let inherited = Self {
            handles,
            _guard: lock()?,
        };
        for handle in inherited.raw() {
            unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT) }
                .map_err(|error| io::Error::other(error.to_string()))?;
        }
        Ok(inherited)
    }

    pub(super) fn raw(&self) -> [HANDLE; 3] {
        self.handles
            .each_ref()
            .map(|handle| HANDLE(handle.as_raw_handle()))
    }
}

/// Taking ownership also closes custom Stdio handles stored in Command before
/// another launch can begin. Waiting for the process happens outside the lock.
pub(super) fn spawn(mut command: Command) -> io::Result<Child> {
    let _guard = lock()?;
    let result = command.spawn();
    drop(command);
    result
}

/// The native callers of this helper use Command::output's default streams.
pub(super) fn output(mut command: Command) -> io::Result<Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    spawn(command)?.wait_with_output()
}

#[cfg(test)]
#[path = "windows_process_launch/tests/cross_inherit_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/support/native_probe.rs"]
#[allow(dead_code)]
pub(super) mod native_probe;
