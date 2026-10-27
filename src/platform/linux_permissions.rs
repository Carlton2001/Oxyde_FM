//! Permissions tab: reads and changes the mode and group of the object the window
//! opened, as Nautilus does through GIO; root may also change the owner. Account names
//! come from NSS (`getent`).
use crate::models::permissions::{Account, PermissionChange, PermissionError, UnixPermissions};
use rustix::fs::{AtFlags, Mode, OFlags, CWD};
use std::{
    collections::HashMap,
    io,
    os::fd::{AsRawFd, OwnedFd},
    path::Path,
    sync::{atomic::AtomicBool, LazyLock, Mutex, MutexGuard},
    time::{Duration, Instant},
};

mod enclosed;

/// The target of a link, as the rest of Properties describes it.
pub(super) fn read(path: &Path) -> io::Result<UnixPermissions> {
    let stat = rustix::fs::stat(path)?;
    let euid = rustix::process::geteuid();
    let account = |database, id| Account { id, name: name(database, id).unwrap_or_else(|| id.to_string()) };
    let owner = account("passwd", stat.st_uid);
    let group = account("group", stat.st_gid);
    // As Nautilus: an administrator may give any account, the owner one of its groups.
    let (mut owners, mut groups) = if euid.is_root() {
        (all("passwd"), all("group"))
    } else {
        let mut gids = rustix::process::getgroups()?;
        gids.push(rustix::process::getegid());
        let mut gids: Vec<u32> = gids.into_iter().map(|gid| gid.as_raw()).collect();
        gids.sort_unstable();
        gids.dedup();
        (Vec::new(), gids.into_iter().map(|gid| account("group", gid)).collect())
    };
    if euid.is_root() && !owners.iter().any(|account| account.id == owner.id) {
        owners.push(owner.clone());
    }
    if !groups.iter().any(|account| account.id == group.id) {
        groups.push(group.clone());
    }
    owners.sort_by(|first, second| first.name.cmp(&second.name));
    groups.sort_by(|first, second| first.name.cmp(&second.name));
    Ok(UnixPermissions {
        device: stat.st_dev,
        inode: stat.st_ino,
        mode: stat.st_mode & 0o7777,
        is_dir: rustix::fs::FileType::from_raw_mode(stat.st_mode).is_dir(),
        owner,
        group,
        groups,
        owners,
        editable: euid.is_root() || euid.as_raw() == stat.st_uid,
    })
}

/// Read from /proc: setting the umask to learn it would race with other threads.
pub(super) fn umask() -> Option<u32> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let value = status.lines().find_map(|line| line.strip_prefix("Umask:"))?;
    u32::from_str_radix(value.trim(), 8).ok()
}

/// NSS answers, kept a few seconds: the items of a selection share their accounts,
/// and each question is a `getent` process.
struct Accounts {
    since: Instant,
    names: HashMap<(&'static str, u32), Option<String>>,
    lists: HashMap<&'static str, Vec<Account>>,
}

impl Accounts {
    fn new() -> Self {
        Self { since: Instant::now(), names: HashMap::new(), lists: HashMap::new() }
    }
}

fn accounts() -> MutexGuard<'static, Accounts> {
    static ACCOUNTS: LazyLock<Mutex<Accounts>> = LazyLock::new(|| Mutex::new(Accounts::new()));
    let mut accounts = ACCOUNTS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if accounts.since.elapsed() > Duration::from_secs(10) {
        *accounts = Accounts::new();
    }
    accounts
}

/// `getent` lines as their fields; an empty key list enumerates the database.
fn getent(database: &str, keys: &[String]) -> Vec<Vec<String>> {
    let output = std::process::Command::new("getent")
        .arg(database)
        .args(keys)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();
    let Ok(output) = output else { return Vec::new() };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.split(':').map(str::to_owned).collect::<Vec<_>>())
        .filter(|fields| fields.len() >= 3)
        .collect()
}

fn name(database: &'static str, id: u32) -> Option<String> {
    let mut accounts = accounts();
    if let Some(name) = accounts.names.get(&(database, id)) {
        return name.clone();
    }
    let name = getent(database, &[id.to_string()])
        .into_iter()
        .find(|fields| fields[2].parse() == Ok(id))
        .map(|fields| fields[0].clone());
    accounts.names.insert((database, id), name.clone());
    name
}

/// Every account of the database, as Nautilus lists them for an administrator.
fn all(database: &'static str) -> Vec<Account> {
    if let Some(list) = accounts().lists.get(database) {
        return list.clone();
    }
    let list: Vec<_> = getent(database, &[])
        .into_iter()
        .filter_map(|fields| {
            let id = fields[2].parse().ok()?;
            let real = (database == "passwd").then(|| fields.get(4).and_then(|gecos| real_name(&fields[0], gecos))).flatten();
            let name = match real {
                Some(real) => format!("{} – {real}", fields[0]),
                None => fields[0].clone(),
            };
            Some(Account { id, name })
        })
        .collect();
    accounts().lists.insert(database, list.clone());
    list
}

/// The part of GECOS before the first comma, `&` standing for the capitalised login,
/// as Nautilus reads it.
fn real_name(login: &str, gecos: &str) -> Option<String> {
    let name = gecos.split(',').next()?.trim();
    if name.is_empty() {
        return None;
    }
    let mut capitalised = login.chars();
    let capitalised: String = capitalised.next().into_iter().flat_map(char::to_uppercase).chain(capitalised).collect();
    Some(name.replace('&', &capitalised))
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
    if (stat.st_dev, stat.st_ino, stat.st_mode & 0o7777, stat.st_uid, stat.st_gid)
        != (expected.device, expected.inode, expected.mode, expected.owner.id, expected.group.id)
    {
        return Err(PermissionError::Changed);
    }
    let enclosed = if expected.is_dir && !change.enclosed.is_empty() {
        enclosed::apply(&file, path, stat.st_dev, &change.enclosed, cancel).map_err(failed)?
    } else {
        enclosed::Outcome::default()
    };
    let mut special = expected.mode & 0o7000;
    let owner = (change.owner != expected.owner.id).then(|| rustix::fs::Uid::from_raw(change.owner));
    let group = (change.group != expected.group.id).then(|| rustix::fs::Gid::from_raw(change.group));
    if owner.is_some() || group.is_some() {
        rustix::fs::chownat(&file, "", owner, group, AtFlags::EMPTY_PATH).map_err(|error| failed(error.into()))?;
        // The kernel may clear setuid and setgid on a change of owner or group: keep its choice.
        special = rustix::fs::fstat(&file).map_err(|error| failed(error.into()))?.st_mode & 0o7000;
    }
    let mode = (change.mode & 0o777) | (special & change.mode);
    if mode != stat.st_mode & 0o7777 {
        chmod(&file, mode).map_err(|error| failed(error.into()))?;
    }
    let applied = rustix::fs::fstat(&file).map_err(|error| failed(error.into()))?;
    if (applied.st_mode & 0o777, applied.st_uid, applied.st_gid) != (mode & 0o777, change.owner, change.group) {
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
