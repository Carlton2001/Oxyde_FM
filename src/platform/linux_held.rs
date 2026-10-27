//! Effects relative to held folders, and renaming or removal of a held object wherever
//! it now is. Linux pins no name: an effect by name checks, just before and after, that
//! it reached the held object, and puts anything else back.
use rustix::{
    fs::{AtFlags, Mode, OFlags, RenameFlags},
    io::Errno,
};
use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io,
    os::{fd::AsRawFd, unix::fs::MetadataExt},
    path::Path,
};

type Identity = (u64, u64);

pub(super) fn leaf(path: &Path) -> io::Result<&OsStr> {
    path.file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("{} has no file name", path.display())))
}

/// The identity of a held object. FUSE (ntfs-3g) no longer describes a descriptor
/// whose name was unlinked while another hard link keeps the object; the kernel still
/// reports its inode and mount.
fn identity(file: &File) -> io::Result<Identity> {
    match file.metadata() {
        Ok(metadata) => Ok((metadata.dev(), metadata.ino())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => kernel_identity(file).ok_or(error),
        Err(error) => Err(error),
    }
}

fn kernel_identity(file: &File) -> Option<Identity> {
    let info = std::fs::read_to_string(format!("/proc/self/fdinfo/{}", file.as_raw_fd())).ok()?;
    let field = |key: &str| info.lines().find_map(|line| line.strip_prefix(key)).map(str::trim);
    let (mount, ino) = (field("mnt_id:")?, field("ino:")?.parse().ok()?);
    let mounts = std::fs::read_to_string("/proc/self/mountinfo").ok()?;
    let device = mounts.lines().find_map(|line| {
        let mut fields = line.split(' ');
        (fields.next()? == mount).then(|| fields.nth(1))?
    })?;
    let (major, minor) = device.split_once(':')?;
    Some((rustix::fs::makedev(major.parse().ok()?, minor.parse().ok()?), ino))
}

fn entry(parent: &File, name: &OsStr) -> io::Result<Identity> {
    let stat = rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)?;
    Ok((stat.st_dev, stat.st_ino))
}

fn changed(name: &OsStr) -> io::Error {
    io::Error::other(format!("{} was replaced during the operation; the replacement was kept", name.to_string_lossy()))
}

/// A folder by its name in `parent`, readable when permitted, else for path use only.
pub(super) fn open_directory(parent: &File, name: &OsStr) -> io::Result<File> {
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    match rustix::fs::openat(parent, name, flags, Mode::empty()) {
        Err(Errno::ACCESS) => rustix::fs::openat(parent, name, flags | OFlags::PATH, Mode::empty()),
        result => result,
    }
    .map(File::from)
    .map_err(io::Error::from)
}

/// Creates the folder unless it exists; `true` when this call created it.
pub(super) fn ensure_directory(parent: &File, path: &Path) -> io::Result<(File, bool)> {
    let name = leaf(path)?;
    let created = match rustix::fs::mkdirat(parent, name, Mode::from_raw_mode(0o777)) {
        Ok(()) => true,
        Err(Errno::EXIST) => false,
        Err(error) => return Err(error.into()),
    };
    Ok((open_directory(parent, name)?, created))
}

pub(super) fn create_directory(parent: &File, path: &Path) -> io::Result<File> {
    let name = leaf(path)?;
    rustix::fs::mkdirat(parent, name, Mode::from_raw_mode(0o777))?;
    open_directory(parent, name)
}

/// An ordinary file by its name in `parent`; `read` asks for its contents, otherwise
/// the descriptor only names the object.
pub(super) fn open_file(parent: &File, path: &Path, read: bool) -> io::Result<File> {
    let name = leaf(path)?;
    // Checked first so that a FIFO is never opened for reading, which would block.
    let stat = rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)?;
    if rustix::fs::FileType::from_raw_mode(stat.st_mode) != rustix::fs::FileType::RegularFile {
        return Err(io::Error::other("Operation source is not an ordinary file"));
    }
    let access = if read { OFlags::RDONLY } else { OFlags::PATH };
    let file = File::from(rustix::fs::openat(parent, name, access | OFlags::NOFOLLOW | OFlags::CLOEXEC, Mode::empty())?);
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("Operation source is not an ordinary file"));
    }
    Ok(file)
}

