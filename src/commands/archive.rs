pub(crate) use super::archive_iso_open::extract_iso_file_for_open;
use super::archive_iso_open::{extract_iso, list_iso};
pub(crate) use super::archive_seven_open::extract_seven_file_for_open;
pub(crate) use super::archive_tar_open::extract_tar_file_for_open;
use crate::models::{CommandError, FileEntry};
use crate::platform::platform;
pub use crate::utils::archive::{is_archive, split_virtual_path, ArchiveFormat};
use crate::utils::archive_limits::{ArchiveBudget, ArchiveLimits, BudgetReader};
use crate::utils::archive_security::{relative_archive_path, ExtractionRoot};
use crate::utils::path_security::validate_path;
use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use log::info;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use tar::Archive as TarArchive;
use xz2::read::XzDecoder;
use zip::ZipArchive;
use zstd::stream::read::Decoder as ZstdDecoder;

pub fn create_archive_directory_with_quality(
    path: &Path,
    quality: &str,
) -> Result<(), std::io::Error> {
    crate::systems::archive_workspace::create_directory(path, quality)
}

pub fn prepare_archive_file_for_open(path: &Path) -> Result<PathBuf, CommandError> {
    crate::systems::archive_workspace::external_file(path)
}

pub fn extract_archive_file_to(path: &Path, folder: &Path) -> Result<PathBuf, CommandError> {
    crate::systems::archive_workspace::export_file(path, folder)
}

/// ZIP supports random access: validate the index, then decode only the requested file.
pub(crate) fn extract_zip_file_for_open(
    archive_path: &Path,
    internal: &str,
    target: &Path,
    limits: ArchiveLimits,
) -> Result<PathBuf, CommandError> {
    extract_zip_file_with_checkpoint(archive_path, internal, target, limits, &|| Ok(()))
}

fn extract_zip_file_with_checkpoint(
    archive_path: &Path,
    internal: &str,
    target: &Path,
    limits: ArchiveLimits,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<PathBuf, CommandError> {
    let requested = relative_archive_path(internal)?;
    if requested.as_os_str().is_empty() {
        return Err(CommandError::ArchiveError(
            "Archive item is not a file".into(),
        ));
    }
    let parents = crate::systems::path_parents::HeldParents::new(archive_path)?;
    let archive_path = parents.resolved_path(archive_path)?;
    let budget = ArchiveBudget::new(limits);
    let mut archive = ZipArchive::new(platform().open_archive_source(&archive_path, false)?)
        .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    budget.check_entry_count(archive.len())?;
    let mut selected = None;
    for index in 0..archive.len() {
        budget.checkpoint(checkpoint)?;
        let entry = archive
            .by_index_raw(index)
            .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        budget.entry(entry.name(), 0)?;
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(CommandError::ArchiveError(
                "Archive links are not supported".into(),
            ));
        }
        if relative_archive_path(entry.name())? == requested {
            if selected.is_some() || entry.is_dir() {
                return Err(CommandError::ArchiveError("Ambiguous archive file".into()));
            }
            budget.declare_bytes(entry.size())?;
            selected = Some(index);
        }
    }
    let index =
        selected.ok_or_else(|| CommandError::ArchiveError("Archive file not found".into()))?;
    budget.checkpoint(checkpoint)?;
    let provenance = platform().read_file_provenance(&archive_path)?;
    let root = ExtractionRoot::new(target)?;
    let destination = root.destination(internal, false)?;
    let mut output = platform().create_archive_file(&destination.path)?;
    let mut entry = archive
        .by_index(index)
        .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    let size = entry.size();
    if budget.copy(&mut entry, &mut output, checkpoint)? != size {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Truncated ZIP member").into());
    }
    if let Some(data) = provenance.as_deref() {
        platform().write_file_provenance(&destination.path, data)?;
    }
    Ok(destination.path.clone())
}

pub fn list_archive_contents(
    archive_path: String,
    internal_path: String,
) -> Result<Vec<FileEntry>, CommandError> {
    // Library unit fixtures exercise parser/operations locally: their executable
    // is Rust's test harness, not the headless app. Integration tests launch the
    // actual binary with the production Windows supervisor and protocol.
    #[cfg(test)]
    { list_archive_contents_local(archive_path, internal_path) }
    #[cfg(not(test))]
    {
        let path = validate_path(&archive_path)?;
        relative_archive_path(&internal_path)?;
        platform().list_archive_directory(&path, &internal_path)
    }
}

/// Parser entry used inside the headless worker, and directly on Linux.
pub(crate) fn list_archive_contents_local(
    archive_path: String,
    internal_path: String,
) -> Result<Vec<FileEntry>, CommandError> {
    let path_buf = validate_path(&archive_path)?;
    let path = path_buf.as_path();
    relative_archive_path(&internal_path)?;
    let format = ArchiveFormat::from_path(path).ok_or(CommandError::ArchiveError(
        "Unsupported archive format".to_string(),
    ))?;

    match format {
        ArchiveFormat::SevenZip => list_seven_zip(path, &internal_path),
        ArchiveFormat::Zip
        | ArchiveFormat::Tar
        | ArchiveFormat::TarGz
        | ArchiveFormat::TarXz
        | ArchiveFormat::TarZst
        | ArchiveFormat::TarBz2 => list_indexed_metadata(path, &internal_path),
        ArchiveFormat::Iso => list_iso(path, &internal_path),
        ArchiveFormat::Rar => Err(CommandError::ArchiveError(
            "Rar navigation not supported yet. Please extract it first.".to_string(),
        )),
    }
}

