//! Filesystem-free archive codecs. The broker owns every input and output object.
use crate::models::CommandError;
use crate::utils::archive_io_protocol::{
    invalid, Entry, Format, Frame, Limits, Message, Operation, Quality, Timestamp, MAX_BLOCK,
};
use crate::utils::archive_io_stream::{Connection, Exchange, Reader, Writer};
use crate::utils::archive_limits::{ArchiveBudget, ArchiveLimits, BudgetReader};
use crate::utils::archive_security::relative_archive_path;
use std::io::{self, Read, Seek, Write};
use std::time::Duration;

fn request<T: Exchange>(connection: &Connection<T>, frame: Frame) -> io::Result<Frame> {
    connection.lock().map_err(|_| invalid())?.exchange(frame)
}
fn notify<T: Exchange>(connection: &Connection<T>, frame: Frame) -> io::Result<()> {
    let reply = request(connection, frame)?;
    if reply
        != Frame::new(Message::Ack {
            pause: false,
            turbo: false,
        })
    {
        return Err(invalid());
    }
    Ok(())
}
fn archive_limits(limits: Limits) -> io::Result<ArchiveLimits> {
    Ok(ArchiveLimits {
        max_entries: limits.entries.try_into().map_err(|_| invalid())?,
        max_bytes: limits.bytes,
        max_depth: limits.depth.try_into().map_err(|_| invalid())?,
        max_duration: Duration::from_secs(limits.seconds),
    })
}
fn empty_entry(id: u64, name: String, directory: bool, size: u64) -> Entry {
    Entry {
        id,
        name,
        directory,
        size: if directory { 0 } else { size },
        modified: None,
        created: None,
        accessed: None,
        mode: if directory { 0o755 } else { 0o644 },
    }
}
fn metadata<T: Exchange>(connection: &Connection<T>, id: u64) -> io::Result<Entry> {
    let frame = request(connection, Frame::new(Message::Source { id }))?;
    let Message::Entry { entry } = frame.message else {
        return Err(invalid());
    };
    if entry.id != id || !frame.data.is_empty() {
        return Err(invalid());
    }
    entry.validate()?;
    Ok(entry)
}

struct ChunkWriter<'a, T> {
    connection: &'a Connection<T>,
    id: u64,
    offset: u64,
}
impl<T: Exchange> Write for ChunkWriter<'_, T> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = bytes.len().min(MAX_BLOCK);
        if count == 0 {
            return Ok(0);
        }
        notify(
            self.connection,
            Frame::data(
                Message::Chunk {
                    id: self.id,
                    offset: self.offset,
                },
                bytes[..count].to_vec(),
            ),
        )?;
        self.offset = self.offset.checked_add(count as u64).ok_or_else(invalid)?;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn copy_entry<T: Exchange>(
    connection: &Connection<T>,
    entry: &Entry,
    input: &mut (impl Read + ?Sized),
    budget: &ArchiveBudget,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> io::Result<()> {
    notify(connection, Frame::new(Message::Begin { id: entry.id }))?;
    if !entry.directory {
        let mut writer = ChunkWriter {
            connection,
            id: entry.id,
            offset: 0,
        };
        if budget.copy(input, &mut writer, checkpoint)? != entry.size {
            return Err(invalid());
        }
    }
    notify(connection, Frame::new(Message::End { id: entry.id }))
}

pub fn run<T: Exchange + 'static>(
    connection: Connection<T>,
    operation: Operation,
) -> Result<(), CommandError> {
    let limits = match &operation {
        Operation::Decode { limits, .. } | Operation::Encode { limits, .. } => *limits,
    };
    let budget = ArchiveBudget::new(archive_limits(limits)?);
    let checkpoint = || notify(&connection, Frame::new(Message::Checkpoint));
    match operation {
        Operation::Encode {
            format,
            quality,
            entries,
            output_limit,
            ..
        } => {
            encode(
                &connection,
                format,
                quality,
                entries,
                output_limit,
                &budget,
                &checkpoint,
            )?;
        }
        Operation::Decode {
            format,
            source_size,
            selected,
            ..
        } => {
            decode(
                &connection,
                format,
                source_size,
                selected.as_deref(),
                &budget,
                &checkpoint,
            )?;
        }
    }
    notify(&connection, Frame::new(Message::Finished))?;
    Ok(())
}

