mod worker;

use super::{FileOperation, FileOperationManager, OpStatus, FILE_OPS_LOG_TARGET};
use crate::models::ConflictAction;
use crate::platform::{platform, ThreadPriority};
#[cfg(all(target_os = "windows", not(test)))]
use crate::utils::archive_limits::{ArchiveBudget, ArchiveLimits};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

impl FileOperationManager {
    fn record_created_copy_directory(
        operation: &Arc<Mutex<FileOperation>>,
        tree: &crate::systems::destination_tree::DestinationTree,
        path: &std::path::Path,
        _is_move: bool,
    ) {
        { let mappings = operation.lock().unwrap().archive_path_mappings.clone(); if !mappings.is_empty() && !mappings.iter().any(|(root, _)| path.starts_with(root)) { crate::systems::archive_recovery::published(tree.path()); } }
        #[cfg(all(target_os = "windows", not(test)))]
        if let Err(error) = tree.retained_directory(path).and_then(|file| Self::capture_archive_creation(operation, path, &file, true)) {
            operation.lock().unwrap().archive_cleanup_error = Some(error.to_string());
        }
        let identity = tree.directory_identity(path)
            .map_err(|error| {
                log::warn!(
                    target: FILE_OPS_LOG_TARGET,
                    "Copy/Move directory identity unavailable for {}: {error}",
                    path.display()
                )
            })
            .ok();
        let mut journal = operation.lock().unwrap();
        journal.created_dirs.push(path.to_path_buf());
        if let Some(identity) = identity {
            journal
                .copied_directory_identities
                .push((path.to_path_buf(), identity));
        }
    }

    #[cfg(all(target_os = "windows", not(test)))]
    fn capture_archive_creation(operation: &Arc<Mutex<FileOperation>>, path: &Path, file: &std::fs::File, directory: bool) -> std::io::Result<()> {
        let owners = operation.lock().unwrap().archive_owned_roots.clone();
        for (root, owner) in owners {
            if path.starts_with(root) {
                let owner = owner.upgrade().ok_or_else(|| std::io::Error::other("Archive workspace ownership expired"))?;
                let mut tree = owner.lock().map_err(|_| std::io::Error::other("Archive workspace ownership unavailable"))?;
                if directory { tree.adopt_created_directory(path, file)?; } else { tree.adopt_created_file(path, file)?; }
            }
        }
        Ok(())
    }

    #[cfg(all(target_os = "windows", not(test)))]
    fn verify_archive_item(operation: &Arc<Mutex<FileOperation>>, path: &Path, file: &std::fs::File) -> std::io::Result<()> {
        let owners = operation.lock().unwrap().archive_owned_roots.clone();
        for (root, owner) in owners {
            if path.starts_with(root) { owner.upgrade().ok_or_else(|| std::io::Error::other("Archive ownership expired"))?.lock().unwrap().verify_owned(path, file)?; }
        }
        Ok(())
    }

    #[cfg(all(target_os = "windows", not(test)))]
    fn remove_archive_replacement(operation: &Arc<Mutex<FileOperation>>, path: &Path) -> std::io::Result<bool> {
        Self::archive_checkpoint(operation).map_err(std::io::Error::other)?;
        let owners = operation.lock().unwrap().archive_owned_roots.clone();
        for (root, owner) in owners {
            if path.starts_with(root) { owner.upgrade().ok_or_else(|| std::io::Error::other("Archive ownership expired"))?.lock().unwrap().remove_selected(path, &mut || Ok(()))?; return Ok(true); }
        }
        Ok(false)
    }