fn virtual_entry_path(archive: &Path, internal: &str, name: &str) -> Result<String, CommandError> {
    let mut path = archive.to_path_buf();
    path.push(relative_archive_path(internal)?);
    path.push(relative_archive_path(name)?);
    Ok(path.to_string_lossy().into_owned())
}

/// Complete index for search; production Windows parsing is delegated through
/// Platform. Unit fixtures use the local parser instead of the Rust test runner.
pub(crate) fn index_archive_contents(path: &Path, checkpoint: &dyn Fn() -> io::Result<()>) -> Result<Vec<FileEntry>, CommandError> {
    #[cfg(test)]
    { index_archive_contents_local(path, checkpoint) }
    #[cfg(not(test))]
    { platform().index_archive_contents(path, checkpoint) }
}

pub(crate) fn index_archive_contents_local(path: &Path, checkpoint: &dyn Fn() -> io::Result<()>) -> Result<Vec<FileEntry>, CommandError> {
    match ArchiveFormat::from_path(path) {
        Some(ArchiveFormat::Zip | ArchiveFormat::Tar | ArchiveFormat::TarGz | ArchiveFormat::TarXz | ArchiveFormat::TarZst | ArchiveFormat::TarBz2) => super::archive_metadata::indexed_files(path, checkpoint),
        Some(ArchiveFormat::SevenZip) => super::archive_seven_open::indexed_files(path, checkpoint),
        Some(ArchiveFormat::Iso) => super::archive_iso_open::indexed_files(path, checkpoint),
        _ => Err(CommandError::ArchiveError("Unsupported archive search format".into())),
    }
}

fn list_indexed_metadata(
    path: &Path,
    internal_prefix: &str,
) -> Result<Vec<FileEntry>, CommandError> {
    let relative_prefix = relative_archive_path(internal_prefix)?;
    let mut seen = HashSet::new();
    let mut files = Vec::new();
    for mut entry in super::archive_metadata::indexed_files(path, &|| Ok(()))? {
        let relative = Path::new(&entry.path)
            .strip_prefix(path)
            .map_err(|_| CommandError::ArchiveError("Invalid archive entry path".into()))?;
        let Ok(relative) = relative.strip_prefix(&relative_prefix) else {
            continue;
        };
        let mut parts = relative.components();
        let Some(first) = parts.next() else {
            continue;
        };
        let name = first.as_os_str().to_string_lossy().into_owned();
        if !seen.insert(name.clone()) {
            continue;
        }
        entry.is_dir |= parts.next().is_some();
        if entry.is_dir {
            entry.size = 0;
        }
        entry.path = path
            .join(&relative_prefix)
            .join(&name)
            .to_string_lossy()
            .into_owned();
        entry.name = name;
        files.push(entry);
    }
    Ok(files)
}

fn list_seven_zip(path: &Path, internal_prefix: &str) -> Result<Vec<FileEntry>, CommandError> {
    let mut entries = Vec::new();
    let normalized = relative_archive_path(internal_prefix)?
        .to_string_lossy()
        .replace('\\', "/");
    let prefix = if normalized.is_empty() {
        "".to_string()
    } else {
        format!("{normalized}/")
    };
    let mut seen: HashSet<String> = HashSet::new();

    // Listing needs only the 7z header; do not decompress every file while navigating.
    for entry in super::archive_seven_open::indexed_files(path, &|| Ok(()))? {
        let name = Path::new(&entry.path)
            .strip_prefix(path)
            .map_err(|_| CommandError::ArchiveError("Invalid archive entry path".into()))?
            .to_string_lossy()
            .replace('\\', "/");
        if name.starts_with(&prefix) && name != prefix {
            let relative = &name[prefix.len()..];
            let parts: Vec<&str> = relative
                .split('/')
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>();
            if !parts.is_empty() {
                let entry_name = parts[0].to_string();
                if !seen.contains(&entry_name) {
                    seen.insert(entry_name.clone());
                    let is_dir = entry.is_dir || parts.len() > 1;
                    entries.push(FileEntry {
                        name: entry_name.clone(),
                        path: virtual_entry_path(path, internal_prefix, &entry_name)?,
                        is_dir,
                        is_hidden: false,
                        is_system: false,
                        is_symlink: false,
                        is_junction: false,
                        size: if is_dir { 0 } else { entry.size },
                        is_calculated: false,
                        folders_count: None,
                        files_count: None,
                        modified: 0,
                        original_path: None,
                        network_details: None,
                        deleted_time: None,
                    });
                }
            }
        }
    }

    Ok(entries)
}

