use super::{FileOperation, FileOperationManager, OpStatus, RestoreNormalPriority};
use crate::platform::{platform, ThreadPriority};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
#[cfg(target_os = "windows")]
mod trash_batch;

impl FileOperationManager {
    #[doc(hidden)]
    pub fn perform_delete(op: &Arc<Mutex<FileOperation>>, sources: Vec<PathBuf>) -> Result<(), String> {
        crate::systems::archive_recovery::scope(|| Self::perform_delete_scoped(op, sources))
    }
    fn perform_delete_scoped(op: &Arc<Mutex<FileOperation>>, sources: Vec<PathBuf>) -> Result<(), String> {
        let (cancel_flag, pause_flag, turbo_flag) = {
            let locked = op.lock().unwrap();
            (
                locked.cancel_flag.clone(),
                locked.pause_flag.clone(),
                locked.turbo_flag.clone(),
            )
        };

        let mut real_sources = Vec::new();
        let mut virtual_sources: HashMap<PathBuf, Vec<String>> = HashMap::new();

        for src in &sources {
            if let Some((archive_path, internal_path)) =
                crate::utils::archive::resolve_virtual_path(src)
            {
                if internal_path.is_empty() {
                    real_sources.push(src.clone());
                } else {
                    virtual_sources
                        .entry(archive_path)
                        .or_default()
                        .push(internal_path);
                }
            } else {
                real_sources.push(src.clone());
            }
        }

        // 1. Handle Virtual Sources (Archive Deletion)
        for (archive_path, internal_paths) in virtual_sources {
            if cancel_flag.load(Ordering::Relaxed) {
                return Ok(());
            }
            Self::remove_archive_entries(op, archive_path, internal_paths)?;
        }

        if real_sources.is_empty() {
            return Ok(());
        }

        // Deletion is counted per top-level item; a directory remains indeterminate
        // while the platform removes its contents.
        let total_items = real_sources.len();
        let current_index = Arc::new(AtomicUsize::new(0));
        let core_count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        let concurrency = core_count.clamp(2, 4);

        let mut handles = Vec::with_capacity(concurrency);
        let real_sources_arc = Arc::new(real_sources);
        let processed_files_atomic = Arc::new(AtomicUsize::new(0));
        let error_captured = Arc::new(Mutex::new(None));

        {
            let mut locked = op.lock().unwrap();
            locked.total_files = total_items;
            locked.status = if locked.pause_flag.load(Ordering::Relaxed) {
                OpStatus::Paused
            } else {
                OpStatus::Running
            };
        }

        for thread_idx in 0..concurrency {
            let sources = real_sources_arc.clone();
            let processed_files = processed_files_atomic.clone();
            let cancel = cancel_flag.clone();
            let pause = pause_flag.clone();
            let turbo = turbo_flag.clone();
            let idx = current_index.clone();
            let error_share = error_captured.clone();
            let operation = op.clone();

            let handle = std::thread::spawn(move || {
                let _priority_guard = RestoreNormalPriority;
                let mut is_in_background_mode = false;

                loop {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }

                    let is_turbo = turbo.load(Ordering::Relaxed);

                    if is_turbo && is_in_background_mode {
                        platform().set_thread_priority(ThreadPriority::Normal);
                        platform().set_thread_priority(ThreadPriority::High);
                        is_in_background_mode = false;
                    } else if !is_turbo && !is_in_background_mode {
                        platform().set_thread_priority(ThreadPriority::Background);
                        is_in_background_mode = true;
                    }

                    if !is_turbo && thread_idx > 0 {
                        if idx.load(Ordering::Relaxed) >= total_items {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        continue;
                    }

                    let i = idx.fetch_add(1, Ordering::Relaxed);
                    if i >= total_items {
                        break;
                    }

                    let src = &sources[i];
                    operation.lock().unwrap().current_file =
                        Some(src.to_string_lossy().into_owned());

                    while pause.load(Ordering::Relaxed) {
                        if cancel.load(Ordering::Relaxed) {
                            return Ok(());
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }

                    operation.lock().unwrap().item_started(src);
                    let res = crate::systems::path_parents::remove(src, &mut || {
                        while pause.load(Ordering::Relaxed) {
                            if cancel.load(Ordering::Relaxed) {
                                return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "Operation cancelled"));
                            }
                            std::thread::sleep(std::time::Duration::from_millis(50));
                        }
                        if cancel.load(Ordering::Relaxed) {
                            return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "Operation cancelled"));
                        }
                        let is_turbo = turbo.load(Ordering::Relaxed);
                        if is_turbo == is_in_background_mode {
                            platform().set_thread_priority(if is_turbo { ThreadPriority::High } else { ThreadPriority::Background });
                            is_in_background_mode = !is_turbo;
                        }
                        Ok(())
                    });

                    let succeeded = res.is_ok();
                    if let Err(e) = res {
                        if e.kind() == std::io::ErrorKind::Interrupted && cancel.load(Ordering::Relaxed) {
                            return Ok(());
                        }
                        if e.kind() != std::io::ErrorKind::NotFound {
                            let key = match e.kind() {
                                std::io::ErrorKind::PermissionDenied => "error_access_denied",
                                std::io::ErrorKind::StorageFull => "error_disk_full",
                                _ => return Err(format!("Failed to delete {:?}: {}", src, e)),
                            }
                            .to_string();
                            let mut share = error_share.lock().unwrap();
                            if share.is_none() {
                                *share = Some(key.clone());
                            }
                            return Err(key);
                        }
                    }

                    if succeeded { operation.lock().unwrap().item_complete(src); }
                    processed_files.fetch_add(1, Ordering::Relaxed);

                }

                if is_in_background_mode {
                    platform().set_thread_priority(ThreadPriority::Normal);
                }

                Ok::<(), String>(())
            });
            handles.push(handle);
        }

        // Loop to emit progress
        while handles.iter().any(|h| !h.is_finished()) {
            std::thread::sleep(std::time::Duration::from_millis(300));
            let current = processed_files_atomic.load(Ordering::Relaxed);
            let mut locked = op.lock().unwrap();
            locked.processed_files = current;
            drop(locked);

            if cancel_flag.load(Ordering::Relaxed) {
                break;
            }
            if error_captured.lock().unwrap().is_some() {
                break;
            }
        }

        for handle in handles {
            match handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    error_captured.lock().unwrap().get_or_insert(error);
                }
                Err(_) => {
                    error_captured
                        .lock()
                        .unwrap()
                        .get_or_insert("Delete worker panicked".to_string());
                }
            }
        }

        // A small deletion can finish before the first periodic progress update.
        op.lock().unwrap().processed_files = processed_files_atomic.load(Ordering::Relaxed);

        // Check if any error was captured
        if let Some(e) = error_captured.lock().unwrap().clone() {
            return Err(e);
        }

        Ok(())
    }

    pub(super) fn perform_trash(op: &Arc<Mutex<FileOperation>>, sources: Vec<PathBuf>) -> Result<(), String> {
        let (cancel_flag, pause_flag, turbo_flag) = {
            let locked = op.lock().unwrap();
            (
                locked.cancel_flag.clone(),
                locked.pause_flag.clone(),
                locked.turbo_flag.clone(),
            )
        };

        let mut real_sources = Vec::new();
        let mut virtual_sources: HashMap<PathBuf, Vec<String>> = HashMap::new();

        for src in &sources {
            if let Some((archive_path, internal_path)) =
                crate::utils::archive::resolve_virtual_path(src)
            {
                if internal_path.is_empty() {
                    real_sources.push(src.clone());
                } else {
                    virtual_sources
                        .entry(archive_path)
                        .or_default()
                        .push(internal_path);
                }
            } else {
                real_sources.push(src.clone());
            }
        }

        // 1. Handle Virtual Sources (Delete from Archive, no Trash support)
        for (archive_path, internal_paths) in virtual_sources {
            if cancel_flag.load(Ordering::Relaxed) {
                return Ok(());
            }
            Self::remove_archive_entries(op, archive_path, internal_paths)?;
        }

        if real_sources.is_empty() {
            return Ok(());
        }

        {
            let mut locked = op.lock().unwrap();
            locked.status = if locked.pause_flag.load(Ordering::Relaxed) {
                OpStatus::Paused
            } else {
                OpStatus::Running
            };
            locked.total_files = real_sources.len();
        }

        #[cfg(target_os = "windows")]
        let selection_started = std::time::Instant::now();
        #[cfg(target_os = "windows")]
        let selected = real_sources
            .iter()
            .map(|path| {
                Self::archive_checkpoint(op)?;
                crate::systems::history_delete::PreparedDelete::new(path).map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, String>>()?;
        #[cfg(target_os = "windows")]
        let selected = crate::systems::history_delete::collapse_roots(selected);
        #[cfg(target_os = "windows")]
        log::info!(target: "oxyde_lib::delete_timing", "trash_selection op={} selected={} wall_s={:.6}",
            op.lock().unwrap().id, selected.len(), selection_started.elapsed().as_secs_f64());
        #[cfg(target_os = "windows")]
        let real_sources = selected.iter().map(|entry| entry.path().to_path_buf()).collect::<Vec<_>>();
        op.lock().unwrap().total_files = real_sources.len();
        #[cfg(target_os = "windows")]
        return Self::perform_selected_trash(op, selected, crate::platform::ShellDeleteControl {
            cancel: cancel_flag, pause: pause_flag, turbo: turbo_flag,
        });
        #[cfg(not(target_os = "windows"))]
        {
            for source in &real_sources {
                if cancel_flag.load(Ordering::Relaxed) {
                    return Ok(());
                }
                while pause_flag.load(Ordering::Relaxed) {
                    if cancel_flag.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                platform().set_thread_priority(if turbo_flag.load(Ordering::Relaxed) {
                    ThreadPriority::Normal
                } else {
                    ThreadPriority::Background
                });
                op.lock().unwrap().current_file = Some(source.to_string_lossy().into_owned());
                op.lock().unwrap().item_started(source);
                platform().shell_delete(std::slice::from_ref(source), true)?;
                op.lock().unwrap().item_complete(source);
                op.lock().unwrap().processed_files += 1;
            }
            Ok(())
        }
    }
}
