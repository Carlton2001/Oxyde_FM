use super::{
    FileOpType, FileOperation, FileOperationManager, OpStatus, OperationScheduler, ScheduledOperation,
};
use crate::models::HistoryManager;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

/// Reserved paths indexed to tell, in linear time, whether others equal one, lie under one
/// or hold one: the lists can hold tens of thousands of files.
struct PathSet<'a> {
    exact: HashSet<&'a str>,
    /// Every folder above a path, up to its root.
    ancestors: HashSet<&'a str>,
}

impl<'a> PathSet<'a> {
    fn new(paths: &'a [String]) -> Self {
        let mut ancestors = HashSet::new();
        for path in paths {
            for (index, _) in path.match_indices('/') {
                ancestors.insert(&path[..index]);
            }
        }
        Self { exact: paths.iter().map(String::as_str).collect(), ancestors }
    }

    fn overlaps(&self, others: &[String]) -> bool {
        others.iter().any(|path| {
            self.exact.contains(path.as_str())
                || self.ancestors.contains(path.as_str())
                || path.match_indices('/').any(|(index, _)| self.exact.contains(&path[..index]))
        })
    }
}

/// Releases an operation's reservation and wakes the others, even if its task panics.
struct Reservation {
    scheduler: Arc<OperationScheduler>,
    id: String,
}

impl Drop for Reservation {
    fn drop(&mut self) {
        let mut waiting = self.scheduler.waiting.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        waiting.remove(&self.id);
        drop(waiting);
        self.scheduler
            .running_paths
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&self.id);
        self.scheduler.changed.notify_waiters();
    }
}

impl OperationScheduler {
    pub(super) fn enqueue(&self, operation: Arc<Mutex<FileOperation>>, paths: Vec<String>) {
        let id = operation.lock().unwrap().id.clone();
        self.waiting
            .lock()
            .unwrap()
            .insert(id, ScheduledOperation { operation, paths });
        self.changed.notify_waiters();
    }

    pub(super) async fn acquire(&self, id: &str) -> Option<tokio::sync::OwnedSemaphorePermit> {
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
                // Built once per pass: each waiting operation is then compared in linear time.
                let running_sets: Vec<(&String, PathSet)> = running
                    .iter()
                    .map(|(active_id, paths)| (active_id, PathSet::new(paths)))
                    .collect();
                let next = waiting
                    .iter()
                    .filter_map(|(candidate_id, entry)| {
                        let operation = entry.operation.lock().unwrap();
                        if operation.cancel_flag.load(Ordering::Relaxed)
                            || FileOperationManager::is_final(&operation.status)
                            || (operation.pause_flag.load(Ordering::Relaxed)
                                && !operation.resume_requested)
                            || running_sets.iter().any(|(active_id, set)| {
                                *active_id != candidate_id && set.overlaps(&entry.paths)
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
                            operation.pause_clock(false);
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

impl FileOperationManager {
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
            let _reservation = Reservation { scheduler: scheduler.clone(), id: scheduled_id.clone() };
            let mut permit = Some(initial_permit);
            let operation = op_clone.clone();
            let mut execution = Box::pin(async move {
                // A Redo checked its paths and made its folders before queuing its copy.
                let redo = operation.lock().unwrap().redo_copy.is_some();
                let checked = match Self::check_representable(&operation) {
                    _ if redo => Ok(()),
                    Ok(()) => Self::check_queued_conflicts(operation.clone()).await,
                    Err(error) => Err(error),
                };
                match checked {
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
                        locked.pause_clock(false);
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
            drop(permit);
            // `_reservation` releases the paths and wakes the others as it drops.
        });

        op_id
    }

    pub(super) fn reserved_paths(op: &FileOperation) -> Vec<String> {
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
                // Conservative lexical reservation only, not archive routing:
                // over-reserving an archive-named directory is safe and avoids
                // filesystem I/O while queue_operation_on runs on the UI thread.
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
                        // `\\?\C:\x` and `C:\x` reserve the same path.
                        std::path::Component::Prefix(prefix) => parts.push(match prefix.kind() {
                            std::path::Prefix::Disk(letter) | std::path::Prefix::VerbatimDisk(letter) => {
                                format!("{}:", char::from(letter))
                            }
                            std::path::Prefix::UNC(server, share)
                            | std::path::Prefix::VerbatimUNC(server, share) => format!(
                                "//{}/{}",
                                server.to_string_lossy(),
                                share.to_string_lossy()
                            ),
                            _ => prefix.as_os_str().to_string_lossy().into_owned(),
                        }),
                        other => parts.push(other.as_os_str().to_string_lossy().into_owned()),
                    }
                }
                parts.join("/").to_lowercase()
            })
            .collect()
    }

    #[cfg(test)]
    pub(super) fn paths_overlap(left: &[String], right: &[String]) -> bool {
        PathSet::new(right).overlaps(left)
    }

    /// Names that are not valid Unicode are refused before any effect: the operations
    /// below rebuild paths from text and journal them as text.
    fn check_representable(op: &Arc<Mutex<FileOperation>>) -> Result<(), String> {
        let locked = op.lock().unwrap();
        let paths: Vec<_> = locked
            .sources
            .iter()
            .chain(locked.destination.iter())
            // Text or already decoded, such a name is refused alike.
            .map(|path| crate::utils::path_text::encode(path))
            .collect();
        crate::models::error::UnrepresentablePathFailure::check(paths.iter().map(String::as_str))
            .map_err(|failure| failure.to_string())
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
}
