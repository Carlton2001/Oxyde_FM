//! Bounded ISO indexing shared by navigation, search and extraction.
use super::archive::safe_iso_component;
use crate::models::{CommandError, FileEntry};
use crate::platform::platform;
use crate::systems::path_parents::HeldParents;
use crate::utils::archive_limits::{ArchiveBudget, ArchiveLimits, BudgetReader};
use crate::utils::archive_security::relative_archive_path;
#[cfg(test)]
use crate::utils::archive_security::ExtractionRoot;
use std::collections::HashSet;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
#[cfg(test)]
use std::io::Write;
use std::path::{Path, PathBuf};

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

struct Image<'a, R = File> {
    file: R,
    length: u64,
    budget: &'a ArchiveBudget,
    checkpoint: &'a dyn Fn() -> io::Result<()>,
}

impl<R: Read + Seek> Image<'_, R> {
    fn check_range(&self, offset: u64, length: u64) -> io::Result<()> {
        if offset
            .checked_add(length)
            .is_none_or(|end| end > self.length)
        {
            return Err(invalid("Truncated ISO extent"));
        }
        Ok(())
    }

    fn read_at(&mut self, offset: u64, bytes: &mut [u8]) -> io::Result<()> {
        self.budget.checkpoint(self.checkpoint)?;
        self.check_range(offset, bytes.len() as u64)?;
        self.file.seek(SeekFrom::Start(offset))?;
        BudgetReader {
            reader: &mut self.file,
            budget: self.budget,
            checkpoint: self.checkpoint,
        }
        .read_exact(bytes)
    }
}

#[derive(Clone, Copy)]
struct Extent {
    offset: u64,
    length: u64,
}

struct Record {
    raw_name: Vec<u8>,
    extent: Extent,
    directory: bool,
    final_extent: bool,
}

struct Entry {
    path: PathBuf,
    directory: bool,
    extents: Vec<Extent>,
}

impl Entry {
    fn size(&self) -> io::Result<u64> {
        self.extents.iter().try_fold(0u64, |size, extent| {
            size.checked_add(extent.length)
                .ok_or_else(|| invalid("Invalid ISO file size"))
        })
    }

    fn file_entry(&self, archive: &std::sync::Arc<PathBuf>) -> io::Result<FileEntry> {
        Ok(FileEntry {
            archive_path: Some(archive.clone()),
            name: self
                .path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            path: archive.join(&self.path).to_string_lossy().into_owned(),
            is_dir: self.directory,
            size: self.size()?,
            ..FileEntry::default()
        })
    }
}

fn both_u32(bytes: &[u8]) -> io::Result<u32> {
    let little = u32::from_le_bytes(bytes[..4].try_into().unwrap());
    if little != u32::from_be_bytes(bytes[4..8].try_into().unwrap()) {
        return Err(invalid("Inconsistent ISO extent metadata"));
    }
    Ok(little)
}

fn record(bytes: &[u8], block_size: u64) -> io::Result<Record> {
    if bytes.len() < 34 || bytes[0] as usize != bytes.len() {
        return Err(invalid("Invalid ISO directory record length"));
    }
    let name_length = bytes[32] as usize;
    if name_length == 0 || 33 + name_length > bytes.len() {
        return Err(invalid("Invalid ISO file identifier length"));
    }
    if bytes[26] != 0 || bytes[27] != 0 || bytes[28..32] != [1, 0, 0, 1] {
        return Err(invalid(
            "Interleaved or multi-volume ISO files are not supported",
        ));
    }
    let location = both_u32(&bytes[2..10])? as u64;
    let length = both_u32(&bytes[10..18])? as u64;
    // The extended-attribute blocks precede the actual content.
    let offset = (location + bytes[1] as u64)
        .checked_mul(block_size)
        .ok_or_else(|| invalid("Invalid ISO extent offset"))?;
    Ok(Record {
        raw_name: bytes[33..33 + name_length].to_vec(),
        extent: Extent { offset, length },
        directory: bytes[25] & 2 != 0,
        final_extent: bytes[25] & 128 == 0,
    })
}

