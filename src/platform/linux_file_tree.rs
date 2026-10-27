//! Permanent deletion of a tree through descriptors: links are removed, never
//! followed; a folder goes after its content, through its held parent; another file
//! system mounted inside is never entered. Pause and cancellation are checked before
//! each item.
use rustix::{
    fs::{AtFlags, FileType, Mode, OFlags},
    io::Errno,
};
use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io,
    os::{fd::OwnedFd, unix::ffi::OsStrExt},
    path::{Path, PathBuf},
};

struct Frame {
    dir: rustix::fs::Dir,
    fd: OwnedFd,
    path: PathBuf,
}

fn located(path: &Path, error: impl Into<io::Error>) -> io::Error {
    let error = error.into();
    io::Error::new(error.kind(), format!("{}: {error}", path.display()))
}

/// Opens a child folder that `stat` described, refusing a replacement or another mount.
fn enter(parent: &OwnedFd, name: &OsStr, path: &Path, device: u64, inode: u64) -> io::Result<Frame> {
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let fd = rustix::fs::openat(parent, name, flags, Mode::empty()).map_err(|error| located(path, error))?;
    let stat = rustix::fs::fstat(&fd).map_err(|error| located(path, error))?;
    if (stat.st_dev, stat.st_ino) != (device, inode) {
        return Err(located(path, io::Error::other("folder changed during deletion")));
    }
    let dir = rustix::fs::Dir::read_from(&fd).map_err(|error| located(path, error))?;
    Ok(Frame { dir, fd, path: path.to_path_buf() })
}

/// `parent` holds the folder of `path`; `path` itself may be a file, a link or a folder.
pub(super) fn remove(parent: &File, path: &Path, checkpoint: &dyn Fn() -> io::Result<()>) -> io::Result<()> {
    let parent = parent.try_clone().map(OwnedFd::from)?;
    let name = path.file_name().ok_or_else(|| io::Error::other("Deletion path has no file name"))?;
    checkpoint()?;
    let stat = rustix::fs::statat(&parent, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| located(path, error))?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory {
        return rustix::fs::unlinkat(&parent, name, AtFlags::empty()).map_err(|error| located(path, error));
    }
    let device = stat.st_dev;
    let mut stack = vec![enter(&parent, name, path, device, stat.st_ino)?];
    while let Some(top) = stack.last_mut() {
        let entry = match top.dir.read() {
            Some(Ok(entry)) => entry,
            Some(Err(error)) => return Err(located(&top.path, error)),
            None => {
                // The folder is empty: remove it through its own parent.
                let done = stack.pop().expect("a frame");
                drop(done.dir);
                let name = done.path.file_name().expect("a named folder").to_os_string();
                let holder = stack.last().map_or(&parent, |frame| &frame.fd);
                checkpoint()?;
                rustix::fs::unlinkat(holder, name.as_os_str(), AtFlags::REMOVEDIR)
                    .map_err(|error| located(&done.path, error))?;
                continue;
            }
        };
        let name = OsString::from(OsStr::from_bytes(entry.file_name().to_bytes()));
        if name == "." || name == ".." {
            continue;
        }
        checkpoint()?;
        let path = top.path.join(&name);
        let stat = match rustix::fs::statat(&top.fd, name.as_os_str(), AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) => stat,
            Err(Errno::NOENT) => continue,
            Err(error) => return Err(located(&path, error)),
        };
        if FileType::from_raw_mode(stat.st_mode) != FileType::Directory {
            match rustix::fs::unlinkat(&top.fd, name.as_os_str(), AtFlags::empty()) {
                Ok(()) | Err(Errno::NOENT) => continue,
                Err(error) => return Err(located(&path, error)),
            }
        }
        if stat.st_dev != device {
            return Err(located(&path, io::Error::other("another file system is mounted here; it was not entered")));
        }
        let frame = enter(&top.fd, &name, &path, device, stat.st_ino)?;
        stack.push(frame);
    }
    Ok(())
}

#[cfg(test)]
#[path = "linux_file_tree/tests/remove.rs"]
mod tests;