    #[doc(hidden)]
    pub fn perform_copy(
        op: &Arc<Mutex<FileOperation>>,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
        is_move: bool,
        resolutions: Option<HashMap<String, ConflictAction>>,
    ) -> Result<(), String> {
        crate::systems::archive_recovery::scope(|| Self::perform_copy_scoped(op, sources, destination, is_move, resolutions))
    }
    fn perform_copy_scoped(op: &Arc<Mutex<FileOperation>>, sources: Vec<PathBuf>, destination: Option<PathBuf>, is_move: bool, resolutions: Option<HashMap<String, ConflictAction>>) -> Result<(), String> {
        let trash_entries = platform().trash_transfer_entries(&sources)?;
        let virtual_effects = destination.as_ref().is_some_and(|path| {
            crate::systems::archive_workspace::virtual_destination(path).is_some()
        }) || sources
            .iter()
            .any(|path| crate::systems::archive_workspace::virtual_source(path).is_some());
        let result = Self::perform_copy_real(
            op,
            sources,
            destination,
            is_move,
            resolutions,
            &trash_entries,
            None,
        );
        let mut metadata_errors = Vec::new();
        if is_move {
            // Also finalize fully moved entries after a later cancellation/error.
            for entry in &trash_entries {
                match platform().detach_trash_metadata(entry) {
                    Ok(Some(metadata)) if virtual_effects => {
                        // Archive transfers have no filesystem Undo, but cleanup
                        // still requires the actual detached object's proof/hash.
                        if let Err(error) = crate::systems::file_copy::remove_verified_output_identity(
                            &metadata.backup, &metadata.hash, metadata.identity.snapshot(),
                        ) {
                            metadata_errors.push(error.to_string());
                        }
                    }
                    Ok(Some(metadata)) => {
                        let mut locked = op.lock().unwrap();
                        locked.trash_metadata_backups.push(metadata.backup.clone());
                        locked.moved_file_identities.push((metadata.backup.clone(), metadata.identity));
                        locked.copied_file_hashes.push((metadata.backup.clone(), metadata.hash));
                        locked.moved_files.push((metadata.source, metadata.backup));
                    }
                    Ok(None) => {}
                    Err(error) => metadata_errors.push(error),
                }
            }
        }
        if metadata_errors.is_empty() {
            result
        } else {
            Err(format!(
                "{}; trash metadata: {}",
                result.err().unwrap_or_default(),
                metadata_errors.join("; ")
            ))
        }
    }
    pub(super) fn perform_copy_real(
        op: &Arc<Mutex<FileOperation>>,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
        is_move: bool,
        resolutions: Option<HashMap<String, ConflictAction>>,
        trash_entries: &[crate::platform::TrashTransferEntry],
        deferred_moves: Option<Arc<Mutex<crate::systems::archive_move_sources::ArchiveMoveSources>>>,
    ) -> Result<(), String> {
        let source_name = |source: &Path| -> Result<std::ffi::OsString, String> {
            trash_entries
                .iter()
                .find(|entry| entry.path == source)
                .map(|entry| entry.original_path.as_path())
                .unwrap_or(source)
                .file_name()
                .map(|name| name.to_os_string())
                .ok_or("Invalid source name".into())
        };
        if sources
            .iter()
            .any(|p| crate::systems::archive_workspace::virtual_source(p).is_some())
            || destination.as_ref().is_some_and(|p| {
                crate::systems::archive_workspace::virtual_destination(p).is_some()
            })
        {
            return Self::perform_archive_transfer(op, sources, destination, is_move, resolutions);
        }
        if op.lock().unwrap().cancel_flag.load(Ordering::Relaxed) {
            return Ok(());
        }
        let target_dir = destination.ok_or("No destination provided for copy/move".to_string())?;

        let destination_tree = Arc::new(
            crate::systems::destination_tree::DestinationTree::new(&target_dir)
                .map_err(|e| format!("Cannot hold destination {:?}: {e}", target_dir))?,
        );
        let target_dir = destination_tree.path().to_path_buf();
        #[cfg(all(target_os = "windows", not(test)))]
        let archive_budget = (!op.lock().unwrap().archive_owned_roots.is_empty()).then(|| ArchiveBudget::new(ArchiveLimits::WORKSPACE));
        #[cfg(all(target_os = "windows", not(test)))]
        if let Some(budget) = &archive_budget { budget.check_entry_count(sources.len()).map_err(|e| e.to_string())?; }
        #[cfg(all(target_os = "windows", not(test)))]
        let mut archive_entries = 0usize;
        #[cfg(all(target_os = "windows", not(test)))]
        let mut archive_name_bytes = 0usize;
        let mut selected_sources = Vec::with_capacity(sources.len());
        let mut canonical_sources = Vec::with_capacity(sources.len());
        for source in &sources {
            Self::archive_checkpoint(op)?;
            let selected = crate::systems::source_selection::SelectedEntry::select(source)
                .map_err(|error| format!("Cannot select source {:?}: {error}", source))?;
            let canonical = selected.resolved_path();
            if canonical == target_dir || target_dir.starts_with(&canonical) {
                return Err(format!(
                    "Destination {:?} is inside source {:?}",
                    target_dir, canonical
                ));
            }
            if canonical_sources.iter().any(|existing: &PathBuf| {
                canonical.starts_with(existing) || existing.starts_with(&canonical)
            }) {
                return Err(format!(
                    "Overlapping source paths are not supported: {:?}",
                    source
                ));
            }
            canonical_sources.push(canonical);
            selected_sources.push(selected);
        }

        if !trash_entries.is_empty() {
            let mut names = std::collections::HashSet::new();
            for source in &sources {
                let name = source_name(source)?.to_string_lossy().to_lowercase();
                if !names.insert(name) {
                    return Err("Selected trash items have the same original name; transfer them separately".into());
                }
            }
        }
        let mut total_bytes = 0;
        let mut total_files = 0;

        // Build the conflict-aware plan before changing any source.
        if selected_sources.is_empty() {
            return Ok(());
        }

        // 2. Calculate size and build file list with resolution checks
        let mut files_to_process = Vec::new();
        let mut directories_to_create = Vec::new();
        let mut source_directories = crate::systems::source_directories::SourceDirectories::default();
        let mut reserved_copy_names = std::collections::HashSet::new();
        let mut fresh_copy_directories = Vec::new();
        let copy_name_suffix = op.lock().unwrap().copy_name_suffix.clone();

        for (selected, canonical_src) in selected_sources.iter().zip(&canonical_sources) {
            Self::archive_checkpoint(op)?;
            let src = selected.path();
            let mut dest_root = target_dir.join(source_name(src)?);
            let self_transfer = crate::systems::transfer_naming::same_entry(src, &dest_root)
                || dest_root == src
                || (dest_root.exists()
                    && std::fs::canonicalize(&dest_root)
                        .is_ok_and(|destination| destination == *canonical_src));
            if self_transfer {
                if is_move { continue; }
                dest_root = crate::systems::transfer_naming::copy_name(src, &target_dir, selected.is_dir(),
                    &copy_name_suffix, &mut reserved_copy_names).map_err(|error| error.to_string())?;
                if selected.is_dir() { fresh_copy_directories.push(dest_root.clone()); }
            }
            let mut walker = crate::systems::source_selection::SourceWalk::new(selected.clone());
            loop {
                #[cfg(all(target_os = "windows", not(test)))]
                if let Some(budget) = &archive_budget { walker.bound_pending(budget.limits().max_entries.saturating_sub(archive_entries), budget.limits().max_entries); }
                let Some(entry) = walker
                .next(&mut || {
                    #[cfg(all(target_os = "windows", not(test)))]
                    if let Some(budget) = &archive_budget { return budget.checkpoint_mut(&mut || Self::archive_checkpoint(op).map_err(std::io::Error::other)); }
                    Self::archive_checkpoint(op).map_err(std::io::Error::other)
                })
                .map_err(|error| format!("Cannot enumerate source {:?}: {error}", src))?
                else { break; };
                #[cfg(all(target_os = "windows", not(test)))]
                { let (_parents, file) = entry.open_archive().map_err(|e| e.to_string())?; Self::verify_archive_item(op, &entry.resolved_path(), &file).map_err(|e| e.to_string())?; }
                let relative = entry
                    .path()
                    .strip_prefix(src)
                    .map_err(|error| error.to_string())?;
                let dest_path = if relative.as_os_str().is_empty() {
                    dest_root.clone()
                } else {
                    dest_root.join(relative)
                };
                #[cfg(all(target_os = "windows", not(test)))]
                if let Some(budget) = &archive_budget {
                    let name = dest_path.strip_prefix(&target_dir).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
                    budget.entry(&name, entry.size()).map_err(|e| e.to_string())?;
                    archive_name_bytes = archive_name_bytes.checked_add(name.len()).ok_or("Archive transfer names exceeded")?;
                    if name.len() > crate::utils::archive_io_protocol::MAX_NAME || archive_name_bytes > 64 * 1024 * 1024 { return Err("Archive transfer names exceeded".into()); }
                    archive_entries += 1;
                }
                if entry.is_dir() {
                    if dest_path.exists() && !dest_path.is_dir() {
                        let replace = resolutions
                            .as_ref()
                            .and_then(|map| Self::get_resolution(entry.path(), map))
                            == Some(ConflictAction::Replace);
                        if !replace {
                            walker.skip_current_dir();
                            continue;
                        }
                    }
                    if is_move || deferred_moves.is_some() {
                        entry
                            .capture_directory(&dest_path, &mut source_directories)
                            .map_err(|error| {
                                format!(
                                    "Cannot prepare source directory {:?} for move: {error}",
                                    entry.path()
                                )
                            })?;
                    }
                    directories_to_create.push(dest_path);
                    continue;
                }
                let replace_existing = !self_transfer && dest_path.exists();
                if replace_existing
                    && resolutions
                        .as_ref()
                        .and_then(|map| Self::get_resolution(entry.path(), map))
                        != Some(ConflictAction::Replace)
                {
                    continue;
                }
                let file = entry.into_file().map_err(|error| error.to_string())?;
                total_bytes += file.size;
                total_files += 1;
                files_to_process.push((file, dest_path, replace_existing));
            }
        }

        {
            let mut locked = op.lock().unwrap();
            locked.total_bytes = total_bytes;
            locked.total_files = total_files;
            locked.status = if locked.pause_flag.load(Ordering::Relaxed) {
                OpStatus::Paused
            } else {
                OpStatus::Running
            };
        }

        // 3. Perform Copy (for remaining or non-move ops)
        let processed_bytes_atomic = Arc::new(AtomicU64::new(0));
        let processed_files_atomic = Arc::new(AtomicUsize::new(0));

        let mut last_processed_bytes = 0;
        let mut last_emit = std::time::Instant::now();
        let mut speed_samples: std::collections::VecDeque<u64> =
            std::collections::VecDeque::with_capacity(4);

        // Cache flags to avoid locking in tight loops
        let (cancel_flag, pause_flag, turbo_flag) = {
            let locked = op.lock().unwrap();
            (
                locked.cancel_flag.clone(),
                locked.pause_flag.clone(),
                locked.turbo_flag.clone(),
            )
        };

        let last_turbo_state = turbo_flag.load(Ordering::Relaxed);
        // Ensure starting priority matches initial mode
        if last_turbo_state {
            platform().set_thread_priority(ThreadPriority::Normal);
        } else {
            platform().set_thread_priority(ThreadPriority::Background);
        }

        for directory in &directories_to_create {
            if cancel_flag.load(Ordering::Relaxed) {
                break;
            }
            if fresh_copy_directories.contains(directory) {
                destination_tree.reserve_copy_directory(directory, &mut |created| {
                    Self::record_created_copy_directory(op, &destination_tree, created, is_move)
                }).map_err(|e| format!("Cannot create new copy directory {:?}: {e}", directory))?;
                continue;
            }
            destination_tree
                .ensure_directory(
                    directory,
                    &mut || Self::archive_checkpoint(op).map_err(std::io::Error::other),
                    &mut |created| {
                        Self::record_created_copy_directory(op, &destination_tree, created, is_move)
                    },
                )
                .map_err(|e| format!("Cannot create directory {:?}: {e}", directory))?;
        }
        // A rename preserves the file itself and does not read its contents.
        // Let the filesystem determine volume boundaries (including mount points),
        // rather than trusting the UI's disk estimate. Only cross-volume moves copy.
        if is_move {
            let mut remaining = Vec::new();
            for (source, destination, replace_existing) in files_to_process {
                Self::archive_checkpoint(op)?;
                let destination_parents = destination_tree
                    .file_parents(
                        &destination,
                        &mut || Self::archive_checkpoint(op).map_err(std::io::Error::other),
                        &mut |created| {
                            Self::record_created_copy_directory(
                                op,
                                &destination_tree,
                                created,
                                is_move,
                            )
                        },
                    )
                    .map_err(|e| format!("Cannot hold destination {:?}: {e}", destination))?;
                match crate::systems::path_parents::move_selected_file_with_parents(
                    &source,
                    &destination,
                    replace_existing,
                    destination_parents,
                    &mut || Self::archive_checkpoint(op).map_err(std::io::Error::other),
                ) {
                    Ok((size, identity)) => {
                        let mut locked = op.lock().unwrap();
                        locked.current_file = Some(source.path().to_string_lossy().into_owned());
                        if replace_existing {
                            locked.overwritten_files.push(destination.clone());
                        }
                        if let Some(identity) = identity {
                            locked
                                .moved_file_identities
                                .push((destination.clone(), identity));
                        }
                        locked
                            .moved_files
                            .push((source.path().to_path_buf(), destination));
                        processed_bytes_atomic.fetch_add(size, Ordering::Relaxed);
                        processed_files_atomic.fetch_add(1, Ordering::Relaxed);
                        locked.processed_bytes = processed_bytes_atomic.load(Ordering::Relaxed);
                        locked.processed_files = processed_files_atomic.load(Ordering::Relaxed);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::CrossesDevices => {
                        remaining.push((source, destination, replace_existing));
                    }
                    Err(error) => {
                        return Err(format!(
                            "Cannot move {:?} to {:?}: {error}",
                            source.path(),
                            destination
                        ));
                    }
                }
            }
            files_to_process = remaining;
        }
        let num_tasks = files_to_process.len();
        let files_to_process_arc = Arc::new(files_to_process);
        let current_index = Arc::new(AtomicUsize::new(0));

        // Parallel streams help with many small files, but contend for the same disk
        // when copying a handful of files or large files.
        let core_count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        let concurrency = if num_tasks == 0 {
            0
        } else {
            Self::copy_worker_count(num_tasks, total_bytes, core_count)
        };

        let mut handles = Vec::with_capacity(concurrency);
        let recovery_target = {
            let mappings = op.lock().unwrap().archive_path_mappings.clone();
            (!mappings.is_empty() && !mappings.iter().any(|(root, _)| target_dir.starts_with(root))).then(|| target_dir.clone())
        };

        let error_captured = Arc::new(Mutex::new(None));

        for thread_idx in 0..concurrency {
            let files = files_to_process_arc.clone();
            let processed_bytes = processed_bytes_atomic.clone();
            let processed_files = processed_files_atomic.clone();
            let cancel = cancel_flag.clone();
            let pause = pause_flag.clone();
            let turbo = turbo_flag.clone();
            let idx = current_index.clone();
            let error_share = error_captured.clone();
            let operation = op.clone();
            let destination_tree = destination_tree.clone();
            let deferred_moves = deferred_moves.clone();
            let recovery_context = crate::systems::archive_recovery::context();
            let recovery_target = recovery_target.clone();

            let handle = std::thread::spawn(move || crate::systems::archive_recovery::with_context(recovery_context, || {
                Self::run_copy_worker(worker::CopyWorkerInputs {
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
                })
            }));
            handles.push(handle);
        }

        // Loop to emit progress while workers are running
        while handles.iter().any(|h| !h.is_finished()) {
            std::thread::sleep(std::time::Duration::from_millis(500));
            Self::emit_progress(
                op,
                &processed_bytes_atomic,
                &processed_files_atomic,
                &mut last_processed_bytes,
                &mut last_emit,
                &mut speed_samples,
            );

            if cancel_flag.load(Ordering::Relaxed) {
                break;
            }
            if error_captured.lock().unwrap().is_some() {
                break;
            }
        }

        // Wait for all threads
        let mut worker_error = None;
        for handle in handles {
            match handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    worker_error.get_or_insert(error);
                }
                Err(_) => {
                    worker_error.get_or_insert("Copy worker panicked".to_string());
                }
            }
        }

