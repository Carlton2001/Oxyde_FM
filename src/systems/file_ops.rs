use crate::models::ConflictAction;
use crate::models::{HistoryManager, Transaction, TransactionDetails, TransactionType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::platform::{platform, ThreadPriority};

struct PendingFile(PathBuf);

impl Drop for PendingFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

struct RestoreNormalPriority;

impl Drop for RestoreNormalPriority {
    fn drop(&mut self) {
        platform().set_thread_priority(ThreadPriority::Normal);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OpStatus {
    Queued,
    Calculating,
    Running,
    Paused,
    Cancelled,
    Completed,
    Error(String),
    // Note: WaitingForConflictResolution was removed — conflicts are resolved before the operation starts
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileOpType {
    Copy,
    Move,
    Delete,
    Trash, // Move to recycle bin
}

#[derive(Debug, Clone, Serialize)]
pub struct FileOperation {
    pub id: String,
    pub op_type: FileOpType,
    pub sources: Vec<PathBuf>,
    pub destination: Option<PathBuf>, // None for Delete/Trash
    pub status: OpStatus,
    pub total_bytes: u64,
    pub processed_bytes: u64,
    pub total_files: usize,
    pub processed_files: usize,
    pub current_file: Option<String>,
    pub bytes_per_second: u64,
    pub turbo: bool,
    pub is_cross_volume: bool,
    pub resolutions: Option<HashMap<String, ConflictAction>>,
    #[serde(skip)]
    pub created_files: Vec<PathBuf>,
    #[serde(skip)]
    pub created_dirs: Vec<PathBuf>,
    #[serde(skip)]
    pub replaced_files: Vec<(PathBuf, PathBuf)>,
    #[serde(skip)]
    pub moved_files: Vec<(PathBuf, PathBuf)>,
    #[serde(skip)]
    pub moved_dirs: Vec<(PathBuf, PathBuf)>,
    #[serde(skip)]
    pub copied_files: Vec<(PathBuf, PathBuf)>,
    #[serde(skip)]
    pub copied_file_hashes: Vec<(PathBuf, String)>,
    // Private/Internal state, not serialized by default unless needed
    #[serde(skip)]
    pub cancel_flag: Arc<AtomicBool>,
    #[serde(skip)]
    pub pause_flag: Arc<AtomicBool>,
    #[serde(skip)]
    pub turbo_flag: Arc<AtomicBool>,
}

impl FileOperation {
    pub fn new(
        op_type: FileOpType,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
        resolutions: Option<HashMap<String, ConflictAction>>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            op_type,
            sources,
            destination,
            status: OpStatus::Queued,
            total_bytes: 0,
            processed_bytes: 0,
            total_files: 0,
            processed_files: 0,
            current_file: None,
            bytes_per_second: 0,
            turbo: false,
            is_cross_volume: false,
            resolutions,
            created_files: Vec::new(),
            created_dirs: Vec::new(),
            replaced_files: Vec::new(),
            moved_files: Vec::new(),
            moved_dirs: Vec::new(),
            copied_files: Vec::new(),
            copied_file_hashes: Vec::new(),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            pause_flag: Arc::new(AtomicBool::new(false)),
            turbo_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub struct FileOperationManager {
    operations: Mutex<HashMap<String, Arc<Mutex<FileOperation>>>>,
}

impl Default for FileOperationManager {
    fn default() -> Self {
        Self {
            operations: Mutex::new(HashMap::new()),
        }
    }
}

impl FileOperationManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn queue_operation(
        &self,
        history_manager: Arc<HistoryManager>,
        op: FileOperation,
    ) -> String {
        self.queue_operation_on(&tokio::runtime::Handle::current(), history_manager, op)
    }

    pub fn queue_operation_on(
        &self,
        runtime: &tokio::runtime::Handle,
        history_manager: Arc<HistoryManager>,
        op: FileOperation,
    ) -> String {
        let op_id = op.id.clone();
        let op_arc = Arc::new(Mutex::new(op));

        {
            let mut ops = self.operations.lock().unwrap();

            // Cleanup: remove operations in final states if more than 50 records exist
            if ops.len() > 50 {
                let to_remove: Vec<String> = ops
                    .iter()
                    .filter(|(_, op_arc)| {
                        let locked = op_arc.lock().unwrap();
                        matches!(
                            locked.status,
                            OpStatus::Completed | OpStatus::Cancelled | OpStatus::Error(_)
                        )
                    })
                    .map(|(id, _)| id.clone())
                    .take(20) // Remove up to 20 at a time
                    .collect();

                for id in to_remove {
                    ops.remove(&id);
                }
            }

            ops.insert(op_id.clone(), op_arc.clone());
        }

        // Spawn background task
        let op_clone = op_arc.clone();

        runtime.spawn(async move {
            Self::execute_operation(history_manager, op_clone).await;
        });

        op_id
    }

    async fn execute_operation(
        history_manager: Arc<HistoryManager>,
        op: Arc<Mutex<FileOperation>>,
    ) {
        let op_clone = op.clone();

        // Run blocking IO in a separate thread
        let result = tokio::task::spawn_blocking(move || {
            let _priority_guard = RestoreNormalPriority;
            let (op_type, sources, destination, initial_turbo, resolutions) = {
                let mut locked = op_clone.lock().unwrap();
                locked.status = OpStatus::Calculating;
                let turbo = locked.turbo_flag.load(Ordering::Relaxed);
                (
                    locked.op_type.clone(),
                    locked.sources.clone(),
                    locked.destination.clone(),
                    turbo,
                    locked.resolutions.clone(),
                )
            };

            // Set initial thread priority based on turbo mode
            if !initial_turbo {
                platform().set_thread_priority(ThreadPriority::Background);
            }

            match op_type {
                FileOpType::Copy => {
                    Self::perform_copy(&op_clone, sources, destination, false, resolutions)
                }
                FileOpType::Move => {
                    Self::perform_copy(&op_clone, sources, destination, true, resolutions)
                }
                FileOpType::Delete => Self::perform_delete(&op_clone, sources),
                FileOpType::Trash => Self::perform_trash(&op_clone, sources),
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
        // If it was cancelled, keep Cancelled status
        if let OpStatus::Cancelled = locked.status {
            // keep it
        } else {
            locked.status = final_status.clone();
        }

        // Record exact filesystem effects, including partial or cancelled work,
        // so Undo never guesses from the original top-level paths.
        let has_effects = !locked.created_files.is_empty()
            || !locked.created_dirs.is_empty()
            || !locked.replaced_files.is_empty()
            || !locked.moved_files.is_empty()
            || !locked.moved_dirs.is_empty()
            || !locked.copied_files.is_empty();
        if final_status == OpStatus::Completed || has_effects {
            let tx_type = match locked.op_type {
                FileOpType::Copy => Some(TransactionType::Copy),
                FileOpType::Move => Some(TransactionType::Move),
                FileOpType::Trash => Some(TransactionType::Delete),
                FileOpType::Delete => None,
            };

            if let Some(t_type) = tx_type {
                let sources_str: Vec<String> = locked
                    .sources
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();
                let target_str = locked
                    .destination
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string());

                let details = TransactionDetails {
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
                    replaced_files: Some(
                        locked
                            .replaced_files
                            .iter()
                            .map(|(p, b)| {
                                (
                                    p.to_string_lossy().into_owned(),
                                    b.to_string_lossy().into_owned(),
                                )
                            })
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
                };

                let tx = Transaction::new(t_type, details);
                history_manager.push(tx);
            }
        }
    }

    fn get_resolution(
        path: &std::path::Path,
        resolutions: &HashMap<String, ConflictAction>,
    ) -> Option<ConflictAction> {
        let path_str = path.to_string_lossy().to_string();
        if let Some(&action) = resolutions.get(&path_str) {
            return Some(action);
        }

        // Check ancestors
        let mut current = path.parent();
        while let Some(parent) = current {
            let parent_str = parent.to_string_lossy().to_string();
            if let Some(&action) = resolutions.get(&parent_str) {
                return Some(action);
            }
            current = parent.parent();
        }
        None
    }

    fn perform_copy(
        op: &Arc<Mutex<FileOperation>>,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
        is_move: bool,
        resolutions: Option<HashMap<String, ConflictAction>>,
    ) -> Result<(), String> {
        let target_dir = destination.ok_or("No destination provided for copy/move".to_string())?;

        let target_dir = std::fs::canonicalize(&target_dir)
            .map_err(|e| format!("Cannot resolve destination {:?}: {e}", target_dir))?;
        if !target_dir.is_dir() {
            return Err(format!("Destination is not a directory: {:?}", target_dir));
        }
        let mut canonical_sources = Vec::with_capacity(sources.len());
        for source in &sources {
            let source_metadata = std::fs::symlink_metadata(source)
                .map_err(|e| format!("Cannot inspect source {:?}: {e}", source))?;
            if source_metadata.file_type().is_symlink() {
                return Err(format!(
                    "Symbolic-link sources are not supported: {:?}",
                    source
                ));
            }
            let canonical = std::fs::canonicalize(source)
                .map_err(|e| format!("Cannot resolve source {:?}: {e}", source))?;
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
        }

        let sources_to_copy = sources.clone();
        let mut total_bytes = 0;
        let mut total_files = 0;

        // Stage moves like copies and remove sources only after all destinations publish.
        if sources_to_copy.is_empty() {
            return Ok(());
        }

        // 2. Calculate size and build file list with resolution checks
        let mut files_to_process = Vec::new();
        let mut directories_to_create = Vec::new();
        let cancel_flag = op.lock().unwrap().cancel_flag.clone();

        for (src, canonical_src) in sources_to_copy.iter().zip(&canonical_sources) {
            if cancel_flag.load(Ordering::Relaxed) {
                return Ok(());
            }
            if !src.exists() {
                continue;
            }
            let file_name = src.file_name().ok_or("Invalid source name")?;
            let dest_root = target_dir.join(file_name);
            if dest_root == *src {
                return Err(format!("Source and destination are identical: {:?}", src));
            }
            if dest_root.exists()
                && std::fs::canonicalize(&dest_root)
                    .is_ok_and(|canonical_destination| canonical_destination == *canonical_src)
            {
                return Err(format!("Source and destination are identical: {:?}", src));
            }

            if src.is_dir() {
                let mut walker = walkdir::WalkDir::new(src).into_iter();
                while let Some(entry) = walker.next() {
                    if cancel_flag.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    let entry = match entry {
                        Ok(e) => e,
                        Err(e) => return Err(e.to_string()),
                    };

                    if entry.file_type().is_symlink() {
                        return Err(format!(
                            "Symbolic links are not supported in copy operations: {:?}",
                            entry.path()
                        ));
                    }

                    let relative = entry.path().strip_prefix(src).map_err(|e| e.to_string())?;
                    let dest_path = dest_root.join(relative);

                    if entry.file_type().is_dir() {
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
                        directories_to_create.push(dest_path);
                        continue;
                    }

                    // Conflict Handling
                    if dest_path.exists() {
                        if let Some(ref res_map) = resolutions {
                            match Self::get_resolution(entry.path(), res_map) {
                                Some(ConflictAction::Skip) => {
                                    if entry.file_type().is_dir() {
                                        walker.skip_current_dir();
                                    }
                                    continue;
                                }
                                Some(ConflictAction::Replace) => {}
                                None => {
                                    continue;
                                }
                            }
                        } else {
                            continue;
                        }
                    }

                    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    total_bytes += size;
                    total_files += 1;
                    files_to_process.push((entry.path().to_path_buf(), dest_path));
                }
            } else {
                let dest_path = dest_root.clone();
                if dest_path.exists() {
                    if let Some(ref res_map) = resolutions {
                        if Self::get_resolution(src, res_map) != Some(ConflictAction::Replace) {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }
                let size = std::fs::metadata(src).map(|m| m.len()).unwrap_or(0);
                total_bytes += size;
                total_files += 1;
                files_to_process.push((src.clone(), dest_path));
            }
        }

        {
            let mut locked = op.lock().unwrap();
            locked.total_bytes = total_bytes;
            locked.total_files = total_files;
            locked.status = OpStatus::Running;
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

        let num_tasks = files_to_process.len();
        for directory in &directories_to_create {
            if cancel_flag.load(Ordering::Relaxed) {
                break;
            }
            let is_new = !directory.exists();
            std::fs::create_dir_all(directory)
                .map_err(|e| format!("Cannot create directory {:?}: {e}", directory))?;
            if is_new {
                op.lock().unwrap().created_dirs.push(directory.clone());
            }
        }
        let files_to_process_arc = Arc::new(files_to_process);
        let current_index = Arc::new(AtomicUsize::new(0));

        // Limit concurrency: For many small files, having more threads helps mask I/O latency.
        // We use roughly 2x core count, but always leave room for the UI.
        let core_count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        let concurrency = (core_count * 2).clamp(4, 16);

        let mut handles = Vec::with_capacity(concurrency);

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

            let handle = std::thread::spawn(move || {
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

                    // In Discret mode, we allow 2 workers instead of just one.
                    // This helps with small files while still being very light on modern CPUs.
                    if !is_turbo && thread_idx > 1 {
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

                    let (src, dest): &(PathBuf, PathBuf) = &files[i];

                    if let Some(parent) = dest.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| format!("Failed to create {:?}: {e}", parent))?;
                    }

                    let mut file_in = std::fs::File::open(src).map_err(|e| {
                        let key = match e.kind() {
                            std::io::ErrorKind::PermissionDenied => "error_access_denied",
                            std::io::ErrorKind::StorageFull => "error_disk_full",
                            _ => return format!("Failed to open {:?}: {}", src, e),
                        }
                        .to_string();
                        let mut share = error_share.lock().unwrap();
                        if share.is_none() {
                            *share = Some(key.clone());
                        }
                        key
                    })?;

                    let temp_path = dest
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new("."))
                        .join(format!(".oxyde-{}.tmp", Uuid::new_v4()));
                    let _temp_guard = PendingFile(temp_path.clone());
                    let mut file_out = std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&temp_path)
                        .map_err(|e| format!("Failed to create {:?}: {e}", temp_path))?;
                    let _ = platform().set_hidden_attribute(&temp_path, true);

                    let buffer_size = 1024 * 1024;
                    let mut buffer = vec![0u8; buffer_size];
                    let mut content_hasher = blake3::Hasher::new();

                    loop {
                        if cancel.load(Ordering::Relaxed) {
                            return Ok(());
                        }
                        if error_share.lock().unwrap().is_some() {
                            return Ok(());
                        }

                        while pause.load(Ordering::Relaxed) {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            if cancel.load(Ordering::Relaxed) {
                                return Ok(());
                            }
                        }

                        let n = match file_in.read(&mut buffer) {
                            Ok(0) => break,
                            Ok(n) => n,
                            Err(e) => {
                                let msg = format!("Read error on {:?}: {}", src, e);
                                let mut share = error_share.lock().unwrap();
                                if share.is_none() {
                                    *share = Some(msg.clone());
                                }
                                return Err(msg);
                            }
                        };

                        if let Err(e) = file_out.write_all(&buffer[..n]) {
                            let msg = format!("Write error on {:?}: {}", dest, e);
                            let mut share = error_share.lock().unwrap();
                            if share.is_none() {
                                *share = Some(msg.clone());
                            }
                            return Err(msg);
                        }
                        content_hasher.update(&buffer[..n]);
                        processed_bytes.fetch_add(n as u64, Ordering::Relaxed);
                    }

                    file_out
                        .sync_all()
                        .map_err(|e| format!("Failed to flush {:?}: {e}", temp_path))?;
                    drop(file_out);
                    platform()
                        .set_hidden_attribute(&temp_path, false)
                        .map_err(|error| {
                            format!("Cannot unhide staging file {:?}: {error}", temp_path)
                        })?;

                    let backup_path = dest
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new("."))
                        .join(format!(".oxyde-{}.bak", Uuid::new_v4()));
                    let had_destination = dest.exists();
                    if had_destination {
                        std::fs::rename(dest, &backup_path)
                            .map_err(|e| format!("Failed to preserve {:?}: {e}", dest))?;
                    }
                    if let Err(error) = std::fs::rename(&temp_path, dest) {
                        if had_destination {
                            if let Err(restore_error) = std::fs::rename(&backup_path, dest) {
                                return Err(format!(
                                    "Cannot publish {:?} ({error}); previous destination preserved at {:?} ({restore_error})",
                                    dest, backup_path
                                ));
                            }
                        }
                        return Err(format!("Cannot publish {:?}: {error}", dest));
                    }
                    if had_destination {
                        operation
                            .lock()
                            .unwrap()
                            .replaced_files
                            .push((dest.clone(), backup_path));
                    } else {
                        operation.lock().unwrap().created_files.push(dest.clone());
                    }
                    operation
                        .lock()
                        .unwrap()
                        .copied_files
                        .push((src.clone(), dest.clone()));
                    operation
                        .lock()
                        .unwrap()
                        .copied_file_hashes
                        .push((dest.clone(), content_hasher.finalize().to_hex().to_string()));

                    processed_files.fetch_add(1, Ordering::Relaxed);
                }

                // Cleanup: Ensure priority is restored before thread dies
                if is_in_background_mode {
                    platform().set_thread_priority(ThreadPriority::Normal);
                }

                Ok::<(), String>(())
            });
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

        // Final update to ensure 100% progress is shown before completion
        {
            let mut locked = op.lock().unwrap();
            locked.processed_bytes = locked.total_bytes;
            locked.processed_files = locked.total_files;
            locked.bytes_per_second = 0;
        }

        if is_move {
            for (source, destination) in files_to_process_arc.iter() {
                std::fs::remove_file(source).map_err(|e| {
                    format!("Copied destination, but cannot remove {:?}: {e}", source)
                })?;
                op.lock()
                    .unwrap()
                    .moved_files
                    .push((source.clone(), destination.clone()));
            }
            let mut source_dirs = Vec::new();
            for source in sources.iter().filter(|source| source.is_dir()) {
                let Some(name) = source.file_name() else {
                    continue;
                };
                let dest_root = target_dir.join(name);
                for entry in walkdir::WalkDir::new(source)
                    .into_iter()
                    .filter_map(Result::ok)
                {
                    if entry.file_type().is_dir() {
                        if let Ok(relative) = entry.path().strip_prefix(source) {
                            let relative = relative.to_path_buf();
                            source_dirs.push((entry.into_path(), dest_root.join(relative)));
                        }
                    }
                }
            }
            source_dirs.sort_by_key(|(path, _)| std::cmp::Reverse(path.components().count()));
            for (directory, dest_directory) in source_dirs {
                match std::fs::remove_dir(&directory) {
                    Ok(()) => op
                        .lock()
                        .unwrap()
                        .moved_dirs
                        .push((directory, dest_directory)),
                    Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(format!(
                            "Moved contents, but cannot remove {:?}: {error}",
                            directory
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    fn emit_progress(
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

    fn perform_delete(op: &Arc<Mutex<FileOperation>>, sources: Vec<PathBuf>) -> Result<(), String> {
        let (turbo, cancel_flag, turbo_flag) = {
            let locked = op.lock().unwrap();
            (
                locked.turbo,
                locked.cancel_flag.clone(),
                locked.turbo_flag.clone(),
            )
        };

        let mut real_sources = Vec::new();
        let mut virtual_sources: HashMap<PathBuf, Vec<String>> = HashMap::new();

        for src in &sources {
            if let Some((archive_path, internal_path)) =
                crate::utils::archive::split_virtual_path(&src.to_string_lossy())
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
            crate::commands::archive::remove_items_from_archive(archive_path, internal_paths)
                .map_err(|e| e.to_string())?;
        }

        if real_sources.is_empty() {
            return Ok(());
        }

        // 2. TURBO MODE for Real Sources
        if turbo {
            platform().shell_delete(&real_sources, false)?;
            return Ok(());
        }

        // 3. Parallel Deletion Loop (Dynamic Turbo/Discret)
        let total_items = real_sources.len();
        let current_index = Arc::new(AtomicUsize::new(0));
        let core_count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        let concurrency = (core_count * 2).clamp(4, 16);

        let mut handles = Vec::with_capacity(concurrency);
        let real_sources_arc = Arc::new(real_sources);
        let processed_files_atomic = Arc::new(AtomicUsize::new(0));
        let error_captured = Arc::new(Mutex::new(None));

        {
            let mut locked = op.lock().unwrap();
            locked.total_files = total_items;
            locked.status = OpStatus::Running;
        }

        for thread_idx in 0..concurrency {
            let sources = real_sources_arc.clone();
            let processed_files = processed_files_atomic.clone();
            let cancel = cancel_flag.clone();
            let turbo = turbo_flag.clone();
            let idx = current_index.clone();
            let error_share = error_captured.clone();

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

                    if !is_turbo && thread_idx > 1 {
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

                    let res = if src.is_dir() {
                        std::fs::remove_dir_all(src)
                    } else {
                        std::fs::remove_file(src)
                    };

                    if let Err(e) = res {
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

                    processed_files.fetch_add(1, Ordering::Relaxed);

                    if !is_turbo {
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
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

        // Check if any error was captured
        if let Some(e) = error_captured.lock().unwrap().clone() {
            return Err(e);
        }

        Ok(())
    }

    fn perform_trash(op: &Arc<Mutex<FileOperation>>, sources: Vec<PathBuf>) -> Result<(), String> {
        let cancel_flag = {
            let locked = op.lock().unwrap();
            locked.cancel_flag.clone()
        };

        let mut real_sources = Vec::new();
        let mut virtual_sources: HashMap<PathBuf, Vec<String>> = HashMap::new();

        for src in &sources {
            if let Some((archive_path, internal_path)) =
                crate::utils::archive::split_virtual_path(&src.to_string_lossy())
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
            crate::commands::archive::remove_items_from_archive(archive_path, internal_paths)
                .map_err(|e| e.to_string())?;
        }

        if real_sources.is_empty() {
            return Ok(());
        }

        {
            let mut locked = op.lock().unwrap();
            locked.status = OpStatus::Running;
            locked.total_files = real_sources.len();
        }

        platform().shell_delete(&real_sources, true)?;

        {
            let mut locked = op.lock().unwrap();
            locked.processed_files = locked.total_files;
        }

        Ok(())
    }

    pub fn set_turbo(&self, id: &str, enabled: bool) {
        let ops = self.operations.lock().unwrap();
        if let Some(op_arc) = ops.get(id) {
            let mut locked = op_arc.lock().unwrap();
            locked.turbo = enabled;
            locked.turbo_flag.store(enabled, Ordering::Relaxed);
        }
    }

    pub fn get_operation(&self, id: &str) -> Option<FileOperation> {
        let ops = self.operations.lock().unwrap();
        ops.get(id).map(|op| op.lock().unwrap().clone())
    }

    pub fn cancel_operation(&self, id: &str) -> bool {
        let ops = self.operations.lock().unwrap();
        if let Some(op) = ops.get(id) {
            let mut locked_op = op.lock().unwrap();
            locked_op.cancel_flag.store(true, Ordering::Relaxed);
            locked_op.status = OpStatus::Cancelled;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operation() -> Arc<Mutex<FileOperation>> {
        Arc::new(Mutex::new(FileOperation::new(
            FileOpType::Move,
            Vec::new(),
            None,
            None,
        )))
    }

    #[test]
    fn move_publishes_complete_file_then_removes_source() {
        let temp = tempfile::tempdir().unwrap();
        let source_dir = temp.path().join("source");
        let target_dir = temp.path().join("target");
        std::fs::create_dir(&source_dir).unwrap();
        std::fs::create_dir(&target_dir).unwrap();
        let source = source_dir.join("file.bin");
        let contents = vec![0x5a; 2 * 1024 * 1024];
        std::fs::write(&source, &contents).unwrap();

        FileOperationManager::perform_copy(
            &operation(),
            vec![source.clone()],
            Some(target_dir.clone()),
            true,
            None,
        )
        .unwrap();

        assert!(!source.exists());
        assert_eq!(
            std::fs::read(target_dir.join("file.bin")).unwrap(),
            contents
        );
    }

    #[test]
    fn copy_skips_existing_destination_without_resolution() {
        let temp = tempfile::tempdir().unwrap();
        let source_dir = temp.path().join("source");
        let target_dir = temp.path().join("target");
        std::fs::create_dir(&source_dir).unwrap();
        std::fs::create_dir(&target_dir).unwrap();
        let source = source_dir.join("file.txt");
        let destination = target_dir.join("file.txt");
        std::fs::write(&source, b"source").unwrap();
        std::fs::write(&destination, b"existing").unwrap();

        FileOperationManager::perform_copy(
            &operation(),
            vec![source.clone()],
            Some(target_dir),
            false,
            None,
        )
        .unwrap();

        assert_eq!(std::fs::read(&source).unwrap(), b"source");
        assert_eq!(std::fs::read(destination).unwrap(), b"existing");
    }

    #[test]
    fn copy_processes_every_selected_source() {
        let temp = tempfile::tempdir().unwrap();
        let source_dir = temp.path().join("source");
        let target_dir = temp.path().join("target");
        std::fs::create_dir(&source_dir).unwrap();
        std::fs::create_dir(&target_dir).unwrap();
        let first = source_dir.join("first.txt");
        let second = source_dir.join("second.txt");
        std::fs::write(&first, b"first").unwrap();
        std::fs::write(&second, b"second").unwrap();

        FileOperationManager::perform_copy(
            &operation(),
            vec![first, second],
            Some(target_dir.clone()),
            false,
            None,
        )
        .unwrap();

        assert_eq!(
            std::fs::read(target_dir.join("first.txt")).unwrap(),
            b"first"
        );
        assert_eq!(
            std::fs::read(target_dir.join("second.txt")).unwrap(),
            b"second"
        );
    }

    #[test]
    fn explicit_replace_keeps_a_backup_and_records_the_effect() {
        let temp = tempfile::tempdir().unwrap();
        let source_dir = temp.path().join("source");
        let target_dir = temp.path().join("target");
        std::fs::create_dir(&source_dir).unwrap();
        std::fs::create_dir(&target_dir).unwrap();
        let source = source_dir.join("file.txt");
        let destination = target_dir.join("file.txt");
        std::fs::write(&source, b"new contents").unwrap();
        std::fs::write(&destination, b"old contents").unwrap();
        let resolutions = HashMap::from([(
            source.to_string_lossy().into_owned(),
            ConflictAction::Replace,
        )]);
        let op = operation();

        FileOperationManager::perform_copy(
            &op,
            vec![source.clone()],
            Some(target_dir.clone()),
            false,
            Some(resolutions),
        )
        .unwrap();

        assert_eq!(std::fs::read(&destination).unwrap(), b"new contents");
        let operation = op.lock().unwrap();
        assert_eq!(operation.replaced_files.len(), 1);
        assert!(operation.replaced_files[0]
            .1
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(".oxyde-"));
        assert_eq!(
            std::fs::read(&operation.replaced_files[0].1).unwrap(),
            b"old contents"
        );
        assert!(std::fs::read_dir(&target_dir).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
        assert_eq!(operation.copied_file_hashes.len(), 1);
        assert_eq!(
            operation.copied_file_hashes[0].1,
            blake3::hash(b"new contents").to_hex().to_string()
        );
    }

    #[test]
    fn move_merge_keeps_skipped_source_files() {
        let temp = tempfile::tempdir().unwrap();
        let source_root = temp.path().join("source");
        let target_root = temp.path().join("target");
        let source = source_root.join("folder");
        let destination = target_root.join("folder");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(&destination).unwrap();
        std::fs::write(source.join("conflict.txt"), b"source version").unwrap();
        std::fs::write(destination.join("conflict.txt"), b"existing version").unwrap();
        std::fs::write(source.join("new.txt"), b"new file").unwrap();

        FileOperationManager::perform_copy(
            &operation(),
            vec![source.clone()],
            Some(target_root),
            true,
            None,
        )
        .unwrap();

        assert_eq!(
            std::fs::read(source.join("conflict.txt")).unwrap(),
            b"source version"
        );
        assert!(!source.join("new.txt").exists());
        assert_eq!(
            std::fs::read(destination.join("new.txt")).unwrap(),
            b"new file"
        );
    }

    #[test]
    fn move_rejects_destination_inside_source() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let target = source.join("nested");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(source.join("file.txt"), b"keep").unwrap();

        let result = FileOperationManager::perform_copy(
            &operation(),
            vec![source.clone()],
            Some(target),
            true,
            None,
        );

        assert!(result.is_err());
        assert_eq!(std::fs::read(source.join("file.txt")).unwrap(), b"keep");
    }

    #[test]
    fn copy_rejects_same_file_even_when_the_source_uses_parent_components() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("folder");
        let detour = folder.join("detour");
        std::fs::create_dir_all(&detour).unwrap();
        let destination = folder.join("file.txt");
        std::fs::write(&destination, b"preserve").unwrap();
        let source_alias = detour.join("..").join("file.txt");
        let resolutions = HashMap::from([(
            source_alias.to_string_lossy().into_owned(),
            ConflictAction::Replace,
        )]);

        assert!(FileOperationManager::perform_copy(
            &operation(),
            vec![source_alias],
            Some(folder),
            true,
            Some(resolutions),
        )
        .is_err());
        assert_eq!(std::fs::read(destination).unwrap(), b"preserve");
    }

    #[test]
    fn move_preserves_source_when_cancelled_before_copy() {
        let temp = tempfile::tempdir().unwrap();
        let source_dir = temp.path().join("source");
        let target_dir = temp.path().join("target");
        std::fs::create_dir(&source_dir).unwrap();
        std::fs::create_dir(&target_dir).unwrap();
        let source = source_dir.join("file.txt");
        std::fs::write(&source, b"preserve me").unwrap();
        let op = operation();
        op.lock()
            .unwrap()
            .cancel_flag
            .store(true, Ordering::Relaxed);

        assert!(FileOperationManager::perform_copy(
            &op,
            vec![source.clone()],
            Some(target_dir.clone()),
            true,
            None,
        )
        .is_ok());
        assert_eq!(std::fs::read(source).unwrap(), b"preserve me");
        assert!(!target_dir.join("file.txt").exists());
    }

    #[test]
    fn move_preserves_empty_directories() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        std::fs::create_dir_all(source.join("empty/nested")).unwrap();
        std::fs::create_dir(&target).unwrap();

        FileOperationManager::perform_copy(
            &operation(),
            vec![source.clone()],
            Some(target.clone()),
            true,
            None,
        )
        .unwrap();

        assert!(target.join("source/empty/nested").is_dir());
        assert!(!source.exists());
    }

    #[cfg(unix)]
    #[test]
    fn copy_rejects_symbolic_links_instead_of_following_them() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        let outside = temp.path().join("outside.txt");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&target).unwrap();
        std::fs::write(&outside, b"outside source tree").unwrap();
        symlink(&outside, source.join("link.txt")).unwrap();

        assert!(FileOperationManager::perform_copy(
            &operation(),
            vec![source.clone()],
            Some(target.clone()),
            false,
            None,
        )
        .is_err());
        assert!(target.join("source/link.txt").exists() == false);
        assert_eq!(std::fs::read(outside).unwrap(), b"outside source tree");
    }
}
