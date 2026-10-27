//! Permissions tab: reads and changes the mode and group of the object the window
//! opened, as Nautilus does through GIO. Account names come from NSS (`getent`).
use crate::models::permissions::{Account, PermissionChange, PermissionError, UnixPermissions};
use rustix::fs::{AtFlags, Mode, OFlags, CWD};
use std::{
    collections::HashMap,
    io,
    os::fd::{AsRawFd, OwnedFd},
    path::Path,
    sync::atomic::AtomicBool,
};

mod enclosed;

/// The target of a link, as the rest of Properties describes it.
pub(super) fn read(path: &Path) -> io::Result<UnixPermissions> {
    let stat = rustix::fs::stat(path)?;
    let euid = rustix::process::geteuid();
    let mut gids = rustix::process::getgroups()?;
    gids.push(rustix::process::getegid());
    let mut gids: Vec<u32> = gids.into_iter().map(|gid| gid.as_raw()).collect();
    gids.push(stat.st_gid);
    gids.sort_unstable();
    gids.dedup();
    let group_names = names("group", &gids);
    let account = |names: &HashMap<u32, String>, id: u32| Account {
        id,
        name: names.get(&id).cloned().unwrap_or_else(|| id.to_string()),
    };
    let mut groups: Vec<_> = gids.iter().map(|gid| account(&group_names, *gid)).collect();
    groups.sort_by(|first, second| first.name.cmp(&second.name));
    Ok(UnixPermissions {
        device: stat.st_dev,
        inode: stat.st_ino,
        mode: stat.st_mode & 0o7777,
        is_dir: rustix::fs::FileType::from_raw_mode(stat.st_mode).is_dir(),
        owner: account(&names("passwd", &[stat.st_uid]), stat.st_uid),
        group: account(&group_names, stat.st_gid),
        groups,
        editable: euid.is_root() || euid.as_raw() == stat.st_uid,
    })
}

/// Names of the given ids in an NSS database; ids it does not know are left out.
fn names(database: &str, ids: &[u32]) -> HashMap<u32, String> {
    let output = std::process::Command::new("getent")
        .arg(database)
        .args(ids.iter().map(u32::to_string))
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();
    let Ok(output) = output else { return HashMap::new() };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split(':');
            let name = fields.next()?;
            let id = fields.nth(1)?.parse().ok()?;
            Some((id, name.to_owned()))
        })
        .collect()
}

/// fchmod refuses an O_PATH descriptor; its /proc entry reaches the same object.
fn chmod(file: &OwnedFd, mode: u32) -> rustix::io::Result<()> {
    let link = format!("/proc/self/fd/{}", file.as_raw_fd());
    rustix::fs::chmodat(CWD, link.as_str(), Mode::from_raw_mode(mode), AtFlags::empty())
}

/// Applies a change to the object read earlier, never to a replacement. The content
/// of a folder changes first, so that the folder's own change cannot close it.
pub(super) fn write(path: &Path, change: &PermissionChange, cancel: &AtomicBool) -> Result<(), PermissionError> {
    let failed = |error: io::Error| PermissionError::Failed(error.to_string());
    let file = rustix::fs::open(path, OFlags::PATH | OFlags::CLOEXEC, Mode::empty())
        .map_err(|error| failed(error.into()))?;
    let expected = &change.expected;
    let stat = rustix::fs::fstat(&file).map_err(|error| failed(error.into()))?;
    if (stat.st_dev, stat.st_ino, stat.st_mode & 0o7777, stat.st_gid)
        != (expected.device, expected.inode, expected.mode, expected.group.id)
    {
        return Err(PermissionError::Changed);
    }
    let enclosed = if expected.is_dir && !change.enclosed.is_empty() {
        enclosed::apply(&file, path, stat.st_dev, &change.enclosed, cancel).map_err(failed)?
    } else {
        enclosed::Outcome::default()
    };
    let mut special = expected.mode & 0o7000;
    if change.group != expected.group.id {
        rustix::fs::chownat(&file, "", None, Some(rustix::fs::Gid::from_raw(change.group)), AtFlags::EMPTY_PATH)
            .map_err(|error| failed(error.into()))?;
        // The kernel may clear setuid and setgid on a change of group: keep its choice.
        special = rustix::fs::fstat(&file).map_err(|error| failed(error.into()))?.st_mode & 0o7000;
    }
    let mode = (change.mode & 0o777) | (special & change.mode);
    if mode != stat.st_mode & 0o7777 {
        chmod(&file, mode).map_err(|error| failed(error.into()))?;
    }
    let applied = rustix::fs::fstat(&file).map_err(|error| failed(error.into()))?;
    if (applied.st_mode & 0o777, applied.st_gid) != (mode & 0o777, change.group) {
        return Err(PermissionError::NotKept);
    }
    match enclosed.first {
        Some(first) => Err(PermissionError::Incomplete { failed: enclosed.failed, first }),
        None => Ok(()),
    }
}

#[cfg(test)]
#[path = "linux_permissions/tests/permissions.rs"]
mod tests;
