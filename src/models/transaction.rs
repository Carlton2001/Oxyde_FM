use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionType {
    Copy,
    Move,
    Rename,
    Delete,
    NewFolder,
    Restore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionDetails {
    pub paths: Vec<String>,
    pub target_dir: Option<String>,
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub created_files: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub timestamp: i64,
    pub op_type: TransactionType,
    pub details: TransactionDetails,
    // Future: backup references for safe undo?
}

impl Transaction {
    pub fn new(op_type: TransactionType, details: TransactionDetails) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            op_type,
            details,
        }
    }
}