#[cfg(test)]
pub(crate) fn extract_archive_with_checkpoint(
    archive_path: String,
    target_dir: String,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<(), CommandError> {
    extract_archive_with_limits(
        archive_path,
        target_dir,
        checkpoint,
        ArchiveLimits::EXTRACTION,
    )
}

#[cfg(test)]
pub(crate) fn extract_archive_with_limits(
    archive_path: String,
    target_dir: String,
    checkpoint: &dyn Fn() -> io::Result<()>,
    limits: ArchiveLimits,
) -> Result<(), CommandError> {
    let budget = ArchiveBudget::new(limits);
    budget.checkpoint(checkpoint)?;
    let archive = validate_path(&archive_path)?;
    let parents = crate::systems::path_parents::HeldParents::new(&archive)?;
    let archive = parents.resolved_path(&archive)?;
    let file = platform().open_archive_source(&archive, false)?;
    extract_open_archive_with_budget(&archive, file, &target_dir, checkpoint, &budget)
}

/// Decode the opened source itself, without resolving its pathname again.
pub(crate) fn extract_open_archive_with_budget(
    path: &Path,
    file: File,
    target_dir: &str,
    checkpoint: &dyn Fn() -> io::Result<()>,
    budget: &ArchiveBudget,
) -> Result<(), CommandError> {
    budget.checkpoint(checkpoint)?;
    let format = ArchiveFormat::from_path(path).ok_or(CommandError::ArchiveError(
        "Unsupported archive format".to_string(),
    ))?;
    let target_buf = validate_path(target_dir)?;
    let target = target_buf.as_path();

    info!("Extracting {:?} to {:?}", path, target);

    if !target.exists() {
        fs::create_dir_all(target).map_err(|e| CommandError::IoError(e.to_string()))?;
    }
    let root = ExtractionRoot::new(target)?;
    let provenance = platform().read_file_provenance(path)?;

    match format {
        ArchiveFormat::Zip => {
            let mut archive =
                ZipArchive::new(file).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
            budget.check_entry_count(archive.len())?;
            for index in 0..archive.len() {
                budget.checkpoint(checkpoint)?;
                let entry = archive
                    .by_index_raw(index)
                    .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
                if entry
                    .unix_mode()
                    .is_some_and(|mode| mode & 0o170000 == 0o120000)
                {
                    return Err(CommandError::ArchiveError(
                        "Archive links are not supported".into(),
                    ));
                }
                budget.entry(entry.name(), if entry.is_dir() { 0 } else { entry.size() })?;
            }
            for index in 0..archive.len() {
                budget.checkpoint(checkpoint)?;
                let mut entry = archive
                    .by_index(index)
                    .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
                let destination = root.destination(entry.name(), entry.is_dir())?;
                if !entry.is_dir() {
                    let mut output = platform().create_archive_file(&destination.path)?;
                    let size = entry.size();
                    if budget.copy(&mut entry, &mut output, checkpoint)? != size {
                        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Truncated ZIP member").into());
                    }
                    if let Some(data) = provenance.as_deref() {
                        platform().write_file_provenance(&destination.path, data)?;
                    }
                }
            }
        }
        ArchiveFormat::SevenZip => {
            decompress_seven_zip_with_checkpoint(
                file,
                &root,
                provenance.as_deref(),
                checkpoint,
                budget,
            )?;
        }
        ArchiveFormat::Tar
        | ArchiveFormat::TarGz
        | ArchiveFormat::TarXz
        | ArchiveFormat::TarZst
        | ArchiveFormat::TarBz2 => {
            let reader: Box<dyn io::Read> = match format {
                ArchiveFormat::TarGz => Box::new(GzDecoder::new(file)),
                ArchiveFormat::TarXz => Box::new(XzDecoder::new(file)),
                ArchiveFormat::TarBz2 => Box::new(BzDecoder::new(file)),
                ArchiveFormat::TarZst => Box::new(
                    ZstdDecoder::new(file).map_err(|e| CommandError::IoError(e.to_string()))?,
                ),
                _ => Box::new(file),
            };
            let mut archive = TarArchive::new(BudgetReader {
                reader,
                checkpoint,
                budget,
            });
            for entry in archive.entries()? {
                let mut entry = entry?;
                budget.checkpoint(checkpoint)?;
                let kind = entry.header().entry_type();
                if !kind.is_file() && !kind.is_dir() {
                    return Err(CommandError::ArchiveError(
                        "Archive links and special files are not supported".into(),
                    ));
                }
                let name = entry
                    .path()?
                    .to_str()
                    .ok_or_else(|| {
                        CommandError::ArchiveError("Invalid archive entry encoding".into())
                    })?
                    .to_owned();
                budget.entry(&name, if kind.is_file() { entry.size() } else { 0 })?;
                let destination = root.destination(&name, kind.is_dir())?;
                if kind.is_file() {
                    let mut output = platform().create_archive_file(&destination.path)?;
                    budget.copy(&mut entry, &mut output, checkpoint)?;
                    if let Some(data) = provenance.as_deref() {
                        platform().write_file_provenance(&destination.path, data)?;
                    }
                    if let Ok(seconds) = entry.header().mtime() {
                        if let Some(modified) = std::time::UNIX_EPOCH
                            .checked_add(std::time::Duration::from_secs(seconds))
                        {
                            output.set_times(fs::FileTimes::new().set_modified(modified))?;
                        }
                    }
                }
            }
        }
        ArchiveFormat::Iso => {
            extract_iso(
                file,
                &root,
                provenance.as_deref(),
                checkpoint,
                budget,
            )?;
        }
        ArchiveFormat::Rar => {
            return Err(CommandError::ArchiveError(
                "Rar extraction requires external tools (like 7-Zip or WinRAR).".to_string(),
            ));
        }
    }
    Ok(())
}

fn decompress_seven_zip_with_checkpoint(
    file: File,
    root: &ExtractionRoot,
    provenance: Option<&[u8]>,
    checkpoint: &dyn Fn() -> io::Result<()>,
    budget: &ArchiveBudget,
) -> Result<(), CommandError> {
    let mut seven = super::archive_seven_open::checked_reader(file, budget, checkpoint)?;
    budget.check_entry_count(seven.archive().files.len())?;
    for entry in &seven.archive().files {
        budget.checkpoint(checkpoint)?;
        super::archive_seven_open::validate_entry_type(entry)?;
        budget.entry(
            entry.name(),
            if entry.is_directory() {
                0
            } else {
                entry.size()
            },
        )?;
    }
    seven.set_thread_count(1);
    seven
        .for_each_entries(|entry, reader| {
            budget.checkpoint(checkpoint)?;
            let destination = root.destination(entry.name(), entry.is_directory())?;
            if !entry.is_directory() {
                let mut output = platform().create_archive_file(&destination.path)?;
                budget.copy(reader, &mut output, checkpoint)?;
                if let Some(data) = provenance {
                    platform().write_file_provenance(&destination.path, data)?;
                }
                let mut times = fs::FileTimes::new();
                if entry.has_last_modified_date {
                    times = times.set_modified(entry.last_modified_date().into());
                }
                if entry.has_access_date {
                    times = times.set_accessed(entry.access_date().into());
                }
                output.set_times(times)?;
            }
            Ok(true)
        })
        .map_err(super::archive_seven_open::archive_error)
}

#[cfg(test)]
fn safe_seven_zip_destination(root: &Path, name: &str) -> io::Result<PathBuf> {
    Ok(ExtractionRoot::new(root)?.destination(name, false)?.path)
}

pub(super) fn safe_iso_component(name: &str) -> Result<&str, CommandError> {
    relative_archive_path(name)?;
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.contains(':')
        || name.contains('\0')
    {
        return Err(CommandError::ArchiveError(format!(
            "Unsafe ISO archive entry name: {name}"
        )));
    }
    Ok(name)
}

#[cfg(test)]
mod archive_tests {
    use super::*;
    use crate::systems::archive_workspace::mutation_operation;
    use crate::systems::file_ops::FileOperationManager;

    #[test]
    fn seven_full_extract_preserves_typed_io_limits_from_decoder_callbacks() {
        use crate::models::error::{ArchiveLimitFailure, ArchiveLimitKind};
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("deadline.7z");
        let mut writer = sevenz_rust2::ArchiveWriter::create(&archive).unwrap();
        writer
            .push_archive_entry(
                sevenz_rust2::ArchiveEntry::new_file("safe.txt"),
                Some(&b"safe"[..]),
            )
            .unwrap();
        writer.finish().unwrap();
        let target = tempfile::tempdir().unwrap();
        let expired = ArchiveBudget::new(ArchiveLimits {
            max_duration: std::time::Duration::ZERO,
            ..ArchiveLimits::EXTRACTION
        });
        let error = extract_archive_with_checkpoint(
            archive.to_string_lossy().into_owned(),
            target.path().to_string_lossy().into_owned(),
            &|| {
                if target.path().join("safe.txt").exists() {
                    expired.check_time()
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        let failure = ArchiveLimitFailure::from_message(&error.to_string())
            .expect("7z erased the structured deadline error");
        assert_eq!(failure.kind, ArchiveLimitKind::Seconds);
        assert_eq!(failure.limit, 0);
    }

    #[test]
    fn listings_refuse_paths_exceeding_the_shared_depth_limit() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let name = format!("{}file.txt", "a/".repeat(65));
        for extension in ["zip", "7z", "tar", "zst"] {
            let path = dir.path().join(format!("deep.{extension}"));
            match extension {
                "zip" => {
                    let mut writer = zip::ZipWriter::new(File::create(&path).unwrap());
                    writer
                        .start_file(&name, zip::write::SimpleFileOptions::default())
                        .unwrap();
                    writer.write_all(b"safe").unwrap();
                    writer.finish().unwrap();
                }
                "7z" => {
                    let mut writer = sevenz_rust2::ArchiveWriter::create(&path).unwrap();
                    writer
                        .push_archive_entry(
                            sevenz_rust2::ArchiveEntry::new_file(&name),
                            Some(&b"safe"[..]),
                        )
                        .unwrap();
                    writer.finish().unwrap();
                }
                _ => {
                    let output: Box<dyn Write> = if extension == "zst" {
                        Box::new(
                            zstd::stream::write::Encoder::new(File::create(&path).unwrap(), 1)
                                .unwrap()
                                .auto_finish(),
                        )
                    } else {
                        Box::new(File::create(&path).unwrap())
                    };
                    let mut builder = tar::Builder::new(output);
                    let mut header = tar::Header::new_gnu();
                    header.set_size(4);
                    header.set_mode(0o644);
                    header.set_cksum();
                    builder
                        .append_data(&mut header, &name, &b"safe"[..])
                        .unwrap();
                    builder.finish().unwrap();
                }
            }
            assert!(
                list_archive_contents(path.to_string_lossy().into_owned(), String::new()).is_err(),
                "unbounded listing for {extension}"
            );
        }
    }

    #[test]
    fn targeted_zip_open_does_not_extract_other_files() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("select.zip");
        let mut writer = zip::ZipWriter::new(File::create(&archive).unwrap());
        for (name, data) in [
            ("selected.txt", &b"safe"[..]),
            ("other.txt", &b"unselected and over the limit"[..]),
        ] {
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();
        let output = tempfile::tempdir().unwrap();
        let limits = ArchiveLimits {
            max_bytes: 4,
            ..ArchiveLimits::OPEN_FILE
        };
        let selected =
            extract_zip_file_for_open(&archive, "selected.txt", output.path(), limits).unwrap();
        assert_eq!(fs::read(selected).unwrap(), b"safe");
        assert!(!output.path().join("other.txt").exists());
        let second = tempfile::tempdir().unwrap();
        assert!(extract_zip_file_for_open(&archive, "other.txt", second.path(), limits).is_err());
        assert!(fs::read_dir(second.path()).unwrap().next().is_none());
    }

    #[test]
    fn targeted_zip_preflight_does_not_construct_unselected_decoders() {
        use std::io::Write;
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("metadata-only.zip");
        let mut zip = zip::ZipWriter::new(File::create(&archive).unwrap());
        for name in ["selected.txt", "other.txt"] {
            zip.start_file(name, zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored)).unwrap();
            zip.write_all(b"safe").unwrap();
        }
        zip.finish().unwrap();
        let mut bytes = fs::read(&archive).unwrap();
        // A valid index whose unselected member uses an unsupported codec.
        // Change matching local/central headers only, without decoding data.
        let mut patched = 0;
        for offset in 0..bytes.len().saturating_sub(46) {
            let (name, method) = match &bytes[offset..offset + 4] {
                b"PK\x03\x04" => (offset + 30, offset + 8),
                b"PK\x01\x02" => (offset + 46, offset + 10),
                _ => continue,
            };
            if bytes.get(name..name + 9) == Some(&b"other.txt"[..]) {
                bytes[method..method + 2].copy_from_slice(&200u16.to_le_bytes());
                patched += 1;
            }
        }
        assert_eq!(patched, 2);
        fs::write(&archive, bytes).unwrap();
        let output = tempfile::tempdir().unwrap();
        let selected = extract_zip_file_for_open(&archive, "selected.txt", output.path(), ArchiveLimits::OPEN_FILE).unwrap();
        assert_eq!(fs::read(selected).unwrap(), b"safe");
        let refused = tempfile::tempdir().unwrap();
        assert!(extract_zip_file_for_open(&archive, "other.txt", refused.path(), ArchiveLimits::OPEN_FILE).is_err());
    }

    #[test]
    fn targeted_zip_cancellation_precedes_destination_creation() {
        use std::io::Write;
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("cancel.zip");
        let mut zip = zip::ZipWriter::new(File::create(&archive).unwrap());
        zip.start_file("safe.txt", zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(b"safe").unwrap();
        zip.finish().unwrap();
        let output = tempfile::tempdir().unwrap();
        let error = extract_zip_file_with_checkpoint(&archive, "safe.txt", output.path(), ArchiveLimits::OPEN_FILE, &|| {
            Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled before ZIP output"))
        }).unwrap_err();
        assert!(error.to_string().contains("cancelled before ZIP output"));
        assert_eq!(fs::read_dir(output.path()).unwrap().count(), 0);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn targeted_zip_retains_source_and_provenance_after_parent_retargeting() {
        use crate::platform::windows_reparse_tests::{junction, Retarget};
        use std::io::Write;
        let temp = tempfile::tempdir().unwrap();
        let chosen = temp.path().join("chosen");
        let outside = temp.path().join("outside");
        fs::create_dir(&chosen).unwrap();
        fs::create_dir(&outside).unwrap();
        for (folder, content) in [(&chosen, &b"safe"[..]), (&outside, &b"evil"[..])] {
            let mut zip = zip::ZipWriter::new(File::create(folder.join("source.zip")).unwrap());
            zip.start_file("safe.txt", zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(content).unwrap();
            zip.finish().unwrap();
        }
        let archive = chosen.join("source.zip");
        let provenance = b"[ZoneTransfer]\r\nZoneId=3\r\n";
        platform().write_file_provenance(&archive, provenance).unwrap();
        let link = temp.path().join("link");
        let template = temp.path().join("template");
        junction(&link, &chosen);
        junction(&template, &outside);
        let retarget = Retarget::new(&link, &template);
        let changed = std::cell::Cell::new(false);
        let output = tempfile::tempdir().unwrap();
        let result = extract_zip_file_with_checkpoint(&link.join("source.zip"), "safe.txt", output.path(), ArchiveLimits::OPEN_FILE, &|| {
            if !changed.replace(true) {
                assert!(File::options().write(true).open(&archive).is_err());
                assert!(fs::rename(&archive, chosen.join("moved.zip")).is_err());
                retarget.apply();
            }
            Ok(())
        }).unwrap();
        assert!(changed.get());
        assert_eq!(fs::read(&result).unwrap(), b"safe");
        assert_eq!(platform().read_file_provenance(&result).unwrap().as_deref(), Some(&provenance[..]));
        drop(retarget);
        fs::remove_dir(link).unwrap();
        fs::remove_dir(template).unwrap();
        assert!(File::options().write(true).open(&archive).is_ok());
    }

    #[test]
    fn extraction_limits_apply_to_every_supported_decoder() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("file.txt");
        fs::write(&source, b"bounded test content").unwrap();
        let limits = ArchiveLimits {
            max_bytes: 2,
            ..ArchiveLimits::EXTRACTION
        };
        for extension in ["zip", "7z", "tar", "zst", "iso"] {
            let archive = dir.path().join(format!("bounded.{extension}"));
            if extension == "iso" {
                test_iso_image(&archive);
            } else {
                FileOperationManager::perform_compress(
                    &mutation_operation("fast"),
                    vec![source.clone()],
                    Some(archive.clone()),
                )
                .unwrap();
            }
            let output = tempfile::tempdir().unwrap();
            let result = extract_archive_with_limits(
                archive.to_string_lossy().into_owned(),
                output.path().to_string_lossy().into_owned(),
                &|| Ok(()),
                limits,
            );
            assert!(result.is_err(), "unbounded {extension}");
            assert_eq!(fs::metadata(&source).unwrap().len(), 20);
        }
    }

    #[test]
    fn zip_preflight_limits_counts_and_depth_and_rejects_ambiguous_selection() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("paths.zip");
        let mut writer = zip::ZipWriter::new(File::create(&archive).unwrap());
        for name in ["a/b/file.txt", "a\\b\\file.txt"] {
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"safe").unwrap();
        }
        writer.finish().unwrap();
        for limits in [
            ArchiveLimits {
                max_depth: 2,
                ..ArchiveLimits::OPEN_FILE
            },
            ArchiveLimits {
                max_entries: 1,
                ..ArchiveLimits::OPEN_FILE
            },
            ArchiveLimits::OPEN_FILE,
        ] {
            let output = tempfile::tempdir().unwrap();
            assert!(
                extract_zip_file_for_open(&archive, "a/b/file.txt", output.path(), limits).is_err()
            );
            assert!(fs::read_dir(output.path()).unwrap().next().is_none());
        }
    }

    #[test]
    fn zip_listing_and_workspace_reject_windows_paths_and_streams() {
        use std::io::Write;
        for name in [
            "a/C:victim.txt",
            "a/../C:victim.txt",
            "C:sentinel.txt",
            "file.txt:extra",
            "dir./file",
            "NUL.txt",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let archive = dir.path().join("unsafe.zip");
            let mut writer = zip::ZipWriter::new(File::create(&archive).unwrap());
            writer
                .start_file("readme.txt", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"safe entry").unwrap();
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"untrusted entry").unwrap();
            writer.finish().unwrap();
            assert!(
                list_archive_contents(archive.to_string_lossy().into_owned(), String::new())
                    .is_err(),
                "listed {name}"
            );
            assert!(
                prepare_archive_file_for_open(&archive.join("readme.txt")).is_err(),
                "opened {name}"
            );
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn archive_outputs_and_external_copies_preserve_internet_provenance() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("readme.txt");
        fs::write(&source, b"safe document").unwrap();
        let data = b"[ZoneTransfer]\r\nZoneId=3\r\n";
        for extension in ["zip", "7z", "tar", "zst"] {
            let archive = dir.path().join(format!("marked.{extension}"));
            FileOperationManager::perform_compress(
                &mutation_operation("fast"),
                vec![source.clone()],
                Some(archive.clone()),
            )
            .unwrap();
            platform().write_file_provenance(&archive, data).unwrap();
            let target = dir.path().join(extension);
            extract_archive_with_checkpoint(
                archive.to_string_lossy().into_owned(),
                target.to_string_lossy().into_owned(),
                &|| Ok(()),
            )
            .unwrap();
            assert_eq!(
                platform()
                    .read_file_provenance(&target.join("readme.txt"))
                    .unwrap()
                    .as_deref(),
                Some(data.as_slice())
            );
            let cached = prepare_archive_file_for_open(&archive.join("readme.txt")).unwrap();
            assert_eq!(
                platform().read_file_provenance(&cached).unwrap().as_deref(),
                Some(data.as_slice())
            );
            fs::remove_file(&cached).unwrap();
            fs::remove_dir(cached.parent().unwrap()).unwrap();
        }
    }

    #[test]
    fn seven_zip_extraction_rejects_absolute_and_parent_paths() {
        let dir = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(dir.path()).unwrap();

        for name in [
            "../outside.txt",
            "nested/../../outside.txt",
            "..\\outside.txt",
            "C:/outside.txt",
            "\\\\server\\share\\outside.txt",
            "/outside.txt",
        ] {
            assert!(
                safe_seven_zip_destination(&root, name).is_err(),
                "unsafe archive path was accepted: {name}"
            );
        }
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn seven_zip_extraction_resolves_nested_paths_under_the_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(dir.path()).unwrap();
        let destination = safe_seven_zip_destination(&root, "nested\\folder\\file.txt").unwrap();

        assert_eq!(
            destination,
            root.join("nested").join("folder").join("file.txt")
        );
        assert!(destination.parent().unwrap().is_dir());
    }

    #[test]
    fn seven_zip_extraction_rejects_malicious_archive_entries() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("malicious.7z");
        let target = dir.path().join("extract");
        fs::create_dir(&target).unwrap();

        let mut writer = sevenz_rust2::ArchiveWriter::create(&archive_path).unwrap();
        writer
            .push_archive_entry(
                sevenz_rust2::ArchiveEntry::new_file("../escaped.txt"),
                Some(&b"must not be written outside the target"[..]),
            )
            .unwrap();
        writer.finish().unwrap();

        assert!(extract_archive_with_checkpoint(
            archive_path.to_string_lossy().into_owned(),
            target.to_string_lossy().into_owned(),
            &|| Ok(()),
        )
        .is_err());
        assert!(!dir.path().join("escaped.txt").exists());
    }

    #[test]
    fn seven_zip_compression_and_extraction_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        let archive_path = dir.path().join("roundtrip.7z");
        let target = dir.path().join("extract");
        fs::write(&source, b"round-trip payload").unwrap();
        fs::create_dir(&target).unwrap();
        FileOperationManager::perform_compress(
            &mutation_operation("fast"),
            vec![source],
            Some(archive_path.clone()),
        )
        .unwrap();
        extract_archive_with_checkpoint(
            archive_path.to_string_lossy().into_owned(),
            target.to_string_lossy().into_owned(),
            &|| Ok(()),
        )
        .unwrap();

        assert_eq!(
            fs::read(target.join("source.txt")).unwrap(),
            b"round-trip payload"
        );
    }

    #[test]
    fn iso_extraction_rejects_path_components() {
        for name in ["", ".", "..", "../outside", "C:", "\\outside"] {
            assert!(
                safe_iso_component(name).is_err(),
                "accepted ISO name: {name}"
            );
        }
        assert_eq!(
            safe_iso_component("document.txt;1").unwrap(),
            "document.txt;1"
        );
    }
}

