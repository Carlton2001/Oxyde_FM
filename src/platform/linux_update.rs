//! Releases for the per-user installation that install.sh makes: each version in its own
//! folder under ~/.local/lib/oxyde, behind a `current` link, so that installing never
//! pulls files from under a running Oxyde. The signed archive's own install.sh places it,
//! which keeps one description of that layout; its `--archive` and `--update` options are
//! therefore a contract between releases.
use super::{Installation, ReleaseSignature};
use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

const ARCHIVE_KEY: &str = "linux-x86_64";
const DEB_KEY: &str = "linux-x86_64-deb";
const RPM_KEY: &str = "linux-x86_64-rpm";
const SYSTEM_EXECUTABLE: &str = "/usr/bin/oxyde";
const DPKG_FILE_LIST: &str = "/var/lib/dpkg/info/oxyde.list";
const SCRIPT_LIMIT: u64 = 1024 * 1024;
/// Prefix of the temporary folder `systems::updates` downloads into.
const DOWNLOAD_FOLDER_PREFIX: &str = "oxyde-update-";

pub(super) fn installation() -> Installation {
    static FOUND: OnceLock<Installation> = OnceLock::new();
    *FOUND.get_or_init(|| match std::env::current_exe() {
        Ok(executable) => detect(&executable, dirs::home_dir().as_deref(), Path::new(DPKG_FILE_LIST).exists()),
        Err(_) => Installation::Unmanaged,
    })
}

fn detect(executable: &Path, home: Option<&Path>, dpkg_owned: bool) -> Installation {
    if executable == Path::new(SYSTEM_EXECUTABLE) {
        // Oxyde publishes a DEB and an RPM, and /usr/bin/oxyde comes from one of them.
        Installation::Package { manifest_key: Some(if dpkg_owned { DEB_KEY } else { RPM_KEY }) }
    } else if user_root(executable, home).is_some() {
        Installation::SelfUpdating { manifest_key: ARCHIVE_KEY, signature: ReleaseSignature::OpenPgp }
    } else {
        Installation::Unmanaged
    }
}

/// `~/.local/lib/oxyde` when `executable` is its `<version>/bin/oxyde` and the folder
/// holds install.sh's `current` link. The executable path is resolved, as the kernel
/// reports it; home may go through a link, as /home does on Fedora Silverblue.
fn user_root(executable: &Path, home: Option<&Path>) -> Option<PathBuf> {
    let bin = executable.parent()?;
    if executable.file_name()? != "oxyde" || bin.file_name()? != "bin" {
        return None;
    }
    let root = bin.parent()?.parent()?;
    let expected = std::fs::canonicalize(home?.join(".local/lib/oxyde")).ok()?;
    (std::fs::canonicalize(root).ok()? == expected && root.join("current").is_symlink()).then(|| root.to_path_buf())
}

/// Install the verified archive with its own install.sh, then start the new version
/// once this process has exited.
pub(super) fn install(archive: &Path) -> io::Result<()> {
    let result = (|| {
        let root = user_root(&std::env::current_exe()?, dirs::home_dir().as_deref())
            .ok_or_else(|| io::Error::other("Oxyde was not installed by install.sh"))?;
        install_into(archive, &root, std::env::var_os("XDG_DATA_HOME").map(PathBuf::from))?;
        relaunch_after(std::process::id(), &root.join("current/bin/oxyde"))
    })();
    remove_download_folder(archive);
    result
}

/// Run the archive's install.sh for the home that holds `root`, the running copy's.
fn install_into(archive: &Path, root: &Path, data_home: Option<PathBuf>) -> io::Result<()> {
    let home = root.parent().and_then(Path::parent).and_then(Path::parent)
        .ok_or_else(|| io::Error::other("Unexpected installation folder"))?;
    let script = archive.with_file_name("install.sh");
    extract_script(archive, &script)?;
    let mut command = Command::new("bash");
    command.arg(&script).arg("--archive").arg(archive).arg("--update")
        .env("HOME", home).env_remove("XDG_DATA_HOME");
    if let Some(data_home) = data_home {
        command.env("XDG_DATA_HOME", data_home);
    }
    let output = command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).output()?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        let message = message.lines().rev().map(str::trim).find(|line| !line.is_empty())
            .map_or_else(|| format!("install.sh failed with {}", output.status), |line| line.trim_start_matches("Error: ").to_owned());
        return Err(io::Error::other(message));
    }
    Ok(())
}

fn remove_download_folder(archive: &Path) {
    if let Some(folder) = archive.parent()
        .filter(|folder| folder.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with(DOWNLOAD_FOLDER_PREFIX)))
    {
        let _ = std::fs::remove_dir_all(folder);
    }
}

/// Copy `oxyde-<version>-x86_64-linux/install.sh` out of the archive.
fn extract_script(archive: &Path, destination: &Path) -> io::Result<()> {
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(File::open(archive)?));
    for entry in archive.entries()? {
        let mut entry = entry?;
        if !is_script(&entry.path()?) {
            continue;
        }
        if entry.header().entry_type() != tar::EntryType::Regular || entry.size() > SCRIPT_LIMIT {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "The update archive's install.sh is not a plain file"));
        }
        let mut file = OpenOptions::new().write(true).create_new(true).mode(0o700).open(destination)?;
        io::copy(&mut (&mut entry).take(SCRIPT_LIMIT), &mut file)?;
        return file.sync_all();
    }
    Err(io::Error::new(io::ErrorKind::InvalidData, "The update archive has no install.sh"))
}

fn is_script(path: &Path) -> bool {
    let mut parts = path.components();
    match (parts.next(), parts.next(), parts.next()) {
        (Some(Component::Normal(top)), Some(Component::Normal(name)), None) => name == "install.sh"
            && top.to_str().is_some_and(|top| top.starts_with("oxyde-") && top.ends_with("-x86_64-linux")),
        _ => false,
    }
}

/// The new version must read the session this one saves on close: a detached shell
/// waits for this process to end before starting it.
fn relaunch_after(process: u32, executable: &Path) -> io::Result<()> {
    let mut command = Command::new("sh");
    super::linux_gpu::leave_out_of(&mut command);
    command
        .arg("-c").arg(r#"while kill -0 "$1" 2>/dev/null; do sleep 0.2; done; exec "$2""#)
        .arg("oxyde-relaunch").arg(process.to_string()).arg(executable)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map(drop)
}

#[cfg(test)]
#[path = "linux_update/tests/update.rs"]
mod tests;
