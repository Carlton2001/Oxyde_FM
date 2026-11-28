//! A file Oxyde writes, from its exclusive creation until it is kept: under its
//! final name, or beside the file it replaces under a temporary name. Not kept, it
//! is emptied through the creation handle, then removed if it is still the one
//! created; a removal the system refuses is reported.
use super::copy_diagnostics::{Phase, Timer};
use super::path_parents::HeldParents;
use crate::models::transaction::HistoryFileIdentity;
use crate::platform::platform;
use std::cell::{Cell, RefCell};
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

/// Longest file name, in UTF-16 units on Windows and bytes elsewhere.
const NAME_LIMIT: usize = 255;
/// Attempts at a free temporary name beside a replaced file.
pub(super) const TEMPORARY_ATTEMPTS: usize = 4;

pub(super) struct Output {
    directory: File,
    path: PathBuf,
    destination: PathBuf,
    /// The creation handle, until verification: emptying a partial file needs no
    /// right beyond the one creation granted.
    writer: RefCell<Option<File>>,
    identity: Cell<Option<HistoryFileIdentity>>,
    journal: RefCell<Option<super::staging_journal::Entry>>,
    misplaced: Cell<bool>,
    kept: Cell<bool>,
}

impl Output {
    /// `destination` is the resolved final path in `parents`. A replacement writes
    /// beside it, a new file under its name.
    pub fn new(parents: &HeldParents, destination: &Path, replacing: bool) -> io::Result<Self> {
        let _timer = Timer::start(Phase::OutputPrepare);
        // Cheap refusal of a folder turned reparse point since it was held; the
        // creation itself is checked by `capture`.
        let metadata = parents.directory().metadata()?;
        if !metadata.is_dir() || platform().file_attributes(&metadata, "").2 {
            return Err(io::Error::other("Copy destination folder changed during the operation"));
        }
        let path = if replacing {
            let name = destination
                .file_name()
                .ok_or_else(|| io::Error::other("Copy destination has no name"))?;
            destination.with_file_name(temporary_name(name, &random_suffix()))
        } else {
            destination.to_path_buf()
        };
        Ok(Self {
            directory: parents.directory().try_clone()?,
            path,
            destination: destination.to_path_buf(),
            writer: RefCell::new(None),
            identity: Cell::new(None),
            journal: RefCell::new(None),
            misplaced: Cell::new(false),
            kept: Cell::new(false),
        })
    }

    /// Where the file is written.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether this output created a file: a refused creation leaves nothing to clean.
    pub fn captured(&self) -> bool {
        self.writer.borrow().is_some() || self.identity.get().is_some()
    }

    fn replacing(&self) -> bool {
        self.path != self.destination
    }

    /// Takes ownership of the file just created, before its first byte.
    pub fn capture(&self, file: &File) -> io::Result<()> {
        if self.captured() {
            return Ok(());
        }
        *self.writer.borrow_mut() = Some(file.try_clone()?);
        let identity = HistoryFileIdentity::from_file(file)?;
        self.identity.set(Some(identity));
        if let Err(error) = platform().verify_created_child(&self.directory, file, &self.path) {
            self.misplaced.set(true);
            return Err(error);
        }
        let created = file.metadata()?.created().ok();
        *self.journal.borrow_mut() =
            super::staging_journal::record_file(&self.path, platform().file_identity(file)?, created);
        Ok(())
    }

    /// Closes the creation handle once its final identity is known: native copies
    /// apply the source's dates after their last callback.
    fn release_writer(&self) -> io::Result<Option<HistoryFileIdentity>> {
        if let Some(writer) = self.writer.borrow_mut().take() {
            let identity = HistoryFileIdentity::from_file(&writer);
            // The writer's access would conflict with the verification handle.
            drop(writer);
            if let Ok(identity) = identity {
                self.identity.set(Some(identity));
            }
            identity?;
        }
        Ok(self.identity.get())
    }

    /// The output reopened for verification, still the object created. A new file
    /// asks only to read it; a replacement also needs to rename it.
    pub fn open_verified(&self) -> io::Result<File> {
        let release_timer = Timer::start(Phase::WriterRelease);
        let expected = self
            .release_writer()?
            .ok_or_else(|| io::Error::other("Copy output ownership unavailable"))?;
        drop(release_timer);
        let _open_timer = Timer::start(Phase::OutputOpen);
        let file = with_sharing_retries(|| {
            if self.replacing() {
                platform().open_copied_file(&self.directory, &self.path)
            } else {
                platform().open_copied_output(&self.directory, &self.path)
            }
        })?;
        if !expected.matches(&file)? {
            return Err(super::file_copy::identity_changed(&self.path));
        }
        // The note proves the file by the date it now has: a native copy applied the
        // source's after the note was written, the WebDAV redirector only on close.
        if let Some(journal) = self.journal.borrow_mut().as_mut() {
            journal.update_created(file.metadata()?.created().ok());
        }
        Ok(file)
    }