#[cfg(test)]
pub(crate) fn test_iso_image(path: &Path) {
    fn record(name: &[u8], sector: u32, length: u32, directory: bool) -> Vec<u8> {
        let mut bytes = vec![0; (33 + name.len() + 1) & !1];
        bytes[0] = bytes.len() as u8;
        bytes[2..6].copy_from_slice(&sector.to_le_bytes());
        bytes[6..10].copy_from_slice(&sector.to_be_bytes());
        bytes[10..14].copy_from_slice(&length.to_le_bytes());
        bytes[14..18].copy_from_slice(&length.to_be_bytes());
        bytes[18..25].copy_from_slice(&[126, 10, 4, 0, 0, 0, 0]);
        bytes[25] = if directory { 2 } else { 0 };
        bytes[28..30].copy_from_slice(&1u16.to_le_bytes());
        bytes[30..32].copy_from_slice(&1u16.to_be_bytes());
        bytes[32] = name.len() as u8;
        bytes[33..33 + name.len()].copy_from_slice(name);
        bytes
    }
    let mut bytes = vec![0u8; 32 * 2048];
    let primary = &mut bytes[16 * 2048..17 * 2048];
    primary[0..7].copy_from_slice(b"\x01CD001\x01");
    primary[8..72].fill(b' ');
    primary[40..45].copy_from_slice(b"OXYDE");
    primary[80..84].copy_from_slice(&32u32.to_le_bytes());
    primary[84..88].copy_from_slice(&32u32.to_be_bytes());
    for offset in [120, 124] {
        primary[offset..offset + 2].copy_from_slice(&1u16.to_le_bytes());
        primary[offset + 2..offset + 4].copy_from_slice(&1u16.to_be_bytes());
    }
    primary[128..130].copy_from_slice(&2048u16.to_le_bytes());
    primary[130..132].copy_from_slice(&2048u16.to_be_bytes());
    primary[132..136].copy_from_slice(&22u32.to_le_bytes());
    primary[136..140].copy_from_slice(&22u32.to_be_bytes());
    primary[140..144].copy_from_slice(&18u32.to_le_bytes());
    primary[148..152].copy_from_slice(&19u32.to_be_bytes());
    primary[156..190].copy_from_slice(&record(&[0], 20, 2048, true));
    primary[190..813].fill(b' ');
    for offset in [813, 830, 847, 864] {
        primary[offset..offset + 16].fill(b'0');
    }
    primary[881] = 1;
    bytes[17 * 2048..17 * 2048 + 7].copy_from_slice(b"\xffCD001\x01");
    for (sector, big) in [(18, false), (19, true)] {
        let table = &mut bytes[sector * 2048..(sector + 1) * 2048];
        for (offset, name, extent) in [(0usize, &[0][..], 20u32), (10usize, &b"DOCS"[..], 21u32)] {
            table[offset] = name.len() as u8;
            table[offset + 2..offset + 6].copy_from_slice(&if big {
                extent.to_be_bytes()
            } else {
                extent.to_le_bytes()
            });
            table[offset + 6..offset + 8].copy_from_slice(&if big {
                1u16.to_be_bytes()
            } else {
                1u16.to_le_bytes()
            });
            table[offset + 8..offset + 8 + name.len()].copy_from_slice(name);
        }
    }
    for (sector, records) in [
        (
            20,
            vec![
                record(&[0], 20, 2048, true),
                record(&[1], 20, 2048, true),
                record(b"DOCS", 21, 2048, true),
                record(b"ROOT.TXT;1", 22, 4, false),
            ],
        ),
        (
            21,
            vec![
                record(&[0], 21, 2048, true),
                record(&[1], 20, 2048, true),
                record(b"README.TXT;1", 23, 11, false),
            ],
        ),
    ] {
        let mut offset = sector * 2048;
        for record in records {
            bytes[offset..offset + record.len()].copy_from_slice(&record);
            offset += record.len();
        }
    }
    bytes[22 * 2048..22 * 2048 + 4].copy_from_slice(b"root");
    bytes[23 * 2048..23 * 2048 + 11].copy_from_slice(b"ISO content");
    fs::write(path, bytes).unwrap();
}

