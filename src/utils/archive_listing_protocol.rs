//! Untrusted worker metadata. The parent constructs every virtual path itself.
use super::archive_security::relative_archive_path;
use crate::models::error::{ArchiveLimitFailure, ArchiveLimitKind, ArchiveListingFailure};
use crate::models::{CommandError, FileEntry};
use std::collections::HashSet;
use std::io::{self, Read, Write};
use std::path::Path;

pub const MAX_REPLY: usize = 32 * 1024 * 1024;
const MAX_ENTRIES: usize = 100_000;
const MAX_ERROR: usize = 4096;
const MAGIC: &[u8; 8] = b"OXYARCH1";
const INDEX_MAGIC: &[u8; 8] = b"OXYARIX1";
#[derive(Clone, Copy)]
enum Mode {
    Directory,
    Index,
}
impl Mode {
    fn magic(self) -> &'static [u8; 8] {
        match self {
            Self::Directory => MAGIC,
            Self::Index => INDEX_MAGIC,
        }
    }
    fn validate(self, name: &str) -> io::Result<()> {
        match self {
            Self::Directory => validate_name(name),
            Self::Index => {
                let relative = relative_archive_path(name)?;
                if relative.as_os_str().is_empty()
                    || relative.to_string_lossy().replace('\\', "/") != name
                {
                    return Err(invalid());
                }
                if relative.components().count() > 64 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        ArchiveLimitFailure {
                            kind: ArchiveLimitKind::Depth,
                            limit: 64,
                        },
                    ));
                }
                Ok(())
            }
        }
    }
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        ArchiveListingFailure::WorkerFailed,
    )
}
fn metadata_limit() -> io::Error {
    io::Error::new(
        io::ErrorKind::FileTooLarge,
        ArchiveListingFailure::MetadataLimit,
    )
}
fn entry_limit() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        ArchiveLimitFailure {
            kind: ArchiveLimitKind::Entries,
            limit: MAX_ENTRIES as u64,
        },
    )
}
fn validate_name(name: &str) -> io::Result<()> {
    if name.is_empty() || name == "." || name.contains(['/', '\\']) {
        return Err(invalid());
    }
    let relative = relative_archive_path(name)?;
    if relative.components().count() != 1 {
        return Err(invalid());
    }
    Ok(())
}

/// Validate the entire reply before writing any byte, allowing a typed error
/// reply instead of a partially published list if metadata exceeds its budget.
pub fn encode(
    result: Result<Vec<FileEntry>, CommandError>,
    output: &mut impl Write,
) -> io::Result<()> {
    encode_mode(result, output, Mode::Directory)
}

pub fn encode_index(
    mut result: Result<Vec<FileEntry>, CommandError>,
    archive: &Path,
    output: &mut impl Write,
) -> io::Result<()> {
    if let Ok(entries) = &mut result {
        for entry in entries {
            let relative = Path::new(&entry.path)
                .strip_prefix(archive)
                .map_err(|_| invalid())?;
            entry.name = relative.to_string_lossy().replace('\\', "/");
        }
    }
    encode_mode(result, output, Mode::Index)
}
fn encode_mode(
    result: Result<Vec<FileEntry>, CommandError>,
    output: &mut impl Write,
    mode: Mode,
) -> io::Result<()> {
    match result {
        Ok(entries) => {
            if entries.len() > MAX_ENTRIES {
                return Err(entry_limit());
            }
            let mut size = 13usize;
            let mut seen = HashSet::new();
            for entry in &entries {
                mode.validate(&entry.name)?;
                if !seen.insert(&entry.name) {
                    return Err(invalid());
                }
                size = size
                    .checked_add(21)
                    .and_then(|n| n.checked_add(entry.name.len()))
                    .ok_or_else(metadata_limit)?;
                if size > MAX_REPLY {
                    return Err(metadata_limit());
                }
            }
            output.write_all(mode.magic())?;
            output.write_all(&[0])?;
            output.write_all(&(entries.len() as u32).to_le_bytes())?;
            for entry in entries {
                output.write_all(&(entry.name.len() as u32).to_le_bytes())?;
                let flags = u8::from(entry.is_dir)
                    | (u8::from(entry.is_hidden) << 1)
                    | (u8::from(entry.is_system) << 2);
                output.write_all(&[flags])?;
                output.write_all(&entry.size.to_le_bytes())?;
                output.write_all(&entry.modified.to_le_bytes())?;
                output.write_all(entry.name.as_bytes())?;
            }
        }
        Err(error) => {
            let detail = error.to_string();
            let mut end = detail.len().min(MAX_ERROR);
            while !detail.is_char_boundary(end) {
                end -= 1;
            }
            output.write_all(mode.magic())?;
            output.write_all(&[1])?;
            output.write_all(&(end as u32).to_le_bytes())?;
            output.write_all(&detail.as_bytes()[..end])?;
        }
    }
    output.flush()
}

