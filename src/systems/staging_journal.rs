//! A note per file being written (`copy_output::Output`), kept beside the
//! configuration until the file is kept or cleaned, so a process ended during an
//! operation (crash, kill, closing mid-copy) does not leave its partial output behind
//! for good: the next start removes the files whose notes no live process still holds.
//!
//! Each note stays locked by its owner; a note that can be locked again belongs to
//! an ended process. A file is removed only when it is still the one created (same
//! identity and creation date); anything else stays, as an in-session cleanup does.
//! Notes of the staging folders earlier versions wrote are still recovered: a folder
//! goes only when it is still the one created (same identity, same `.pin` marker),
//! and only its `output` and marker.
use crate::platform::{platform, FileIdentity};
use std::fs::File;
use std::io::{self, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

#[cfg(test)]
#[path = "staging_journal/tests/recovery.rs"]
mod tests;

/// Off until the application turns it on: tests and tools write no notes.
static ENABLED: AtomicBool = AtomicBool::new(false);

const FOLDER: &str = "interrupted";
const PREFIX: &str = ".oxyde-";

/// Notes the files written from now on in this process.
pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
enum Record {
    /// A file written under its own name, or as the temporary of a replacement.
    File {
        file: PathBuf,
        identity: FileIdentity,
        /// Absent where the file system keeps no creation date: the identity alone
        /// does not prove the file, whose number a later one may reuse.
        created: Option<SystemTime>,
    },
    /// A staging folder of an earlier version.
    Folder {
        path: PathBuf,
        directory: FileIdentity,
        pin: FileIdentity,
    },
}

impl Record {
    fn path(&self) -> &Path {
        match self {
            Self::File { file, .. } => file,
            Self::Folder { path, .. } => path,
        }
    }
}

/// A written file's note, removed once the file is kept or cleaned.
pub(crate) struct Entry {
    lock: Option<File>,
    path: PathBuf,
    /// What the note holds, to rewrite it; none for a note recovery removes.
    record: Option<Record>,
}

impl Entry {
    /// Notes the creation date the file ended with: a native copy applies the source's
    /// date after the note was written, and a note left with the first one would keep
    /// a finished copy after a crash. A rewrite that fails leaves an unreadable note,
    /// which recovery discards without removing the file.
    pub(crate) fn update_created(&mut self, created: Option<SystemTime>) {
        let Some(Record::File { created: noted, .. }) = &mut self.record else { return };
        if *noted == created {
            return;
        }
        *noted = created;
        let result = match self.lock.as_mut() {
            Some(lock) => store(lock, self.record.as_ref().expect("noted file")),
            None => return,
        };
        if let Err(error) = result {
            log::warn!("Cannot update output note {}: {error}", self.path.display());
        }
    }
}

impl Drop for Entry {
    fn drop(&mut self) {
        // Windows refuses to delete a file its own exclusive handle holds.
        drop(self.lock.take());
        if let Err(error) = std::fs::remove_file(&self.path) {
            if error.kind() != io::ErrorKind::NotFound {
                log::warn!("Cannot remove output note {}: {error}", self.path.display());
            }
        }
    }
}

fn folder() -> Option<PathBuf> {
    crate::app_paths::config_dir().map(|config| config.join(FOLDER))
}

/// Notes the file just created at `path` with its identity and creation date; a
/// failure only loses the recovery of this file.
pub(crate) fn record_file(path: &Path, identity: FileIdentity, created: Option<SystemTime>) -> Option<Entry> {
    if !ENABLED.load(Ordering::Relaxed) {
        return None;
    }
    let result = folder()
        .ok_or_else(|| io::Error::other("No configuration folder"))
        .and_then(|folder| write_file(&folder, path, identity, created));
    match result {
        Ok(entry) => Some(entry),
        Err(error) => {
            log::warn!("Cannot note output {}: {error}", path.display());
            None
        }
    }
}

fn write_file(folder: &Path, path: &Path, identity: FileIdentity, created: Option<SystemTime>) -> io::Result<Entry> {
    let record = Record::File { file: path.to_path_buf(), identity, created };
    write(folder, &format!("file-{}", uuid::Uuid::new_v4()), &record)
}

/// The note of an earlier version's staging folder.
#[cfg(test)]
fn write_folder(folder: &Path, staging: &Path, directory: &File, pin: &File) -> io::Result<Entry> {
    let name = staging.file_name().ok_or_else(|| io::Error::other("Staging has no name"))?;
    let record = Record::Folder {
        path: staging.to_path_buf(),
        directory: platform().file_identity(directory)?,
        pin: platform().file_identity(pin)?,
    };
    // Named after the folder, without its leading dot.
    write(folder, name.to_string_lossy().trim_start_matches('.'), &record)
}

fn write(folder: &Path, name: &str, record: &Record) -> io::Result<Entry> {
    std::fs::create_dir_all(folder)?;
    let path = folder.join(format!("{name}.json"));
    let lock = platform().lock_cache_file(&path)?;
    // From here, dropping the entry removes a note left incomplete.
    let mut entry = Entry { lock: Some(lock), path, record: Some(record.clone()) };
    store(entry.lock.as_mut().expect("held note"), record)?;
    Ok(entry)
}

/// Replaces the content of the held note with `record`.
fn store(lock: &mut File, record: &Record) -> io::Result<()> {
    let bytes = serde_json::to_vec(record).map_err(io::Error::other)?;
    lock.set_len(0)?;
    lock.rewind()?;
    lock.write_all(&bytes)
}

/// What a start found left by ended processes.
#[derive(Debug, Default)]
pub struct Recovered {
    /// Partial files removed, and staging folders removed with their partial output.
    pub removed: Vec<PathBuf>,
    /// Left in place: changed since, unproven, or a folder holding more than its output.
    pub kept: Vec<PathBuf>,
}

/// Removes the partial files and staging folders of ended processes. Blocking: call
/// it off the UI thread.
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
            let cleaned = match &record {
                Record::File { file, identity, created } => clean_file(file, *identity, *created),
                Record::Folder { path, directory, pin } => clean_folder(path, *directory, *pin),
            };
            let target = record.path().to_path_buf();
            match cleaned {
                Ok(Cleaned::Removed) => recovered.removed.push(target),
                Ok(Cleaned::Gone) => {}
                // A USB key unplugged since: the note waits for a start that reaches it,
                // unless it has waited long enough for the volume to be given up.
                Ok(Cleaned::Unreachable) if !expired(&path) => {
                    log::info!("Interrupted output out of reach for now: {}", target.display());
                    drop(lock);
                    continue;
                }
                Ok(Cleaned::Unreachable) => {}
                Err(error) => {
                    log::warn!("Interrupted output kept at {}: {error}", target.display());
                    recovered.kept.push(target);
                }
            }
        }
        drop(Entry { lock: Some(lock), path, record: None });
    }
    recovered
}

