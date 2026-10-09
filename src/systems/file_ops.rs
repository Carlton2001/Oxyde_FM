use crate::models::ConflictAction;
use crate::models::{HistoryManager, Transaction, TransactionDetails, TransactionType};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{Notify, Semaphore};
use uuid::Uuid;

use crate::platform::{platform, ThreadPriority};
use crate::utils::archive_limits::{ArchiveBudget, ArchiveLimits};

const SPEED_HISTORY_LIMIT: usize = 60;

struct PendingFile(PathBuf);

impl Drop for PendingFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

struct ArchiveReader {
    input: std::fs::File,
    operation: Arc<Mutex<FileOperation>>,
}
impl Read for ArchiveReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        FileOperationManager::archive_checkpoint(&self.operation).map_err(std::io::Error::other)?;
        let count = self.input.read(buffer)?;
        FileOperationManager::archive_progress(&self.operation, count as u64);
        Ok(count)
    }
}

struct RestoreNormalPriority;

impl Drop for RestoreNormalPriority {
    fn drop(&mut self) {
        platform().set_thread_priority(ThreadPriority::Normal);
    }
}

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone)]
pub enum FileOpType {
    Copy,
    Move,
    Delete,
    Trash, // Move to recycle bin
    Compress,
    Extract,
}

#[derive(Debug, Clone)]
pub struct FileOperation {
    pub id: String,
    pub sequence: u64,
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
    pub speed_history: Vec<u64>,
    pub duration_ms: Option<u64>,
    started_at: Option<std::time::Instant>,
    speed_sample_at: Option<std::time::Instant>,
    speed_sample_bytes: u64,
    pub compression_quality: String,
    pub archive_qualities: HashMap<String, String>,
    archive_path_mappings: Vec<(PathBuf, PathBuf)>,
    pub turbo: bool,
    pub resolutions: Option<HashMap<String, ConflictAction>>,
    pub created_files: Vec<PathBuf>,
    pub created_dirs: Vec<PathBuf>,
    pub overwritten_files: Vec<PathBuf>,
    pub moved_files: Vec<(PathBuf, PathBuf)>,
    trash_metadata_backups: Vec<PathBuf>,
    pub moved_dirs: Vec<(PathBuf, PathBuf)>,
    pub copied_files: Vec<(PathBuf, PathBuf)>,
    pub copied_file_hashes: Vec<(PathBuf, String)>,
    copied_file_identities: Vec<(PathBuf, crate::models::transaction::HistoryFileIdentity)>,
    copied_directory_identities: Vec<(
        PathBuf,
        crate::models::transaction::HistoryDirectoryIdentity,
    )>,
    // Internal execution state.
    pub cancel_flag: Arc<AtomicBool>,
    pub pause_flag: Arc<AtomicBool>,
    pub turbo_flag: Arc<AtomicBool>,
    resume_requested: bool,
    execution_started: bool,
}

impl FileOperation {
    fn snapshot(&self) -> Self {
        Self {
            id: self.id.clone(),
            sequence: self.sequence,
            op_type: self.op_type.clone(),
            sources: self.sources.clone(),
            destination: self.destination.clone(),
            status: self.status.clone(),
            total_bytes: self.total_bytes,
            processed_bytes: self.processed_bytes,
            total_files: self.total_files,
            processed_files: self.processed_files,
            current_file: self.current_file.as_ref().map(|path| {
                let path = Path::new(path);
                self.archive_path_mappings
                    .iter()
                    .find_map(|(root, archive)| {
                        path.strip_prefix(root)
                            .ok()
                            .map(|relative| archive.join(relative))
                    })
                    .unwrap_or_else(|| path.to_path_buf())
                    .to_string_lossy()
                    .into_owned()
            }),
            bytes_per_second: self.bytes_per_second,
            speed_history: self.speed_history.clone(),
            duration_ms: self.duration_ms,
            started_at: self.started_at,
            speed_sample_at: None,
            speed_sample_bytes: 0,
            compression_quality: self.compression_quality.clone(),
            archive_qualities: self.archive_qualities.clone(),
            archive_path_mappings: self.archive_path_mappings.clone(),
            turbo: self.turbo,
            resolutions: None,
            created_files: Vec::new(),
            created_dirs: Vec::new(),
            overwritten_files: Vec::new(),
            moved_files: Vec::new(),
            trash_metadata_backups: Vec::new(),
            moved_dirs: Vec::new(),
            copied_files: Vec::new(),
            copied_file_hashes: Vec::new(),
            copied_file_identities: Vec::new(),
            copied_directory_identities: Vec::new(),
            cancel_flag: self.cancel_flag.clone(),
            pause_flag: self.pause_flag.clone(),
            turbo_flag: self.turbo_flag.clone(),
            resume_requested: self.resume_requested,
            execution_started: self.execution_started,
        }
    }

    pub fn new(
        op_type: FileOpType,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
        resolutions: Option<HashMap<String, ConflictAction>>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            sequence: 0,
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
            speed_history: Vec::new(),
            duration_ms: None,
            started_at: None,
            speed_sample_at: None,
            speed_sample_bytes: 0,
            compression_quality: "normal".into(),
            archive_qualities: HashMap::new(),
            archive_path_mappings: Vec::new(),
            turbo: false,
            resolutions,
            created_files: Vec::new(),
            created_dirs: Vec::new(),
            overwritten_files: Vec::new(),
            moved_files: Vec::new(),
            trash_metadata_backups: Vec::new(),
            moved_dirs: Vec::new(),
            copied_files: Vec::new(),
            copied_file_hashes: Vec::new(),
            copied_file_identities: Vec::new(),
            copied_directory_identities: Vec::new(),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            pause_flag: Arc::new(AtomicBool::new(false)),
            turbo_flag: Arc::new(AtomicBool::new(false)),
            resume_requested: false,
            execution_started: false,
        }
    }

    fn record_speed(&mut self, bytes_per_second: u64) {
        if self.speed_history.len() == SPEED_HISTORY_LIMIT {
            self.speed_history.remove(0);
        }
        self.speed_history.push(bytes_per_second);
    }
}

pub struct FileOperationManager {
    operations: Mutex<HashMap<String, Arc<Mutex<FileOperation>>>>,
    next_sequence: AtomicU64,
    scheduler: Arc<OperationScheduler>,
}

struct ScheduledOperation {
    operation: Arc<Mutex<FileOperation>>,
    paths: Vec<String>,
}

struct OperationScheduler {
    slots: Arc<Semaphore>,
    running_paths: Mutex<HashMap<String, Vec<String>>>,
    waiting: Mutex<HashMap<String, ScheduledOperation>>,
    changed: Notify,
}

impl OperationScheduler {
    fn enqueue(&self, operation: Arc<Mutex<FileOperation>>, paths: Vec<String>) {
        let id = operation.lock().unwrap().id.clone();
        self.waiting
            .lock()
            .unwrap()
            .insert(id, ScheduledOperation { operation, paths });
        self.changed.notify_waiters();
    }

    async fn acquire(&self, id: &str) -> Option<tokio::sync::OwnedSemaphorePermit> {
        loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            {
                let mut waiting = self.waiting.lock().unwrap();
                let mut running = self.running_paths.lock().unwrap();
                let entry = waiting.get(id)?;
                let stopped = {
                    let operation = entry.operation.lock().unwrap();
                    operation.cancel_flag.load(Ordering::Relaxed)
                        || FileOperationManager::is_final(&operation.status)
                };
                if stopped {
                    waiting.remove(id);
                    return None;
                }
                let next = waiting
                    .iter()
                    .filter_map(|(candidate_id, entry)| {
                        let operation = entry.operation.lock().unwrap();
                        if operation.cancel_flag.load(Ordering::Relaxed)
                            || FileOperationManager::is_final(&operation.status)
                            || (operation.pause_flag.load(Ordering::Relaxed)
                                && !operation.resume_requested)
                            || running.iter().any(|(active_id, paths)| {
                                active_id != candidate_id
                                    && FileOperationManager::paths_overlap(&entry.paths, paths)
                            })
                        {
                            return None;
                        }
                        Some((
                            candidate_id.clone(),
                            (!operation.resume_requested, operation.sequence),
                        ))
                    })
                    .min_by_key(|(_, priority)| *priority)
                    .map(|(id, _)| id);
                if next.as_deref() == Some(id) {
                    if let Ok(permit) = self.slots.clone().try_acquire_owned() {
                        let entry = waiting.remove(id).unwrap();
                        let mut operation = entry.operation.lock().unwrap();
                        if operation.pause_flag.load(Ordering::Relaxed)
                            && !operation.resume_requested
                        {
                            drop(operation);
                            waiting.insert(id.to_string(), entry);
                        } else {
                            operation.pause_flag.store(false, Ordering::Relaxed);
                            operation.resume_requested = false;
                            operation.execution_started = true;
                            operation.status = if operation.started_at.is_some() {
                                OpStatus::Running
                            } else {
                                OpStatus::Calculating
                            };
                            running.insert(id.to_string(), entry.paths);
                            self.changed.notify_waiters();
                            return Some(permit);
                        }
                    }
                }
            }
            // Also observe permits released outside this scheduler (e.g. shutdown).
            let _ = tokio::time::timeout(std::time::Duration::from_millis(25), notified).await;
        }
    }
}

impl Default for FileOperationManager {
    fn default() -> Self {
        Self {
            operations: Mutex::new(HashMap::new()),
            next_sequence: AtomicU64::new(1),
            scheduler: Arc::new(OperationScheduler {
                slots: Arc::new(Semaphore::new(2)),
                running_paths: Mutex::new(HashMap::new()),
                waiting: Mutex::new(HashMap::new()),
                changed: Notify::new(),
            }),
        }
    }
}

