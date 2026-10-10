use super::{FileOpType, FileOperation, FileOperationManager, OpStatus};
#[cfg(all(target_os = "windows", not(test)))]
use crate::utils::archive_limits::ArchiveBudget;
use crate::utils::archive_limits::ArchiveLimits;
use std::collections::HashMap;
#[cfg(any(not(target_os = "windows"), test))]
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

#[cfg(any(not(target_os = "windows"), test))]
struct ArchiveReader {
    _parents: crate::systems::path_parents::HeldParents,
    input: std::fs::File,
    operation: Arc<Mutex<FileOperation>>,
}
#[cfg(any(not(target_os = "windows"), test))]
impl Read for ArchiveReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        FileOperationManager::archive_checkpoint(&self.operation).map_err(std::io::Error::other)?;
        let count = self.input.read(buffer)?;
        FileOperationManager::archive_progress(&self.operation, count as u64);
        Ok(count)
    }
}

impl FileOperationManager {
    #[doc(hidden)]
    pub fn perform_compress(
        op: &Arc<Mutex<FileOperation>>,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
    ) -> Result<(), String> {
        crate::systems::archive_recovery::scope(|| {
            Self::perform_compress_prepared(op, sources, destination, &mut || Ok(()))?;
            if let Some(destination) = &op.lock().unwrap().destination { crate::systems::archive_recovery::published(destination); }
            Ok(())
        })
    }

