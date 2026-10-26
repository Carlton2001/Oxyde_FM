use crate::models::QuickAccessItem;
mod file_names;
mod file_drag;
pub use file_drag::{FileDragFeedback, FileDropEffect, FileDropPhase, NativeFileDrop};
mod viewer_media;
mod memory;
pub use memory::ProcessMemorySnapshot;
pub use viewer_media::{decode_media_report, encode_media_report, ViewerMediaCommand, ViewerMediaControl, MEDIA_ENDED, MEDIA_FAILED, MEDIA_LOADING, MEDIA_PAUSED, MEDIA_PLAYING, MEDIA_REPORT_BYTES};
pub use file_names::FileNameRules;
mod volumes;
pub use volumes::Volumes;

#[derive(Debug)]
pub enum NetworkDisconnectError {
    Busy,
    Other(String),
}

mod thread_priority;
pub use thread_priority::{apply_thread_priority, follow_turbo_priority, RestoreNormalPriority, ThreadPriority};

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct FileIdentity {
    volume: u64,
    value: [u8; 16],
    kind: u8,
}

/// Keeps a platform cancellation monitor alive only for its owning I/O scope.
#[doc(hidden)]
pub struct IoCancellationGuard {
    _state: Box<dyn std::any::Any>,
}

/// The item receipt is independent of a later failure of the native operation.
#[derive(Debug)]
pub struct ShellDeleteOutcome {
    pub completed: bool,
    pub error: Option<String>,
    /// Confirmed descendants, including separately recycled payloads.
    pub effects: Vec<crate::models::transaction::HistoryDeletedItem>,
}

