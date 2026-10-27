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
    sync::{atomic::{AtomicBool, Ordering}, mpsc, Mutex},
};

/// `BTRFS_SUPER_MAGIC`: its tree locks serialise concurrent removals.
const BTRFS: i64 = 0x9123_683e;

type Checkpoint<'a> = &'a (dyn Fn() -> io::Result<()> + Sync);

struct Frame {
    dir: rustix::fs::Dir,
    fd: OwnedFd,
    path: PathBuf,
}

/// Measured: on Btrfs, removing several folders at once is about 10 % slower than one
/// after the other; on ext4 it is 2.5 to 3 times faster.
pub(super) fn parallel(path: &Path) -> bool {
    let folder = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(path);
    rustix::fs::statfs(folder).map_or(true, |stat| stat.f_type as i64 != BTRFS)
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

/// Next child of the folder read by `dir` and held by `fd`: non-directories are
/// removed in place, an ordinary subfolder is opened and returned. `None` once the
/// folder is empty.
fn next_child(dir: &mut rustix::fs::Dir, fd: &OwnedFd, folder: &Path, device: u64, checkpoint: Checkpoint) -> io::Result<Option<Option<Frame>>> {
    let entry = match dir.read() {
        Some(Ok(entry)) => entry,
        Some(Err(error)) => return Err(located(folder, error)),
        None => return Ok(None),
    };
    let name = OsString::from(OsStr::from_bytes(entry.file_name().to_bytes()));
    if name == "." || name == ".." {
        return Ok(Some(None));
    }
    checkpoint()?;
    let path = folder.join(&name);
    let stat = match rustix::fs::statat(fd, name.as_os_str(), AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => stat,
        Err(Errno::NOENT) => return Ok(Some(None)),
        Err(error) => return Err(located(&path, error)),
    };
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory {
        return match rustix::fs::unlinkat(fd, name.as_os_str(), AtFlags::empty()) {
            Ok(()) | Err(Errno::NOENT) => Ok(Some(None)),
            Err(error) => Err(located(&path, error)),
        };
    }
    if stat.st_dev != device {
        return Err(located(&path, io::Error::other("another file system is mounted here; it was not entered")));
    }
    enter(fd, &name, &path, device, stat.st_ino).map(|frame| Some(Some(frame)))
}

/// Removes `root` and everything under it; `holder` is the held parent of `root`.
fn drain(root: Frame, holder: &OwnedFd, device: u64, checkpoint: Checkpoint) -> io::Result<()> {
    let mut stack = vec![root];
    while let Some(top) = stack.last_mut() {
        match next_child(&mut top.dir, &top.fd, &top.path, device, checkpoint)? {
            Some(Some(child)) => stack.push(child),
            Some(None) => {}
            None => {
                // The folder is empty: remove it through its own parent.
                let done = stack.pop().expect("a frame");
                drop(done.dir);
                let name = done.path.file_name().expect("a named folder").to_os_string();
                let parent = stack.last().map_or(holder, |frame| &frame.fd);
                checkpoint()?;
                rustix::fs::unlinkat(parent, name.as_os_str(), AtFlags::REMOVEDIR)
                    .map_err(|error| located(&done.path, error))?;
            }
        }
    }
    Ok(())
}

/// `parent` holds the folder of `path`; `path` itself may be a file, a link or a folder.
/// With several workers, the subfolders of a selected folder are removed in parallel.
pub(super) fn remove(parent: &File, path: &Path, workers: usize, checkpoint: Checkpoint) -> io::Result<()> {
    let parent = parent.try_clone().map(OwnedFd::from)?;
    let name = path.file_name().ok_or_else(|| io::Error::other("Deletion path has no file name"))?;
    checkpoint()?;
    let stat = rustix::fs::statat(&parent, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| located(path, error))?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory {
        return rustix::fs::unlinkat(&parent, name, AtFlags::empty()).map_err(|error| located(path, error));
    }
    let root = enter(&parent, name, path, stat.st_dev, stat.st_ino)?;
    if workers <= 1 {
        return drain(root, &parent, stat.st_dev, checkpoint);
    }
    remove_parallel(root, &parent, stat.st_dev, workers, checkpoint)
}

/// The calling thread lists the selected folder, removing its files and opening each
/// subfolder; a bounded pool removes those subfolders. The selected folder goes last.
fn remove_parallel(root: Frame, holder: &OwnedFd, device: u64, workers: usize, checkpoint: Checkpoint) -> io::Result<()> {
    let Frame { mut dir, fd, path } = root;
    let stop = AtomicBool::new(false);
    let failure = Mutex::new(None::<io::Error>);
    // Workers stop at their next checkpoint after any failure or cancellation.
    let guarded = || {
        if stop.load(Ordering::Relaxed) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "Deletion stopped"));
        }
        checkpoint()
    };
    let (sender, receiver) = mpsc::sync_channel::<Frame>(workers);
    let receiver = Mutex::new(receiver);
    let result = std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let job = receiver.lock().unwrap_or_else(|p| p.into_inner()).recv();
                let Ok(frame) = job else { break };
                if stop.load(Ordering::Relaxed) {
                    continue; // Release the queued folders undeleted.
                }
                if let Err(error) = drain(frame, &fd, device, &guarded) {
                    stop.store(true, Ordering::Relaxed);
                    failure.lock().unwrap_or_else(|p| p.into_inner()).get_or_insert(error);
                }
            });
        }
        let result = (|| -> io::Result<()> {
            loop {
                match next_child(&mut dir, &fd, &path, device, &guarded)? {
                    Some(Some(child)) => sender.send(child).map_err(|_| io::Error::other("Deletion workers stopped"))?,
                    Some(None) => {}
                    None => return Ok(()),
                }
            }
        })();
        if result.is_err() {
            stop.store(true, Ordering::Relaxed);
        }
        drop(sender);
        result
    });
    // Every worker has finished: report its first failure before the caller's.
    if let Some(error) = failure.into_inner().unwrap_or_else(|p| p.into_inner()) {
        if !matches!(&result, Err(own) if own.kind() != io::ErrorKind::Interrupted) {
            return Err(error);
        }
    }
    result?;
    drop((dir, fd));
    let name = path.file_name().expect("a named folder").to_os_string();
    checkpoint()?;
    rustix::fs::unlinkat(holder, name.as_os_str(), AtFlags::REMOVEDIR).map_err(|error| located(&path, error))
}

#[cfg(test)]
#[path = "linux_file_tree/tests/remove.rs"]
mod tests;
