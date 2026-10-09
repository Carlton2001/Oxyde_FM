//! Prepare one TAR member without extracting unrelated members to disk.
use crate::models::CommandError;
use crate::platform::platform;
use crate::systems::path_parents::HeldParents;
use crate::utils::archive::ArchiveFormat;
use crate::utils::archive_limits::{ArchiveBudget, ArchiveLimits, BudgetReader};
use crate::utils::archive_security::{relative_archive_path, ExtractionRoot};
use std::{
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

struct SeekReader<'a> {
    inner: BudgetReader<'a, File>,
    length: u64,
}

impl Read for SeekReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buffer)
    }
}

impl Seek for SeekReader<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.inner.budget.reader_checkpoint(self.inner.checkpoint)?;
        let position = self.inner.reader.seek(position)?;
        self.inner.budget.check_time()?;
        // tar's seek iterator can otherwise skip a truncated payload past EOF.
        if position > self.length {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Truncated TAR member",
            ));
        }
        Ok(position)
    }
}

fn archive_error(message: &str) -> CommandError {
    CommandError::ArchiveError(message.into())
}

struct Selection {
    index: usize,
    size: u64,
    hash: blake3::Hash,
}

struct HashingWriter<'a> {
    output: &'a mut File,
    hash: blake3::Hasher,
}
impl Write for HashingWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let count = self.output.write(buffer)?;
        self.hash.update(&buffer[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}

fn preflight<R: Read>(
    entries: tar::Entries<'_, R>,
    requested: &Path,
    budget: &ArchiveBudget,
) -> Result<Selection, CommandError> {
    let mut selected = None;
    for (index, entry) in entries.enumerate() {
        let mut entry = entry?;
        budget.check_time()?;
        let kind = entry.header().entry_type();
        if !kind.is_file() && !kind.is_dir() {
            return Err(archive_error(
                "Archive links and special files are not supported",
            ));
        }
        let name = entry.path()?;
        let name = name
            .to_str()
            .ok_or_else(|| archive_error("Invalid archive entry encoding"))?;
        budget.entry(name, 0)?;
        if relative_archive_path(name)? == requested {
            if selected.is_some() || !kind.is_file() {
                return Err(archive_error("Ambiguous archive file"));
            }
            budget.declare_bytes(entry.size())?;
            let size = entry.size();
            let mut hash = blake3::Hasher::new();
            if io::copy(&mut entry, &mut hash)? != size {
                return Err(
                    io::Error::new(io::ErrorKind::UnexpectedEof, "Truncated TAR member").into(),
                );
            }
            selected = Some(Selection {
                index,
                size,
                hash: hash.finalize(),
            });
        }
    }
    selected.ok_or_else(|| archive_error("Archive file not found"))
}

fn copy_selected<R: Read>(
    mut entries: tar::Entries<'_, R>,
    selected: &Selection,
    requested: &Path,
    root: &ExtractionRoot,
    budget: &ArchiveBudget,
    provenance: Option<&[u8]>,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<PathBuf, CommandError> {
    // Iterator::skip would discard errors while crossing preceding members,
    // hiding decoded/deadline failures behind "file not found".
    for _ in 0..selected.index {
        entries
            .next()
            .ok_or_else(|| archive_error("Archive file not found"))??;
    }
    let mut entry = entries
        .next()
        .ok_or_else(|| archive_error("Archive file not found"))??;
    let name = entry.path()?;
    let name = name
        .to_str()
        .ok_or_else(|| archive_error("Invalid archive entry encoding"))?;
    if !entry.header().entry_type().is_file()
        || relative_archive_path(name)? != requested
        || entry.size() != selected.size
    {
        return Err(archive_error(
            &crate::models::error::ArchiveSourceConflict.to_string(),
        ));
    }
    // Keep TAR's existing externally opened filename, without unrelated folders.
    let name = requested
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| archive_error("Archive item is not a file"))?;
    let destination = root.destination(name, false)?;
    let mut output = platform().create_archive_file(&destination.path)?;
    let mut writer = HashingWriter {
        output: &mut output,
        hash: blake3::Hasher::new(),
    };
    let copied = budget.copy(&mut entry, &mut writer, checkpoint)?;
    if copied != entry.size() {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Truncated TAR member").into());
    }
    if writer.hash.finalize() != selected.hash {
        return Err(archive_error(
            &crate::models::error::ArchiveSourceConflict.to_string(),
        ));
    }
    if let Ok(seconds) = entry.header().mtime() {
        if let Some(modified) =
            std::time::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(seconds))
        {
            output.set_times(fs::FileTimes::new().set_modified(modified))?;
        }
    }
    if let Some(data) = provenance {
        platform().write_file_provenance(&destination.path, data)?;
    }
    Ok(destination.path)
}