fn encode<T: Exchange>(
    connection: &Connection<T>,
    format: Format,
    quality: Quality,
    entries: u64,
    output_limit: u64,
    budget: &ArchiveBudget,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<(), CommandError> {
    budget.check_entry_count(entries.try_into().map_err(|_| invalid())?)?;
    let output = Writer::new(connection.clone(), output_limit);
    let level = match quality {
        Quality::Fast => 1,
        Quality::Normal => 6,
        Quality::Best => 9,
    };
    match format {
        Format::Zip => {
            let mut writer = zip::ZipWriter::new(io::BufWriter::new(output));
            for id in 0..entries {
                budget.checkpoint(checkpoint)?;
                let entry = metadata(connection, id)?;
                budget.entry(&entry.name, entry.size)?;
                notify(connection, Frame::new(Message::Begin { id }))?;
                let options = zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated)
                    .compression_level(Some(level));
                if entry.directory {
                    writer
                        .add_directory(format!("{}/", entry.name.trim_end_matches('/')), options)
                        .map_err(io::Error::other)?;
                } else {
                    writer
                        .start_file(entry.name, options)
                        .map_err(io::Error::other)?;
                    let mut reader = Reader::new(connection.clone(), id, entry.size);
                    if budget.copy(&mut reader, &mut writer, checkpoint)? != entry.size {
                        return Err(invalid().into());
                    }
                }
                notify(connection, Frame::new(Message::End { id }))?;
            }
            writer.finish().map_err(io::Error::other)?.flush()?;
        }
        Format::SevenZip => {
            let mut writer = sevenz_rust2::ArchiveWriter::new(output)
                .map_err(super::archive_seven_open::archive_error)?;
            writer.set_content_methods(vec![
                sevenz_rust2::encoder_options::Lzma2Options::from_level(level as u32).into(),
            ]);
            for id in 0..entries {
                budget.checkpoint(checkpoint)?;
                let entry = metadata(connection, id)?;
                budget.entry(&entry.name, entry.size)?;
                notify(connection, Frame::new(Message::Begin { id }))?;
                let mut seven = if entry.directory {
                    sevenz_rust2::ArchiveEntry::new_directory(&entry.name)
                } else {
                    sevenz_rust2::ArchiveEntry::new_file(&entry.name)
                };
                for (time, value, flag) in [
                    (
                        entry.modified,
                        &mut seven.last_modified_date,
                        &mut seven.has_last_modified_date,
                    ),
                    (
                        entry.created,
                        &mut seven.creation_date,
                        &mut seven.has_creation_date,
                    ),
                    (
                        entry.accessed,
                        &mut seven.access_date,
                        &mut seven.has_access_date,
                    ),
                ] {
                    if let Some(time) = time {
                        if let Ok(date) = time.system()?.try_into() {
                            *value = date;
                            *flag = u64::from(date) > 0;
                        }
                    }
                }
                let reader =
                    (!entry.directory).then(|| Reader::new(connection.clone(), id, entry.size));
                let encoded = writer
                    .push_archive_entry(seven, reader)
                    .map_err(super::archive_seven_open::archive_error)?;
                if !entry.directory && encoded.size() != entry.size {
                    return Err(invalid().into());
                }
                notify(connection, Frame::new(Message::End { id }))?;
            }
            writer.finish()?.flush()?;
        }
        Format::Tar | Format::TarZst => {
            let level = match quality {
                Quality::Fast => 1,
                Quality::Normal => 3,
                Quality::Best => 19,
            };
            if format == Format::TarZst {
                let encoder = zstd::stream::write::Encoder::new(output, level)?;
                let mut writer = tar::Builder::new(encoder);
                encode_tar(connection, entries, &mut writer, budget, checkpoint)?;
                writer.into_inner()?.finish()?.flush()?;
            } else {
                let mut writer = tar::Builder::new(io::BufWriter::new(output));
                encode_tar(connection, entries, &mut writer, budget, checkpoint)?;
                writer.into_inner()?.flush()?;
            }
        }
        _ => return Err(invalid().into()),
    }
    Ok(())
}

