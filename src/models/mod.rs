pub mod error;
pub mod file_entry;
pub mod history;
pub mod progress;
pub mod preferences;
pub mod session;
pub mod transaction;

pub use error::CommandError;
pub type Result<T> = std::result::Result<T, CommandError>;

pub use config::{AppConfig, ConfigManager, TrashDriveSettings};
pub use file_entry::{
    get_file_entry_from_path, ConflictAction, ConflictEntry, ConflictResponse, DriveInfo,
    FileEntry, FileProperties, FileSummary, FolderSizeResult, NetResource, QuickAccessItem,
    ShortcutInfo, SidebarNode, SnapRect, TrashEntry, WinMenuItem,
};
pub use history::HistoryManager;
pub use progress::ProgressEvent;
pub use session::{SessionManager, SessionState, Tab};
pub use transaction::{Transaction, TransactionDetails, TransactionType};

pub mod config;
