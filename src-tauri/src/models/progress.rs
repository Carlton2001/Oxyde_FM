
use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct ProgressEvent {
    pub id: String, // Unique ID for the operation (optional, or use "global")
    pub task: String, // "copy", "move", "delete", "calculate_size"
    pub current: u64,
    pub total: u64,
    pub status: String, // "Running", "Completed", "Error"
    pub filename: Option<String>,
}
