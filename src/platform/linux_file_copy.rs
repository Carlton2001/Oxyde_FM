//! Copy one regular file's bytes and dates into a new output, reading
//! only the source descriptor the selection verified.
use rustix::io::Errno;
use std::{
    fs::{File, FileTimes, Permissions},
    io,
    os::unix::fs::{FileExt, MetadataExt, PermissionsExt},
    path::Path,
};

/// One kernel copy request: short enough to keep pause and cancel responsive.
const KERNEL_CHUNK: usize = 16 * 1024 * 1024;
const BUFFER: usize = 1024 * 1024;
/// Written bytes after which the previous window must reach the device.
const WRITEBACK_WINDOW: u64 = 16 * 1024 * 1024;
const SET_ID: u32 = 0o6000;

/// Keeps at most two windows of dirty pages, so progress follows the device
/// instead of the page cache and the final flush stays short (slow USB keys).
struct Writeback<'a> {
    destination_parent: &'a File,
    /// Decided when the first window fills: a smaller file never asks for the device.
    enabled: Option<bool>,
    started: u64,
    waited: u64,
}

impl<'a> Writeback<'a> {
    fn new(destination_parent: &'a File) -> Self {
        Self {
            destination_parent,
            enabled: None,
            started: 0,
            waited: 0,
        }
    }

    /// Only for destinations that are not solid-state (USB keys, hard disks, shares):
    /// waiting on a fast SSD would cost more than it shows.
    fn enabled(&mut self) -> bool {
        use std::os::fd::AsRawFd;
        *self.enabled.get_or_insert_with(|| {
            std::fs::read_link(format!("/proc/self/fd/{}", self.destination_parent.as_raw_fd()))
                .is_ok_and(|folder| !super::platform().volumes().is_ssd(&folder))
        })
    }

    fn advance(&mut self, output: &File, copied: u64) {
        use std::os::fd::AsRawFd;
        if copied < self.started + WRITEBACK_WINDOW || !self.enabled() {
            return;
        }
        let range = |from: u64, to: u64, flags: u32| {
            // A length of zero would mean "to the end of the file".
            if to <= from {
                return;
            }
            // Best effort: a file system without support is flushed by `sync_all` anyway.
            // SAFETY: plain system call on a descriptor this function borrows.
            unsafe {
                libc::sync_file_range(output.as_raw_fd(), from as i64, (to - from) as i64, flags)
            };
        };
        range(
            self.waited,
            self.started,
            libc::SYNC_FILE_RANGE_WAIT_BEFORE
                | libc::SYNC_FILE_RANGE_WRITE
                | libc::SYNC_FILE_RANGE_WAIT_AFTER,
        );
        range(self.started, copied, libc::SYNC_FILE_RANGE_WRITE);
        self.waited = self.started;
        self.started = copied;
    }
}

pub(super) fn copy(
    source: &Path,
    destination: &Path,
    held_source: Option<&File>,
    destination_parent: &File,
    created: &mut dyn FnMut(&File) -> io::Result<()>,
    progress: &mut dyn FnMut(u64) -> io::Result<()>,
) -> io::Result<()> {
    let opened;
    let input = match held_source {
        Some(file) => file,
        None => {
            opened = File::open(source)?;
            &opened
        }
    };
    let metadata = input.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::other("Copy source is not an ordinary file"));
    }
    let output = super::linux_held::create_file(destination_parent, destination)?;
    created(&output)?;
    // Explicit offsets: a held descriptor may already have been read to its end.
    let mut writeback = Writeback::new(destination_parent);
    let mut copied = kernel_copy(input, &output, &mut writeback, progress)?;
    copy_remaining(input, &output, &mut copied, &mut writeback, progress)?;
    {
        let _timer = crate::systems::copy_diagnostics::Timer::start(crate::systems::copy_diagnostics::Phase::Flush);
        output.sync_all()?;
    }
    super::linux_extended_attributes::copy(input, &output, source);
    set_mode(&output, mode(&metadata, &output.metadata()?))?;
    output.set_times(
        FileTimes::new()
            .set_accessed(metadata.accessed()?)
            .set_modified(metadata.modified()?),
    )
}

/// Lets the kernel share blocks (Btrfs, XFS) or copy without user-space buffers.
/// Returns the bytes copied before the end or a file system that refuses.
fn kernel_copy(
    input: &File,
    output: &File,
    writeback: &mut Writeback<'_>,
    progress: &mut dyn FnMut(u64) -> io::Result<()>,
) -> io::Result<u64> {
    let mut copied = 0;
    loop {
        progress(copied)?;
        let (mut from, mut to) = (copied, copied);
        match rustix::fs::copy_file_range(input, Some(&mut from), output, Some(&mut to), KERNEL_CHUNK) {
            Ok(0) => return Ok(copied),
            Ok(count) => {
                copied += count as u64;
                writeback.advance(output, copied);
            }
            // Other volumes, FUSE mounts such as ntfs-3g, FAT and older kernels.
            Err(Errno::XDEV | Errno::NOSYS | Errno::OPNOTSUPP | Errno::INVAL) => return Ok(copied),
            Err(error) => return Err(error.into()),
        }
    }
}

/// Reads until the actual end, which also covers files whose size reads as zero.
fn copy_remaining(
    input: &File,
    output: &File,
    copied: &mut u64,
    writeback: &mut Writeback<'_>,
    progress: &mut dyn FnMut(u64) -> io::Result<()>,
) -> io::Result<()> {
    let mut buffer = vec![0_u8; BUFFER];
    loop {
        progress(*copied)?;
        let count = match input.read_at(&mut buffer, *copied) {
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if count == 0 {
            return Ok(());
        }
        output.write_all_at(&buffer[..count], *copied)?;
        *copied += count as u64;
        writeback.advance(output, *copied);
    }
}

/// Attributes of a source directory, keeping read, write and search for the owner:
/// Undo and later transfers must still be able to change its content. The mode
/// comes after the ACL, so the owner's full access applies to both.
pub(super) fn restore_directory(directory: &File, source: &super::DirectoryAttributes) -> io::Result<()> {
    let extended = super::linux_extended_attributes::write(directory, &source.extended);
    let metadata = &source.metadata;
    set_mode(directory, mode(metadata, &directory.metadata()?) | 0o700)?;
    directory.set_times(
        FileTimes::new()
            .set_accessed(metadata.accessed()?)
            .set_modified(metadata.modified()?),
    )?;
    extended
}

/// FAT and exFAT refuse setuid, setgid and sticky bits with `EPERM`, and keep no other
/// bit: the copy goes on without them, as file managers copy to such volumes. A mount
/// with no modes at all, such as GVfs's FUSE view of a network share, answers
/// `EOPNOTSUPP`: the file keeps the mode the mount gives.
fn set_mode(file: &File, mode: u32) -> io::Result<()> {
    match file.set_permissions(Permissions::from_mode(mode)) {
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied && mode & 0o7000 != 0 => {
            file.set_permissions(Permissions::from_mode(mode & 0o777))
        }
        Err(error) if error.raw_os_error() == Some(Errno::OPNOTSUPP.raw_os_error()) => Ok(()),
        result => result,
    }
}

/// The source's mode; setuid and setgid only for a copy with the same owner and group.
fn mode(source: &std::fs::Metadata, output: &std::fs::Metadata) -> u32 {
    let mode = source.mode() & 0o7777;
    if source.uid() == output.uid() && source.gid() == output.gid() {
        mode
    } else {
        mode & !SET_ID
    }
}

#[cfg(test)]
#[path = "linux_file_copy/tests/copy.rs"]
mod tests;
