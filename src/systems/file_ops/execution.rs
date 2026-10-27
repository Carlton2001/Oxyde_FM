use super::{FileOpType, FileOperation, FileOperationManager, OpStatus, RestoreNormalPriority};
use crate::models::{HistoryManager, Transaction, TransactionDetails, TransactionType};
use crate::platform::{follow_turbo_priority, ThreadPriority};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

impl FileOperationManager {
    pub(super) async fn execute_operation(
        history_manager: Arc<HistoryManager>,
        op: Arc<Mutex<FileOperation>>,
    ) {
        let op_clone = op.clone();
        let started_at = std::time::Instant::now();
        {
            let operation = op.lock().unwrap();
            if matches!(operation.op_type, FileOpType::Copy | FileOpType::Move) {
                log::info!(target: "oxyde_lib::copy_timing", "copy_operation_start op={} type={:?} turbo={}",
                    operation.id, operation.op_type, operation.turbo);
            } else if matches!(operation.op_type, FileOpType::Delete | FileOpType::Trash) {
                log::info!(target: "oxyde_lib::delete_timing", "delete_operation_start op={} type={:?} selected={} turbo={}",
                    operation.id, operation.op_type, operation.sources.len(), operation.turbo);
            }
        }

        // Run blocking IO in a separate thread
        let result = tokio::task::spawn_blocking(move || {
            let _priority_guard = RestoreNormalPriority;
            let (op_type, sources, destination, initial_turbo, resolutions) = {
                let mut locked = op_clone.lock().unwrap();
                locked.status = if locked.pause_flag.load(Ordering::Relaxed) {
                    OpStatus::Paused
                } else {
                    OpStatus::Calculating
                };
                locked.started_at = Some(started_at);
                let turbo = locked.turbo_flag.load(Ordering::Relaxed);
                (
                    locked.op_type.clone(),
                    locked.sources.clone(),
                    locked.destination.clone(),
                    turbo,
                    locked.resolutions.clone(),
                )
            };

            // Each operation then follows Turbo with its own priority.
            follow_turbo_priority(initial_turbo, ThreadPriority::Normal);

            op_clone.lock().unwrap().item_tracking_atomic = matches!(op_type, FileOpType::Compress | FileOpType::Extract)
                || sources.iter().any(|path| crate::systems::archive_workspace::virtual_source(path).is_some())
                || destination.as_ref().is_some_and(|path| crate::systems::archive_workspace::virtual_destination(path).is_some());
            match op_type {
                FileOpType::Copy => {
                    Self::perform_copy(&op_clone, sources, destination, false, resolutions)
                }
                FileOpType::Move => {
                    Self::perform_copy(&op_clone, sources, destination, true, resolutions)
                }
                FileOpType::Delete => Self::perform_delete(&op_clone, sources),
                FileOpType::Trash => Self::perform_trash(&op_clone, sources),
                FileOpType::Compress => Self::perform_compress(&op_clone, sources, destination),
                FileOpType::Extract => Self::perform_extract(&op_clone, sources, destination),
            }
        })
        .await;

        // Restore priority or cleanup if needed... (already done in thread pool worker thread)

        // Handle result of the blocking task
        let final_status = match result {
            Ok(Ok(_)) => OpStatus::Completed,
            Ok(Err(e)) => OpStatus::Error(e),
            Err(join_err) => OpStatus::Error(format!("Task execution failed: {}", join_err)),
        };

        let mut locked = op.lock().unwrap();
        locked.duration_ms = locked
            .started_at
            .map(|started| started.elapsed().as_millis().min(u64::MAX as u128) as u64);
        Self::finish_status(&mut locked, final_status.clone());
        // Keep the bounded graph for final cards and persisted history.
        locked.bytes_per_second = 0;
        locked.speed_sample_at = None;
        locked.speed_sample_bytes = 0;

        // Record exact filesystem effects, including partial or cancelled work,
        // so Undo never guesses from the original top-level paths.
        let has_effects = !locked.deleted_items.is_empty()
            || !locked.created_files.is_empty()
            || !locked.created_dirs.is_empty()
            || !locked.overwritten_files.is_empty()
            || !locked.moved_files.is_empty()
            || !locked.moved_dirs.is_empty()
            || !locked.copied_files.is_empty()
            || !locked.created_links.is_empty()
            || !locked.moved_links.is_empty();
        if final_status == OpStatus::Completed || has_effects {
            let virtual_effects = locked
                .sources
                .iter()
                .any(|p| crate::systems::archive_workspace::virtual_source(p).is_some())
                || locked.destination.as_ref().is_some_and(|p| {
                    crate::systems::archive_workspace::virtual_destination(p).is_some()
                });
            let tx_type = if virtual_effects {
                None
            } else {
                match locked.op_type {
                    FileOpType::Copy => Some(TransactionType::Copy),
                    FileOpType::Move => Some(TransactionType::Move),
                    FileOpType::Trash => Some(TransactionType::Delete),
                    FileOpType::Delete => None,
                    FileOpType::Compress | FileOpType::Extract => None,
                }
            };

            let has_undoable_effects = match locked.op_type {
                FileOpType::Copy => {
                    !locked.created_files.is_empty()
                        || !locked.created_dirs.is_empty()
                        || !locked.created_links.is_empty()
                        // Keep an overwrite-only copy as an undo-history
                        // barrier so Undo cannot accidentally target an older action.
                        || !locked.overwritten_files.is_empty()
                }
                FileOpType::Move => {
                    !locked.created_files.is_empty()
                        || !locked.created_dirs.is_empty()
                        || !locked.moved_files.is_empty()
                        || !locked.moved_dirs.is_empty()
                        || !locked.created_links.is_empty()
                        || !locked.moved_links.is_empty()
                }
                _ => true,
            };
            if let Some(t_type) = tx_type.filter(|_| has_undoable_effects) {
                let sources_str: Vec<String> = locked
                    .sources
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();
                #[cfg(target_os = "windows")]
                let sources_str = if matches!(locked.op_type, FileOpType::Trash) {
                    locked.deleted_items.iter().map(|entry| entry.path.clone()).collect()
                } else { sources_str };
                let target_str = locked
                    .destination
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string());

                let details = TransactionDetails {
                    deleted_items: Some(locked.deleted_items.clone()),
                    created_links: Some(locked.created_links.clone()),
                    moved_links: Some(locked.moved_links.clone()),
                    renamed_item_identity: None,
                    moved_directory_identities: Some(
                        locked.moved_directory_identities.iter()
                            .map(|(path, identity)| (path.to_string_lossy().into_owned(), identity.clone()))
                            .collect(),
                    ),
                    new_folder_identity: None,
                    moved_file_identities: Some(
                        locked.moved_file_identities.iter()
                            .map(|(path, identity)| {
                                (path.to_string_lossy().into_owned(), identity.clone())
                            })
                            .collect(),
                    ),
                    copied_directory_identities: Some(
                        locked
                            .copied_directory_identities
                            .iter()
                            .map(|(path, identity)| {
                                (path.to_string_lossy().into_owned(), identity.clone())
                            })
                            .collect(),
                    ),
                    paths: sources_str,
                    target_dir: target_str,
                    old_path: None,
                    new_path: None,
                    created_files: Some(
                        locked
                            .created_files
                            .iter()
                            .map(|p| p.to_string_lossy().into_owned())
                            .collect(),
                    ),
                    created_dirs: Some(
                        locked
                            .created_dirs
                            .iter()
                            .map(|p| p.to_string_lossy().into_owned())
                            .collect(),
                    ),
                    overwritten_files: Some(
                        locked
                            .overwritten_files
                            .iter()
                            .map(|p| p.to_string_lossy().into_owned())
                            .collect(),
                    ),
                    moved_files: Some(
                        locked
                            .moved_files
                            .iter()
                            .map(|(s, d)| {
                                (
                                    s.to_string_lossy().into_owned(),
                                    d.to_string_lossy().into_owned(),
                                )
                            })
                            .collect(),
                    ),
                    trash_metadata_backups: Some(
                        locked
                            .trash_metadata_backups
                            .iter()
                            .map(|path| path.to_string_lossy().into_owned())
                            .collect(),
                    ),
                    moved_dirs: Some(
                        locked
                            .moved_dirs
                            .iter()
                            .map(|(s, d)| {
                                (
                                    s.to_string_lossy().into_owned(),
                                    d.to_string_lossy().into_owned(),
                                )
                            })
                            .collect(),
                    ),
                    copied_files: Some(
                        locked
                            .copied_files
                            .iter()
                            .map(|(s, d)| {
                                (
                                    s.to_string_lossy().into_owned(),
                                    d.to_string_lossy().into_owned(),
                                )
                            })
                            .collect(),
                    ),
                    copied_file_hashes: Some(
                        locked
                            .copied_file_hashes
                            .iter()
                            .map(|(path, hash)| (path.to_string_lossy().into_owned(), hash.clone()))
                            .collect(),
                    ),
                    copied_file_identities: Some(
                        locked
                            .copied_file_identities
                            .iter()
                            .map(|(path, identity)| {
                                (path.to_string_lossy().into_owned(), *identity)
                            })
                            .collect(),
                    ),
                };

                let tx = Transaction::new(t_type, details);
                history_manager.push(tx);
            }
        }
        let operation = &*locked;
        if matches!(operation.op_type, FileOpType::Copy | FileOpType::Move) {
            log::info!(target: "oxyde_lib::copy_timing",
                "copy_operation_end op={} status={:?} wall_s={:.6} processed_bytes={} total_bytes={} processed_files={} total_files={} turbo_final={}",
                operation.id, operation.status, started_at.elapsed().as_secs_f64(), operation.processed_bytes,
                operation.total_bytes, operation.processed_files, operation.total_files, operation.turbo);
        } else if matches!(operation.op_type, FileOpType::Delete | FileOpType::Trash) {
            log::info!(target: "oxyde_lib::delete_timing", "delete_operation_end op={} type={:?} status={:?} wall_s={:.6} processed_files={} total_files={} history_entries={} turbo_final={}",
                operation.id, operation.op_type, operation.status, started_at.elapsed().as_secs_f64(),
                operation.processed_files, operation.total_files, operation.deleted_items.len(), operation.turbo);
        }
    }
}
