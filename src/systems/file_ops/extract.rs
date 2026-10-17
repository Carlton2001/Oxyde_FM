use super::{FileOperation, FileOperationManager, OpStatus};
use crate::platform::platform;
use crate::utils::archive_limits::{ArchiveBudget, ArchiveLimits};
#[cfg(any(not(target_os = "windows"), test))]
use std::io::{Read, Write};
use std::path::PathBuf;
#[cfg(any(not(target_os = "windows"), test))]
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

impl FileOperationManager {
    #[doc(hidden)]
    pub fn perform_extract(
        op: &Arc<Mutex<FileOperation>>,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
    ) -> Result<(), String> {
        crate::systems::archive_recovery::scope(|| {
            Self::perform_extract_with_limits(op, sources, destination, ArchiveLimits::EXTRACTION)?;
            Self::archive_publication_finished(op); Ok(())
        })
    }

    pub(super) fn perform_extract_with_limits(
        op: &Arc<Mutex<FileOperation>>,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
        limits: ArchiveLimits,
    ) -> Result<(), String> {
        Self::perform_extract_prepared(op, sources, destination, limits, &mut || Ok(()))
    }

    pub(super) fn perform_extract_prepared(
        op: &Arc<Mutex<FileOperation>>,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
        limits: ArchiveLimits,
        before_extract: &mut dyn FnMut() -> std::io::Result<()>,
    ) -> Result<(), String> {
        let budget = ArchiveBudget::new(limits);
        let checkpoint = || Self::archive_checkpoint(op).map_err(std::io::Error::other);
        let source = sources.first().ok_or("Missing archive")?;
        let requested = destination.ok_or("Missing extraction destination")?;
        budget.checkpoint(&checkpoint).map_err(|error| error.to_string())?;
        let parents = crate::systems::path_parents::HeldParents::new(source).map_err(|error| error.to_string())?;
        let source = parents.resolved_path(source).map_err(|error| error.to_string())?;
        // Keep the selected object stable across preparation, provenance reads,
        // pause/resume and the complete traversal. No later reopen by pathname.
        let file = platform().open_archive_source(&source, false).map_err(|error| error.to_string())?;
        before_extract().map_err(|error| error.to_string())?;
        #[cfg(all(target_os = "windows", not(test)))]
        {
            let target_parents = crate::systems::path_parents::HeldParents::new(&requested).map_err(|e| e.to_string())?;
            let mut target = Self::unique_archive_path(&target_parents.resolved_path(&requested).map_err(|e| e.to_string())?);
            let tracking = loop {
                match platform().create_journal_directory(target_parents.directory(), &target, false) {
                    Ok(file) => break file,
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => target = Self::unique_archive_path(&target),
                    Err(error) => return Err(error.to_string()),
                }
            };
            let mut tree = match crate::systems::archive_owned_tree::OwnedTree::created(&target, &tracking) {
                Ok(tree) => tree,
                Err(error) => { let _ = platform().remove_empty_operation_directory(target_parents.directory(), &target, platform().file_identity(&tracking).map_err(|e| e.to_string())?); return Err(error.to_string()); }
            };
            { let mut op = op.lock().unwrap(); op.destination = Some(target); op.total_files = 0; op.total_bytes = 0; op.status = OpStatus::Running; }
            crate::systems::archive_io::decode(&source, file, &mut tree, None, &budget, Some(op)).map_err(|e| e.to_string())?;
            tree.keep();
            return Ok(());
        }
        #[cfg(any(not(target_os = "windows"), test))]
        {
        let mut target = Self::unique_archive_path(&requested);
        loop {
            match std::fs::create_dir(&target) {
                Ok(()) => break,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    target = Self::unique_archive_path(&target);
                }
                Err(error) => return Err(error.to_string()),
            }
        }
        struct PendingDirectory(PathBuf);
        impl Drop for PendingDirectory {
            fn drop(&mut self) {
                if !self.0.as_os_str().is_empty() {
                    let _ = std::fs::remove_dir_all(&self.0);
                }
            }
        }
        let mut guard = PendingDirectory(target.clone());
        let extraction_root = crate::utils::archive_security::ExtractionRoot::new(&target)
            .map_err(|error| error.to_string())?;
        let provenance = platform()
            .read_file_provenance(&source)
            .map_err(|error| error.to_string())?;
        {
            let mut locked = op.lock().unwrap();
            locked.destination = Some(target.clone());
            locked.status = if locked.pause_flag.load(Ordering::Relaxed) {
                OpStatus::Paused
            } else {
                OpStatus::Running
            };
        }
        if source
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
        {
            let mut archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
            budget
                .check_entry_count(archive.len())
                .map_err(|error| error.to_string())?;
            let mut total_bytes = 0u64;
            for index in 0..archive.len() {
                budget
                    .checkpoint(&checkpoint)
                    .map_err(|error| error.to_string())?;
                let entry = archive.by_index_raw(index).map_err(|error| error.to_string())?;
                if entry.unix_mode().is_some_and(|mode| mode & 0o170000 == 0o120000) {
                    return Err("Archive contains a symbolic link".into());
                }
                let name = entry.name().map_err(|error| error.to_string())?;
                budget
                    .entry(&name, if entry.is_dir() { 0 } else { entry.size() })
                    .map_err(|error| error.to_string())?;
                total_bytes = total_bytes
                    .checked_add(if entry.is_dir() { 0 } else { entry.size() })
                    .ok_or("Archive size overflow")?;
            }
            {
                let mut locked = op.lock().unwrap();
                locked.total_files = archive.len();
                locked.total_bytes = total_bytes;
            }
            let mut buffer = [0u8; 256 * 1024];
            for index in 0..archive.len() {
                budget
                    .checkpoint(&checkpoint)
                    .map_err(|error| error.to_string())?;
                let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
                let name = entry.name().map_err(|error| error.to_string())?;
                let destination = extraction_root
                    .destination(&name, entry.is_dir())
                    .map_err(|error| error.to_string())?;
                let output = &destination.path;
                op.lock().unwrap().current_file = Some(output.to_string_lossy().into_owned());
                if !entry.is_dir() {
                    let mut copied = 0u64;
                    let mut file = platform()
                        .create_archive_file(output)
                        .map_err(|error| error.to_string())?;
                    loop {
                        budget
                            .checkpoint(&checkpoint)
                            .map_err(|error| error.to_string())?;
                        let count = entry.read(&mut buffer).map_err(|error| error.to_string())?;
                        budget.check_time().map_err(|error| error.to_string())?;
                        if count == 0 {
                            break;
                        }
                        budget
                            .charge_written(count)
                            .map_err(|error| error.to_string())?;
                        file.write_all(&buffer[..count])
                            .map_err(|error| error.to_string())?;
                        copied += count as u64;
                        Self::archive_progress(op, count as u64);
                    }
                    if copied != entry.size() {
                        return Err("Truncated ZIP member".into());
                    }
                    if let Some(data) = provenance.as_deref() {
                        platform()
                            .write_file_provenance(output, data)
                            .map_err(|error| error.to_string())?;
                    }
                }
                op.lock().unwrap().processed_files += 1;
            }
        } else {
            Self::archive_checkpoint(op)?;
            crate::commands::archive::extract_open_archive_with_budget(
                &source,
                file,
                &target.to_string_lossy(),
                &checkpoint,
                &budget,
            )
            .map_err(|error| error.to_string())?;
            let mut locked = op.lock().unwrap();
            locked.total_files = 1;
            locked.processed_files = 1;
        }
        guard.0 = PathBuf::new();
        Ok(())
        }
    }
}
