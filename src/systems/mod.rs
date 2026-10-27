pub(crate) mod archive_workspace;
pub(crate) mod archive_recovery;
#[cfg(all(target_os = "windows", not(test)))]
pub(crate) mod archive_io;
#[cfg(all(target_os = "windows", not(test)))]
mod archive_encode;
#[cfg(target_os = "windows")]
pub(crate) mod archive_owned_tree;
mod archive_move_sources;
pub mod file_ops;
pub(crate) mod transfer_naming;
pub(crate) mod file_copy;
pub(crate) mod copy_diagnostics;
#[doc(hidden)]
pub mod copy_benchmark;
mod copied_source;
mod destination_tree;
mod source_directories;
mod source_selection;
mod source_path_set;
mod compression_sources;
mod compression_output;
pub(crate) mod history_move;
pub(crate) mod history_move_redo;
pub(crate) mod history_file_undo;
pub(crate) mod history_links;
mod link_moves;
pub(crate) mod history_directories;
pub(crate) mod history_directory_attributes;
pub(crate) mod history_renamed_dirs;
pub(crate) mod item_rename;
pub(crate) mod new_folder_history;
pub mod quick_access_watcher;
pub mod updates;

pub(crate) mod folder_sizes;

pub(crate) mod path_parents;
pub(crate) mod archive_open_cache;

#[cfg(target_os = "windows")]
pub(crate) mod history_delete;
