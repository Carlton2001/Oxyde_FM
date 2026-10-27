//! Symbolic links transferred as links: always named relative to a held parent
//! and never followed.
use rustix::{
    fs::{AtFlags, RenameFlags},
    io::Errno,
};
use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io,
    os::unix::ffi::OsStringExt,
    path::{Path, PathBuf},
};

fn leaf(path: &Path) -> io::Result<&OsStr> {
    path.file_name()
        .ok_or_else(|| io::Error::other("Link has no file name"))
}

fn staging_name() -> String {
    format!(".oxyde-link-{}", uuid::Uuid::new_v4())
}

/// Reads the target of the captured link itself: an `O_PATH` descriptor names
/// that object, so a link installed later under the same name is never read.
pub(super) fn target(link: &File) -> io::Result<PathBuf> {
    let target = rustix::fs::readlinkat(link, "", Vec::new())?;
    Ok(PathBuf::from(OsString::from_vec(target.into_bytes())))
}

/// Opens the link itself in its held parent, never its target.
fn open(parent: &File, name: &OsStr) -> io::Result<File> {
    super::linux_source_selection::open(parent, Path::new(name), None)
}

/// Renames within one held parent without replacing anything. FUSE file systems
/// such as ntfs-3g refuse the flag; the fallback then checks the name first.
fn rename_exclusive(parent: &File, from: &OsStr, to: &OsStr) -> io::Result<()> {
    match rustix::fs::renameat_with(parent, from, parent, to, RenameFlags::NOREPLACE) {
        Err(Errno::INVAL) => {}
        result => return result.map_err(io::Error::from),
    }
    match rustix::fs::statat(parent, to, AtFlags::SYMLINK_NOFOLLOW) {
        Err(Errno::NOENT) => {}
        Ok(_) => return Err(io::ErrorKind::AlreadyExists.into()),
        Err(error) => return Err(error.into()),
    }
    rustix::fs::renameat(parent, from, parent, to).map_err(io::Error::from)
}

/// Creates the link exclusively, or swaps it in over an existing non-directory
/// destination in one rename. Returns the created link itself.
pub(super) fn create(parent: &File, path: &Path, target: &Path, replace: bool) -> io::Result<File> {
    let name = leaf(path)?;
    if replace {
        let staged = staging_name();
        rustix::fs::symlinkat(target, parent, &staged)?;
        if let Err(error) = rustix::fs::renameat(parent, &staged, parent, name) {
            let _ = rustix::fs::unlinkat(parent, &staged, AtFlags::empty());
            return Err(error.into());
        }
    } else {
        rustix::fs::symlinkat(target, parent, name)?;
    }
    open(parent, name)
}

/// Removes a link only after `verify` accepts the object set aside under a
/// private name; a rejected object is put back under its own name.
pub(super) fn remove(
    parent: &File,
    path: &Path,
    verify: &mut dyn FnMut(&File) -> io::Result<()>,
) -> io::Result<()> {
    let name = leaf(path)?;
    let staged = staging_name();
    rename_exclusive(parent, name, OsStr::new(&staged))?;
    let checked = open(parent, OsStr::new(&staged)).and_then(|link| {
        if !link.metadata()?.file_type().is_symlink() {
            return Err(io::Error::other(format!("Not a symbolic link: {}", path.display())));
        }
        verify(&link)
    });
    if let Err(error) = checked {
        if let Err(restore) = rename_exclusive(parent, OsStr::new(&staged), name) {
            return Err(io::Error::new(
                error.kind(),
                format!("{error}; link retained as {staged}: {restore}"),
            ));
        }
        return Err(error);
    }
    rustix::fs::unlinkat(parent, &staged, AtFlags::empty()).map_err(io::Error::from)
}
