//! System calls a worker never needs, refused as Flatpak and GNOME's thumbnailers refuse
//! them: kernel administration, other processes' memory, mounts and namespaces, and all
//! sockets but local ones. libseccomp compiles the filter; bubblewrap installs it just
//! before the worker starts. Calls of another architecture end the worker.
use super::super::linux_library::library;
use std::ffi::{c_char, c_int, c_uint, c_void, CString};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::OnceLock;

#[repr(C)]
pub(super) struct Comparison {
    argument: c_uint,
    operation: c_int,
    first: u64,
    second: u64,
}

library! { Seccomp from "libseccomp.so.2" {
    seccomp_init: fn(u32) -> *mut c_void;
    seccomp_release: fn(*mut c_void);
    seccomp_syscall_resolve_name: fn(*const c_char) -> c_int;
    seccomp_rule_add_array: fn(*mut c_void, u32, c_int, c_uint, *const Comparison) -> c_int;
    seccomp_export_bpf: fn(*mut c_void, c_int) -> c_int;
}}

const ALLOW: u32 = 0x7fff_0000;
const NOT_EQUAL: c_int = 1;
const MASKED_EQUAL: c_int = 7;
const EPERM: u32 = 1;
const ENOSYS: u32 = 38;
const EAFNOSUPPORT: u32 = 97;
const AF_UNIX: u64 = 1;
/// Typing into, or reprogramming, a terminal the worker shares.
const TERMINAL_INJECTION: [u64; 2] = [0x5412, 0x541c];

const REFUSED: &[&str] = &[
    "acct", "add_key", "adjtimex", "bpf", "chroot", "clock_adjtime", "clock_settime", "delete_module",
    "fanotify_init", "finit_module", "get_mempolicy", "init_module", "io_uring_enter", "io_uring_register",
    "io_uring_setup", "ioperm", "iopl", "kcmp", "kexec_file_load", "kexec_load", "keyctl", "lookup_dcookie",
    "mbind", "migrate_pages", "mount", "move_pages", "name_to_handle_at", "nfsservctl", "open_by_handle_at",
    "perf_event_open", "pivot_root", "process_vm_readv", "process_vm_writev", "ptrace", "quotactl", "reboot",
    "request_key", "set_mempolicy", "setns", "settimeofday", "swapoff", "swapon", "syslog", "umount",
    "umount2", "unshare", "uselib", "userfaultfd", "vm86", "vm86old",
];
/// The newer mount interface, unknown to older kernels.
const UNKNOWN: &[&str] = &["fsconfig", "fsmount", "fsopen", "fspick", "mount_setattr", "move_mount", "open_tree"];

fn errno(code: u32) -> u32 {
    0x0005_0000 | code
}

/// The compiled filter, or None, logged once, where libseccomp is missing.
pub(super) fn filter() -> Option<&'static [u8]> {
    static FILTER: OnceLock<Option<Vec<u8>>> = OnceLock::new();
    FILTER.get_or_init(|| compile(true).inspect_err(|error| log::warn!("Preview workers run without a system call filter: {error}")).ok())
        .as_deref()
}

/// The same filter refusing every socket, for workers that bubblewrap cannot cut off from the
/// user's local sockets and that have none to reach.
pub(super) fn filter_without_sockets() -> Option<&'static [u8]> {
    static FILTER: OnceLock<Option<Vec<u8>>> = OnceLock::new();
    FILTER.get_or_init(|| compile(false).inspect_err(|error| log::warn!("Preview workers run without a system call filter: {error}")).ok())
        .as_deref()
}

/// A descriptor bubblewrap reads the filter from.
pub(super) fn descriptor(filter: &[u8]) -> io::Result<OwnedFd> {
    let (reader, writer) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC)?;
    // A few kilobytes, well within a pipe's buffer.
    File::from(writer).write_all(filter)?;
    Ok(reader)
}

fn compile(local_sockets: bool) -> io::Result<Vec<u8>> {
    let library = Seccomp::load().map_err(io::Error::other)?;
    // SAFETY: a fresh context, released below.
    let context = unsafe { (library.seccomp_init)(ALLOW) };
    if context.is_null() {
        return Err(io::Error::other("libseccomp could not start a filter"));
    }
    let result = rules(&library, context, local_sockets).and_then(|()| {
        let (reader, writer) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC)?;
        // SAFETY: a valid context and a descriptor open for writing.
        let exported = unsafe { (library.seccomp_export_bpf)(context, writer.as_raw_fd()) };
        drop(writer);
        if exported != 0 {
            return Err(io::Error::from_raw_os_error(-exported));
        }
        let mut filter = Vec::new();
        File::from(reader).read_to_end(&mut filter)?;
        Ok(filter)
    });
    // SAFETY: the context created above, no longer used.
    unsafe { (library.seccomp_release)(context) };
    result
}

fn rules(library: &Seccomp, context: *mut c_void, local_sockets: bool) -> io::Result<()> {
    let add = |name: &str, action: u32, comparisons: &[Comparison]| -> io::Result<()> {
        let symbol = CString::new(name).expect("no NUL in system call names");
        // SAFETY: a NUL-terminated name.
        let number = unsafe { (library.seccomp_syscall_resolve_name)(symbol.as_ptr()) };
        if number < 0 {
            // Absent from this architecture.
            return Ok(());
        }
        // SAFETY: a valid context and `comparisons.len()` comparisons.
        match unsafe { (library.seccomp_rule_add_array)(context, action, number, comparisons.len() as c_uint, comparisons.as_ptr()) } {
            0 => Ok(()),
            error => Err(io::Error::other(format!("libseccomp refused the rule on {name}: {}", io::Error::from_raw_os_error(-error)))),
        }
    };
    for name in REFUSED {
        add(name, errno(EPERM), &[])?;
    }
    for name in UNKNOWN {
        add(name, errno(ENOSYS), &[])?;
    }
    if local_sockets {
        add("socket", errno(EAFNOSUPPORT), &[Comparison { argument: 0, operation: NOT_EQUAL, first: AF_UNIX, second: 0 }])?;
    } else {
        add("socket", errno(EAFNOSUPPORT), &[])?;
    }
    for request in TERMINAL_INJECTION {
        add("ioctl", errno(EPERM), &[Comparison { argument: 1, operation: MASKED_EQUAL, first: 0xffff_ffff, second: request }])?;
    }
    // Reading the execution domain stays possible; changing it, to drop address
    // randomization for instance, does not.
    add("personality", errno(EPERM), &[Comparison { argument: 0, operation: NOT_EQUAL, first: 0xffff_ffff, second: 0 }])
}
