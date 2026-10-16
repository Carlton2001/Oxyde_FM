use super::*;
use serde::{Deserialize, Serialize};

type Operations = Arc<Mutex<HashMap<String, Arc<Mutex<FileOperation>>>>>;

#[cfg(test)]
#[path = "tests/persistence.rs"]
mod tests;

#[derive(Serialize, Deserialize)]
struct Record {
    #[serde(default)]
    item_outcomes_version: u8,
    id: String,
    sequence: u64,
    op_type: FileOpType,
    sources: Vec<PathBuf>,
    destination: Option<PathBuf>,
    status: OpStatus,
    total_bytes: u64,
    processed_bytes: u64,
    total_files: usize,
    processed_files: usize,
    current_file: Option<String>,
    duration_ms: Option<u64>,
    speed_history: Vec<u64>,
    #[serde(default)]
    item_outcomes: std::collections::BTreeMap<PathBuf, ItemOutcome>,
}

type HistoryKey = Vec<(String, u64, OpStatus, Option<u64>, usize)>;
type SelectedOperations = Vec<Arc<Mutex<FileOperation>>>;

/// Final records are immutable through the manager API. Dismissal, replacement,
/// final status changes and a new completion invalidate this lightweight key.
/// Never clone sources/outcomes just to discover that nothing changed.
fn selected_operations(operations: &Operations) -> (HistoryKey, SelectedOperations) {
    let references: Vec<_> = operations.lock().unwrap().values().cloned().collect();
    let mut finals: Vec<_> = references
        .into_iter()
        .filter_map(|operation| {
            let op = operation.lock().unwrap();
            FileOperationManager::is_final(&op.status).then(|| {
                (
                    (
                        op.id.clone(),
                        op.sequence,
                        op.status.clone(),
                        op.duration_ms,
                        Arc::as_ptr(&operation) as usize,
                    ),
                    operation.clone(),
                )
            })
        })
        .collect();
    finals.sort_by(|(a, _), (b, _)| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
    let keep = finals.len().saturating_sub(10);
    finals.into_iter().skip(keep).unzip()
}

fn encode_selected(operations: &SelectedOperations) -> Result<Vec<u8>, serde_json::Error> {
    let records: Vec<_> = operations
        .iter()
        .filter_map(|op| {
            let op = op.lock().unwrap();
            FileOperationManager::is_final(&op.status).then(|| Record {
                item_outcomes_version: 2,
                id: op.id.clone(),
                sequence: op.sequence,
                op_type: op.op_type.clone(),
                sources: op.sources.clone(),
                destination: op.destination.clone(),
                status: op.status.clone(),
                total_bytes: op.total_bytes,
                processed_bytes: op.processed_bytes,
                total_files: op.total_files,
                processed_files: op.processed_files,
                current_file: op.display_current_file(),
                duration_ms: op.duration_ms,
                speed_history: op.speed_history.clone(),
                item_outcomes: op.snapshot_item_outcomes().into_iter().collect(),
            })
        })
        .collect();
    // Both manager and per-operation locks have been released before JSON work.
    serde_json::to_vec_pretty(&records)
}

#[cfg(test)]
fn encode(operations: &Operations) -> Result<Vec<u8>, serde_json::Error> {
    encode_selected(&selected_operations(operations).1)
}

pub(super) struct OperationPersistence {
    stop: std::sync::mpsc::Sender<()>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Drop for OperationPersistence {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl FileOperationManager {
    pub fn with_persistent_history() -> Self {
        let Ok(config) = crate::models::ConfigManager::config_path() else {
            return Self::new();
        };
        Self::with_history_path(config.with_file_name("operation_history.json"))
    }

    fn with_history_path(path: PathBuf) -> Self {
        let mut manager = Self::new();
        match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Vec<Record>>(&bytes) {
                Ok(mut records) => {
                    records.retain(|record| Self::is_final(&record.status));
                    records.sort_by_key(|record| record.sequence);
                    let keep = records.len().saturating_sub(10);
                    for record in records.into_iter().skip(keep) {
                        let mut op = FileOperation::new(
                            record.op_type,
                            record.sources,
                            record.destination,
                            None,
                        );
                        op.id = record.id;
                        op.sequence = record.sequence;
                        op.status = record.status;
                        op.total_bytes = record.total_bytes;
                        op.processed_bytes = record.processed_bytes;
                        op.total_files = record.total_files;
                        op.processed_files = record.processed_files;
                        op.current_file = record.current_file;
                        op.duration_ms = record.duration_ms;
                        op.speed_history = record.speed_history;
                        op.item_outcomes = record
                            .item_outcomes
                            .into_iter()
                            .map(|(path, outcome)| {
                                let outcome = if record.item_outcomes_version < 2
                                    && outcome == ItemOutcome::Partial
                                {
                                    ItemOutcome::Unknown
                                } else {
                                    outcome
                                };
                                (path, outcome)
                            })
                            .collect();
                        op.item_progress.clear();
                        manager
                            .next_sequence
                            .fetch_max(op.sequence.saturating_add(1), Ordering::Relaxed);
                        manager
                            .operations
                            .lock()
                            .unwrap()
                            .insert(op.id.clone(), Arc::new(Mutex::new(op)));
                    }
                }
                Err(error) => log::warn!("Cannot load operation history: {error}"),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => log::warn!("Cannot read operation history: {error}"),
        }
        let operations = manager.operations.clone();
        let (stop, receiver) = std::sync::mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("operation-history".into())
            .spawn(move || {
                let mut previous = Vec::new();
                let mut previous_key = None;
                loop {
                    let stopping = !matches!(
                        receiver.recv_timeout(std::time::Duration::from_millis(100)),
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                    );
                    let (key, selected) = selected_operations(&operations);
                    if !stopping && previous_key.as_ref() == Some(&key) {
                        continue;
                    }
                    if let Ok(bytes) = encode_selected(&selected) {
                        if bytes != previous {
                            let result = (|| -> std::io::Result<()> {
                                if let Some(parent) = path.parent() {
                                    std::fs::create_dir_all(parent)?;
                                }
                                let temporary = path.with_extension("json.tmp");
                                std::fs::write(&temporary, &bytes)?;
                                std::fs::rename(temporary, &path)
                            })();
                            match result {
                                Ok(()) => {
                                    previous = bytes;
                                    previous_key = Some(key);
                                }
                                Err(error) => {
                                    log::warn!("Cannot save operation history: {error}");
                                }
                            }
                        } else {
                            previous_key = Some(key);
                        }
                    }
                    if stopping {
                        break;
                    }
                }
            });
        match worker {
            Ok(worker) => {
                manager.persistence = Some(OperationPersistence {
                    stop,
                    worker: Some(worker),
                })
            }
            Err(error) => {
                log::warn!("Cannot start operation history writer: {error}");
            }
        }
        manager
    }
}
