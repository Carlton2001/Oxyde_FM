use crate::models::{CommandError, FileEntry};
pub use crate::utils::archive::{is_archive, split_virtual_path, ArchiveFormat};
use crate::utils::path_security::validate_path;
use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use iso9660_core::iso9660entry::{ISO9660Record, IsISO9660Record};
use log::info;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tar::Archive as TarArchive;
use xz2::read::XzDecoder;
use zip::ZipArchive;
use zstd::stream::read::Decoder as ZstdDecoder;
use zstd::stream::write::Encoder as ZstdEncoder;

pub struct ArchiveState(pub AtomicBool);

struct TemporaryArchive(PathBuf);

impl Drop for TemporaryArchive {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub fn remove_items_from_archive(
    archive_path: PathBuf,
    internal_paths: Vec<String>,
) -> Result<(), CommandError> {
    let format = ArchiveFormat::from_path(&archive_path).ok_or(CommandError::ArchiveError(
        "Unsupported archive format".to_string(),
    ))?;

    match format {
        ArchiveFormat::Zip => remove_from_zip(&archive_path, &internal_paths),
        _ => Err(CommandError::ArchiveError(
            "Deleting from this archive format is not supported yet.".to_string(),
        )),
    }
}

fn remove_from_zip(archive_path: &Path, internal_paths: &[String]) -> Result<(), CommandError> {
    let file = File::open(archive_path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| CommandError::ArchiveError(e.to_string()))?;

    let temp_file_path = archive_path.with_extension(format!("{}.zip.tmp", uuid::Uuid::new_v4()));
    let temp_file =
        File::create_new(&temp_file_path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let _temp_guard = TemporaryArchive(temp_file_path.clone());
    let mut writer = zip::ZipWriter::new(temp_file);

    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        let entry_name = entry.name().to_string();

        let should_remove = internal_paths.iter().any(|p| {
            let normalized_p = p.replace('\\', "/");
            let normalized_archive_entry = entry_name.replace('\\', "/");

            if normalized_archive_entry == normalized_p {
                return true;
            }

            let dir_prefix = if normalized_p.ends_with('/') {
                normalized_p.clone()
            } else {
                format!("{}/", normalized_p)
            };

            normalized_archive_entry.starts_with(&dir_prefix)
        });

        if !should_remove {
            writer
                .raw_copy_file(entry)
                .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        }
    }

    let _ = writer
        .finish()
        .map_err(|e| CommandError::ArchiveError(e.to_string()))?;

    drop(archive);

    // Windows cannot rename over an existing file. Keep the original until the
    // replacement is in place, and restore it if that last rename fails.
    let backup_path = archive_path.with_extension(format!("{}.zip.bak", uuid::Uuid::new_v4()));
    if let Err(error) = fs::rename(archive_path, &backup_path) {
        let _ = fs::remove_file(&temp_file_path);
        return Err(CommandError::IoError(error.to_string()));
    }
    if let Err(error) = fs::rename(&temp_file_path, archive_path) {
        let restore = fs::rename(&backup_path, archive_path);
        let _ = fs::remove_file(&temp_file_path);
        return Err(CommandError::IoError(match restore {
            Ok(()) => error.to_string(),
            Err(restore_error) => format!(
                "Could not replace archive ({error}); original preserved at {} but could not be restored ({restore_error})",
                backup_path.display()
            ),
        }));
    }
    // The replacement is valid; a failed backup cleanup is harmless and can be
    // retried by the user without losing either archive.
    let _ = fs::remove_file(&backup_path);

    Ok(())
}

pub fn create_archive_directory(path: &Path) -> Result<(), std::io::Error> {
    create_archive_directory_with_quality(path, "normal")
}

pub fn create_archive_directory_with_quality(
    path: &Path,
    quality: &str,
) -> Result<(), std::io::Error> {
    crate::systems::archive_workspace::create_directory(path, quality)
}

pub fn prepare_archive_file_for_open(path: &Path) -> Result<PathBuf, CommandError> {
    crate::systems::archive_workspace::external_file(path).map_err(CommandError::ArchiveError)
}

pub fn list_archive_contents(
    archive_path: String,
    internal_path: String,
) -> Result<Vec<FileEntry>, CommandError> {
    let path_buf = validate_path(&archive_path)?;
    let path = path_buf.as_path();
    let format = ArchiveFormat::from_path(path).ok_or(CommandError::ArchiveError(
        "Unsupported archive format".to_string(),
    ))?;

    match format {
        ArchiveFormat::Zip => list_zip(path, &internal_path),
        ArchiveFormat::SevenZip => list_seven_zip(path, &internal_path),
        ArchiveFormat::Tar
        | ArchiveFormat::TarGz
        | ArchiveFormat::TarXz
        | ArchiveFormat::TarZst
        | ArchiveFormat::TarBz2 => list_tar(path, &internal_path, format),
        ArchiveFormat::Iso => list_iso(path, &internal_path),
        ArchiveFormat::Rar => Err(CommandError::ArchiveError(
            "Rar navigation not supported yet. Please extract it first.".to_string(),
        )),
    }
}

fn list_iso(path: &Path, internal_prefix: &str) -> Result<Vec<FileEntry>, CommandError> {
    let file = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut iso = iso9660_core::ISO9660::load(file)
        .map_err(|e| CommandError::ArchiveError(format!("{:?}", e)))?;

    let internal_path = if internal_prefix.is_empty() {
        "/"
    } else {
        internal_prefix
    };
    let mut results = Vec::new();

    let mut entries_iter = iso
        .listdir(internal_path)
        .map_err(|e| CommandError::ArchiveError(format!("{:?}", e)))?;

    loop {
        let record = entries_iter.next(&mut iso);
        let Some(rec) = record else {
            break;
        };

        let name = match &rec {
            ISO9660Record::Directory(d) => d.identifier(),
            ISO9660Record::File(f) => f.identifier(),
        };

        if name == "." || name == ".." {
            continue;
        }

        let display_name = name.split(';').next().unwrap_or(name).to_string();
        if display_name.is_empty() {
            continue;
        }

        let is_dir = matches!(rec, ISO9660Record::Directory(_));
        let size = match &rec {
            ISO9660Record::File(f) => f.data_length() as u64,
            _ => 0,
        };

        let full_virtual_path = virtual_entry_path(path, internal_prefix, &display_name);

        results.push(FileEntry {
            name: display_name,
            path: full_virtual_path,
            is_dir,
            is_hidden: false,
            is_system: false,
            is_symlink: false,
            is_junction: false,
            size,
            is_calculated: false,
            folders_count: None,
            files_count: None,
            modified: 0,
            is_readonly: true,
            is_protected: false,
            original_path: None,
            network_details: None,
            deleted_time: None,
        });
    }

    Ok(results)
}

fn virtual_entry_path(archive: &Path, internal: &str, name: &str) -> String {
    let mut path = archive.to_path_buf();
    let normalized = internal.replace('\\', "/");
    for component in normalized.split('/').filter(|part| !part.is_empty()) {
        path.push(component);
    }
    path.push(name);
    path.to_string_lossy().into_owned()
}

fn list_zip(path: &Path, internal_prefix: &str) -> Result<Vec<FileEntry>, CommandError> {
    let file = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    let mut entries = Vec::new();
    let prefix = if internal_prefix.is_empty() {
        "".to_string()
    } else {
        format!("{}/", internal_prefix.trim_end_matches('/'))
    };

    let mut seen: HashSet<String> = HashSet::new();

    for i in 0..archive.len() {
        let file = archive
            .by_index(i)
            .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        let name = file.name().replace('\\', "/");

        if name.starts_with(&prefix) && name != prefix {
            let relative = &name[prefix.len()..];
            let parts: Vec<&str> = relative
                .split('/')
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>();
            if parts.is_empty() {
                continue;
            }

            let entry_name = parts[0].to_string();
            if seen.contains(&entry_name) {
                continue;
            }
            seen.insert(entry_name.clone());

            let is_dir = file.is_dir() || parts.len() > 1;
            let full_virtual_path = virtual_entry_path(path, internal_prefix, &entry_name);

            entries.push(FileEntry {
                name: entry_name,
                path: full_virtual_path,
                is_dir,
                is_hidden: false,
                is_system: false,
                is_symlink: false,
                is_junction: false,
                size: if is_dir { 0 } else { file.size() },
                is_calculated: false,
                folders_count: None,
                files_count: None,
                modified: file
                    .last_modified()
                    .and_then(|dt| {
                        let t: Result<chrono::NaiveDateTime, _> = dt.try_into();
                        t.ok()
                    })
                    .map(|ts| ts.and_utc().timestamp_millis() as u64)
                    .unwrap_or(0),
                is_readonly: false,
                is_protected: false,
                original_path: None,
                network_details: None,
                deleted_time: None,
            });
        }
    }
    Ok(entries)
}

fn list_seven_zip(path: &Path, internal_prefix: &str) -> Result<Vec<FileEntry>, CommandError> {
    let file = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let reader = sevenz_rust2::ArchiveReader::new(file, sevenz_rust2::Password::empty())
        .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    let mut entries = Vec::new();
    let prefix = if internal_prefix.is_empty() {
        "".to_string()
    } else {
        format!("{}/", internal_prefix.trim_end_matches('/'))
    };
    let mut seen: HashSet<String> = HashSet::new();

    // Listing needs only the 7z header; do not decompress every file while navigating.
    for entry in &reader.archive().files {
        let name = entry.name().replace('\\', "/");
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
                    let is_dir = entry.is_directory() || parts.len() > 1;
                    entries.push(FileEntry {
                        name: entry_name.clone(),
                        path: virtual_entry_path(path, internal_prefix, &entry_name),
                        is_dir,
                        is_hidden: false,
                        is_system: false,
                        is_symlink: false,
                        is_junction: false,
                        size: if is_dir { 0 } else { entry.size() },
                        is_calculated: false,
                        folders_count: None,
                        files_count: None,
                        modified: 0,
                        is_readonly: false,
                        is_protected: false,
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

fn list_tar(
    path: &Path,
    internal_prefix: &str,
    format: ArchiveFormat,
) -> Result<Vec<FileEntry>, CommandError> {
    let file = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let reader: Box<dyn io::Read> = match format {
        ArchiveFormat::TarGz => Box::new(GzDecoder::new(file)),
        ArchiveFormat::TarXz => Box::new(XzDecoder::new(file)),
        ArchiveFormat::TarBz2 => Box::new(BzDecoder::new(file)),
        ArchiveFormat::TarZst => {
            Box::new(ZstdDecoder::new(file).map_err(|e| CommandError::IoError(e.to_string()))?)
        }
        _ => Box::new(file),
    };

    let mut archive = TarArchive::new(reader);
    let mut entries = Vec::new();
    let prefix = if internal_prefix.is_empty() {
        "".to_string()
    } else {
        format!("{}/", internal_prefix.trim_end_matches('/'))
    };
    let mut seen: HashSet<String> = HashSet::new();

    for entry in archive
        .entries()
        .map_err(|e| CommandError::ArchiveError(e.to_string()))?
    {
        let entry = entry.map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        let path_field = entry
            .path()
            .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        let name = path_field.to_string_lossy().replace('\\', "/");

        if name.starts_with(&prefix) && name != prefix {
            let relative = &name[prefix.len()..];
            let parts: Vec<&str> = relative
                .split('/')
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>();
            if parts.is_empty() {
                continue;
            }

            let entry_name = parts[0].to_string();
            if seen.contains(&entry_name) {
                continue;
            }
            seen.insert(entry_name.clone());

            let is_dir = entry.header().entry_type().is_dir() || parts.len() > 1;
            entries.push(FileEntry {
                path: virtual_entry_path(path, internal_prefix, &entry_name),
                name: entry_name,
                is_dir,
                is_hidden: false,
                is_system: false,
                is_symlink: false,
                is_junction: false,
                size: entry.header().size().unwrap_or(0),
                is_calculated: false,
                folders_count: None,
                files_count: None,
                modified: entry.header().mtime().unwrap_or(0) * 1000,
                is_readonly: false,
                is_protected: false,
                original_path: None,
                network_details: None,
                deleted_time: None,
            });
        }
    }
    Ok(entries)
}

pub async fn extract_archive(
    archive_path: String,
    target_dir: String,
    state: &ArchiveState,
) -> Result<(), CommandError> {
    extract_archive_with_checkpoint(archive_path, target_dir, &|| {
        if state.0.load(Ordering::Relaxed) {
            Err(io::Error::other("Cancelled"))
        } else {
            Ok(())
        }
    })
}

struct CheckedReader<'a, R> {
    reader: R,
    checkpoint: &'a dyn Fn() -> io::Result<()>,
}
impl<R: io::Read> io::Read for CheckedReader<'_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        (self.checkpoint)()?;
        self.reader.read(buffer)
    }
}

pub(crate) fn extract_archive_with_checkpoint(
    archive_path: String,
    target_dir: String,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<(), CommandError> {
    checkpoint().map_err(|e| CommandError::IoError(e.to_string()))?;
    let path_buf = validate_path(&archive_path)?;
    let path = path_buf.as_path();
    let format = ArchiveFormat::from_path(path).ok_or(CommandError::ArchiveError(
        "Unsupported archive format".to_string(),
    ))?;
    let target_buf = validate_path(&target_dir)?;
    let target = target_buf.as_path();

    info!("Extracting {:?} to {:?}", path, target);

    if !target.exists() {
        fs::create_dir_all(target).map_err(|e| CommandError::IoError(e.to_string()))?;
    }

    match format {
        ArchiveFormat::Zip => {
            let file = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
            let mut archive =
                ZipArchive::new(file).map_err(|e| CommandError::ArchiveError(e.to_string()))?;
            for index in 0..archive.len() {
                checkpoint().map_err(|e| CommandError::IoError(e.to_string()))?;
                let mut entry = archive
                    .by_index(index)
                    .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
                let relative = entry
                    .enclosed_name()
                    .ok_or_else(|| CommandError::ArchiveError("Unsafe ZIP entry path".into()))?;
                let destination = target.join(relative);
                if entry.is_dir() {
                    fs::create_dir_all(&destination)?;
                } else {
                    if let Some(parent) = destination.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    let mut output = File::create(destination)?;
                    io::copy(
                        &mut CheckedReader {
                            reader: &mut entry,
                            checkpoint,
                        },
                        &mut output,
                    )?;
                }
            }
        }
        ArchiveFormat::SevenZip => {
            decompress_seven_zip_with_checkpoint(path, target, checkpoint)?;
        }
        ArchiveFormat::Tar
        | ArchiveFormat::TarGz
        | ArchiveFormat::TarXz
        | ArchiveFormat::TarZst
        | ArchiveFormat::TarBz2 => {
            let file = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
            let reader: Box<dyn io::Read> = match format {
                ArchiveFormat::TarGz => Box::new(GzDecoder::new(file)),
                ArchiveFormat::TarXz => Box::new(XzDecoder::new(file)),
                ArchiveFormat::TarBz2 => Box::new(BzDecoder::new(file)),
                ArchiveFormat::TarZst => Box::new(
                    ZstdDecoder::new(file).map_err(|e| CommandError::IoError(e.to_string()))?,
                ),
                _ => Box::new(file),
            };
            let mut archive = TarArchive::new(CheckedReader { reader, checkpoint });
            archive
                .unpack(target)
                .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        }
        ArchiveFormat::Iso => {
            extract_iso(&archive_path, &target_dir, checkpoint)?;
        }
        ArchiveFormat::Rar => {
            return Err(CommandError::ArchiveError(
                "Rar extraction requires external tools (like 7-Zip or WinRAR).".to_string(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
fn decompress_seven_zip_safely(path: &Path, target: &Path) -> Result<(), CommandError> {
    decompress_seven_zip_with_checkpoint(path, target, &|| Ok(()))
}

fn decompress_seven_zip_with_checkpoint(
    path: &Path,
    target: &Path,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<(), CommandError> {
    let root = fs::canonicalize(target).map_err(|e| CommandError::IoError(e.to_string()))?;
    sevenz_rust2::decompress_file_with_extract_fn(path, target, |entry, reader, _| {
        let destination = safe_seven_zip_destination(&root, entry.name())?;
        checkpoint()?;
        sevenz_rust2::default_entry_extract_fn(
            entry,
            &mut CheckedReader { reader, checkpoint },
            &destination,
        )
    })
    .map_err(|e| CommandError::ArchiveError(e.to_string()))
}

fn safe_seven_zip_destination(
    extraction_root: &Path,
    entry_name: &str,
) -> std::io::Result<PathBuf> {
    let normalized = entry_name.replace('\\', "/");
    let first_component = normalized.split('/').next().unwrap_or_default();
    let has_drive_prefix = first_component.as_bytes().get(1) == Some(&b':');
    let components: Vec<_> = normalized.split('/').collect();
    if normalized.starts_with('/')
        || has_drive_prefix
        || components
            .iter()
            .any(|part| *part == ".." || part.contains(':'))
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Unsafe 7z archive entry path: {entry_name}"),
        ));
    }

    let relative_path: PathBuf = components
        .into_iter()
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    if relative_path.as_os_str().is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "7z archive entry resolves to the extraction root",
        ));
    }

    let mut canonical_parent = extraction_root.to_path_buf();
    if let Some(relative_parent) = relative_path.parent() {
        for component in relative_parent.components() {
            canonical_parent.push(component.as_os_str());
            match std::fs::symlink_metadata(&canonical_parent) {
                Ok(metadata) => {
                    if !metadata.is_dir() {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!(
                                "7z archive parent is not a directory: {}",
                                canonical_parent.display()
                            ),
                        ));
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    match std::fs::create_dir(&canonical_parent) {
                        Ok(()) => {}
                        Err(error)
                            if error.kind() == std::io::ErrorKind::AlreadyExists
                                && canonical_parent.is_dir() => {}
                        Err(error) => return Err(error),
                    }
                }
                Err(error) => return Err(error),
            }
            canonical_parent = std::fs::canonicalize(&canonical_parent)?;
            if !canonical_parent.starts_with(extraction_root) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("7z archive entry escapes the extraction directory: {entry_name}"),
                ));
            }
        }
    }

