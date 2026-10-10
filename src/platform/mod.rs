use crate::models::{DriveInfo, QuickAccessItem};

#[derive(Debug)]
pub enum NetworkDisconnectError {
    Busy,
    Other(String),
}

#[derive(Clone, Copy)]
pub enum ThreadPriority {
    Background,
    Normal,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct MountedDiskImage {
    pub image_path: String,
    pub roots: Vec<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PeekStatus {
    pub installed: bool,
    pub enabled: bool,
    pub space_enabled: bool,
    pub activation_shortcut: Option<String>,
}

/// Native identity of a selected trash payload, resolved on a background worker.
#[derive(Debug, Clone)]
pub struct TrashTransferEntry {
    pub path: std::path::PathBuf,
    pub original_path: std::path::PathBuf,
    pub metadata_path: std::path::PathBuf,
    pub metadata_identity: crate::models::transaction::HistoryFileIdentity,
    pub metadata_hash: String,
}

/// A detached sidecar remains bound to its captured contents and object.
#[derive(Debug)]
pub struct DetachedTrashMetadata {
    pub source: std::path::PathBuf,
    pub backup: std::path::PathBuf,
    pub identity: crate::models::transaction::HistoryMoveIdentity,
    pub hash: String,
}

#[derive(Clone, Copy)]
pub enum SystemExecutable {
    PowerShell,
    Explorer,
}

/// Opaque identity of an opened file or directory. Keep a handle alive while comparing it
/// across an operation so a deleted file's identifier cannot be reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileIdentity {
    volume: u64,
    value: [u8; 16],
    kind: u8,
}

/// The item receipt is independent of a later failure of the native operation.
#[derive(Debug)]
pub struct ShellDeleteOutcome {
    pub completed: bool,
    pub error: Option<String>,
}

impl ShellDeleteOutcome {
    pub fn from_result(result: Result<(), String>) -> Self {
        Self { completed: result.is_ok(), error: result.err() }
    }
}

/// Abstraction des opérations spécifiques à l'OS.
/// Toute fonctionnalité native (lecteurs, ouverture, raccourcis, etc.)
/// doit passer par ce trait plutôt qu'appeler directement les API OS.
pub trait Platform: Send + Sync {
    fn list_archive_directory(&self, path: &std::path::Path, internal: &str) -> Result<Vec<crate::models::FileEntry>, crate::models::CommandError>;
    fn initialize_preview_worker(&self) -> std::io::Result<()> { Ok(()) }
    #[doc(hidden)]
    fn decode_preview_worker(&self, _mode: &str, _path: &std::path::Path) -> crate::utils::preview_protocol::Reply {
        crate::utils::preview_protocol::Reply::Error {limit:false,detail:"Native preview worker unavailable".into()}
    }
    #[doc(hidden)]
    fn decode_video_worker(&self, _path: String, _on_frame: &mut dyn FnMut(image::RgbaImage,std::time::Duration) -> bool) -> Result<(),crate::models::CommandError> {
        Err(crate::models::CommandError::Other("Native video worker unavailable".into()))
    }
    fn stream_preview_worker(&self, _path: &std::path::Path, _on_frame: &mut dyn FnMut(image::RgbaImage, std::time::Duration) -> bool, _cancelled: &dyn Fn() -> bool) -> std::io::Result<()> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "Video preview worker unavailable"))
    }
    /// Windows executes bounded preview parsing in a hidden resource-limited process.
    fn run_preview_worker(&self, _mode: &str, _path: &std::path::Path) -> std::io::Result<Vec<u8>> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "Preview worker unavailable on this platform"))
    }
    /// Snapshot mode denies source writes and replacement on Windows. Tracking
    /// mode keeps the identity alive while permitting subsequent external edits.
    fn open_archive_source(&self, path: &std::path::Path, allow_changes: bool) -> std::io::Result<std::fs::File>;
    fn file_identity(&self, file: &std::fs::File) -> std::io::Result<FileIdentity>;
    fn system_executable(&self, executable: SystemExecutable) -> std::io::Result<std::path::PathBuf>;
    /// Holds an ordinary directory without following a reparse point. On Windows
    /// the handle denies deletion/renaming until it is dropped.
    fn hold_archive_directory(&self, path: &std::path::Path) -> std::io::Result<std::fs::File>;
    /// On Windows deny parent renames. Existing reparse directories also deny
    /// data writers, but attribute-only reparse mutation remains possible.
    /// Ordinary directories allow write sharing for native child renames.
    fn hold_operation_directory(&self, path: &std::path::Path) -> std::io::Result<std::fs::File>;
    /// Open a resolved ordinary parent, rejecting reparse traversal atomically on Windows.
    fn hold_resolved_operation_directory(&self, path: &std::path::Path) -> std::io::Result<std::fs::File>;
    /// Ensure one ordinary child relative to the held parent on Windows. The
    /// boolean reports whether this native call created it, without an exists race.
    fn ensure_operation_directory(&self, parent: &std::fs::File, path: &std::path::Path) -> std::io::Result<(std::fs::File, bool)>;
    /// Track an ordinary source directory by identity, allowing subsequent child moves.
    fn track_operation_directory(&self, parent: &std::fs::File, path: &std::path::Path) -> std::io::Result<std::fs::File>;
    /// Exclusively create a journal directory, retaining metadata access without
    /// blocking child moves. Windows resolves only the child of the held parent.
    fn create_journal_directory(&self, parent: &std::fs::File, path: &std::path::Path, hidden: bool) -> std::io::Result<std::fs::File>;
    /// Restore readonly/hidden on the created object, rather than resolving its name.
    fn restore_journal_directory(&self, directory: &std::fs::File, path: &std::path::Path, permissions: std::fs::Permissions, hidden: bool) -> std::io::Result<()>;
    /// On Windows reopen one child without reparse traversal, verify the retained
    /// identity, and delete by handle only if the kernel finds it empty.
    fn remove_empty_operation_directory(&self, parent: &std::fs::File, path: &std::path::Path, expected: FileIdentity) -> std::io::Result<()>;
    /// Open the basename relative to the retained parent, without following reparse objects.
    fn open_operation_file(&self, parent: &std::fs::File, path: &std::path::Path, read_contents: bool) -> std::io::Result<std::fs::File>;
    /// Inspect one child of a bound source parent without following its final link.
    fn track_source_item(&self, parent: &std::fs::File, path: &std::path::Path) -> std::io::Result<std::fs::File>;
    /// Read/list an ordinary source directory, preventing its rename on Windows.
    fn open_source_directory(&self, parent: &std::fs::File, path: &std::path::Path) -> std::io::Result<std::fs::File>;
    /// Read-only snapshot of a bound source; Copy does not require DELETE access.
    fn open_copy_source(&self, parent: &std::fs::File, path: &std::path::Path) -> std::io::Result<std::fs::File>;
    /// Enumerate the opened directory itself. Each invocation starts at its beginning.
    fn enumerate_source_directory(&self, directory: &std::fs::File, visitor: &mut dyn FnMut(std::ffi::OsString) -> std::io::Result<()>) -> std::io::Result<()>;
    /// Hold a rename source, including directories and the reparse entry itself.
    /// Windows prevents replacement after opening and resolves only the held parent's child.
    fn open_rename_item(
        &self,
        parent: &std::fs::File,
        path: &std::path::Path,
    ) -> std::io::Result<std::fs::File>;
    /// Rename into a held directory without replacing an existing destination.
    fn rename_operation_item(
        &self,
        file: &std::fs::File,
        source: &std::path::Path,
        source_parent: &std::fs::File,
        parent: &std::fs::File,
        destination: &std::path::Path,
    ) -> std::io::Result<()>;
    /// Hold a copied output for verification/publication and attribute-safe cleanup on Windows.
    fn open_copied_file(&self, parent: &std::fs::File, path: &std::path::Path) -> std::io::Result<std::fs::File>;
    /// Delete the held copy, clearing readonly only by handle and for a single link on Windows.
    fn remove_copied_file(&self, file: &std::fs::File, path: &std::path::Path) -> std::io::Result<()>;
    /// Rename by source identity into the retained destination parent on Windows.
    fn rename_operation_file(&self, file: &std::fs::File, source: &std::path::Path, parent: &std::fs::File, destination: &std::path::Path, replace: bool) -> std::io::Result<()>;
    /// Remove the selected object and its children by retained identities on Windows.
    /// Reparse entries are removed themselves; their targets are never traversed.
    fn remove_operation_tree(&self, parent: &std::fs::File, path: &std::path::Path, checkpoint: &mut dyn FnMut() -> std::io::Result<()>) -> std::io::Result<()>;
    /// Exclusively create hidden copy staging and a retained non-deletable marker on Windows.
    fn create_copy_staging(&self, parent: &std::fs::File, path: &std::path::Path) -> std::io::Result<(std::fs::File, std::fs::File)>;
    /// Create a private archive writer relative to staging; retain rename/delete
    /// access and exclude external writers/replacement on Windows.
    fn create_operation_output(&self, parent: &std::fs::File, path: &std::path::Path) -> std::io::Result<std::fs::File>;
    /// Remove only the staging marker and empty directory. Output cleanup needs a separately retained ownership proof.
    fn remove_copy_staging(&self, directory: &std::fs::File, pin: std::fs::File, path: &std::path::Path) -> std::io::Result<()>;
    fn lock_cache_file(&self, path: &std::path::Path) -> std::io::Result<std::fs::File>;
    fn read_cache_file(&self, path: &std::path::Path) -> std::io::Result<std::fs::File>;
    /// Publish an optional preview cache. Windows retains ordinary parents and
    /// the exclusively created file through write, rename and error cleanup.
    fn write_preview_cache(
        &self,
        path: &std::path::Path,
        temporary: &std::path::Path,
        write: &mut dyn FnMut(&mut std::fs::File) -> std::io::Result<()>,
    ) -> std::io::Result<()>;
    fn open_cache_cleanup(&self, path: &std::path::Path, directory: bool) -> std::io::Result<std::fs::File>;
    fn ensure_archive_directory(&self, path: &std::path::Path) -> std::io::Result<std::fs::File>;
    fn create_archive_file(&self, path: &std::path::Path) -> std::io::Result<std::fs::File>;
    /// Read/write bounded native Internet provenance; unsupported platforms use None.
    fn read_file_provenance(&self, path: &std::path::Path) -> std::io::Result<Option<Vec<u8>>>;
    fn write_file_provenance(&self, path: &std::path::Path, data: &[u8]) -> std::io::Result<()>;
    /// Blocking copy with native metadata/streams where supported. The callback
    /// Reports main-stream bytes; only an absent destination is accepted.
    /// A supplied source handle must already hold the same source path/identity.
    /// The destination parent handle must retain the same directory as the path.
    /// The creation observer receives the actual output before progress callbacks.
    /// It may duplicate that handle for ownership tracking.
    /// The progress callback may wait for pause or return an error to stop.
    fn copy_file_with_progress(
        &self,
        source: &std::path::Path,
        destination: &std::path::Path,
        held_source: Option<&std::fs::File>,
        destination_parent: &std::fs::File,
        created: &mut dyn FnMut(&std::fs::File) -> std::io::Result<()>,
        progress: &mut dyn FnMut(u64) -> std::io::Result<()>,
    ) -> std::io::Result<()>;
    /// Opens a regular file for a history move, keeping its identity stable.
    fn open_file_for_move(&self, path: &std::path::Path) -> std::io::Result<std::fs::File>;
    /// Renames the opened file without replacing an existing destination.
    fn rename_open_file_exclusive(
        &self,
        file: &std::fs::File,
        source: &std::path::Path,
        destination: &std::path::Path,
    ) -> std::io::Result<()>;
    /// Publishes a completed copy over an explicitly accepted existing target.
    fn replace_open_file(
        &self,
        file: &std::fs::File,
        source: &std::path::Path,
        destination: &std::path::Path,
    ) -> std::io::Result<()>;
    /// Removes the opened source after a verified copy has been published.
    fn remove_open_file(
        &self,
        file: &std::fs::File,
        path: &std::path::Path,
    ) -> std::io::Result<()>;
    /// Remove an opened ordinary directory only when the kernel finds it empty.
    /// Windows deletes that held object, without a path reopen or recursive removal.
    fn remove_open_empty_directory(
        &self,
        directory: &std::fs::File,
        path: &std::path::Path,
    ) -> std::io::Result<()>;
    /// Blocking native trash operations; callers must run them on a worker.
    fn list_trash_items(&self) -> Result<Vec<trash::TrashItem>, String>;
    fn restore_trash_items(&self, items: Vec<trash::TrashItem>) -> Result<(), String>;
    #[cfg(target_os = "windows")]
    fn restore_trash_history(&self, items: Vec<trash::TrashItem>, journal: &[crate::models::transaction::HistoryDeletedItem]) -> Result<(), String>;
    fn purge_trash_items(&self, items: Vec<trash::TrashItem>) -> Result<(), String>;
    fn empty_trash(&self) -> Result<(), String>;
    /// Prevent recursive sidebar expansion into OS-managed branches.
    fn bulk_expand_excluded(&self, path: &str, name: &str) -> bool;

    fn recycle_bin_usage(&self) -> Result<(u64, u64), String>;
    fn recycle_bin_config(&self, path: String) -> Result<crate::commands::ops::TrashSystemConfig, String>;
    fn set_recycle_bin_config(&self, config: crate::commands::ops::TrashSystemConfig) -> Result<(), String>;
    /// Read the original path from native trash metadata when the Shell name is opaque.
    fn trash_original_path(&self, id: &std::path::Path) -> Option<std::path::PathBuf>;

    /// Recognizes actual trash entries; ordinary file paths are omitted.
    fn trash_transfer_entries(&self, paths: &[std::path::PathBuf]) -> Result<Vec<TrashTransferEntry>, String>;
    /// Retain metadata for Undo only when the entire payload has left the trash.
    fn detach_trash_metadata(&self, entry: &TrashTransferEntry) -> Result<Option<DetachedTrashMetadata>, String>;

    /// Final process exit, called only after application cleanup and runtime shutdown.
    /// Windows bypasses DLL detach callbacks that can hang in native graphics drivers.
    fn finish_process_shutdown(&self) -> Result<(), String>;

    /// Load native icons on a background worker.
    fn file_icon(&self, path: String, large: bool) -> Result<image::RgbaImage, String>;

    /// Current user's regional formatting locale.
    fn regional_locale(&self) -> Option<String>;
    /// Identifies native time-zone settings, including daylight-saving rules.
    fn calendar_timezone_signature(&self) -> Option<String>;
    /// Current user's interface language, independent of regional date formats.
    fn interface_locale(&self) -> Option<String>;

    /// Returns hidden, system, and reparse point flags for a directory entry.
    fn file_attributes(&self, metadata: &std::fs::Metadata, name: &str) -> (bool, bool, bool);

    /// Mark an application-owned staging or undo-backup file as hidden.
    fn set_hidden_attribute(&self, path: &std::path::Path, hidden: bool) -> Result<(), String>;

    /// Adjusts the current worker thread's scheduling priority.
    fn set_thread_priority(&self, priority: ThreadPriority);

    /// Deletes files permanently or sends them to the system trash.
    fn shell_delete(&self, paths: &[std::path::PathBuf], recycle: bool) -> Result<(), String>;

    /// Windows obtains an item-level receipt, checking a known identity at dispatch.
    fn shell_delete_tracked(&self, path: &std::path::Path, _proof: Option<crate::models::transaction::HistoryFileIdentity>) -> ShellDeleteOutcome {
        ShellDeleteOutcome::from_result(self.shell_delete(&[path.to_path_buf()], true))
    }

    /// Lists entries in the operating system's network neighborhood.
    fn list_network_vicinity(&self) -> Result<Vec<crate::models::FileEntry>, String>;
    fn list_network_shares(&self, server: &str) -> Result<Vec<crate::models::FileEntry>, String>;

    fn is_network_path(&self, path: &std::path::Path) -> bool;
    /// Blocking network operations; call on a background worker.
    fn map_network_drive(&self, letter: String, path: String, reconnect: bool, username: Option<String>, password: Option<String>) -> Result<(), String>;
    fn disconnect_network_drive(&self, path: String, force: bool) -> Result<(), NetworkDisconnectError>;
    fn physical_disk_id(&self, path: &std::path::Path) -> u64;
    fn is_ssd(&self, path: &std::path::Path) -> bool;

    fn thumbnail(&self, path: String) -> Result<image::RgbaImage, crate::models::CommandError> {
        thumbnails::get_thumbnail_raw(path)
    }

    fn office_thumbnail(
        &self,
        path: String,
    ) -> Result<image::RgbaImage, crate::models::CommandError> {
        thumbnails::get_office_thumbnail_raw(path)
    }

    fn pdf_thumbnail(&self, path: String) -> Result<image::RgbaImage, crate::models::CommandError> {
        thumbnails::get_pdf_thumbnail_raw(path)
    }

    fn video_frames(
        &self,
        path: String,
        on_frame: Box<dyn FnMut(image::RgbaImage, std::time::Duration) -> bool + Send>,
        cancelled: Box<dyn Fn() -> bool + Send>,
    ) -> Result<(), crate::models::CommandError> {
        thumbnails::stream_video_frames_sync(path, on_frame, cancelled)
    }

    fn office_text_preview(&self, path: String) -> Result<String, crate::models::CommandError> {
        thumbnails::get_office_text_preview(path)
    }

    /// Liste les points de montage / lecteurs disponibles.
    fn list_drives(&self, skip_hardware_info: bool) -> Vec<DriveInfo>;
    /// Fast mount snapshot; detailed hardware/capacity queries run separately.
    fn list_drive_roots(&self) -> Vec<DriveInfo> { self.list_drives(true) }

    /// The returned guard owns the registration. Callbacks only increment revision.
    fn watch_drive_changes(&self, _revision: std::sync::Arc<std::sync::atomic::AtomicU64>)
        -> Result<Option<Box<dyn Send>>, String> { Ok(None) }
    fn eject_drive(&self, path: &str) -> Result<(), String>;
    /// Blocking disk-image operations; run on a background worker.
    fn mounted_disk_images(&self) -> Result<Vec<MountedDiskImage>, String>;
    fn mount_disk_image(&self, path: &str) -> Result<Vec<String>, String>;
    fn unmount_disk_image(&self, path: &str) -> Result<(), String>;
    /// Install on the native window's UI thread; never changes system preferences.
    fn install_disk_image_autoplay_handler(&self, window: raw_window_handle::RawWindowHandle) -> Result<(), String>;

    /// Ouvre un fichier ou dossier avec l'application par défaut de l'OS.
    fn open_item(&self, path: &str) -> Result<(), String>;

    /// Retourne les éléments de l'Accès Rapide (Windows) ou des Favoris GTK (Linux).
    fn get_quick_access_items(&self) -> Vec<QuickAccessItem>;

    fn pin_quick_access(&self, path: &str) -> Result<(), crate::models::CommandError> {
        let _ = path;
        Err(crate::models::CommandError::SystemError(
            "Quick access is only supported on Windows".into(),
        ))
    }
    fn unpin_quick_access(&self, path: &str) -> Result<(), crate::models::CommandError> {
        let _ = path;
        Err(crate::models::CommandError::SystemError(
            "Quick access is only supported on Windows".into(),
        ))
    }
    /// Best-effort native name of a virtual Shell item; ordinary properties keep their fallback.
    fn shell_item_friendly_name(&self, path: &str) -> Result<Option<String>, String> {
        let _ = path;
        Ok(None)
    }
    fn peek_status(&self) -> Result<PeekStatus, String>;
    fn open_peek(&self, path: &str) -> Result<(), String>;
    fn shortcut_info(&self, path: &std::path::Path) -> Option<crate::models::ShortcutInfo>;
    fn set_shortcut_info(&self, path: &str, info: &crate::models::ShortcutInfo) -> Result<(), String>;
    /// The returned guard owns the watcher. All these calls run in the background.
    fn watch_quick_access_changes(&self, revision: std::sync::Arc<std::sync::atomic::AtomicU64>)
        -> Result<Option<Box<dyn Send>>, String>;

    /// Affiche la boîte de dialogue propriétés native pour un fichier/dossier.
    fn show_properties_dialog(&self, path: &str);

    /// Définit l'icône de l'application pour la barre des tâches et la fenêtre.
    fn set_application_icon(&self, window: raw_window_handle::RawWindowHandle, icon_bytes: &[u8]);

    /// Transfers selected file paths through the native file clipboard.
    fn set_clipboard_files(&self, paths: Vec<String>, cut: bool) -> Result<(), String>;

    /// Reads file paths and their copy/move intent from the native clipboard.
    fn get_clipboard_files(&self) -> Result<(Vec<String>, bool), String>;

    /// Checks whether the supported file-transfer format is advertised, without
    /// opening or reading the clipboard. Paste must still validate its payload.
    fn clipboard_has_file_format(&self) -> bool;

    /// Builds a plain-data snapshot of an OS context menu for a file or folder.
    fn native_context_menu_items(
        &self,
        path: &str,
        is_background: bool,
    ) -> Result<Vec<crate::models::WinMenuItem>, String> {
        let _ = (path, is_background);
        Err("Native context menus are unavailable on this platform".to_string())
    }

    /// Executes a command ID from a previously built native context-menu snapshot.
    fn execute_native_context_menu_item(
        &self,
        path: &str,
        id: i32,
        is_background: bool,
    ) -> Result<(), String> {
        let _ = (path, id, is_background);
        Err("Native context menus are unavailable on this platform".to_string())
    }
}