#[cfg(test)]
mod disk_image_tests {
    use super::*;
    fn malformed_iso_rejected_without_panic(record_byte: usize, value: u8) {
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("invalid.iso");
        test_iso_image(&image);
        let mut bytes = fs::read(&image).unwrap();
        let mut offset = 20 * 2048;
        for _ in 0..3 {
            offset += bytes[offset] as usize;
        }
        bytes[offset + record_byte] = value;
        fs::write(&image, bytes).unwrap();
        let output = temp.path().join("output");
        fs::create_dir(&output).unwrap();
        let listing = std::panic::catch_unwind(|| {
            list_archive_contents(image.to_string_lossy().into_owned(), String::new())
        });
        let extraction = std::panic::catch_unwind(|| {
            extract_archive_with_checkpoint(
                image.to_string_lossy().into_owned(),
                output.to_string_lossy().into_owned(),
                &|| Ok(()),
            )
        });
        assert!(listing.is_ok(), "listing panicked on a malformed ISO");
        assert!(extraction.is_ok(), "extraction panicked on a malformed ISO");
        assert!(listing.unwrap().is_err(), "invalid ISO listing succeeded");
        assert!(
            extraction.unwrap().is_err(),
            "incomplete ISO extraction succeeded"
        );
        assert!(fs::read_dir(output).unwrap().next().is_none());
    }

