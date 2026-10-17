//! Validated metadata for ZIP/TAR navigation and name search. No output files.
use crate::models::{CommandError, FileEntry};
use crate::platform::platform;
use crate::systems::path_parents::HeldParents;
use crate::utils::archive::ArchiveFormat;
use crate::utils::archive_limits::{ArchiveBudget, ArchiveLimits, BudgetReader};
use crate::utils::archive_security::relative_archive_path;
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

struct Source<'a> {
    file: File,
    length: u64,
    budget: &'a ArchiveBudget,
    checkpoint: &'a dyn Fn() -> io::Result<()>,
}

impl Read for Source<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.budget.reader_checkpoint(self.checkpoint)?;
        let count = self.file.read(buffer)?;
        self.budget.check_time()?;
        Ok(count)
    }
}

impl Seek for Source<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.budget.reader_checkpoint(self.checkpoint)?;
        let position = self.file.seek(position)?;
        self.budget.check_time()?;
        if position > self.length {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Truncated archive member",
            ));
        }
        Ok(position)
    }
}

// Raw TAR can skip file payloads by seeking; ZIP reads only headers.
struct IndexReader<'a>(BudgetReader<'a, Source<'a>>);

impl Read for IndexReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.0.read(buffer)
    }
}

impl Seek for IndexReader<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.0.reader.seek(position)
    }
}

fn invalid(message: &str) -> CommandError {
    CommandError::ArchiveError(message.into())
}

fn member(
    archive_path: &std::sync::Arc<std::path::PathBuf>,
    name: &str,
    directory: bool,
    size: u64,
    modified: u64,
    budget: &ArchiveBudget,
) -> Result<Option<FileEntry>, CommandError> {
    budget.entry(name, 0)?;
    let relative = relative_archive_path(name)?;
    let Some(name) = relative.file_name() else {
        return if directory {
            Ok(None)
        } else {
            Err(invalid("Empty archive file name"))
        };
    };
    Ok(Some(FileEntry {
        archive_path: Some(archive_path.clone()),
        name: name.to_string_lossy().into_owned(),
        path: archive_path.join(&relative).to_string_lossy().into_owned(),
        is_dir: directory,
        size: if directory { 0 } else { size },
        modified,
        ..FileEntry::default()
    }))
}

