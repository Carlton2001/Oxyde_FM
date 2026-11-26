//! The copy a Redo makes again. Shown, it runs as an operation of the panel, queued like
//! the others, with its progress, pause, Turbo and Cancel; the Redo keeps its checks,
//! its folders and its rollback, and waits for it. It records no history entry: the
//! Redo moves its own.
use super::{FileOpType, FileOperation, FileOperationManager, OpStatus};
use crate::models::transaction::HistoryFileIdentity;
use crate::models::{CommandError, HistoryManager};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Source, destination and recorded hash of each file.
pub(crate) type RedoCopyFiles = Vec<(PathBuf, PathBuf, String)>;
pub(crate) type RedoCopyIdentities = Vec<(String, HistoryFileIdentity)>;

/// Where a Redo shows its copy.
pub struct RedoCopyPanel {
    pub manager: Arc<FileOperationManager>,
    pub runtime: tokio::runtime::Handle,
    pub history: Arc<HistoryManager>,
    pub turbo: bool,
    /// Receives the operation's id once it is queued.
    pub queued: Mutex<Option<futures::channel::oneshot::Sender<String>>>,
}

/// The files to copy, then what the copy returned, shared with the queued operation.
#[derive(Clone)]
pub(super) struct RedoCopy(Arc<Job>);
struct Job {
    files: Mutex<Option<RedoCopyFiles>>,
    result: Mutex<Option<Result<RedoCopyIdentities, CommandError>>>,
}
impl std::fmt::Debug for RedoCopy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RedoCopy")
    }
}

fn cancelled() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "Redo copy was cancelled")
}

impl RedoCopyPanel {
    /// Blocking: queues the copy, waits until it ends and returns what it published.
    /// A copy cancelled before it started returns an interruption.
    pub(crate) fn copy(
        &self,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
        files: RedoCopyFiles,
    ) -> Result<RedoCopyIdentities, CommandError> {
        let job = RedoCopy(Arc::new(Job { files: Mutex::new(Some(files)), result: Mutex::new(None) }));
        let mut operation = FileOperation::new(FileOpType::Copy, sources, destination, None);
        operation.redo_copy = Some(job.clone());
        operation.set_turbo(self.turbo);
        let id = self.manager.queue_operation_on(&self.runtime, self.history.clone(), operation);
        if let Some(queued) = self.queued.lock().unwrap().take() {
            let _ = queued.send(id.clone());
        }
        let result = || job.0.result.lock().unwrap().take();
        loop {
            if let Some(result) = result() {
                return result;
            }
            let ended = self
                .manager
                .operation_progress(&id)
                .is_none_or(|progress| FileOperationManager::is_final(&progress.status));
            if ended {
                return result().unwrap_or_else(|| Err(cancelled().into()));
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

impl FileOperationManager {
    pub(super) fn perform_redo_copy(op: &Arc<Mutex<FileOperation>>, job: RedoCopy) -> Result<(), String> {
        let files = job.0.files.lock().unwrap().take().unwrap_or_default();
        {
            let mut locked = op.lock().unwrap();
            locked.item_tracking_atomic = true;
            locked.total_files = files.len();
            locked.total_bytes = files
                .iter()
                .filter_map(|(source, _, _)| std::fs::metadata(source).ok())
                .map(|metadata| metadata.len())
                .sum();
            locked.status = OpStatus::Running;
        }
        let result = copy_files(files, &mut Shown { op, reported: 0 });
        // All or nothing: a failed copy removed what it had published.
        {
            let mut locked = op.lock().unwrap();
            let outcome = if result.is_ok() { super::ItemOutcome::Completed } else { super::ItemOutcome::NotProcessed };
            for source in locked.sources.clone() {
                locked.item_outcomes.insert(source, outcome);
            }
        }
        let status = result.as_ref().map(|_| ()).map_err(ToString::to_string);
        *job.0.result.lock().unwrap() = Some(result);
        status
    }
}

/// What the copy tells while it runs; an error stops it, and it removes what it published.
pub(crate) trait CopyObserver {
    fn start(&mut self, _source: &Path) {}
    /// Bytes copied of the current file so far; may wait for a pause.
    fn progress(&mut self, _copied: u64) -> io::Result<()> {
        Ok(())
    }
    fn finished(&mut self) {}
}
impl CopyObserver for () {}

struct Shown<'a> {
    op: &'a Arc<Mutex<FileOperation>>,
    reported: u64,
}
impl CopyObserver for Shown<'_> {
    fn start(&mut self, source: &Path) {
        self.reported = 0;
        self.op.lock().unwrap().current_file = Some(source.to_string_lossy().into_owned());
    }
    fn progress(&mut self, copied: u64) -> io::Result<()> {
        FileOperationManager::archive_checkpoint(self.op).map_err(|_| cancelled())?;
        // The copy reports from the start of the file again while it hashes the output.
        if copied > self.reported {
            FileOperationManager::archive_progress(self.op, copied - self.reported);
            self.reported = copied;
        }
        Ok(())
    }
    fn finished(&mut self) {
        self.op.lock().unwrap().processed_files += 1;
    }
}

/// Copies each file checked against its recorded hash; on an error, removes the outputs
/// already published, and only those whose content and identity are still theirs.
pub(crate) fn copy_files(
    files: RedoCopyFiles,
    observer: &mut dyn CopyObserver,
) -> Result<RedoCopyIdentities, CommandError> {
    let mut published: Vec<(PathBuf, String, Option<HistoryFileIdentity>)> = Vec::new();
    let mut identities = Vec::new();
    for (source, destination, expected) in files {
        observer.start(&source);
        let result = (|| {
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent)?;
            }
            crate::systems::file_copy::copy_verified(&source, &destination, &expected, &mut |copied| {
                observer.progress(copied)
            })
        })();
        match result {
            Ok((hash, identity)) => {
                if let Some(identity) = identity {
                    identities.push((destination.to_string_lossy().into_owned(), identity));
                }
                published.push((destination, hash, identity));
                observer.finished();
            }
            Err(error) => {
                let rollback_errors: Vec<_> = published
                    .iter()
                    .rev()
                    .filter_map(|(path, hash, identity)| {
                        crate::systems::file_copy::remove_verified_output_identity(path, hash, *identity)
                            .err()
                            .map(|error| format!("{}: {error}", path.display()))
                    })
                    .collect();
                return Err(if rollback_errors.is_empty() {
                    CommandError::from(error)
                } else {
                    CommandError::IoError(format!(
                        "{error}; redo copy rollback incomplete: {}",
                        rollback_errors.join("; ")
                    ))
                });
            }
        }
    }
    Ok(identities)
}
