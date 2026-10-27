//! Archive-open cache cleanup. A file is evicted only under a write lease: the kernel
//! grants one only when no other open file description or mapping exists, from any
//! process, and holds back new openers until the lease is released. This is the Linux
//! counterpart of the exclusive Windows sharing check.
use std::{
    ffi::c_int,
    fs::{File, OpenOptions},
    io,
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    path::Path,
};

// asm-generic values, shared by x86, x86_64, arm, aarch64 and riscv.
const F_SETOWN: c_int = 8;
const F_SETSIG: c_int = 10;
const F_SETLEASE: c_int = 1024;
const F_GETLEASE: c_int = 1025;
const F_WRLCK: c_int = 1;
const O_NOFOLLOW: c_int = 0o400000;
const O_NONBLOCK: c_int = 0o4000;
const O_DIRECTORY: c_int = 0o200000;

extern "C" {
    fn fcntl(fd: c_int, cmd: c_int, ...) -> c_int;
}

fn checked(result: c_int) -> io::Result<c_int> {
    if result == -1 { Err(io::Error::last_os_error()) } else { Ok(result) }
}

/// Opens a cache entry for eviction without following links. A regular file comes
/// back leased; one still open elsewhere is refused with `WouldBlock`, after two short
/// retries for transient holders such as a process being spawned, which shares every
/// descriptor until it executes.
pub(super) fn open_cleanup(path: &Path, directory: bool) -> io::Result<File> {
    let mut attempt = 0;
    loop {
        match open_once(path, directory) {
            Err(error) if error.kind() == io::ErrorKind::WouldBlock && attempt < 2 => {
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            result => return result,
        }
    }
}

fn open_once(path: &Path, directory: bool) -> io::Result<File> {
    // Never block on a FIFO swapped in after the scan.
    let flags = O_NOFOLLOW | O_NONBLOCK | if directory { O_DIRECTORY } else { 0 };
    let file = OpenOptions::new().read(true).custom_flags(flags).open(path)?;
    let metadata = file.metadata()?;
    if directory {
        return if metadata.is_dir() { Ok(file) } else { Err(io::Error::other("Cache cleanup expected a directory")) };
    }
    if !metadata.is_file() {
        return Err(io::Error::other("Cache cleanup refuses special files"));
    }
    let fd = file.as_raw_fd();
    let urgent = rustix::process::Signal::URG.as_raw();
    // SAFETY: fcntl with integer arguments on a descriptor that `file` owns.
    unsafe {
        // A lease break signals the holder, SIGIO by default, which would end the
        // process: use a signal ignored by default, then name no owner at all.
        checked(fcntl(fd, F_SETSIG, urgent))?;
        checked(fcntl(fd, F_SETLEASE, F_WRLCK)).map_err(|error| match error.raw_os_error() {
            Some(code) if code == rustix::io::Errno::AGAIN.raw_os_error() => {
                io::Error::new(io::ErrorKind::WouldBlock, "Cache file is still open elsewhere")
            }
            _ => error,
        })?;
        checked(fcntl(fd, F_SETOWN, 0))?;
    }
    Ok(file)
}

/// Whether a cleanup file still holds its lease: another process starting to open it
/// breaks the lease, and that opener waits until the file is closed.
pub(super) fn still_exclusive(file: &File) -> io::Result<()> {
    if !file.metadata()?.is_file() {
        return Ok(());
    }
    // SAFETY: fcntl query on a descriptor that `file` owns.
    if checked(unsafe { fcntl(file.as_raw_fd(), F_GETLEASE) })? == F_WRLCK {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::WouldBlock, "Cache file was opened during cleanup"))
    }
}

#[cfg(test)]
#[path = "linux_cache/tests/lease.rs"]
mod tests;