impl FileOperationManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn queue_operation_on(
        &self,
        runtime: &tokio::runtime::Handle,
        history_manager: Arc<HistoryManager>,
        mut op: FileOperation,
    ) -> String {
        op.sequence = self.next_sequence.fetch_add(1, Ordering::Relaxed);
        if op.pause_flag.load(Ordering::Relaxed) {
            op.status = OpStatus::Paused;
        }
        let op_id = op.id.clone();
        let paths = Self::reserved_paths(&op);
        let op_arc = Arc::new(Mutex::new(op));

        {
            let mut ops = self.operations.lock().unwrap();

            // Cleanup: remove operations in final states if more than 50 records exist
            if ops.len() > 50 {
                let mut to_remove: Vec<(u64, String)> = ops
                    .iter()
                    .filter(|(_, op_arc)| {
                        let locked = op_arc.lock().unwrap();
                        matches!(
                            locked.status,
                            OpStatus::Completed | OpStatus::Cancelled | OpStatus::Error(_)
                        )
                    })
                    .map(|(id, op)| (op.lock().unwrap().sequence, id.clone()))
                    .collect();
                to_remove.sort_by_key(|(sequence, _)| *sequence);
                for (_, id) in to_remove.into_iter().take(20) {
                    ops.remove(&id);
                }
            }

            ops.insert(op_id.clone(), op_arc.clone());
        }

        // Spawn background task
        let op_clone = op_arc.clone();
        let scheduler = self.scheduler.clone();
        let scheduled_id = op_id.clone();
        scheduler.enqueue(op_arc, paths.clone());

        runtime.spawn(async move {
            let Some(initial_permit) = scheduler.acquire(&scheduled_id).await else {
                op_clone.lock().unwrap().status = OpStatus::Cancelled;
                return;
            };
            let mut permit = Some(initial_permit);
            let operation = op_clone.clone();
            let mut execution = Box::pin(async move {
                match Self::check_queued_conflicts(operation.clone()).await {
                    Ok(())
                        if !operation
                            .lock()
                            .unwrap()
                            .cancel_flag
                            .load(Ordering::Relaxed) =>
                    {
                        Self::execute_operation(history_manager, operation.clone()).await;
                    }
                    Ok(()) => operation.lock().unwrap().status = OpStatus::Cancelled,
                    Err(error) => {
                        let mut locked = operation.lock().unwrap();
                        locked.status = if locked.cancel_flag.load(Ordering::Relaxed) {
                            OpStatus::Cancelled
                        } else {
                            OpStatus::Error(error)
                        };
                    }
                }
            });
            loop {
                let notified = scheduler.changed.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();
                let should_release = {
                    let mut locked = op_clone.lock().unwrap();
                    if permit.is_some()
                        && locked.resume_requested
                        && !locked.cancel_flag.load(Ordering::Relaxed)
                    {
                        locked.pause_flag.store(false, Ordering::Relaxed);
                        locked.resume_requested = false;
                        locked.status = if locked.started_at.is_some() {
                            OpStatus::Running
                        } else {
                            OpStatus::Calculating
                        };
                    }
                    locked.pause_flag.load(Ordering::Relaxed)
                        && !locked.cancel_flag.load(Ordering::Relaxed)
                };
                if should_release && permit.is_some() {
                    // Keep path reservations while parked, but free execution capacity.
                    scheduler.enqueue(op_clone.clone(), paths.clone());
                    drop(permit.take());
                    scheduler.changed.notify_waiters();
                }
                if permit.is_none() {
                    let acquired = Box::pin(scheduler.acquire(&scheduled_id));
                    match futures::future::select(execution.as_mut(), acquired).await {
                        futures::future::Either::Left(_) => break,
                        futures::future::Either::Right((Some(acquired), _)) => {
                            permit = Some(acquired);
                        }
                        futures::future::Either::Right((None, _)) => {
                            execution.await;
                            break;
                        }
                    }
                } else {
                    let changed = Box::pin(tokio::time::timeout(
                        std::time::Duration::from_millis(25),
                        notified,
                    ));
                    if matches!(
                        futures::future::select(execution.as_mut(), changed).await,
                        futures::future::Either::Left(_)
                    ) {
                        break;
                    }
                }
            }
            scheduler.waiting.lock().unwrap().remove(&scheduled_id);
            scheduler
                .running_paths
                .lock()
                .unwrap()
                .remove(&scheduled_id);
            drop(permit);
            scheduler.changed.notify_waiters();
        });

        op_id
    }

    fn reserved_paths(op: &FileOperation) -> Vec<String> {
        let destinations: Vec<PathBuf> = match (&op.op_type, &op.destination) {
            (FileOpType::Copy | FileOpType::Move, Some(destination)) => op
                .sources
                .iter()
                .filter_map(|source| source.file_name().map(|name| destination.join(name)))
                .collect(),
            (_, destination) => destination.iter().cloned().collect(),
        };
        op.sources
            .iter()
            .chain(destinations.iter())
            .map(|path| {
                let path = crate::utils::archive::split_virtual_path(&path.to_string_lossy())
                    .map(|(archive, _)| archive)
                    .unwrap_or_else(|| path.clone());
                let absolute = if path.is_absolute() {
                    path.clone()
                } else {
                    std::env::current_dir().unwrap_or_default().join(path)
                };
                let mut parts = Vec::new();
                for component in absolute.components() {
                    match component {
                        std::path::Component::CurDir => {}
                        std::path::Component::ParentDir => {
                            parts.pop();
                        }
                        other => parts.push(other.as_os_str().to_string_lossy().into_owned()),
                    }
                }
                parts.join("/").to_lowercase()
            })
            .collect()
    }

    fn paths_overlap(left: &[String], right: &[String]) -> bool {
        left.iter().any(|a| {
            right.iter().any(|b| {
                a == b || a.starts_with(&format!("{b}/")) || b.starts_with(&format!("{a}/"))
            })
        })
    }

    async fn check_queued_conflicts(op: Arc<Mutex<FileOperation>>) -> Result<(), String> {
        let (sources, destination, resolutions) = {
            let locked = op.lock().unwrap();
            if !matches!(locked.op_type, FileOpType::Copy | FileOpType::Move) {
                return Ok(());
            }
            (
                locked
                    .sources
                    .iter()
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect(),
                locked
                    .destination
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
                locked.resolutions.clone().unwrap_or_default(),
            )
        };
        let Some(destination) = destination else {
            return Ok(());
        };
        tokio::task::spawn_blocking(move || {
            let checked = futures::executor::block_on(crate::commands::ops::check_conflicts(sources, destination))
                .map_err(|error| error.to_string())?;
            if checked.iter().any(|conflict| {
                Self::get_resolution(std::path::Path::new(&conflict.source.path), &resolutions).is_none()
            }) {
                return Err("Destination changed while the operation was waiting; retry the transfer to resolve new conflicts".to_string());
            }
            Ok(())
        })
        .await
        .map_err(|error| error.to_string())?
    }

    async fn execute_operation(
        history_manager: Arc<HistoryManager>,
        op: Arc<Mutex<FileOperation>>,
    ) {
        let op_clone = op.clone();
        let started_at = std::time::Instant::now();

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
        // If it was cancelled, keep Cancelled status
        let finished_before_cancel = final_status == OpStatus::Completed
            && locked.total_files > 0
            && locked.processed_files == locked.total_files;
        if locked.cancel_flag.load(Ordering::Relaxed) && !finished_before_cancel {
            locked.status = OpStatus::Cancelled;
        } else {
            locked.status = final_status.clone();
        }
        locked.speed_history.clear();

        // Record exact filesystem effects, including partial or cancelled work,
        // so Undo never guesses from the original top-level paths.
        let has_effects = !locked.created_files.is_empty()
            || !locked.created_dirs.is_empty()
            || !locked.overwritten_files.is_empty()
            || !locked.moved_files.is_empty()
            || !locked.moved_dirs.is_empty()
            || !locked.copied_files.is_empty();
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
                        // Keep an overwrite-only copy as an undo-history
                        // barrier so Undo cannot accidentally target an older action.
                        || !locked.overwritten_files.is_empty()
                }
                FileOpType::Move => {
                    !locked.created_files.is_empty()
                        || !locked.created_dirs.is_empty()
                        || !locked.moved_files.is_empty()
                        || !locked.moved_dirs.is_empty()
                }
                _ => true,
            };
            if let Some(t_type) = tx_type.filter(|_| has_undoable_effects) {
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

    fn perform_archive_transfer(
        op: &Arc<Mutex<FileOperation>>,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
        is_move: bool,
        resolutions: Option<HashMap<String, ConflictAction>>,
    ) -> Result<(), String> {
        Self::perform_archive_transfer_with(op, sources, destination, is_move, resolutions, &mut || Ok(()))
    }

    fn perform_archive_transfer_with(
        op: &Arc<Mutex<FileOperation>>,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
        is_move: bool,
        resolutions: Option<HashMap<String, ConflictAction>>,
        after_copy: &mut dyn FnMut() -> Result<(), String>,
    ) -> Result<(), String> {
        use crate::systems::archive_workspace::{
            lock_for_operation, mutation_locks, virtual_destination, virtual_source, writable,
            ArchiveWorkspaces,
        };

        Self::archive_checkpoint(op)?;
        let target = destination.ok_or("Missing destination")?;
        let target_archive = virtual_destination(&target)
            .map(|(archive, _)| std::fs::canonicalize(archive).map_err(|e| e.to_string()))
            .transpose()?;
        if target_archive.as_ref().is_some_and(|p| !writable(p))
            || (is_move
                && sources
                    .iter()
                    .filter_map(|p| virtual_source(p))
                    .any(|(archive, _)| !writable(&archive)))
        {
            return Err("This archive format is read-only".into());
        }
        if target_archive.as_ref().is_some_and(|archive| {
            sources
                .iter()
                .any(|source| std::fs::canonicalize(source).is_ok_and(|source| source == *archive))
        }) {
            return Err("Cannot put an archive inside itself".into());
        }
        let locks = mutation_locks(
            target_archive.iter().cloned().chain(
                sources
                    .iter()
                    .filter_map(|p| virtual_source(p).map(|(archive, _)| archive)),
            ),
        )?;
        let _guards = locks
            .iter()
            .map(|lock| lock_for_operation(lock, op))
            .collect::<Result<Vec<_>, _>>()?;
        let mut workspaces = ArchiveWorkspaces::default();
        let real_target = workspaces.resolve_for_operation(&target, true, op)?;
        let mut real_sources = Vec::new();
        for source in &sources {
            real_sources.push(workspaces.resolve_for_operation(source, false, op)?);
        }
        op.lock().unwrap().archive_path_mappings = workspaces
            .0
            .values()
            .map(|workspace| (workspace.root().to_path_buf(), workspace.archive.clone()))
            .collect();
        Self::archive_checkpoint(op)?;
        let mapped_resolutions = resolutions
            .map(|resolutions| {
                resolutions
                    .into_iter()
                    .map(|(path, action)| {
                        workspaces
                            .resolve(Path::new(&path), false)
                            .map(|path| (path.to_string_lossy().into_owned(), action))
                    })
                    .collect::<Result<HashMap<_, _>, _>>()
            })
            .transpose()?;
        let deferred_moves = is_move.then(|| Arc::new(Mutex::new(
            super::archive_move_sources::ArchiveMoveSources::default(),
        )));
        let trash_entries = platform().trash_transfer_entries(&real_sources)?;
        Self::perform_copy_real(
            op,
            real_sources.clone(),
            Some(real_target.clone()),
            false,
            mapped_resolutions,
            &trash_entries,
            deferred_moves.clone(),
        )?;
        Self::archive_checkpoint(op)?;
        after_copy()?;
        let copied = op.lock().unwrap().copied_files.clone();
        let mut changed = std::collections::HashSet::new();
        if let Some(archive) = &target_archive {
            changed.insert(archive.clone());
        }
        // Stage removals only for files that were actually copied. Skipped conflicts stay put.
        if is_move {
            let mut moves = deferred_moves.as_ref().unwrap().lock().unwrap();
            moves.verify_all(&mut || Self::archive_checkpoint(op).map_err(std::io::Error::other))
                .map_err(|error| error.to_string())?;
            moves.remove_matching(&|source| workspaces.display(source) != source,
                &mut || Self::archive_checkpoint(op).map_err(std::io::Error::other))
                .map_err(|error| error.to_string())?;
            drop(moves);
            for (source, _) in &copied {
                for (archive, workspace) in &workspaces.0 {
                    if source.starts_with(workspace.root()) {
                        changed.insert(archive.clone());
                    }
                }
            }
            deferred_moves.as_ref().unwrap().lock().unwrap().remove_directories_matching(
                &|source| workspaces.display(source) != source,
                &mut || Self::archive_checkpoint(op).map_err(std::io::Error::other),
                &mut |source, _| {
                    for (archive, workspace) in &workspaces.0 {
                        if source.starts_with(workspace.root()) {
                            changed.insert(archive.clone());
                        }
                    }
                },
            ).map_err(|error| error.to_string())?;
        }
        // Publish destination first: a failed source rewrite leaves a recoverable copy.
        if let Some(archive) = &target_archive {
            workspaces.0[archive].commit(op)?;
            changed.remove(archive);
        }
        for archive in changed {
            workspaces.0[&archive].commit(op)?;
        }
        if is_move {
            // Sources have remained held since copy, including during reconstruction.
            deferred_moves.as_ref().unwrap().lock().unwrap().remove_matching(
                &|source| workspaces.display(source) == source,
                &mut || Self::archive_checkpoint(op).map_err(std::io::Error::other),
            ).map_err(|error| error.to_string())?;
            deferred_moves.as_ref().unwrap().lock().unwrap().remove_directories_matching(
                &|source| workspaces.display(source) == source,
                &mut || Self::archive_checkpoint(op).map_err(std::io::Error::other),
                &mut |_, _| {},
            ).map_err(|error| error.to_string())?;
        }
        // These effects are not filesystem Undo entries: their paths may belong to a temporary view.
        let mut locked = op.lock().unwrap();
        locked.created_files.clear();
        locked.created_dirs.clear();
        locked.overwritten_files.clear();
        locked.copied_files.clear();
        locked.copied_file_hashes.clear();
        locked.copied_file_identities.clear();
        locked.copied_directory_identities.clear();
        Ok(())
    }

    fn remove_archive_entries(
        op: &Arc<Mutex<FileOperation>>,
        archive: PathBuf,
        internal: Vec<String>,
    ) -> Result<(), String> {
        use crate::systems::archive_workspace::{
            lock_for_operation, mutation_locks, writable, ArchiveWorkspace,
        };
        let locks = mutation_locks([archive.clone()])?;
        let _lock = lock_for_operation(&locks[0], op)?;
        if !writable(&archive) {
            return Err("This archive format is read-only".into());
        }
        Self::archive_checkpoint(op)?;
        let workspace = ArchiveWorkspace::open_for_operation(archive, op)?;
        op.lock().unwrap().archive_path_mappings =
            vec![(workspace.root().to_path_buf(), workspace.archive.clone())];
        for path in internal {
            Self::archive_checkpoint(op)?;
            let path = workspace.path(&path)?;
            if path.is_dir() {
                std::fs::remove_dir_all(&path).map_err(|e| e.to_string())?;
            } else if path.exists() {
                std::fs::remove_file(&path).map_err(|e| e.to_string())?;
            }
        }
        workspace.commit(op)
    }

    fn record_created_copy_directory(
        operation: &Arc<Mutex<FileOperation>>,
        tree: &super::destination_tree::DestinationTree,
        path: &std::path::Path,
        is_move: bool,
    ) {
        let identity = if is_move {
            None
        } else {
            tree.directory_identity(path)
                .map_err(|error| {
                    log::warn!(
                        "Copy directory identity unavailable for {}: {error}",
                        path.display()
                    )
                })
                .ok()
        };
        let mut journal = operation.lock().unwrap();
        journal.created_dirs.push(path.to_path_buf());
        if let Some(identity) = identity {
            journal
                .copied_directory_identities
                .push((path.to_path_buf(), identity));
        }
    }

    fn perform_copy(
        op: &Arc<Mutex<FileOperation>>,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
        is_move: bool,
        resolutions: Option<HashMap<String, ConflictAction>>,
    ) -> Result<(), String> {
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
                    Ok(Some((_, backup))) if virtual_effects => {
                        // Archive transfers already have no filesystem Undo journal.
                        if let Err(error) = std::fs::remove_file(&backup) {
                            metadata_errors.push(error.to_string());
                        }
                    }
                    Ok(Some((source, backup))) => {
                        let mut locked = op.lock().unwrap();
                        locked.trash_metadata_backups.push(backup.clone());
                        locked.moved_files.push((source, backup));
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
    fn perform_copy_real(
        op: &Arc<Mutex<FileOperation>>,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
        is_move: bool,
        resolutions: Option<HashMap<String, ConflictAction>>,
        trash_entries: &[crate::platform::TrashTransferEntry],
        deferred_moves: Option<Arc<Mutex<super::archive_move_sources::ArchiveMoveSources>>>,
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
        let target_dir = destination.ok_or("No destination provided for copy/move".to_string())?;

        let destination_tree = Arc::new(super::destination_tree::DestinationTree::new(&target_dir)
            .map_err(|e| format!("Cannot hold destination {:?}: {e}", target_dir))?);
        let target_dir = destination_tree.path().to_path_buf();
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

        if !trash_entries.is_empty() {
            let mut names = std::collections::HashSet::new();
            for source in &sources {
                let name = source_name(source)?.to_string_lossy().to_lowercase();
                if !names.insert(name) {
                    return Err("Selected trash items have the same original name; transfer them separately".into());
                }
            }
        }
        let sources_to_copy = sources.clone();
        let mut total_bytes = 0;
        let mut total_files = 0;

        // Build the conflict-aware plan before changing any source.
        if sources_to_copy.is_empty() {
            return Ok(());
        }

        // 2. Calculate size and build file list with resolution checks
        let mut files_to_process = Vec::new();
        let mut directories_to_create = Vec::new();
        let mut source_directories = super::source_directories::SourceDirectories::default();
        let cancel_flag = op.lock().unwrap().cancel_flag.clone();

        for (src, canonical_src) in sources_to_copy.iter().zip(&canonical_sources) {
            if cancel_flag.load(Ordering::Relaxed) {
                return Ok(());
            }
            if !src.exists() {
                continue;
            }
            let file_name = source_name(src)?;
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
                        if is_move || deferred_moves.is_some() {
                            source_directories.capture(entry.path(), &dest_path).map_err(|error| {
                                format!("Cannot prepare source directory {:?} for move: {error}", entry.path())
                            })?;
                        }
                        directories_to_create.push(dest_path);
                        continue;
                    }

                    // Conflict Handling
                    let replace_existing = dest_path.exists();
                    if replace_existing {
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
                    files_to_process.push((
                        entry.path().to_path_buf(),
                        dest_path,
                        replace_existing,
                    ));
                }
            } else {
                let dest_path = dest_root.clone();
                let replace_existing = dest_path.exists();
                if replace_existing {
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
                files_to_process.push((src.clone(), dest_path, replace_existing));
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
                match super::path_parents::move_file_with_parents(&source, &destination, replace_existing, destination_parents, &mut || {
                    Self::archive_checkpoint(op).map_err(std::io::Error::other)
                }) {
                    Ok(size) => {
                        let mut locked = op.lock().unwrap();
                        locked.current_file = Some(source.to_string_lossy().into_owned());
                        if replace_existing {
                            locked.overwritten_files.push(destination.clone());
                        }
                        locked.moved_files.push((source, destination));
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
                            source, destination
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

                    let (src, dest, replace_existing): &(PathBuf, PathBuf, bool) = &files[i];
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

                    let source_guard =
                        if is_move || deferred_moves.is_some() {
                            Some(super::copied_source::CopiedSource::open(src).map_err(
                                |error| format!("Cannot hold move source {:?}: {error}", src),
                            )?)
                        } else {
                            None
                        };
                    let mut copied = 0;
                    let output = super::file_copy::copy_with_parents(
                        src,
                        dest,
                        *replace_existing,
                        source_guard.as_ref().map(|source| source.file()),
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
                    let copied_identity = if !is_move {
                        match crate::models::transaction::HistoryFileIdentity::from_file(
                            &output.file,
                        ) {
                            Ok(identity) => Some(identity),
                            Err(error) => {
                                log::warn!(
                                    "Copy identity unavailable for {}: {error}",
                                    dest.display()
                                );
                                None
                            }
                        }
                    } else {
                        None
                    };
                    let mut journal = operation.lock().unwrap();
                    if *replace_existing {
                        journal.overwritten_files.push(dest.clone());
                    } else {
                        journal.created_files.push(dest.clone());
                    }
                    journal.copied_files.push((src.clone(), dest.clone()));
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
                            moves.lock().unwrap().record(src.clone(), source, output);
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
                            .push((src.clone(), dest.clone()));
                    }

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

        if let Some(deferred_moves) = deferred_moves {
            deferred_moves.lock().unwrap().retain_directories(source_directories, destination_tree);
        } else if is_move {
            source_directories.remove_empty(
                &mut || {
                    if cancel_flag.load(Ordering::Relaxed) {
                        Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "Operation cancelled"))
                    } else {
                        Ok(())
                    }
                },
                &mut |directory, destination| {
                    op.lock().unwrap().moved_dirs.push((directory.to_path_buf(), destination.to_path_buf()));
                },
            ).map_err(|error| error.to_string())?;
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

    fn copy_worker_count(file_count: usize, total_bytes: u64, core_count: usize) -> usize {
        if file_count < 8 || total_bytes / file_count as u64 >= 32 * 1024 * 1024 {
            1
        } else {
            core_count.clamp(2, 4)
        }
    }

    fn perform_delete(op: &Arc<Mutex<FileOperation>>, sources: Vec<PathBuf>) -> Result<(), String> {
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

                    let res = super::path_parents::remove(src, &mut || {
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

        // A small deletion can finish before the first periodic progress update.
        op.lock().unwrap().processed_files = processed_files_atomic.load(Ordering::Relaxed);

        // Check if any error was captured
        if let Some(e) = error_captured.lock().unwrap().clone() {
            return Err(e);
        }

        Ok(())
    }

    fn perform_trash(op: &Arc<Mutex<FileOperation>>, sources: Vec<PathBuf>) -> Result<(), String> {
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
            platform().shell_delete(std::slice::from_ref(source), true)?;
            op.lock().unwrap().processed_files += 1;
            if !turbo_flag.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
        }

        Ok(())
    }

    pub(crate) fn archive_checkpoint(op: &Arc<Mutex<FileOperation>>) -> Result<(), String> {
        let (cancel, pause, turbo) = {
            let locked = op.lock().unwrap();
            (
                locked.cancel_flag.clone(),
                locked.pause_flag.clone(),
                locked.turbo_flag.clone(),
            )
        };
        while pause.load(Ordering::Relaxed) {
            if cancel.load(Ordering::Relaxed) {
                return Err("Operation cancelled".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        if cancel.load(Ordering::Relaxed) {
            return Err("Operation cancelled".into());
        }
        platform().set_thread_priority(if turbo.load(Ordering::Relaxed) {
            ThreadPriority::Normal
        } else {
            ThreadPriority::Background
        });
        Ok(())
    }

    fn archive_progress(op: &Arc<Mutex<FileOperation>>, bytes: u64) {
        let mut locked = op.lock().unwrap();
        locked.processed_bytes += bytes;
        let now = std::time::Instant::now();
        let previous = locked
            .speed_sample_at
            .unwrap_or_else(|| locked.started_at.unwrap_or(now));
        let elapsed = now.duration_since(previous);
        if elapsed >= std::time::Duration::from_millis(500) {
            let bytes = locked
                .processed_bytes
                .saturating_sub(locked.speed_sample_bytes);
            let current_speed = (bytes as f64 / elapsed.as_secs_f64()) as u64;
            locked.bytes_per_second = current_speed;
            locked.record_speed(current_speed);
            locked.speed_sample_bytes = locked.processed_bytes;
            locked.speed_sample_at = Some(now);
        } else if locked.speed_sample_at.is_none() {
            locked.speed_sample_at = Some(previous);
        }
    }

    fn unique_archive_path(path: &Path) -> PathBuf {
        if !path.exists() {
            return path.to_path_buf();
        }
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
        let (stem, extension) = if let Some(name) = file_name.strip_suffix(".tar.zst") {
            (name.to_string(), Some("tar.zst".to_string()))
        } else {
            (
                stem.into_owned(),
                path.extension().map(|s| s.to_string_lossy().into_owned()),
            )
        };
        for index in 1.. {
            let name = match &extension {
                Some(ext) => format!("{stem} ({index}).{ext}"),
                None => format!("{stem} ({index})"),
            };
            let candidate = parent.join(name);
            if !candidate.exists() {
                return candidate;
            }
        }
        unreachable!()
    }

    pub(crate) fn perform_compress(
        op: &Arc<Mutex<FileOperation>>,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
    ) -> Result<(), String> {
        let requested = destination.ok_or("Missing archive destination")?;
        let update_destination = matches!(op.lock().unwrap().op_type, FileOpType::Compress);
        let target = Self::unique_archive_path(&requested);
        let parent = target.parent().ok_or("Invalid archive destination")?;
        let mut entries = Vec::new();
        let mut total_bytes = 0u64;
        for source in &sources {
            let base = source.file_name().ok_or("Invalid source name")?;
            for item in walkdir::WalkDir::new(source).follow_links(false) {
                Self::archive_checkpoint(op)?;
                let item = item.map_err(|error| error.to_string())?;
                let metadata = item.metadata().map_err(|error| error.to_string())?;
                if item.file_type().is_symlink() {
                    return Err(format!(
                        "Symbolic links cannot be compressed: {}",
                        item.path().display()
                    ));
                }
                let relative = item
                    .path()
                    .strip_prefix(source)
                    .map_err(|error| error.to_string())?;
                // Joining an empty relative path to a file name can leave a trailing
                // separator on Windows. ZIP readers then interpret the file as a directory.
                let archive_path = if relative.as_os_str().is_empty() {
                    PathBuf::from(base)
                } else {
                    Path::new(base).join(relative)
                };
                let name = archive_path.to_string_lossy().replace('\\', "/");
                if metadata.is_file() {
                    total_bytes += metadata.len();
                }
                entries.push((item.path().to_path_buf(), name, metadata.is_dir()));
            }
        }
        {
            let mut locked = op.lock().unwrap();
            if update_destination {
                locked.destination = Some(target.clone());
            }
            locked.total_files = entries.len();
            locked.total_bytes = total_bytes;
            locked.status = if locked.pause_flag.load(Ordering::Relaxed) {
                OpStatus::Paused
            } else {
                OpStatus::Running
            };
        }
        let temp = parent.join(format!(".oxyde-{}.archive.tmp", Uuid::new_v4()));
        let guard = PendingFile(temp.clone());
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|error| error.to_string())?;
        let format = requested
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match format.as_str() {
            "zip" => {
                let mut writer = zip::ZipWriter::new(std::io::BufWriter::new(file));
                let mut buffer = [0u8; 256 * 1024];
                for (path, name, is_dir) in entries {
                    Self::archive_checkpoint(op)?;
                    op.lock().unwrap().current_file = Some(path.to_string_lossy().into_owned());
                    let level = if op.lock().unwrap().turbo_flag.load(Ordering::Relaxed) {
                        1
                    } else {
                        match op.lock().unwrap().compression_quality.as_str() {
                            "fast" => 1,
                            "best" => 9,
                            _ => 6,
                        }
                    };
                    let options = zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated)
                        .compression_level(Some(level));
                    if is_dir {
                        writer
                            .add_directory(format!("{name}/"), options)
                            .map_err(|error| error.to_string())?;
                    } else {
                        writer
                            .start_file(name, options)
                            .map_err(|error| error.to_string())?;
                        let mut input =
                            std::fs::File::open(&path).map_err(|error| error.to_string())?;
                        loop {
                            Self::archive_checkpoint(op)?;
                            let count =
                                input.read(&mut buffer).map_err(|error| error.to_string())?;
                            if count == 0 {
                                break;
                            }
                            writer
                                .write_all(&buffer[..count])
                                .map_err(|error| error.to_string())?;
                            Self::archive_progress(op, count as u64);
                        }
                    }
                    op.lock().unwrap().processed_files += 1;
                }
                writer.finish().map_err(|error| error.to_string())?;
            }
            "7z" => {
                let quality = op.lock().unwrap().compression_quality.clone();
                let preset = match quality.as_str() {
                    "fast" => 1,
                    "best" => 9,
                    _ => 6,
                };
                let mut writer =
                    sevenz_rust2::ArchiveWriter::new(file).map_err(|e| e.to_string())?;
                writer.set_content_methods(vec![
                    sevenz_rust2::encoder_options::Lzma2Options::from_level(preset).into(),
                ]);
                for (path, name, is_dir) in entries {
                    Self::archive_checkpoint(op)?;
                    op.lock().unwrap().current_file = Some(path.to_string_lossy().into_owned());
                    let entry = sevenz_rust2::ArchiveEntry::from_path(&path, name);
                    if is_dir {
                        writer
                            .push_archive_entry(entry, None::<ArchiveReader>)
                            .map_err(|e| e.to_string())?;
                    } else {
                        let reader = ArchiveReader {
                            input: std::fs::File::open(&path).map_err(|e| e.to_string())?,
                            operation: op.clone(),
                        };
                        writer
                            .push_archive_entry(entry, Some(reader))
                            .map_err(|e| e.to_string())?;
                    }
                    op.lock().unwrap().processed_files += 1;
                }
                writer.finish().map_err(|e| e.to_string())?;
            }
            "tar" | "zst" => {
                let quality = op.lock().unwrap().compression_quality.clone();
                let level = match quality.as_str() {
                    "fast" => 1,
                    "best" => 19,
                    _ => 3,
                };
                // Zstandard wraps the TAR stream, preserving directories and multiple selections.
                if format == "zst" {
                    Self::write_tar_entries(
                        op,
                        entries,
                        zstd::stream::write::Encoder::new(file, level)
                            .map_err(|e| e.to_string())?,
                    )?;
                } else {
                    let mut tar_bytes = tar::Builder::new(std::io::BufWriter::new(file));
                    Self::append_tar_entries(op, entries, &mut tar_bytes)?;
                    tar_bytes
                        .into_inner()
                        .map_err(|e| e.to_string())?
                        .flush()
                        .map_err(|e| e.to_string())?;
                }
            }
            _ => return Err("Unsupported compression format".into()),
        }
        Self::archive_checkpoint(op)?;
        let mut final_target = target;
        loop {
            match std::fs::hard_link(&temp, &final_target) {
                Ok(()) => break,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    final_target = Self::unique_archive_path(&final_target);
                }
                Err(_) => {
                    // Filesystems without hard links still get an exclusive publish.
                    let output = std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&final_target);
                    match output {
                        Ok(mut output) => {
                            let mut partial = PendingFile(final_target.clone());
                            let mut input =
                                std::fs::File::open(&temp).map_err(|error| error.to_string())?;
                            let mut buffer = [0u8; 256 * 1024];
                            loop {
                                Self::archive_checkpoint(op)?;
                                let count =
                                    input.read(&mut buffer).map_err(|error| error.to_string())?;
                                if count == 0 {
                                    break;
                                }
                                output
                                    .write_all(&buffer[..count])
                                    .map_err(|error| error.to_string())?;
                            }
                            output.sync_all().map_err(|error| error.to_string())?;
                            partial.0 = PathBuf::new();
                            break;
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                            final_target = Self::unique_archive_path(&final_target);
                        }
                        Err(error) => return Err(error.to_string()),
                    }
                }
            }
        }
        if update_destination {
            op.lock().unwrap().destination = Some(final_target);
        }
        drop(guard);
        Ok(())
    }

    fn append_tar_entries<W: Write>(
        op: &Arc<Mutex<FileOperation>>,
        entries: Vec<(PathBuf, String, bool)>,
        writer: &mut tar::Builder<W>,
    ) -> Result<(), String> {
        for (path, name, is_dir) in entries {
            Self::archive_checkpoint(op)?;
            op.lock().unwrap().current_file = Some(path.to_string_lossy().into_owned());
            let mut header = tar::Header::new_gnu();
            header.set_metadata(&std::fs::metadata(&path).map_err(|e| e.to_string())?);
            if is_dir {
                writer
                    .append_data(&mut header, name, std::io::empty())
                    .map_err(|e| e.to_string())?;
            } else {
                writer
                    .append_data(
                        &mut header,
                        name,
                        ArchiveReader {
                            input: std::fs::File::open(&path).map_err(|e| e.to_string())?,
                            operation: op.clone(),
                        },
                    )
                    .map_err(|e| e.to_string())?;
            }
            op.lock().unwrap().processed_files += 1;
        }
        writer.finish().map_err(|e| e.to_string())
    }

    fn write_tar_entries<W: Write>(
        op: &Arc<Mutex<FileOperation>>,
        entries: Vec<(PathBuf, String, bool)>,
        encoder: zstd::stream::write::Encoder<'_, W>,
    ) -> Result<(), String> {
        let mut writer = tar::Builder::new(encoder);
        Self::append_tar_entries(op, entries, &mut writer)?;
        writer
            .into_inner()
            .map_err(|e| e.to_string())?
            .finish()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn perform_extract(
        op: &Arc<Mutex<FileOperation>>,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
    ) -> Result<(), String> {
        Self::perform_extract_with_limits(op, sources, destination, ArchiveLimits::EXTRACTION)
    }

    fn perform_extract_with_limits(
        op: &Arc<Mutex<FileOperation>>,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
        limits: ArchiveLimits,
    ) -> Result<(), String> {
        let budget = ArchiveBudget::new(limits);
        let checkpoint = || Self::archive_checkpoint(op).map_err(std::io::Error::other);
        let source = sources.first().ok_or("Missing archive")?;
        let requested = destination.ok_or("Missing extraction destination")?;
        let mut target = Self::unique_archive_path(&requested);
        loop {
            match std::fs::create_dir(&target) {
                Ok(()) => break,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    target = Self::unique_archive_path(&target);
                }
                Err(error) => return Err(error.to_string()),
            }
        }
        struct PendingDirectory(PathBuf);
        impl Drop for PendingDirectory {
            fn drop(&mut self) {
                if !self.0.as_os_str().is_empty() {
                    let _ = std::fs::remove_dir_all(&self.0);
                }
            }
        }
        let mut guard = PendingDirectory(target.clone());
        let extraction_root = crate::utils::archive_security::ExtractionRoot::new(&target)
            .map_err(|error| error.to_string())?;
        let provenance = platform()
            .read_file_provenance(source)
            .map_err(|error| error.to_string())?;
        {
            let mut locked = op.lock().unwrap();
            locked.destination = Some(target.clone());
            locked.status = if locked.pause_flag.load(Ordering::Relaxed) {
                OpStatus::Paused
            } else {
                OpStatus::Running
            };
        }
        if source
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
        {
            let file = std::fs::File::open(source).map_err(|error| error.to_string())?;
            let mut archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
            budget
                .check_entry_count(archive.len())
                .map_err(|error| error.to_string())?;
            let mut total_bytes = 0u64;
            for index in 0..archive.len() {
                let entry = archive.by_index(index).map_err(|error| error.to_string())?;
                budget
                    .checkpoint(&checkpoint)
                    .map_err(|error| error.to_string())?;
                budget
                    .entry(entry.name(), if entry.is_dir() { 0 } else { entry.size() })
                    .map_err(|error| error.to_string())?;
                total_bytes = total_bytes
                    .checked_add(entry.size())
                    .ok_or("Archive size overflow")?;
            }
            {
                let mut locked = op.lock().unwrap();
                locked.total_files = archive.len();
                locked.total_bytes = total_bytes;
            }
            let mut buffer = [0u8; 256 * 1024];
            for index in 0..archive.len() {
                budget
                    .checkpoint(&checkpoint)
                    .map_err(|error| error.to_string())?;
                let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
                if entry
                    .unix_mode()
                    .is_some_and(|mode| mode & 0o170000 == 0o120000)
                {
                    return Err("Archive contains a symbolic link".into());
                }
                let destination = extraction_root
                    .destination(entry.name(), entry.is_dir())
                    .map_err(|error| error.to_string())?;
                let output = &destination.path;
                op.lock().unwrap().current_file = Some(output.to_string_lossy().into_owned());
                if !entry.is_dir() {
                    let mut file = platform()
                        .create_archive_file(output)
                        .map_err(|error| error.to_string())?;
                    loop {
                        budget
                            .checkpoint(&checkpoint)
                            .map_err(|error| error.to_string())?;
                        let count = entry.read(&mut buffer).map_err(|error| error.to_string())?;
                        budget.check_time().map_err(|error| error.to_string())?;
                        if count == 0 {
                            break;
                        }
                        budget
                            .charge_written(count)
                            .map_err(|error| error.to_string())?;
                        file.write_all(&buffer[..count])
                            .map_err(|error| error.to_string())?;
                        Self::archive_progress(op, count as u64);
                    }
                    if let Some(data) = provenance.as_deref() {
                        platform()
                            .write_file_provenance(output, data)
                            .map_err(|error| error.to_string())?;
                    }
                }
                op.lock().unwrap().processed_files += 1;
            }
        } else {
            Self::archive_checkpoint(op)?;
            crate::commands::archive::extract_archive_with_limits(
                source.to_string_lossy().into_owned(),
                target.to_string_lossy().into_owned(),
                &checkpoint,
                limits,
            )
            .map_err(|error| error.to_string())?;
            let mut locked = op.lock().unwrap();
            locked.total_files = 1;
            locked.processed_files = 1;
        }
        guard.0 = PathBuf::new();
        Ok(())
    }

    pub fn set_turbo(&self, id: &str, enabled: bool) {
        let ops = self.operations.lock().unwrap();
        if let Some(op_arc) = ops.get(id) {
            let mut locked = op_arc.lock().unwrap();
            if !Self::is_final(&locked.status) {
                locked.turbo = enabled;
                locked.turbo_flag.store(enabled, Ordering::Relaxed);
            }
        }
    }

    fn is_final(status: &OpStatus) -> bool {
        matches!(
            status,
            OpStatus::Completed | OpStatus::Cancelled | OpStatus::Error(_)
        )
    }

    pub fn list_operations(&self) -> Vec<FileOperation> {
        let ops = self.operations.lock().unwrap();
        let mut snapshots: Vec<_> = ops
            .values()
            .map(|op| op.lock().unwrap().snapshot())
            .collect();
        snapshots.sort_by_key(|op| op.sequence);
        snapshots
    }

    pub fn dismiss_operation(&self, id: &str) -> bool {
        let mut ops = self.operations.lock().unwrap();
        if ops
            .get(id)
            .is_some_and(|op| Self::is_final(&op.lock().unwrap().status))
        {
            ops.remove(id);
            return true;
        }
        false
    }

    /// Removes all finished operations while preserving active ones.
    pub fn dismiss_finished_operations(&self) -> usize {
        let mut ops = self.operations.lock().unwrap();
        let before = ops.len();
        ops.retain(|_, op| !Self::is_final(&op.lock().unwrap().status));
        before - ops.len()
    }

    pub fn set_paused(&self, id: &str, paused: bool) -> bool {
        let ops = self.operations.lock().unwrap();
        if let Some(op) = ops.get(id) {
            let mut locked = op.lock().unwrap();
            if Self::is_final(&locked.status) {
                return false;
            }
            if paused {
                locked.pause_flag.store(true, Ordering::Relaxed);
                locked.resume_requested = false;
                locked.status = OpStatus::Paused;
            } else if locked.pause_flag.load(Ordering::Relaxed)
                || matches!(locked.status, OpStatus::Queued)
            {
                // A launch request is prioritized; only the scheduler can mark it running.
                locked.resume_requested = true;
                locked.status = OpStatus::Queued;
            }
            self.scheduler.changed.notify_waiters();
            return true;
        }
        false
    }

    pub fn get_operation(&self, id: &str) -> Option<FileOperation> {
        let ops = self.operations.lock().unwrap();
        ops.get(id).map(|op| op.lock().unwrap().snapshot())
    }

    pub fn cancel_operation(&self, id: &str) -> bool {
        let ops = self.operations.lock().unwrap();
        if let Some(op) = ops.get(id) {
            let mut locked_op = op.lock().unwrap();
            if Self::is_final(&locked_op.status) {
                return false;
            }
            locked_op.cancel_flag.store(true, Ordering::Relaxed);
            if !locked_op.execution_started {
                locked_op.status = OpStatus::Cancelled;
            }
            self.scheduler.changed.notify_waiters();
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn extraction_failure_cleans_output_for_all_formats() {
        use super::*;
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("file.txt");
        std::fs::write(&source, b"bounded test content").unwrap();
        for extension in ["zip", "7z", "tar", "zst", "iso"] {
            let archive = temp.path().join(format!("limited.{extension}"));
            if extension == "iso" {
                crate::commands::archive::test_iso_image(&archive);
            } else {
                FileOperationManager::perform_compress(
                    &crate::systems::archive_workspace::mutation_operation("fast"),
                    vec![source.clone()],
                    Some(archive.clone()),
                )
                .unwrap();
            }
            for cancelled in [false, true] {
                let output = temp.path().join(format!("output-{extension}-{cancelled}"));
                let op = Arc::new(Mutex::new(FileOperation::new(
                    FileOpType::Extract,
                    vec![archive.clone()],
                    Some(output.clone()),
                    None,
                )));
                op.lock()
                    .unwrap()
                    .cancel_flag
                    .store(cancelled, Ordering::Relaxed);
                let limits = ArchiveLimits {
                    max_bytes: 2,
                    ..ArchiveLimits::EXTRACTION
                };
                assert!(FileOperationManager::perform_extract_with_limits(
                    &op,
                    vec![archive.clone()],
                    Some(output.clone()),
                    limits
                )
                .is_err());
                assert!(!output.exists(), "partial output kept for {extension}");
                assert!(archive.is_file());
            }
        }
    }

    #[test]
    fn zip_operations_reject_windows_escape_and_alternate_stream_names() {
        use super::*;
        for name in ["a/C:victim.txt", "a/../C:victim.txt", "file.txt:extra"] {
            let temp = tempfile::tempdir().unwrap();
            let archive = temp.path().join("unsafe.zip");
            let mut writer = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"marker").unwrap();
            writer.finish().unwrap();
            let output = temp.path().join("output");
            let op = Arc::new(Mutex::new(FileOperation::new(
                FileOpType::Extract,
                vec![archive.clone()],
                Some(output.clone()),
                None,
            )));
            assert!(FileOperationManager::perform_extract(
                &op,
                vec![archive],
                Some(output.clone())
            )
            .is_err());
            assert!(!output.exists());
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn zip_operations_mark_outputs_before_publication() {
        use super::*;
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("marked.zip");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
        writer
            .start_file("file.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"marker").unwrap();
        writer.finish().unwrap();
        let data = b"[ZoneTransfer]\r\nZoneId=3\r\n";
        platform().write_file_provenance(&archive, data).unwrap();
        let output = temp.path().join("output");
        let op = Arc::new(Mutex::new(FileOperation::new(
            FileOpType::Extract,
            vec![archive.clone()],
            Some(output.clone()),
            None,
        )));
        FileOperationManager::perform_extract(&op, vec![archive], Some(output.clone())).unwrap();
        assert_eq!(
            platform()
                .read_file_provenance(&output.join("file.txt"))
                .unwrap()
                .as_deref(),
            Some(data.as_slice())
        );
    }
    use super::*;

    #[test]
    fn speed_graph_history_is_bounded_and_present_in_snapshots() {
        let mut operation = FileOperation::new(FileOpType::Copy, Vec::new(), None, None);
        for speed in 0..75 {
            operation.record_speed(speed);
        }
        let snapshot = operation.snapshot();
        assert_eq!(snapshot.speed_history.len(), SPEED_HISTORY_LIMIT);
        assert_eq!(snapshot.speed_history.first(), Some(&15));
        assert_eq!(snapshot.speed_history.last(), Some(&74));
    }

    #[test]
    fn copy_and_archive_progress_feed_speed_graph() {
        let copy = Arc::new(Mutex::new(FileOperation::new(
            FileOpType::Copy,
            Vec::new(),
            None,
            None,
        )));
        let bytes = Arc::new(AtomicU64::new(2048));
        let files = Arc::new(AtomicUsize::new(1));
        let mut last_bytes = 0;
        let mut last_emit = std::time::Instant::now() - std::time::Duration::from_millis(600);
        let mut samples = std::collections::VecDeque::new();
        FileOperationManager::emit_progress(
            &copy,
            &bytes,
            &files,
            &mut last_bytes,
            &mut last_emit,
            &mut samples,
        );
        assert!(copy.lock().unwrap().speed_history[0] > 0);

        let archive = Arc::new(Mutex::new(FileOperation::new(
            FileOpType::Compress,
            Vec::new(),
            None,
            None,
        )));
        archive.lock().unwrap().speed_sample_at =
            Some(std::time::Instant::now() - std::time::Duration::from_millis(600));
        FileOperationManager::archive_progress(&archive, 2048);
        assert!(archive.lock().unwrap().speed_history[0] > 0);
    }

    #[test]
    fn copy_uses_parallel_workers_only_for_many_small_files() {
        assert_eq!(FileOperationManager::copy_worker_count(0, 0, 8), 1);
        assert_eq!(
            FileOperationManager::copy_worker_count(3, 3 * 1024 * 1024, 8),
            1
        );
        assert_eq!(
            FileOperationManager::copy_worker_count(8, 8 * 1024 * 1024, 8),
            4
        );
        assert_eq!(
            FileOperationManager::copy_worker_count(8, 8 * 64 * 1024 * 1024, 8),
            1
        );
    }

    #[test]
    fn copy_from_iso_preserves_image_and_extracts_selected_entry() {
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("disc.iso");
        crate::commands::archive::test_iso_image(&image);
        let original = std::fs::read(&image).unwrap();
        let target = temp.path().join("output");
        std::fs::create_dir(&target).unwrap();
        let source = image.join("DOCS/README.TXT");
        let operation = Arc::new(Mutex::new(FileOperation::new(
            FileOpType::Copy,
            vec![source.clone()],
            Some(target.clone()),
            None,
        )));
        FileOperationManager::perform_copy(
            &operation,
            vec![source],
            Some(target.clone()),
            false,
            None,
        )
        .unwrap();
        assert_eq!(
            std::fs::read(target.join("README.TXT")).unwrap(),
            b"ISO content"
        );
        assert_eq!(std::fs::read(image).unwrap(), original);
    }

    #[test]
    fn archive_roundtrip_tracks_progress_and_preserves_existing_destinations() {
        for format in ["zip", "7z", "tar", "zst"] {
            for quality in ["fast", "normal", "best"] {
                let temp = tempfile::tempdir().unwrap();
                let source = temp.path().join("folder");
                std::fs::create_dir(&source).unwrap();
                std::fs::create_dir(source.join("empty")).unwrap();
                std::fs::write(source.join("hello.txt"), b"archive roundtrip").unwrap();
                let archive_path = temp.path().join(format!("folder.{format}"));
                std::fs::write(&archive_path, b"existing archive").unwrap();
                let compress = Arc::new(Mutex::new(FileOperation::new(
                    FileOpType::Compress,
                    vec![source.clone()],
                    Some(archive_path.clone()),
                    None,
                )));
                compress.lock().unwrap().compression_quality = quality.into();
                FileOperationManager::perform_compress(
                    &compress,
                    vec![source],
                    Some(archive_path.clone()),
                )
                .unwrap();
                let compressed = compress.lock().unwrap().snapshot();
                let produced = compressed.destination.unwrap();
                assert_ne!(produced, archive_path);
                assert_eq!(std::fs::read(&archive_path).unwrap(), b"existing archive");
                assert_eq!(compressed.processed_files, compressed.total_files);
                assert_eq!(compressed.processed_bytes, compressed.total_bytes);

                let extraction = Arc::new(Mutex::new(FileOperation::new(
                    FileOpType::Extract,
                    vec![produced.clone()],
                    Some(temp.path().join("folder")),
                    None,
                )));
                FileOperationManager::perform_extract(
                    &extraction,
                    vec![produced],
                    Some(temp.path().join("folder")),
                )
                .unwrap();
                let extracted = extraction.lock().unwrap().snapshot();
                assert_eq!(
                    std::fs::read(
                        extracted
                            .destination
                            .as_ref()
                            .unwrap()
                            .join("folder/hello.txt")
                    )
                    .unwrap(),
                    b"archive roundtrip"
                );
                assert!(extracted
                    .destination
                    .as_ref()
                    .unwrap()
                    .join("folder/empty")
                    .is_dir());
                assert_eq!(extracted.processed_bytes, extracted.total_bytes);
            }
        }
    }

    #[test]
    fn removing_archive_items_keeps_other_entries_and_cleans_temporary_files() {
        use crate::systems::archive_workspace::{mutation_operation, ArchiveWorkspace};

        for extension in ["zip", "7z", "tar", "tar.zst"] {
            let directory = tempfile::tempdir().unwrap();
            let sources = tempfile::tempdir().unwrap();
            std::fs::write(sources.path().join("keep.txt"), b"keep").unwrap();
            std::fs::write(sources.path().join("remove.txt"), b"remove").unwrap();
            std::fs::create_dir_all(sources.path().join("remove/nested")).unwrap();
            std::fs::write(sources.path().join("remove/nested/file.txt"), b"remove").unwrap();
            std::fs::create_dir(sources.path().join("remove-neighbor")).unwrap();
            std::fs::write(sources.path().join("remove-neighbor/file.txt"), b"neighbor").unwrap();
            let archive = directory.path().join(format!("sample.{extension}"));
            FileOperationManager::perform_compress(
                &mutation_operation("normal"),
                vec![
                    sources.path().join("keep.txt"),
                    sources.path().join("remove.txt"),
                    sources.path().join("remove"),
                    sources.path().join("remove-neighbor"),
                ],
                Some(archive.clone()),
            )
            .unwrap();
            let operation = Arc::new(Mutex::new(FileOperation::new(
                FileOpType::Delete,
                vec![],
                None,
                None,
            )));
            FileOperationManager::perform_delete(
                &operation,
                vec![archive.join("remove.txt"), archive.join("remove")],
            )
            .unwrap();

            let workspace = ArchiveWorkspace::open(archive).unwrap();
            assert_eq!(
                std::fs::read(workspace.root().join("keep.txt")).unwrap(),
                b"keep"
            );
            assert_eq!(
                std::fs::read(workspace.root().join("remove-neighbor/file.txt")).unwrap(),
                b"neighbor"
            );
            assert!(!workspace.root().join("remove.txt").exists());
            assert!(!workspace.root().join("remove").exists());
            assert_eq!(std::fs::read_dir(workspace.root()).unwrap().count(), 2);
            drop(workspace);
            let remaining = std::fs::read_dir(directory.path())
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect::<Vec<_>>();
            assert_eq!(remaining.len(), 1, "{extension}: residual files {remaining:?}");
        }
    }

    #[test]
    fn compression_does_not_overwrite_an_existing_archive() {
        use crate::systems::archive_workspace::{mutation_operation, ArchiveWorkspace};

        for extension in ["zip", "7z", "tar", "tar.zst"] {
            let directory = tempfile::tempdir().unwrap();
            let sources = tempfile::tempdir().unwrap();
            let source = sources.path().join("source.txt");
            std::fs::write(&source, b"new payload").unwrap();
            let requested = directory.path().join(format!("existing.{extension}"));
            std::fs::write(&requested, b"original").unwrap();
            let operation = mutation_operation("fast");
            FileOperationManager::perform_compress(
                &operation,
                vec![source],
                Some(requested.clone()),
            )
            .unwrap();

            assert_eq!(std::fs::read(&requested).unwrap(), b"original");
            let published = operation.lock().unwrap().destination.clone().unwrap();
            assert_eq!(
                published,
                directory.path().join(format!("existing (1).{extension}"))
            );
            let workspace = ArchiveWorkspace::open(published).unwrap();
            assert_eq!(
                std::fs::read(workspace.root().join("source.txt")).unwrap(),
                b"new payload"
            );
            drop(workspace);
            assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
        }
    }

    #[test]
    fn archive_file_operations_work_inside_and_between_all_created_formats() {
        for extension in ["zip", "7z", "tar", "tar.zst", "7Z"] {
            let directory = tempfile::tempdir().unwrap();
            let file = directory.path().join("original.txt");
            std::fs::write(&file, b"original").unwrap();
            let archive = directory.path().join(format!("editable.{extension}"));
            let make_op = || {
                Arc::new(Mutex::new(FileOperation::new(
                    FileOpType::Copy,
                    vec![],
                    None,
                    None,
                )))
            };
            FileOperationManager::perform_compress(
                &make_op(),
                vec![file.clone()],
                Some(archive.clone()),
            )
            .unwrap();
            let folder = archive.join("nested");
            crate::commands::archive::create_archive_directory_with_quality(&folder, "normal")
                .unwrap();
            let added = directory.path().join("added.txt");
            std::fs::write(&added, b"added").unwrap();
            FileOperationManager::perform_copy(
                &make_op(),
                vec![added.clone()],
                Some(folder.clone()),
                false,
                None,
            )
            .unwrap();
            assert!(added.exists());
            let output = directory.path().join("output");
            std::fs::create_dir(&output).unwrap();
            FileOperationManager::perform_copy(
                &make_op(),
                vec![folder.join("added.txt")],
                Some(output.clone()),
                false,
                None,
            )
            .unwrap();
            assert_eq!(std::fs::read(output.join("added.txt")).unwrap(), b"added");
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime
                .block_on(crate::commands::io::rename_item_controlled(
                    folder.join("added.txt").to_string_lossy().into_owned(),
                    folder.join("renamed.txt").to_string_lossy().into_owned(),
                    "normal".into(),
                    Arc::new(AtomicBool::new(false)),
                ))
                .unwrap();
            FileOperationManager::perform_copy(
                &make_op(),
                vec![folder.join("renamed.txt")],
                Some(archive.clone()),
                true,
                None,
            )
            .unwrap();
            let nested = crate::commands::archive::list_archive_contents(
                archive.to_string_lossy().into_owned(),
                "nested".into(),
            )
            .unwrap();
            assert!(nested.is_empty());
            FileOperationManager::perform_copy(
                &make_op(),
                vec![archive.join("renamed.txt")],
                Some(output.clone()),
                true,
                None,
            )
            .unwrap();
            assert_eq!(std::fs::read(output.join("renamed.txt")).unwrap(), b"added");
            let second_archive = directory.path().join(format!("second.{extension}"));
            FileOperationManager::perform_compress(
                &make_op(),
                vec![],
                Some(second_archive.clone()),
            )
            .unwrap();
            FileOperationManager::perform_copy(
                &make_op(),
                vec![archive.join("original.txt")],
                Some(second_archive.clone()),
                true,
                None,
            )
            .unwrap();
            assert!(crate::commands::archive::list_archive_contents(
                second_archive.to_string_lossy().into_owned(),
                "".into()
            )
            .unwrap()
            .iter()
            .any(|entry| entry.name == "original.txt"));
            FileOperationManager::perform_delete(&make_op(), vec![folder]).unwrap();
            assert!(crate::commands::archive::list_archive_contents(
                archive.to_string_lossy().into_owned(),
                "".into()
            )
            .unwrap()
            .is_empty());
        }
    }

    #[test]
    fn archive_move_preserves_skipped_files_and_reports_conflicts() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("keep.txt"), b"keep").unwrap();
        std::fs::write(source.join("move.txt"), b"move").unwrap();
        let archive = directory.path().join("test.7z");
        let op = Arc::new(Mutex::new(FileOperation::new(
            FileOpType::Copy,
            vec![],
            None,
            None,
        )));
        FileOperationManager::perform_compress(&op, vec![source.clone()], Some(archive.clone()))
            .unwrap();
        let target = directory.path().join("target");
        std::fs::create_dir_all(target.join("source")).unwrap();
        std::fs::write(target.join("source/keep.txt"), b"existing").unwrap();
        let virtual_folder = archive.join("source");
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let conflicts = runtime
            .block_on(crate::commands::ops::check_conflicts(
                vec![virtual_folder.to_string_lossy().into_owned()],
                target.to_string_lossy().into_owned(),
            ))
            .unwrap();
        assert!(conflicts
            .iter()
            .any(|conflict| conflict.source.path
                == virtual_folder.join("keep.txt").to_string_lossy()));
        let resolutions = HashMap::from([(
            virtual_folder
                .join("keep.txt")
                .to_string_lossy()
                .into_owned(),
            ConflictAction::Skip,
        )]);
        let op = Arc::new(Mutex::new(FileOperation::new(
            FileOpType::Move,
            vec![],
            None,
            None,
        )));
        FileOperationManager::perform_copy(
            &op,
            vec![virtual_folder],
            Some(target.clone()),
            true,
            Some(resolutions),
        )
        .unwrap();
        let remaining = crate::commands::archive::list_archive_contents(
            archive.to_string_lossy().into_owned(),
            "source".into(),
        )
        .unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].name, "keep.txt");
        assert_eq!(
            std::fs::read(target.join("source/keep.txt")).unwrap(),
            b"existing"
        );
        assert_eq!(
            std::fs::read(target.join("source/move.txt")).unwrap(),
            b"move"
        );
        let resolutions = HashMap::from([(
            target
                .join("source/keep.txt")
                .to_string_lossy()
                .into_owned(),
            ConflictAction::Replace,
        )]);
        FileOperationManager::perform_copy(
            &Arc::new(Mutex::new(FileOperation::new(
                FileOpType::Copy,
                vec![],
                None,
                None,
            ))),
            vec![target.join("source")],
            Some(archive.clone()),
            false,
            Some(resolutions),
        )
        .unwrap();
        let files = crate::commands::archive::list_archive_contents(
            archive.to_string_lossy().into_owned(),
            "source".into(),
        )
        .unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn unsafe_zip_entry_is_rejected_and_partial_output_removed() {
        let temp = tempfile::tempdir().unwrap();
        let archive_path = temp.path().join("unsafe.zip");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&archive_path).unwrap());
        writer
            .start_file("../escape.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"escape").unwrap();
        writer.finish().unwrap();
        let output = temp.path().join("unpacked");
        let extraction = Arc::new(Mutex::new(FileOperation::new(
            FileOpType::Extract,
            vec![archive_path.clone()],
            Some(output.clone()),
            None,
        )));
        assert!(FileOperationManager::perform_extract(
            &extraction,
            vec![archive_path],
            Some(output.clone())
        )
        .is_err());
        assert!(!output.exists());
        assert!(!temp.path().join("escape.txt").exists());
    }

    #[test]
    fn overlapping_paths_are_serialized_without_blocking_independent_paths() {
        let temp = tempfile::tempdir().unwrap();
        let destination = temp.path().join("target");
        let first = FileOperation::new(
            FileOpType::Copy,
            vec![temp.path().join("source-a")],
            Some(destination.clone()),
            None,
        );
        let same_destination = FileOperation::new(
            FileOpType::Move,
            vec![temp.path().join("source-b")],
            Some(destination.join("source-a/nested")),
            None,
        );
        let independent = FileOperation::new(
            FileOpType::Copy,
            vec![temp.path().join("source-c")],
            Some(destination),
            None,
        );
        assert!(FileOperationManager::paths_overlap(
            &FileOperationManager::reserved_paths(&first),
            &FileOperationManager::reserved_paths(&same_destination)
        ));
        assert!(!FileOperationManager::paths_overlap(
            &FileOperationManager::reserved_paths(&first),
            &FileOperationManager::reserved_paths(&independent)
        ));
    }

    #[test]
    fn controls_and_dismissal_target_only_one_operation() {
        let manager = FileOperationManager::new();
        let mut first = FileOperation::new(FileOpType::Copy, Vec::new(), None, None);
        let mut second = FileOperation::new(FileOpType::Move, Vec::new(), None, None);
        first.sequence = 1;
        second.sequence = 2;
        let first_id = first.id.clone();
        let second_id = second.id.clone();
        {
            let mut ops = manager.operations.lock().unwrap();
            ops.insert(first_id.clone(), Arc::new(Mutex::new(first)));
            ops.insert(second_id.clone(), Arc::new(Mutex::new(second)));
        }
        assert!(manager.set_paused(&first_id, true));
        manager.set_turbo(&first_id, true);
        assert_eq!(
            manager.get_operation(&first_id).unwrap().status,
            OpStatus::Paused
        );
        assert!(manager.get_operation(&first_id).unwrap().turbo);
        assert_eq!(
            manager.get_operation(&second_id).unwrap().status,
            OpStatus::Queued
        );
        assert!(!manager.get_operation(&second_id).unwrap().turbo);
        assert!(manager.cancel_operation(&second_id));
        assert_eq!(
            manager.get_operation(&second_id).unwrap().status,
            OpStatus::Cancelled
        );
        assert!(manager.dismiss_operation(&second_id));
        assert_eq!(manager.list_operations().len(), 1);
        assert!(!manager.dismiss_operation(&first_id));
    }

    #[test]
    fn dismiss_finished_operations_preserves_running_operations() {
        let manager = FileOperationManager::new();
        let mut completed = FileOperation::new(FileOpType::Copy, Vec::new(), None, None);
        let mut running = FileOperation::new(FileOpType::Move, Vec::new(), None, None);
        completed.status = OpStatus::Completed;
        running.status = OpStatus::Running;
        let completed_id = completed.id.clone();
        let running_id = running.id.clone();
        {
            let mut operations = manager.operations.lock().unwrap();
            operations.insert(completed_id.clone(), Arc::new(Mutex::new(completed)));
            operations.insert(running_id.clone(), Arc::new(Mutex::new(running)));
        }

        assert_eq!(manager.dismiss_finished_operations(), 1);
        assert!(manager.get_operation(&completed_id).is_none());
        assert_eq!(
            manager.get_operation(&running_id).unwrap().status,
            OpStatus::Running
        );
    }

    #[test]
    fn destination_created_while_queued_requires_a_new_conflict_decision() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let temp = tempfile::tempdir().unwrap();
            let source = temp.path().join("item.txt");
            let target = temp.path().join("target");
            std::fs::create_dir(&target).unwrap();
            std::fs::write(&source, b"source").unwrap();
            let manager = FileOperationManager::new();
            let held_slots = manager
                .scheduler
                .slots
                .clone()
                .acquire_many_owned(2)
                .await
                .unwrap();
            let id = manager.queue_operation_on(
                &tokio::runtime::Handle::current(),
                Arc::new(HistoryManager::new()),
                FileOperation::new(
                    FileOpType::Copy,
                    vec![source.clone()],
                    Some(target.clone()),
                    None,
                ),
            );
            std::fs::write(target.join("item.txt"), b"existing").unwrap();
            drop(held_slots);
            let status = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    let status = manager.get_operation(&id).unwrap().status;
                    if FileOperationManager::is_final(&status) {
                        break status;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            assert!(matches!(status, OpStatus::Error(_)));
            assert_eq!(std::fs::read(&source).unwrap(), b"source");
            assert_eq!(std::fs::read(target.join("item.txt")).unwrap(), b"existing");
        });
    }

    #[test]
    fn paused_operations_do_not_hold_capacity_and_queued_launch_stays_queued_until_started() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let temp = tempfile::tempdir().unwrap();
            let manager = FileOperationManager::new();
            let history = Arc::new(HistoryManager::new());
            let mut ids = Vec::new();
            for index in 0..3 {
                let source_dir = temp.path().join(format!("source-{index}"));
                let target_dir = temp.path().join(format!("target-{index}"));
                std::fs::create_dir(&source_dir).unwrap();
                std::fs::create_dir(&target_dir).unwrap();
                let source = source_dir.join("item.txt");
                std::fs::write(&source, b"copy me").unwrap();
                let op = FileOperation::new(FileOpType::Copy, vec![source], Some(target_dir), None);
                op.pause_flag.store(true, Ordering::Relaxed);
                ids.push(manager.queue_operation_on(
                    &tokio::runtime::Handle::current(),
                    history.clone(),
                    op,
                ));
            }
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    let statuses: Vec<_> = ids
                        .iter()
                        .map(|id| manager.get_operation(id).unwrap().status)
                        .collect();
                    if statuses.iter().all(|status| *status == OpStatus::Paused) {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            let held_slots = manager
                .scheduler
                .slots
                .clone()
                .acquire_many_owned(2)
                .await
                .unwrap();
            assert!(manager.set_paused(&ids[2], false));
            assert_eq!(
                manager.get_operation(&ids[2]).unwrap().status,
                OpStatus::Queued
            );
            drop(held_slots);
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    if manager.get_operation(&ids[2]).unwrap().status == OpStatus::Completed {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            assert_eq!(
                manager.get_operation(&ids[0]).unwrap().status,
                OpStatus::Paused
            );
            assert_eq!(
                manager.get_operation(&ids[1]).unwrap().status,
                OpStatus::Paused
            );
            for id in &ids {
                manager.cancel_operation(id);
            }
        });
    }

    async fn wait_for_status(manager: &FileOperationManager, id: &str, expected: OpStatus) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let status = manager.get_operation(id).unwrap().status;
                if status == expected {
                    break;
                }
                assert!(
                    !matches!(status, OpStatus::Error(_)),
                    "Unexpected failure: {status:?}"
                );
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap();
    }

    #[test]
    fn pausing_an_active_copy_starts_the_next_copy_but_protects_the_paused_paths() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let temp = tempfile::tempdir().unwrap();
            let source = temp.path().join("large.bin");
            let file = std::fs::File::create(&source).unwrap();
            file.set_len(32 * 1024 * 1024).unwrap();
            drop(file);
            let other = temp.path().join("small.txt");
            std::fs::write(&other, b"next operation").unwrap();
            let target = temp.path().join("target");
            std::fs::create_dir(&target).unwrap();
            let manager = FileOperationManager::new();
            let history = Arc::new(HistoryManager::new());
            // Leave only one execution slot, so advancing the queue requires releasing it.
            let held = manager
                .scheduler
                .slots
                .clone()
                .acquire_owned()
                .await
                .unwrap();
            let first = manager.queue_operation_on(
                &tokio::runtime::Handle::current(),
                history.clone(),
                FileOperation::new(
                    FileOpType::Copy,
                    vec![source.clone()],
                    Some(target.clone()),
                    None,
                ),
            );
            wait_for_status(&manager, &first, OpStatus::Running).await;
            assert!(manager.set_paused(&first, true));
            let next = manager.queue_operation_on(
                &tokio::runtime::Handle::current(),
                history.clone(),
                FileOperation::new(FileOpType::Copy, vec![other], Some(target.clone()), None),
            );
            let overlapping = manager.queue_operation_on(
                &tokio::runtime::Handle::current(),
                history.clone(),
                FileOperation::new(FileOpType::Copy, vec![source], Some(target.clone()), None),
            );
            wait_for_status(&manager, &next, OpStatus::Completed).await;
            assert_eq!(
                std::fs::read(target.join("small.txt")).unwrap(),
                b"next operation"
            );
            assert_eq!(
                manager.get_operation(&first).unwrap().status,
                OpStatus::Paused
            );
            assert_eq!(
                manager.get_operation(&overlapping).unwrap().status,
                OpStatus::Queued
            );
            manager.cancel_operation(&overlapping);
            assert!(manager.set_paused(&first, false));
            wait_for_status(&manager, &first, OpStatus::Completed).await;
            assert_eq!(
                std::fs::metadata(target.join("large.bin")).unwrap().len(),
                32 * 1024 * 1024
            );
            drop(held);
        });
    }

    #[test]
    fn manual_launch_takes_priority_over_earlier_waiting_operations() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let manager = FileOperationManager::new();
            let mut ids = Vec::new();
            for sequence in 1..=3 {
                let mut operation = FileOperation::new(FileOpType::Copy, Vec::new(), None, None);
                operation.sequence = sequence;
                let id = operation.id.clone();
                let operation = Arc::new(Mutex::new(operation));
                manager
                    .operations
                    .lock()
                    .unwrap()
                    .insert(id.clone(), operation.clone());
                manager.scheduler.enqueue(operation, Vec::new());
                ids.push(id);
            }
            let held = manager
                .scheduler
                .slots
                .clone()
                .acquire_owned()
                .await
                .unwrap();
            assert!(manager.set_paused(&ids[2], false));
            assert_eq!(
                manager.get_operation(&ids[2]).unwrap().status,
                OpStatus::Queued
            );
            let launched = tokio::time::timeout(
                std::time::Duration::from_secs(1),
                manager.scheduler.acquire(&ids[2]),
            )
            .await
            .unwrap()
            .unwrap();
            assert_eq!(
                manager.get_operation(&ids[0]).unwrap().status,
                OpStatus::Queued
            );
            assert_eq!(
                manager.get_operation(&ids[1]).unwrap().status,
                OpStatus::Queued
            );
            drop(launched);
            drop(held);
        });
    }

    fn operation() -> Arc<Mutex<FileOperation>> {
        Arc::new(Mutex::new(FileOperation::new(
            FileOpType::Move,
            Vec::new(),
            None,
            None,
        )))
    }

    #[test]
    fn same_volume_move_preserves_file_identity_without_copying() {
        let temp = tempfile::tempdir().unwrap();
        let source_dir = temp.path().join("source");
        let target_dir = temp.path().join("target");
        std::fs::create_dir(&source_dir).unwrap();
        std::fs::create_dir(&target_dir).unwrap();
        let source = source_dir.join("file.bin");
        let contents = vec![0x5a; 2 * 1024 * 1024];
        std::fs::write(&source, &contents).unwrap();
        let alias = temp.path().join("alias.bin");
        std::fs::hard_link(&source, &alias).unwrap();
        let op = operation();

        FileOperationManager::perform_copy(
            &op,
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
        std::fs::write(target_dir.join("file.bin"), b"changed").unwrap();
        assert_eq!(std::fs::read(alias).unwrap(), b"changed");
        let locked = op.lock().unwrap();
        assert_eq!(locked.moved_files.len(), 1);
        assert!(locked.copied_files.is_empty());
        assert!(locked.copied_file_hashes.is_empty());
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn archive_move_publication_conflict_preserves_held_real_sources() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        let archive = root.path().join("target.zip");
        std::fs::write(&source, b"source data").unwrap();
        FileOperationManager::perform_compress(&operation(), vec![], Some(archive.clone())).unwrap();
        let op = operation();
        let result = FileOperationManager::perform_archive_transfer_with(
            &op, vec![source.clone()], Some(archive.clone()), true, None,
            &mut || {
                assert!(std::fs::OpenOptions::new().write(true).open(&source).is_err());
                assert!(std::fs::remove_file(&source).is_err());
                let output = op.lock().unwrap().copied_files[0].1.clone();
                assert!(std::fs::OpenOptions::new().write(true).open(&output).is_err());
                assert!(std::fs::remove_file(&output).is_err());
                std::fs::write(&archive, b"foreign archive edit").map_err(|error| error.to_string())
            },
        );
        assert!(result.is_err());
        assert_eq!(std::fs::read(&source).unwrap(), b"source data");
        assert_eq!(std::fs::read(&archive).unwrap(), b"foreign archive edit");
        assert!(std::fs::OpenOptions::new().write(true).open(&source).is_ok());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 2);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn archive_move_locks_copied_files_but_preserves_editable_skipped_conflicts() {
        use crate::systems::archive_workspace::ArchiveWorkspace;
        let root = tempfile::tempdir().unwrap();
        let seed = root.path().join("seed/source");
        let source = root.path().join("source");
        let archive = root.path().join("target.zip");
        std::fs::create_dir_all(&seed).unwrap();
        std::fs::create_dir(&source).unwrap();
        std::fs::write(seed.join("keep"), b"existing archive entry").unwrap();
        std::fs::write(source.join("keep"), b"skipped source").unwrap();
        std::fs::write(source.join("move"), b"moved source").unwrap();
        FileOperationManager::perform_compress(&operation(), vec![seed], Some(archive.clone())).unwrap();
        let resolutions = HashMap::from([(source.join("keep").to_string_lossy().into_owned(), ConflictAction::Skip)]);
        FileOperationManager::perform_archive_transfer_with(
            &operation(), vec![source.clone()], Some(archive.clone()), true, Some(resolutions),
            &mut || {
                assert!(std::fs::OpenOptions::new().write(true).open(source.join("move")).is_err());
                std::fs::write(source.join("keep"), b"edited skipped source").map_err(|error| error.to_string())
            },
        ).unwrap();
        assert_eq!(std::fs::read(source.join("keep")).unwrap(), b"edited skipped source");
        assert!(!source.join("move").exists());
        let workspace = ArchiveWorkspace::open(archive).unwrap();
        assert_eq!(std::fs::read(workspace.root().join("source/keep")).unwrap(), b"existing archive entry");
        assert_eq!(std::fs::read(workspace.root().join("source/move")).unwrap(), b"moved source");
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn archive_directory_move_preserves_a_replacement_after_copy() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let original = root.path().join("relocated");
        let archive = root.path().join("target.zip");
        std::fs::create_dir(&source).unwrap();
        FileOperationManager::perform_compress(&operation(), vec![], Some(archive.clone())).unwrap();
        let result = FileOperationManager::perform_archive_transfer_with(
            &operation(), vec![source.clone()], Some(archive), true, None,
            &mut || {
                std::fs::rename(&source, &original).map_err(|error| error.to_string())?;
                std::fs::create_dir(&source).map_err(|error| error.to_string())
            },
        );
        assert!(source.is_dir(), "archive transfer deleted a replacement directory");
        assert!(original.is_dir());
        assert!(result.unwrap_err().contains("replacement was preserved"));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn archive_directory_move_keeps_the_initial_parent_after_junction_retargeting() {
        use crate::platform::windows_reparse_tests::{junction, Retarget};
        let root = tempfile::tempdir().unwrap();
        let chosen = root.path().join("chosen");
        let outside = root.path().join("outside");
        let link = root.path().join("link");
        let template = root.path().join("template");
        let archive = root.path().join("target.zip");
        std::fs::create_dir_all(chosen.join("source")).unwrap();
        std::fs::create_dir_all(outside.join("source")).unwrap();
        std::fs::write(outside.join("witness"), b"foreign witness").unwrap();
        junction(&link, &chosen);
        junction(&template, &outside);
        let retarget = Retarget::new(&link, &template);
        FileOperationManager::perform_compress(&operation(), vec![], Some(archive.clone())).unwrap();
        FileOperationManager::perform_archive_transfer_with(
            &operation(), vec![link.join("source")], Some(archive), true, None,
            &mut || { retarget.apply(); Ok(()) },
        ).unwrap();
        assert!(outside.join("source").is_dir());
        assert!(!chosen.join("source").exists());
        assert_eq!(std::fs::read(outside.join("witness")).unwrap(), b"foreign witness");
        drop(retarget);
        std::fs::remove_dir(&link).unwrap();
        std::fs::remove_dir(&template).unwrap();
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn archive_directory_move_preserves_a_new_empty_child_absent_from_the_plan() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let archive = root.path().join("target.zip");
        std::fs::create_dir(&source).unwrap();
        FileOperationManager::perform_compress(&operation(), vec![], Some(archive.clone())).unwrap();
        FileOperationManager::perform_archive_transfer_with(
            &operation(), vec![source.clone()], Some(archive), true, None,
            &mut || std::fs::create_dir(source.join("new")).map_err(|error| error.to_string()),
        ).unwrap();
        assert!(source.join("new").is_dir());
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn archive_directory_move_rejects_a_changed_destination_before_publication() {
        use crate::platform::windows_reparse_tests::{junction, Retarget};
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let outside = root.path().join("outside");
        let template = root.path().join("template");
        let archive = root.path().join("target.zip");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&outside).unwrap();
        junction(&template, &outside);
        FileOperationManager::perform_compress(&operation(), vec![], Some(archive.clone())).unwrap();
        let original = std::fs::read(&archive).unwrap();
        let op = operation();
        let mut retarget = None;
        let result = FileOperationManager::perform_archive_transfer_with(
            &op, vec![source.clone()], Some(archive.clone()), true, None,
            &mut || {
                let destination = op.lock().unwrap().created_dirs.last().unwrap().clone();
                let changed = Retarget::new(&destination, &template);
                changed.apply();
                retarget = Some(changed);
                Ok(())
            },
        );
        // Closing the writer completes any deletion queued by workspace cleanup.
        drop(retarget.take());
        for (workspace, _) in &op.lock().unwrap().archive_path_mappings {
            if workspace.exists() {
                std::fs::remove_dir(workspace).unwrap();
            }
        }
        std::fs::remove_dir(&template).unwrap();
        assert!(result.unwrap_err().contains("destination directory"));
        assert!(source.is_dir());
        assert!(outside.is_dir());
        assert_eq!(std::fs::read(&archive).unwrap(), original);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn archive_directory_move_roundtrips_nested_empty_directories_in_all_writable_formats() {
        use crate::systems::archive_workspace::ArchiveWorkspace;
        for extension in ["zip", "7z", "tar", "tar.zst"] {
            let root = tempfile::tempdir().unwrap();
            let source = root.path().join("source");
            let output = root.path().join("output");
            let archive = root.path().join(format!("target.{extension}"));
            std::fs::create_dir_all(source.join("écriture/空")).unwrap();
            std::fs::create_dir(&output).unwrap();
            FileOperationManager::perform_compress(&operation(), vec![], Some(archive.clone())).unwrap();
            FileOperationManager::perform_copy(&operation(), vec![source.clone()], Some(archive.clone()), true, None).unwrap();
            assert!(!source.exists(), "{extension}");
            FileOperationManager::perform_copy(&operation(), vec![archive.join("source")], Some(output.clone()), true, None).unwrap();
            assert!(output.join("source/écriture/空").is_dir(), "{extension}");
            let workspace = ArchiveWorkspace::open(archive).unwrap();
            assert!(!workspace.root().join("source").exists(), "{extension}");
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn archive_directory_move_cancellation_keeps_the_original_archive() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let output = root.path().join("output");
        let archive = root.path().join("source.zip");
        std::fs::create_dir_all(source.join("nested/empty")).unwrap();
        std::fs::create_dir(&output).unwrap();
        FileOperationManager::perform_compress(&operation(), vec![source], Some(archive.clone())).unwrap();
        let original = std::fs::read(&archive).unwrap();
        let op = operation();
        let result = FileOperationManager::perform_archive_transfer_with(
            &op, vec![archive.join("source")], Some(output), true, None,
            &mut || { op.lock().unwrap().cancel_flag.store(true, Ordering::Relaxed); Ok(()) },
        );
        assert!(result.is_err());
        assert_eq!(std::fs::read(archive).unwrap(), original);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn archive_directory_move_keeps_an_entire_subtree_skipped_for_a_file_conflict() {
        use crate::systems::archive_workspace::ArchiveWorkspace;
        let root = tempfile::tempdir().unwrap();
        let seed = root.path().join("seed/source");
        let source = root.path().join("source");
        let archive = root.path().join("target.zip");
        std::fs::create_dir_all(&seed).unwrap();
        std::fs::write(seed.join("blocked"), b"existing archive file").unwrap();
        std::fs::create_dir_all(source.join("blocked/nested")).unwrap();
        std::fs::write(source.join("blocked/nested/keep"), b"skipped source data").unwrap();
        std::fs::create_dir(source.join("empty")).unwrap();
        FileOperationManager::perform_compress(&operation(), vec![seed], Some(archive.clone())).unwrap();
        FileOperationManager::perform_copy(&operation(), vec![source.clone()], Some(archive.clone()), true, None).unwrap();
        assert_eq!(std::fs::read(source.join("blocked/nested/keep")).unwrap(), b"skipped source data");
        assert!(!source.join("empty").exists());
        let workspace = ArchiveWorkspace::open(archive).unwrap();
        assert_eq!(std::fs::read(workspace.root().join("source/blocked")).unwrap(), b"existing archive file");
        assert!(workspace.root().join("source/empty").is_dir());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn real_directory_delete_preserves_junction_targets_and_reports_completion() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        let outside = root.path().join("outside");
        std::fs::create_dir_all(selected.join("nested")).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(selected.join("nested/content"), b"selected").unwrap();
        std::fs::write(outside.join("witness"), b"outside witness").unwrap();
        crate::platform::windows_reparse_tests::junction(&selected.join("link"), &outside);
        let op = Arc::new(Mutex::new(FileOperation::new(FileOpType::Delete, vec![selected.clone()], None, None)));
        FileOperationManager::perform_delete(&op, vec![selected.clone()]).unwrap();
        assert!(!selected.exists());
        assert_eq!(std::fs::read(outside.join("witness")).unwrap(), b"outside witness");
        let locked = op.lock().unwrap();
        assert_eq!(locked.total_files, 1);
        assert_eq!(locked.processed_files, 1);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn directory_copy_refuses_a_nested_destination_junction_before_creation() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
        let outside = root.path().join("outside");
        std::fs::create_dir_all(source.join("nested/empty")).unwrap();
        std::fs::create_dir(&target).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("witness"), b"outside witness").unwrap();
        crate::platform::windows_reparse_tests::junction(&target.join("source"), &outside);
        let op = operation();
        let result = FileOperationManager::perform_copy(&op, vec![source.clone()], Some(target.clone()), false, None);
        let created_outside = outside.join("nested").exists();
        std::fs::remove_dir(target.join("source")).unwrap();
        assert!(result.is_err(), "directory creation accepted a nested destination junction");
        assert!(!created_outside, "directory creation escaped into a junction target");
        assert_eq!(std::fs::read(outside.join("witness")).unwrap(), b"outside witness");
        assert!(source.join("nested/empty").is_dir());
        assert!(op.lock().unwrap().created_dirs.is_empty());
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
    #[cfg(target_os = "windows")]
    fn directory_move_cleans_initial_directories_and_journals_empty_descendants() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
        std::fs::create_dir_all(source.join("nested/empty")).unwrap();
        std::fs::create_dir(&target).unwrap();
        std::fs::write(source.join("nested/data"), b"moved data").unwrap();
        let op = operation();
        FileOperationManager::perform_copy(&op, vec![source.clone()], Some(target.clone()), true, None).unwrap();
        assert!(!source.exists());
        assert_eq!(std::fs::read(target.join("source/nested/data")).unwrap(), b"moved data");
        assert!(target.join("source/nested/empty").is_dir());
        let moved = op.lock().unwrap().moved_dirs.clone();
        assert_eq!(moved.len(), 3);
        let resolved_target = std::fs::canonicalize(&target).unwrap();
        assert_eq!(moved[0], (source.join("nested").join("empty"), resolved_target.join("source/nested/empty")));
        assert_eq!(moved[2], (source, resolved_target.join("source")));
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
    fn explicit_replace_preserves_overwrite_policy_without_a_backup() {
        let temp = tempfile::tempdir().unwrap();
        let source_dir = temp.path().join("source");
        let target_dir = temp.path().join("target");
        std::fs::create_dir(&source_dir).unwrap();
        std::fs::create_dir(&target_dir).unwrap();
        let source = source_dir.join("file.txt");
        let destination = target_dir.join("file.txt");
        std::fs::write(&source, b"new contents").unwrap();
        std::fs::write(&destination, b"old contents that are longer").unwrap();
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
        assert_eq!(
            operation.overwritten_files,
            vec![std::fs::canonicalize(&destination).unwrap()]
        );
        assert!(operation.created_files.is_empty());
        assert_eq!(std::fs::read_dir(&target_dir).unwrap().count(), 1);
        assert_eq!(operation.copied_file_hashes.len(), 1);
        assert_eq!(
            operation.copied_file_hashes[0].1,
            blake3::hash(b"new contents").to_hex().to_string()
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn native_initial_copy_preserves_streams_attributes_and_modified_time() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        let target_dir = root.path().join("target");
        std::fs::create_dir(&target_dir).unwrap();
        std::fs::write(&source, b"main content").unwrap();
        std::fs::write(
            source.with_file_name("source.txt:oxyde-test"),
            b"alternate content",
        )
        .unwrap();
        let modified = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        std::fs::File::options()
            .write(true)
            .open(&source)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        platform().set_hidden_attribute(&source, true).unwrap();
        FileOperationManager::perform_copy(
            &operation(),
            vec![source],
            Some(target_dir.clone()),
            false,
            None,
        )
        .unwrap();
        let destination = target_dir.join("source.txt");
        assert_eq!(std::fs::read(&destination).unwrap(), b"main content");
        assert_eq!(
            std::fs::read(target_dir.join("source.txt:oxyde-test")).unwrap(),
            b"alternate content"
        );
        let metadata = std::fs::metadata(&destination).unwrap();
        assert_eq!(metadata.modified().unwrap(), modified);
        assert!(platform().file_attributes(&metadata, "source.txt").0);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn copy_undo_preserves_a_replacement_directory_containing_the_original_output() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let target_dir = root.path().join("target");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&target_dir).unwrap();
        std::fs::write(source.join("copied.txt"), b"original copied contents").unwrap();
        let target = target_dir.join("source");
        let displaced = root.path().join("original-directory");
        let history = Arc::new(HistoryManager::new());
        let op = Arc::new(Mutex::new(FileOperation::new(
            FileOpType::Copy,
            vec![source],
            Some(target_dir),
            None,
        )));
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            FileOperationManager::execute_operation(history.clone(), op.clone()).await;
            assert_eq!(op.lock().unwrap().status, OpStatus::Completed);
            let proof = history.get_state().undo_stack[0].details.copied_file_identities.as_ref().unwrap()[0].1;
            std::fs::rename(&target, &displaced).unwrap();
            std::fs::create_dir(&target).unwrap();
            std::fs::rename(displaced.join("copied.txt"), target.join("copied.txt")).unwrap();
            assert!(proof.matches(&std::fs::File::open(target.join("copied.txt")).unwrap()).unwrap());
            let error = crate::commands::ops::undo_last_action(&history).await.unwrap_err();
            assert!(matches!(error, crate::models::CommandError::HistoryIdentity(failure) if failure.issue == crate::models::error::HistoryIdentityIssue::Changed));
            assert_eq!(std::fs::read(target.join("copied.txt")).unwrap(), b"original copied contents");
            assert!(displaced.is_dir());
            assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 3);
            assert_eq!(history.get_state().undo_stack.len(), 1);
            assert!(history.get_state().redo_stack.is_empty());
        });
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn copy_undo_preserves_an_empty_directory_replaced_since_creation() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source-empty");
        let target_dir = root.path().join("target");
        std::fs::create_dir_all(source.join("nested-empty")).unwrap();
        std::fs::create_dir(&target_dir).unwrap();
        let target = target_dir.join("source-empty");
        let displaced = root.path().join("original-copy");
        let history = Arc::new(HistoryManager::new());
        let op = Arc::new(Mutex::new(FileOperation::new(
            FileOpType::Copy,
            vec![source],
            Some(target_dir.clone()),
            None,
        )));
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            FileOperationManager::execute_operation(history.clone(), op.clone()).await;
            assert_eq!(op.lock().unwrap().status, OpStatus::Completed);
            let original = platform().hold_resolved_operation_directory(&std::fs::canonicalize(&target).unwrap()).unwrap();
            let identity = platform().file_identity(&original).unwrap();
            drop(original);
            std::fs::rename(&target, &displaced).unwrap();
            std::fs::create_dir(&target).unwrap();
            let replacement = platform().hold_resolved_operation_directory(&std::fs::canonicalize(&target).unwrap()).unwrap();
            assert_ne!(platform().file_identity(&replacement).unwrap(), identity);
            drop(replacement);
            let result = crate::commands::ops::undo_last_action(&history).await;
            assert!(matches!(result, Err(crate::models::CommandError::HistoryIdentity(failure)) if failure.issue == crate::models::error::HistoryIdentityIssue::Changed), "Undo must preserve an independent empty replacement directory");
            assert!(target.is_dir());
            assert!(displaced.join("nested-empty").is_dir());
            assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
            assert_eq!(history.get_state().undo_stack.len(), 1);
            assert!(history.get_state().redo_stack.is_empty());
            std::fs::remove_dir(&target).unwrap();
            std::fs::rename(&displaced, &target).unwrap();
            let transaction_id = history.get_state().undo_stack[0].id.clone();
            for _ in 0..2 {
                let state = history.get_state();
                assert_eq!(state.undo_stack[0].id, transaction_id);
                let identities = state.undo_stack[0].details.copied_directory_identities.as_ref().unwrap();
                assert_eq!(identities.len(), 2);
                for (path, identity) in identities {
                    let directory = platform().hold_resolved_operation_directory(&std::fs::canonicalize(path).unwrap()).unwrap();
                    assert!(identity.snapshot().unwrap().matches(&directory).unwrap());
                }
                // Keeping history must not pin either copied directory in place.
                std::fs::rename(&target, &displaced).unwrap();
                std::fs::rename(&displaced, &target).unwrap();
                crate::commands::ops::undo_last_action(&history).await.unwrap();
                assert!(!target.exists());
                crate::commands::ops::redo_last_action(&history).await.unwrap();
                assert!(target.join("nested-empty").is_dir());
            }
            crate::commands::ops::undo_last_action(&history).await.unwrap();
            assert_eq!(std::fs::read_dir(&target_dir).unwrap().count(), 0);
        });
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn native_readonly_copy_history_preserves_streams_on_redo() {
        struct WritablePaths(Vec<PathBuf>);
        impl Drop for WritablePaths {
            fn drop(&mut self) {
                for path in &self.0 {
                    if let Ok(metadata) = std::fs::metadata(path) {
                        let mut permissions = metadata.permissions();
                        permissions.set_readonly(false);
                        std::fs::set_permissions(path, permissions).unwrap();
                    }
                }
            }
        }
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        let target_dir = root.path().join("target");
        std::fs::create_dir(&target_dir).unwrap();
        let target = target_dir.join("source.txt");
        let _cleanup = WritablePaths(vec![source.clone(), target.clone()]);
        std::fs::write(&source, b"readonly main").unwrap();
        std::fs::write(root.path().join("source.txt:extra"), b"readonly stream").unwrap();
        let modified = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        std::fs::File::options()
            .write(true)
            .open(&source)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        let mut permissions = std::fs::metadata(&source).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&source, permissions).unwrap();
        let history = Arc::new(HistoryManager::new());
        let op = Arc::new(Mutex::new(FileOperation::new(
            FileOpType::Copy,
            vec![source.clone()],
            Some(target_dir.clone()),
            None,
        )));
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            FileOperationManager::execute_operation(history.clone(), op.clone()).await;
            assert_eq!(op.lock().unwrap().status, OpStatus::Completed);
            let transaction_id = history.get_state().undo_stack[0].id.clone();
            for _ in 0..2 {
                let state = history.get_state();
                assert_eq!(state.undo_stack[0].id, transaction_id);
                let identities = state.undo_stack[0]
                    .details
                    .copied_file_identities
                    .as_ref()
                    .unwrap();
                assert_eq!(identities.len(), 1);
                assert_eq!(
                    PathBuf::from(&identities[0].0),
                    std::fs::canonicalize(&target).unwrap()
                );
                assert!(
                    identities[0]
                        .1
                        .matches(&std::fs::File::open(&target).unwrap())
                        .unwrap()
                );
                // The journal contains values only; it must not keep a rename lock.
                let renamed = target_dir.join("temporarily-renamed.txt");
                std::fs::rename(&target, &renamed).unwrap();
                std::fs::rename(&renamed, &target).unwrap();
                let metadata = std::fs::metadata(&target).unwrap();
                assert!(metadata.permissions().readonly());
                assert_eq!(metadata.modified().unwrap(), modified);
                assert_eq!(
                    std::fs::read(target_dir.join("source.txt:extra")).unwrap(),
                    b"readonly stream"
                );
                crate::commands::ops::undo_last_action(&history)
                    .await
                    .unwrap();
                assert!(!target.exists());
                assert!(std::fs::metadata(&source).unwrap().permissions().readonly());
                crate::commands::ops::redo_last_action(&history)
                    .await
                    .unwrap();
            }
            crate::commands::ops::undo_last_action(&history)
                .await
                .unwrap();
            assert_eq!(std::fs::read_dir(&target_dir).unwrap().count(), 0);
        });
    }
    #[test]
    fn overwrite_only_copy_is_recorded_as_non_undoable_history_entry() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let temp = tempfile::tempdir().unwrap();
            let source_dir = temp.path().join("source");
            let target_dir = temp.path().join("target");
            std::fs::create_dir(&source_dir).unwrap();
            std::fs::create_dir(&target_dir).unwrap();
            let source = source_dir.join("item.txt");
            let destination = target_dir.join("item.txt");
            std::fs::write(&source, b"new version").unwrap();
            std::fs::write(&destination, b"old version").unwrap();
            let resolutions = HashMap::from([(
                source.to_string_lossy().into_owned(),
                ConflictAction::Replace,
            )]);
            let history = Arc::new(HistoryManager::new());
            let manager = FileOperationManager::new();
            let id = manager.queue_operation_on(
                &tokio::runtime::Handle::current(),
                history.clone(),
                FileOperation::new(
                    FileOpType::Copy,
                    vec![source],
                    Some(target_dir),
                    Some(resolutions),
                ),
            );
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    let status = manager.get_operation(&id).unwrap().status;
                    if FileOperationManager::is_final(&status) {
                        assert_eq!(status, OpStatus::Completed);
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();

            let undo_stack = history.get_state().undo_stack;
            assert_eq!(undo_stack.len(), 1);
            assert!(undo_stack[0].undo_blocked_by_overwrite());
            assert_eq!(std::fs::read(&destination).unwrap(), b"new version");
            assert!(crate::commands::ops::undo_last_action(&history)
                .await
                .is_err());
            assert_eq!(history.get_state().undo_stack.len(), 1);
        });
    }

    #[test]
    fn undo_mixed_copy_with_overwrite_is_blocked_as_a_whole() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let temp = tempfile::tempdir().unwrap();
            let source_dir = temp.path().join("source");
            let target_dir = temp.path().join("target");
            std::fs::create_dir(&source_dir).unwrap();
            std::fs::create_dir(&target_dir).unwrap();
            let existing_source = source_dir.join("existing.txt");
            let new_source = source_dir.join("new.txt");
            let existing_target = target_dir.join("existing.txt");
            let new_target = target_dir.join("new.txt");
            std::fs::write(&existing_source, b"new version").unwrap();
            std::fs::write(&new_source, b"new file").unwrap();
            std::fs::write(&existing_target, b"old version").unwrap();
            let resolutions = HashMap::from([(
                existing_source.to_string_lossy().into_owned(),
                ConflictAction::Replace,
            )]);
            let history = Arc::new(HistoryManager::new());
            let manager = FileOperationManager::new();
            let id = manager.queue_operation_on(
                &tokio::runtime::Handle::current(),
                history.clone(),
                FileOperation::new(
                    FileOpType::Copy,
                    vec![existing_source, new_source],
                    Some(target_dir.clone()),
                    Some(resolutions),
                ),
            );
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    let status = manager.get_operation(&id).unwrap().status;
                    if FileOperationManager::is_final(&status) {
                        assert_eq!(status, OpStatus::Completed);
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            assert_eq!(history.get_state().undo_stack.len(), 1);
            assert!(crate::commands::ops::undo_last_action(&history)
                .await
                .is_err());
            assert_eq!(std::fs::read(&existing_target).unwrap(), b"new version");
            assert_eq!(std::fs::read(&new_target).unwrap(), b"new file");
            assert_eq!(std::fs::read_dir(&target_dir).unwrap().count(), 2);
            assert_eq!(history.get_state().undo_stack.len(), 1);
        });
    }

    #[test]
    fn undo_replacing_move_restores_source_without_a_backup() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let temp = tempfile::tempdir().unwrap();
            let source_dir = temp.path().join("source");
            let target_dir = temp.path().join("target");
            std::fs::create_dir(&source_dir).unwrap();
            std::fs::create_dir(&target_dir).unwrap();
            let source = source_dir.join("file.txt");
            let target = target_dir.join("file.txt");
            std::fs::write(&source, b"moving file").unwrap();
            std::fs::write(&target, b"old target").unwrap();
            let resolutions = HashMap::from([(
                source.to_string_lossy().into_owned(),
                ConflictAction::Replace,
            )]);
            let history = Arc::new(HistoryManager::new());
            let manager = FileOperationManager::new();
            let id = manager.queue_operation_on(
                &tokio::runtime::Handle::current(),
                history.clone(),
                FileOperation::new(
                    FileOpType::Move,
                    vec![source.clone()],
                    Some(target_dir.clone()),
                    Some(resolutions),
                ),
            );
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    let status = manager.get_operation(&id).unwrap().status;
                    if FileOperationManager::is_final(&status) {
                        assert_eq!(status, OpStatus::Completed);
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            assert!(!source.exists());
            assert_eq!(std::fs::read(&target).unwrap(), b"moving file");
            crate::commands::ops::undo_last_action(&history)
                .await
                .unwrap();
            assert_eq!(std::fs::read(&source).unwrap(), b"moving file");
            assert!(!target.exists());
            assert_eq!(std::fs::read_dir(&target_dir).unwrap().count(), 0);
        });
    }

    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "requires OXYDE_TEST_SECOND_VOLUME; moves only generated fixtures between two volumes"]
    fn native_cross_volume_move_history_roundtrip() {
        let second_root = PathBuf::from(
            std::env::var_os("OXYDE_TEST_SECOND_VOLUME")
                .expect("set OXYDE_TEST_SECOND_VOLUME to a writable second volume"),
        );
        assert!(second_root.is_absolute() && second_root.is_dir());
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::Builder::new()
            .prefix("oxyde-a19-")
            .tempdir_in(second_root)
            .unwrap();
        let probe = first.path().join("volume-probe");
        std::fs::write(&probe, b"probe").unwrap();
        assert_eq!(
            std::fs::rename(&probe, second.path().join("volume-probe"))
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::CrossesDevices
        );
        std::fs::remove_file(probe).unwrap();

        let source = first.path().join("Données 日本語 🦀");
        std::fs::create_dir_all(source.join("nested/empty")).unwrap();
        std::fs::write(source.join("nested/file.txt"), b"nested fixture").unwrap();
        std::fs::write(source.join("nested/file.txt:extra"), b"initial move stream").unwrap();
        let modified = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        std::fs::File::options()
            .write(true)
            .open(source.join("nested/file.txt"))
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        std::fs::write(
            source.join("shortcut.url"),
            b"[InternetShortcut]\nURL=https://example.org\n",
        )
        .unwrap();
        let destination = second.path().join(source.file_name().unwrap());
        let history = Arc::new(HistoryManager::new());
        let manager = FileOperationManager::new();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let id = manager.queue_operation_on(
                &tokio::runtime::Handle::current(),
                history.clone(),
                FileOperation::new(
                    FileOpType::Move,
                    vec![source.clone()],
                    Some(second.path().into()),
                    None,
                ),
            );
            tokio::time::timeout(std::time::Duration::from_secs(15), async {
                loop {
                    let status = manager.get_operation(&id).unwrap().status;
                    if FileOperationManager::is_final(&status) {
                        assert_eq!(status, OpStatus::Completed);
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            assert!(!source.exists());
            assert_eq!(
                std::fs::read(destination.join("nested/file.txt:extra")).unwrap(),
                b"initial move stream"
            );
            assert_eq!(
                std::fs::metadata(destination.join("nested/file.txt"))
                    .unwrap()
                    .modified()
                    .unwrap(),
                modified
            );
            // A27: the files have already crossed volumes when this original
            // empty directory is recreated. Its replacement by a foreign file
            // must roll back the whole Undo and keep the history retryable.
            let blocker = source.join("nested/empty");
            std::fs::create_dir_all(blocker.parent().unwrap()).unwrap();
            std::fs::write(&blocker, b"foreign directory blocker").unwrap();
            let error = crate::commands::ops::undo_last_action(&history)
                .await
                .unwrap_err();
            assert!(error.to_string().contains("Not a directory"), "{error}");
            assert_eq!(history.get_state().undo_stack.len(), 1);
            assert!(history.get_state().redo_stack.is_empty());
            assert_eq!(
                std::fs::read(&blocker).unwrap(),
                b"foreign directory blocker"
            );
            assert!(!source.join("nested/file.txt").exists());
            assert!(!source.join("shortcut.url").exists());
            assert_eq!(
                std::fs::read(destination.join("nested/file.txt")).unwrap(),
                b"nested fixture"
            );
            assert!(destination.join("nested/empty").is_dir());
            std::fs::remove_file(blocker).unwrap();
            for cycle in 0..2 {
                crate::commands::ops::undo_last_action(&history)
                    .await
                    .unwrap();
                assert_eq!(
                    std::fs::read(source.join("nested/file.txt")).unwrap(),
                    b"nested fixture"
                );
                assert!(source.join("nested/empty").is_dir());
                assert_eq!(
                    std::fs::read(source.join("nested/file.txt:extra")).unwrap(),
                    b"initial move stream"
                );
                assert_eq!(
                    std::fs::metadata(source.join("nested/file.txt"))
                        .unwrap()
                        .modified()
                        .unwrap(),
                    modified
                );
                assert!(source.join("shortcut.url").is_file());
                assert!(!destination.exists());
                if cycle == 0 {
                    // Preflight can read this file, but the second move cannot
                    // acquire its exclusive handle. The first moved file must
                    // roll back across volumes before history can be retried.
                    let state = history.get_state();
                    let moved = state
                        .redo_stack
                        .last()
                        .unwrap()
                        .details
                        .moved_files
                        .as_ref()
                        .unwrap();
                    assert!(moved.len() >= 2);
                    let writer = std::fs::File::options()
                        .write(true)
                        .open(&moved[1].0)
                        .unwrap();
                    assert!(crate::commands::ops::redo_last_action(&history)
                        .await
                        .is_err());
                    assert_eq!(history.get_state().redo_stack.len(), 1);
                    assert!(history.get_state().undo_stack.is_empty());
                    for (original, target) in moved {
                        assert!(std::path::Path::new(original).is_file());
                        assert!(!std::path::Path::new(target).exists());
                    }
                    assert!(!destination.exists());
                    drop(writer);
                }
                crate::commands::ops::redo_last_action(&history)
                    .await
                    .unwrap();
                assert!(!source.exists());
                assert_eq!(
                    std::fs::read(destination.join("nested/file.txt")).unwrap(),
                    b"nested fixture"
                );
                assert!(destination.join("nested/empty").is_dir());
                assert_eq!(
                    std::fs::read(destination.join("nested/file.txt:extra")).unwrap(),
                    b"initial move stream"
                );
                assert_eq!(
                    std::fs::metadata(destination.join("nested/file.txt"))
                        .unwrap()
                        .modified()
                        .unwrap(),
                    modified
                );
                assert!(destination.join("shortcut.url").is_file());
            }
        });
    }

    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "requires OXYDE_TEST_SECOND_VOLUME; checks partial initial Move using generated fixtures"]
    fn native_cross_volume_refused_source_delete_can_be_undone() {
        struct WritablePaths(Vec<PathBuf>);
        impl Drop for WritablePaths {
            fn drop(&mut self) {
                for path in &self.0 {
                    if let Ok(metadata) = std::fs::metadata(path) {
                        let mut permissions = metadata.permissions();
                        permissions.set_readonly(false);
                        std::fs::set_permissions(path, permissions).unwrap();
                    }
                }
            }
        }
        let second_root = PathBuf::from(
            std::env::var_os("OXYDE_TEST_SECOND_VOLUME").expect("set a writable second volume"),
        );
        assert!(second_root.is_absolute() && second_root.is_dir());
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::Builder::new()
            .prefix("oxyde-a31-")
            .tempdir_in(second_root)
            .unwrap();
        let regular = first.path().join("regular.txt");
        let readonly = first.path().join("readonly.txt");
        let target = second.path().join("readonly.txt");
        let _cleanup = WritablePaths(vec![readonly.clone(), target.clone()]);
        std::fs::write(&regular, b"regular move").unwrap();
        std::fs::write(&readonly, b"readonly move").unwrap();
        assert_eq!(
            std::fs::rename(&regular, second.path().join("probe"))
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::CrossesDevices
        );
        let mut permissions = std::fs::metadata(&readonly).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&readonly, permissions).unwrap();
        let history = Arc::new(HistoryManager::new());
        let op = Arc::new(Mutex::new(FileOperation::new(
            FileOpType::Move,
            vec![regular.clone(), readonly.clone()],
            Some(second.path().into()),
            None,
        )));
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            FileOperationManager::execute_operation(history.clone(), op.clone()).await;
            let state = op.lock().unwrap().clone();
            assert!(matches!(state.status, OpStatus::Error(ref message) if message.contains("cannot remove")), "{:?}", state.status);
            assert_eq!(state.moved_files.len(), 1);
            assert!(!regular.exists());
            assert_eq!(std::fs::read(&readonly).unwrap(), b"readonly move");
            assert_eq!(std::fs::read(&target).unwrap(), b"readonly move");
            assert!(std::fs::metadata(&readonly).unwrap().permissions().readonly());
            assert_eq!(history.get_state().undo_stack.len(), 1);
            crate::commands::ops::undo_last_action(&history).await.unwrap();
            assert_eq!(std::fs::read(&regular).unwrap(), b"regular move");
            assert_eq!(std::fs::read(&readonly).unwrap(), b"readonly move");
            assert!(std::fs::metadata(&readonly).unwrap().permissions().readonly());
            assert_eq!(std::fs::read_dir(second.path()).unwrap().count(), 0);
            assert_eq!(history.get_state().redo_stack.len(), 1);
        });
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

