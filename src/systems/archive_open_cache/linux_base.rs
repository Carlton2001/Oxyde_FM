//! Where Linux keeps the copies of opened archive members: the user cache folder, where
//! eviction takes a write lease. A network home (NFS, 9p) grants no lease, and nothing
//! would ever be evicted there: a private folder in the system's temporary folder takes
//! the copies instead, `/tmp` being shared by every account.
use crate::platform::platform;
use std::fs;
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub(super) fn base() -> io::Result<PathBuf> {
    static LEASES: OnceLock<bool> = OnceLock::new();
    let cache = crate::app_paths::cache_dir().ok_or_else(|| io::Error::other("No user cache folder"))?;
    fs::create_dir_all(&cache)?;
    if *LEASES.get_or_init(|| leases(&cache)) {
        return Ok(cache);
    }
    private_folder(&crate::app_paths::system_temporary_dir()?)
}

/// Whether the file system of `folder` grants the lease eviction needs.
pub(super) fn leases(folder: &Path) -> bool {
    let probe = folder.join(format!(".lease-probe-{}", std::process::id()));
    if fs::write(&probe, b"").is_err() {
        return true;
    }
    let granted = platform().open_cache_cleanup(&probe, false);
    let _ = fs::remove_file(&probe);
    match granted {
        Err(error) if matches!(error.raw_os_error(), Some(code) if [libc::EINVAL, libc::ENOSYS, libc::ENOLCK, libc::EOPNOTSUPP].contains(&code)) => {
            log::info!("{} grants no file lease: archive members open from the temporary folder", folder.display());
            false
        }
        _ => true,
    }
}

/// `oxyde-<uid>` in `temporary`, only if this account owns it alone.
pub(super) fn private_folder(temporary: &Path) -> io::Result<PathBuf> {
    let uid = rustix::process::getuid().as_raw();
    let folder = temporary.join(format!("oxyde-{uid}"));
    match fs::DirBuilder::new().mode(0o700).create(&folder) {
        Err(error) if error.kind() != io::ErrorKind::AlreadyExists => return Err(error),
        _ => {}
    }
    let metadata = fs::symlink_metadata(&folder)?;
    if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
        return Err(io::Error::other(format!("{} is not a private folder of this account", folder.display())));
    }
    Ok(folder)
}
