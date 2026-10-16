//! Shared 7z metadata checks and targeted decoding of the selected compression block.
use crate::models::{CommandError, FileEntry};
use crate::platform::platform;
use crate::systems::path_parents::HeldParents;
use crate::utils::archive_limits::{ArchiveBudget, ArchiveLimits};
use crate::utils::archive_security::relative_archive_path;
#[cfg(any(not(target_os = "windows"), test))]
use crate::utils::{archive_limits::BudgetReader, archive_security::ExtractionRoot};
use sevenz_rust2::{Archive, ArchiveEntry, Password};
#[cfg(any(not(target_os = "windows"), test))]
use sevenz_rust2::{ArchiveReader, BlockDecoder};
use std::fs::File;
#[cfg(any(not(target_os = "windows"), test))]
use std::fs;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
#[cfg(any(not(target_os = "windows"), test))]
use std::path::PathBuf;

#[cfg(test)]
pub(crate) use tests::block_crc_fixture_with_method;

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub(super) fn archive_error(error: sevenz_rust2::Error) -> CommandError {
    // sevenz's Display delegates to Debug, which erases the Display marker of
    // typed IO causes such as ArchiveLimitFailure. Keep their original text.
    CommandError::ArchiveError(match error {
        sevenz_rust2::Error::Io(error, context) => {
            if context.is_empty() {
                error.to_string()
            } else {
                format!("{context}: {error}")
            }
        }
        sevenz_rust2::Error::FileOpen(error, context) => format!("{context}: {error}"),
        sevenz_rust2::Error::MaybeBadPassword(error) => {
            format!("Possibly bad archive password: {error}")
        }
        error => error.to_string(),
    })
}

pub(super) struct CheckedSource<'a> {
    file: File,
    budget: &'a ArchiveBudget,
    checkpoint: &'a dyn Fn() -> io::Result<()>,
}

#[cfg(any(not(target_os = "windows"), test))]
pub(super) fn checked_reader<'a>(
    file: File,
    budget: &'a ArchiveBudget,
    checkpoint: &'a dyn Fn() -> io::Result<()>,
) -> Result<ArchiveReader<CheckedSource<'a>>, CommandError> {
    ArchiveReader::new(
        CheckedSource {
            file,
            budget,
            checkpoint,
        },
        Password::empty(),
    )
    .map_err(archive_error)
}

pub(super) fn validate_entry_type(entry: &ArchiveEntry) -> io::Result<()> {
    let mode = (entry.windows_attributes >> 16) & 0o170000;
    if entry.is_anti_item
        || (entry.has_windows_attributes
            && (entry.windows_attributes & 0x400 != 0 || !matches!(mode, 0 | 0o100000 | 0o040000)))
    {
        return Err(invalid("Archive links and special files are not supported"));
    }
    Ok(())
}

/// Names and sizes come from the header, without constructing content decoders.
pub(super) fn indexed_files(
    archive_path: &Path,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<Vec<FileEntry>, CommandError> {
    indexed_with_limits(archive_path, ArchiveLimits::LISTING, checkpoint)
}

fn indexed_with_limits(
    archive_path: &Path,
    limits: ArchiveLimits,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<Vec<FileEntry>, CommandError> {
    let parents = HeldParents::new(archive_path)?;
    let resolved = parents.resolved_path(archive_path)?;
    indexed_file(archive_path, platform().open_archive_source(&resolved, false)?, limits, checkpoint)
}
pub(super) fn indexed_file(
    archive_path: &Path,
    file: File,
    limits: ArchiveLimits,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<Vec<FileEntry>, CommandError> {
    let budget = ArchiveBudget::new(limits);
    let mut source = CheckedSource {
        file,
        budget: &budget,
        checkpoint,
    };
    let archive = Archive::read(&mut source, &Password::empty()).map_err(archive_error)?;
    budget.check_entry_count(archive.files.len())?;
    let mut entries = Vec::new();
    let archive_origin = std::sync::Arc::new(archive_path.to_path_buf());
    for entry in &archive.files {
        budget.checkpoint(checkpoint)?;
        budget.entry(entry.name(), 0)?;
        validate_entry_type(entry)?;
        let relative = relative_archive_path(entry.name())?;
        if relative.as_os_str().is_empty() && entry.is_directory() {
            continue;
        }
        let name = relative
            .file_name()
            .ok_or_else(|| invalid("Invalid empty archive entry name"))?
            .to_string_lossy()
            .into_owned();
        entries.push(FileEntry {
            archive_path: Some(archive_origin.clone()),
            name,
            path: archive_path.join(relative).to_string_lossy().into_owned(),
            is_dir: entry.is_directory(),
            size: if entry.is_directory() {
                0
            } else {
                entry.size()
            },
            ..FileEntry::default()
        });
    }
    Ok(entries)
}
impl Read for CheckedSource<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.budget.reader_checkpoint(self.checkpoint)?;
        let count = self.file.read(bytes)?;
        self.budget.check_time()?;
        Ok(count)
    }
}
impl Seek for CheckedSource<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.budget.reader_checkpoint(self.checkpoint)?;
        let position = self.file.seek(position)?;
        self.budget.check_time()?;
        Ok(position)
    }
}