/// Reads a copy without updating the access date it kept, when the owner may ask so.
pub(super) fn open_copy(parent: &File, path: &Path) -> io::Result<File> {
    let name = leaf(path)?;
    let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let file = match rustix::fs::openat(parent, name, flags | OFlags::NOATIME, Mode::empty()) {
        Err(Errno::PERM) => rustix::fs::openat(parent, name, flags, Mode::empty())?,
        result => result?,
    };
    let file = File::from(file);
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("Operation source is not an ordinary file"));
    }
    Ok(file)
}

/// The item itself, a final link included, for renaming.
pub(super) fn open_item(parent: &File, path: &Path) -> io::Result<File> {
    let flags = OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    Ok(File::from(rustix::fs::openat(parent, leaf(path)?, flags, Mode::empty())?))
}

/// Creates a new private file; an existing name, a link included, is refused.
pub(super) fn create_file(parent: &File, path: &Path) -> io::Result<File> {
    let flags = OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    Ok(File::from(rustix::fs::openat(parent, leaf(path)?, flags, Mode::from_raw_mode(0o666))?))
}

/// The folder and name under which `file` is reachable now, checked against its
/// identity. The kernel follows renames of the object and of its folders; `fallback`
/// serves without `/proc`, and when the opened name was unlinked while another hard
/// link keeps the object, as after a publication by link on ntfs-3g.
fn located(file: &File, fallback: &Path) -> io::Result<(File, OsString)> {
    let expected = identity(file)?;
    if file.metadata().is_ok_and(|metadata| metadata.nlink() == 0) {
        return Err(io::Error::new(io::ErrorKind::NotFound, "The held object was already removed"));
    }
    let current = std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd())).ok();
    let mut last = None;
    for path in current.as_deref().into_iter().chain([fallback]) {
        let name = leaf(path)?.to_os_string();
        let parent = path.parent().ok_or_else(|| io::Error::other("The held object has no folder"))?;
        // A link swapped in for a folder since is caught by the identity check.
        let found = super::linux_access::hold_directory(parent, true)
            .and_then(|folder| Ok((entry(&folder, &name)?, folder)));
        match found {
            Ok((found, folder)) if found == expected => return Ok((folder, name)),
            Ok(_) => last = Some(changed(&name)),
            Err(error) => last = Some(error),
        }
    }
    Err(last.unwrap_or_else(|| changed(fallback.as_os_str())))
}

fn rename_noreplace(from_parent: &File, from: &OsStr, parent: &File, to: &OsStr) -> io::Result<()> {
    super::linux_rename::exclusive(
        rustix::fd::AsFd::as_fd(from_parent),
        Path::new(from),
        rustix::fd::AsFd::as_fd(parent),
        Path::new(to),
    )
}

/// Removes `name` from `parent` only if it is `expected`. The entry is renamed aside
/// first, so that what is checked is what is removed; anything else is put back.
fn remove_entry(parent: &File, name: &OsStr, expected: Identity, directory: bool) -> io::Result<()> {
    // Cheap refusals first, which leave the entry untouched: renaming it aside and
    // back would change its change time.
    if entry(parent, name)? != expected {
        return Err(changed(name));
    }
    if directory && has_entries(parent, name)? {
        return Err(Errno::NOTEMPTY.into());
    }
    let flags = if directory { AtFlags::REMOVEDIR } else { AtFlags::empty() };
    if renames_reset_creation(parent) {
        return rustix::fs::unlinkat(parent, name, flags).map_err(io::Error::from);
    }
    let aside = OsString::from(format!(".oxyde-removing-{}", uuid::Uuid::new_v4()));
    rename_noreplace(parent, name, parent, &aside)?;
    let result = match entry(parent, &aside) {
        Ok(found) if found == expected => rustix::fs::unlinkat(parent, &aside, flags).map_err(io::Error::from),
        Ok(_) => Err(changed(name)),
        Err(error) => Err(error),
    };
    if result.is_err() {
        if let Err(error) = rename_noreplace(parent, &aside, parent, name) {
            log::warn!("{} kept as {}: {error}", name.to_string_lossy(), aside.to_string_lossy());
        }
    }
    result
}

/// exFAT gives a folder a new creation time whenever an entry is renamed into it, which
/// would void the folder's history proof: there, the entry checked just before is
/// unlinked directly, leaving a short window for a substitution.
fn renames_reset_creation(parent: &File) -> bool {
    const EXFAT_SUPER_MAGIC: u64 = 0x2011_BAB0;
    rustix::fs::fstatfs(parent).is_ok_and(|stat| stat.f_type as u64 == EXFAT_SUPER_MAGIC)
}