        // Check if any error was captured
        if let Some(e) = error_captured.lock().unwrap().clone() {
            return Err(e);
        }
        if let Some(error) = worker_error {
            return Err(error);
        }
        if cancel_flag.load(Ordering::Relaxed) {
            return Err("Operation cancelled".to_string());
        }

        // Release planned source directory pins before bottom-up cleanup. Deferred
        // archive sources retain only the ancestors of their still-held files.
        drop(files_to_process_arc);
        drop(selected_sources);

        // Final update to ensure 100% progress is shown before completion
        {
            let mut locked = op.lock().unwrap();
            locked.processed_bytes = locked.total_bytes;
            locked.processed_files = locked.total_files;
            locked.bytes_per_second = 0;
        }

        if let Some(deferred_moves) = deferred_moves {
            deferred_moves
                .lock()
                .unwrap()
                .retain_directories(source_directories, destination_tree);
        } else if is_move {
            source_directories
                .remove_empty(
                    &mut || {
                        if cancel_flag.load(Ordering::Relaxed) {
                            Err(std::io::Error::new(
                                std::io::ErrorKind::Interrupted,
                                "Operation cancelled",
                            ))
                        } else {
                            Ok(())
                        }
                    },
                    &mut |directory, destination| {
                        let mut journal = op.lock().unwrap();
                        journal
                            .moved_dirs
                            .push((directory.to_path_buf(), destination.to_path_buf()));
                        journal.moved_directory_identities.push((
                            directory.to_path_buf(),
                            crate::models::transaction::HistoryDirectoryIdentity::vacant(),
                        ));
                    },
                )
                .map_err(|error| error.to_string())?;
        }