#[cfg(target_os = "windows")]
pub mod clipboard;
#[cfg(target_os = "windows")]
mod windows_clipboard_write;
mod hardware;
#[cfg(target_os = "windows")]
pub mod icons;
#[cfg(not(target_os = "windows"))]
mod linux_impl;
#[cfg(target_os = "windows")]
pub mod network;
mod thumbnails;
#[cfg(target_os = "windows")]
mod windows_disk_images;
#[cfg(target_os = "windows")]
mod windows_drives;
#[cfg(target_os = "windows")]
mod windows_impl;
#[cfg(target_os = "windows")]
mod windows_executables;
#[cfg(target_os = "windows")]
mod windows_archive_security;
#[cfg(target_os = "windows")]
mod windows_cache;
#[cfg(target_os = "windows")]
mod windows_no_reparse;
#[cfg(target_os = "windows")]
mod windows_com;
#[cfg(target_os = "windows")]
mod windows_thumbnail_bitmap;
#[cfg(target_os = "windows")]
mod windows_icon_bitmap;
#[cfg(target_os = "windows")]
mod windows_icon_cache;
#[cfg(target_os = "windows")]
mod windows_preview_cache;
#[cfg(target_os = "windows")]
mod windows_preview_quota;
#[cfg(target_os = "windows")]
mod windows_pdf_thumbnail;
#[cfg(target_os = "windows")]
mod windows_file_moves;
#[cfg(target_os = "windows")]
mod windows_file_tree;
#[cfg(target_os = "windows")]
mod windows_copy_staging;
#[cfg(all(test, target_os = "windows"))]
pub(crate) mod windows_reparse_tests;
#[cfg(target_os = "windows")]
mod windows_archive_source;
#[cfg(target_os = "windows")]
mod windows_file_copy;
#[cfg(target_os = "windows")]
mod windows_shell;
#[cfg(target_os = "windows")]
mod windows_shell_properties;
#[cfg(target_os = "windows")]
mod windows_trash;

