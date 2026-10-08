use crate::models::{CommandError, FileEntry};
use crate::platform::platform;
pub use crate::utils::archive::{is_archive, split_virtual_path, ArchiveFormat};
use crate::utils::archive_security::{relative_archive_path, ExtractionRoot};
use crate::utils::path_security::validate_path;
use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use iso9660_core::iso9660entry::{ISO9660Record, IsISO9660Record};
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
    crate::systems::archive_workspace::external_file(path).map_err(CommandError::ArchiveError)
}

pub fn list_archive_contents(
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
        safe_iso_component(&display_name)?;

        let is_dir = matches!(rec, ISO9660Record::Directory(_));
        let size = match &rec {
            ISO9660Record::File(f) => f.data_length() as u64,
            _ => 0,
        };

        let full_virtual_path = virtual_entry_path(path, internal_prefix, &display_name)?;

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
            original_path: None,
            network_details: None,
            deleted_time: None,
        });
    }

    Ok(results)
}

fn virtual_entry_path(archive: &Path, internal: &str, name: &str) -> Result<String, CommandError> {
    let mut path = archive.to_path_buf();
    path.push(relative_archive_path(internal)?);
    path.push(relative_archive_path(name)?);
    Ok(path.to_string_lossy().into_owned())
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
        relative_archive_path(&name)?;

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
            let full_virtual_path = virtual_entry_path(path, internal_prefix, &entry_name)?;

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
        relative_archive_path(&name)?;
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
                        path: virtual_entry_path(path, internal_prefix, &entry_name)?,
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
        relative_archive_path(&name)?;

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
                path: virtual_entry_path(path, internal_prefix, &entry_name)?,
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
                original_path: None,
                network_details: None,
                deleted_time: None,
            });
        }
    }
    Ok(entries)
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
    let root = ExtractionRoot::new(target)?;
    let provenance = platform().read_file_provenance(path)?;

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
                if entry
                    .unix_mode()
                    .is_some_and(|mode| mode & 0o170000 == 0o120000)
                {
                    return Err(CommandError::ArchiveError(
                        "Archive links are not supported".into(),
                    ));
                }
                let destination = root.destination(entry.name(), entry.is_dir())?;
                if !entry.is_dir() {
                    let mut output = platform().create_archive_file(&destination.path)?;
                    io::copy(
                        &mut CheckedReader {
                            reader: &mut entry,
                            checkpoint,
                        },
                        &mut output,
                    )?;
                    if let Some(data) = provenance.as_deref() {
                        platform().write_file_provenance(&destination.path, data)?;
                    }
                }
            }
        }
        ArchiveFormat::SevenZip => {
            decompress_seven_zip_with_checkpoint(
                path,
                target,
                &root,
                provenance.as_deref(),
                checkpoint,
            )?;
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
            for entry in archive.entries()? {
                let mut entry = entry?;
                checkpoint()?;
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
                let destination = root.destination(&name, kind.is_dir())?;
                if kind.is_file() {
                    let mut output = platform().create_archive_file(&destination.path)?;
                    io::copy(&mut entry, &mut output)?;
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
            extract_iso(&archive_path, &root, provenance.as_deref(), checkpoint)?;
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
    path: &Path,
    target: &Path,
    root: &ExtractionRoot,
    provenance: Option<&[u8]>,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<(), CommandError> {
    sevenz_rust2::decompress_file_with_extract_fn(path, target, |entry, reader, _| {
        checkpoint()?;
        let destination = root.destination(entry.name(), entry.is_directory())?;
        if !entry.is_directory() {
            let mut output = platform().create_archive_file(&destination.path)?;
            io::copy(&mut CheckedReader { reader, checkpoint }, &mut output)?;
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
    .map_err(|e| CommandError::ArchiveError(e.to_string()))
}

#[cfg(test)]
fn safe_seven_zip_destination(root: &Path, name: &str) -> io::Result<PathBuf> {
    Ok(ExtractionRoot::new(root)?.destination(name, false)?.path)
}

fn safe_iso_component(name: &str) -> Result<&str, CommandError> {
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

fn extract_iso(
    archive_path: &str,
    root: &ExtractionRoot,
    provenance: Option<&[u8]>,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<(), CommandError> {
    let file = File::open(archive_path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let mut iso = iso9660_core::ISO9660::load(file)
        .map_err(|e| CommandError::ArchiveError(format!("{:?}", e)))?;

    extract_iso_recursive(&mut iso, "/", root, provenance, checkpoint)
}

fn extract_iso_recursive<T: iso9660_core::block_device::ISORead>(
    iso: &mut iso9660_core::ISO9660<T>,
    internal_path: &str,
    root: &ExtractionRoot,
    provenance: Option<&[u8]>,
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

        let relative_path = new_internal.trim_start_matches('/');
        let destination =
            root.destination(relative_path, matches!(rec, ISO9660Record::Directory(_)))?;

        match rec {
            ISO9660Record::Directory(_) => {
                extract_iso_recursive(iso, &new_internal, root, provenance, checkpoint)?;
            }
            ISO9660Record::File(_) => {
                let size = iso
                    .total_size(&new_internal)
                    .map_err(|e| CommandError::ArchiveError(format!("{:?}", e)))?;
                let mut writer = platform().create_archive_file(&destination.path)?;

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
                if let Some(data) = provenance {
                    platform().write_file_provenance(&destination.path, data)?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod archive_tests {
    use super::*;
    use crate::systems::archive_workspace::mutation_operation;
    use crate::systems::file_ops::FileOperationManager;

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