fn root_record<R: Read + Seek>(image: &mut Image<'_, R>) -> io::Result<(u64, Record)> {
    let mut offset = 16 * 2048;
    loop {
        let mut descriptor = [0u8; 2048];
        image.read_at(offset, &mut descriptor)?;
        if &descriptor[1..7] != b"CD001\x01" {
            return Err(invalid("Invalid ISO volume descriptor"));
        }
        match descriptor[0] {
            1 => {
                let block_size =
                    u16::from_le_bytes(descriptor[128..130].try_into().unwrap()) as u64;
                if block_size != u16::from_be_bytes(descriptor[130..132].try_into().unwrap()) as u64
                    || block_size < 512
                    || !block_size.is_power_of_two()
                {
                    return Err(invalid("Invalid ISO logical block size"));
                }
                let root = record(&descriptor[156..190], block_size)?;
                if !root.directory || !root.final_extent || root.raw_name != [0] {
                    return Err(invalid("Invalid ISO root directory"));
                }
                return Ok((block_size, root));
            }
            255 => return Err(invalid("Missing ISO primary volume descriptor")),
            _ => offset += 2048,
        }
    }
}

fn index<R: Read + Seek>(image: &mut Image<'_, R>) -> Result<Vec<Entry>, CommandError> {
    let (block_size, root) = root_record(image)?;
    let mut directories = vec![(PathBuf::new(), root.extent)];
    let mut visited = HashSet::new();
    let mut paths = HashSet::new();
    let mut entries: Vec<Entry> = Vec::new();
    while let Some((parent, directory)) = directories.pop() {
        image.budget.checkpoint(image.checkpoint)?;
        image.check_range(directory.offset, directory.length)?;
        if !visited.insert((directory.offset, directory.length)) {
            return Err(invalid("Cyclic or aliased ISO directory").into());
        }
        let end = directory.offset + directory.length;
        let mut position = directory.offset;
        let mut continuation: Option<(Vec<u8>, usize)> = None;
        while position < end {
            let mut bytes = [0u8; 255];
            image.read_at(position, &mut bytes[..1])?;
            let length = bytes[0] as usize;
            if length == 0 {
                let next = (position / block_size + 1) * block_size;
                // ISO records cannot continue past zero padding in this block.
                while position < next.min(end) {
                    let count = ((next.min(end) - position) as usize).min(bytes.len());
                    image.read_at(position, &mut bytes[..count])?;
                    if bytes[..count].iter().any(|byte| *byte != 0) {
                        return Err(invalid("Invalid ISO directory padding").into());
                    }
                    position += count as u64;
                }
                continue;
            }
            if length < 34
                || position + length as u64 > end
                || position % block_size + length as u64 > block_size
            {
                return Err(invalid("Invalid ISO directory record bounds").into());
            }
            image.read_at(position + 1, &mut bytes[1..length])?;
            position += length as u64;
            let entry = record(&bytes[..length], block_size)?;
            if entry.raw_name == [0] || entry.raw_name == [1] {
                if !entry.directory || continuation.is_some() {
                    return Err(invalid("Invalid ISO special directory record").into());
                }
                continue;
            }
            let identifier = std::str::from_utf8(&entry.raw_name)
                .map_err(|_| invalid("Invalid ISO file identifier encoding"))?;
            // Match the existing ISO listing's version and trailing-dot handling.
            let display = identifier.split(';').next().unwrap_or("");
            let display = display.strip_suffix('.').unwrap_or(display);
            safe_iso_component(display)?;
            let path = parent.join(display);
            let relative = path
                .to_str()
                .ok_or_else(|| invalid("Invalid ISO entry encoding"))?;
            image.budget.entry(relative, 0)?;
            image.check_range(entry.extent.offset, entry.extent.length)?;
            if let Some((previous, _)) = &continuation {
                if previous != &entry.raw_name || entry.directory {
                    return Err(invalid("Invalid ISO multi-extent continuation").into());
                }
            } else if !paths.insert(path.clone()) {
                return Err(invalid("Ambiguous ISO archive file").into());
            }
            if entry.directory {
                if !entry.final_extent {
                    return Err(invalid("Multi-extent ISO directories are not supported").into());
                }
                directories.push((path.clone(), entry.extent));
            }
            let entry_index = if let Some((_, entry_index)) = &continuation {
                entries[*entry_index].extents.push(entry.extent);
                *entry_index
            } else {
                let entry_index = entries.len();
                entries.push(Entry {
                    path,
                    directory: entry.directory,
                    extents: if entry.directory {
                        Vec::new()
                    } else {
                        vec![entry.extent]
                    },
                });
                entry_index
            };
            continuation = (!entry.final_extent).then_some((entry.raw_name, entry_index));
        }
        if continuation.is_some() {
            return Err(invalid("Incomplete ISO multi-extent file").into());
        }
    }
    Ok(entries)
}

