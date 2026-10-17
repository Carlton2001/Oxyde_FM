use super::super::{FileOperation, FileOperationManager, RestoreNormalPriority, FILE_OPS_LOG_TARGET};
use crate::platform::{follow_turbo_priority, ThreadPriority};
use crate::systems::archive_move_sources::ArchiveMoveSources;
use crate::systems::destination_tree::DestinationTree;
use crate::systems::source_selection::SelectedFile;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub(super) struct CopyWorkerInputs {
    pub(super) thread_idx: usize,
    pub(super) num_tasks: usize,
    pub(super) is_move: bool,
    pub(super) files: Arc<Vec<(SelectedFile, PathBuf, bool)>>,
    pub(super) processed_bytes: Arc<AtomicU64>,
    pub(super) processed_files: Arc<AtomicUsize>,
    pub(super) cancel: Arc<AtomicBool>,
    pub(super) pause: Arc<AtomicBool>,
    pub(super) turbo: Arc<AtomicBool>,
    pub(super) idx: Arc<AtomicUsize>,
    pub(super) error_share: Arc<Mutex<Option<String>>>,
    pub(super) operation: Arc<Mutex<FileOperation>>,
    pub(super) destination_tree: Arc<DestinationTree>,
    pub(super) deferred_moves: Option<Arc<Mutex<ArchiveMoveSources>>>,
    pub(super) recovery_target: Option<PathBuf>,
}

