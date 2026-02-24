use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::SystemTime;
use crate::models::CommandError;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_hidden: bool,
    pub is_system: bool,
    pub is_symlink: bool,
    pub is_junction: bool,
    pub size: u64,
    pub modified: u64,
    pub is_readonly: bool,
    pub is_calculated: bool,
    pub original_path: Option<String>,
    pub deleted_time: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutInfo {
    pub target: String,
    pub arguments: String,
    pub working_dir: String,
    pub description: String,
    pub icon_location: String,
    pub icon_index: i32,
    pub run_window: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileProperties {
    pub name: String,
    pub path: String,
    pub parent: String,
    pub is_dir: bool,
    pub size: u64,
    pub is_calculated: bool,
    pub created: u64,
    pub modified: u64,
    pub accessed: u64,
    pub readonly: bool,
    pub is_hidden: bool,
    pub is_system: bool,
    pub original_path: Option<String>,
    pub deleted_time: Option<i64>,
    pub folders_count: Option<u64>,
    pub files_count: Option<u64>,
    pub shortcut: Option<ShortcutInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSummary {
    pub count: usize,
    pub total_size: u64,
    pub files_count: usize,
    pub folders_count: usize,
    pub all_readonly: bool,
    pub any_readonly: bool,
    pub all_hidden: bool,
    pub any_hidden: bool,
    pub parent_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderSizeResult {
    pub size: u64,
    pub folders_count: u64,
    pub files_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarNode {
    pub name: String,
    pub path: String,
    pub is_hidden: bool,
    pub is_system: bool,
    pub is_readonly: bool,
    pub has_subdirs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveInfo {
    pub path: String,
    pub label: String,
    pub drive_type: String,
    pub is_readonly: bool,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub media_type: Option<String>,
    pub physical_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickAccessItem {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WinMenuItem {
    pub id: i32,
    pub label: String,
    pub verb: Option<String>,
    pub has_submenu: bool,
    pub children: Vec<WinMenuItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictEntry {
    pub name: String,
    pub source: FileEntry,
    pub target: FileEntry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictResponse {
    pub conflicts: Vec<ConflictEntry>,
    pub total_size: u64,
    pub total_files: usize,
    pub is_cross_volume: bool,
    pub likely_large: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashEntry {
    pub name: String,
    pub original_path: String,
    pub deleted_time: u64,
    pub is_dir: bool,
    pub size: u64,
    pub path: String,
    pub modified: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SnapRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub fn get_file_entry_from_path(path: &Path) -> Result<FileEntry, CommandError> {
    // Use symlink_metadata to get attributes of the link itself (e.g. Hidden/System on 'Documents and Settings')
    let metadata = std::fs::symlink_metadata(path).map_err(|e| CommandError::IoError(e.to_string()))?;
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    
    let modified = metadata.modified()
        .unwrap_or(SystemTime::UNIX_EPOCH)
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let (is_hidden, is_system, is_reparse_point) = crate::utils::get_file_attributes(&metadata, &name);
    
    let is_readonly = metadata.permissions().readonly();
    
    // For is_dir, we want to know if it behaves like a directory.
    // On Windows, a Junction/SymlinkDir has the Directory attribute in its symlink_metadata too.
    let is_dir = metadata.is_dir(); 
    
    let is_symlink = metadata.file_type().is_symlink();
    
    // On Windows, junctions are reparse points that are dirs but (historically) returned false for is_symlink in older Rust, 
    // but modern Rust might return true. 
    // Secure check: It is a junction if it is a reparse point, is a directory, and (is_symlink logic is variable).
    // Let's rely on our explicit reparse_point check.
    // If it is a symlink AND a dir, it is a symlink-to-dir. 
    // If it is a reparse point AND a dir AND NOT a symlink (strictly), it is a junction.
    // However, Rust's `is_symlink()` now often covers junctions too.
    // For UI purposes, we just want to know "Is it a fancy link?". 
    // We will separate them if possible, but identifying it as a System+Hidden item is the priority.
    
    let is_junction = is_reparse_point && is_dir && !is_symlink;

    // Retrieve size: if it's a file, get len. If symlink/junction, size is usually 0/irrelevant for listing.
    let size = if is_dir { 0 } else { metadata.len() };

    Ok(FileEntry {
        name,
        path: path.to_string_lossy().to_string(),
        is_dir,
        is_hidden,
        is_system,
        is_symlink,
        is_junction,
        size,
        modified,
        is_readonly,
        is_calculated: false,
        original_path: None,
        deleted_time: None,
    })
}