fn image<'a>(
    file: File,
    budget: &'a ArchiveBudget,
    checkpoint: &'a dyn Fn() -> io::Result<()>,
) -> io::Result<Image<'a>> {
    Ok(Image {
        length: file.metadata()?.len(),
        file,
        budget,
        checkpoint,
    })
}

#[cfg(test)]
fn copy_extents<R: Read + Seek>(
    image: &mut Image<'_, R>,
    extents: &[Extent],
    output: &mut File,
) -> io::Result<()> {
    let mut buffer = [0u8; 64 * 1024];
    for extent in extents {
        let mut offset = 0;
        while offset < extent.length {
            let count = (extent.length - offset).min(buffer.len() as u64) as usize;
            image.read_at(extent.offset + offset, &mut buffer[..count])?;
            image.budget.charge_written(count)?;
            output.write_all(&buffer[..count])?;
            offset += count as u64;
        }
    }
    Ok(())
}

pub(super) fn indexed_files(
    archive: &Path,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<Vec<FileEntry>, CommandError> {
    let parents = HeldParents::new(archive)?;
    let source = parents.resolved_path(archive)?;
    indexed_file(archive, platform().open_archive_source(&source, false)?, checkpoint)
}
pub(super) fn indexed_file(
    archive: &Path,
    file: File,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<Vec<FileEntry>, CommandError> {
    let budget = ArchiveBudget::new(ArchiveLimits::LISTING);
    let mut image = image(
        file,
        &budget,
        checkpoint,
    )?;
    let archive_origin = std::sync::Arc::new(archive.to_path_buf());
    index(&mut image)?
        .iter()
        .map(|entry| {
            budget.checkpoint(checkpoint)?;
            Ok(entry.file_entry(&archive_origin)?)
        })
        .collect()
}

pub(super) fn list_iso(archive: &Path, internal: &str) -> Result<Vec<FileEntry>, CommandError> {
    let requested = relative_archive_path(internal)?;
    let entries = indexed_files(archive, &|| Ok(()))?;
    if !requested.as_os_str().is_empty()
        && !entries
            .iter()
            .any(|entry| entry.is_dir && Path::new(&entry.path) == archive.join(&requested))
    {
        return Err(invalid("Archive directory not found").into());
    }
    let directory = archive.join(requested);
    Ok(entries
        .into_iter()
        .filter(|entry| Path::new(&entry.path).parent() == Some(directory.as_path()))
        .collect())
}

#[cfg(test)]
pub(super) fn extract_iso(
    file: File,
    root: &ExtractionRoot,
    provenance: Option<&[u8]>,
    checkpoint: &dyn Fn() -> io::Result<()>,
    budget: &ArchiveBudget,
) -> Result<(), CommandError> {
    let mut image = image(file, budget, checkpoint)?;
    let entries = index(&mut image)?;
    // Validate the complete index and aggregate size before creating any entry.
    for entry in &entries {
        budget.checkpoint(checkpoint)?;
        budget.declare_bytes(entry.size()?)?;
    }
    for entry in entries {
        budget.checkpoint(checkpoint)?;
        let destination = root.destination(entry.path.to_str().unwrap(), entry.directory)?;
        if !entry.directory {
            let mut output = platform().create_archive_file(&destination.path)?;
            copy_extents(&mut image, &entry.extents, &mut output)?;
            if let Some(data) = provenance {
                platform().write_file_provenance(&destination.path, data)?;
            }
        }
    }
    Ok(())
}

type EntryCallback<'a> =
    dyn FnMut(&crate::utils::archive_io_protocol::Entry, Option<&mut dyn Read>) -> io::Result<()> + 'a;

/// The same checked ISO index, with a seekable source and streamed output only.
pub(super) fn stream_iso<R: Read + Seek>(
    source: R,
    length: u64,
    selected: Option<&Path>,
    budget: &ArchiveBudget,
    checkpoint: &dyn Fn() -> io::Result<()>,
    entry_callback: &mut EntryCallback<'_>,
    manifest_end: &mut dyn FnMut() -> io::Result<()>,
) -> Result<(), CommandError> {
    let mut image = Image {
        file: source,
        length,
        budget,
        checkpoint,
    };
    let entries = index(&mut image)?;
    let mut found = false;
    for (id, entry) in entries.iter().enumerate() {
        let chosen = selected.is_none_or(|selected| entry.path == selected);
        if chosen {
            if selected.is_some() && entry.directory {
                return Err(invalid("Archive item is not a file").into());
            }
            budget.declare_bytes(entry.size()?)?;
            found = true;
        }
        let meta = iso_meta(id as u64, entry)?;
        entry_callback(&meta, None)?;
    }
    if selected.is_some() && !found {
        return Err(invalid("Archive file not found").into());
    }
    manifest_end()?;
    for (id, entry) in entries.iter().enumerate() {
        if selected.is_some_and(|selected| entry.path != selected) {
            continue;
        }
        let meta = iso_meta(id as u64, entry)?;
        let mut reader = ExtentReader {
            image: &mut image,
            extents: &entry.extents,
            index: 0,
            offset: 0,
        };
        entry_callback(&meta, Some(&mut reader))?;
    }
    Ok(())
}

fn iso_meta(id: u64, entry: &Entry) -> io::Result<crate::utils::archive_io_protocol::Entry> {
    Ok(crate::utils::archive_io_protocol::Entry {
        id,
        name: entry
            .path
            .to_str()
            .ok_or_else(|| invalid("Invalid ISO entry encoding"))?
            .replace('\\', "/"),
        directory: entry.directory,
        size: entry.size()?,
        modified: None,
        created: None,
        accessed: None,
        mode: 0o644,
    })
}

struct ExtentReader<'a, 'b, R> {
    image: &'a mut Image<'b, R>,
    extents: &'a [Extent],
    index: usize,
    offset: u64,
}
impl<R: Read + Seek> Read for ExtentReader<'_, '_, R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        while let Some(extent) = self.extents.get(self.index) {
            if self.offset == extent.length {
                self.index += 1;
                self.offset = 0;
                continue;
            }
            let count = (extent.length - self.offset).min(bytes.len() as u64) as usize;
            if count == 0 {
                return Ok(0);
            }
            self.image
                .read_at(extent.offset + self.offset, &mut bytes[..count])?;
            self.offset += count as u64;
            return Ok(count);
        }
        Ok(0)
    }
}

