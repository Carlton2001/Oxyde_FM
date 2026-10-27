//! Copy one regular file's bytes and dates into a new private output, reading
//! only the source descriptor the selection verified.
use rustix::io::Errno;
use std::{
    fs::{File, FileTimes, OpenOptions, Permissions},
    io,
    os::unix::fs::{FileExt, MetadataExt, PermissionsExt},
    path::Path,
};

/// One kernel copy request: short enough to keep pause and cancel responsive.
const KERNEL_CHUNK: usize = 16 * 1024 * 1024;
const BUFFER: usize = 1024 * 1024;
const SET_ID: u32 = 0o6000;

pub(super) fn copy(
    source: &Path,
    destination: &Path,
    held_source: Option<&File>,
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
    let output = OpenOptions::new().write(true).create_new(true).open(destination)?;
    created(&output)?;
    // Explicit offsets: a held descriptor may already have been read to its end.
    let mut copied = kernel_copy(input, &output, progress)?;
    copy_remaining(input, &output, &mut copied, progress)?;
    {
        let _timer = crate::systems::copy_diagnostics::Timer::start(crate::systems::copy_diagnostics::Phase::Flush);
        output.sync_all()?;
    }
    output.set_permissions(Permissions::from_mode(mode(&metadata, &output.metadata()?)))?;
    output.set_times(
        FileTimes::new()
            .set_accessed(metadata.accessed()?)
            .set_modified(metadata.modified()?),
    )
}

/// Lets the kernel share blocks (Btrfs, XFS) or copy without user-space buffers.
/// Returns the bytes copied before the end or a file system that refuses.
fn kernel_copy(input: &File, output: &File, progress: &mut dyn FnMut(u64) -> io::Result<()>) -> io::Result<u64> {
    let mut copied = 0;
    loop {
        progress(copied)?;
        let (mut from, mut to) = (copied, copied);
        match rustix::fs::copy_file_range(input, Some(&mut from), output, Some(&mut to), KERNEL_CHUNK) {
            Ok(0) => return Ok(copied),
            Ok(count) => copied += count as u64,
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
    }
}

/// Mode and dates of a source directory, keeping read, write and search for the
/// owner: Undo and later transfers must still be able to change its content.
pub(super) fn restore_directory(directory: &File, source: &std::fs::Metadata) -> io::Result<()> {
    directory.set_permissions(Permissions::from_mode(mode(source, &directory.metadata()?) | 0o700))?;
    directory.set_times(
        FileTimes::new()
            .set_accessed(source.accessed()?)
            .set_modified(source.modified()?),
    )
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
