//! A note per private staging folder (`file_copy::Staging`), kept beside the
//! configuration while the folder exists, so a process ended during an operation
//! (crash, kill, closing mid-copy) does not leave its partial output behind for good:
//! the next start removes the folders whose notes no live process still holds.
//!
//! Each note stays locked by its owner; a note that can be locked again belongs to
//! an ended process. A folder is removed only when it is still the one created
//! (same identity, same `.pin` marker), and only its `output` and marker: anything
//! else keeps it, as an in-session cleanup does.
use crate::platform::{platform, FileIdentity};
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(test)]
#[path = "staging_journal/tests/recovery.rs"]
mod tests;

/// Off until the application turns it on: tests and tools write no notes.
static ENABLED: AtomicBool = AtomicBool::new(false);

const FOLDER: &str = "interrupted";
const PREFIX: &str = ".oxyde-";

/// Notes the staging folders created from now on in this process.
pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Record {
    path: PathBuf,
    directory: FileIdentity,
    pin: FileIdentity,
}

/// A staging folder's note, removed when the folder's cleanup is over.
pub(crate) struct Entry {
    lock: Option<File>,
    path: PathBuf,
}

impl Drop for Entry {
    fn drop(&mut self) {
        // Windows refuses to delete a file its own exclusive handle holds.
        drop(self.lock.take());
        if let Err(error) = std::fs::remove_file(&self.path) {
            if error.kind() != io::ErrorKind::NotFound {
                log::warn!("Cannot remove staging note {}: {error}", self.path.display());
            }
        }
    }
}

fn folder() -> Option<PathBuf> {
    crate::app_paths::config_dir().map(|config| config.join(FOLDER))
}

/// Notes `staging` with the identities of its folder and marker; a failure only
/// loses the recovery of this folder.
pub(crate) fn record(staging: &Path, directory: &File, pin: &File) -> Option<Entry> {
    if !ENABLED.load(Ordering::Relaxed) {
        return None;
    }
    let result = folder()
        .ok_or_else(|| io::Error::other("No configuration folder"))
        .and_then(|folder| write(&folder, staging, directory, pin));
    match result {
        Ok(entry) => Some(entry),
        Err(error) => {
            log::warn!("Cannot note staging {}: {error}", staging.display());
            None
        }
    }
}

fn write(folder: &Path, staging: &Path, directory: &File, pin: &File) -> io::Result<Entry> {
    std::fs::create_dir_all(folder)?;
    let name = staging.file_name().ok_or_else(|| io::Error::other("Staging has no name"))?;
    // Named after the folder, without its leading dot.
    let path = folder.join(format!("{}.json", name.to_string_lossy().trim_start_matches('.')));
    let lock = platform().lock_cache_file(&path)?;
    // From here, dropping the entry removes a note left incomplete.
    let mut entry = Entry { lock: Some(lock), path };
    let record = Record {
        path: staging.to_path_buf(),
        directory: platform().file_identity(directory)?,
        pin: platform().file_identity(pin)?,
    };
    let bytes = serde_json::to_vec(&record).map_err(io::Error::other)?;
    let lock = entry.lock.as_mut().expect("held note");
    lock.set_len(0)?;
    lock.write_all(&bytes)?;
    Ok(entry)
}

/// What a start found left by ended processes.
#[derive(Debug, Default)]
pub struct Recovered {
    /// Staging folders removed with their partial output.
    pub removed: Vec<PathBuf>,
    /// Folders left in place: changed since, or holding more than their output.
    pub kept: Vec<PathBuf>,
}

/// Removes the staging folders of ended processes. Blocking: call it off the UI thread.
pub fn recover() -> Recovered {
    folder().map(|folder| recover_in(&folder)).unwrap_or_default()
}

fn recover_in(folder: &Path) -> Recovered {
    let mut recovered = Recovered::default();
    let Ok(notes) = std::fs::read_dir(folder) else { return recovered };
    for note in notes.flatten() {
        let path = note.path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        // Still held: its process runs, possibly an Oxyde ending or a worker.
        let Ok(mut lock) = platform().lock_cache_file(&path) else { continue };
        let mut text = String::new();
        let record = lock
            .read_to_string(&mut text)
            .ok()
            .and_then(|_| serde_json::from_str::<Record>(&text).ok());
        if let Some(record) = record {
            match clean(&record) {
                Ok(true) => recovered.removed.push(record.path),
                Ok(false) => {}
                Err(error) => {
                    log::warn!("Interrupted staging kept at {}: {error}", record.path.display());
                    recovered.kept.push(record.path);
                }
            }
        }
        drop(Entry { lock: Some(lock), path });
    }
    recovered
}

/// Removes the noted folder if it is still the one created; false when it is gone.
fn clean(record: &Record) -> io::Result<bool> {
    let named = record
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(PREFIX));
    if !named {
        return Err(io::Error::other("Not a staging folder"));
    }
    let directory = match platform().open_copy_staging(&record.path) {
        Ok(directory) => directory,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if platform().file_identity(&directory)? != record.directory {
        return Err(io::Error::other("Staging folder was replaced"));
    }
    let marker = record.path.join(".pin");
    let pin = platform().open_copied_file(&directory, &marker)?;
    if platform().file_identity(&pin)? != record.pin {
        return Err(io::Error::other("Staging marker was replaced"));
    }
    let output = record.path.join("output");
    match platform().open_copied_file(&directory, &output) {
        Ok(file) => platform().remove_copied_file(&file, &output)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    platform().remove_copy_staging(&directory, pin, &record.path)?;
    Ok(true)
}
