use super::*;
#[path = "parallel.rs"]
mod parallel;
use parallel::*;

fn turbo<T: Exchange>(connection: &Connection<T>) -> io::Result<bool> {
    Ok(connection.lock().map_err(|_| invalid())?.turbo())
}

pub(super) fn encode<T: Exchange>(
    connection: &Connection<T>,
    format: Format,
    quality: Quality,
    entries: u64,
    output_limit: u64,
    password: Option<&ArchiveSecret>,
    budget: &ArchiveBudget,
    checkpoint: &dyn Fn() -> io::Result<()>,
) -> Result<(), CommandError> {
    // AES encrypts each 7z block: the key comes before the compressor.
    let seven_methods = |level: u32, turbo: bool| -> Vec<sevenz_rust2::EncoderConfiguration> {
        let mut methods = Vec::new();
        if let Some(password) = password {
            methods.push(sevenz_rust2::encoder_options::AesEncoderOptions::new(password.seven()).into());
        }
        methods.push(seven_options(level, turbo).into());
        methods
    };
    budget.check_entry_count(entries.try_into().map_err(|_| invalid())?)?;
    budget.checkpoint(checkpoint)?;
    let output = Writer::new(connection.clone(), output_limit);
    let level = match quality {
        Quality::Fast => 1,
        Quality::Normal => 6,
        Quality::Best => 9,
    };
    match format {
        Format::Zip => {
            // raw_copy_file copies in small blocks; coalesce them before IPC.
            let mut writer = zip::ZipWriter::new(io::BufWriter::with_capacity(MAX_BLOCK, output));
            let mut jobs = ZipJobs::new();
            for id in 0..entries {
                budget.checkpoint(checkpoint)?;
                let entry = metadata(connection, id)?;
                budget.entry(&entry.name, entry.size)?;
                notify(connection, Frame::new(Message::Begin { id }))?;
                // Encryption happens as the writer stores: compressed jobs cannot be copied raw.
                let count = if password.is_some() { 1 } else { threads(turbo(connection)?) };
                while jobs.full(entry.size.min(ZIP_BUFFER), count) {
                    jobs.drain_one(&mut writer, checkpoint)?;
                }
                if count > 1 && entry.size <= ZIP_BUFFER {
                    let mut data = Vec::with_capacity(entry.size as usize);
                    let mut reader = io::BufReader::with_capacity(
                        MAX_BLOCK,
                        Reader::new(connection.clone(), id, entry.size),
                    );
                    if !entry.directory
                        && budget.copy(&mut reader, &mut data, checkpoint)? != entry.size
                    {
                        return Err(invalid().into());
                    }
                    notify(connection, Frame::new(Message::End { id }))?;
                    jobs.push(entry, data, level)?;
                    continue;
                }
                jobs.drain(&mut writer, checkpoint)?;
                let options = zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated)
                    .compression_level(Some(level));
                if entry.directory {
                    // A folder holds no data to encrypt.
                    writer
                        .add_directory(format!("{}/", entry.name.trim_end_matches('/')), options)
                        .map_err(io::Error::other)?;
                } else {
                    let options = match password {
                        Some(password) => options.with_aes_encryption(zip::AesMode::Aes256, password.expose()),
                        None => options,
                    };
                    writer
                        .start_file(entry.name, options)
                        .map_err(io::Error::other)?;
                    let mut reader = io::BufReader::with_capacity(
                        MAX_BLOCK,
                        Reader::new(connection.clone(), id, entry.size),
                    );
                    if budget.copy(&mut reader, &mut writer, checkpoint)? != entry.size {
                        return Err(invalid().into());
                    }
                }
                notify(connection, Frame::new(Message::End { id }))?;
            }
            jobs.drain(&mut writer, checkpoint)?;
            writer.finish().map_err(io::Error::other)?.flush()?;
        }
        Format::SevenZip => {
            let mut writer = sevenz_rust2::ArchiveWriter::new(output)
                .map_err(super::super::archive_seven_open::archive_error)?;
            writer
                .set_content_methods(seven_methods(level as u32, turbo(connection)?));
            let mut solid = Vec::new();
            let mut solid_bytes = 0u64;
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
                if entry.directory {
                    // Directory metadata needs no stream. Keep the buffered file
                    // block across directories instead of restarting its dictionary.
                    writer
                        .push_archive_entry(seven, None::<io::Empty>)
                        .map_err(super::super::archive_seven_open::archive_error)?;
                    notify(connection, Frame::new(Message::End { id }))?;
                    continue;
                }
                if solid_bytes + entry.size > SOLID_BUFFER {
                    flush_solid(&mut writer, &mut solid, connection)?;
                    solid_bytes = 0;
                }
                // lzma-rust2 0.22 keeps at most one chunk per thread in flight, so
                // large files no longer need a single-threaded stream to bound memory.
                writer.set_content_methods(seven_methods(level as u32, turbo(connection)?));
                if !entry.directory && entry.size <= SOLID_BUFFER {
                    let mut bytes = Vec::with_capacity(entry.size as usize);
                    let mut reader = io::BufReader::with_capacity(
                        MAX_BLOCK,
                        Reader::new(connection.clone(), id, entry.size),
                    );
                    if budget.copy(&mut reader, &mut bytes, checkpoint)? != entry.size {
                        return Err(invalid().into());
                    }
                    solid_bytes += entry.size;
                    solid.push((seven, bytes));
                    notify(connection, Frame::new(Message::End { id }))?;
                    continue;
                }
                let reader = (!entry.directory).then(|| {
                    io::BufReader::with_capacity(
                        MAX_BLOCK,
                        Reader::new(connection.clone(), id, entry.size),
                    )
                });
                let encoded = writer
                    .push_archive_entry(seven, reader)
                    .map_err(super::super::archive_seven_open::archive_error)?;
                if !entry.directory && encoded.size() != entry.size {
                    return Err(invalid().into());
                }
                notify(connection, Frame::new(Message::End { id }))?;
            }
            flush_solid(&mut writer, &mut solid, connection)?;
            writer.finish()?.flush()?;
        }
        Format::Tar => {
            let mut writer = tar::Builder::new(io::BufWriter::with_capacity(MAX_BLOCK, output));
            encode_tar(connection, entries, &mut writer, budget, checkpoint)?;
            writer.into_inner()?.flush()?;
        }
        format if format.tar_codec().is_some_and(|codec| codec.writable()) => {
            let level = match quality {
                Quality::Fast => crate::utils::archive_codec::Level::Fast,
                Quality::Normal => crate::utils::archive_codec::Level::Normal,
                Quality::Best => crate::utils::archive_codec::Level::Best,
            };
            // Thread count is fixed at stream start; later Turbo changes only priority.
            let encoder = format.tar_codec().ok_or_else(invalid)?.encoder(
                io::BufWriter::with_capacity(MAX_BLOCK, output),
                level,
                threads(turbo(connection)?),
            )?;
            let mut writer = tar::Builder::new(encoder);
            encode_tar(connection, entries, &mut writer, budget, checkpoint)?;
            writer.into_inner()?.finish()?.flush()?;
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
            io::BufReader::with_capacity(
                MAX_BLOCK,
                Reader::new(connection.clone(), id, entry.size),
            ),
        )?;
        notify(connection, Frame::new(Message::End { id }))?;
    }
    writer.finish()
}

fn flush_solid<W: Write + Seek, T: Exchange>(
    writer: &mut sevenz_rust2::ArchiveWriter<W>,
    solid: &mut Vec<(sevenz_rust2::ArchiveEntry, Vec<u8>)>,
    connection: &Connection<T>,
) -> io::Result<()> {
    if solid.is_empty() {
        return Ok(());
    }
    let (entries, readers): (Vec<_>, Vec<_>) = std::mem::take(solid)
        .into_iter()
        .map(|(entry, data)| {
            (
                entry,
                sevenz_rust2::SourceReader::new(Checked {
                    reader: io::Cursor::new(data),
                    connection: connection.clone(),
                    since_check: MAX_BLOCK,
                }),
            )
        })
        .unzip();
    writer
        .push_archive_entries(entries, readers)
        .map_err(io::Error::other)?;
    Ok(())
}
