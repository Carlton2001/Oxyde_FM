#[cfg(any(not(target_os = "windows"), test))]
pub(crate) use super::archive_iso_open::extract_iso_file_for_open;
use super::archive_iso_open::list_iso;
#[cfg(any(not(target_os = "windows"), test))]
use super::archive_iso_open::extract_iso;
#[cfg(any(not(target_os = "windows"), test))]
pub(crate) use super::archive_seven_open::extract_seven_file_for_open;
#[cfg(any(not(target_os = "windows"), test))]
pub(crate) use super::archive_tar_open::extract_tar_file_for_open;
use crate::models::{CommandError, FileEntry};
use crate::platform::platform;
pub use crate::utils::archive::{is_archive, split_virtual_path, ArchiveFormat};
#[cfg(any(not(target_os = "windows"), test))]
use crate::utils::archive_limits::{ArchiveBudget, ArchiveLimits};
#[cfg(any(not(target_os = "windows"), test))]
use crate::utils::archive_limits::BudgetReader;
use crate::utils::archive_security::relative_archive_path;
#[cfg(any(not(target_os = "windows"), test))]
use crate::utils::archive_security::ExtractionRoot;
use crate::utils::path_security::validate_path;
#[cfg(any(not(target_os = "windows"), test))]
use {bzip2::read::BzDecoder, flate2::read::GzDecoder, log::info};
use std::collections::HashSet;
#[cfg(any(not(target_os = "windows"), test))]
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
#[cfg(any(not(target_os = "windows"), test))]
use {tar::Archive as TarArchive, xz2::read::XzDecoder, zstd::stream::read::Decoder as ZstdDecoder};
#[cfg(any(not(target_os = "windows"), test))]
use zip::ZipArchive;

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

/// Keep cache integration fixtures inside their owned test directory.
#[doc(hidden)]
pub fn with_archive_cache_base_for_tests<T>(base: &Path, run: impl FnOnce() -> T) -> T {
    crate::systems::archive_open_cache::with_test_base(base, run)
}

/// ZIP supports random access: validate the index, then decode only the requested file.
#[cfg(any(not(target_os = "windows"), test))]
pub(crate) fn extract_zip_file_for_open(
    archive_path: &Path,
    internal: &str,
    target: &Path,
    limits: ArchiveLimits,
) -> Result<PathBuf, CommandError> {
    extract_zip_file_with_checkpoint(archive_path, internal, target, limits, &|| Ok(()))
}

#[cfg(any(not(target_os = "windows"), test))]
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
#[cfg(any(not(target_os = "windows"), test))]
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

#[cfg(any(not(target_os = "windows"), test))]
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
#[path = "archive/tests/archive_tests.rs"]
mod archive_tests;

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
#[path = "archive/tests/disk_image_tests.rs"]
mod disk_image_tests;