#[cfg(test)]
pub(crate) fn extract_iso_file_for_open(
    archive: &Path,
    internal: &str,
    target: &Path,
    limits: ArchiveLimits,
) -> Result<PathBuf, CommandError> {
    extract_with_checkpoint(archive, internal, target, limits, &|| Ok(()))
}

#[cfg(test)]
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
    let archive = parents.resolved_path(archive)?;
    let file = platform().open_archive_source(&archive, false)?;
    let budget = ArchiveBudget::new(limits);
    let mut image = image(file, &budget, checkpoint)?;
    let selected = index(&mut image)?
        .into_iter()
        .find(|entry| entry.path == requested)
        .ok_or_else(|| invalid("Archive file not found"))?;
    if selected.directory {
        return Err(invalid("Archive item is not a file").into());
    }
    budget.declare_bytes(selected.size()?)?;
    budget.checkpoint(checkpoint)?;
    let provenance = platform().read_file_provenance(&archive)?;
    let root = ExtractionRoot::new(target)?;
    let basename = requested
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid("Archive item is not a file"))?;
    let destination = root.destination(basename, false)?;
    let mut output = platform().create_archive_file(&destination.path)?;
    copy_extents(&mut image, &selected.extents, &mut output)?;
    if let Some(data) = provenance {
        platform().write_file_provenance(&destination.path, &data)?;
    }
    Ok(destination.path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::archive::test_iso_image;
    use crate::models::error::{ArchiveLimitFailure, ArchiveLimitKind};
    use std::fs;

    fn limits() -> ArchiveLimits {
        ArchiveLimits {
            max_bytes: 4,
            max_entries: 16,
            ..ArchiveLimits::OPEN_FILE
        }
    }

    fn entries(bytes: &[u8], sector: usize) -> Vec<usize> {
        let mut offsets = Vec::new();
        let mut offset = sector * 2048;
        while bytes[offset] != 0 {
            offsets.push(offset);
            offset += bytes[offset] as usize;
        }
        offsets
    }

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let folder = tempfile::tempdir().unwrap();
        let archive = folder.path().join("disc.iso");
        test_iso_image(&archive);
        (folder, archive)
    }

    fn assert_invalid_all_routes(archive: &Path) {
        assert!(list_iso(archive, "").is_err());
        let full = tempfile::tempdir().unwrap();
        let budget = ArchiveBudget::new(ArchiveLimits::EXTRACTION);
        assert!(extract_iso(
            File::open(archive).unwrap(),
            &ExtractionRoot::new(full.path()).unwrap(),
            None,
            &|| Ok(()),
            &budget,
        )
        .is_err());
        assert_eq!(fs::read_dir(full.path()).unwrap().count(), 0);
    }

    #[test]
    fn retrying_iso_header_read_does_not_swallow_a_checkpoint_cancellation() {
        let (_folder, archive) = fixture();
        let target = tempfile::tempdir().unwrap();
        let calls = std::cell::Cell::new(0);
        let error = extract_with_checkpoint(&archive, "ROOT.TXT", target.path(), limits(), &|| {
            calls.set(calls.get() + 1);
            if calls.get() == 2 {
                Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "cancelled in ISO header",
                ))
            } else {
                Ok(())
            }
        })
        .expect_err("ISO preparation continued after cancellation");
        assert!(error.to_string().contains("cancelled in ISO header"));
        assert_eq!(calls.get(), 2);
        assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
    }

    #[test]
    fn targeted_iso_open_reads_only_selected_extents_and_preserves_external_names() {
        for extension in ["iso", "ISO", "img"] {
            let (folder, initial) = fixture();
            let archive = folder.path().join(format!("disc.{extension}"));
            if archive != initial {
                fs::rename(initial, &archive).unwrap();
            }
            let mut bytes = fs::read(&archive).unwrap();
            let other = entries(&bytes, 20)[3];
            let large = 2 * 1024 * 1024u32;
            bytes[other + 10..other + 14].copy_from_slice(&large.to_le_bytes());
            bytes[other + 14..other + 18].copy_from_slice(&large.to_be_bytes());
            bytes.resize(22 * 2048 + large as usize, 0);
            // The selected nested entry is unchanged, despite the huge neighbor.
            bytes[23 * 2048..23 * 2048 + 11].copy_from_slice(b"ISO content");
            fs::write(&archive, bytes).unwrap();
            let target = tempfile::tempdir().unwrap();
            let allowance = ArchiveLimits {
                max_bytes: 11,
                ..limits()
            };
            let selected =
                extract_iso_file_for_open(&archive, "DOCS\\README.TXT", target.path(), allowance)
                    .unwrap();
            assert_eq!(selected, target.path().join("README.TXT"));
            assert_eq!(fs::read(selected).unwrap(), b"ISO content");
            assert_eq!(fs::read_dir(target.path()).unwrap().count(), 1);
            let refused = tempfile::tempdir().unwrap();
            let error = extract_iso_file_for_open(&archive, "ROOT.TXT", refused.path(), allowance)
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

    #[test]
    fn targeted_iso_open_validates_late_unsafe_names_and_ambiguous_versions_before_output() {
        for name in [
            b"C:BADTXT;1".as_slice(),
            b"../BAD.TXT",
            b"NUL.TXT;1 ",
            b"ROOT.TXT;2",
            b"BAD\\XTXT;1",
        ] {
            let (_folder, archive) = fixture();
            let mut bytes = fs::read(&archive).unwrap();
            let record = entries(&bytes, 20)[3];
            let duplicate = bytes[record..record + bytes[record] as usize].to_vec();
            let after = record + duplicate.len();
            bytes[after..after + duplicate.len()].copy_from_slice(&duplicate);
            assert_eq!(name.len(), 10);
            bytes[after + 33..after + 43].copy_from_slice(name);
            fs::write(&archive, bytes).unwrap();
            let target = tempfile::tempdir().unwrap();
            assert!(
                extract_iso_file_for_open(&archive, "ROOT.TXT", target.path(), limits()).is_err()
            );
            assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
            assert_invalid_all_routes(&archive);
        }
    }

    #[test]
    fn targeted_iso_open_refuses_corrupt_records_cycles_and_truncated_ranges() {
        for variant in 0..8 {
            let (_folder, archive) = fixture();
            let mut bytes = fs::read(&archive).unwrap();
            let directory = entries(&bytes, 20)[2];
            let file = entries(&bytes, 20)[3];
            match variant {
                0 => bytes[file + 32] = 255, // Identifier exceeds its record.
                1 => bytes[file] = 20,
                2 => {
                    // Root is revisited via a directory entry.
                    bytes[directory + 2..directory + 6].copy_from_slice(&20u32.to_le_bytes());
                    bytes[directory + 6..directory + 10].copy_from_slice(&20u32.to_be_bytes());
                }
                3 => bytes.truncate(23 * 2048 + 2),
                4 => bytes[file + 14] ^= 1, // Mixed-endian size disagrees.
                5 => bytes[file + 25] |= 128, // Missing next extent.
                6 => bytes[16 * 2048 + 128..16 * 2048 + 132].fill(0),
                7 => bytes[file + 26] = 1, // Unsupported interleaving.
                _ => unreachable!(),
            }
            fs::write(&archive, bytes).unwrap();
            let target = tempfile::tempdir().unwrap();
            assert!(
                extract_iso_file_for_open(&archive, "ROOT.TXT", target.path(), limits()).is_err(),
                "variant {variant}"
            );
            assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
            assert_invalid_all_routes(&archive);
        }
    }

    #[test]
    fn targeted_iso_open_preserves_multi_extent_order_and_bounds_its_total() {
        let (_folder, archive) = fixture();
        let mut bytes = fs::read(&archive).unwrap();
        let first = entries(&bytes, 20)[3];
        let mut next = bytes[first..first + bytes[first] as usize].to_vec();
        bytes[first + 25] |= 128;
        next[2..6].copy_from_slice(&24u32.to_le_bytes());
        next[6..10].copy_from_slice(&24u32.to_be_bytes());
        let offset = first + next.len();
        bytes[offset..offset + next.len()].copy_from_slice(&next);
        bytes[24 * 2048..24 * 2048 + 4].copy_from_slice(b"tail");
        fs::write(&archive, bytes).unwrap();
        let target = tempfile::tempdir().unwrap();
        let output = extract_iso_file_for_open(
            &archive,
            "ROOT.TXT",
            target.path(),
            ArchiveLimits {
                max_bytes: 8,
                ..limits()
            },
        )
        .unwrap();
        assert_eq!(fs::read(output).unwrap(), b"roottail");
        let listed = list_iso(&archive, "").unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(
            listed
                .iter()
                .find(|entry| entry.name == "ROOT.TXT")
                .unwrap()
                .size,
            8
        );
        let full = tempfile::tempdir().unwrap();
        let budget = ArchiveBudget::new(ArchiveLimits::EXTRACTION);
        extract_iso(
            File::open(&archive).unwrap(),
            &ExtractionRoot::new(full.path()).unwrap(),
            None,
            &|| Ok(()),
            &budget,
        )
        .unwrap();
        assert_eq!(fs::read(full.path().join("ROOT.TXT")).unwrap(), b"roottail");
        assert_eq!(
            fs::read(full.path().join("DOCS/README.TXT")).unwrap(),
            b"ISO content"
        );
        let refused = tempfile::tempdir().unwrap();
        let error =
            extract_iso_file_for_open(&archive, "ROOT.TXT", refused.path(), limits()).unwrap_err();
        assert_eq!(
            ArchiveLimitFailure::from_message(&error.to_string())
                .unwrap()
                .kind,
            ArchiveLimitKind::Bytes
        );
        assert_eq!(fs::read_dir(refused.path()).unwrap().count(), 0);
    }

    #[test]
    fn targeted_iso_open_keeps_count_depth_and_deadline_errors_structured() {
        let (_folder, archive) = fixture();
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
                    max_depth: 1,
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
            let error = extract_iso_file_for_open(&archive, "ROOT.TXT", target.path(), allowance)
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
        let error = extract_with_checkpoint(&archive, "ROOT.TXT", target.path(), limits(), &|| {
            Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
        })
        .unwrap_err();
        assert!(error.to_string().contains("cancelled"));
        assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
    }

    #[test]
    fn full_iso_preflight_keeps_limit_errors_structured_and_creates_no_entries() {
        let (_folder, archive) = fixture();
        for (allowance, kind) in [
            (
                ArchiveLimits {
                    max_bytes: 14,
                    ..ArchiveLimits::EXTRACTION
                },
                ArchiveLimitKind::Bytes,
            ),
            (
                ArchiveLimits {
                    max_entries: 2,
                    ..ArchiveLimits::EXTRACTION
                },
                ArchiveLimitKind::Entries,
            ),
            (
                ArchiveLimits {
                    max_depth: 1,
                    ..ArchiveLimits::EXTRACTION
                },
                ArchiveLimitKind::Depth,
            ),
            (
                ArchiveLimits {
                    max_duration: std::time::Duration::ZERO,
                    ..ArchiveLimits::EXTRACTION
                },
                ArchiveLimitKind::Seconds,
            ),
        ] {
            let target = tempfile::tempdir().unwrap();
            let budget = ArchiveBudget::new(allowance);
            let error = extract_iso(
                File::open(&archive).unwrap(),
                &ExtractionRoot::new(target.path()).unwrap(),
                None,
                &|| Ok(()),
                &budget,
            )
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

    #[test]
    fn full_iso_extracts_the_opened_source_and_preserves_provenance_and_existing_files() {
        let (folder, archive) = fixture();
        let source = File::open(&archive).unwrap();
        fs::rename(&archive, folder.path().join("original.iso")).unwrap();
        fs::write(&archive, b"replacement must not be read").unwrap();
        let target = tempfile::tempdir().unwrap();
        let budget = ArchiveBudget::new(ArchiveLimits::EXTRACTION);
        let provenance = b"[ZoneTransfer]\r\nZoneId=3\r\n";
        extract_iso(
            source,
            &ExtractionRoot::new(target.path()).unwrap(),
            Some(provenance),
            &|| Ok(()),
            &budget,
        )
        .unwrap();
        assert_eq!(fs::read(target.path().join("ROOT.TXT")).unwrap(), b"root");
        assert_eq!(
            fs::read(target.path().join("DOCS/README.TXT")).unwrap(),
            b"ISO content"
        );
        #[cfg(target_os = "windows")]
        assert_eq!(
            platform()
                .read_file_provenance(&target.path().join("DOCS/README.TXT"))
                .unwrap()
                .as_deref(),
            Some(provenance.as_slice())
        );
        let budget = ArchiveBudget::new(ArchiveLimits::EXTRACTION);
        assert!(extract_iso(
            File::open(folder.path().join("original.iso")).unwrap(),
            &ExtractionRoot::new(target.path()).unwrap(),
            None,
            &|| Ok(()),
            &budget,
        )
        .is_err());
        assert_eq!(fs::read(target.path().join("ROOT.TXT")).unwrap(), b"root");
        assert_eq!(fs::read(&archive).unwrap(), b"replacement must not be read");
    }

    #[test]
    fn targeted_iso_open_bounds_directory_padding_reads_with_a_structured_error() {
        let (_folder, archive) = fixture();
        let mut bytes = fs::read(&archive).unwrap();
        let original = bytes[20 * 2048..21 * 2048].to_vec();
        let root = 16 * 2048 + 156;
        bytes[root + 2..root + 6].copy_from_slice(&32u32.to_le_bytes());
        bytes[root + 6..root + 10].copy_from_slice(&32u32.to_be_bytes());
        let size = 1024 * 1024u32;
        bytes[root + 10..root + 14].copy_from_slice(&size.to_le_bytes());
        bytes[root + 14..root + 18].copy_from_slice(&size.to_be_bytes());
        bytes.resize(32 * 2048 + size as usize, 0);
        bytes[32 * 2048..33 * 2048].copy_from_slice(&original);
        fs::write(&archive, bytes).unwrap();
        let target = tempfile::tempdir().unwrap();
        let error =
            extract_iso_file_for_open(&archive, "ROOT.TXT", target.path(), limits()).unwrap_err();
        let failure = ArchiveLimitFailure::from_message(&error.to_string()).unwrap();
        assert_eq!(failure.kind, ArchiveLimitKind::Decoded);
        assert_eq!(failure.limit, 4 + 16 * 4096);
        assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
    }

    #[test]
    fn targeted_iso_open_skips_extended_attributes_before_copying_content() {
        let (_folder, archive) = fixture();
        let mut bytes = fs::read(&archive).unwrap();
        let file = entries(&bytes, 20)[3];
        bytes[file + 1] = 2;
        bytes[24 * 2048..24 * 2048 + 4].copy_from_slice(b"safe");
        fs::write(&archive, bytes).unwrap();
        let target = tempfile::tempdir().unwrap();
        let output =
            extract_iso_file_for_open(&archive, "ROOT.TXT", target.path(), limits()).unwrap();
        assert_eq!(fs::read(output).unwrap(), b"safe");
    }

    #[test]
    fn targeted_iso_open_creates_an_empty_file_for_a_zero_length_extent() {
        let (_folder, archive) = fixture();
        let mut bytes = fs::read(&archive).unwrap();
        let file = entries(&bytes, 20)[3];
        bytes[file + 10..file + 18].fill(0);
        fs::write(&archive, bytes).unwrap();
        let target = tempfile::tempdir().unwrap();
        let output =
            extract_iso_file_for_open(&archive, "ROOT.TXT", target.path(), limits()).unwrap();
        assert_eq!(fs::read(output).unwrap(), b"");
        assert_eq!(fs::read_dir(target.path()).unwrap().count(), 1);
    }

    #[test]
    fn targeted_iso_open_refuses_absent_directory_and_existing_output_selections() {
        let (_folder, archive) = fixture();
        for internal in ["DOCS", "missing.txt", "../ROOT.TXT", "C:ROOT.TXT", ""] {
            let target = tempfile::tempdir().unwrap();
            assert!(
                extract_iso_file_for_open(&archive, internal, target.path(), limits()).is_err()
            );
            assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
        }
        let target = tempfile::tempdir().unwrap();
        let existing = target.path().join("ROOT.TXT");
        fs::write(&existing, b"foreign").unwrap();
        assert!(extract_iso_file_for_open(&archive, "ROOT.TXT", target.path(), limits()).is_err());
        assert_eq!(fs::read(existing).unwrap(), b"foreign");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn targeted_iso_open_retains_the_source_and_internet_provenance_after_parent_retargeting() {
        use crate::platform::windows_reparse_tests::{junction, Retarget};
        use std::cell::Cell;
        let (folder, archive) = fixture();
        let chosen = folder.path().join("chosen");
        let outside = folder.path().join("outside");
        fs::create_dir(&chosen).unwrap();
        fs::create_dir(&outside).unwrap();
        let source = chosen.join("disc.iso");
        fs::rename(&archive, &source).unwrap();
        let foreign = outside.join("disc.iso");
        test_iso_image(&foreign);
        let mut bytes = fs::read(&foreign).unwrap();
        bytes[22 * 2048..22 * 2048 + 4].copy_from_slice(b"evil");
        fs::write(&foreign, &bytes).unwrap();
        let provenance = b"[ZoneTransfer]\r\nZoneId=3\r\n";
        platform()
            .write_file_provenance(&source, provenance)
            .unwrap();
        let link = folder.path().join("link");
        let template = folder.path().join("template");
        junction(&link, &chosen);
        junction(&template, &outside);
        let target = tempfile::tempdir().unwrap();
        let retarget = Retarget::new(&link, &template);
        let attempted = Cell::new(false);
        let result = extract_with_checkpoint(
            &link.join("disc.iso"),
            "ROOT.TXT",
            target.path(),
            limits(),
            &|| {
                if !attempted.replace(true) {
                    assert!(File::options().write(true).open(&source).is_err());
                    assert!(fs::rename(&source, chosen.join("moved.iso")).is_err());
                    retarget.apply();
                }
                Ok(())
            },
        )
        .unwrap();
        assert!(attempted.get());
        assert_eq!(fs::read(&result).unwrap(), b"root");
        assert_eq!(
            platform().read_file_provenance(&result).unwrap().as_deref(),
            Some(&provenance[..])
        );
        assert_eq!(fs::read(&foreign).unwrap(), bytes);
        assert!(File::options().write(true).open(&source).is_ok());
        retarget.clear();
        drop(retarget);
        fs::remove_dir(link).unwrap();
        fs::remove_dir(template).unwrap();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn targeted_iso_open_refuses_a_destination_junction_without_touching_its_target() {
        use crate::platform::windows_reparse_tests::{junction, Retarget};
        use std::cell::Cell;
        let (folder, archive) = fixture();
        let target = folder.path().join("target");
        let outside = folder.path().join("outside");
        fs::create_dir(&target).unwrap();
        fs::create_dir(&outside).unwrap();
        let witness = outside.join("ROOT.TXT");
        fs::write(&witness, b"foreign").unwrap();
        let template = folder.path().join("template");
        junction(&template, &outside);
        let retarget = Retarget::new(&target, &template);
        let changed = Cell::new(false);
        let result = extract_with_checkpoint(&archive, "ROOT.TXT", &target, limits(), &|| {
            if !changed.replace(true) {
                retarget.apply();
            }
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(fs::read(witness).unwrap(), b"foreign");
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
        retarget.clear();
        drop(retarget);
        fs::remove_dir(template).unwrap();
        assert_eq!(fs::read_dir(target).unwrap().count(), 0);
    }
}
