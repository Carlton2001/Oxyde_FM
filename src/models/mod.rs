pub mod error;
pub mod file_entry;
pub mod history;
pub mod preferences;
pub mod progress;
pub mod sort;
pub mod transaction;

pub use error::CommandError;
pub type Result<T> = std::result::Result<T, CommandError>;

pub use config::{AppConfig, ConfigManager, TrashDriveSettings};
pub use file_entry::{
    get_file_entry_from_path, ConflictAction, ConflictEntry, ConflictResponse, DriveInfo,
    FileEntry, FileProperties, FileSummary, FolderSizeResult, NetResource, QuickAccessItem,
    ShortcutInfo, SidebarNode, TrashEntry, WinMenuItem,
};
pub use history::HistoryManager;
pub use progress::ProgressEvent;
pub use transaction::{Transaction, TransactionDetails, TransactionType};

pub mod config;