pub(crate) fn extract_tar_file_for_open(
    archive_path: &Path,
    internal: &str,
    target: &Path,
    limits: ArchiveLimits,
) -> Result<PathBuf, CommandError> {
    extract_with_checkpoint(archive_path, internal, target, limits, &|| Ok(()))
}

fn compressed_reader(source: File, format: &ArchiveFormat) -> io::Result<Box<dyn Read>> {
    Ok(match format {
        ArchiveFormat::TarGz => Box::new(flate2::read::GzDecoder::new(source)),
        ArchiveFormat::TarXz => Box::new(xz2::read::XzDecoder::new(source)),
        ArchiveFormat::TarBz2 => Box::new(bzip2::read::BzDecoder::new(source)),
        ArchiveFormat::TarZst => Box::new(zstd::stream::read::Decoder::new(source)?),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Unsupported compressed TAR format",
            ))
        }
    })
}

fn extract_with_checkpoint(
    archive_path: &Path,
    internal: &str,
    target: &Path,
    limits: ArchiveLimits,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<PathBuf, CommandError> {
    let requested = relative_archive_path(internal)?;
    if requested.as_os_str().is_empty() {
        return Err(archive_error("Archive item is not a file"));
    }
    let format = ArchiveFormat::from_path(archive_path)
        .filter(|format| {
            matches!(
                format,
                ArchiveFormat::Tar
                    | ArchiveFormat::TarGz
                    | ArchiveFormat::TarXz
                    | ArchiveFormat::TarBz2
                    | ArchiveFormat::TarZst
            )
        })
        .ok_or_else(|| archive_error("Unsupported targeted TAR format"))?;
    let parents = HeldParents::new(archive_path)?;
    let archive_path = parents.resolved_path(archive_path)?;
    // Both passes use this object. Windows refuses incompatible writers/replacement.
    let mut source = platform().open_archive_source(&archive_path, false)?;
    let length = source.metadata()?.len();
    let budget = ArchiveBudget::new(limits);
    let selected = if format == ArchiveFormat::Tar {
        let mut archive = tar::Archive::new(SeekReader {
            inner: BudgetReader {
                reader: source.try_clone()?,
                budget: &budget,
                checkpoint,
            },
            length,
        });
        preflight(archive.entries_with_seek()?, &requested, &budget)?
    } else {
        let decoder = compressed_reader(source.try_clone()?, &format)?;
        let mut archive = tar::Archive::new(BudgetReader {
            reader: decoder,
            budget: &budget,
            checkpoint,
        });
        preflight(archive.entries()?, &requested, &budget)?
    };
    budget.checkpoint(checkpoint)?;
    source.seek(SeekFrom::Start(0))?;
    let provenance = platform().read_file_provenance(&archive_path)?;
    let root = ExtractionRoot::new(target)?;
    if format == ArchiveFormat::Tar {
        let mut archive = tar::Archive::new(SeekReader {
            inner: BudgetReader {
                reader: source,
                budget: &budget,
                checkpoint,
            },
            length,
        });
        copy_selected(
            archive.entries_with_seek()?,
            &selected,
            &requested,
            &root,
            &budget,
            provenance.as_deref(),
            checkpoint,
        )
    } else {
        // Each traversal has its own decoded counter; one deadline covers both.
        // Written/declared bytes still belong to the single requested-file budget.
        let decoded = ArchiveBudget::new(limits);
        let decoded_checkpoint = || budget.checkpoint(checkpoint);
        let decoder = compressed_reader(source, &format)?;
        let mut archive = tar::Archive::new(BudgetReader {
            reader: decoder,
            budget: &decoded,
            checkpoint: &decoded_checkpoint,
        });
        copy_selected(
            archive.entries()?,
            &selected,
            &requested,
            &root,
            &budget,
            provenance.as_deref(),
            checkpoint,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::error::{ArchiveLimitFailure, ArchiveLimitKind};

    const TAR_EXTENSIONS: [&str; 5] = ["tar", "tar.zst", "tar.gz", "tar.xz", "tar.bz2"];

    fn tar_bytes(entries: &[(&str, tar::EntryType, &[u8])]) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        for (name, kind, data) in entries {
            let mut header = tar::Header::new_gnu();
            let bytes = name.as_bytes();
            assert!(bytes.len() < 100);
            header.as_mut_bytes()[..bytes.len()].copy_from_slice(bytes);
            header.set_entry_type(*kind);
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_mtime(1_234_567_890);
            header.set_cksum();
            builder.append(&header, *data).unwrap();
        }
        builder.into_inner().unwrap()
    }

    fn write_archive(folder: &Path, extension: &str, bytes: &[u8]) -> PathBuf {
        let path = folder.join(format!("selection.{extension}"));
        let contents = match ArchiveFormat::from_path(&path).unwrap() {
            ArchiveFormat::Tar => bytes.to_vec(),
            ArchiveFormat::TarZst => zstd::stream::encode_all(bytes, 1).unwrap(),
            format => {
                let mut reader: Box<dyn Read + '_> = match format {
                    ArchiveFormat::TarGz => Box::new(flate2::read::GzEncoder::new(
                        bytes,
                        flate2::Compression::fast(),
                    )),
                    ArchiveFormat::TarXz => Box::new(xz2::read::XzEncoder::new(bytes, 1)),
                    ArchiveFormat::TarBz2 => Box::new(bzip2::read::BzEncoder::new(
                        bytes,
                        bzip2::Compression::fast(),
                    )),
                    _ => panic!("Unsupported fixture format"),
                };
                let mut contents = Vec::new();
                reader.read_to_end(&mut contents).unwrap();
                contents
            }
        };
        fs::write(&path, contents).unwrap();
        path
    }

    fn limits() -> ArchiveLimits {
        ArchiveLimits {
            max_bytes: 4,
            max_entries: 4,
            ..ArchiveLimits::OPEN_FILE
        }
    }

    #[test]
    fn retrying_tar_header_reads_do_not_swallow_a_checkpoint_cancellation() {
        let bytes = tar_bytes(&[("selected.txt", tar::EntryType::Regular, b"safe")]);
        let mut observed = Vec::new();
        for extension in TAR_EXTENSIONS {
            let temp = tempfile::tempdir().unwrap();
            let archive = write_archive(temp.path(), extension, &bytes);
            let target = tempfile::tempdir().unwrap();
            let calls = std::cell::Cell::new(0);
            let result =
                extract_with_checkpoint(&archive, "selected.txt", target.path(), limits(), &|| {
                    calls.set(calls.get() + 1);
                    if calls.get() == 1 {
                        Err(io::Error::new(
                            io::ErrorKind::Interrupted,
                            "cancelled in TAR header",
                        ))
                    } else {
                        Ok(())
                    }
                });
            if let Err(error) = &result {
                assert!(error.to_string().contains("cancelled in TAR header"));
            }
            observed.push((
                extension,
                result.is_err(),
                calls.get(),
                fs::read_dir(target.path()).unwrap().count(),
            ));
        }
        assert!(
            observed
                .iter()
                .all(|(_, failed, calls, outputs)| *failed && *calls == 1 && *outputs == 0),
            "TAR preparation continued after cancellation: {observed:?}"
        );
    }

    #[test]
    fn targeted_tar_open_accepts_compressed_aliases_without_extracting_neighbors() {
        let bytes = tar_bytes(&[
            ("selected.txt", tar::EntryType::Regular, b"safe"),
            (
                "other.txt",
                tar::EntryType::Regular,
                b"over the selection limit",
            ),
        ]);
        for extension in ["tgz", "txz", "tbz2", "tzst", "TAR.GZ", "TAR.XZ", "TAR.BZ2"] {
            let fixture = tempfile::tempdir().unwrap();
            let archive = write_archive(fixture.path(), extension, &bytes);
            let target = tempfile::tempdir().unwrap();
            let output =
                extract_tar_file_for_open(&archive, "selected.txt", target.path(), limits())
                    .unwrap();
            assert_eq!(fs::read(output).unwrap(), b"safe");
            assert_eq!(fs::read_dir(target.path()).unwrap().count(), 1);
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn targeted_tar_open_preserves_internet_provenance_for_every_codec() {
        let bytes = tar_bytes(&[("selected.txt", tar::EntryType::Regular, b"safe")]);
        let provenance =
            b"[ZoneTransfer]\r\nZoneId=3\r\nHostUrl=https://example.invalid/archive\r\n";
        for extension in TAR_EXTENSIONS {
            let fixture = tempfile::tempdir().unwrap();
            let archive = write_archive(fixture.path(), extension, &bytes);
            platform()
                .write_file_provenance(&archive, provenance)
                .unwrap();
            let target = tempfile::tempdir().unwrap();
            let output =
                extract_tar_file_for_open(&archive, "selected.txt", target.path(), limits())
                    .unwrap();
            assert_eq!(
                platform().read_file_provenance(&output).unwrap().as_deref(),
                Some(&provenance[..])
            );
            assert_eq!(fs::read(output).unwrap(), b"safe");
        }
    }

    #[test]
    fn targeted_tar_open_writes_only_the_selected_unicode_member_before_or_after_other_data() {
        for extension in TAR_EXTENSIONS {
            for reverse in [false, true] {
                let folder = tempfile::tempdir().unwrap();
                let mut entries = vec![
                    ("école/文件.txt", tar::EntryType::Regular, &b"safe"[..]),
                    (
                        "unrelated/large.txt",
                        tar::EntryType::Regular,
                        &b"unselected and over the file limit"[..],
                    ),
                ];
                if reverse {
                    entries.reverse();
                }
                let archive = write_archive(folder.path(), extension, &tar_bytes(&entries));
                let target = tempfile::tempdir().unwrap();
                let output =
                    extract_tar_file_for_open(&archive, "école\\文件.txt", target.path(), limits())
                        .unwrap();
                assert_eq!(output, target.path().join("文件.txt"));
                assert_eq!(fs::read(&output).unwrap(), b"safe");
                assert_eq!(
                    fs::metadata(&output).unwrap().modified().unwrap(),
                    std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_234_567_890)
                );
                assert_eq!(fs::read_dir(target.path()).unwrap().count(), 1);
                let refused = tempfile::tempdir().unwrap();
                let error = extract_tar_file_for_open(
                    &archive,
                    "unrelated/large.txt",
                    refused.path(),
                    limits(),
                )
                .unwrap_err();
                assert_eq!(
                    ArchiveLimitFailure::from_message(&error.to_string())
                        .unwrap()
                        .kind,
                    ArchiveLimitKind::Bytes
                );
                assert_eq!(fs::read_dir(refused.path()).unwrap().count(), 0);
            }
        }
    }

    #[test]
    fn targeted_tar_open_rejects_late_unsafe_names_and_special_entries_without_output() {
        for extension in TAR_EXTENSIONS {
            for (name, kind) in [
                ("../outside", tar::EntryType::Regular),
                ("file:stream", tar::EntryType::Regular),
                ("C:victim.txt", tar::EntryType::Regular),
                ("NUL.txt", tar::EntryType::Regular),
                ("link", tar::EntryType::Symlink),
                ("hardlink", tar::EntryType::Link),
                ("fifo", tar::EntryType::Fifo),
            ] {
                let folder = tempfile::tempdir().unwrap();
                let archive = write_archive(
                    folder.path(),
                    extension,
                    &tar_bytes(&[
                        ("selected.txt", tar::EntryType::Regular, b"safe"),
                        (name, kind, b""),
                    ]),
                );
                let target = tempfile::tempdir().unwrap();
                assert!(
                    extract_tar_file_for_open(&archive, "selected.txt", target.path(), limits())
                        .is_err(),
                    "accepted {name} in {extension}"
                );
                assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
            }
        }
    }

    #[test]
    fn targeted_tar_open_rejects_ambiguous_absent_and_directory_selections() {
        for extension in TAR_EXTENSIONS {
            for entries in [
                vec![
                    ("a/b.txt", tar::EntryType::Regular, &b"safe"[..]),
                    ("a\\b.txt", tar::EntryType::Regular, &b"safe"[..]),
                ],
                vec![("a/b.txt", tar::EntryType::Directory, &b""[..])],
                vec![("other.txt", tar::EntryType::Regular, &b"safe"[..])],
            ] {
                let folder = tempfile::tempdir().unwrap();
                let archive = write_archive(folder.path(), extension, &tar_bytes(&entries));
                let target = tempfile::tempdir().unwrap();
                assert!(
                    extract_tar_file_for_open(&archive, "a/b.txt", target.path(), limits())
                        .is_err()
                );
                assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
            }
        }
    }

    #[test]
    fn targeted_tar_open_keeps_entry_depth_and_deadline_failures_structured() {
        let bytes = tar_bytes(&[
            ("selected.txt", tar::EntryType::Regular, b"safe"),
            ("a/b/c", tar::EntryType::Regular, b""),
        ]);
        for extension in TAR_EXTENSIONS {
            let folder = tempfile::tempdir().unwrap();
            let archive = write_archive(folder.path(), extension, &bytes);
            for (limits, kind) in [
                (
                    ArchiveLimits {
                        max_entries: 1,
                        ..limits()
                    },
                    ArchiveLimitKind::Entries,
                ),
                (
                    ArchiveLimits {
                        max_depth: 2,
                        ..limits()
                    },
                    ArchiveLimitKind::Depth,
                ),
                (
                    ArchiveLimits {
                        max_duration: std::time::Duration::ZERO,
                        ..limits()
                    },
                    ArchiveLimitKind::Seconds,
                ),
            ] {
                let target = tempfile::tempdir().unwrap();
                let error =
                    extract_tar_file_for_open(&archive, "selected.txt", target.path(), limits)
                        .unwrap_err();
                assert_eq!(
                    ArchiveLimitFailure::from_message(&error.to_string())
                        .unwrap()
                        .kind,
                    kind
                );
                assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
            }
        }
    }

    #[test]
    fn targeted_tar_open_skips_plain_payload_but_bounds_compressed_decoded_streams() {
        let large = vec![0; 16 * 1024];
        let bytes = tar_bytes(&[
            ("selected.txt", tar::EntryType::Regular, b"safe"),
            ("large.txt", tar::EntryType::Regular, &large),
        ]);
        let folder = tempfile::tempdir().unwrap();
        let plain = write_archive(folder.path(), "tar", &bytes);
        let tiny = ArchiveLimits {
            max_entries: 2,
            ..limits()
        };
        let target = tempfile::tempdir().unwrap();
        assert_eq!(
            fs::read(
                extract_tar_file_for_open(&plain, "selected.txt", target.path(), tiny).unwrap()
            )
            .unwrap(),
            b"safe"
        );
        for extension in &TAR_EXTENSIONS[1..] {
            let compressed = write_archive(folder.path(), extension, &bytes);
            let refused = tempfile::tempdir().unwrap();
            let error =
                extract_tar_file_for_open(&compressed, "selected.txt", refused.path(), tiny)
                    .unwrap_err();
            let failure = ArchiveLimitFailure::from_message(&error.to_string()).unwrap();
            assert_eq!(failure.kind, ArchiveLimitKind::Decoded, "{extension}");
            assert_eq!(failure.limit, 4 + 2 * 4096);
            assert_eq!(fs::read_dir(refused.path()).unwrap().count(), 0);
        }
    }

    #[test]
    fn targeted_tar_open_refuses_truncated_unselected_payload_before_creating_output() {
        let large = vec![0; 2048];
        let mut bytes = tar_bytes(&[
            ("selected.txt", tar::EntryType::Regular, b"safe"),
            ("large.txt", tar::EntryType::Regular, &large),
        ]);
        bytes.truncate(1536 + 3);
        for extension in TAR_EXTENSIONS {
            let folder = tempfile::tempdir().unwrap();
            let archive = write_archive(folder.path(), extension, &bytes);
            let target = tempfile::tempdir().unwrap();
            assert!(
                extract_tar_file_for_open(&archive, "selected.txt", target.path(), limits())
                    .is_err()
            );
            assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
        }
    }

    #[test]
    fn targeted_tar_open_refuses_truncated_compressed_streams_before_creating_output() {
        let mut state = 1u32;
        let large: Vec<u8> = (0..64 * 1024)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect();
        let bytes = tar_bytes(&[
            ("selected.txt", tar::EntryType::Regular, b"safe"),
            ("large.txt", tar::EntryType::Regular, &large),
        ]);
        // Enough decoded allowance to reach the broken stream, rather than
        // accidentally testing only the decoded-byte ceiling again.
        let allowance = ArchiveLimits {
            max_entries: 32,
            ..limits()
        };
        for extension in &TAR_EXTENSIONS[1..] {
            for header_only in [false, true] {
                let fixture = tempfile::tempdir().unwrap();
                let archive = write_archive(fixture.path(), extension, &bytes);
                let mut compressed = fs::read(&archive).unwrap();
                compressed.truncate(if header_only { 8 } else { compressed.len() / 2 });
                fs::write(&archive, compressed).unwrap();
                let target = tempfile::tempdir().unwrap();
                let error =
                    extract_tar_file_for_open(&archive, "selected.txt", target.path(), allowance)
                        .unwrap_err();
                assert!(
                    ArchiveLimitFailure::from_message(&error.to_string()).is_none(),
                    "{extension}: {error}"
                );
                assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
            }
        }
    }

    #[test]
    fn targeted_tar_open_never_overwrites_an_existing_destination() {
        for extension in TAR_EXTENSIONS {
            let folder = tempfile::tempdir().unwrap();
            let archive = write_archive(
                folder.path(),
                extension,
                &tar_bytes(&[("folder/selected.txt", tar::EntryType::Regular, b"safe")]),
            );
            let target = tempfile::tempdir().unwrap();
            fs::write(target.path().join("selected.txt"), b"foreign").unwrap();
            assert!(extract_tar_file_for_open(
                &archive,
                "folder/selected.txt",
                target.path(),
                limits()
            )
            .is_err());
            assert_eq!(
                fs::read(target.path().join("selected.txt")).unwrap(),
                b"foreign"
            );
            assert_eq!(fs::read_dir(target.path()).unwrap().count(), 1);
        }
    }

    #[test]
    fn targeted_tar_open_preserves_a_deadline_failure_before_the_selected_replay_entry() {
        let folder = tempfile::tempdir().unwrap();
        let bytes = tar_bytes(&[
            ("other.txt", tar::EntryType::Regular, b"safe"),
            ("selected.txt", tar::EntryType::Regular, b"safe"),
        ]);
        let archive = write_archive(folder.path(), "tar", &bytes);
        let target = tempfile::tempdir().unwrap();
        let budget = ArchiveBudget::new(ArchiveLimits {
            max_duration: std::time::Duration::ZERO,
            ..limits()
        });
        let selected = Selection {
            index: 1,
            size: 4,
            hash: blake3::hash(b"safe"),
        };
        let mut archive = tar::Archive::new(SeekReader {
            inner: BudgetReader {
                reader: File::open(archive).unwrap(),
                budget: &budget,
                checkpoint: &|| Ok(()),
            },
            length: bytes.len() as u64,
        });
        let root = ExtractionRoot::new(target.path()).unwrap();
        let error = copy_selected(
            archive.entries_with_seek().unwrap(),
            &selected,
            Path::new("selected.txt"),
            &root,
            &budget,
            None,
            &|| Ok(()),
        )
        .unwrap_err();
        assert_eq!(
            ArchiveLimitFailure::from_message(&error.to_string())
                .unwrap()
                .kind,
            ArchiveLimitKind::Seconds
        );
        assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn targeted_tar_open_keeps_the_source_object_after_parent_junction_retargeting() {
        use crate::platform::windows_reparse_tests::{junction, Retarget};
        use std::cell::Cell;
        for extension in TAR_EXTENSIONS {
            let folder = tempfile::tempdir().unwrap();
            let chosen = folder.path().join("chosen");
            let outside = folder.path().join("outside");
            fs::create_dir(&chosen).unwrap();
            fs::create_dir(&outside).unwrap();
            let archive = write_archive(
                &chosen,
                extension,
                &tar_bytes(&[("selected.txt", tar::EntryType::Regular, b"safe")]),
            );
            let foreign = write_archive(
                &outside,
                extension,
                &tar_bytes(&[("selected.txt", tar::EntryType::Regular, b"evil")]),
            );
            let original_foreign = fs::read(&foreign).unwrap();
            let link = folder.path().join("link");
            let template = folder.path().join("template");
            junction(&link, &chosen);
            junction(&template, &outside);
            let retarget = Retarget::new(&link, &template);
            let applied = Cell::new(false);
            let target = tempfile::tempdir().unwrap();
            let output = extract_with_checkpoint(
                &link.join(archive.file_name().unwrap()),
                "selected.txt",
                target.path(),
                limits(),
                &|| {
                    if !applied.replace(true) {
                        assert!(
                            File::options().write(true).open(&archive).is_err(),
                            "held source must reject a writer"
                        );
                        assert!(
                            fs::rename(&archive, chosen.join("replaced")).is_err(),
                            "held source must reject replacement"
                        );
                        retarget.apply();
                    }
                    Ok(())
                },
            )
            .unwrap();
            assert!(applied.get());
            assert_eq!(fs::read(output).unwrap(), b"safe");
            assert_eq!(fs::read(foreign).unwrap(), original_foreign);
            drop(retarget);
            fs::remove_dir(link).unwrap();
            fs::remove_dir(template).unwrap();
            assert!(File::options().write(true).open(&archive).is_ok());
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn targeted_tar_open_refuses_a_new_destination_junction_without_touching_its_target() {
        use crate::platform::windows_reparse_tests::{junction, Retarget};
        use std::cell::Cell;
        for extension in TAR_EXTENSIONS {
            let folder = tempfile::tempdir().unwrap();
            let archive = write_archive(
                folder.path(),
                extension,
                &tar_bytes(&[("selected.txt", tar::EntryType::Regular, b"safe")]),
            );
            let outside = folder.path().join("outside");
            let target = folder.path().join("target");
            fs::create_dir(&outside).unwrap();
            fs::create_dir(&target).unwrap();
            fs::write(outside.join("selected.txt"), b"foreign witness").unwrap();
            let template = folder.path().join("template");
            junction(&template, &outside);
            let retarget = Retarget::new(&target, &template);
            let applied = Cell::new(false);
            let result =
                extract_with_checkpoint(&archive, "selected.txt", &target, limits(), &|| {
                    if !applied.replace(true) {
                        retarget.apply();
                    }
                    Ok(())
                });
            assert!(applied.get());
            assert!(result.is_err());
            assert_eq!(
                fs::read(outside.join("selected.txt")).unwrap(),
                b"foreign witness"
            );
            assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
            retarget.clear();
            drop(retarget);
            fs::remove_dir(template).unwrap();
            assert_eq!(fs::read_dir(target).unwrap().count(), 0);
        }
    }
}