        Ok(())
    }

    pub(super) fn emit_progress(
        op: &Arc<Mutex<FileOperation>>,
        processed_bytes_atomic: &Arc<AtomicU64>,
        processed_files_atomic: &Arc<AtomicUsize>,
        last_processed_bytes: &mut u64,
        last_emit: &mut std::time::Instant,
        speed_samples: &mut std::collections::VecDeque<u64>,
    ) {
        let elapsed = last_emit.elapsed();
        if elapsed.as_millis() < 500 {
            return;
        }

        let current_bytes = processed_bytes_atomic.load(Ordering::Relaxed);
        let current_files = processed_files_atomic.load(Ordering::Relaxed);

        let mut locked = op.lock().unwrap();

        // Calculate speed (average over last 2 seconds / 4 samples)
        let bytes_diff = current_bytes.saturating_sub(*last_processed_bytes);
        let secs = elapsed.as_secs_f64();
        if secs > 0.0 {
            let current_speed = (bytes_diff as f64 / secs) as u64;
            locked.record_speed(current_speed);
            speed_samples.push_back(current_speed);
            if speed_samples.len() > 4 {
                speed_samples.pop_front();
            }

            let sum: u64 = speed_samples.iter().sum();
            locked.bytes_per_second = sum / speed_samples.len() as u64;
        }

        locked.processed_bytes = current_bytes;
        locked.processed_files = current_files;

        *last_processed_bytes = current_bytes;
        *last_emit = std::time::Instant::now();
    }

    pub(super) fn copy_worker_count(file_count: usize, total_bytes: u64, core_count: usize) -> usize {
        if file_count < 8 || total_bytes / file_count as u64 >= 32 * 1024 * 1024 {
            1
        } else {
            core_count.clamp(2, 4)
        }
    }
}