fn read_u32(input: &mut &[u8]) -> io::Result<u32> {
    let mut bytes = [0; 4];
    input.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}
fn read_u64(input: &mut &[u8]) -> io::Result<u64> {
    let mut bytes = [0; 8];
    input.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}
fn text<'a>(input: &mut &'a [u8], length: usize) -> io::Result<&'a str> {
    if length > input.len() {
        return Err(invalid());
    }
    let (value, rest) = input.split_at(length);
    *input = rest;
    std::str::from_utf8(value).map_err(|_| invalid())
}

pub fn decode(
    bytes: &[u8],
    archive: &Path,
    internal: &str,
) -> Result<Vec<FileEntry>, CommandError> {
    decode_mode(bytes, archive, internal, Mode::Directory, &|| Ok(()))
}
pub fn decode_index(
    bytes: &[u8],
    archive: &Path,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<Vec<FileEntry>, CommandError> {
    decode_mode(bytes, archive, "", Mode::Index, checkpoint)
}
fn decode_mode(
    bytes: &[u8],
    archive: &Path,
    internal: &str,
    mode: Mode,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<Vec<FileEntry>, CommandError> {
    decode_io(bytes, archive, internal, mode, checkpoint).map_err(|error| {
        if error.kind() == io::ErrorKind::Interrupted {
            return CommandError::IoError(error.to_string());
        }
        let detail = error.to_string();
        CommandError::ArchiveError(
            if ArchiveListingFailure::from_message(&detail).is_some()
                || ArchiveLimitFailure::from_message(&detail).is_some()
            {
                detail
            } else {
                ArchiveListingFailure::WorkerFailed.to_string()
            },
        )
    })?
}
fn decode_io(
    mut bytes: &[u8],
    archive: &Path,
    internal: &str,
    mode: Mode,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> io::Result<Result<Vec<FileEntry>, CommandError>> {
    checkpoint()?;
    if bytes.len() > MAX_REPLY {
        return Err(metadata_limit());
    }
    let parent = archive.join(relative_archive_path(internal)?);
    let archive_path = std::sync::Arc::new(archive.to_path_buf());
    let mut magic = [0; 8];
    bytes.read_exact(&mut magic)?;
    if &magic != mode.magic() {
        return Err(invalid());
    }
    let mut status = [0];
    bytes.read_exact(&mut status)?;
    let count = read_u32(&mut bytes)? as usize;
    if status == [1] {
        if count > MAX_ERROR || bytes.len() != count {
            return Err(invalid());
        }
        return Ok(Err(CommandError::ArchiveError(
            text(&mut bytes, count)?.into(),
        )));
    }
    if status != [0] {
        return Err(invalid());
    }
    if count > MAX_ENTRIES {
        return Err(entry_limit());
    }
    // Reject advertised allocation counts that cannot fit even their headers.
    if count > bytes.len() / 22 {
        return Err(invalid());
    }
    // Reconstructed paths repeat the trusted archive prefix. A bounded wire
    // reply alone would still allow huge parent allocations for long paths.
    let per_entry = std::mem::size_of::<FileEntry>()
        .checked_add(parent.to_string_lossy().len())
        .and_then(|n| n.checked_add(1))
        .ok_or_else(metadata_limit)?;
    let mut metadata_bytes = count.checked_mul(per_entry).ok_or_else(metadata_limit)?;
    if metadata_bytes > MAX_REPLY {
        return Err(metadata_limit());
    }
    let mut entries = Vec::new();
    entries.try_reserve_exact(count).map_err(|_| invalid())?;
    let mut seen = HashSet::new();
    for _ in 0..count {
        checkpoint()?;
        let length = read_u32(&mut bytes)? as usize;
        let mut flags = [0];
        bytes.read_exact(&mut flags)?;
        if flags[0] & !7 != 0 {
            return Err(invalid());
        }
        let size = read_u64(&mut bytes)?;
        let modified = read_u64(&mut bytes)?;
        let name = text(&mut bytes, length)?;
        mode.validate(name)?;
        let relative = relative_archive_path(name)?;
        metadata_bytes = name
            .len()
            .checked_mul(2)
            .and_then(|n| metadata_bytes.checked_add(n))
            .ok_or_else(metadata_limit)?;
        if metadata_bytes > MAX_REPLY {
            return Err(metadata_limit());
        }
        if !seen.insert(name) {
            return Err(invalid());
        }
        entries.push(FileEntry {
            archive_path: Some(archive_path.clone()),
            name: match mode {
                Mode::Directory => name.into(),
                Mode::Index => relative
                    .file_name()
                    .ok_or_else(invalid)?
                    .to_string_lossy()
                    .into_owned(),
            },
            path: parent.join(&relative).to_string_lossy().into_owned(),
            is_dir: flags[0] & 1 != 0,
            is_hidden: flags[0] & 2 != 0,
            is_system: flags[0] & 4 != 0,
            size,
            modified,
            ..FileEntry::default()
        });
    }
    if !bytes.is_empty() {
        return Err(invalid());
    }
    checkpoint()?;
    Ok(Ok(entries))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bytes(name: &str) -> Vec<u8> {
        let mut bytes = Vec::new();
        encode(
            Ok(vec![FileEntry {
                name: name.into(),
                path: "C:\\untrusted\\injected".into(),
                size: 12,
                modified: 34,
                ..FileEntry::default()
            }]),
            &mut bytes,
        )
        .unwrap();
        bytes
    }
    #[test]
    fn reply_preserves_metadata_but_reconstructs_paths_and_refuses_every_truncation() {
        let bytes = bytes("école.txt");
        let archive = Path::new("selected.zip");
        let entry = decode(&bytes, archive, "safe").unwrap().remove(0);
        assert_eq!(
            entry.path,
            archive.join("safe").join("école.txt").to_string_lossy()
        );
        assert_eq!((entry.size, entry.modified), (12, 34));
        assert!(!entry.is_symlink && entry.original_path.is_none());
        for end in 0..bytes.len() {
            assert!(decode(&bytes[..end], archive, "").is_err(), "{end}");
        }
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(decode(&extra, archive, "").is_err());
    }
    #[test]
    fn hostile_names_counts_flags_duplicates_and_utf8_are_rejected_before_publication() {
        let archive = Path::new("selected.zip");
        for name in ["..", "C:", "a/b", "a\\b", "NUL", "a.", "a ", "", ".", "\0"] {
            let mut output = Vec::new();
            assert!(encode(
                Ok(vec![FileEntry {
                    name: name.into(),
                    ..FileEntry::default()
                }]),
                &mut output
            )
            .is_err());
            assert!(output.is_empty());
            let mut reply = bytes("a");
            reply.truncate(13);
            reply.extend_from_slice(&(name.len() as u32).to_le_bytes());
            reply.push(0);
            reply.extend_from_slice(&[0; 16]);
            reply.extend_from_slice(name.as_bytes());
            assert!(decode(&reply, archive, "").is_err(), "{name:?}");
        }
        for count in [u32::MAX, 100001, 100000] {
            let mut reply = bytes("a");
            reply[9..13].copy_from_slice(&count.to_le_bytes());
            assert!(decode(&reply, archive, "").is_err());
        }
        let mut reply = bytes("a");
        reply[17] = 128;
        assert!(decode(&reply, archive, "").is_err());
        let mut reply = bytes("a");
        *reply.last_mut().unwrap() = 255;
        assert!(decode(&reply, archive, "").is_err());
        let mut reply = bytes("a");
        reply[9..13].copy_from_slice(&2u32.to_le_bytes());
        reply.extend_from_within(13..);
        assert!(decode(&reply, archive, "").is_err());
        assert!(decode(&bytes("a"), archive, "../outside").is_err());
    }
    #[test]
    fn metadata_limit_is_distinct_from_decompressed_size_and_errors_remain_typed() {
        let mut output = Vec::new();
        let entry = FileEntry {
            name: "a".repeat(MAX_REPLY),
            ..FileEntry::default()
        };
        let error = encode(Ok(vec![entry]), &mut output).unwrap_err();
        assert!(matches!(
            ArchiveListingFailure::from_message(&error.to_string()),
            Some(ArchiveListingFailure::MetadataLimit)
        ));
        assert!(output.is_empty());
        let error = ArchiveLimitFailure {
            kind: ArchiveLimitKind::Seconds,
            limit: 120,
        };
        encode(
            Err(CommandError::ArchiveError(error.to_string())),
            &mut output,
        )
        .unwrap();
        assert_eq!(
            ArchiveLimitFailure::from_message(
                &decode(&output, Path::new("a.zip"), "")
                    .unwrap_err()
                    .to_string()
            )
            .unwrap(),
            error
        );
    }

    #[test]
    fn long_trusted_prefix_cannot_amplify_a_small_reply_into_unbounded_parent_paths() {
        let entries = (0..1100)
            .map(|index| FileEntry {
                name: format!("f{index}"),
                ..FileEntry::default()
            })
            .collect();
        let mut bytes = Vec::new();
        encode(Ok(entries), &mut bytes).unwrap();
        assert!(bytes.len() < 32 * 1024);
        let prefix = "p".repeat(32700);
        let error = decode(&bytes, Path::new(&prefix), "").unwrap_err();
        assert!(matches!(
            ArchiveListingFailure::from_message(&error.to_string()),
            Some(ArchiveListingFailure::MetadataLimit)
        ));
    }
    #[test]
    fn index_preserves_full_paths_and_basename_without_accepting_directory_reply_or_foreign_roots()
    {
        let archive = Path::new("selected.zip");
        let entries = ["docs/école.txt", "other/école.txt"]
            .into_iter()
            .map(|relative| FileEntry {
                name: "école.txt".into(),
                path: archive.join(relative).to_string_lossy().into_owned(),
                size: 12,
                modified: 34,
                ..FileEntry::default()
            })
            .collect();
        let mut reply = Vec::new();
        encode_index(Ok(entries), archive, &mut reply).unwrap();
        let entries = decode_index(&reply, archive, &|| Ok(())).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries
            .iter()
            .all(|entry| entry.name == "école.txt" && entry.size == 12 && entry.modified == 34));
        assert_eq!(
            entries[0].path,
            archive.join("docs").join("école.txt").to_string_lossy()
        );
        assert_ne!(entries[0].path, entries[1].path);
        assert!(decode(&reply, archive, "").is_err());
        assert!(decode_index(&bytes("a"), archive, &|| Ok(())).is_err());
        let mut output = Vec::new();
        assert!(encode_index(
            Ok(vec![FileEntry {
                path: "foreign.txt".into(),
                ..FileEntry::default()
            }]),
            archive,
            &mut output
        )
        .is_err());
        assert!(output.is_empty());
        for end in 0..reply.len() {
            assert!(decode_index(&reply[..end], archive, &|| Ok(())).is_err());
        }
    }
    #[test]
    fn index_rejects_escaping_aliases_duplicate_paths_and_depth_before_publication() {
        let archive = Path::new("selected.zip");
        let deep = std::iter::repeat_n("a", 65).collect::<Vec<_>>().join("/");
        for name in [
            "../outside",
            "C:/outside",
            "/outside",
            "a/./b",
            "a//b",
            "a\\b",
            "a/../b",
            "a/NUL",
            &deep,
        ] {
            let mut reply = INDEX_MAGIC.to_vec();
            reply.push(0);
            reply.extend_from_slice(&1u32.to_le_bytes());
            reply.extend_from_slice(&(name.len() as u32).to_le_bytes());
            reply.push(0);
            reply.extend_from_slice(&[0; 16]);
            reply.extend_from_slice(name.as_bytes());
            assert!(decode_index(&reply, archive, &|| Ok(())).is_err(), "{name}");
        }
        let mut reply = INDEX_MAGIC.to_vec();
        reply.extend_from_slice(&bytes("a")[8..]);
        reply[9..13].copy_from_slice(&2u32.to_le_bytes());
        reply.extend_from_within(13..);
        assert!(decode_index(&reply, archive, &|| Ok(())).is_err());
    }
    #[test]
    fn index_decoding_cancellation_discards_partial_results() {
        let archive = Path::new("selected.zip");
        let mut reply = Vec::new();
        let entries = (0..3)
            .map(|index| FileEntry {
                path: archive
                    .join(format!("docs/f{index}"))
                    .to_string_lossy()
                    .into_owned(),
                ..FileEntry::default()
            })
            .collect();
        encode_index(Ok(entries), archive, &mut reply).unwrap();
        let calls = std::cell::Cell::new(0);
        let checkpoint = || {
            calls.set(calls.get() + 1);
            if calls.get() >= 3 {
                Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
            } else {
                Ok(())
            }
        };
        let error = decode_index(&reply, archive, &checkpoint).unwrap_err();
        assert!(error.to_string().contains("cancelled"));
        assert!(ArchiveListingFailure::from_message(&error.to_string()).is_none());
    }
}