impl FileOperationManager {
    pub(super) fn run_copy_worker(inputs: CopyWorkerInputs) -> Result<(), String> {
        let CopyWorkerInputs {
            thread_idx,
            num_tasks,
            is_move,
            files,
            processed_bytes,
            processed_files,
            cancel,
            pause,
            turbo,
            idx,
            error_share,
            operation,
            destination_tree,
            deferred_moves,
            recovery_target,
        } = inputs;

        let _timing_context = crate::systems::copy_diagnostics::WorkerScope::enter(
            operation.lock().unwrap().id.clone(), thread_idx,
        );

        let _priority_guard = RestoreNormalPriority;

        loop {
            if cancel.load(Ordering::Relaxed) {
                break;
            }

            let is_turbo = turbo.load(Ordering::Relaxed);
            follow_turbo_priority(is_turbo, ThreadPriority::Normal);

            // Normal mode keeps one worker active; fast mode can use the
            // extra workers selected for a large batch of small files.
            if !is_turbo && thread_idx > 0 {
                if idx.load(Ordering::Relaxed) >= num_tasks {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
                continue;
            }

            let i = idx.fetch_add(1, Ordering::Relaxed);
            if i >= num_tasks {
                break;
            }

            while pause.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if cancel.load(Ordering::Relaxed) {
                    return Ok(());
                }
            }

            let (selected, dest, replace_existing) = &files[i];
            let src = selected.path();
            operation.lock().unwrap().item_started(src);
            operation.lock().unwrap().current_file =
                Some(src.to_string_lossy().into_owned());

            let prepare_started = std::time::Instant::now();
            let destination_parents = destination_tree
                .file_parents(
                    dest,
                    &mut || {
                        Self::archive_checkpoint(&operation).map_err(std::io::Error::other)
                    },
                    &mut |created| {
                        Self::record_created_copy_directory(
                            &operation,
                            &destination_tree,
                            created,
                            is_move,
                        )
                    },
                )
                .map_err(|e| format!("Cannot hold destination {:?}: {e}", dest))?;
            #[cfg(all(target_os = "windows", not(test)))]
            { Self::archive_checkpoint(&operation)?; Self::verify_archive_item(&operation, dest.parent().ok_or("Missing copy parent")?, destination_parents.directory()).map_err(|e| e.to_string())?; }

            let destination_prepare = prepare_started.elapsed();
            let source_started = std::time::Instant::now();
            let source_guard = if is_move || deferred_moves.is_some() {
                Some(
                    crate::systems::copied_source::CopiedSource::open_selected(selected).map_err(
                        |error| {
                            let message =
                                format!("Cannot hold move source {:?}: {error}", src);
                            error_share.lock().unwrap().get_or_insert(message.clone());
                            message
                        },
                    )?,
                )
            } else {
                None
            };
            let copy_source = if source_guard.is_none() {
                Some(selected.open_copy().map_err(|error| {
                    let message = format!("Cannot hold copy source {:?}: {error}", src);
                    error_share.lock().unwrap().get_or_insert(message.clone());
                    message
                })?)
            } else {
                None
            };
            let source_file = source_guard
                .as_ref()
                .map(|source| source.file())
                .or_else(|| copy_source.as_ref().map(|(_, file)| file))
                .expect("selected copy source is held");
            let resolved_source = selected.resolved_path();
            log::info!(target: "oxyde_lib::copy_timing",
                "copy_prepare op={} worker={} source={:?} destination_parent_s={:.6} source_open_s={:.6}",
                operation.lock().unwrap().id, thread_idx, src,
                destination_prepare.as_secs_f64(), source_started.elapsed().as_secs_f64());
            let mut copied = 0;
            let output = crate::systems::file_copy::copy_with_parents_observed(
                &resolved_source,
                dest,
                *replace_existing,
                Some(source_file),
                destination_parents,
                &mut |bytes| {
                    #[cfg(test)]
                    {
                        let id = operation.lock().unwrap().id.clone();
                        super::super::item_outcomes::copy_progress_test_hook(&id, src, bytes);
                    }
                    processed_bytes
                        .fetch_add(bytes.saturating_sub(copied), Ordering::Relaxed);
                    copied = bytes;
                    let mut pause_timer = None;
                    loop {
                        if cancel.load(Ordering::Relaxed)
                            || error_share.lock().unwrap().is_some()
                        {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::Interrupted,
                                "Operation cancelled",
                            ));
                        }
                        follow_turbo_priority(turbo.load(Ordering::Relaxed), ThreadPriority::Normal);
                        if !pause.load(Ordering::Relaxed) {
                            break;
                        }
                        pause_timer.get_or_insert_with(|| crate::systems::copy_diagnostics::Timer::start(
                            crate::systems::copy_diagnostics::Phase::Pause,
                        ));
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    Ok(())
                },
                Some(&mut || {
                    #[cfg(all(target_os = "windows", not(test)))]
                    { return Self::remove_archive_replacement(&operation, dest); }
                    #[cfg(any(not(target_os = "windows"), test))]
                    { Ok(false) }
                }),
            )
            .map_err(|error| {
                let message = match error.kind() {
                    std::io::ErrorKind::PermissionDenied => {
                        "error_access_denied".to_string()
                    }
                    std::io::ErrorKind::StorageFull => "error_disk_full".to_string(),
                    _ => format!("Cannot copy {:?} to {:?}: {error}", src, dest),
                };
                let mut shared = error_share.lock().unwrap();
                if shared.is_none() {
                    *shared = Some(message.clone());
                }
                message
            })?;
            // Capture from the published output's retained handle, before
            // it is dropped or a pathname can be substituted.
            if let Some(target) = &recovery_target { crate::systems::archive_recovery::published(target); }
            #[cfg(all(target_os = "windows", not(test)))]
            Self::capture_archive_creation(&operation, dest, &output.file, false).map_err(|error| error.to_string())?;
            let copied_identity =
                match crate::models::transaction::HistoryFileIdentity::from_file(
                    &output.file,
                ) {
                    Ok(identity) => Some(identity),
                    Err(error) => {
                        log::warn!(
                            target: FILE_OPS_LOG_TARGET,
                            "Copy identity unavailable for {}: {error}",
                            dest.display()
                        );
                        None
                    }
                };
            let moved_identity = if is_move {
                crate::models::transaction::HistoryMoveIdentity::from_file(&output.file)
                    .map_err(|error| {
                        log::warn!(
                            target: FILE_OPS_LOG_TARGET,
                            "Move identity unavailable for {}: {error}",
                            dest.display()
                        )
                    })
                    .ok()
            } else {
                None
            };
            let mut journal = operation.lock().unwrap();
            if *replace_existing {
                journal.overwritten_files.push(dest.clone());
            } else {
                journal.created_files.push(dest.clone());
            }
            journal.copied_files.push((src.to_path_buf(), dest.clone()));
            journal.item_effect(src);
            if !is_move && deferred_moves.is_none() { journal.item_done(src); }
            if let Some(identity) = moved_identity {
                journal.moved_file_identities.push((dest.clone(), identity));
            }
            journal
                .copied_file_hashes
                .push((dest.clone(), output.hash.clone()));
            if let Some(identity) = copied_identity {
                journal
                    .copied_file_identities
                    .push((dest.clone(), identity));
            }
            drop(journal);
            if let Some(source) = source_guard {
                if let Some(moves) = &deferred_moves {
                    moves
                        .lock()
                        .unwrap()
                        .record(src.to_path_buf(), dest.clone(), source, output);
                    processed_files.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
                source
                    .finish(output, &mut || loop {
                        if cancel.load(Ordering::Relaxed)
                            || error_share.lock().unwrap().is_some()
                        {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::Interrupted,
                                "Operation cancelled",
                            ));
                        }
                        if !pause.load(Ordering::Relaxed) {
                            return Ok(());
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    })
                    .map_err(|error| {
                        let message = format!(
                            "Copied destination, but cannot remove {:?}: {error}",
                            src
                        );
                        error_share.lock().unwrap().get_or_insert(message.clone());
                        message
                    })?;
                operation
                    .lock()
                    .unwrap()
                    .moved_files
                    .push((src.to_path_buf(), dest.clone()));
                operation.lock().unwrap().item_done(src);
            }

            processed_files.fetch_add(1, Ordering::Relaxed);
        }

        Ok::<(), String>(())
    }
}
