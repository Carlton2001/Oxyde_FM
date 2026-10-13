//! Encoder resources are the admitted source plan and one parent-owned private writer.
use super::{
    archive_io::{controls, format, limits},
    compression_sources::Entry as Planned,
    file_ops::{FileOperation, FileOperationManager},
    path_parents::HeldParents,
};
use crate::{
    platform::platform,
    utils::{
        archive_io_protocol::{
            invalid, Entry, Frame, Message, Operation, Quality, Timestamp, MAX_BLOCK,
        },
        archive_limits::ArchiveBudget,
    },
};
use std::{
    collections::HashMap,
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};

struct Source {
    _parents: HeldParents,
    file: File,
    entry: Entry,
    read: u64,
    active: bool,
    hash: Option<blake3::Hasher>,
}

pub(super) fn source_failure(error: io::Error) -> io::Error {
    if let Some(mut value) =
        crate::models::error::ArchiveLimitFailure::from_message(&error.to_string())
    {
        if value.kind == crate::models::error::ArchiveLimitKind::Bytes {
            value.kind = crate::models::error::ArchiveLimitKind::SourceBytes;
            return io::Error::new(error.kind(), value);
        }
    }
    error
}

pub(super) fn run(
    path: &Path,
    entries: &[Planned],
    retained: Option<&HashMap<PathBuf, File>>,
    expected: Option<&HashMap<PathBuf, crate::platform::FileIdentity>>,
    output: &mut File,
    budget: &ArchiveBudget,
    op: &Arc<Mutex<FileOperation>>,
) -> io::Result<()> {
    let (cancel, pause) = {
        let locked = op.lock().unwrap();
        (locked.cancel_flag.clone(), locked.pause_flag.clone())
    };
    let _io_cancellation = platform().watch_broker_io(budget.remaining()?, cancel, pause)?;
    let verify_tar = format(path)? == crate::utils::archive_io_protocol::Format::Tar;
    let mut expected_records = Vec::new();
    let raw = entries.iter().try_fold(0u64, |total, entry| {
        total.checked_add(entry.source.size()).ok_or_else(invalid)
    })?;
    let output_limit = raw
        .checked_add(64 * 1024 * 1024)
        .and_then(|n| n.checked_add((entries.len() as u64).checked_mul(4096)?))
        .ok_or_else(invalid)?;
    let mut limits = limits(budget.limits());
    limits.seconds = budget.remaining()?.as_secs().saturating_add(1);
    let quality = match op.lock().unwrap().compression_quality.as_str() {
        "fast" => Quality::Fast,
        "best" => Quality::Best,
        _ => Quality::Normal,
    };
    let operation = Operation::Encode {
        format: format(path)?,
        quality,
        entries: entries.len() as u64,
        output_limit,
        limits,
    };
    let mut source: Option<Source> = None;
    let mut next = 0u64;
    let mut flushed = false;
    let mut finished = false;
    let started = Instant::now();
    let active =
        platform().run_archive_io_worker(operation, &controls(Some(op)), &mut |frame| {
            frame.validate()?;
            if finished {
                return Err(invalid());
            }
            match frame.message {
                Message::Source { id } => {
                    if id != next || source.is_some() {
                        return Err(invalid());
                    }
                    let planned = entries.get(id as usize).ok_or_else(invalid)?;
                    let (parents, file) = planned.open(retained)?;
                    if let Some(expected) = expected {
                        if expected.get(&planned.source.resolved_path())
                            != Some(&platform().file_identity(&file)?)
                        {
                            return Err(crate::models::error::SourceSelectionFailure::changed(
                                planned.source.path(),
                            ));
                        }
                    }
                    let metadata = file.metadata()?;
                    if metadata.is_dir() != planned.source.is_dir()
                        || (!metadata.is_dir() && metadata.len() != planned.source.size())
                    {
                        return Err(crate::models::error::SourceSelectionFailure::changed(
                            planned.source.path(),
                        ));
                    }
                    let mut header = tar::Header::new_gnu();
                    header.set_metadata(&metadata);
                    let entry = Entry {
                        id,
                        name: planned.name.clone(),
                        directory: metadata.is_dir(),
                        size: planned.source.size(),
                        mode: header.mode()? & 0o7777,
                        modified: metadata
                            .modified()
                            .ok()
                            .map(Timestamp::from_system)
                            .transpose()?,
                        created: metadata
                            .created()
                            .ok()
                            .map(Timestamp::from_system)
                            .transpose()?,
                        accessed: metadata
                            .accessed()
                            .ok()
                            .map(Timestamp::from_system)
                            .transpose()?,
                    };
                    entry.validate()?;
                    op.lock().unwrap().current_file =
                        Some(planned.source.path().to_string_lossy().into_owned());
                    source = Some(Source {
                        _parents: parents,
                        file,
                        entry: entry.clone(),
                        read: 0,
                        active: false,
                        hash: verify_tar.then(blake3::Hasher::new),
                    });
                    return Ok(Frame::new(Message::Entry { entry }));
                }
                Message::Begin { id } => {
                    let source = source.as_mut().ok_or_else(invalid)?;
                    if source.entry.id != id || source.active {
                        return Err(invalid());
                    }
                    source.active = true;
                }
                Message::Read {
                    source: id,
                    offset,
                    length,
                } => {
                    let source = source.as_mut().ok_or_else(invalid)?;
                    if !source.active
                        || source.entry.directory
                        || id != source.entry.id
                        || offset != source.read
                        || length == 0
                        || length as usize > MAX_BLOCK
                        || offset
                            .checked_add(length as u64)
                            .is_none_or(|end| end > source.entry.size)
                    {
                        return Err(invalid());
                    }
                    source.file.seek(SeekFrom::Start(offset))?;
                    crate::utils::archive_mutation_test::checkpoint(
                        crate::utils::archive_mutation_test::Stage::BrokerBeforeSourceRead,
                        source._parents.resolved_directory(),
                        path,
                    )?;
                    let mut data = vec![0; length as usize];
                    source.file.read_exact(&mut data)?;
                    if let Some(hash) = &mut source.hash {
                        hash.update(&data);
                    }
                    source.read += length as u64;
                    FileOperationManager::archive_progress(op, length as u64);
                    return Ok(Frame::data(Message::Bytes { source: id, offset }, data));
                }
                Message::End { id } => {
                    let source_ref = source.as_ref().ok_or_else(invalid)?;
                    if !source_ref.active
                        || source_ref.entry.id != id
                        || source_ref.read != source_ref.entry.size
                    {
                        return Err(invalid());
                    }
                    if verify_tar {
                        let entry = &source_ref.entry;
                        expected_records.push(crate::utils::tar_fidelity::Record {
                            name: crate::utils::archive_security::relative_archive_path(
                                &entry.name,
                            )?,
                            directory: entry.directory,
                            size: entry.size,
                            mode: entry.mode,
                            modified: Some(
                                entry
                                    .modified
                                    .filter(|time| time.seconds >= 0)
                                    .map_or(0, |time| time.seconds),
                            ),
                            hash: source_ref.hash.as_ref().map(|hash| hash.finalize()),
                        });
                    }
                    source.take();
                    next = next.checked_add(1).ok_or_else(invalid)?;
                    op.lock().unwrap().processed_files += 1;
                }
                Message::Write { offset } => {
                    if frame.data.is_empty() {
                        return Err(invalid());
                    }
                    if offset
                        .checked_add(frame.data.len() as u64)
                        .is_none_or(|end| end > output_limit)
                    {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            crate::models::error::ArchiveLimitFailure {
                                kind: crate::models::error::ArchiveLimitKind::EncodedBytes,
                                limit: output_limit,
                            },
                        ));
                    }
                    output.seek(SeekFrom::Start(offset))?;
                    output.write_all(&frame.data)?;
                    flushed = false;
                }
                Message::Flush => {
                    output.flush()?;
                    flushed = true;
                }
                Message::Finished => {
                    if source.is_some() || next != entries.len() as u64 || !flushed {
                        return Err(invalid());
                    }
                    finished = true;
                }
                _ => return Err(invalid()),
            }
            Ok(Frame::new(Message::Ack {
                pause: false,
                turbo: false,
            }))
        })?;
    budget.exclude_wait(started.elapsed().saturating_sub(active));
    budget.check_time()?;
    if !finished || output.metadata()?.len() > output_limit {
        return Err(invalid());
    }
    if verify_tar {
        let actual = crate::utils::tar_fidelity::scan_canonical(output, budget.limits(), &|| {
            platform().check_broker_pressure()?;
            budget.checkpoint(&|| {
                FileOperationManager::archive_checkpoint(op).map_err(io::Error::other)
            })
        })?;
        if actual != expected_records {
            return Err(crate::utils::tar_fidelity::failure());
        }
    }
    Ok(())
}
