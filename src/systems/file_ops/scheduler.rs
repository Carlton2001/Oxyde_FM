use super::{
    FileOpType, FileOperation, FileOperationManager, OpStatus, OperationScheduler, ScheduledOperation,
};
use crate::models::HistoryManager;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

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

    pub(super) fn paths_overlap(left: &[String], right: &[String]) -> bool {
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
}
