use serde::Serialize;
use std::fmt;

#[derive(Debug, Serialize)]
pub enum CommandError {
    IoError(String),
    PathError(String),
    SystemError(String),
    ArchiveError(String),
    TrashError(String),
    Other(String),
}

impl std::error::Error for CommandError {}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::IoError(msg) => write!(f, "IO Error: {}", msg),
            CommandError::PathError(msg) => write!(f, "Path Error: {}", msg),
            CommandError::SystemError(msg) => write!(f, "System Error: {}", msg),
            CommandError::ArchiveError(msg) => write!(f, "Archive Error: {}", msg),
            CommandError::TrashError(msg) => write!(f, "Trash Error: {}", msg),
            CommandError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl From<std::io::Error> for CommandError {
    fn from(err: std::io::Error) -> Self {
        CommandError::IoError(err.to_string())
    }
}

impl From<String> for CommandError {
    fn from(err: String) -> Self {
        CommandError::Other(err)
    }
}

impl From<&str> for CommandError {
    fn from(err: &str) -> Self {
        CommandError::Other(err.to_string())
    }
}

// Helper for convenient error creation
impl CommandError {
    pub fn new(msg: impl Into<String>) -> Self {
        CommandError::Other(msg.into())
    }
}
