pub mod error;
pub mod file_entry;
pub mod session;
pub mod progress;
pub mod transaction;
pub mod history;

pub use error::CommandError;
pub type Result<T> = std::result::Result<T, CommandError>;

pub use file_entry::{FileEntry, FileProperties, ShortcutInfo, FileSummary, FolderSizeResult, DriveInfo, WinMenuItem, QuickAccessItem, ConflictEntry, ConflictResponse, TrashEntry, SidebarNode, SnapRect, NetResource, get_file_entry_from_path};
pub use session::{SessionState, SessionManager, Tab};
pub use config::{AppConfig, ConfigManager};
pub use progress::ProgressEvent;
pub use transaction::{Transaction, TransactionType, TransactionDetails};
pub use history::HistoryManager;

pub mod config;
