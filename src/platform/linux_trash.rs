//! Freedesktop trash shared by GNOME, KDE and other XDG desktops: `$trash/files/<name>`
//! holds the content, `$trash/info/<name>.trashinfo` its record and listing identity.
//! Restoration never overwrites nor recreates a missing original folder. Automatic cleanup
//! belongs to the desktop: GNOME housekeeping settings or KDE trash limits.
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use crate::models::error::{TrashRestoreFailure, TrashRestoreIssue};

/// Content of a listed item, beside its `.trashinfo` record in the same trash directory.
pub(super) fn content_path(item: &trash::TrashItem) -> Option<PathBuf> {
    let info = Path::new(&item.id);
    Some(trash_root(info)?.join("files").join(info.file_stem()?))
}

fn trash_root(info: &Path) -> Option<&Path> {
    info.parent()?.parent()
}

/// Recognizes `$trash/files/...`, so transfers never split content from its record.
pub(super) fn inside_trash(path: &Path) -> bool {
    path.ancestors().skip(1).any(|ancestor| {
        ancestor.file_name() == Some(OsStr::new("files"))
            && ancestor.parent().is_some_and(|root| root.join("info").is_dir() && is_trash_root(root))
    })
}

fn is_trash_root(root: &Path) -> bool {
    let name = root.file_name().map(OsStr::as_bytes).unwrap_or_default();
    name == b"Trash"
        || name.starts_with(b".Trash-")
        || root.parent().and_then(Path::file_name) == Some(OsStr::new(".Trash"))
}