#[cfg(any(not(target_os = "windows"), test))]
fn selection(
    archive: &Archive,
    requested: &Path,
    budget: &ArchiveBudget,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<usize, CommandError> {
    budget.check_entry_count(archive.files.len())?;
    let mut selected = None;
    for (index, entry) in archive.files.iter().enumerate() {
        budget.checkpoint(checkpoint)?;
        budget.entry(entry.name(), 0)?;
        // These entries describe updates/links, not an ordinary file to open.
        validate_entry_type(entry)?;
        if relative_archive_path(entry.name())? == requested {
            if selected.is_some() || entry.is_directory() {
                return Err(invalid("Ambiguous archive file").into());
            }
            budget.declare_bytes(entry.size())?;
            selected = Some(index);
        }
    }
    selected.ok_or_else(|| invalid("Archive file not found").into())
}

#[cfg(any(not(target_os = "windows"), test))]
fn set_times(output: &File, entry: &ArchiveEntry) -> io::Result<()> {
    let mut times = fs::FileTimes::new();
    if entry.has_last_modified_date {
        times = times.set_modified(entry.last_modified_date().into());
    }
    if entry.has_access_date {
        times = times.set_accessed(entry.access_date().into());
    }
    output.set_times(times)
}

#[cfg(any(not(target_os = "windows"), test))]
pub(crate) fn extract_seven_file_for_open(
    archive: &Path,
    internal: &str,
    target: &Path,
    limits: ArchiveLimits,
) -> Result<PathBuf, CommandError> {
    extract_with_checkpoint(archive, internal, target, limits, &|| Ok(()))
}

#[cfg(any(not(target_os = "windows"), test))]
fn extract_with_checkpoint(
    archive: &Path,
    internal: &str,
    target: &Path,
    limits: ArchiveLimits,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<PathBuf, CommandError> {
    let requested = relative_archive_path(internal)?;
    if requested.as_os_str().is_empty() {
        return Err(invalid("Archive item is not a file").into());
    }
    let parents = HeldParents::new(archive)?;
    let archive_path = parents.resolved_path(archive)?;
    let budget = ArchiveBudget::new(limits);
    let mut source = CheckedSource {
        file: platform().open_archive_source(&archive_path, false)?,
        budget: &budget,
        checkpoint,
    };
    let password = Password::empty();
    let archive = Archive::read(&mut source, &password).map_err(archive_error)?;
    let index = selection(&archive, &requested, &budget, checkpoint)?;
    let selected = &archive.files[index];
    let block_index = if selected.has_stream {
        Some(
            archive
                .stream_map
                .file_block_index
                .get(index)
                .copied()
                .flatten()
                .filter(|block| *block < archive.blocks.len())
                .ok_or_else(|| invalid("Invalid 7z file block mapping"))?,
        )
    } else {
        if selected.size() != 0 {
            return Err(invalid("Invalid empty 7z file").into());
        }
        None
    };
    // Do not lose the selected file in a span containing non-stream records.
    // This library's block decoder uses a contiguous sub-stream-count slice.
    if let Some(block_index) = block_index {
        let decoder = BlockDecoder::new(1, block_index, &archive, &password, &mut source);
        if !decoder
            .entries()
            .iter()
            .any(|entry| std::ptr::eq(entry, selected))
        {
            return Err(invalid("Unsupported 7z compression block layout").into());
        }
    }
    budget.checkpoint(checkpoint)?;
    let provenance = platform().read_file_provenance(&archive_path)?;
    let root = ExtractionRoot::new(target)?;
    let basename = requested
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid("Archive item is not a file"))?;
    let destination = root.destination(basename, false)?;
    if let Some(block_index) = block_index {
        // A file without its own CRC can rely on the block's CRC instead.
        // In that case consume the rest of this block before accepting it.
        let verify_block_crc = !selected.has_crc && archive.blocks[block_index].has_crc;
        let mut selected_output = None;
        let mut decoded_bytes = 0u64;
        BlockDecoder::new(1, block_index, &archive, &password, &mut source)
            .for_each_entries(&mut |entry, reader| {
                budget.checkpoint(checkpoint)?;
                let mut reader = BudgetReader {
                    reader,
                    budget: &budget,
                    checkpoint,
                };
                if std::ptr::eq(entry, selected) {
                    let mut output = platform().create_archive_file(&destination.path)?;
                    let count = budget.copy(&mut reader, &mut output, checkpoint)?;
                    if count != entry.size() {
                        return Err(invalid("Truncated 7z file").into());
                    }
                    decoded_bytes += count;
                    selected_output = Some(output);
                    Ok(verify_block_crc)
                } else {
                    // Required block members are consumed without using the
                    // requested file's written-byte budget or allocating a Vec.
                    let count = io::copy(&mut reader, &mut io::sink())?;
                    if count != entry.size() {
                        return Err(invalid("Truncated 7z block member").into());
                    }
                    decoded_bytes += count;
                    Ok(true)
                }
            })
            .map_err(archive_error)?;
        if verify_block_crc && decoded_bytes != archive.blocks[block_index].get_unpack_size() {
            return Err(invalid("Incomplete 7z compression block").into());
        }
        let output = selected_output.ok_or_else(|| invalid("Archive file not found"))?;
        set_times(&output, selected)?;
        if let Some(data) = provenance.as_deref() {
            platform().write_file_provenance(&destination.path, data)?;
        }
    } else {
        let output = platform().create_archive_file(&destination.path)?;
        set_times(&output, selected)?;
        if let Some(data) = provenance {
            platform().write_file_provenance(&destination.path, &data)?;
        }
    }
    Ok(destination.path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::error::{ArchiveLimitFailure, ArchiveLimitKind};
    use sevenz_rust2::{ArchiveWriter, EncoderConfiguration, EncoderMethod, SourceReader};

    // A wire-format fixture with a block CRC and no individual file CRCs.
    // ArchiveWriter normally emits file CRCs, so it cannot cover this variant.
    fn block_crc_fixture(path: &Path, corrupt_tail: bool) {
        block_crc_fixture_with_method(path, corrupt_tail, 0);
    }

    pub(crate) fn block_crc_fixture_with_method(path: &Path, corrupt_tail: bool, method: u8) {
        fn crc32(bytes: &[u8]) -> u32 {
            let mut crc = u32::MAX;
            for byte in bytes {
                crc ^= *byte as u32;
                for _ in 0..8 {
                    crc = (crc >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(crc & 1));
                }
            }
            !crc
        }
        assert_eq!(crc32(b"123456789"), 0xcbf43926);
        let content = b"safetail";
        let mut header = vec![
            1, 4, // Header and MainStreamsInfo.
            6, 0, 1, 9, 8, 0, // One COPY pack stream of eight bytes.
            7, 11, 1, 0, 1, 1, method, 12, 8, 10, 1, // One block, with a CRC.
        ];
        header.extend(crc32(content).to_le_bytes());
        header.extend([0, 8, 13, 2, 9, 4, 10, 0, 0, 0, 0]); // Two sub-streams, neither with a CRC.
        let mut names = vec![0];
        for name in ["selected.txt", "tail.txt"] {
            for unit in name.encode_utf16().chain([0]) {
                names.extend(unit.to_le_bytes());
            }
        }
        header.extend([5, 2, 17, names.len() as u8]);
        header.extend(names);
        header.extend([0, 0]);
        let mut bytes = vec![0u8; 32];
        bytes[..8].copy_from_slice(b"7z\xbc\xaf\x27\x1c\x00\x04");
        bytes[12..20].copy_from_slice(&8u64.to_le_bytes());
        bytes[20..28].copy_from_slice(&(header.len() as u64).to_le_bytes());
        bytes[28..32].copy_from_slice(&crc32(&header).to_le_bytes());
        let start_crc = crc32(&bytes[12..32]);
        bytes[8..12].copy_from_slice(&start_crc.to_le_bytes());
        bytes.extend(content);
        if corrupt_tail {
            bytes[36] ^= 0x55;
        }
        bytes.extend(header);
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn targeted_seven_open_verifies_the_block_crc_when_the_file_has_no_individual_crc() {
        for corrupt_tail in [false, true] {
            let fixture = tempfile::tempdir().unwrap();
            let archive = fixture.path().join("block-crc.7z");
            block_crc_fixture(&archive, corrupt_tail);
            let parsed = Archive::open(&archive).unwrap();
            assert!(parsed.blocks[0].has_crc);
            assert!(!parsed.files[0].has_crc);
            let target = tempfile::tempdir().unwrap();
            let result =
                extract_seven_file_for_open(&archive, "selected.txt", target.path(), limits());
            if corrupt_tail {
                assert!(
                    result.is_err(),
                    "the unverified prefix must not be accepted"
                );
            } else {
                assert_eq!(fs::read(result.unwrap()).unwrap(), b"safe");
            }
            assert_eq!(fs::read_dir(target.path()).unwrap().count(), 1);
        }
    }

    fn limits() -> ArchiveLimits {
        ArchiveLimits {
            max_bytes: 4,
            max_entries: 4,
            ..ArchiveLimits::OPEN_FILE
        }
    }

    fn write_entries(
        path: &Path,
        entries: Vec<(ArchiveEntry, Vec<u8>)>,
        solid: bool,
        method: EncoderMethod,
    ) {
        let mut writer = ArchiveWriter::create(path).unwrap();
        writer.set_content_methods(vec![EncoderConfiguration::new(method)]);
        if solid {
            let (entries, readers): (Vec<_>, Vec<_>) = entries
                .into_iter()
                .map(|(entry, bytes)| (entry, SourceReader::new(io::Cursor::new(bytes))))
                .unzip();
            writer.push_archive_entries(entries, readers).unwrap();
        } else {
            for (entry, bytes) in entries {
                if entry.has_stream {
                    writer
                        .push_archive_entry(entry, Some(io::Cursor::new(bytes)))
                        .unwrap();
                } else {
                    writer
                        .push_archive_entry::<io::Cursor<Vec<u8>>>(entry, None)
                        .unwrap();
                }
            }
        }
        writer.finish().unwrap();
    }

    fn write_archive(path: &Path, entries: &[(&str, &[u8])], solid: bool) {
        write_entries(
            path,
            entries
                .iter()
                .map(|(name, bytes)| (ArchiveEntry::new_file(name), bytes.to_vec()))
                .collect(),
            solid,
            EncoderMethod::COPY,
        );
    }

    #[test]
    fn seven_metadata_keeps_limit_and_cancellation_errors_structured() {
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("metadata.7z");
        write_archive(
            &archive,
            &[("a/b/c", b"safe"), ("other.txt", b"safe")],
            false,
        );
        for (allowance, kind) in [
            (
                ArchiveLimits {
                    max_entries: 1,
                    ..ArchiveLimits::LISTING
                },
                ArchiveLimitKind::Entries,
            ),
            (
                ArchiveLimits {
                    max_depth: 2,
                    ..ArchiveLimits::LISTING
                },
                ArchiveLimitKind::Depth,
            ),
            (
                ArchiveLimits {
                    max_duration: std::time::Duration::ZERO,
                    ..ArchiveLimits::LISTING
                },
                ArchiveLimitKind::Seconds,
            ),
        ] {
            let error = indexed_with_limits(&archive, allowance, &|| Ok(())).unwrap_err();
            assert_eq!(
                ArchiveLimitFailure::from_message(&error.to_string())
                    .unwrap()
                    .kind,
                kind
            );
        }
        let checkpoints = std::cell::Cell::new(0);
        let error = indexed_files(&archive, &|| {
            let count = checkpoints.get() + 1;
            checkpoints.set(count);
            if count == 3 {
                Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "cancelled during header read",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(checkpoints.get(), 3);
        assert!(error.to_string().contains("cancelled during header read"));
    }

    #[test]
    fn seven_metadata_and_listing_preserve_folders_empty_files_and_unicode_names() {
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("metadata.7z");
        let mut empty = ArchiveEntry::new_file("folder/empty.txt");
        empty.has_stream = false;
        write_entries(
            &archive,
            vec![
                (ArchiveEntry::new_directory("."), Vec::new()),
                (ArchiveEntry::new_directory("folder"), Vec::new()),
                (empty, Vec::new()),
                (
                    ArchiveEntry::new_file("folder/inner/école.txt"),
                    b"hello".to_vec(),
                ),
                (ArchiveEntry::new_file("small.txt"), b"safe".to_vec()),
            ],
            false,
            EncoderMethod::COPY,
        );
        let indexed = indexed_files(&archive, &|| Ok(())).unwrap();
        assert_eq!(indexed.len(), 4);
        assert!(indexed
            .iter()
            .any(|entry| entry.is_dir && entry.name == "folder"));
        assert!(indexed
            .iter()
            .any(|entry| !entry.is_dir && entry.name == "empty.txt" && entry.size == 0));
        let list = |internal: &str| {
            crate::commands::archive::list_archive_contents(
                archive.to_string_lossy().into_owned(),
                internal.into(),
            )
            .unwrap()
        };
        assert_eq!(list("").len(), 2);
        let folder = list("folder");
        assert_eq!(folder.len(), 2);
        assert!(folder
            .iter()
            .any(|entry| entry.is_dir && entry.name == "inner"));
        let nested = list("folder\\inner");
        assert_eq!(nested.len(), 1);
        assert_eq!(nested[0].size, 5);
        assert_eq!(nested[0].name, "école.txt");
        assert_eq!(
            Path::new(&nested[0].path),
            archive.join("folder/inner/école.txt")
        );
    }

    fn reject_metadata_and_full_extraction(archive: &Path) {
        assert!(indexed_files(archive, &|| Ok(())).is_err());
        assert!(crate::commands::archive::list_archive_contents(
            archive.to_string_lossy().into_owned(),
            String::new(),
        )
        .is_err());
        let target = tempfile::tempdir().unwrap();
        assert!(crate::commands::archive::extract_archive_with_limits(
            archive.to_string_lossy().into_owned(),
            target.path().to_string_lossy().into_owned(),
            &|| Ok(()),
            ArchiveLimits::EXTRACTION,
        )
        .is_err());
        assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
    }

    #[test]
    fn targeted_seven_open_writes_only_the_unicode_selection_with_copy_and_lzma2() {
        for method in [EncoderMethod::COPY, EncoderMethod::LZMA2] {
            for solid in [false, true] {
                for reverse in [false, true] {
                    let fixture = tempfile::tempdir().unwrap();
                    let archive = fixture.path().join("selection.7z");
                    let mut selected = ArchiveEntry::new_file("école/文件.txt");
                    let modified =
                        std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_234_567_890);
                    selected.has_last_modified_date = true;
                    selected.last_modified_date = modified.try_into().unwrap();
                    let mut entries = vec![
                        (selected, b"safe".to_vec()),
                        (
                            ArchiveEntry::new_file("other.txt"),
                            b"unselected and larger than four bytes".to_vec(),
                        ),
                    ];
                    if reverse {
                        entries.reverse();
                    }
                    write_entries(&archive, entries, solid, method);
                    let target = tempfile::tempdir().unwrap();
                    let output = extract_seven_file_for_open(
                        &archive,
                        "école\\文件.txt",
                        target.path(),
                        limits(),
                    )
                    .unwrap();
                    assert_eq!(output, target.path().join("文件.txt"));
                    assert_eq!(fs::read(&output).unwrap(), b"safe");
                    assert_eq!(fs::metadata(output).unwrap().modified().unwrap(), modified);
                    assert_eq!(fs::read_dir(target.path()).unwrap().count(), 1);
                    let refused = tempfile::tempdir().unwrap();
                    let error = extract_seven_file_for_open(
                        &archive,
                        "other.txt",
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
    }

    #[test]
    fn targeted_seven_open_bounds_solid_predecessors_and_skips_other_blocks_and_later_members() {
        let large = vec![0; 16 * 1024];
        let allowance = ArchiveLimits {
            max_entries: 2,
            ..limits()
        };
        for solid in [false, true] {
            for reverse in [false, true] {
                let fixture = tempfile::tempdir().unwrap();
                let archive = fixture.path().join("selection.7z");
                let mut entries = vec![("selected.txt", &b"safe"[..]), ("other.txt", &large[..])];
                if reverse {
                    entries.reverse();
                }
                write_archive(&archive, &entries, solid);
                let target = tempfile::tempdir().unwrap();
                let result =
                    extract_seven_file_for_open(&archive, "selected.txt", target.path(), allowance);
                if solid && reverse {
                    let failure =
                        ArchiveLimitFailure::from_message(&result.unwrap_err().to_string())
                            .unwrap();
                    assert_eq!(failure.kind, ArchiveLimitKind::Decoded);
                    assert_eq!(failure.limit, 4 + 2 * 4096);
                    assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
                } else {
                    assert_eq!(fs::read(result.unwrap()).unwrap(), b"safe");
                    assert_eq!(fs::read_dir(target.path()).unwrap().count(), 1);
                }
            }
        }
    }

    #[test]
    fn targeted_seven_open_rejects_late_unsafe_names_duplicates_and_special_entries_before_output()
    {
        for solid in [false, true] {
            for name in [
                "../outside",
                "C:outside",
                "file:stream",
                "NUL.txt",
                "a\0b",
                "a\\selected.txt",
            ] {
                let fixture = tempfile::tempdir().unwrap();
                let archive = fixture.path().join("selection.7z");
                write_archive(
                    &archive,
                    &[("a/selected.txt", b"safe"), (name, b"safe")],
                    solid,
                );
                let target = tempfile::tempdir().unwrap();
                assert!(
                    extract_seven_file_for_open(
                        &archive,
                        "a/selected.txt",
                        target.path(),
                        limits()
                    )
                    .is_err(),
                    "{name:?}"
                );
                assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
            }
            for attributes in [0x400, 0o120000 << 16, 0o010000 << 16] {
                let fixture = tempfile::tempdir().unwrap();
                let archive = fixture.path().join("selection.7z");
                let mut special = ArchiveEntry::new_file("link");
                special.has_windows_attributes = true;
                special.windows_attributes = attributes;
                write_entries(
                    &archive,
                    vec![
                        (ArchiveEntry::new_file("selected.txt"), b"safe".to_vec()),
                        (special, b"target".to_vec()),
                    ],
                    solid,
                    EncoderMethod::COPY,
                );
                let target = tempfile::tempdir().unwrap();
                assert!(extract_seven_file_for_open(
                    &archive,
                    "selected.txt",
                    target.path(),
                    limits()
                )
                .is_err());
                assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
                reject_metadata_and_full_extraction(&archive);
            }
        }
        let fixture = tempfile::tempdir().unwrap();
        let archive = fixture.path().join("anti.7z");
        let mut anti = ArchiveEntry::new_file("deleted.txt");
        anti.has_stream = false;
        anti.is_anti_item = true;
        write_entries(
            &archive,
            vec![
                (ArchiveEntry::new_file("selected.txt"), b"safe".to_vec()),
                (anti, Vec::new()),
            ],
            false,
            EncoderMethod::COPY,
        );
        let target = tempfile::tempdir().unwrap();
        assert!(
            extract_seven_file_for_open(&archive, "selected.txt", target.path(), limits()).is_err()
        );
        assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
        reject_metadata_and_full_extraction(&archive);
    }

    #[test]
    fn targeted_seven_open_keeps_entry_depth_deadline_and_cancellation_errors() {
        let fixture = tempfile::tempdir().unwrap();
        let archive = fixture.path().join("selection.7z");
        write_archive(&archive, &[("selected.txt", b"safe"), ("a/b/c", b"")], true);
        for (allowance, kind) in [
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
                extract_seven_file_for_open(&archive, "selected.txt", target.path(), allowance)
                    .unwrap_err();
            assert_eq!(
                ArchiveLimitFailure::from_message(&error.to_string())
                    .unwrap()
                    .kind,
                kind
            );
            assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
        }
        let target = tempfile::tempdir().unwrap();
        let error =
            extract_with_checkpoint(&archive, "selected.txt", target.path(), limits(), &|| {
                Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
            })
            .unwrap_err();
        assert!(error.to_string().contains("cancelled"));
        assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
    }

    #[test]
    fn targeted_seven_open_handles_empty_files_and_refuses_directories_absent_files_and_overwrites()
    {
        let fixture = tempfile::tempdir().unwrap();
        let archive = fixture.path().join("selection.7z");
        let mut empty = ArchiveEntry::new_file("empty.txt");
        empty.has_stream = false;
        write_entries(
            &archive,
            vec![
                (ArchiveEntry::new_directory("folder"), Vec::new()),
                (empty, Vec::new()),
                (ArchiveEntry::new_file("selected.txt"), b"safe".to_vec()),
            ],
            false,
            EncoderMethod::COPY,
        );
        let target = tempfile::tempdir().unwrap();
        let output =
            extract_seven_file_for_open(&archive, "empty.txt", target.path(), limits()).unwrap();
        assert_eq!(fs::read(output).unwrap(), b"");
        for internal in ["folder", "missing.txt", "../selected.txt", ""] {
            let target = tempfile::tempdir().unwrap();
            assert!(
                extract_seven_file_for_open(&archive, internal, target.path(), limits()).is_err()
            );
            assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
        }
        let target = tempfile::tempdir().unwrap();
        let existing = target.path().join("selected.txt");
        fs::write(&existing, b"foreign").unwrap();
        assert!(
            extract_seven_file_for_open(&archive, "selected.txt", target.path(), limits()).is_err()
        );
        assert_eq!(fs::read(existing).unwrap(), b"foreign");
    }

    #[test]
    fn targeted_seven_open_ignores_corrupt_unselected_blocks_but_refuses_a_bad_selected_crc() {
        for solid in [false, true] {
            let fixture = tempfile::tempdir().unwrap();
            let archive = fixture.path().join("selection.7z");
            write_archive(
                &archive,
                &[("other.txt", b"other"), ("selected.txt", b"safe")],
                solid,
            );
            let parsed = Archive::open(&archive).unwrap();
            let offset =
                (32 + parsed.pack_pos() + parsed.stream_map.pack_stream_offsets()[0]) as usize;
            let mut bytes = fs::read(&archive).unwrap();
            bytes[offset] ^= 0x55;
            fs::write(&archive, &bytes).unwrap();
            let target = tempfile::tempdir().unwrap();
            let result =
                extract_seven_file_for_open(&archive, "selected.txt", target.path(), limits());
            if solid {
                assert!(result.is_err());
                assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
            } else {
                assert_eq!(fs::read(result.unwrap()).unwrap(), b"safe");
            }
            // Restore the prefix and corrupt the selected content instead.
            bytes[offset] ^= 0x55;
            let block = parsed.stream_map.file_block_index[1].unwrap();
            let start = if solid {
                offset + 5
            } else {
                let pack = parsed.stream_map.block_first_pack_stream_index()[block];
                (32 + parsed.pack_pos() + parsed.stream_map.pack_stream_offsets()[pack]) as usize
            };
            bytes[start] ^= 0x55;
            fs::write(&archive, bytes).unwrap();
            let target = tempfile::tempdir().unwrap();
            let error =
                extract_seven_file_for_open(&archive, "selected.txt", target.path(), limits())
                    .unwrap_err();
            assert!(!error.to_string().is_empty());
            assert_eq!(fs::read_dir(target.path()).unwrap().count(), 1); // Caller owns cleanup after a late failure.
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn targeted_seven_open_retains_source_identity_and_provenance_after_parent_retargeting() {
        use crate::platform::windows_reparse_tests::{junction, Retarget};
        use std::cell::Cell;
        for solid in [false, true] {
            let fixture = tempfile::tempdir().unwrap();
            let chosen = fixture.path().join("chosen");
            let outside = fixture.path().join("outside");
            fs::create_dir(&chosen).unwrap();
            fs::create_dir(&outside).unwrap();
            let archive = chosen.join("selection.7z");
            let foreign = outside.join("selection.7z");
            write_archive(&archive, &[("selected.txt", b"safe")], solid);
            write_archive(&foreign, &[("selected.txt", b"evil")], solid);
            let original = fs::read(&foreign).unwrap();
            let provenance = b"[ZoneTransfer]\r\nZoneId=3\r\n";
            platform()
                .write_file_provenance(&archive, provenance)
                .unwrap();
            let link = fixture.path().join("link");
            let template = fixture.path().join("template");
            junction(&link, &chosen);
            junction(&template, &outside);
            let retarget = Retarget::new(&link, &template);
            let changed = Cell::new(false);
            let target = tempfile::tempdir().unwrap();
            let result = extract_with_checkpoint(
                &link.join("selection.7z"),
                "selected.txt",
                target.path(),
                limits(),
                &|| {
                    if !changed.replace(true) {
                        assert!(File::options().write(true).open(&archive).is_err());
                        assert!(fs::rename(&archive, chosen.join("moved.7z")).is_err());
                        retarget.apply();
                    }
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(fs::read(&result).unwrap(), b"safe");
            assert_eq!(
                platform().read_file_provenance(&result).unwrap().as_deref(),
                Some(&provenance[..])
            );
            assert_eq!(fs::read(&foreign).unwrap(), original);
            drop(retarget);
            fs::remove_dir(link).unwrap();
            fs::remove_dir(template).unwrap();
            assert!(File::options().write(true).open(&archive).is_ok());
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn targeted_seven_open_refuses_a_new_destination_junction() {
        use crate::platform::windows_reparse_tests::{junction, Retarget};
        use std::cell::Cell;
        let fixture = tempfile::tempdir().unwrap();
        let archive = fixture.path().join("selection.7z");
        write_archive(&archive, &[("selected.txt", b"safe")], true);
        let target = fixture.path().join("target");
        let outside = fixture.path().join("outside");
        fs::create_dir(&target).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("selected.txt"), b"foreign").unwrap();
        let template = fixture.path().join("template");
        junction(&template, &outside);
        let retarget = Retarget::new(&target, &template);
        let changed = Cell::new(false);
        let result = extract_with_checkpoint(&archive, "selected.txt", &target, limits(), &|| {
            if !changed.replace(true) {
                retarget.apply();
            }
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(fs::read(outside.join("selected.txt")).unwrap(), b"foreign");
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
        retarget.clear();
        drop(retarget);
        fs::remove_dir(template).unwrap();
        assert_eq!(fs::read_dir(target).unwrap().count(), 0);
    }
}
