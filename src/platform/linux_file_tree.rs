//! Permanent deletion of a tree through descriptors: links are removed, never
//! followed; a folder goes after its content, through its held parent; another file
//! system or a bind mount, inside or selected, is never entered. Pause and cancellation
//! are checked before each item. A folder found not empty after a pass that removed
//! something is listed again: FUSE and network listings can skip entries that go.
use super::linux_mount_place::{status, status_of, Place};
use rustix::{
    fs::{AtFlags, FileType, Mode, OFlags},
    io::Errno,
};
use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io,
    os::{fd::{AsFd, BorrowedFd, OwnedFd}, unix::ffi::OsStrExt},
    path::{Path, PathBuf},
    sync::{atomic::{AtomicBool, Ordering}, mpsc, Mutex},
};

/// `BTRFS_SUPER_MAGIC`: its tree locks serialise concurrent removals.
const BTRFS: i64 = 0x9123_683e;

type Checkpoint<'a> = &'a (dyn Fn() -> io::Result<()> + Sync);

/// A folder being emptied, through a single descriptor: the open file limit bounds the
/// depth reached.
struct Frame {
    dir: rustix::fs::Dir,
    path: PathBuf,
    /// Something went since the listing last started.
    removed: bool,
}

impl Frame {
    fn fd(&self) -> io::Result<BorrowedFd<'_>> {
        self.dir.fd().map_err(|error| located(&self.path, error))
    }
}

enum Step {
    Removed,
    Skipped,
    Enter(Frame),
    End,
}

/// Measured: on Btrfs, removing several folders at once is about 10 % slower than one
/// after the other; on ext4 it is 2.5 to 3 times faster.
pub(super) fn parallel(path: &Path) -> bool {
    let folder = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(path);
    rustix::fs::statfs(folder).map_or(true, |stat| stat.f_type != BTRFS)
}

fn located(path: &Path, error: impl Into<io::Error>) -> io::Error {
    let error = error.into();
    io::Error::new(error.kind(), format!("{}: {error}", path.display()))
}

fn not_entered(path: &Path) -> io::Error {
    located(path, io::Error::other("another file system is mounted here; it was not entered"))
}

/// Opens a child folder that `status` described, refusing a replacement or another mount.
fn enter(parent: BorrowedFd, name: &OsStr, path: &Path, place: Place, inode: u64) -> io::Result<Frame> {
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let fd = rustix::fs::openat(parent, name, flags, Mode::empty()).map_err(|error| located(path, error))?;
    let opened = status_of(&fd).map_err(|error| located(path, error))?;
    if (opened.place, opened.inode) != (place, inode) {
        return Err(located(path, io::Error::other("folder changed during deletion")));
    }
    let dir = rustix::fs::Dir::new(fd).map_err(|error| located(path, error))?;
    Ok(Frame { dir, path: path.to_path_buf(), removed: false })
}

/// Next child of `folder`: non-directories are removed in place, an ordinary subfolder
/// is opened and returned.
fn next_child(folder: &mut Frame, boundary: Place, checkpoint: Checkpoint) -> io::Result<Step> {
    let entry = match folder.dir.read() {
        Some(Ok(entry)) => entry,
        Some(Err(error)) => return Err(located(&folder.path, error)),
        None => return Ok(Step::End),
    };
    let name = OsString::from(OsStr::from_bytes(entry.file_name().to_bytes()));
    if name == "." || name == ".." {
        return Ok(Step::Skipped);
    }
    checkpoint()?;
    let path = folder.path.join(&name);
    let fd = folder.fd()?;
    let item = match status(&fd, name.as_os_str()) {
        Ok(item) => item,
        Err(Errno::NOENT) => return Ok(Step::Skipped),
        Err(error) => return Err(located(&path, error)),
    };
    if item.kind != FileType::Directory {
        return match rustix::fs::unlinkat(fd, name.as_os_str(), AtFlags::empty()) {
            Ok(()) => Ok(Step::Removed),
            Err(Errno::NOENT) => Ok(Step::Skipped),
            Err(error) => Err(located(&path, error)),
        };
    }
    if !boundary.holds(item.place) {
        return Err(not_entered(&path));
    }
    enter(fd, &name, &path, item.place, item.inode).map(Step::Enter)
}