pub(super) fn indexed_files(
    path: &Path,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<Vec<FileEntry>, CommandError> {
    indexed_with_limits(path, ArchiveLimits::LISTING, checkpoint)
}

fn indexed_with_limits(
    path: &Path,
    limits: ArchiveLimits,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<Vec<FileEntry>, CommandError> {
    let parents = HeldParents::new(path)?;
    let resolved = parents.resolved_path(path)?;
    let file = platform().open_archive_source(&resolved, false)?;
    indexed_file(path, file, limits, checkpoint)
}

/// The isolated worker receives the selected read-only object, never opens a
/// document pathname, and shares the same parser and limits with local callers.
pub(super) fn indexed_file(
    path: &Path,
    file: File,
    limits: ArchiveLimits,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<Vec<FileEntry>, CommandError> {
    let budget = ArchiveBudget::new(limits);
    let archive_origin = std::sync::Arc::new(path.to_path_buf());
    budget.checkpoint(checkpoint)?;
    let format = ArchiveFormat::from_path(path)
        .filter(|format| {
            matches!(
                format,
                ArchiveFormat::Zip
                    | ArchiveFormat::Tar
                    | ArchiveFormat::TarGz
                    | ArchiveFormat::TarXz
                    | ArchiveFormat::TarBz2
                    | ArchiveFormat::TarZst
            )
        })
        .ok_or_else(|| invalid("Unsupported metadata format"))?;
    let source = Source {
        length: file.metadata()?.len(),
        file,
        budget: &budget,
        checkpoint,
    };
    if format == ArchiveFormat::Zip {
        let mut archive = zip::ZipArchive::new(IndexReader(BudgetReader {
            reader: source,
            budget: &budget,
            checkpoint,
        }))
        .map_err(|error| invalid(&error.to_string()))?;
        budget.check_entry_count(archive.len())?;
        let mut files = Vec::new();
        for index in 0..archive.len() {
            budget.checkpoint(checkpoint)?;
            // Raw access validates the local header without constructing a decoder.
            let file = archive
                .by_index_raw(index)
                .map_err(|error| invalid(&error.to_string()))?;
            let mode = file.unix_mode().unwrap_or(0) & 0o170000;
            if !matches!(mode, 0 | 0o100000 | 0o040000) {
                return Err(invalid("Archive links and special files are not supported"));
            }
            let modified = file
                .last_modified()
                .and_then(|dt| chrono::NaiveDateTime::try_from(dt).ok())
                .and_then(|dt| u64::try_from(dt.and_utc().timestamp_millis()).ok())
                .unwrap_or(0);
            if let Some(entry) = member(
                &archive_origin,
                &file.name().map_err(|error| invalid(&error.to_string()))?,
                file.is_dir(),
                file.size(),
                modified,
                &budget,
            )? {
                files.push(entry);
            }
        }
        budget.checkpoint(checkpoint)?;
        return Ok(files);
    }
    let files = if format == ArchiveFormat::Tar {
        let mut archive = tar::Archive::new(IndexReader(BudgetReader {
            reader: source,
            budget: &budget,
            checkpoint,
        }));
        tar_files(&archive_origin, archive.entries_with_seek()?, &budget, checkpoint)?
    } else {
        let decoder: Box<dyn Read + '_> = match format {
            ArchiveFormat::TarGz => Box::new(flate2::read::GzDecoder::new(source)),
            ArchiveFormat::TarXz => Box::new(xz2::read::XzDecoder::new(source)),
            ArchiveFormat::TarBz2 => Box::new(bzip2::read::BzDecoder::new(source)),
            ArchiveFormat::TarZst => Box::new(zstd::stream::read::Decoder::new(source)?),
            _ => return Err(invalid("Unsupported metadata format")),
        };
        let mut archive = tar::Archive::new(BudgetReader {
            reader: decoder,
            budget: &budget,
            checkpoint,
        });
        tar_files(&archive_origin, archive.entries()?, &budget, checkpoint)?
    };
    budget.checkpoint(checkpoint)?;
    Ok(files)
}

fn tar_files<R: Read>(
    archive_path: &std::sync::Arc<std::path::PathBuf>,
    entries: tar::Entries<'_, R>,
    budget: &ArchiveBudget,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<Vec<FileEntry>, CommandError> {
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry?;
        budget.checkpoint(checkpoint)?;
        let kind = entry.header().entry_type();
        if !kind.is_file() && !kind.is_dir() {
            return Err(invalid("Archive links and special files are not supported"));
        }
        let name = entry.path()?;
        let name = name
            .to_str()
            .ok_or_else(|| invalid("Invalid archive entry encoding"))?;
        let modified = entry
            .header()
            .mtime()?
            .checked_mul(1000)
            .ok_or_else(|| invalid("Invalid archive modification time"))?;
        if let Some(entry) = member(
            archive_path,
            name,
            kind.is_dir(),
            entry.size(),
            modified,
            budget,
        )? {
            files.push(entry);
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::error::{ArchiveLimitFailure, ArchiveLimitKind};
    use std::{cell::Cell, io::Write, time::Duration};

    const EXTENSIONS: [&str; 6] = ["zip", "tar", "tar.gz", "tar.xz", "tar.zst", "tar.bz2"];

    fn fixture(path: &Path, names: &[&str], payload: &[u8]) {
        if path.extension().is_some_and(|extension| extension == "zip") {
            let mut writer = zip::ZipWriter::new(File::create(path).unwrap());
            let options = zip::write::SimpleFileOptions::default().last_modified_time(
                zip::DateTime::from_date_and_time(2009, 2, 13, 23, 31, 30).unwrap(),
            );
            for name in names {
                if name.ends_with('/') {
                    writer.add_directory(*name, options).unwrap();
                } else {
                    writer.start_file(*name, options).unwrap();
                    writer.write_all(payload).unwrap();
                }
            }
            writer.finish().unwrap();
            return;
        }
        let mut builder = tar::Builder::new(Vec::new());
        for name in names {
            let mut header = tar::Header::new_gnu();
            header.as_mut_bytes()[..name.len()].copy_from_slice(name.as_bytes());
            let directory = name.ends_with('/');
            header.set_entry_type(if directory {
                tar::EntryType::Directory
            } else {
                tar::EntryType::Regular
            });
            header.set_mode(0o644);
            header.set_size(if directory { 0 } else { payload.len() as u64 });
            header.set_mtime(1_234_567_890);
            header.set_cksum();
            builder
                .append(&header, if directory { &[][..] } else { payload })
                .unwrap();
        }
        write_tar(path, &builder.into_inner().unwrap());
    }

    fn write_tar(path: &Path, bytes: &[u8]) {
        let mut output = Vec::new();
        match ArchiveFormat::from_path(path).unwrap() {
            ArchiveFormat::Tar => output.extend_from_slice(bytes),
            ArchiveFormat::TarZst => output = zstd::stream::encode_all(bytes, 1).unwrap(),
            format => {
                let mut encoder: Box<dyn Read + '_> = match format {
                    ArchiveFormat::TarGz => Box::new(flate2::read::GzEncoder::new(
                        bytes,
                        flate2::Compression::fast(),
                    )),
                    ArchiveFormat::TarXz => Box::new(xz2::read::XzEncoder::new(bytes, 1)),
                    ArchiveFormat::TarBz2 => Box::new(bzip2::read::BzEncoder::new(
                        bytes,
                        bzip2::Compression::fast(),
                    )),
                    _ => panic!("unsupported fixture"),
                };
                encoder.read_to_end(&mut output).unwrap();
            }
        }
        std::fs::write(path, output).unwrap();
    }

    fn marker(error: CommandError) -> ArchiveLimitFailure {
        ArchiveLimitFailure::from_message(&error.to_string()).unwrap()
    }

    #[test]
    fn metadata_limits_preserve_entry_depth_and_deadline_markers() {
        let temp = tempfile::tempdir().unwrap();
        for extension in EXTENSIONS {
            let path = temp.path().join(format!("limits.{extension}"));
            fixture(&path, &["one.txt", "nested/two.txt"], b"safe");
            for (limits, kind, limit) in [
                (
                    ArchiveLimits {
                        max_entries: 1,
                        ..ArchiveLimits::LISTING
                    },
                    ArchiveLimitKind::Entries,
                    1,
                ),
                (
                    ArchiveLimits {
                        max_depth: 1,
                        ..ArchiveLimits::LISTING
                    },
                    ArchiveLimitKind::Depth,
                    1,
                ),
                (
                    ArchiveLimits {
                        max_duration: Duration::ZERO,
                        ..ArchiveLimits::LISTING
                    },
                    ArchiveLimitKind::Seconds,
                    0,
                ),
            ] {
                assert_eq!(
                    marker(indexed_with_limits(&path, limits, &|| Ok(())).unwrap_err()),
                    ArchiveLimitFailure { kind, limit },
                    "{extension}"
                );
            }
        }
    }

    #[test]
    fn metadata_reader_cancellation_is_terminal_during_parsing() {
        let temp = tempfile::tempdir().unwrap();
        for extension in EXTENSIONS {
            let path = temp.path().join(format!("cancel.{extension}"));
            fixture(&path, &["one.txt", "two.txt"], b"safe");
            let calls = Cell::new(0);
            let result = indexed_files(&path, &|| {
                calls.set(calls.get() + 1);
                if calls.get() == 4 {
                    Err(io::Error::new(
                        io::ErrorKind::Interrupted,
                        "metadata cancelled once",
                    ))
                } else {
                    Ok(())
                }
            });
            assert!(
                result.is_err(),
                "{extension} swallowed cancellation after {} calls",
                calls.get()
            );
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("metadata cancelled once"),
                "{extension}"
            );
            assert_eq!(calls.get(), 4, "{extension} retried cancellation");
        }
    }

    #[test]
    fn metadata_compressed_tar_payload_traversal_is_bounded() {
        let temp = tempfile::tempdir().unwrap();
        let limits = ArchiveLimits {
            max_bytes: 0,
            max_entries: 4,
            ..ArchiveLimits::LISTING
        };
        for extension in &EXTENSIONS[1..] {
            let path = temp.path().join(format!("bytes.{extension}"));
            fixture(&path, &["large.txt"], &vec![b'a'; 20_000]);
            let result = indexed_with_limits(&path, limits, &|| Ok(()));
            if *extension == "tar" {
                // Seeking over valid payloads does not consume a decoded-byte budget.
                assert_eq!(result.unwrap()[0].size, 20_000);
            } else {
                assert_eq!(
                    marker(result.unwrap_err()),
                    ArchiveLimitFailure {
                        kind: ArchiveLimitKind::Decoded,
                        limit: 4 * 4096
                    },
                    "{extension}"
                );
            }
        }
    }

    #[test]
    fn metadata_truncated_tar_does_not_return_a_partial_index() {
        let temp = tempfile::tempdir().unwrap();
        let mut header = tar::Header::new_gnu();
        header.set_path("missing.txt").unwrap();
        header.set_size(4096);
        header.set_mode(0o644);
        header.set_mtime(123);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        for extension in &EXTENSIONS[1..] {
            let path = temp.path().join(format!("truncated.{extension}"));
            write_tar(&path, header.as_bytes());
            assert!(
                indexed_files(&path, &|| Ok(())).is_err(),
                "{extension} accepted missing payload"
            );
        }
    }

    #[test]
    fn metadata_navigation_normalizes_paths_and_retains_dates_sizes_and_empty_files() {
        let temp = tempfile::tempdir().unwrap();
        for extension in EXTENSIONS {
            let path = temp.path().join(format!("valid.{extension}"));
            fixture(&path, &["./", "folder/", "folder\\Été.txt"], b"safe");
            let index = indexed_files(&path, &|| Ok(())).unwrap();
            assert_eq!(index.len(), 2, "{extension}");
            let file = index.iter().find(|entry| !entry.is_dir).unwrap();
            assert_eq!(Path::new(&file.path), path.join("folder/Été.txt"));
            assert_eq!(file.size, 4);
            assert_eq!(file.modified, 1_234_567_890_000);
            let listing = super::super::archive::list_archive_contents(
                path.to_string_lossy().into_owned(),
                ".\\folder\\".into(),
            )
            .unwrap();
            assert_eq!(listing.len(), 1);
            assert_eq!(listing[0].name, "Été.txt");
            fixture(&path, &["empty.txt"], b"");
            assert_eq!(indexed_files(&path, &|| Ok(())).unwrap()[0].size, 0);
        }
    }
}