    /// The creation handle itself, for verification by position, where the platform
    /// allows it (see `Platform::copy_writer_verifies`): it is the object created and
    /// keeps every other opener out until kept. None otherwise, and on a volume that
    /// identifies files by path (WebDAV), which applies the copied dates on close.
    pub fn verified_writer(&self) -> io::Result<Option<File>> {
        let _timer = Timer::start(Phase::OutputOpen);
        let writer = self.writer.borrow();
        let Some(writer) = writer.as_ref().filter(|writer| platform().copy_writer_verifies(writer)) else {
            return Ok(None);
        };
        // Taken again: a native copy applied the source's dates after its last callback.
        let identity = HistoryFileIdentity::from_file(writer)?;
        if identity.follows_path() {
            return Ok(None);
        }
        self.identity.set(Some(identity));
        if let Some(journal) = self.journal.borrow_mut().as_mut() {
            journal.update_created(writer.metadata()?.created().ok());
        }
        writer.try_clone().map(Some)
    }

    /// Keeps the verified output: a replacement takes the place of its target.
    pub fn keep(&self, parents: &HeldParents, file: &File, replace: bool) -> io::Result<()> {
        if self.replacing() {
            parents.rename(file, &self.path, &self.destination, replace)?;
        }
        self.kept.set(true);
        drop(self.journal.borrow_mut().take());
        Ok(())
    }

    fn clean(&self) -> io::Result<()> {
        let writer = self.writer.borrow_mut().take();
        if let Some(writer) = &writer {
            // A partial file the system refuses to remove must not pass for a copy.
            if let Err(error) = writer.set_len(0) {
                log::debug!("Cannot empty copy output {}: {error}", self.path.display());
            }
        }
        // Windows: removed through the creation handle, wherever it lies, when that
        // handle carries delete access. A pending native deletion also ends here.
        #[cfg(target_os = "windows")]
        if let Some(writer) = &writer {
            if platform().remove_copied_file(writer, &self.path).is_ok() {
                return Ok(());
            }
        }
        let identity = match self.release_identity(writer) {
            Some(identity) => identity,
            None => return Ok(()),
        };
        if self.misplaced.get() {
            return Err(io::Error::other("Copy output created outside its folder was kept"));
        }
        let file = match platform().open_copied_file(&self.directory, &self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        if !identity.matches(&file)? {
            return Err(super::file_copy::identity_changed(&self.path));
        }
        platform().remove_copied_file(&file, &self.path)
    }

    /// The identity to clean by, once the writer is closed; none when nothing was created.
    fn release_identity(&self, writer: Option<File>) -> Option<HistoryFileIdentity> {
        if let Some(writer) = writer {
            if let Ok(identity) = HistoryFileIdentity::from_file(&writer) {
                self.identity.set(Some(identity));
            }
        }
        self.identity.get()
    }

    /// Where a kept partial file is reported: the creation handle knows where it lies.
    fn retained_path(&self) -> PathBuf {
        self.writer
            .borrow()
            .as_ref()
            .and_then(|writer| platform().retained_file_path(writer, &self.path).ok())
            .unwrap_or_else(|| self.path.clone())
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        if self.kept.get() || !self.captured() {
            return;
        }
        let _timer = Timer::start(Phase::Cleanup);
        let retained = self.retained_path();
        if let Err(error) = self.clean() {
            log::warn!("Copy output kept at {}: {error}", retained.display());
            super::archive_recovery::retained(&retained);
        }
        // The note goes with the attempt: what is left was reported.
        drop(self.journal.borrow_mut().take());
    }
}

/// Indexers, antivirus scanners and sync clients may hold a new file a moment.
fn with_sharing_retries(mut open: impl FnMut() -> io::Result<File>) -> io::Result<File> {
    const SHARING_VIOLATION: i32 = 32;
    let mut attempts = 0;
    loop {
        match open() {
            Err(error) if cfg!(target_os = "windows") && error.raw_os_error() == Some(SHARING_VIOLATION) && attempts < 5 => {
                attempts += 1;
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            result => return result,
        }
    }
}

fn random_suffix() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..6].to_string()
}

/// `.<name>.oxyde-<suffix>`, the name shortened to fit the file system's limit.
pub(super) fn temporary_name(name: &OsStr, suffix: &str) -> OsString {
    let tail = format!(".oxyde-{suffix}");
    let budget = NAME_LIMIT.saturating_sub(1 + tail.len());
    let mut result = OsString::from(".");
    result.push(shortened(name, budget));
    result.push(tail);
    result
}

#[cfg(target_os = "windows")]
fn shortened(name: &OsStr, budget: usize) -> OsString {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    let mut units: Vec<u16> = name.encode_wide().collect();
    if units.len() > budget {
        units.truncate(budget);
        // Never end on the first half of a surrogate pair.
        if units.last().is_some_and(|unit| (0xD800..0xDC00).contains(unit)) {
            units.pop();
        }
    }
    OsString::from_wide(&units)
}

#[cfg(not(target_os = "windows"))]
fn shortened(name: &OsStr, budget: usize) -> OsString {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    let bytes = name.as_bytes();
    if bytes.len() <= budget {
        return name.to_os_string();
    }
    let mut end = budget;
    // A UTF-8 name keeps whole characters; other bytes are cut where they fall.
    if std::str::from_utf8(bytes).is_ok() {
        while end > 0 && (bytes[end] & 0xC0) == 0x80 {
            end -= 1;
        }
    }
    OsString::from_vec(bytes[..end].to_vec())
}

#[cfg(test)]
#[path = "copy_output/tests/naming.rs"]
mod tests;

#[cfg(all(test, target_os = "linux"))]
#[path = "copy_output/tests/cleanup.rs"]
mod cleanup_tests;