#[cfg(not(target_os = "windows"))]
use linux_impl::LinuxPlatform as CurrentPlatform;
#[cfg(target_os = "windows")]
use windows_impl::WindowsPlatform as CurrentPlatform;

/// Retourne l'implémentation platform courante (singleton).
pub fn platform() -> &'static dyn Platform {
    use std::sync::OnceLock;
    static INSTANCE: OnceLock<CurrentPlatform> = OnceLock::new();
    INSTANCE.get_or_init(CurrentPlatform::new)
}

/// Internal worker dispatch; never called by the UI to bypass process isolation.
#[cfg(target_os = "windows")]
pub fn local_preview_worker_reply(mode: &str, path: &std::path::Path) -> crate::utils::preview_protocol::Reply {
    platform().decode_preview_worker(mode, path)
}

#[cfg(target_os = "windows")]
pub fn local_video_preview_worker(path: String, mut on_frame: impl FnMut(image::RgbaImage,std::time::Duration) -> bool) -> Result<(), crate::models::CommandError> {
    platform().decode_video_worker(path, &mut on_frame)
}

#[cfg(target_os = "windows")]
mod windows_peek;
#[cfg(target_os = "windows")]
mod windows_shortcuts;

#[cfg(target_os = "windows")]
mod windows_trash_clipboard;

#[cfg(target_os = "windows")]
mod windows_source_selection;
#[cfg(target_os = "linux")]
mod linux_source_selection;

#[cfg(target_os = "windows")]
mod windows_trash_restore;
#[cfg(target_os = "windows")]
mod windows_shell_delete;
#[cfg(target_os = "windows")]
mod windows_shell_delete_tracked;
#[cfg(target_os = "windows")]
mod windows_preview_worker;
#[cfg(target_os = "windows")]
mod windows_archive_worker;
#[cfg(target_os = "windows")]
mod windows_icon_pixels;
