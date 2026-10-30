//! Parent-owned files and destinations for the headless archive codecs.
use super::archive_owned_tree::OwnedTree;
use super::file_ops::{FileOperation, FileOperationManager};
use crate::{
    platform::platform,
    utils::{
        archive::ArchiveFormat,
        archive_io_protocol::{
            invalid, Control, Format, Frame, Limits, Message, Operation, MAX_BLOCK,
        },
        archive_io_state::ExtractionState,
        archive_limits::{ArchiveBudget, ArchiveLimits},
        archive_security::relative_archive_path,
    },
};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::{atomic::Ordering, Arc, Mutex},
};

pub(crate) fn controls(op: Option<&Arc<Mutex<FileOperation>>>) -> impl Fn() -> io::Result<Control> {
    let flags = op.map(|op| {
        let op = op.lock().unwrap();
        (
            op.cancel_flag.clone(),
            op.pause_flag.clone(),
            op.turbo_flag.clone(),
        )
    });
    move || {
        Ok(flags
            .as_ref()
            .map_or(Control::default(), |(cancel, pause, turbo)| Control {
                cancelled: cancel.load(Ordering::Relaxed),
                paused: pause.load(Ordering::Relaxed),
                turbo: turbo.load(Ordering::Relaxed),
            }))
    }
}
pub(crate) fn format(path: &Path) -> io::Result<Format> {
    Ok(match ArchiveFormat::from_path(path).ok_or_else(invalid)? {
        ArchiveFormat::Zip => Format::Zip,
        ArchiveFormat::SevenZip => Format::SevenZip,
        ArchiveFormat::Tar => Format::Tar,
        ArchiveFormat::TarGz => Format::TarGz,
        ArchiveFormat::TarXz => Format::TarXz,
        ArchiveFormat::TarBz2 => Format::TarBz2,
        ArchiveFormat::TarZst => Format::TarZst,
        ArchiveFormat::Iso => Format::Iso,
        ArchiveFormat::Rar => {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "RAR extraction is not supported",
            ))
        }
    })
}
pub(crate) fn limits(value: ArchiveLimits) -> Limits {
    Limits {
        entries: value.max_entries as u64,
        bytes: value.max_bytes,
        depth: value.max_depth as u64,
        seconds: value.max_duration.as_secs(),
    }
}
fn ack() -> Frame {
    Frame::new(Message::Ack {
        pause: false,
        turbo: false,
    })
}
pub(crate) fn watch_io(
    op: Option<&Arc<Mutex<FileOperation>>>,
    remaining: std::time::Duration,
) -> io::Result<crate::platform::IoCancellationGuard> {
    let (cancel, pause) = op
        .map(|op| {
            let locked = op.lock().unwrap();
            (locked.cancel_flag.clone(), locked.pause_flag.clone())
        })
        .unwrap_or_default();
    platform().watch_broker_io(remaining, cancel, pause)
}