/// How long a note waits for its unreachable volume to come back.
const UNREACHABLE_KEPT: std::time::Duration = std::time::Duration::from_secs(90 * 24 * 60 * 60);

fn expired(note: &Path) -> bool {
    std::fs::metadata(note)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|written| written.elapsed().ok())
        .is_some_and(|age| age > UNREACHABLE_KEPT)
}

enum Cleaned {
    Removed,
    /// The item no longer exists where its parent still does.
    Gone,
    /// Neither the item nor its parent can be reached (volume unplugged).
    Unreachable,
}

fn parent_reachable(path: &Path) -> bool {
    path.parent().is_some_and(Path::exists)
}

/// Removes the noted file if it is still the one created.
fn clean_file(path: &Path, identity: FileIdentity, created: Option<SystemTime>) -> io::Result<Cleaned> {
    let parents = match super::path_parents::HeldParents::new(path) {
        Ok(parents) => parents,
        Err(_) if !parent_reachable(path) => return Ok(Cleaned::Unreachable),
        Err(error) => return Err(error),
    };
    let resolved = parents.resolved_path(path)?;
    let file = match platform().open_copied_file(parents.directory(), &resolved) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Cleaned::Gone),
        Err(error) => return Err(error),
    };
    if platform().file_identity(&file)? != identity {
        return Err(io::Error::other("Partial file was replaced"));
    }
    let created = created.ok_or_else(|| io::Error::other("No creation date proves the partial file"))?;
    if file.metadata()?.created().ok() != Some(created) {
        return Err(io::Error::other("Partial file was replaced"));
    }
    platform().remove_copied_file(&file, &resolved)?;
    Ok(Cleaned::Removed)
}

/// Removes the noted staging folder if it is still the one created.
fn clean_folder(path: &Path, directory_identity: FileIdentity, pin_identity: FileIdentity) -> io::Result<Cleaned> {
    let named = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(PREFIX));
    if !named {
        return Err(io::Error::other("Not a staging folder"));
    }
    let directory = match platform().open_copy_staging(path) {
        Ok(directory) => directory,
        Err(_) if !parent_reachable(path) => return Ok(Cleaned::Unreachable),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Cleaned::Gone),
        Err(error) => return Err(error),
    };
    if platform().file_identity(&directory)? != directory_identity {
        return Err(io::Error::other("Staging folder was replaced"));
    }
    let marker = path.join(".pin");
    let pin = platform().open_copied_file(&directory, &marker)?;
    if platform().file_identity(&pin)? != pin_identity {
        return Err(io::Error::other("Staging marker was replaced"));
    }
    let output = path.join("output");
    match platform().open_copied_file(&directory, &output) {
        Ok(file) => platform().remove_copied_file(&file, &output)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    platform().remove_copy_staging(&directory, pin, path)?;
    Ok(Cleaned::Removed)
}