/// Removes `root` and everything under it; `holder` is the held parent of `root`.
fn drain(root: Frame, holder: BorrowedFd, boundary: Place, checkpoint: Checkpoint) -> io::Result<()> {
    let mut stack = vec![root];
    while let Some(top) = stack.last_mut() {
        match next_child(top, boundary, checkpoint)? {
            Step::Enter(child) => stack.push(child),
            Step::Removed => top.removed = true,
            Step::Skipped => {}
            Step::End => {
                // The folder looks empty: remove it through its own parent.
                let mut done = stack.pop().expect("a frame");
                let name = done.path.file_name().expect("a named folder").to_os_string();
                checkpoint()?;
                let parent = match stack.last() {
                    Some(frame) => frame.fd()?,
                    None => holder,
                };
                match rustix::fs::unlinkat(parent, name.as_os_str(), AtFlags::REMOVEDIR) {
                    Err(Errno::NOTEMPTY) if done.removed => {
                        done.removed = false;
                        done.dir.rewind();
                        stack.push(done);
                    }
                    Err(error) => return Err(located(&done.path, error)),
                    Ok(()) => {
                        if let Some(parent) = stack.last_mut() {
                            parent.removed = true;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// `parent` holds the folder of `path`; `path` itself may be a file, a link or a folder.
/// With several workers, the subfolders of a selected folder are removed in parallel.
pub(super) fn remove(parent: &File, path: &Path, workers: usize, checkpoint: Checkpoint) -> io::Result<()> {
    let parent = parent.as_fd();
    let name = path.file_name().ok_or_else(|| io::Error::other("Deletion path has no file name"))?;
    checkpoint()?;
    let item = status(&parent, name).map_err(|error| located(path, error))?;
    if item.kind != FileType::Directory {
        return rustix::fs::unlinkat(parent, name, AtFlags::empty()).map_err(|error| located(path, error));
    }
    // A selected mount point would be emptied before its own removal fails.
    let holder = status_of(&parent).map_err(|error| located(path, error))?;
    if item.place.mounted_on(holder.place) {
        return Err(not_entered(path));
    }
    let root = enter(parent, name, path, item.place, item.inode)?;
    if workers <= 1 {
        return drain(root, parent, item.place, checkpoint);
    }
    remove_parallel(root, parent, item.place, workers, checkpoint)
}

/// The calling thread lists the selected folder, removing its files and opening each
/// subfolder; a bounded pool removes those subfolders. The selected folder goes last.
fn remove_parallel(mut root: Frame, holder: BorrowedFd, boundary: Place, workers: usize, checkpoint: Checkpoint) -> io::Result<()> {
    // The workers remove their subfolders through a copy: the listing moves the original.
    let shared: OwnedFd = root.fd()?.try_clone_to_owned().map_err(|error| located(&root.path, error))?;
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
                if let Err(error) = drain(frame, shared.as_fd(), boundary, &guarded) {
                    stop.store(true, Ordering::Relaxed);
                    failure.lock().unwrap_or_else(|p| p.into_inner()).get_or_insert(error);
                }
            });
        }
        let result = (|| -> io::Result<()> {
            loop {
                match next_child(&mut root, boundary, &guarded)? {
                    Step::Enter(child) => sender.send(child).map_err(|_| io::Error::other("Deletion workers stopped"))?,
                    Step::Removed | Step::Skipped => {}
                    Step::End => return Ok(()),
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
    drop(shared);
    // Its listing is over: the selected folder goes, or is listed again if it is not empty.
    root.removed = true;
    drain(root, holder, boundary, checkpoint)
}

#[cfg(test)]
#[path = "linux_file_tree/tests/remove.rs"]
mod tests;