/// Whether the folder lists any entry; a folder that cannot be listed is left to
/// the removal itself to judge.
fn has_entries(parent: &File, name: &OsStr) -> io::Result<bool> {
    let folder = match rustix::fs::openat(parent, name, OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC, Mode::empty()) {
        Err(Errno::ACCESS) => return Ok(false),
        result => result?,
    };
    for item in rustix::fs::Dir::read_from(&folder)? {
        if !matches!(item?.file_name().to_bytes(), b"." | b"..") {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Removes the held object, file or empty folder, wherever it now is.
pub(super) fn remove(file: &File, fallback: &Path) -> io::Result<()> {
    let expected = identity(file)?;
    let (folder, name) = located(file, fallback)?;
    let mode = rustix::fs::statat(&folder, &name, AtFlags::SYMLINK_NOFOLLOW)?.st_mode;
    let directory = rustix::fs::FileType::from_raw_mode(mode) == rustix::fs::FileType::Directory;
    remove_entry(&folder, &name, expected, directory)
}

/// Removes the empty folder `path` of `parent` if it is still `expected`.
pub(super) fn remove_directory(parent: &File, path: &Path, expected: &File) -> io::Result<()> {
    remove_entry(parent, leaf(path)?, identity(expected)?, true)
}

/// After a rename, the destination must hold the expected object; otherwise the one
/// that took the source's place in between goes back to the source name. The identity
/// is read before the rename: FUSE may no longer describe a descriptor whose name went.
fn confirm(expected: Identity, from_parent: &File, from: &OsStr, parent: &File, to: &OsStr) -> io::Result<()> {
    if entry(parent, to)? == expected {
        return Ok(());
    }
    if let Err(error) = rename_noreplace(parent, to, from_parent, from) {
        log::warn!("{} could not be put back: {error}", to.to_string_lossy());
    }
    Err(changed(from))
}

/// Exclusive rename of a file. FUSE file systems such as ntfs-3g refuse
/// `RENAME_NOREPLACE`: a hard link never replaces, then the old name is removed if it
/// is still the same object. Without hard links, the name is checked then renamed.
fn publish_exclusive(from_parent: &File, from: &OsStr, parent: &File, to: &OsStr, expected: Identity) -> io::Result<()> {
    match rustix::fs::renameat_with(from_parent, from, parent, to, RenameFlags::NOREPLACE) {
        Err(Errno::INVAL) => {}
        result => return result.map_err(io::Error::from),
    }
    match rustix::fs::linkat(from_parent, from, parent, to, AtFlags::empty()) {
        Ok(()) => {}
        Err(Errno::PERM | Errno::OPNOTSUPP | Errno::MLINK) => return rename_noreplace(from_parent, from, parent, to),
        Err(error) => return Err(error.into()),
    }
    match remove_entry(from_parent, from, expected, false) {
        // Another object took the old name: the held one now lives at `to` only.
        Err(_) if entry(from_parent, from).ok() != Some(expected) => Ok(()),
        Err(error) => {
            let _ = remove_entry(parent, to, expected, false);
            Err(error)
        }
        Ok(()) => Ok(()),
    }
}

/// Renames the held file, wherever it now is, to `destination` in `parent`.
pub(super) fn rename_file(file: &File, source: &Path, parent: &File, destination: &Path, replace: bool) -> io::Result<()> {
    let expected = identity(file)?;
    let (folder, name) = located(file, source)?;
    let target = leaf(destination)?;
    if replace {
        rustix::fs::renameat(&folder, &name, parent, target)?;
    } else {
        publish_exclusive(&folder, &name, parent, target, expected)?;
    }
    confirm(expected, &folder, &name, parent, target)
}

/// Renames an item checked by the caller from `source_parent` without replacing.
pub(super) fn rename_item(file: &File, source_parent: &File, source: &Path, parent: &File, destination: &Path) -> io::Result<()> {
    let (name, target, expected) = (leaf(source)?, leaf(destination)?, identity(file)?);
    if entry(source_parent, name)? != expected {
        return Err(changed(name));
    }
    rename_noreplace(source_parent, name, parent, target)?;
    confirm(expected, source_parent, name, parent, target)
}

/// A held folder for an absolute path whose last component is the effect's name.
pub(super) fn hold_parent(path: &Path) -> io::Result<File> {
    let parent = path.parent().ok_or_else(|| io::Error::other("Operation path has no folder"))?;
    super::linux_access::hold_directory(parent, true)
}

#[cfg(test)]
#[path = "linux_held/tests/held.rs"]
mod tests;