pub(super) fn purge(items: &[trash::TrashItem]) -> Result<(), String> {
    for item in items {
        let info = Path::new(&item.id);
        let content = content_path(item).ok_or_else(|| format!("Invalid trash record: {info:?}"))?;
        match std::fs::symlink_metadata(&content) {
            // Links are removed themselves, never their target.
            Ok(metadata) if metadata.is_dir() => std::fs::remove_dir_all(&content),
            Ok(_) => std::fs::remove_file(&content),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
        .map_err(|error| format!("{content:?}: {error}"))?;
        std::fs::remove_file(info).map_err(|error| format!("{info:?}: {error}"))?;
        forget_directory_size(item);
    }
    Ok(())
}

pub(super) fn restore(item: &trash::TrashItem) -> Result<(), String> {
    let original = item.original_path();
    let failure = |issue, detail: String| TrashRestoreFailure {
        issue, path: original.to_string_lossy().into_owned(), detail,
    }.to_string();
    let info = Path::new(&item.id);
    let content = content_path(item)
        .ok_or_else(|| failure(TrashRestoreIssue::Unavailable, "Invalid trash record".into()))?;
    if let Err(error) = std::fs::symlink_metadata(&content) {
        return Err(failure(TrashRestoreIssue::Changed, format!("Trash content unavailable: {error}")));
    }
    if !item.original_parent.is_dir() {
        return Err(failure(TrashRestoreIssue::Unavailable, "Original folder no longer exists".into()));
    }
    rename_no_replace(&content, &original).map_err(|error| match error.raw_os_error() {
        Some(17) => failure(TrashRestoreIssue::Conflict, error.to_string()), // EEXIST
        Some(18) => failure(TrashRestoreIssue::Unavailable, // EXDEV
            "The trash and the original folder are on different file systems".into()),
        _ => failure(TrashRestoreIssue::Unavailable, error.to_string()),
    })?;
    // The content is back; a leftover record is skipped by listings, never restored twice.
    if let Err(error) = std::fs::remove_file(info) {
        log::warn!(target: "oxyde_lib::trash_restore", "stage=record_cleanup id={info:?} error={error}");
    }
    forget_directory_size(item);
    Ok(())
}

fn rename_no_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::ffi::{c_char, c_int, c_uint, CString};
    unsafe extern "C" {
        fn renameat2(old_dir: c_int, old_name: *const c_char, new_dir: c_int,
            new_name: *const c_char, flags: c_uint) -> c_int;
    }
    const AT_FDCWD: c_int = -100;
    const RENAME_NOREPLACE: c_uint = 1;
    let path = |path: &Path| CString::new(path.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Path contains a NUL byte"));
    let (from, to) = (path(from)?, path(to)?);
    // No replacement fallback: unsupported file systems refuse the restoration.
    let result = unsafe { renameat2(AT_FDCWD, from.as_ptr(), AT_FDCWD, to.as_ptr(), RENAME_NOREPLACE) };
    if result == 0 { Ok(()) } else { Err(std::io::Error::last_os_error()) }
}

/// Keep the Trash specification 1.0 directory size cache, read by KDE, in step.
fn forget_directory_size(item: &trash::TrashItem) {
    let info = Path::new(&item.id);
    let (Some(root), Some(name)) = (trash_root(info), info.file_stem()) else { return };
    let cache = root.join("directorysizes");
    let result = match std::fs::read(&cache) {
        Ok(content) => match without_cached_size(&content, name) {
            Some(kept) => write_atomically(&cache, &kept),
            None => Ok(()),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    };
    if let Err(error) = result {
        log::warn!(target: "oxyde_lib::trash", "stage=directory_sizes cache={cache:?} error={error}");
    }
}

/// Lines are `size mtime percent-encoded-name`; None when `name` has no entry.
fn without_cached_size(content: &[u8], name: &OsStr) -> Option<Vec<u8>> {
    let mut kept = Vec::with_capacity(content.len());
    let mut found = false;
    for line in content.split(|byte| *byte == b'\n').filter(|line| !line.is_empty()) {
        let entry = line.splitn(3, |byte| *byte == b' ').nth(2).map(percent_decode);
        if entry.as_deref() == Some(name.as_bytes()) {
            found = true;
        } else {
            kept.extend_from_slice(line);
            kept.push(b'\n');
        }
    }
    found.then_some(kept)
}

fn percent_decode(encoded: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::with_capacity(encoded.len());
    let mut index = 0;
    while index < encoded.len() {
        let hex = encoded.get(index + 1..index + 3)
            .and_then(|digits| std::str::from_utf8(digits).ok())
            .and_then(|digits| u8::from_str_radix(digits, 16).ok());
        match (encoded[index], hex) {
            (b'%', Some(byte)) => { decoded.push(byte); index += 3; }
            (byte, _) => { decoded.push(byte); index += 1; }
        }
    }
    decoded
}

fn write_atomically(path: &Path, content: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let parent = path.parent().ok_or_else(|| std::io::Error::other("Path has no parent"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(content)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map(|_| ()).map_err(|error| error.error)
}

pub(super) fn usage() -> Result<(u64, u64), String> {
    let items = trash::os_limited::list().map_err(|error| error.to_string())?;
    let mut size = 0_u64;
    let mut count = 0_u64;
    for content in items.iter().filter_map(content_path) {
        let mut stack = vec![content];
        let mut present = false;
        while let Some(path) = stack.pop() {
            let Ok(metadata) = std::fs::symlink_metadata(&path) else { continue };
            present = true;
            if metadata.is_dir() {
                if let Ok(children) = std::fs::read_dir(&path) {
                    stack.extend(children.filter_map(Result::ok).map(|child| child.path()));
                }
            } else {
                size = size.saturating_add(metadata.len());
            }
        }
        count += u64::from(present);
    }
    Ok((size, count))
}

/// Automatic cleanup configured by the running desktop; Oxyde only edits its settings.
#[derive(Clone, Debug, PartialEq)]
pub enum TrashRetention {
    Unavailable,
    /// GNOME housekeeping empties items older than `days`, whichever application trashed them.
    Gnome { enabled: bool, days: u32 },
    /// KDE applies these limits when its own applications move files to a trash directory.
    Kde(Vec<KdeTrashLimits>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct KdeTrashLimits {
    pub trash: PathBuf,
    pub use_time_limit: bool,
    pub days: u32,
    pub use_size_limit: bool,
    /// Share of the partition, as in KDE's own settings.
    pub percent: f64,
    pub action: KdeLimitAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KdeLimitAction {
    Warn,
    DeleteOldest,
    DeleteLargest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Desktop {
    Gnome,
    Kde,
    Other,
}

fn desktop() -> Desktop {
    let current = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_ascii_uppercase();
    let names: Vec<_> = current.split(':').collect();
    if names.contains(&"KDE") {
        Desktop::Kde
    } else if names.contains(&"GNOME") {
        Desktop::Gnome
    } else {
        Desktop::Other
    }
}

const GNOME_SCHEMA: &str = "org.gnome.desktop.privacy";

pub(super) fn retention() -> Result<TrashRetention, String> {
    match desktop() {
        Desktop::Gnome => {
            let enabled = gsettings(&["get", GNOME_SCHEMA, "remove-old-trash-files"])?.trim() == "true";
            // GVariant prints the unsigned age as `uint32 30`.
            let age = gsettings(&["get", GNOME_SCHEMA, "old-files-age"])?;
            let days = age.split_whitespace().last().and_then(|days| days.parse().ok())
                .ok_or_else(|| format!("Unexpected old-files-age value: {age}"))?;
            Ok(TrashRetention::Gnome { enabled, days })
        }
        Desktop::Kde => {
            let text = match std::fs::read_to_string(kde_config_path()?) {
                Ok(text) => text,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(error) => return Err(error.to_string()),
            };
            Ok(TrashRetention::Kde(kde_trash_directories().into_iter()
                .map(|trash| kde_limits(&text, trash)).collect()))
        }
        Desktop::Other => Ok(TrashRetention::Unavailable),
    }
}

pub(super) fn set_retention(retention: &TrashRetention) -> Result<(), String> {
    match retention {
        TrashRetention::Unavailable => Ok(()),
        TrashRetention::Gnome { enabled, days } => {
            gsettings(&["set", GNOME_SCHEMA, "old-files-age", &days.to_string()])?;
            gsettings(&["set", GNOME_SCHEMA, "remove-old-trash-files", if *enabled { "true" } else { "false" }])?;
            Ok(())
        }
        TrashRetention::Kde(limits) => {
            let path = kde_config_path()?;
            let mut text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(error) => return Err(error.to_string()),
            };
            for limits in limits {
                text = set_kconfig_values(&text, &limits.trash.to_string_lossy(), &kde_values(limits));
            }
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            write_atomically(&path, text.as_bytes()).map_err(|error| error.to_string())
        }
    }
}

fn gsettings(arguments: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new("gsettings").args(arguments).output()
        .map_err(|error| format!("gsettings: {error}"))?;
    if !output.status.success() {
        return Err(format!("gsettings: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn kde_config_path() -> Result<PathBuf, String> {
    dirs::config_dir().map(|config| config.join("ktrashrc"))
        .ok_or_else(|| "The configuration directory is unavailable".into())
}

/// The home trash, then each other trash directory that currently holds items.
fn kde_trash_directories() -> Vec<PathBuf> {
    let mut directories: Vec<PathBuf> = dirs::data_dir().map(|data| data.join("Trash")).into_iter().collect();
    for item in trash::os_limited::list().unwrap_or_default() {
        if let Some(root) = trash_root(Path::new(&item.id)) {
            if !directories.iter().any(|known| known == root) {
                directories.push(root.to_path_buf());
            }
        }
    }
    directories
}

/// Defaults are those KDE applies when a key is absent.
fn kde_limits(text: &str, trash: PathBuf) -> KdeTrashLimits {
    let group = trash.to_string_lossy().into_owned();
    let value = |key| kconfig_value(text, &group, key);
    let flag = |key, default| value(key).map_or(default, |value: &str| value.eq_ignore_ascii_case("true"));
    KdeTrashLimits {
        use_time_limit: flag("UseTimeLimit", false),
        days: value("Days").and_then(|days| days.parse().ok()).unwrap_or(7),
        use_size_limit: flag("UseSizeLimit", true),
        percent: value("Percent").and_then(|percent| percent.parse().ok()).unwrap_or(10.0),
        action: match value("LimitReachedAction") {
            Some("1") => KdeLimitAction::DeleteOldest,
            Some("2") => KdeLimitAction::DeleteLargest,
            _ => KdeLimitAction::Warn,
        },
        trash,
    }
}

fn kde_values(limits: &KdeTrashLimits) -> Vec<(&'static str, String)> {
    vec![
        ("UseTimeLimit", limits.use_time_limit.to_string()),
        ("Days", limits.days.to_string()),
        ("UseSizeLimit", limits.use_size_limit.to_string()),
        ("Percent", limits.percent.to_string()),
        ("LimitReachedAction", match limits.action {
            KdeLimitAction::Warn => "0",
            KdeLimitAction::DeleteOldest => "1",
            KdeLimitAction::DeleteLargest => "2",
        }.to_string()),
    ]
}

fn kconfig_group(line: &str) -> Option<&str> {
    line.trim().strip_prefix('[')?.strip_suffix(']')
}

fn kconfig_key(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once('=')?;
    Some((key.trim(), value.trim()))
}

fn kconfig_value<'a>(text: &'a str, group: &str, key: &str) -> Option<&'a str> {
    let mut current = None;
    let mut found = None;
    for line in text.lines() {
        if let Some(name) = kconfig_group(line) {
            current = Some(name);
        } else if current == Some(group) {
            if let Some((_, value)) = kconfig_key(line).filter(|(name, _)| *name == key) {
                found = Some(value);
            }
        }
    }
    found
}

/// Update or add keys in one group, keeping every other group, key and comment.
fn set_kconfig_values(text: &str, group: &str, values: &[(&str, String)]) -> String {
    let mut output = Vec::new();
    let mut written = vec![false; values.len()];
    let mut in_group = false;
    let mut seen_group = false;
    let flush = |output: &mut Vec<String>, written: &mut Vec<bool>| {
        // Keep trailing blank lines after the keys added to the group.
        let blanks = output.iter().rev().take_while(|line: &&String| line.trim().is_empty()).count();
        let at = output.len() - blanks;
        for (index, (key, value)) in values.iter().enumerate().rev() {
            if !written[index] {
                output.insert(at, format!("{key}={value}"));
                written[index] = true;
            }
        }
    };
    for line in text.lines() {
        if let Some(name) = kconfig_group(line) {
            if in_group {
                flush(&mut output, &mut written);
            }
            in_group = name == group;
            seen_group |= in_group;
        } else if in_group {
            if let Some(index) = kconfig_key(line).and_then(|(key, _)| values.iter().position(|(name, _)| *name == key)) {
                output.push(format!("{}={}", values[index].0, values[index].1));
                written[index] = true;
                continue;
            }
        }
        output.push(line.to_string());
    }
    if in_group {
        flush(&mut output, &mut written);
    }
    if !seen_group {
        if output.last().is_some_and(|line| !line.trim().is_empty()) {
            output.push(String::new());
        }
        output.push(format!("[{group}]"));
        flush(&mut output, &mut written);
    }
    let mut result = output.join("\n");
    result.push('\n');
    result
}

#[cfg(test)]
#[path = "linux_trash/tests/storage.rs"]
mod tests;