fn encode_tar<T: Exchange, W: Write>(
    connection: &Connection<T>,
    entries: u64,
    writer: &mut tar::Builder<W>,
    budget: &ArchiveBudget,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> io::Result<()> {
    for id in 0..entries {
        budget.checkpoint(checkpoint)?;
        let entry = metadata(connection, id)?;
        budget.entry(&entry.name, entry.size)?;
        notify(connection, Frame::new(Message::Begin { id }))?;
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(if entry.directory {
            tar::EntryType::Directory
        } else {
            tar::EntryType::Regular
        });
        header.set_size(entry.size);
        header.set_mode(entry.mode);
        if let Some(time) = entry.modified.filter(|time| time.seconds >= 0) {
            header.set_mtime(time.seconds as u64);
        }
        header.set_cksum();
        writer.append_data(
            &mut header,
            entry.name,
            Reader::new(connection.clone(), id, entry.size),
        )?;
        notify(connection, Frame::new(Message::End { id }))?;
    }
    writer.finish()
}

fn decode<T: Exchange + 'static>(
    connection: &Connection<T>,
    format: Format,
    size: u64,
    selected: Option<&str>,
    budget: &ArchiveBudget,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<(), CommandError> {
    let requested = selected.map(relative_archive_path).transpose()?;
    match format {
        Format::Zip => {
            let mut archive = zip::ZipArchive::new(Reader::new(connection.clone(), 0, size))
                .map_err(io::Error::other)?;
            budget.check_entry_count(archive.len())?;
            let mut selection = None;
            for index in 0..archive.len() {
                budget.checkpoint(checkpoint)?;
                let entry = archive.by_index_raw(index).map_err(io::Error::other)?;
                if entry
                    .unix_mode()
                    .is_some_and(|mode| mode & 0o170000 == 0o120000)
                {
                    return Err(invalid().into());
                }
                let meta = empty_entry(
                    index as u64,
                    entry.name().into(),
                    entry.is_dir(),
                    entry.size(),
                );
                let chosen = requested.as_ref().is_none_or(|requested| {
                    relative_archive_path(entry.name()).is_ok_and(|path| path == *requested)
                });
                budget.entry(entry.name(), if chosen { meta.size } else { 0 })?;
                if requested.is_some() && chosen {
                    if selection.is_some() || meta.directory {
                        return Err(invalid().into());
                    }
                    selection = Some(index);
                }
                notify(connection, Frame::new(Message::Entry { entry: meta }))?;
            }
            if requested.is_some() && selection.is_none() {
                return Err(invalid().into());
            }
            notify(connection, Frame::new(Message::ManifestEnd))?;
            for index in 0..archive.len() {
                if requested.is_some() && selection != Some(index) {
                    continue;
                }
                let mut entry = archive.by_index(index).map_err(io::Error::other)?;
                let meta = empty_entry(
                    index as u64,
                    entry.name().into(),
                    entry.is_dir(),
                    entry.size(),
                );
                copy_entry(connection, &meta, &mut entry, budget, checkpoint)?;
            }
        }
        Format::SevenZip => {
            decode_seven(connection, size, requested.as_deref(), budget, checkpoint)?
        }
        Format::Tar | Format::TarGz | Format::TarXz | Format::TarBz2 | Format::TarZst => {
            decode_tar(
                connection,
                format,
                size,
                requested.as_deref(),
                budget,
                checkpoint,
            )?;
        }
        Format::Iso => {
            super::archive_iso_open::stream_iso(
                Reader::new(connection.clone(), 0, size),
                size,
                requested.as_deref(),
                budget,
                checkpoint,
                &mut |entry, input| {
                    if let Some(input) = input {
                        copy_entry(connection, entry, input, budget, checkpoint)?;
                    } else {
                        notify(
                            connection,
                            Frame::new(Message::Entry {
                                entry: entry.clone(),
                            }),
                        )?;
                    }
                    Ok(())
                },
                &mut || notify(connection, Frame::new(Message::ManifestEnd)),
            )?;
        }
    }
    Ok(())
}

fn seven_meta(id: u64, entry: &sevenz_rust2::ArchiveEntry) -> io::Result<Entry> {
    let mut meta = empty_entry(id, entry.name().into(), entry.is_directory(), entry.size());
    if entry.has_last_modified_date {
        meta.modified = Some(Timestamp::from_system(entry.last_modified_date().into())?);
    }
    if entry.has_access_date {
        meta.accessed = Some(Timestamp::from_system(entry.access_date().into())?);
    }
    if entry.has_creation_date {
        meta.created = Some(Timestamp::from_system(entry.creation_date().into())?);
    }
    meta.validate()?;
    Ok(meta)
}

fn decode_seven<T: Exchange>(
    connection: &Connection<T>,
    size: u64,
    requested: Option<&std::path::Path>,
    budget: &ArchiveBudget,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<(), CommandError> {
    let password = sevenz_rust2::Password::empty();
    let mut source = Reader::new(connection.clone(), 0, size);
    let archive = sevenz_rust2::Archive::read(&mut source, &password)
        .map_err(super::archive_seven_open::archive_error)?;
    budget.check_entry_count(archive.files.len())?;
    let mut selected = None;
    for (index, entry) in archive.files.iter().enumerate() {
        budget.checkpoint(checkpoint)?;
        super::archive_seven_open::validate_entry_type(entry)?;
        let meta = seven_meta(index as u64, entry)?;
        let chosen = requested.is_none_or(|requested| {
            relative_archive_path(entry.name()).is_ok_and(|path| path == requested)
        });
        budget.entry(entry.name(), if chosen { meta.size } else { 0 })?;
        if requested.is_some() && chosen {
            if selected.is_some() || meta.directory {
                return Err(invalid().into());
            }
            selected = Some(index);
        }
        notify(connection, Frame::new(Message::Entry { entry: meta }))?;
    }
    if requested.is_some() && selected.is_none() {
        return Err(invalid().into());
    }
    notify(connection, Frame::new(Message::ManifestEnd))?;
    if let Some(index) = selected {
        let entry = &archive.files[index];
        if !entry.has_stream {
            if entry.size() != 0 {
                return Err(invalid().into());
            }
            copy_entry(
                connection,
                &seven_meta(index as u64, entry)?,
                &mut io::empty(),
                budget,
                checkpoint,
            )?;
        } else {
            let block = archive
                .stream_map
                .file_block_index
                .get(index)
                .copied()
                .flatten()
                .filter(|block| *block < archive.blocks.len())
                .ok_or_else(invalid)?;
            let verify = !entry.has_crc && archive.blocks[block].has_crc;
            let mut found = false;
            let mut decoded = 0u64;
            sevenz_rust2::BlockDecoder::new(1, block, &archive, &password, &mut source)
                .for_each_entries(&mut |candidate, input| {
                    let mut input = BudgetReader {
                        reader: input,
                        budget,
                        checkpoint,
                    };
                    if std::ptr::eq(candidate, entry) {
                        copy_entry(
                            connection,
                            &seven_meta(index as u64, entry)?,
                            &mut input,
                            budget,
                            checkpoint,
                        )?;
                        decoded = decoded.checked_add(entry.size()).ok_or_else(invalid)?;
                        found = true;
                        Ok(verify)
                    } else {
                        let count = io::copy(&mut input, &mut io::sink())?;
                        if count != candidate.size() {
                            return Err(invalid().into());
                        }
                        decoded = decoded.checked_add(count).ok_or_else(invalid)?;
                        Ok(true)
                    }
                })
                .map_err(super::archive_seven_open::archive_error)?;
            if !found || (verify && decoded != archive.blocks[block].get_unpack_size()) {
                return Err(invalid().into());
            }
        }
    } else {
        // Rewind through an independent reader after preflight; no pathname reopen.
        let indices: std::collections::HashMap<_, _> = archive
            .files
            .iter()
            .enumerate()
            .map(|(index, entry)| (entry.name(), index))
            .collect();
        let mut seven =
            sevenz_rust2::ArchiveReader::new(Reader::new(connection.clone(), 0, size), password)
                .map_err(super::archive_seven_open::archive_error)?;
        seven.set_thread_count(1);
        seven
            .for_each_entries(|entry, input| {
                let index = *indices.get(entry.name()).ok_or_else(invalid)?;
                copy_entry(
                    connection,
                    &seven_meta(index as u64, entry)?,
                    input,
                    budget,
                    checkpoint,
                )?;
                Ok(true)
            })
            .map_err(super::archive_seven_open::archive_error)?;
    }
    Ok(())
}

fn tar_reader<T: Exchange + 'static>(
    connection: &Connection<T>,
    format: Format,
    size: u64,
) -> io::Result<Box<dyn Read>> {
    let source = Reader::new(connection.clone(), 0, size);
    Ok(match format {
        Format::TarGz => Box::new(flate2::read::GzDecoder::new(source)),
        Format::TarXz => Box::new(xz2::read::XzDecoder::new(source)),
        Format::TarBz2 => Box::new(bzip2::read::BzDecoder::new(source)),
        Format::TarZst => Box::new(zstd::stream::read::Decoder::new(source)?),
        Format::Tar => Box::new(source),
        _ => return Err(invalid()),
    })
}
fn tar_meta<R: Read>(id: u64, entry: &tar::Entry<'_, R>) -> io::Result<Entry> {
    let kind = entry.header().entry_type();
    if !kind.is_file() && !kind.is_dir() {
        return Err(invalid());
    }
    let name = entry.path()?.to_str().ok_or_else(invalid)?.to_owned();
    let mut meta = empty_entry(id, name, kind.is_dir(), entry.size());
    meta.mode = entry.header().mode()? & 0o7777;
    meta.modified = entry
        .header()
        .mtime()
        .ok()
        .and_then(|seconds| seconds.try_into().ok())
        .map(|seconds| Timestamp { seconds, nanos: 0 });
    meta.validate()?;
    Ok(meta)
}

fn decode_tar<T: Exchange + 'static>(
    connection: &Connection<T>,
    format: Format,
    size: u64,
    requested: Option<&std::path::Path>,
    budget: &ArchiveBudget,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<(), CommandError> {
    if let Some(requested) = requested.filter(|_| format == Format::Tar) {
        return decode_plain_tar_selection(connection, size, requested, budget, checkpoint);
    }
    // A selected TAR uses a complete first pass, then reads only that member on
    // a fresh source cursor. The hash ties the two traversals to the selected data.
    let mut selection = None;
    let mut archive = tar::Archive::new(BudgetReader {
        reader: tar_reader(connection, format, size)?,
        budget,
        checkpoint,
    });
    for (index, entry) in archive.entries()?.enumerate() {
        let mut entry = entry?;
        let meta = tar_meta(index as u64, &entry)?;
        let chosen = requested.is_none_or(|requested| {
            relative_archive_path(&meta.name).is_ok_and(|path| path == requested)
        });
        budget.entry(&meta.name, if chosen { meta.size } else { 0 })?;
        notify(
            connection,
            Frame::new(Message::Entry {
                entry: meta.clone(),
            }),
        )?;
        if requested.is_none() {
            copy_entry(connection, &meta, &mut entry, budget, checkpoint)?;
        } else if chosen {
            if selection.is_some() || meta.directory {
                return Err(invalid().into());
            }
            let mut hash = blake3::Hasher::new();
            if io::copy(&mut entry, &mut hash)? != meta.size {
                return Err(invalid().into());
            }
            selection = Some((meta, hash.finalize()));
        }
    }
    notify(connection, Frame::new(Message::ManifestEnd))?;
    if requested.is_some() {
        let (meta, hash) = selection.ok_or_else(invalid)?;
        let second = ArchiveBudget::new(budget.limits());
        let check = || budget.checkpoint(checkpoint);
        let mut archive = tar::Archive::new(BudgetReader {
            reader: tar_reader(connection, format, size)?,
            budget: &second,
            checkpoint: &check,
        });
        let mut found = false;
        for (index, entry) in archive.entries()?.enumerate() {
            let mut entry = entry?;
            if index as u64 != meta.id {
                continue;
            }
            if tar_meta(index as u64, &entry)? != meta {
                return Err(invalid().into());
            }
            notify(connection, Frame::new(Message::Begin { id: meta.id }))?;
            let mut hash_writer = HashWriter {
                output: ChunkWriter {
                    connection,
                    id: meta.id,
                    offset: 0,
                },
                hash: blake3::Hasher::new(),
            };
            if budget.copy(&mut entry, &mut hash_writer, checkpoint)? != meta.size
                || hash_writer.hash.finalize() != hash
            {
                return Err(invalid().into());
            }
            notify(connection, Frame::new(Message::End { id: meta.id }))?;
            found = true;
            break;
        }
        if !found {
            return Err(invalid().into());
        }
    }
    Ok(())
}

struct SeekBudget<'a, R> {
    reader: BudgetReader<'a, R>,
}
impl<R: Read> Read for SeekBudget<'_, R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.reader.read(bytes)
    }
}
impl<R: Seek> Seek for SeekBudget<'_, R> {
    fn seek(&mut self, position: io::SeekFrom) -> io::Result<u64> {
        self.reader
            .budget
            .reader_checkpoint(self.reader.checkpoint)?;
        self.reader.reader.seek(position)
    }
}
fn decode_plain_tar_selection<T: Exchange>(
    connection: &Connection<T>,
    size: u64,
    requested: &std::path::Path,
    budget: &ArchiveBudget,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<(), CommandError> {
    let mut selection = None;
    let mut archive = tar::Archive::new(SeekBudget {
        reader: BudgetReader {
            reader: Reader::new(connection.clone(), 0, size),
            budget,
            checkpoint,
        },
    });
    for (index, entry) in archive.entries_with_seek()?.enumerate() {
        let mut entry = entry?;
        let meta = tar_meta(index as u64, &entry)?;
        let chosen = relative_archive_path(&meta.name)? == requested;
        budget.entry(&meta.name, if chosen { meta.size } else { 0 })?;
        notify(
            connection,
            Frame::new(Message::Entry {
                entry: meta.clone(),
            }),
        )?;
        if chosen {
            if selection.is_some() || meta.directory {
                return Err(invalid().into());
            }
            let mut hash = blake3::Hasher::new();
            if io::copy(&mut entry, &mut hash)? != meta.size {
                return Err(invalid().into());
            }
            selection = Some((meta, hash.finalize()));
        }
    }
    let (meta, hash) = selection.ok_or_else(invalid)?;
    notify(connection, Frame::new(Message::ManifestEnd))?;
    let mut archive = tar::Archive::new(SeekBudget {
        reader: BudgetReader {
            reader: Reader::new(connection.clone(), 0, size),
            budget,
            checkpoint,
        },
    });
    for (index, entry) in archive.entries_with_seek()?.enumerate() {
        let mut entry = entry?;
        if index as u64 != meta.id {
            continue;
        }
        if tar_meta(index as u64, &entry)? != meta {
            return Err(invalid().into());
        }
        notify(connection, Frame::new(Message::Begin { id: meta.id }))?;
        let mut writer = HashWriter {
            output: ChunkWriter {
                connection,
                id: meta.id,
                offset: 0,
            },
            hash: blake3::Hasher::new(),
        };
        if budget.copy(&mut entry, &mut writer, checkpoint)? != meta.size
            || writer.hash.finalize() != hash
        {
            return Err(invalid().into());
        }
        notify(connection, Frame::new(Message::End { id: meta.id }))?;
        return Ok(());
    }
    Err(invalid().into())
}
struct HashWriter<W> {
    output: W,
    hash: blake3::Hasher,
}
impl<W: Write> Write for HashWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = self.output.write(bytes)?;
        self.hash.update(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::archive_io_protocol::Wire;
    use crate::utils::archive_io_state::ExtractionState;
    use std::collections::HashMap;
    use std::io::{Cursor, SeekFrom};
    use std::sync::{Arc, Mutex};

    struct Broker {
        worker: Wire,
        parent: Wire,
        sources: HashMap<u64, Vec<u8>>,
        metadata: Vec<Entry>,
        output: Cursor<Vec<u8>>,
        output_limit: u64,
        state: Option<ExtractionState>,
        extracted: HashMap<u64, Vec<u8>>,
        read_bytes: u64,
        finished: bool,
    }
    impl Broker {
        fn new() -> Self {
            Self {
                worker: Wire::new([5; 16]),
                parent: Wire::new([5; 16]),
                sources: HashMap::new(),
                metadata: Vec::new(),
                output: Cursor::new(Vec::new()),
                output_limit: 2 * 1024 * 1024,
                state: None,
                extracted: HashMap::new(),
                read_bytes: 0,
                finished: false,
            }
        }
        fn handle(&mut self, frame: Frame) -> io::Result<Frame> {
            match &frame.message {
                Message::Read {
                    source,
                    offset,
                    length,
                } => {
                    let source_bytes = self.sources.get(source).ok_or_else(invalid)?;
                    let end = offset
                        .checked_add(*length as u64)
                        .filter(|end| *end <= source_bytes.len() as u64)
                        .ok_or_else(invalid)?;
                    self.read_bytes += *length as u64;
                    return Ok(Frame::data(
                        Message::Bytes {
                            source: *source,
                            offset: *offset,
                        },
                        source_bytes[*offset as usize..end as usize].to_vec(),
                    ));
                }
                Message::Write { offset } => {
                    if self.state.is_some()
                        || offset
                            .checked_add(frame.data.len() as u64)
                            .is_none_or(|end| end > self.output_limit)
                    {
                        return Err(invalid());
                    }
                    self.output.seek(SeekFrom::Start(*offset))?;
                    self.output.write_all(&frame.data)?;
                }
                Message::Source { id } => {
                    return Ok(Frame::new(Message::Entry {
                        entry: self.metadata.get(*id as usize).ok_or_else(invalid)?.clone(),
                    }));
                }
                Message::Checkpoint | Message::Flush => {}
                _ => {
                    if let Some(state) = &mut self.state {
                        state.accept(&frame)?;
                        match &frame.message {
                            Message::Begin { id } if !state.entry(*id)?.directory => {
                                self.extracted.insert(*id, Vec::new());
                            }
                            Message::Chunk { id, .. } => {
                                self.extracted
                                    .get_mut(id)
                                    .ok_or_else(invalid)?
                                    .extend_from_slice(&frame.data);
                            }
                            _ => {}
                        }
                    }
                    if frame.message == Message::Finished {
                        self.finished = true;
                    }
                }
            }
            Ok(Frame::new(Message::Ack {
                pause: false,
                turbo: false,
            }))
        }
    }
    impl Exchange for Broker {
        fn exchange(&mut self, frame: Frame) -> io::Result<Frame> {
            let mut request = Vec::new();
            self.worker.write(&mut request, &frame)?;
            let frame = self.parent.read(&mut request.as_slice())?;
            let reply = self.handle(frame)?;
            let mut bytes = Vec::new();
            self.parent.write(&mut bytes, &reply)?;
            self.worker.read(&mut bytes.as_slice())
        }
    }
    fn limits(bytes: u64) -> Limits {
        Limits {
            entries: 100,
            bytes,
            depth: 64,
            seconds: 120,
        }
    }
    fn encoded(format: Format, quality: Quality) -> (Vec<u8>, Vec<u8>) {
        let content: Vec<u8> = (0..(MAX_BLOCK + 13))
            .map(|i| ((i * 29 + i / 257) % 256) as u8)
            .collect();
        let mut broker = Broker::new();
        broker.metadata = vec![
            empty_entry(0, "docs".into(), true, 0),
            empty_entry(
                1,
                "docs/école_日本語.txt".into(),
                false,
                content.len() as u64,
            ),
            empty_entry(2, "empty.txt".into(), false, 0),
        ];
        broker.sources.insert(1, content.clone());
        broker.sources.insert(0, Vec::new());
        broker.sources.insert(2, Vec::new());
        let connection = Arc::new(Mutex::new(broker));
        run(
            connection.clone(),
            Operation::Encode {
                format,
                quality,
                entries: 3,
                output_limit: 2 * 1024 * 1024,
                limits: limits(content.len() as u64),
            },
        )
        .unwrap();
        let broker = connection.lock().unwrap();
        assert!(broker.finished);
        (broker.output.get_ref().clone(), content)
    }
    fn decoded(format: Format, bytes: Vec<u8>, selected: Option<&str>, allowance: u64) -> Broker {
        let size = bytes.len() as u64;
        let mut broker = Broker::new();
        broker.sources.insert(0, bytes);
        broker.state = Some(
            ExtractionState::new(
                limits(allowance),
                selected,
                format != Format::Tar
                    && format != Format::TarGz
                    && format != Format::TarXz
                    && format != Format::TarBz2
                    && format != Format::TarZst
                    || selected.is_some(),
            )
            .unwrap(),
        );
        let connection = Arc::new(Mutex::new(broker));
        run(
            connection.clone(),
            Operation::Decode {
                format,
                source_size: size,
                selected: selected.map(str::to_owned),
                limits: limits(allowance),
            },
        )
        .unwrap();
        Arc::try_unwrap(connection)
            .ok()
            .unwrap()
            .into_inner()
            .unwrap()
    }

    #[test]
    fn four_encoders_roundtrip_through_framed_streams_at_all_existing_qualities() {
        for format in [Format::Zip, Format::SevenZip, Format::Tar, Format::TarZst] {
            for quality in [Quality::Fast, Quality::Normal, Quality::Best] {
                let (bytes, content) = encoded(format, quality);
                let result = decoded(format, bytes.clone(), None, content.len() as u64);
                assert!(result.finished);
                assert!(result.state.as_ref().unwrap().finished());
                assert_eq!(result.extracted.len(), 2);
                assert!(result.extracted.values().any(|bytes| *bytes == content));
                assert!(result.extracted.values().any(Vec::is_empty));
                let result = decoded(
                    format,
                    bytes,
                    Some("docs/école_日本語.txt"),
                    content.len() as u64,
                );
                assert_eq!(result.extracted.len(), 1);
                assert_eq!(result.extracted.values().next().unwrap(), &content);
            }
        }
    }

    #[test]
    fn all_tar_compressions_stream_full_and_selected_members() {
        let (tar, content) = encoded(Format::Tar, Quality::Normal);
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gzip.write_all(&tar).unwrap();
        let mut xz = xz2::write::XzEncoder::new(Vec::new(), 1);
        xz.write_all(&tar).unwrap();
        let mut bz = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::default());
        bz.write_all(&tar).unwrap();
        for (format, bytes) in [
            (Format::TarGz, gzip.finish().unwrap()),
            (Format::TarXz, xz.finish().unwrap()),
            (Format::TarBz2, bz.finish().unwrap()),
        ] {
            let full = decoded(format, bytes.clone(), None, content.len() as u64);
            assert_eq!(full.extracted.len(), 2);
            assert!(full.extracted.values().any(|bytes| *bytes == content));
            let selected = decoded(format, bytes, Some("empty.txt"), content.len() as u64);
            assert_eq!(selected.extracted.len(), 1);
            assert!(selected.extracted.values().all(Vec::is_empty));
        }
    }

    #[test]
    fn iso_uses_the_existing_checked_index_over_a_remote_reader() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("disc.iso");
        super::super::archive::test_iso_image(&path);
        let bytes = std::fs::read(path).unwrap();
        let full = decoded(Format::Iso, bytes.clone(), None, 1024);
        assert!(full.state.as_ref().unwrap().finished());
        assert_eq!(full.extracted.len(), 2);
        let selected = decoded(Format::Iso, bytes, Some("DOCS/README.TXT"), 11);
        assert_eq!(
            selected.extracted.values().collect::<Vec<_>>(),
            vec![&b"ISO content".to_vec()]
        );
    }

    #[test]
    fn plain_tar_selection_skips_neighbor_payloads_on_both_passes() {
        let mut tar = tar::Builder::new(Vec::new());
        for (name, content) in [
            ("large", vec![0; MAX_BLOCK * 2]),
            ("selected", b"safe".to_vec()),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(content.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, name, content.as_slice())
                .unwrap();
        }
        let result = decoded(Format::Tar, tar.into_inner().unwrap(), Some("selected"), 4);
        assert!(result.read_bytes < 8 * 1024);
        assert_eq!(result.extracted.values().next().unwrap(), b"safe");
    }

    fn failing_decode(
        format: Format,
        bytes: Vec<u8>,
        selected: Option<&str>,
        allowance: u64,
    ) -> Broker {
        let mut broker = Broker::new();
        let size = bytes.len() as u64;
        broker.sources.insert(0, bytes.clone());
        broker.state = Some(ExtractionState::new(limits(allowance), selected, true).unwrap());
        let connection = Arc::new(Mutex::new(broker));
        assert!(run(
            connection.clone(),
            Operation::Decode {
                format,
                source_size: size,
                selected: selected.map(str::to_owned),
                limits: limits(allowance)
            }
        )
        .is_err());
        let broker = Arc::try_unwrap(connection)
            .ok()
            .unwrap()
            .into_inner()
            .unwrap();
        assert!(!broker.finished);
        assert!(!broker.state.as_ref().unwrap().finished());
        assert_eq!(broker.sources[&0], bytes);
        broker
    }

    #[test]
    fn targeted_seven_keeps_block_crc_verification_for_unselected_tail() {
        let folder = tempfile::tempdir().unwrap();
        for corrupt in [false, true] {
            let path = folder.path().join("crc.7z");
            super::super::archive_seven_open::block_crc_fixture_with_method(&path, corrupt, 0);
            let bytes = std::fs::read(path).unwrap();
            if corrupt {
                failing_decode(Format::SevenZip, bytes, Some("selected.txt"), 4);
            } else {
                let result = decoded(Format::SevenZip, bytes, Some("selected.txt"), 4);
                assert_eq!(result.extracted.values().next().unwrap(), b"safe");
                assert_eq!(result.extracted.len(), 1);
            }
        }
    }

    #[test]
    fn unsafe_late_zip_entry_and_truncated_codecs_cannot_finish() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for name in ["safe", "../unsafe"] {
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"safe").unwrap();
        }
        let result = failing_decode(Format::Zip, writer.finish().unwrap().into_inner(), None, 8);
        assert!(result.extracted.is_empty());
        for format in [Format::Zip, Format::SevenZip, Format::Tar, Format::TarZst] {
            let (mut bytes, _) = encoded(format, Quality::Fast);
            bytes.truncate(10);
            failing_decode(format, bytes, None, MAX_BLOCK as u64);
        }
    }

    #[test]
    fn source_budget_is_enforced_before_any_content_and_root_markers_remain_valid() {
        let (bytes, content) = encoded(Format::Zip, Quality::Fast);
        assert!(
            failing_decode(Format::Zip, bytes, None, content.len() as u64 - 1)
                .extracted
                .is_empty()
        );
        let mut tar = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Directory);
        header.set_size(0);
        header.set_mode(0o755);
        header.set_cksum();
        tar.append_data(&mut header, ".", io::empty()).unwrap();
        let result = decoded(Format::Tar, tar.into_inner().unwrap(), None, 0);
        assert!(result.finished);
        assert!(result.extracted.is_empty());
    }

    #[test]
    fn portable_times_roundtrip_on_both_sides_of_unix_epoch() {
        for nanos in [0, 1, 100, 999_999_999] {
            let duration = Duration::new(123, nanos);
            for time in [
                std::time::UNIX_EPOCH + duration,
                std::time::UNIX_EPOCH - duration,
            ] {
                assert_eq!(
                    Timestamp::from_system(time).unwrap().system().unwrap(),
                    time
                );
            }
        }
    }
}
