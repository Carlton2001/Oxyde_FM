use crate::models::ConflictAction;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{Notify, Semaphore};
use uuid::Uuid;

use crate::platform::{platform, ThreadPriority};

mod archive_transfer;
mod compress;
mod copy;
mod delete;
mod execution;
mod extract;
mod scheduler;

const SPEED_HISTORY_LIMIT: usize = 60;
const FILE_OPS_LOG_TARGET: &str = module_path!();

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
    pub copy_name_suffix: String,
    archive_path_mappings: Vec<(PathBuf, PathBuf)>,
    archive_publication_finished: bool,
    #[cfg(all(target_os = "windows", not(test)))]
    archive_owned_roots: Vec<(PathBuf, std::sync::Weak<Mutex<super::archive_owned_tree::OwnedTree>>)>,
    #[cfg(all(target_os = "windows", not(test)))]
    archive_cleanup_error: Option<String>,
    pub turbo: bool,
    pub resolutions: Option<HashMap<String, ConflictAction>>,
    pub created_files: Vec<PathBuf>,
    pub created_dirs: Vec<PathBuf>,
    pub overwritten_files: Vec<PathBuf>,
    pub moved_files: Vec<(PathBuf, PathBuf)>,
    moved_directory_identities: Vec<(PathBuf, crate::models::transaction::HistoryDirectoryIdentity)>,
    moved_file_identities: Vec<(PathBuf, crate::models::transaction::HistoryMoveIdentity)>,
    trash_metadata_backups: Vec<PathBuf>,
    deleted_items: Vec<crate::models::transaction::HistoryDeletedItem>,
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
            copy_name_suffix: self.copy_name_suffix.clone(),
            archive_path_mappings: self.archive_path_mappings.clone(),
            archive_publication_finished: self.archive_publication_finished,
            #[cfg(all(target_os = "windows", not(test)))]
            archive_owned_roots: Vec::new(),
            #[cfg(all(target_os = "windows", not(test)))]
            archive_cleanup_error: None,
            turbo: self.turbo,
            resolutions: None,
            created_files: Vec::new(),
            created_dirs: Vec::new(),
            overwritten_files: Vec::new(),
            moved_files: Vec::new(),
            moved_directory_identities: Vec::new(),
            moved_file_identities: Vec::new(),
            trash_metadata_backups: Vec::new(),
            deleted_items: Vec::new(),
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
            copy_name_suffix: "Copie".into(),
            archive_path_mappings: Vec::new(),
            archive_publication_finished: false,
            #[cfg(all(target_os = "windows", not(test)))]
            archive_owned_roots: Vec::new(),
            #[cfg(all(target_os = "windows", not(test)))]
            archive_cleanup_error: None,
            turbo: false,
            resolutions,
            created_files: Vec::new(),
            created_dirs: Vec::new(),
            overwritten_files: Vec::new(),
            moved_files: Vec::new(),
            moved_directory_identities: Vec::new(),
            moved_file_identities: Vec::new(),
            trash_metadata_backups: Vec::new(),
            deleted_items: Vec::new(),
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

    pub(crate) fn archive_checkpoint(op: &Arc<Mutex<FileOperation>>) -> Result<(), String> {
        #[cfg(all(target_os = "windows", not(test)))]
        if let Some(error) = &op.lock().unwrap().archive_cleanup_error { return Err(error.clone()); }
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

    pub(crate) fn archive_progress(op: &Arc<Mutex<FileOperation>>, bytes: u64) {
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
    pub(crate) fn archive_publication_finished(op: &Arc<Mutex<FileOperation>>) { op.lock().unwrap().archive_publication_finished = true; }

    fn finish_status(operation: &mut FileOperation, result: OpStatus) {
        let completed = result == OpStatus::Completed && (operation.archive_publication_finished || (operation.total_files > 0 && operation.processed_files == operation.total_files));
        let recovery = matches!(&result, OpStatus::Error(detail) if crate::models::error::ArchiveRecoveryFailure::from_message(detail).is_some());
        operation.status = if operation.cancel_flag.load(Ordering::Relaxed) && !completed && !recovery { OpStatus::Cancelled } else { result };
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
#[path = "file_ops/tests/mod.rs"]
mod tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "file_ops/tests/trash_transfer.rs"]
mod trash_transfer_tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "file_ops/tests/same_folder_transfer.rs"]
mod same_folder_transfer_tests;
