use super::*;
use crate::models::HistoryManager;

fn operation() -> Arc<Mutex<FileOperation>> {
    Arc::new(Mutex::new(FileOperation::new(
        FileOpType::Move,
        Vec::new(),
        None,
        None,
    )))
}


mod archive_preparation;
mod progress;
mod archives;
mod scheduler;
mod archive_moves;
mod native_copy;
mod copy_history;
mod move_history;
mod transfer_safety;
