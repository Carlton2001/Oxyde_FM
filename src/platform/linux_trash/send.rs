//! Moving an item to the trash, as the freedesktop specification describes: a record in
//! `info`, then the item renamed into `files` under the same unique name. The item is
//! renamed through its held parent, after its identity is checked when one is expected,
//! and the new entry's content path is returned, so that Undo restores this entry and no
//! other item that later came from the same place.
use super::super::linux_mount_place::{status, status_of, Place};
use crate::models::transaction::HistoryFileIdentity;
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use rustix::fs::{AtFlags, FileType, Mode, OFlags, RenameFlags};
use rustix::io::Errno;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{self, Write};
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// Characters a `.trashinfo` path keeps unescaped, as the trash crate writes them.
const KEPT: &AsciiSet = &NON_ALPHANUMERIC.remove(b'/').remove(b'-').remove(b'.').remove(b'_').remove(b'~');
/// Names tried before giving up on a crowded trash.
const NAMES: usize = 10_000;

/// A trashed item: where its content now lies, and the object it was.
pub(in crate::platform) struct Sent {
    pub content: PathBuf,
    pub identity: HistoryFileIdentity,
}

#[derive(Debug)]
pub(in crate::platform) enum SendError {
    /// Another object took the expected item's place.
    Changed,
    /// The trash cannot take the item; `0` names the cause.
    Refused(io::Error),
}

impl From<io::Error> for SendError {
    fn from(error: io::Error) -> Self {
        Self::Refused(error)
    }
}

impl From<Errno> for SendError {
    fn from(error: Errno) -> Self {
        Self::Refused(error.into())
    }
}

fn located(path: &Path, error: impl Into<io::Error>) -> io::Error {
    let error = error.into();
    io::Error::new(error.kind(), format!("{}: {error}", path.display()))
}

fn directory(path: &Path) -> io::Result<OwnedFd> {
    rustix::fs::open(path, OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC, Mode::empty())
        .map_err(|error| located(path, error))
}

/// `name` in `parent`, created private when missing; never a link.
fn subdirectory(parent: &OwnedFd, name: &OsStr, path: &Path) -> io::Result<OwnedFd> {
    match rustix::fs::mkdirat(parent, name, Mode::from_raw_mode(0o700)) {
        Ok(()) | Err(Errno::EXIST) => {}
        Err(error) => return Err(located(path, error)),
    }
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    rustix::fs::openat(parent, name, flags, Mode::empty()).map_err(|error| located(path, error))
}

/// `$XDG_DATA_HOME/Trash`, as the trash crate and GIO name it.
fn home_trash() -> Option<PathBuf> {
    dirs::data_dir().map(|data| data.join("Trash"))
}

/// The folder at the root of the mount holding `folder`.
fn mount_root(folder: &Path, place: Place) -> io::Result<PathBuf> {
    let mut root = folder.to_path_buf();
    while let Some(parent) = root.parent() {
        let above = status(directory(parent)?, ".").map_err(|error| located(parent, error))?;
        if place.mounted_on(above.place) {
            break;
        }
        root = parent.to_path_buf();
    }
    Ok(root)
}

/// The trash of the mount at `root`: `.Trash/$uid` when the administrator prepared a
/// sticky `.Trash`, otherwise `.Trash-$uid`, created when missing.
fn mount_trash(root: &Path) -> io::Result<(PathBuf, OwnedFd)> {
    let uid = rustix::process::getuid().as_raw().to_string();
    let top = directory(root)?;
    if let Ok(shared) = status(&top, ".Trash") {
        let sticky = shared.mode & 0o1000 != 0;
        if shared.kind == FileType::Directory && sticky {
            let path = root.join(".Trash");
            let shared = subdirectory(&top, OsStr::new(".Trash"), &path)?;
            let own = path.join(&uid);
            if let Ok(trash) = subdirectory(&shared, OsStr::new(&uid), &own) {
                return Ok((own, trash));
            }
        }
    }
    let name = OsString::from(format!(".Trash-{uid}"));
    let path = root.join(&name);
    let trash = subdirectory(&top, &name, &path)?;
    Ok((path, trash))
}

/// The trash for an item at `place` inside `folder`: the home trash on its own mount,
/// otherwise the trash of the item's mount.
fn trash_for(folder: &Path, place: Place) -> io::Result<(PathBuf, OwnedFd)> {
    if let Some(home) = home_trash() {
        if let Some(data) = home.parent() {
            std::fs::create_dir_all(data).map_err(|error| located(data, error))?;
            let trash = subdirectory(&directory(data)?, OsStr::new("Trash"), &home)?;
            let home_place = status_of(&trash)?.place;
            if home_place.holds(place) && place.holds(home_place) {
                return Ok((home, trash));
            }
        }
    }
    mount_trash(&mount_root(folder, place)?)
}