    let file_name = relative_path.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "7z archive entry has no file name",
        )
    })?;
    let safe_destination = canonical_parent.join(file_name);
    match std::fs::symlink_metadata(&safe_destination) {
        Ok(_) => {
            let existing = std::fs::canonicalize(&safe_destination)?;
            if !existing.starts_with(extraction_root) {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!(
                    "7z archive entry would overwrite a path outside the extraction directory: {entry_name}"
                )));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    Ok(safe_destination)
}

fn safe_iso_component(name: &str) -> Result<&str, CommandError> {
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

fn extract_iso(
    archive_path: &str,
    target_dir: &str,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<(), CommandError> {
    let file = File::open(archive_path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut iso = iso9660_core::ISO9660::load(file)
        .map_err(|e| CommandError::ArchiveError(format!("{:?}", e)))?;

    extract_iso_recursive(&mut iso, "/", target_dir, checkpoint)
}

fn extract_iso_recursive<T: iso9660_core::block_device::ISORead>(
    iso: &mut iso9660_core::ISO9660<T>,
    internal_path: &str,
    target_base: &str,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<(), CommandError> {
    let mut iter = iso
        .listdir(internal_path)
        .map_err(|e| CommandError::ArchiveError(format!("{:?}", e)))?;

    let mut records = Vec::new();
    while let Some(record) = iter.next(iso) {
        checkpoint()?;
        records.push(record);
    }

    for rec in records {
        checkpoint()?;
        let name = match &rec {
            ISO9660Record::Directory(d) => d.identifier(),
            ISO9660Record::File(f) => f.identifier(),
        };

        if name == "." || name == ".." {
            continue;
        }

        let display_name = safe_iso_component(name.split(';').next().unwrap_or(name))?;
        let new_internal = if internal_path == "/" {
            format!("/{}", display_name)
        } else {
            format!("{}/{}", internal_path.trim_end_matches('/'), display_name)
        };

        let extraction_root =
            fs::canonicalize(target_base).map_err(|e| CommandError::IoError(e.to_string()))?;
        let relative_path = new_internal.trim_start_matches('/');
        let target_path = safe_seven_zip_destination(&extraction_root, relative_path)
            .map_err(|e| CommandError::ArchiveError(e.to_string()))?;

        match rec {
            ISO9660Record::Directory(_) => {
                fs::create_dir_all(&target_path)
                    .map_err(|e| CommandError::IoError(e.to_string()))?;
                extract_iso_recursive(iso, &new_internal, target_base, checkpoint)?;
            }
            ISO9660Record::File(_) => {
                if let Some(parent) = target_path.parent() {
                    fs::create_dir_all(parent).map_err(|e| CommandError::IoError(e.to_string()))?;
                }

                let size = iso
                    .total_size(&new_internal)
                    .map_err(|e| CommandError::ArchiveError(format!("{:?}", e)))?;
                let mut writer =
                    File::create(target_path).map_err(|e| CommandError::IoError(e.to_string()))?;

                let mut offset = 0;
                let mut buf = [0u8; 65536];
                while offset < size {
                    checkpoint()?;
                    let to_read = std::cmp::min(buf.len(), size - offset);
                    let n = iso
                        .read(&new_internal, &mut buf[..to_read], offset)
                        .map_err(|e| CommandError::ArchiveError(format!("{:?}", e)))?;
                    if n == 0 {
                        return Err(CommandError::ArchiveError(
                            "Unexpected end of ISO file".into(),
                        ));
                    }
                    use std::io::Write;
                    writer
                        .write_all(&buf[..n])
                        .map_err(|e| CommandError::IoError(e.to_string()))?;
                    offset += n;
                }
            }
        }
    }
    Ok(())
}

pub async fn cancel_archive_operation(state: &ArchiveState) -> Result<(), CommandError> {
    state.0.store(true, Ordering::Relaxed);
    Ok(())
}

pub async fn compress_to_archive(
    paths: Vec<String>,
    archive_path: String,
    format: String,
    quality: String,
    state: &ArchiveState,
) -> Result<(), CommandError> {
    state.0.store(false, Ordering::Relaxed);
    let target_path_buf = validate_path(&archive_path)?;
    let target_path = target_path_buf.as_path();

    if target_path.exists() {
        return Err(CommandError::ArchiveError(format!(
            "Archive already exists: {}",
            target_path.display()
        )));
    }

    info!("Compressing {:?} items to {:?}", paths.len(), target_path);

    let temporary_path =
        target_path.with_extension(format!("{}.archive.tmp", uuid::Uuid::new_v4()));
    let _temporary_guard = TemporaryArchive(temporary_path.clone());

    match format.to_lowercase().as_str() {
        "zip" => compress_zip(paths, &temporary_path, &quality, state),
        "7z" => compress_seven_zip(paths, &temporary_path, &quality, state),
        "tar" => compress_tar(paths, &temporary_path, false, state),
        "zst" | "tar.zst" => compress_tar_zst(paths, &temporary_path, &quality, state),
        _ => Err(CommandError::ArchiveError("Unsupported format".to_string())),
    }?;

    // hard_link creates the final name only if it is still free. Some removable
    // filesystems do not support hard links, so use rename there on Windows.
    if fs::hard_link(&temporary_path, target_path).is_err() {
        if target_path.exists() {
            return Err(CommandError::ArchiveError(format!(
                "Archive already exists: {}",
                target_path.display()
            )));
        }
        fs::rename(&temporary_path, target_path)
            .map_err(|error| CommandError::IoError(error.to_string()))?;
    }
    Ok(())
}

pub async fn add_to_archive(
    paths: Vec<String>,
    archive_path: String,
    state: &ArchiveState,
) -> Result<(), CommandError> {
    state.0.store(false, Ordering::Relaxed);
    let target_path_buf = validate_path(&archive_path)?;
    let target_path = target_path_buf.as_path();

    let format = ArchiveFormat::from_path(target_path).ok_or(CommandError::ArchiveError(
        "Unsupported archive format".to_string(),
    ))?;

    match format {
        ArchiveFormat::Zip => add_to_zip(paths, target_path, state),
        _ => Err(CommandError::ArchiveError(
            "Adding to this archive format is not supported yet.".to_string(),
        )),
    }
}

fn add_to_zip(paths: Vec<String>, target: &Path, state: &ArchiveState) -> Result<(), CommandError> {
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(target)
        .map_err(|e| CommandError::IoError(e.to_string()))?;

    let mut zip =
        zip::ZipWriter::new_append(file).map_err(|e| CommandError::ArchiveError(e.to_string()))?;

    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    for p in paths {
        if state.0.load(Ordering::Relaxed) {
            return Err(CommandError::Other("Cancelled".into()));
        }
        let path = Path::new(&p);
        let parent = path.parent().unwrap_or(path);

        if path.is_dir() {
            for entry in walkdir::WalkDir::new(path) {
                if state.0.load(Ordering::Relaxed) {
                    return Err(CommandError::Other("Cancelled".into()));
                }
                let entry = entry.map_err(|e| CommandError::IoError(e.to_string()))?;
                let entry_path = entry.path();
                let name = entry_path
                    .strip_prefix(parent)
                    .map_err(|e| CommandError::PathError(e.to_string()))?;
                let name_str = name.to_string_lossy().replace('\\', "/");

                if entry.file_type().is_dir() {
                    zip.add_directory(name_str, options)
                        .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
                } else {
                    zip.start_file(name_str, options)
                        .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
                    let mut f =
                        File::open(entry_path).map_err(|e| CommandError::IoError(e.to_string()))?;
                    io::copy(&mut f, &mut zip).map_err(|e| CommandError::IoError(e.to_string()))?;
                }
            }
        } else {
            let name = path
                .file_name()
                .ok_or(CommandError::PathError("Invalid path".to_string()))?;
            zip.start_file(name.to_string_lossy().replace('\\', "/"), options)
                .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
            let mut f = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
            io::copy(&mut f, &mut zip).map_err(|e| CommandError::IoError(e.to_string()))?;
        }
    }

    zip.finish()
        .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    Ok(())
}

fn compress_zip(
    paths: Vec<String>,
    target: &Path,
    quality: &str,
    state: &ArchiveState,
) -> Result<(), CommandError> {
    let file = File::create(target).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut zip = zip::ZipWriter::new(io::BufWriter::with_capacity(128 * 1024, file));

    let method = match quality {
        "fast" => zip::CompressionMethod::Deflated,
        "best" => zip::CompressionMethod::Deflated,
        _ => zip::CompressionMethod::Deflated,
    };

    let level = match quality {
        "fast" => Some(1),
        "best" => Some(9),
        _ => Some(6),
    };

    let options = zip::write::SimpleFileOptions::default()
        .compression_method(method)
        .compression_level(level)
        .unix_permissions(0o755);

    for p in paths {
        if state.0.load(Ordering::Relaxed) {
            return Err(CommandError::Other("Cancelled".into()));
        }
        let path = Path::new(&p);
        let parent = path.parent().unwrap_or(path);

        if path.is_dir() {
            for entry in walkdir::WalkDir::new(path) {
                if state.0.load(Ordering::Relaxed) {
                    return Err(CommandError::Other("Cancelled".into()));
                }
                let entry = entry.map_err(|e| CommandError::IoError(e.to_string()))?;
                let entry_path = entry.path();
                let name = entry_path
                    .strip_prefix(parent)
                    .map_err(|e| CommandError::PathError(e.to_string()))?;
                let name_str = name.to_string_lossy().replace('\\', "/");

                if entry.file_type().is_dir() {
                    zip.add_directory(name_str, options)
                        .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
                } else {
                    zip.start_file(name_str, options)
                        .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
                    let mut f =
                        File::open(entry_path).map_err(|e| CommandError::IoError(e.to_string()))?;
                    io::copy(&mut f, &mut zip).map_err(|e| CommandError::IoError(e.to_string()))?;
                }
            }
        } else {
            let name = path
                .file_name()
                .ok_or(CommandError::PathError("Invalid path".to_string()))?;
            zip.start_file(name.to_string_lossy().replace('\\', "/"), options)
                .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
            let mut f = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
            io::copy(&mut f, &mut zip).map_err(|e| CommandError::IoError(e.to_string()))?;
        }
    }

    zip.finish()
        .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    Ok(())
}

fn compress_seven_zip(
    paths: Vec<String>,
    target: &Path,
    _quality: &str,
    state: &ArchiveState,
) -> Result<(), CommandError> {
    if paths.is_empty() {
        return Ok(());
    }

    let file = File::create(target).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut writer = sevenz_rust2::ArchiveWriter::new(file)
        .map_err(|e| CommandError::ArchiveError(e.to_string()))?;

    for p in paths {
        if state.0.load(Ordering::Relaxed) {
            return Err(CommandError::Other("Cancelled".into()));
        }
        let path = Path::new(&p);
        if path.is_dir() {
            add_dir_to_sevenz(&mut writer, path, path.parent().unwrap_or(path), state)?;
        } else {
            add_file_to_sevenz(&mut writer, path, path.parent().unwrap_or(path))?;
        }
    }
    writer
        .finish()
        .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    Ok(())
}

fn add_file_to_sevenz<W: io::Write + io::Seek>(
    writer: &mut sevenz_rust2::ArchiveWriter<W>,
    path: &Path,
    base: &Path,
) -> Result<(), CommandError> {
    let f = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let name = path
        .strip_prefix(base)
        .map_err(|e| CommandError::PathError(e.to_string()))?
        .to_string_lossy();
    writer
        .push_archive_entry(
            sevenz_rust2::ArchiveEntry::from_path(path, name.into()),
            Some(&mut io::BufReader::with_capacity(128 * 1024, f)),
        )
        .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    Ok(())
}

fn add_dir_to_sevenz<W: io::Write + io::Seek>(
    writer: &mut sevenz_rust2::ArchiveWriter<W>,
    path: &Path,
    base: &Path,
    state: &ArchiveState,
) -> Result<(), CommandError> {
    let name = path
        .strip_prefix(base)
        .map_err(|e| CommandError::PathError(e.to_string()))?
        .to_string_lossy();
    writer
        .push_archive_entry(
            sevenz_rust2::ArchiveEntry::from_path(path, name.into()),
            None::<&mut File>,
        )
        .map_err(|e| CommandError::ArchiveError(e.to_string()))?;

    for entry in fs::read_dir(path).map_err(|e| CommandError::IoError(e.to_string()))? {
        if state.0.load(Ordering::Relaxed) {
            return Err(CommandError::Other("Cancelled".into()));
        }
        let entry = entry.map_err(|e| CommandError::IoError(e.to_string()))?;
        let p = entry.path();
        if p.is_dir() {
            add_dir_to_sevenz(writer, &p, base, state)?;
        } else {
            add_file_to_sevenz(writer, &p, base)?;
        }
    }
    Ok(())
}

fn compress_tar(
    paths: Vec<String>,
    target: &Path,
    _gz: bool,
    state: &ArchiveState,
) -> Result<(), CommandError> {
    let file = File::create(target).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut tar = tar::Builder::new(io::BufWriter::with_capacity(128 * 1024, file));

    for p in paths {
        if state.0.load(Ordering::Relaxed) {
            return Err(CommandError::Other("Cancelled".into()));
        }
        let path = Path::new(&p);
        let name = path
            .file_name()
            .ok_or(CommandError::PathError("Invalid path".to_string()))?;
        if path.is_dir() {
            tar.append_dir_all(name, path)
                .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        } else {
            let f = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
            let mut header = tar::Header::new_gnu();
            header
                .set_path(name)
                .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
            header.set_size(
                f.metadata()
                    .map_err(|e| CommandError::IoError(e.to_string()))?
                    .len(),
            );
            header.set_cksum();
            tar.append(&header, &mut io::BufReader::with_capacity(128 * 1024, f))
                .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        }
    }
    tar.finish()
        .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    Ok(())
}

fn compress_tar_zst(
    paths: Vec<String>,
    target: &Path,
    quality: &str,
    state: &ArchiveState,
) -> Result<(), CommandError> {
    let file = File::create(target).map_err(|e| CommandError::IoError(e.to_string()))?;
    let level = match quality {
        "fast" => 1,
        "best" => 19,
        _ => 3,
    };
    let zstd = ZstdEncoder::new(file, level)
        .map_err(|e| CommandError::IoError(e.to_string()))?
        .auto_finish();
    let mut tar = tar::Builder::new(zstd);

    for p in paths {
        if state.0.load(Ordering::Relaxed) {
            return Err(CommandError::Other("Cancelled".into()));
        }
        let path = Path::new(&p);
        let name = path
            .file_name()
            .ok_or(CommandError::PathError("Invalid path".to_string()))?;
        if path.is_dir() {
            tar.append_dir_all(name, path)
                .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        } else {
            let f = File::open(path).map_err(|e| CommandError::IoError(e.to_string()))?;
            let mut header = tar::Header::new_gnu();
            header
                .set_path(name)
                .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
            header.set_size(
                f.metadata()
                    .map_err(|e| CommandError::IoError(e.to_string()))?
                    .len(),
            );
            header.set_cksum();
            tar.append(&header, &mut io::BufReader::with_capacity(128 * 1024, f))
                .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
        }
    }
    tar.finish()
        .map_err(|e| CommandError::ArchiveError(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod archive_tests {
    use super::*;
    use std::io::Write;

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

        assert!(decompress_seven_zip_safely(&archive_path, &target).is_err());
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
        let state = ArchiveState(AtomicBool::new(false));

        compress_seven_zip(
            vec![source.to_string_lossy().into_owned()],
            &archive_path,
            "fast",
            &state,
        )
        .unwrap();
        decompress_seven_zip_safely(&archive_path, &target).unwrap();

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

    #[test]
    fn removing_zip_item_keeps_other_entries_and_cleans_temporary_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.zip");
        let file = File::create(&path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        for (name, contents) in [("keep.txt", &b"keep"[..]), ("remove.txt", &b"remove"[..])] {
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(contents).unwrap();
        }
        writer.finish().unwrap();

        remove_items_from_archive(path.clone(), vec!["remove.txt".into()]).unwrap();

        let mut archive = ZipArchive::new(File::open(&path).unwrap()).unwrap();
        assert_eq!(archive.len(), 1);
        assert_eq!(archive.by_index(0).unwrap().name(), "keep.txt");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn compression_does_not_overwrite_an_existing_archive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("existing.zip");
        fs::write(&path, b"original").unwrap();
        let state = ArchiveState(AtomicBool::new(false));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(compress_to_archive(
            Vec::new(),
            path.to_string_lossy().into_owned(),
            "zip".into(),
            "fast".into(),
            &state,
        ));
        assert!(result.is_err());
        assert_eq!(fs::read(path).unwrap(), b"original");
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
    #[test]
    fn iso_navigation_extraction_and_cancellation() {
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("disc.ISO");
        test_iso_image(&image);
        let root =
            list_archive_contents(image.to_string_lossy().into_owned(), String::new()).unwrap();
        assert_eq!(root.len(), 2);
        assert!(root.iter().all(|entry| entry.is_readonly));
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