pub(crate) fn decode(
    path: &Path,
    mut source: File,
    target: &mut OwnedTree,
    selected: Option<&str>,
    budget: &ArchiveBudget,
    op: Option<&Arc<Mutex<FileOperation>>>,
) -> io::Result<()> {
    let (cancel, pause) = op
        .map(|op| {
            let locked = op.lock().unwrap();
            (locked.cancel_flag.clone(), locked.pause_flag.clone())
        })
        .unwrap_or_default();
    let _io_cancellation = platform().watch_broker_io(budget.remaining()?, cancel, pause)?;
    let format = format(path)?;
    let control = controls(op);
    let checkpoint = || loop {
        platform().check_broker_pressure()?;
        let current = control()?;
        if current.cancelled {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Operation cancelled",
            ));
        }
        if !current.paused {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    let mut fidelity = if format == Format::Tar {
        Some(crate::utils::tar_fidelity::Verifier::new(
            crate::utils::tar_fidelity::scan(&mut source, budget.limits(), selected, true, &|| {
                budget.checkpoint(&checkpoint)
            })
            .map_err(crate::utils::tar_fidelity::source_error)?,
        ))
    } else {
        None
    };
    let mut limits = limits(budget.limits());
    limits.seconds = budget.remaining()?.as_secs().saturating_add(1);
    let mut state = ExtractionState::new(
        limits,
        selected,
        selected.is_some() || matches!(format, Format::Zip | Format::SevenZip | Format::Iso),
    )?;
    let size = source.metadata()?.len();
    let provenance_parent = super::path_parents::HeldParents::new(path)?;
    let provenance =
        platform().read_archive_output_provenance(provenance_parent.directory(), &source, path)?;
    let mut output: Option<File> = None;
    let mut decoded = 0u64;
    let decoded_limit = limits
        .bytes
        .checked_add(limits.entries.checked_mul(4096).ok_or_else(invalid)?)
        .and_then(|n| {
            n.checked_mul(
                if selected.is_some()
                    && matches!(
                        format,
                        Format::TarGz | Format::TarXz | Format::TarBz2 | Format::TarZst
                    )
                {
                    2
                } else {
                    1
                },
            )
        })
        .ok_or_else(invalid)?;
    let operation = Operation::Decode {
        format,
        source_size: size,
        selected: selected.map(str::to_owned),
        limits,
    };
    let started = std::time::Instant::now();
    let result = platform().run_archive_io_worker(operation, &control, &mut |frame| {
        frame.validate()?;
        match frame.message {
            Message::Read {
                source: id,
                offset,
                length,
            } => {
                if id != 0
                    || length == 0
                    || length as usize > MAX_BLOCK
                    || offset
                        .checked_add(length as u64)
                        .is_none_or(|end| end > size)
                {
                    return Err(invalid());
                }
                source.seek(SeekFrom::Start(offset))?;
                crate::utils::archive_mutation_test::checkpoint(
                    crate::utils::archive_mutation_test::Stage::BrokerBeforeSourceRead,
                    path,
                    target.path(),
                )?;
                let mut data = vec![0; length as usize];
                source.read_exact(&mut data)?;
                return Ok(Frame::data(Message::Bytes { source: id, offset }, data));
            }
            Message::Decoded { bytes } => {
                if bytes == 0 || bytes > MAX_BLOCK as u64 {
                    return Err(invalid());
                }
                decoded = decoded
                    .checked_add(bytes)
                    .filter(|n| *n <= decoded_limit)
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            crate::models::error::ArchiveLimitFailure {
                                kind: crate::models::error::ArchiveLimitKind::Decoded,
                                limit: decoded_limit,
                            },
                        )
                    })?;
                return Ok(ack());
            }
            _ => {}
        }
        state.accept(&frame)?;
        if let Some(verifier) = &mut fidelity {
            match &frame.message {
                Message::Entry { entry } => verifier.entry(entry)?,
                Message::Begin { id } => verifier.begin(*id)?,
                Message::Chunk { id, .. } => verifier.chunk(*id, &frame.data)?,
                Message::End { id } => verifier.end(*id)?,
                Message::Finished => verifier.finish()?,
                _ => {}
            }
        }
        match &frame.message {
            Message::Entry { entry } => {
                if let Some(op) = op {
                    let mut op = op.lock().unwrap();
                    op.total_files += 1;
                    if selected.is_none()
                        || relative_archive_path(&entry.name)?
                            == relative_archive_path(selected.unwrap())?
                    {
                        op.total_bytes += entry.size;
                    }
                }
            }
            Message::Begin { id } => {
                let entry = state.entry(*id)?;
                let path = target.path().join(relative_archive_path(&entry.name)?);
                if let Some(op) = op {
                    op.lock().unwrap().current_file = Some(path.to_string_lossy().into_owned());
                }
                if entry.directory {
                    target.ensure_directory(&path)?;
                } else {
                    output = Some(target.create_file(&path)?);
                }
            }
            Message::Chunk { id, .. } => {
                let entry = state.entry(*id)?;
                let path = target.path().join(relative_archive_path(&entry.name)?);
                let file = output.as_mut().ok_or_else(invalid)?;
                target.before_cache_write(&path, file, &frame.data)?;
                file.write_all(&frame.data)?;
                target.cache_write_observed(&path)?;
                if let Some(op) = op {
                    FileOperationManager::archive_progress(op, frame.data.len() as u64);
                }
            }
            Message::End { id } => {
                let entry = state.entry(*id)?;
                if let Some(file) = output.take() {
                    let mut times = std::fs::FileTimes::new();
                    if let Some(time) = entry.modified {
                        times = times.set_modified(time.system()?);
                    }
                    if let Some(time) = entry.accessed {
                        times = times.set_accessed(time.system()?);
                    }
                    file.set_times(times)?;
                    // Linux file managers leave extraction to the page cache: flushing
                    // each file costs seconds per thousand on Btrfs.
                    #[cfg(target_os = "windows")]
                    file.sync_all()?;
                    target.check_at(
                        target
                            .path()
                            .join(relative_archive_path(&entry.name)?)
                            .parent()
                            .ok_or_else(invalid)?,
                    )?;
                    if let Some(data) = &provenance {
                        target.mark(
                            &target.path().join(relative_archive_path(&entry.name)?),
                            &file,
                            data,
                        )?;
                    }
                    target.complete_cache_write(
                        &target.path().join(relative_archive_path(&entry.name)?),
                        &file,
                    )?;
                }
                if let Some(op) = op {
                    op.lock().unwrap().processed_files += 1;
                }
            }
            Message::Finished => {
                target.check_at(target.path())?;
            }
            Message::ManifestEnd => {}
            _ => return Err(invalid()),
        }
        Ok(ack())
    });
    drop(output);
    let active = result?;
    budget.exclude_wait(started.elapsed().saturating_sub(active));
    budget.check_time()?;
    if !state.finished() {
        return Err(invalid());
    }
    let parent_check = || loop {
        let current = control()?;
        if current.cancelled {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Operation cancelled",
            ));
        }
        if !current.paused {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    target.check_controlled(&mut || budget.checkpoint(&parent_check))?;
    Ok(())
}