    pub(super) fn perform_compress_prepared(
        op: &Arc<Mutex<FileOperation>>,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
        after_plan: &mut dyn FnMut() -> std::io::Result<()>,
    ) -> Result<(), String> {
        Self::perform_compress_impl(op, sources, destination, after_plan, None, None, None, None, None)
    }
    #[cfg(any(not(target_os = "windows"), test))]
    pub(crate) fn perform_compress_retained(
        op: &Arc<Mutex<FileOperation>>,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
        retained: &HashMap<PathBuf, std::fs::File>,
    ) -> Result<(), String> {
        Self::perform_compress_impl(op, sources, destination, &mut || Ok(()), Some(retained), None, None, None, None)
    }
    #[cfg(all(target_os = "windows", not(test)))]
    pub(crate) fn perform_compress_workspace(op: &Arc<Mutex<FileOperation>>, sources: Vec<PathBuf>, destination: PathBuf, retained: &HashMap<PathBuf, std::fs::File>, expected: &HashMap<PathBuf, crate::platform::FileIdentity>, rename: Option<(&Path, &Path)>) -> Result<std::fs::File, String> {
        let mut completed = None;
        Self::perform_compress_impl(op, sources, Some(destination), &mut || Ok(()), Some(retained), None, Some(expected), Some(&mut completed), rename)?;
        completed.ok_or_else(|| "Archive output ownership unavailable".into())
    }
    #[doc(hidden)]
    pub fn perform_compress_with_limits_for_tests(op: &Arc<Mutex<FileOperation>>, sources: Vec<PathBuf>, destination: Option<PathBuf>, limits: ArchiveLimits) -> Result<(), String> {
        crate::systems::archive_recovery::scope(|| Self::perform_compress_impl(op, sources, destination, &mut || Ok(()), None, Some(limits), None, None, None))
    }
    fn perform_compress_impl(
        op: &Arc<Mutex<FileOperation>>,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
        after_plan: &mut dyn FnMut() -> std::io::Result<()>,
        retained: Option<&HashMap<PathBuf, std::fs::File>>,
        _limits: Option<ArchiveLimits>,
        _expected: Option<&HashMap<PathBuf, crate::platform::FileIdentity>>,
        completed: Option<&mut Option<std::fs::File>>,
        _rename: Option<(&Path, &Path)>,
    ) -> Result<(), String> {
        let requested = destination.ok_or("Missing archive destination")?;
        let update_destination = matches!(op.lock().unwrap().op_type, FileOpType::Compress);
        let destination_parents =
            crate::systems::path_parents::HeldParents::new(&requested).map_err(|error| error.to_string())?;
        let target = Self::unique_archive_path(
            &destination_parents
                .resolved_path(&requested)
                .map_err(|error| error.to_string())?,
        );
        #[cfg(all(target_os = "windows", not(test)))]
        let budget = ArchiveBudget::new(_limits.unwrap_or(if retained.is_some() { ArchiveLimits::WORKSPACE } else { ArchiveLimits::EXTRACTION }));
        #[cfg(all(target_os = "windows", not(test)))]
        let mut entries = crate::systems::compression_sources::plan_controlled(&sources, &mut || Self::archive_checkpoint(op).map_err(std::io::Error::other), Some(&budget)).map_err(|e| crate::systems::archive_encode::source_failure(e).to_string())?;
        #[cfg(all(target_os = "windows", not(test)))]
        if let Some((source, destination)) = _rename { crate::systems::compression_sources::remap(&mut entries, source, destination, budget.limits()).map_err(|e| e.to_string())?; }
        #[cfg(any(not(target_os = "windows"), test))]
        let entries = crate::systems::compression_sources::plan(&sources, &mut || {
            Self::archive_checkpoint(op).map_err(std::io::Error::other)
        })
        .map_err(|error| error.to_string())?;
        let total_bytes = entries.iter().map(|entry| entry.source.size()).sum();
        after_plan().map_err(|error| error.to_string())?;
        {
            let mut locked = op.lock().unwrap();
            if update_destination {
                locked.destination = Some(target.clone());
            }
            locked.total_files = entries.len();
            locked.total_bytes = total_bytes;
            locked.status = if locked.pause_flag.load(Ordering::Relaxed) {
                OpStatus::Paused
            } else {
                OpStatus::Running
            };
        }
        let mut output = crate::systems::compression_output::Output::new(&destination_parents)
            .map_err(|error| error.to_string())?;
        #[cfg(all(target_os = "windows", not(test)))]
        {
            { let mut op = op.lock().unwrap(); op.processed_files = 0; op.processed_bytes = 0; op.speed_sample_at = Some(std::time::Instant::now()); op.speed_sample_bytes = 0; op.bytes_per_second = 0; }
            crate::systems::archive_encode::run(&requested, &entries, retained, _expected, &mut output.file, &budget, op).map_err(|e| e.to_string())?;
        }
        #[cfg(any(not(target_os = "windows"), test))]
        {
        let file = output.file.try_clone().map_err(|error| error.to_string())?;
        let format = requested
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match format.as_str() {
            "zip" => {
                let mut writer = zip::ZipWriter::new(std::io::BufWriter::new(file));
                let mut buffer = [0u8; 256 * 1024];
                for entry in entries {
                    let path = entry.source.path();
                    let (_source_parents, mut input) =
                        entry.open(retained).map_err(|error| error.to_string())?;
                    let is_dir = entry.source.is_dir();
                    let name = entry.name;
                    Self::archive_checkpoint(op)?;
                    op.lock().unwrap().current_file = Some(path.to_string_lossy().into_owned());
                    let level = if op.lock().unwrap().turbo_flag.load(Ordering::Relaxed) {
                        1
                    } else {
                        match op.lock().unwrap().compression_quality.as_str() {
                            "fast" => 1,
                            "best" => 9,
                            _ => 6,
                        }
                    };
                    let options = zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated)
                        .compression_level(Some(level));
                    if is_dir {
                        writer
                            .add_directory(format!("{name}/"), options)
                            .map_err(|error| error.to_string())?;
                    } else {
                        writer
                            .start_file(name, options)
                            .map_err(|error| error.to_string())?;
                        loop {
                            Self::archive_checkpoint(op)?;
                            let count =
                                input.read(&mut buffer).map_err(|error| error.to_string())?;
                            if count == 0 {
                                break;
                            }
                            writer
                                .write_all(&buffer[..count])
                                .map_err(|error| error.to_string())?;
                            Self::archive_progress(op, count as u64);
                        }
                    }
                    op.lock().unwrap().processed_files += 1;
                }
                writer
                    .finish()
                    .map_err(|error| error.to_string())?
                    .flush()
                    .map_err(|error| error.to_string())?;
            }
            "7z" => {
                let quality = op.lock().unwrap().compression_quality.clone();
                let preset = match quality.as_str() {
                    "fast" => 1,
                    "best" => 9,
                    _ => 6,
                };
                let mut writer =
                    sevenz_rust2::ArchiveWriter::new(file).map_err(|e| e.to_string())?;
                writer.set_content_methods(vec![
                    sevenz_rust2::encoder_options::Lzma2Options::from_level(preset).into(),
                ]);
                for selected in entries {
                    let path = selected.source.path();
                    let (parents, input) =
                        selected.open(retained).map_err(|error| error.to_string())?;
                    let metadata = input.metadata().map_err(|error| error.to_string())?;
                    let is_dir = metadata.is_dir();
                    Self::archive_checkpoint(op)?;
                    op.lock().unwrap().current_file = Some(path.to_string_lossy().into_owned());
                    let entry = crate::systems::compression_sources::seven_entry(selected.name, &metadata);
                    if is_dir {
                        writer
                            .push_archive_entry(entry, None::<ArchiveReader>)
                            .map_err(|e| e.to_string())?;
                    } else {
                        let reader = ArchiveReader {
                            _parents: parents,
                            input,
                            operation: op.clone(),
                        };
                        writer
                            .push_archive_entry(entry, Some(reader))
                            .map_err(|e| e.to_string())?;
                    }
                    op.lock().unwrap().processed_files += 1;
                }
                writer.finish().map_err(|e| e.to_string())?;
            }
            "tar" | "zst" => {
                let quality = op.lock().unwrap().compression_quality.clone();
                let level = match quality.as_str() {
                    "fast" => 1,
                    "best" => 19,
                    _ => 3,
                };
                // Zstandard wraps the TAR stream, preserving directories and multiple selections.
                if format == "zst" {
                    Self::write_tar_entries(
                        op,
                        entries,
                        retained,
                        zstd::stream::write::Encoder::new(file, level)
                            .map_err(|e| e.to_string())?,
                    )?;
                } else {
                    let mut tar_bytes = tar::Builder::new(std::io::BufWriter::new(file));
                    Self::append_tar_entries(op, entries, &mut tar_bytes, retained)?;
                    tar_bytes
                        .into_inner()
                        .map_err(|e| e.to_string())?
                        .flush()
                        .map_err(|e| e.to_string())?;
                }
            }
            _ => return Err("Unsupported compression format".into()),
        }
        }
        Self::archive_checkpoint(op)?;
        output.file.sync_all().map_err(|error| error.to_string())?;
        let mut final_target = target;
        loop {
            match output.publish(&destination_parents, &final_target) {
                Ok(()) => break,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    final_target = Self::unique_archive_path(&final_target);
                }
                Err(error) => return Err(error.to_string()),
            }
        }
        if let Some(completed) = completed { *completed = Some(output.file.try_clone().map_err(|e| e.to_string())?); }
        Self::archive_publication_finished(op);
        if update_destination {
            let parent_unchanged = requested.parent().is_some_and(|parent| {
                std::fs::canonicalize(parent)
                    .is_ok_and(|resolved| resolved == destination_parents.resolved_directory())
            });
            let display_path = if parent_unchanged {
                requested.with_file_name(
                    final_target
                        .file_name()
                        .ok_or("Archive output has no name")?,
                )
            } else {
                final_target
            };
            op.lock().unwrap().destination = Some(display_path);
        }
        Ok(())
    }

    #[cfg(any(not(target_os = "windows"), test))]
    fn append_tar_entries<W: Write>(
        op: &Arc<Mutex<FileOperation>>,
        entries: Vec<crate::systems::compression_sources::Entry>,
        writer: &mut tar::Builder<W>,
        retained: Option<&HashMap<PathBuf, std::fs::File>>,
    ) -> Result<(), String> {
        for selected in entries {
            let path = selected.source.path();
            let (parents, input) = selected.open(retained).map_err(|error| error.to_string())?;
            let metadata = input.metadata().map_err(|error| error.to_string())?;
            let name = selected.name;
            let is_dir = metadata.is_dir();
            Self::archive_checkpoint(op)?;
            op.lock().unwrap().current_file = Some(path.to_string_lossy().into_owned());
            let mut header = tar::Header::new_gnu();
            header.set_metadata(&metadata);
            if is_dir {
                writer
                    .append_data(&mut header, name, std::io::empty())
                    .map_err(|e| e.to_string())?;
            } else {
                writer
                    .append_data(
                        &mut header,
                        name,
                        ArchiveReader {
                            _parents: parents,
                            input,
                            operation: op.clone(),
                        },
                    )
                    .map_err(|e| e.to_string())?;
            }
            op.lock().unwrap().processed_files += 1;
        }
        writer.finish().map_err(|e| e.to_string())
    }

    #[cfg(any(not(target_os = "windows"), test))]
    fn write_tar_entries<W: Write>(
        op: &Arc<Mutex<FileOperation>>,
        entries: Vec<crate::systems::compression_sources::Entry>,
        retained: Option<&HashMap<PathBuf, std::fs::File>>,
        encoder: zstd::stream::write::Encoder<'_, W>,
    ) -> Result<(), String> {
        let mut writer = tar::Builder::new(encoder);
        Self::append_tar_entries(op, entries, &mut writer, retained)?;
        writer
            .into_inner()
            .map_err(|e| e.to_string())?
            .finish()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