#[derive(Clone)]
pub struct ShellDeleteControl {
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub pause: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub turbo: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl ShellDeleteControl {
    /// Re-evaluated at native checkpoints, so Turbo also reaches folder recycling.
    pub fn follow_turbo(&self) {
        follow_turbo_priority(self.turbo.load(std::sync::atomic::Ordering::Relaxed), ThreadPriority::Normal);
    }
}
/// Keeps the selected archive and its ancestors retained for a parser session.
#[doc(hidden)]
pub struct RetainedArchiveSource {
    pub file: std::fs::File,
    _parents: crate::systems::path_parents::HeldParents,
}

impl ShellDeleteOutcome {
    pub fn from_result(result: Result<(), String>) -> Self {
        Self { completed: result.is_ok(), error: result.err(), effects: Vec::new() }
    }
}

/// Abstraction des opérations spécifiques à l'OS.
/// Toute fonctionnalité native (lecteurs, ouverture, raccourcis, etc.)
/// doit passer par ce trait plutôt qu'appeler directement les API OS.
pub trait Platform: Send + Sync {
    /// Replace the window's file drop target. The callback runs on its UI thread.
    fn install_file_drop_target(&self, _window: raw_window_handle::RawWindowHandle,
        _callback: Box<dyn Fn(NativeFileDrop) -> FileDropEffect>) -> Result<(), String> {
        Err("Native file drops unavailable".into())
    }
    /// Queue a native session after GPUI releases its App/Window borrows.
    fn queue_file_drag(&self, _window: raw_window_handle::RawWindowHandle, _paths: Vec<String>,
        _callback: Box<dyn FnMut(FileDragFeedback)>) -> Result<(), String> {
        Err("Native file dragging unavailable".into())
    }
    /// Includes foreign windows overlapping the owner's client rectangle.
    fn file_drag_pointer_outside(&self, _window: raw_window_handle::RawWindowHandle) -> bool { false }
    /// Deferred placement of a passive drag ghost; topmost only for a native session.
    fn place_drag_window(&self, _window: raw_window_handle::RawWindowHandle,
        _size: [f32; 2], _scale: f32, _native: bool) -> Result<(), String> { Ok(()) }
    /// Cancel queued placements and hide this ghost permanently until destruction.
    fn dismiss_drag_window(&self, _window: raw_window_handle::RawWindowHandle) -> Result<(), String> { Ok(()) }
    /// Diagnostic scan of this process only. Call from a background thread.
    fn process_memory_snapshot(&self) -> Result<ProcessMemorySnapshot, String> {
        Err("Process memory diagnostics are unavailable on this platform".into())
    }
    /// Override the image zoom/pan cursor on the owning UI window.
    fn viewer_image_cursor(&self, _handle: raw_window_handle::RawWindowHandle, _zooming: bool, _dragging: bool) {}
    fn viewer_media_parent(&self, _handle: raw_window_handle::RawWindowHandle) -> Option<u64> { None }
    /// Place a nonactivating playback overlay in physical owner-client coordinates.
    fn place_viewer_controls(&self, _window: raw_window_handle::RawWindowHandle, _owner: raw_window_handle::RawWindowHandle, _rect: [i32; 4], _visible: bool) -> Result<(), String> { Ok(()) }
    /// Like `place_viewer_controls`, stacked directly above the owner's video surface.
    fn place_viewer_overlay(&self, _window: raw_window_handle::RawWindowHandle, _owner: raw_window_handle::RawWindowHandle, _rect: [i32; 4], _visible: bool) -> Result<(), String> { Ok(()) }
    /// Hand overlays owned by the video surfaces of `owner` back to it.
    fn release_viewer_overlays(&self, _owner: raw_window_handle::RawWindowHandle) {}
    /// The pointer in physical client coordinates of a window, even outside it.
    fn cursor_client_position(&self, _handle: raw_window_handle::RawWindowHandle) -> Option<[i32; 2]> { None }
    /// Blocking native session. Call only on a background worker; cancellation stops its process.
    fn play_viewer_media(&self, _path: &std::path::Path, _control: &ViewerMediaControl) -> std::io::Result<()> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "Native media playback unavailable"))
    }
    #[doc(hidden)]
    fn run_viewer_media_worker(&self, _path: &std::path::Path) -> std::io::Result<()> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "Native media worker unavailable"))
    }
    /// Background-only lookup. Extension includes its leading dot; no file is opened.
    fn file_kind_for_extension(&self, _extension: &str) -> Option<crate::models::file_entry::NativeFileKind> { None }

    /// Name comparison of an existing directory; newly created children inherit it.
    fn file_name_rules(&self, directory: &std::fs::File) -> std::io::Result<FileNameRules>;
    /// Available OneDrive sync roots configured for the current user.
    fn onedrive_installed(&self) -> bool { false }
    fn onedrive_folders(&self) -> Vec<crate::models::QuickAccessItem> { Vec::new() }
    fn show_startup_error(&self, _title: &str, message: &str) { eprintln!("{message}"); }
    /// HTTPS GET into memory, bounded by `limit` bytes.
    fn download(&self, _url: &str, _limit: usize) -> std::io::Result<Vec<u8>> {
        Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
    }
    /// Launch the verified installer in the background; it waits for this process.
    fn launch_update(&self, _setup: &std::path::Path) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
    }
    #[doc(hidden)]
    fn check_broker_pressure(&self) -> std::io::Result<()> { Ok(()) }
    #[doc(hidden)]
    fn cache_proof_key(&self, _path: &std::path::Path) -> std::io::Result<[u8; 32]> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "Persistent cache keys unavailable"))
    }
    #[doc(hidden)]
    fn seal_cache_proof(&self, _bytes: &[u8], _key: &[u8; 32], _protect: bool) -> std::io::Result<Vec<u8>> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "Persistent cache proofs unavailable"))
    }
    /// Attempt to interrupt synchronous broker I/O on cancellation/deadline.
    /// Drivers may refuse cancellation; this is not an unconditional deadline.
    #[doc(hidden)]
    fn watch_broker_io(&self, _remaining: std::time::Duration,
        _cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
        _pause: std::sync::Arc<std::sync::atomic::AtomicBool>) -> std::io::Result<IoCancellationGuard> {
        Ok(IoCancellationGuard { _state: Box::new(()) })
    }
    #[doc(hidden)]
    fn retain_archive_source(&self, path: &std::path::Path) -> std::io::Result<RetainedArchiveSource> {
        let _cancellation = self.watch_broker_io(std::time::Duration::from_secs(15), Default::default(), Default::default())?;
        let parents = crate::systems::path_parents::HeldParents::new(path)?;
        let file = self.open_archive_source(&parents.resolved_path(path)?, false)?;
        Ok(RetainedArchiveSource { file, _parents: parents })
    }
    fn run_archive_io_worker(&self, operation: crate::utils::archive_io_protocol::Operation,
        controls: &dyn Fn() -> std::io::Result<crate::utils::archive_io_protocol::Control>,
        handler: &mut dyn FnMut(crate::utils::archive_io_protocol::Frame) -> std::io::Result<crate::utils::archive_io_protocol::Frame>) -> std::io::Result<std::time::Duration>;
    fn list_archive_directory(&self, path: &std::path::Path, internal: &str) -> Result<Vec<crate::models::FileEntry>, crate::models::CommandError>;
    fn index_archive_contents(&self, path: &std::path::Path, checkpoint: &dyn Fn() -> std::io::Result<()>) -> Result<Vec<crate::models::FileEntry>, crate::models::CommandError>;
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
    fn retained_file_path(&self, file: &std::fs::File, fallback: &std::path::Path) -> std::io::Result<std::path::PathBuf>;
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
    /// `workers` above one lets Windows remove independent subfolders in parallel;
    /// the checkpoint is then called from several threads.
    fn remove_operation_tree(&self, parent: &std::fs::File, path: &std::path::Path, workers: usize, checkpoint: &(dyn Fn() -> std::io::Result<()> + Sync)) -> std::io::Result<()>;
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
    /// Mark the exclusively created output relative to its retained parent.
    fn write_archive_output_provenance(&self, parent: &std::fs::File, file: &std::fs::File, path: &std::path::Path, data: &[u8]) -> std::io::Result<()>;
    fn read_archive_output_provenance(&self, parent: &std::fs::File, file: &std::fs::File, path: &std::path::Path) -> std::io::Result<Option<Vec<u8>>>;
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
    /// Optional optimized display listing; never used as authority for mutations.
    fn list_trash_entries(&self) -> Result<Option<Vec<crate::models::TrashEntry>>, String> {
        Ok(None)
    }
    /// Return false when the display request has been cancelled.
    fn stream_trash_entries(&self, emit: &mut dyn FnMut(Vec<crate::models::TrashEntry>) -> bool) -> Result<(), String> {
        let entries = crate::commands::ops::list_trash_unsorted().map_err(|error| error.to_string())?;
        emit(entries);
        Ok(())
    }
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
    /// Freedesktop content of a listed item; its `id` names the `.trashinfo` record.
    #[cfg(not(target_os = "windows"))]
    fn trash_content_path(&self, item: &trash::TrashItem) -> Option<std::path::PathBuf>;
    /// Automatic trash cleanup owned by the desktop (GNOME or KDE); blocking.
    #[cfg(not(target_os = "windows"))]
    fn trash_retention(&self) -> Result<TrashRetention, String>;
    #[cfg(not(target_os = "windows"))]
    fn set_trash_retention(&self, retention: &TrashRetention) -> Result<(), String>;

    /// Final process exit, called only after application cleanup and runtime shutdown.
    /// Windows bypasses DLL detach callbacks that can hang in native graphics drivers.
    fn finish_process_shutdown(&self) -> Result<(), String>;

    /// Resolve a cache source without I/O; identify directories and preserve special entries.
    fn file_icon_source(&self, path: &str, is_dir: bool, preserve_path: bool) -> String;
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

    /// Native fixture setup only; absent from production builds and other targets.
    #[cfg(all(test, target_os = "windows"))]
    fn set_fixture_file_attributes(&self, path: &std::path::Path, attributes: u32) -> Result<(), String>;

    #[cfg(all(test, target_os = "windows"))]
    fn set_fixture_case_sensitive(&self, path: &std::path::Path) -> std::io::Result<()>;

    /// Applies an absolute scheduling priority to the current worker thread.
    /// Callers go through `apply_thread_priority`, which tracks the applied state.
    fn set_thread_priority(&self, priority: ThreadPriority);

    /// Deletes files permanently or sends them to the system trash.
    fn shell_delete(&self, paths: &[std::path::PathBuf], recycle: bool) -> Result<(), String>;

    /// Windows obtains an item-level receipt, checking a known identity at dispatch.
    fn shell_delete_tracked(&self, path: &std::path::Path, _proof: Option<crate::models::transaction::HistoryFileIdentity>) -> ShellDeleteOutcome {
        ShellDeleteOutcome::from_result(self.shell_delete(&[path.to_path_buf()], true))
    }
    #[cfg(target_os = "windows")]
    fn shell_delete_tracked_controlled(&self, path: &std::path::Path, proof: Option<crate::models::transaction::HistoryFileIdentity>, _control: ShellDeleteControl) -> ShellDeleteOutcome {
        self.shell_delete_tracked(path, proof)
    }
    #[cfg(target_os = "windows")]
    fn shell_delete_files_tracked(&self, files: &[(std::path::PathBuf, crate::models::transaction::HistoryFileIdentity)], control: ShellDeleteControl) -> Vec<ShellDeleteOutcome> {
        files.iter().map(|(path, proof)| self.shell_delete_tracked_controlled(path, Some(*proof), control.clone())).collect()
    }

    /// Lists entries in the operating system's network neighborhood.
    fn list_network_vicinity(&self) -> Result<Vec<crate::models::FileEntry>, String>;
    fn list_network_shares(&self, server: &str) -> Result<Vec<crate::models::FileEntry>, String>;

    fn is_network_path(&self, path: &std::path::Path) -> bool;
    /// Blocking network operations; call on a background worker.
    fn map_network_drive(&self, letter: String, path: String, reconnect: bool, username: Option<String>, password: Option<String>) -> Result<(), String>;
    fn disconnect_network_drive(&self, path: String, force: bool) -> Result<(), NetworkDisconnectError>;

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

    fn pdf_grid_thumbnail(&self, path: String) -> Result<image::RgbaImage, crate::models::CommandError> {
        #[cfg(target_os = "windows")]
        { thumbnails::get_pdf_grid_thumbnail_raw(path) }
        #[cfg(not(target_os = "windows"))]
        { self.pdf_thumbnail(path) }
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

    /// Complete document for the Viewer, as `tag<TAB>text` lines.
    fn office_document(&self, path: String) -> Result<String, crate::models::CommandError> {
        thumbnails::get_office_document(path)
    }

    /// Complete-or-error search text. Windows must never fall back to local parsing.
    fn office_search_text(&self, path: &std::path::Path, checkpoint: &dyn Fn() -> std::io::Result<()>) -> std::io::Result<String> {
        #[cfg(target_os = "windows")]
        { let _ = (path, checkpoint); Err(std::io::ErrorKind::Unsupported.into()) }
        #[cfg(not(target_os = "windows"))]
        {
            let started = std::time::Instant::now();
            crate::utils::office_document::search::extract(self.open_archive_source(path, false)?,
                path.extension().and_then(|extension| extension.to_str()).unwrap_or_default(), &|| {
                    checkpoint()?;
                    if started.elapsed() >= std::time::Duration::from_secs(15) { return Err(std::io::ErrorKind::TimedOut.into()); }
                    Ok(())
                })
        }
    }

    /// Cover-only requests use a reusable codec session and raw, bounded pixels.
    fn audio_cover(&self, path: &std::path::Path, thumbnail: bool) -> std::io::Result<Option<image::RgbaImage>> {
        #[cfg(target_os = "windows")]
        {
            use crate::utils::preview_protocol::{self, Reply, Limits};
            let side = if thumbnail { 128 } else { 512 };
            let bytes = self.run_preview_worker(if thumbnail { "audio-cover-thumbnail" } else { "audio-cover" }, path)?;
            match preview_protocol::decode_with(&bytes, Limits { pixels: (side * side * 4) as usize, side })? {
                Reply::Images { mut frames, .. } if frames.len() == 1 => Ok(Some(frames.remove(0).into_buffer())),
                Reply::Text(text) if text.is_empty() => Ok(None),
                Reply::Error { detail, .. } => Err(std::io::Error::other(detail)),
                _ => Err(std::io::Error::other("Invalid audio cover reply")),
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            crate::utils::audio_metadata::read_cover(self.retain_archive_source(path)?.file, thumbnail)?
                .map(|cover| image::RgbaImage::from_raw(cover.width, cover.height, cover.pixels)
                    .ok_or_else(|| std::io::Error::other("Invalid audio cover pixels"))).transpose()
        }
    }

    /// Audio tags and already decoded cover pixels; called away from the UI thread.
    fn audio_metadata(&self, path: &std::path::Path) -> std::io::Result<crate::utils::audio_metadata::AudioMetadata> {
        #[cfg(target_os = "windows")]
        {
            let bytes = self.run_preview_worker("audio-metadata", path)?;
            if bytes.len() > crate::utils::audio_metadata::MAX_REPLY {
                return Err(std::io::Error::other("Audio metadata reply exceeds its limit"));
            }
            let metadata: crate::utils::audio_metadata::AudioMetadata =
                serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
            metadata.validate()?;
            Ok(metadata)
        }
        #[cfg(not(target_os = "windows"))]
        {
            crate::utils::audio_metadata::read(self.retain_archive_source(path)?.file)
        }
    }

    /// Starts the Viewer's image decoder ahead of its first image.
    fn warm_viewer_images(&self) {}
    /// Ends the Viewer's image decoder while it shows other content.
    fn release_viewer_images(&self) {}
    /// Opens a PDF once for the Viewer; pages are then rendered on request.
    fn open_pdf_session(&self, _path: &std::path::Path) -> Result<Box<dyn PdfSession>, crate::models::CommandError> {
        Err(crate::models::CommandError::Other("PDF viewing is unavailable on this platform".into()))
    }
    #[doc(hidden)]
    fn run_pdf_session_worker(&self, _path: &std::path::Path) -> std::io::Result<()> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "PDF session worker unavailable"))
    }
    #[doc(hidden)]
    fn run_pdf_preview_session_worker(&self) -> std::io::Result<()> {
        Err(std::io::Error::other("PDF previews unavailable on this platform"))
    }

    fn volumes(&self) -> &dyn Volumes;
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

    /// Opens native properties for a complete selection when supported by the OS.
    fn show_selection_properties(&self, paths: &[String]) -> Result<(), String> {
        match paths {
            [path] => { self.show_properties_dialog(path); Ok(()) }
            _ => Err("Selection properties are unavailable on this platform".into()),
        }
    }

    /// Définit l'icône de l'application pour la barre des tâches et la fenêtre.
    fn set_application_icon(&self, window: raw_window_handle::RawWindowHandle, icon_bytes: &[u8]);

    /// Keep button drags active outside the client area. Called on the UI thread.
    fn install_pointer_capture(&self, window: raw_window_handle::RawWindowHandle) -> Result<(), String>;

    /// Keep a non-modal tool window above its owning application window.
    /// Call on the UI thread with two application-owned native handles.
    fn set_window_owner(&self, window: raw_window_handle::RawWindowHandle, owner: raw_window_handle::RawWindowHandle) -> Result<(), String>;

    /// Configure an application-owned, non-activating, mouse-transparent hover window.
    fn configure_hover_window(&self, window: raw_window_handle::RawWindowHandle, owner: raw_window_handle::RawWindowHandle) -> Result<(), String>;

    /// Show beside the native pointer, constrained to its monitor, or hide with None.
    /// Tuple is (outer width, outer height, transparent inset), in logical pixels.
    /// The cursor offset applies to the visible frame, excluding that inset.
    /// UI thread only.
    fn place_hover_window(&self, window: raw_window_handle::RawWindowHandle, size: Option<(f32, f32, f32)>, scale: f32) -> Result<(), String>;

    /// Set a fixed owner-client anchor (x, y, width, height), or follow the cursor.
    /// `above` always opens a non-beside popup over the anchor top.
    fn set_popup_anchor(&self, window: raw_window_handle::RawWindowHandle, bounds: Option<(f32, f32, f32, f32)>, open_left: bool, beside: bool, above: bool) -> Result<(), String>;

    /// Own a fixed dialog, disable its owner while shown and keep it centered
    /// over the owner. UI thread only; Win32 work is deferred to the message loop.
    fn configure_fixed_dialog(&self, window: raw_window_handle::RawWindowHandle, owner: raw_window_handle::RawWindowHandle) -> Result<(), String>;

    /// Recenter the fixed dialogs of a moved or resized owner.
    fn follow_fixed_dialog_owner(&self, owner: raw_window_handle::RawWindowHandle) -> Result<(), String>;

    /// Keep a menu non-activating; interactive menus receive their form's queued
    /// keyboard input. Passive suggestions leave input in the source field.
    fn configure_menu_window(&self, window: raw_window_handle::RawWindowHandle, owner: raw_window_handle::RawWindowHandle, coordinate_owner: raw_window_handle::RawWindowHandle, interactive: bool) -> Result<(), String>;
    fn popup_point_to_owner(&self, window: raw_window_handle::RawWindowHandle, owner: raw_window_handle::RawWindowHandle, point: (f32, f32)) -> Result<(f32, f32), String>;
    fn popup_family_active(&self, owner: raw_window_handle::RawWindowHandle) -> bool;
    fn popup_work_area_size(&self, window: raw_window_handle::RawWindowHandle) -> Result<(f32, f32), String>;

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
        path: &[String],
        is_background: bool,
    ) -> Result<Vec<crate::models::WinMenuItem>, String> {
        let _ = (path, is_background);
        Err("Native context menus are unavailable on this platform".to_string())
    }

    /// Executes a command ID from a previously built native context-menu snapshot.
    fn execute_native_context_menu_item(
        &self,
        path: &[String],
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
#[cfg(not(target_os = "windows"))]
mod linux_trash;
#[cfg(not(target_os = "windows"))]
mod linux_volumes;
#[cfg(not(target_os = "windows"))]
pub use linux_trash::{KdeLimitAction, KdeTrashLimits, TrashRetention};
#[cfg(target_os = "windows")]
pub mod network;
mod thumbnails;
#[cfg(target_os = "windows")]
mod windows_disk_images;
#[cfg(target_os = "windows")]
mod windows_pointer;
#[cfg(target_os = "windows")]
mod windows_file_drag;
#[cfg(target_os = "windows")]
mod windows_window_owner;

#[cfg(target_os = "windows")]
mod windows_hover_window;
#[cfg(target_os = "windows")]
mod windows_fixed_dialog;
#[cfg(target_os = "windows")]
mod windows_drives;
#[cfg(target_os = "windows")]
mod windows_volumes;
#[cfg(target_os = "windows")]
mod windows_impl;
#[cfg(target_os = "windows")]
mod windows_executables;
#[cfg(target_os = "windows")]
mod windows_archive_security;
#[cfg(target_os = "windows")]
mod windows_cache;
#[cfg(target_os = "windows")]
mod windows_cache_proof;
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
mod windows_icon_persistence;
#[cfg(target_os = "windows")]
mod windows_preview_cache;
#[cfg(target_os = "windows")]
mod windows_preview_quota;
#[cfg(target_os = "windows")]
mod windows_pdf_thumbnail;
#[cfg(target_os = "windows")]
mod windows_pdf_session;

/// An open Viewer PDF. Rendering blocks; call it on a background job.
pub trait PdfSession: Send {
    /// Release the native document even while obsolete render references remain.
    /// Called off the UI thread after cancelling pending page requests.
    fn close(&mut self) -> Result<(), crate::models::CommandError> { Ok(()) }
    /// Page sizes in document order; only their ratios are meaningful.
    fn sizes(&self) -> &[(f32, f32)];
    fn page(&mut self, index: u32) -> Result<image::RgbaImage, crate::models::CommandError>;
}
#[cfg(target_os = "windows")]
mod windows_file_moves;
#[cfg(target_os = "windows")]
mod windows_file_names;
#[cfg(target_os = "windows")]
#[doc(hidden)]
pub mod worker_test_support;
#[cfg(target_os = "windows")]
mod windows_file_tree;
#[cfg(target_os = "windows")]
mod windows_copy_staging;
#[cfg(all(test, target_os = "windows"))]
pub(crate) mod windows_reparse_tests;
#[cfg(target_os = "windows")]
mod windows_archive_source;
#[cfg(target_os = "windows")]
mod windows_broker_cancellation;
#[cfg(target_os = "windows")]
mod windows_file_copy;
#[cfg(target_os = "windows")]
mod windows_shell;
#[cfg(target_os = "windows")]
mod windows_shell_selection;
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

/// Static dispatch avoids linking every native Platform operation into the
/// headless archive executable. Preserve the same final shutdown contract.
pub(crate) fn finish_archive_worker_shutdown() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        <CurrentPlatform as Platform>::finish_process_shutdown(&CurrentPlatform::new())
    }
    #[cfg(not(target_os = "windows"))]
    {
        platform().finish_process_shutdown()
    }
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
mod windows_office_search;
#[cfg(target_os = "windows")]
mod windows_viewer_media;
#[cfg(target_os = "windows")]
mod windows_viewer_controls;
#[cfg(target_os = "windows")]
mod windows_archive_worker;
#[cfg(target_os = "windows")]
mod windows_archive_io_worker;
#[cfg(target_os = "windows")]
mod windows_worker_sandbox;
#[cfg(target_os = "windows")]
mod windows_process_launch;
#[cfg(target_os = "windows")]
mod windows_worker_budget;
#[cfg(target_os = "windows")]
mod windows_resource_pressure;
#[cfg(target_os = "windows")]
mod windows_icon_pixels;
#[cfg(target_os = "windows")]
mod windows_icon_source;

#[cfg(target_os = "windows")]
mod windows_onedrive;
#[cfg(target_os = "windows")]
mod windows_download;
#[cfg(target_os = "windows")]
mod windows_file_kind;
