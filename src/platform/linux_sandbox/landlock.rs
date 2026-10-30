//! The confinement left where bubblewrap cannot run: Landlock limits the files, as bubblewrap's
//! mounts do, and scopes signals and abstract sockets; the seccomp filter limits the rest. The
//! parent prepares everything, since only system calls may run between fork and exec.
//! Weaker than namespaces: `/proc` is simply out of reach, and a worker allowed local sockets
//! can reach any socket by its path.
use std::ffi::c_long;
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

const EXECUTE: u64 = 1 << 0;
const WRITE_FILE: u64 = 1 << 1;
const READ_FILE: u64 = 1 << 2;
const READ_DIR: u64 = 1 << 3;
/// Removing and creating entries of every kind.
const CHANGE_DIR: u64 = 0x1ff0;
const REFER: u64 = 1 << 13;
const TRUNCATE: u64 = 1 << 14;
const IOCTL_DEV: u64 = 1 << 15;
const NET_TCP: u64 = 3;
const SCOPE_ABSTRACT_UNIX: u64 = 1;
const SCOPE_SIGNAL: u64 = 2;
const RULE_PATH_BENEATH: c_long = 1;
const CREATE_VERSION: c_long = 1;
const SECCOMP_SET_MODE_FILTER: c_long = 1;

const FILE_RIGHTS: u64 = EXECUTE | WRITE_FILE | READ_FILE | TRUNCATE | IOCTL_DEV;
pub(super) const READ: u64 = READ_FILE | READ_DIR;
pub(super) const RUN: u64 = READ | EXECUTE;
pub(super) const CHANGE: u64 = READ | WRITE_FILE | TRUNCATE | CHANGE_DIR | REFER;
pub(super) const DEVICE: u64 = READ_FILE | WRITE_FILE | TRUNCATE;

#[repr(C)]
struct RulesetAttributes {
    handled_fs: u64,
    handled_net: u64,
    scoped: u64,
}

#[repr(C, packed)]
struct PathBeneath {
    allowed: u64,
    parent: i32,
}

#[repr(C)]
struct FilterProgram {
    length: u16,
    instructions: *const u8,
}

/// The rights this kernel can restrict, by the version of its Landlock.
fn handled(version: i64) -> RulesetAttributes {
    let mut handled_fs = EXECUTE | WRITE_FILE | READ | CHANGE_DIR;
    for (since, right) in [(2, REFER), (3, TRUNCATE), (5, IOCTL_DEV)] {
        if version >= since {
            handled_fs |= right;
        }
    }
    RulesetAttributes {
        handled_fs,
        handled_net: if version >= 4 { NET_TCP } else { 0 },
        scoped: if version >= 6 { SCOPE_ABSTRACT_UNIX | SCOPE_SIGNAL } else { 0 },
    }
}

/// The size of the attributes this kernel knows.
fn attributes_size(version: i64) -> usize {
    match version {
        ..4 => 8,
        4..6 => 16,
        _ => 24,
    }
}

/// A set of rules being filled, or None where the kernel has no usable Landlock.
pub(super) struct Ruleset {
    descriptor: OwnedFd,
    handled_fs: u64,
}

impl Ruleset {
    pub(super) fn new() -> Option<Self> {
        // SAFETY: the version query takes no attributes.
        let version = unsafe {
            libc::syscall(libc::SYS_landlock_create_ruleset, std::ptr::null::<u8>(), 0usize, CREATE_VERSION)
        } as i64;
        if version < 1 {
            log::warn!("Landlock is unavailable: {}", io::Error::last_os_error());
            return None;
        }
        let attributes = handled(version);
        // SAFETY: attributes of at least the given size.
        let descriptor = unsafe {
            libc::syscall(libc::SYS_landlock_create_ruleset, &attributes as *const RulesetAttributes,
                attributes_size(version), 0 as c_long)
        };
        if descriptor < 0 {
            log::warn!("Landlock refused the rules: {}", io::Error::last_os_error());
            return None;
        }
        // SAFETY: a new descriptor owned by nothing else.
        let descriptor = unsafe { OwnedFd::from_raw_fd(descriptor as RawFd) };
        Some(Self { descriptor, handled_fs: attributes.handled_fs })
    }

    /// Grants `rights` on the path and what lies beneath it; a missing path grants nothing.
    pub(super) fn allow(&self, path: &Path, rights: u64) -> io::Result<()> {
        use rustix::fs::{Mode, OFlags};
        match rustix::fs::open(path, OFlags::PATH | OFlags::CLOEXEC, Mode::empty()) {
            Ok(file) => self.allow_file(&File::from(file), rights),
            Err(rustix::io::Errno::NOENT) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    pub(super) fn allow_file(&self, file: &File, rights: u64) -> io::Result<()> {
        // A file takes only the rights of a file, and the kernel refuses the others.
        let rights = if file.metadata()?.is_dir() { rights } else { rights & FILE_RIGHTS };
        let rule = PathBeneath { allowed: rights & self.handled_fs, parent: file.as_raw_fd() };
        // SAFETY: a valid descriptor and rule, read during the call.
        let added = unsafe {
            libc::syscall(libc::SYS_landlock_add_rule, self.descriptor.as_raw_fd(), RULE_PATH_BENEATH,
                &rule as *const PathBeneath, 0 as c_long)
        };
        if added != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub(super) fn into_descriptor(self) -> OwnedFd {
        self.descriptor
    }
}

/// Makes `command` confine itself before it starts. The descriptor must outlive the spawn.
pub(super) fn confine(command: &mut Command, ruleset: Option<RawFd>, filter: Option<&'static [u8]>) {
    // SAFETY: only system calls between fork and exec, on data that outlives them.
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
                return Err(io::Error::last_os_error());
            }
            if let Some(ruleset) = ruleset {
                if libc::syscall(libc::SYS_landlock_restrict_self, ruleset, 0 as c_long) != 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            if let Some(filter) = filter {
                // Eight bytes per instruction; a filter never comes near the 16-bit count.
                let program = FilterProgram { length: (filter.len() / 8) as u16, instructions: filter.as_ptr() };
                if libc::syscall(libc::SYS_seccomp, SECCOMP_SET_MODE_FILTER, 0 as c_long, &program as *const FilterProgram) != 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
}