/// The record of an item deleted from `original`, now.
fn record(original: &Path) -> String {
    let path: String = utf8_percent_encode(&String::from_utf8_lossy(original.as_os_str().as_bytes()), KEPT).collect();
    format!("[Trash Info]\nPath={path}\nDeletionDate={}\n", chrono::Local::now().format("%Y-%m-%dT%H:%M:%S"))
}

/// `name`, then `name.2`, `name.3`… as the trash crate numbers its entries.
fn candidate(name: &OsStr, index: usize) -> OsString {
    let mut candidate = name.to_os_string();
    if index > 1 {
        candidate.push(format!(".{index}"));
    }
    candidate
}

/// Renames without replacing; file systems without `RENAME_NOREPLACE` take an empty
/// placeholder that the rename replaces, as the trash crate does.
fn rename_new(parent: &OwnedFd, name: &OsStr, files: &OwnedFd, target: &OsStr, directory: bool) -> rustix::io::Result<()> {
    match rustix::fs::renameat_with(parent, name, files, target, RenameFlags::NOREPLACE) {
        Err(Errno::INVAL | Errno::NOSYS) => {}
        result => return result,
    }
    if directory {
        rustix::fs::mkdirat(files, target, Mode::from_raw_mode(0o700))?;
    } else {
        let flags = OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        drop(rustix::fs::openat(files, target, flags, Mode::from_raw_mode(0o600))?);
    }
    rustix::fs::renameat(parent, name, files, target).inspect_err(|_| {
        let flags = if directory { AtFlags::REMOVEDIR } else { AtFlags::empty() };
        let _ = rustix::fs::unlinkat(files, target, flags);
    })
}

/// Moves `path` to its trash. With `expected`, refuses another object found there.
pub(in crate::platform) fn send(path: &Path, expected: Option<HistoryFileIdentity>) -> Result<Sent, SendError> {
    let (Some(folder), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(io::Error::other(format!("{}: not an item that can be trashed", path.display())).into());
    };
    let folder = std::fs::canonicalize(folder).map_err(|error| located(folder, error))?;
    let original = folder.join(name);
    if super::inside_trash(&original) {
        return Err(io::Error::other(format!("{}: already in the trash", original.display())).into());
    }
    let parent = directory(&folder)?;
    let item = rustix::fs::openat(&parent, name, OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC, Mode::empty())
        .map_err(|error| located(&original, error))?;
    let held = status_of(&item)?;
    let identity = HistoryFileIdentity::from_file(&File::from(item.try_clone()?))?;
    if expected.is_some_and(|expected| expected != identity) {
        return Err(SendError::Changed);
    }
    let (trash, trash_fd) = trash_for(&folder, held.place)?;
    let files = subdirectory(&trash_fd, OsStr::new("files"), &trash.join("files"))?;
    let info = subdirectory(&trash_fd, OsStr::new("info"), &trash.join("info"))?;
    let content = record(&original);
    for index in 1..=NAMES {
        let entry = candidate(name, index);
        let mut record_name = entry.clone();
        record_name.push(".trashinfo");
        let flags = OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let record = match rustix::fs::openat(&info, &record_name, flags, Mode::from_raw_mode(0o600)) {
            Ok(record) => record,
            Err(Errno::EXIST) => continue,
            Err(error) => return Err(located(&trash.join("info").join(&record_name), error).into()),
        };
        let forget = || { let _ = rustix::fs::unlinkat(&info, &record_name, AtFlags::empty()); };
        if let Err(error) = File::from(record).write_all(content.as_bytes()) {
            forget();
            return Err(located(&trash.join("info").join(&record_name), error).into());
        }
        // The residual race of a path-based rename: the object is checked once more.
        let current = rustix::fs::statat(&parent, name, AtFlags::SYMLINK_NOFOLLOW);
        if current.is_err_and(|error| error != Errno::NOENT) || current.as_ref().is_ok_and(|stat| stat.st_ino != held.inode) {
            forget();
            return Err(SendError::Changed);
        }
        match rename_new(&parent, name, &files, &entry, held.kind == FileType::Directory) {
            Ok(()) => return Ok(Sent { content: trash.join("files").join(&entry), identity }),
            Err(Errno::EXIST | Errno::NOTEMPTY) => forget(),
            Err(error) => {
                forget();
                return Err(located(&original, error).into());
            }
        }
    }
    Err(io::Error::other(format!("{}: the trash has no free name for this item", original.display())).into())
}

impl SendError {
    /// As the trash crate reported it: access refusals name the item.
    pub(in crate::platform) fn message(&self, path: &Path) -> String {
        match self {
            Self::Changed => crate::models::error::HistoryDeleteFailure { changed: true, path: path.to_string_lossy().into_owned() }.to_string(),
            Self::Refused(error) if matches!(error.kind(), io::ErrorKind::PermissionDenied) => {
                crate::models::error::DeleteAccessDenied { path: path.to_string_lossy().into_owned() }.to_string()
            }
            Self::Refused(error) => error.to_string(),
        }
    }
}

#[cfg(test)]
#[path = "tests/send.rs"]
mod tests;
