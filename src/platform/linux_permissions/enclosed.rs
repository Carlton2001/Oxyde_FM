//! Permissions of everything inside a folder, at any depth. A folder changes after
//! its content, so that one losing its search permission is still walked. Links are
//! never followed, other file systems never entered, and each item is changed
//! through the descriptor whose type was checked.
use super::chmod;
use crate::models::permissions::Levels;
use rustix::{fs::{FileType, Mode, OFlags}, io::Errno};
use std::{
    ffi::OsStr,
    fmt::Display,
    os::{fd::OwnedFd, unix::ffi::OsStrExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
pub(super) struct Outcome {
    pub(super) failed: u64,
    pub(super) first: Option<String>,
}

impl Outcome {
    fn fail(&mut self, path: &Path, error: impl Display) {
        self.failed += 1;
        self.first.get_or_insert_with(|| format!("{}: {error}", path.display()));
    }
}

struct Frame {
    dir: rustix::fs::Dir,
    fd: OwnedFd,
    path: PathBuf,
    /// The folder itself, changed once its content is done.
    own: Option<(OwnedFd, u32)>,
}

fn frame(fd: OwnedFd, path: PathBuf, own: Option<(OwnedFd, u32)>) -> rustix::io::Result<Frame> {
    Ok(Frame { dir: rustix::fs::Dir::read_from(&fd)?, fd, path, own })
}

fn open_directory(item: &OwnedFd) -> rustix::io::Result<OwnedFd> {
    rustix::fs::openat(item, ".", OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC, Mode::empty())
}

/// Sets `mode` unless the item has it, and checks the file system kept it.
fn set(item: &OwnedFd, current: u32, mode: u32, path: &Path, outcome: &mut Outcome) {
    if current == mode {
        return;
    }
    match chmod(item, mode).and_then(|()| rustix::fs::fstat(item)) {
        Ok(stat) if stat.st_mode & 0o777 == mode & 0o777 => {}
        Ok(_) => outcome.fail(path, "permissions not kept by the file system"),
        Err(error) => outcome.fail(path, std::io::Error::from(error)),
    }
}

/// `root` is the folder, already checked by the caller, on `device`.
pub(super) fn apply(
    root: &OwnedFd,
    root_path: &Path,
    device: u64,
    change: &Levels,
    cancel: &AtomicBool,
) -> std::io::Result<Outcome> {
    let mut outcome = Outcome::default();
    let mut stack = vec![frame(open_directory(root)?, root_path.to_path_buf(), None)?];
    while let Some(top) = stack.last_mut() {
        if cancel.load(Ordering::Relaxed) {
            return Err(std::io::ErrorKind::Interrupted.into());
        }
        let entry = match top.dir.read() {
            Some(Ok(entry)) => entry,
            end => {
                if let Some(Err(error)) = end {
                    outcome.fail(&top.path, std::io::Error::from(error));
                }
                let done = stack.pop().unwrap();
                if let Some((item, mode)) = done.own {
                    let current = rustix::fs::fstat(&item).map(|stat| stat.st_mode & 0o7777);
                    match current {
                        Ok(current) => set(&item, current, mode, &done.path, &mut outcome),
                        Err(error) => outcome.fail(&done.path, std::io::Error::from(error)),
                    }
                }
                continue;
            }
        };
        let name = entry.file_name();
        if matches!(name.to_bytes(), b"." | b"..") {
            continue;
        }
        let path = top.path.join(OsStr::from_bytes(name.to_bytes()));
        let opened = rustix::fs::openat(&top.fd, name, OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC, Mode::empty())
            .and_then(|item| rustix::fs::fstat(&item).map(|stat| (item, stat)));
        let (item, stat) = match opened {
            Ok(opened) => opened,
            // Removed since the listing.
            Err(Errno::NOENT) => continue,
            Err(error) => {
                outcome.fail(&path, std::io::Error::from(error));
                continue;
            }
        };
        if stat.st_dev != device {
            continue;
        }
        let current = stat.st_mode & 0o7777;
        match FileType::from_raw_mode(stat.st_mode) {
            FileType::RegularFile => set(&item, current, change.mode(current, false), &path, &mut outcome),
            FileType::Directory => {
                let mode = change.mode(current, true);
                let directory = match open_directory(&item) {
                    // A folder the change opens to its owner is changed first, then walked.
                    Err(Errno::ACCESS) => {
                        set(&item, current, mode, &path, &mut outcome);
                        open_directory(&item)
                    }
                    opened => opened,
                };
                match directory.and_then(|directory| frame(directory, path.clone(), Some((item, mode)))) {
                    Ok(frame) => stack.push(frame),
                    Err(error) => outcome.fail(&path, std::io::Error::from(error)),
                }
            }
            // Links, devices, pipes and sockets keep their permissions, as in Nautilus.
            _ => {}
        }
    }
    Ok(outcome)
}