#[cfg(all(test, target_os = "windows"))]
mod trash_transfer_tests {
    use super::*;

    struct Fixture {
        root: tempfile::TempDir,
        originals: Vec<PathBuf>,
        physical: Vec<PathBuf>,
    }
    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            let originals: Vec<_> = [
                "Rapport école 🦀.txt",
                "Raccourci école.lnk",
                "Lien école.url",
                "Dossier école",
                "Dossier vide",
            ]
            .iter()
            .map(|name| root.path().join(name))
            .collect();
            for source in &originals[..3] {
                std::fs::write(source, b"trash transfer fixture").unwrap();
            }
            for source in &originals[3..] {
                std::fs::create_dir(source).unwrap();
            }
            std::fs::write(originals[3].join("contenu.txt"), b"nested fixture").unwrap();
            platform().shell_delete(&originals, true).unwrap();
            let listed = crate::commands::ops::list_trash().unwrap();
            let physical = originals
                .iter()
                .map(|source| {
                    PathBuf::from(
                        &listed
                            .iter()
                            .find(|entry| PathBuf::from(&entry.original_path) == *source)
                            .unwrap()
                            .path,
                    )
                })
                .collect();
            Self {
                root,
                originals,
                physical,
            }
        }
        fn target(&self, name: &str) -> PathBuf {
            let path = self.root.path().join(name);
            std::fs::create_dir(&path).unwrap();
            path
        }
        fn assert_payload(&self, target: &Path) {
            for source in &self.originals[..3] {
                assert_eq!(
                    std::fs::read(target.join(source.file_name().unwrap())).unwrap(),
                    b"trash transfer fixture"
                );
            }
            assert_eq!(
                std::fs::read(
                    target
                        .join(self.originals[3].file_name().unwrap())
                        .join("contenu.txt")
                )
                .unwrap(),
                b"nested fixture"
            );
            assert!(target.join(self.originals[4].file_name().unwrap()).is_dir());
            assert_eq!(
                std::fs::read_dir(target).unwrap().count(),
                self.originals.len()
            );
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            // Restore only generated entries still present, including on assertion failure.
            if let Ok(listed) = crate::commands::ops::list_trash() {
                let ids: Vec<_> = listed
                    .into_iter()
                    .filter(|entry| {
                        self.originals
                            .contains(&PathBuf::from(&entry.original_path))
                    })
                    .map(|entry| entry.path)
                    .collect();
                if !ids.is_empty() {
                    let result = std::thread::spawn(move || {
                        tokio::runtime::Runtime::new()
                            .unwrap()
                            .block_on(crate::commands::ops::restore_items(ids))
                    })
                    .join();
                    if !matches!(result, Ok(Ok(_))) {
                        eprintln!("Generated trash fixture cleanup failed: {result:?}");
                    }
                }
            }
        }
    }
    async fn queued(
        history: Arc<HistoryManager>,
        sources: Vec<PathBuf>,
        target: PathBuf,
        move_files: bool,
        resolutions: Option<HashMap<String, ConflictAction>>,
    ) -> OpStatus {
        let manager = FileOperationManager::new();
        let id = manager.queue_operation_on(
            &tokio::runtime::Handle::current(),
            history,
            FileOperation::new(
                if move_files {
                    FileOpType::Move
                } else {
                    FileOpType::Copy
                },
                sources,
                Some(target),
                resolutions,
            ),
        );
        tokio::time::timeout(std::time::Duration::from_secs(20), async {
            loop {
                let status = manager.get_operation(&id).unwrap().status;
                if FileOperationManager::is_final(&status) {
                    return status;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }

    #[test]
    #[ignore = "copies/moves only generated native trash fixtures through the GPUI operation queue"]
    fn native_trash_transfer_names_metadata_and_history_roundtrip() {
        let fixture = Fixture::new();
        let target = fixture.target("output");
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let history = Arc::new(HistoryManager::new());
            assert_eq!(
                queued(
                    history.clone(),
                    fixture.physical.clone(),
                    target.clone(),
                    false,
                    None
                )
                .await,
                OpStatus::Completed
            );
            fixture.assert_payload(&target);
            assert_eq!(
                platform()
                    .trash_transfer_entries(&fixture.physical)
                    .unwrap()
                    .len(),
                fixture.physical.len()
            );
            let conflicts = crate::commands::ops::check_conflicts(
                fixture
                    .physical
                    .iter()
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect(),
                target.to_string_lossy().into_owned(),
            )
            .await
            .unwrap();
            for source in &fixture.originals {
                let name = source.file_name().unwrap().to_string_lossy();
                assert!(conflicts
                    .iter()
                    .any(|conflict| conflict.name == name && conflict.source.name == name));
            }
            crate::commands::ops::undo_last_action(&history)
                .await
                .unwrap();
            assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
            crate::commands::ops::redo_last_action(&history)
                .await
                .unwrap();
            fixture.assert_payload(&target);
            crate::commands::ops::undo_last_action(&history)
                .await
                .unwrap();
            history.clear();
            assert_eq!(
                queued(
                    history.clone(),
                    fixture.physical.clone(),
                    target.clone(),
                    true,
                    None
                )
                .await,
                OpStatus::Completed
            );
            fixture.assert_payload(&target);
            let backups = history.get_state().undo_stack[0]
                .details
                .trash_metadata_backups
                .clone()
                .unwrap();
            assert_eq!(backups.len(), fixture.originals.len());
            let saved = PathBuf::from(&backups[0]).with_extension("fixture-saved");
            std::fs::rename(&backups[0], &saved).unwrap();
            assert!(crate::commands::ops::undo_last_action(&history)
                .await
                .is_err());
            fixture.assert_payload(&target);
            assert!(fixture.physical.iter().all(|path| !path.exists()));
            std::fs::rename(&saved, &backups[0]).unwrap();
            for _ in 0..2 {
                assert!(fixture.physical.iter().all(|path| !path.exists()));
                assert!(backups.iter().all(|path| Path::new(path).exists()));
                crate::commands::ops::undo_last_action(&history)
                    .await
                    .unwrap();
                assert_eq!(
                    platform()
                        .trash_transfer_entries(&fixture.physical)
                        .unwrap()
                        .len(),
                    fixture.physical.len()
                );
                assert!(backups.iter().all(|path| !Path::new(path).exists()));
                assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
                let blocker = target.join(fixture.originals[0].file_name().unwrap());
                std::fs::write(&blocker, b"redo conflict fixture").unwrap();
                assert!(crate::commands::ops::redo_last_action(&history)
                    .await
                    .is_err());
                assert_eq!(std::fs::read(&blocker).unwrap(), b"redo conflict fixture");
                assert!(fixture.physical.iter().all(|path| path.exists()));
                std::fs::remove_file(blocker).unwrap();
                crate::commands::ops::redo_last_action(&history)
                    .await
                    .unwrap();
                fixture.assert_payload(&target);
            }
            history.clear();
            assert!(backups.iter().all(|path| !Path::new(path).exists()));
        });
    }

    #[test]
    #[ignore = "checks conflicts and partial moves using only generated native trash fixtures"]
    fn native_trash_transfer_conflicts_preserve_skipped_metadata() {
        let fixture = Fixture::new();
        let target = fixture.target("output");
        let original_name = fixture.originals[0].file_name().unwrap();
        std::fs::write(target.join(original_name), b"existing fixture").unwrap();
        let folder = target.join(fixture.originals[3].file_name().unwrap());
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("contenu.txt"), b"existing nested fixture").unwrap();
        std::fs::write(
            fixture.physical[3].join("nouveau.txt"),
            b"new nested fixture",
        )
        .unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let history = Arc::new(HistoryManager::new());
            let sources = vec![fixture.physical[0].clone(), fixture.physical[3].clone()];
            // Queue revalidation uses the logical destination name too.
            assert!(matches!(
                queued(history.clone(), sources.clone(), target.clone(), true, None).await,
                OpStatus::Error(_)
            ));
            assert!(history.get_state().undo_stack.is_empty());
            let resolutions = HashMap::from([
                (
                    fixture.physical[0].to_string_lossy().into_owned(),
                    ConflictAction::Skip,
                ),
                (
                    fixture.physical[3]
                        .join("contenu.txt")
                        .to_string_lossy()
                        .into_owned(),
                    ConflictAction::Skip,
                ),
                (
                    fixture.physical[3].to_string_lossy().into_owned(),
                    ConflictAction::Replace,
                ),
            ]);
            assert_eq!(
                queued(
                    history.clone(),
                    sources.clone(),
                    target.clone(),
                    true,
                    Some(resolutions)
                )
                .await,
                OpStatus::Completed
            );
            assert_eq!(
                std::fs::read(target.join(original_name)).unwrap(),
                b"existing fixture"
            );
            assert_eq!(
                std::fs::read(folder.join("contenu.txt")).unwrap(),
                b"existing nested fixture"
            );
            assert_eq!(
                std::fs::read(folder.join("nouveau.txt")).unwrap(),
                b"new nested fixture"
            );
            assert_eq!(
                platform().trash_transfer_entries(&sources).unwrap().len(),
                sources.len()
            );
            assert!(history.get_state().undo_stack[0]
                .details
                .trash_metadata_backups
                .as_ref()
                .unwrap()
                .is_empty());
            crate::commands::ops::undo_last_action(&history)
                .await
                .unwrap();
            assert!(fixture.physical[3].join("nouveau.txt").exists());
            assert!(!folder.join("nouveau.txt").exists());
        });
    }
    #[test]
    #[ignore = "transfers only generated native trash fixtures into a temporary archive"]
    fn native_trash_transfer_into_archive_uses_original_names() {
        let fixture = Fixture::new();
        let archive = fixture.root.path().join("output.zip");
        let file = std::fs::File::create(&archive).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .add_directory("contents/", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.finish().unwrap();
        let target = archive.join("contents");
        for move_files in [false, true] {
            let op = Arc::new(Mutex::new(FileOperation::new(
                if move_files {
                    FileOpType::Move
                } else {
                    FileOpType::Copy
                },
                fixture.physical.clone(),
                Some(target.clone()),
                None,
            )));
            let resolutions = if move_files {
                Some(
                    fixture
                        .physical
                        .iter()
                        .map(|path| (path.to_string_lossy().into_owned(), ConflictAction::Replace))
                        .collect(),
                )
            } else {
                None
            };
            FileOperationManager::perform_copy(
                &op,
                fixture.physical.clone(),
                Some(target.clone()),
                move_files,
                resolutions,
            )
            .unwrap();
            let mut zip = zip::ZipArchive::new(std::fs::File::open(&archive).unwrap()).unwrap();
            for source in &fixture.originals[..3] {
                let mut content = Vec::new();
                zip.by_name(&format!(
                    "contents/{}",
                    source.file_name().unwrap().to_string_lossy()
                ))
                .unwrap()
                .read_to_end(&mut content)
                .unwrap();
                assert_eq!(content, b"trash transfer fixture");
            }
            assert!(fixture
                .physical
                .iter()
                .all(|path| path.exists() != move_files));
            assert!(op.lock().unwrap().trash_metadata_backups.is_empty());
        }
    }
    #[test]
    #[ignore = "checks Delete Undo/Redo with only generated native trash fixtures"]
    fn native_delete_undo_redo_preserves_original_names() {
        let fixture = Fixture::new();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            crate::commands::ops::restore_items(
                fixture
                    .physical
                    .iter()
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect(),
            )
            .await
            .unwrap();
            let history = Arc::new(HistoryManager::new());
            let operation = Arc::new(Mutex::new(FileOperation::new(
                FileOpType::Trash,
                fixture.originals.clone(),
                None,
                None,
            )));
            FileOperationManager::execute_operation(history.clone(), operation.clone()).await;
            assert_eq!(operation.lock().unwrap().status, OpStatus::Completed);
            for _ in 0..2 {
                assert!(fixture.originals.iter().all(|path| !path.exists()));
                crate::commands::ops::undo_last_action(&history)
                    .await
                    .unwrap();
                fixture.assert_payload(fixture.root.path());
                assert_eq!(history.get_state().redo_stack.len(), 1);
                crate::commands::ops::redo_last_action(&history)
                    .await
                    .unwrap();
                assert!(fixture.originals.iter().all(|path| !path.exists()));
                assert_eq!(history.get_state().undo_stack.len(), 1);
            }
            crate::commands::ops::undo_last_action(&history)
                .await
                .unwrap();
            fixture.assert_payload(fixture.root.path());
        });
    }
}