    #[test]
    fn iso_invalid_identifier_returns_error_instead_of_panicking() {
        malformed_iso_rejected_without_panic(32, 255);
    }

    #[test]
    fn iso_invalid_record_is_not_a_successful_end_of_directory() {
        malformed_iso_rejected_without_panic(0, 20);
    }

    #[test]
    fn iso_navigation_extraction_and_cancellation() {
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("disc.ISO");
        test_iso_image(&image);
        let root =
            list_archive_contents(image.to_string_lossy().into_owned(), String::new()).unwrap();
        assert_eq!(root.len(), 2);
        let docs = root.iter().find(|entry| entry.name == "DOCS").unwrap();
        assert!(docs.is_dir);
        let nested =
            list_archive_contents(image.to_string_lossy().into_owned(), "DOCS".into()).unwrap();
        assert_eq!(
            nested[0].path,
            image.join("DOCS").join("README.TXT").to_string_lossy()
        );
        let workspace =
            crate::systems::archive_workspace::ArchiveWorkspace::open(image.clone()).unwrap();
        assert_eq!(
            fs::read(workspace.path("DOCS/README.TXT").unwrap()).unwrap(),
            b"ISO content"
        );
        assert!(!crate::systems::archive_workspace::writable(&image));
        let output = temp.path().join("cancelled");
        fs::create_dir(&output).unwrap();
        let checkpoints = std::cell::Cell::new(0);
        assert!(extract_archive_with_checkpoint(
            image.to_string_lossy().into_owned(),
            output.to_string_lossy().into_owned(),
            &|| {
                let count = checkpoints.get() + 1;
                checkpoints.set(count);
                if count >= 6 {
                    Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
                } else {
                    Ok(())
                }
            }
        )
        .is_err());
        assert!(checkpoints.get() >= 6);
    }
}
