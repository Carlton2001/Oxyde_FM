use super::super::{FileOperation, FileOperationManager, RestoreNormalPriority, FILE_OPS_LOG_TARGET};
use crate::platform::{platform, ThreadPriority};
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

        let _priority_guard = RestoreNormalPriority;
        let mut is_in_background_mode = false;

        loop {
            if cancel.load(Ordering::Relaxed) {
                break;
            }

            let is_turbo = turbo.load(Ordering::Relaxed);

            // Dynamic Priority Adjustment
            if is_turbo && is_in_background_mode {
                platform().set_thread_priority(ThreadPriority::Normal);
                is_in_background_mode = false;
            } else if !is_turbo && !is_in_background_mode {
                platform().set_thread_priority(ThreadPriority::Background);
                is_in_background_mode = true;
            }

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
            operation.lock().unwrap().current_file =
                Some(src.to_string_lossy().into_owned());

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
            let mut copied = 0;
            let output = crate::systems::file_copy::copy_with_parents_observed(
                &resolved_source,
                dest,
                *replace_existing,
                Some(source_file),
                destination_parents,
                &mut |bytes| {
                    processed_bytes
                        .fetch_add(bytes.saturating_sub(copied), Ordering::Relaxed);
                    copied = bytes;
                    loop {
                        if cancel.load(Ordering::Relaxed)
                            || error_share.lock().unwrap().is_some()
                        {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::Interrupted,
                                "Operation cancelled",
                            ));
                        }
                        let fast = turbo.load(Ordering::Relaxed);
                        if fast && is_in_background_mode {
                            platform().set_thread_priority(ThreadPriority::Normal);
                            is_in_background_mode = false;
                        } else if !fast && !is_in_background_mode {
                            platform().set_thread_priority(ThreadPriority::Background);
                            is_in_background_mode = true;
                        }
                        if !pause.load(Ordering::Relaxed) {
                            break;
                        }
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
            }

            processed_files.fetch_add(1, Ordering::Relaxed);
        }

        // Cleanup: Ensure priority is restored before thread dies
        if is_in_background_mode {
            platform().set_thread_priority(ThreadPriority::Normal);
        }

        Ok::<(), String>(())
    }
}
