mod file_names;
mod file_drag;
pub use file_drag::{FileDragFeedback, FileDropEffect, FileDropPhase, NativeFileDrop};
mod viewer_media;
mod memory;
pub use memory::ProcessMemorySnapshot;
pub use viewer_media::{decode_media_info, decode_media_report, encode_media_info, encode_media_report, MediaInfo, MEDIA_INFO, ViewerMediaCommand, ViewerMediaControl, MEDIA_ENDED, MEDIA_FAILED, MEDIA_LOADING, MEDIA_PAUSED, MEDIA_PLAYING, MEDIA_REPORT_BYTES};
pub use file_names::FileNameRules;
mod file_clipboard;
mod quick_access;
mod volumes;
mod remote;
pub use file_clipboard::FileClipboard;
pub use remote::{PasswordSave, RemoteAnswer, RemoteError, RemoteLocation, RemotePrompt};
pub use quick_access::QuickAccess;
pub use volumes::{DriveActions, UnmountedVolume, VolumeError, Volumes};

#[derive(Debug)]
pub enum NetworkDisconnectError {
    Busy,
    Other(String),
}

/// Extended attributes a copy keeps, by name: `user.*` and POSIX ACLs on Linux.
#[derive(Debug, Clone, Default)]
pub struct ExtendedAttributes(#[cfg_attr(target_os = "windows", allow(dead_code))] Vec<(std::ffi::OsString, Vec<u8>)>);

/// What a copy keeps of a source directory, read before its listing changes it.
#[derive(Debug, Clone)]
pub struct DirectoryAttributes {
    pub metadata: std::fs::Metadata,
    pub extended: ExtendedAttributes,
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

/// What Oxyde lists in the menu of its taskbar button, in the user's language.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TaskbarMenu {
    /// The title above the pinned folders.
    pub category: String,
    /// Name and path of each pinned folder.
    pub pinned: Vec<(String, String)>,
    /// The task that starts Oxyde as administrator.
    pub administrator: String,
}

/// What another process asks the running Oxyde to show: a second launch, a program
/// calling FileManager1 such as a browser's « Show in folder », or the desktop portal
/// asking for a file on behalf of an application.
#[derive(Debug)]
pub enum InstanceRequest {
    /// Folders, each in a new tab; none only brings the window forward.
    Folders(Vec<String>),
    /// Items selected in their folder, one tab per folder.
    Items(Vec<String>),
    /// Items whose Properties window opens.
    Properties(Vec<String>),
    /// Files to open, or where to save, chosen in a window of its own.
    ChooseFile(Box<FileChooserRequest>),
}

/// One rule of a file chooser's type filter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileTypePattern {
    /// A shell pattern on the name, such as `*.png`, case kept.
    Glob(String),
    /// A MIME type, such as `image/png` or `image/*`.
    MimeType(String),
}

/// A named choice of file types the application offers, such as "Images".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileTypeFilter {
    pub name: String,
    pub patterns: Vec<FileTypePattern>,
}

/// What the application asks the file chooser for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileChooserPurpose {
    /// Files or folders to open.
    Open,
    /// Where to save a file, starting from the name proposed, possibly empty.
    Save { name: String },
    /// A folder to save files in, under these names.
    SaveFiles { names: Vec<std::ffi::OsString> },
}

/// A question the application asks beside the files, such as an encoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChooserChoice {
    pub id: String,
    pub label: String,
    /// The answers offered, by id and label; none for a checkbox, answered
    /// `"true"` or `"false"`.
    pub options: Vec<(String, String)>,
    /// The answer given first, one of the options.
    pub initial: String,
}

/// A text field an application adds beside the files (Windows' edit boxes): its
/// text goes back with the choice, as a choice's answer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileChooserField {
    pub id: String,
    pub label: String,
    pub text: String,
}

/// A folder an application adds to the places, at their start or their end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChooserPlace {
    pub path: std::path::PathBuf,
    pub first: bool,
}

/// What only some applications ask of their dialog, Windows' options for the most
/// part; the default asks none of it and keeps the chooser's own behavior.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileChooserExtras {
    /// The application's labels for the name field and for Cancel.
    pub name_label: Option<String>,
    pub cancel_label: Option<String>,
    /// Hidden and system files are listed from the start.
    pub show_hidden: bool,
    /// A file saved over is replaced without asking.
    pub no_overwrite_prompt: bool,
    /// A name saved must have an extension of the type filter shown, which is added.
    pub strict_types: bool,
    /// A read-only file or folder cannot be saved to.
    pub refuse_read_only: bool,
    /// The button waits for the user to browse, select or type.
    pub needs_interaction: bool,
    /// The user's pinned places are left out.
    pub hide_pinned: bool,
    pub places: Vec<FileChooserPlace>,
    /// Who asks, to start where that application last chose: its own key, else none.
    pub client: Option<String>,
    /// The folder to start in when nothing better is known: the application's default.
    pub default_folder: Option<std::path::PathBuf>,
    pub fields: Vec<FileChooserField>,
    /// Texts the application shows beside its controls, read only.
    pub notes: Vec<String>,
    /// Ways to accept, offered in a menu beside the button (Windows' open drop-down):
    /// the answer chosen goes back as this choice's, the first one by default.
    pub accept_menu: Option<FileChooserChoice>,
    /// The choices and fields shown but not to be changed.
    pub disabled: Vec<String>,
}

/// What the user chose in a file chooser.
#[derive(Debug, Default, PartialEq)]
pub struct FileChooserAnswer {
    pub paths: Vec<std::path::PathBuf>,
    /// The type filter shown when the user chose, as a position in the request's.
    pub filter: Option<usize>,
    /// Each choice's id and the answer given.
    pub choices: Vec<(String, String)>,
}

/// An application's request, through the desktop portal, for files or folders to open,
/// or where to save.
#[derive(Debug)]
pub struct FileChooserRequest {
    pub purpose: FileChooserPurpose,
    /// The application's title for the window; empty when it gives none.
    pub title: String,
    /// The application's label for the button that accepts, mnemonics removed.
    pub accept_label: Option<String>,
    /// Several items may be chosen.
    pub multiple: bool,
    /// Folders are chosen instead of files, as when saving several files.
    pub directory: bool,
    /// The application's type filters, and the one shown first.
    pub filters: Vec<FileTypeFilter>,
    pub current_filter: Option<usize>,
    /// The folder the application suggests, known to be one; else the last one.
    pub current_folder: Option<std::path::PathBuf>,
    /// The extension a name saved without one takes, where applications leave that
    /// to the dialog (Windows): the shown filter's when it names one, else this one.
    pub default_extension: Option<String>,
    pub choices: Vec<FileChooserChoice>,
    pub extras: FileChooserExtras,
    /// The application's window, which the chooser opens over.
    pub parent: Option<FileChooserParent>,
    /// Receives the choice, or nothing when the user cancels.
    pub answer: futures::channel::oneshot::Sender<Option<FileChooserAnswer>>,
    /// Resolves when the application withdraws its request: the window closes unanswered.
    /// Also ends when the request is given up, which leaves the window unopened.
    pub withdrawn: futures::channel::oneshot::Receiver<()>,
    /// Told once the window is open; taken by the interface. A window that does not
    /// open in time leaves the request to the system's file chooser.
    pub shown: Option<futures::channel::oneshot::Sender<()>>,
}

/// An application's window, as the desktop portal names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileChooserParent {
    /// An X11 window id.
    X11(u32),
    /// A toplevel the application exported through xdg-foreign.
    Wayland(String),
    /// A window handle.
    #[cfg(target_os = "windows")]
    Windows(isize),
}

#[derive(Debug)]
pub struct InstanceMessage {
    pub request: InstanceRequest,
    /// The token the sender's launcher gave it, which lets the window come forward.
    pub activation_token: Option<String>,
}

/// Whether this process is the session's Oxyde.
pub enum InstanceClaim {
    /// The first one: requests from later launches and FileManager1 calls arrive here.
    First(futures::channel::mpsc::UnboundedReceiver<InstanceMessage>),
    /// Another Oxyde runs and received the locations: this process ends.
    Forwarded,
    /// Started for the file chooser portal after starts that failed before showing a
    /// window: this process shows none and only relays the portal's calls.
    Relay,
    /// No session bus, or no single instance on this system: this process stands alone.
    Alone,
}

/// Parent of the WSL distributions, which Explorer calls "Linux".
pub const WSL_ROOT: &str = r"\\wsl.localhost";

pub fn is_wsl_path(path: &str) -> bool {
    path.get(..WSL_ROOT.len()).is_some_and(|root| root.eq_ignore_ascii_case(WSL_ROOT))
        && path[WSL_ROOT.len()..].chars().next().is_none_or(|next| next == '\\')
}

/// What delivered the running Oxyde, which decides how a newer release reaches it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Installation {
    /// Built from source, or placed by hand: releases are not checked.
    Unmanaged,
    /// Oxyde downloads, verifies and installs this entry of the update manifest itself.
    SelfUpdating { manifest_key: &'static str, signature: ReleaseSignature },
    /// A system package: Oxyde announces releases and lets the user download this
    /// manifest entry, or the release page when there is none.
    Package { manifest_key: Option<&'static str> },
}

/// How a self-updating installation checks a release before installing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseSignature {
    /// The Tauri updater key, compiled into Oxyde: Windows.
    Minisign,
    /// Oxyde's package signing key, checked by `Platform::verify_openpgp`: Linux.
    OpenPgp,
}

/// Look of the minimize, maximize and close buttons drawn by Oxyde's own title bars.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowControlStyle {
    /// Full-height caption buttons with a red close hover.
    Windows,
    /// GNOME: round buttons on a translucent disc.
    Adwaita,
    /// KDE Plasma: bare glyphs, a disc only under the pointer.
    Breeze,
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
    /// `size` includes a transparent `inset` on each side, for the shadow: the visible ghost,
    /// not the window, keeps its distance from the cursor.
    fn place_drag_window(&self, _window: raw_window_handle::RawWindowHandle,
        _size: [f32; 2], _inset: f32, _scale: f32, _native: bool) -> Result<(), String> { Ok(()) }
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
    /// Open the native window menu at the pointer, for a right click on a custom title bar.
    fn show_window_system_menu(&self, _window: raw_window_handle::RawWindowHandle) -> Result<(), String> { Ok(()) }
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
    /// Registered WSL distributions, read without starting any of them. Blocking.
    fn wsl_distributions(&self) -> Vec<crate::models::SidebarNode> { Vec::new() }
    fn show_startup_error(&self, _title: &str, message: &str) { eprintln!("{message}"); }
    /// Keeps GPUs the desktop does not render with asleep; returns what it disabled and
    /// why. Changes the process environment: call it while the process has a single thread.
    fn prefer_display_gpu(&self) -> Option<String> { None }
    /// Locations the desktop asked this launch to open, instead of the saved tabs.
    fn startup_locations(&self) -> Vec<String> { Vec::new() }
    /// Becomes the session's Oxyde, or hands `locations` to the one already running.
    /// Runs before the first window, which the forwarding process never opens.
    fn claim_instance(&self, _locations: &[String]) -> InstanceClaim { InstanceClaim::Alone }
    /// Names the process for the taskbar, so that its button is the one of its
    /// shortcuts. Before the first window.
    fn identify_process(&self) {}
    /// Lists `menu` under Oxyde's taskbar button. Blocking.
    fn set_taskbar_menu(&self, _menu: &TaskbarMenu) {}
    /// Starts Oxyde again as administrator, which the taskbar menu's task asks for.
    fn start_as_administrator(&self) -> std::io::Result<()> { Err(std::io::Error::from(std::io::ErrorKind::Unsupported)) }
    /// Lists Oxyde under `label` in the menus of the system's folders where this copy
    /// should be, and removes it where it should not. Blocking.
    fn refresh_folder_menus(&self, _label: &str) {}
    /// Removes Oxyde from the menus of the system's folders, before it is uninstalled.
    fn withdraw_from_folder_menus(&self) {}
    /// Stops taking later launches, as the application starts to quit: one handed over
    /// from then on would never be shown. Returns at once.
    fn release_instance(&self) {}
    /// Whether this process was started to choose files for an application, by the
    /// session bus for the desktop portal or by the launcher of a Windows dialog: it
    /// opens no main window until a later launch asks for one.
    fn started_for_file_chooser(&self) -> bool { false }
    /// Whether an application still waits for a file chosen through Oxyde, which keeps
    /// Oxyde running after its last window.
    fn answering_file_chooser(&self) -> bool { false }
    /// The interface thread is panicking: applications waiting for a file from Oxyde get
    /// the system's file chooser instead. Blocks until they are answered.
    fn hand_off_file_choosers(&self) {}
    /// Hands the desktop portal's calls to the system's file chooser, without a window,
    /// until none came for a while. Blocking.
    fn relay_file_chooser(&self) {}
    /// How releases reach this copy of Oxyde. Cheap: the About page asks at each render.
    fn installation(&self) -> Installation { Installation::Unmanaged }
    /// Window buttons that match the running desktop.
    fn window_control_style(&self) -> WindowControlStyle { WindowControlStyle::Windows }
    /// HTTPS GET into memory, bounded by `limit` bytes.
    fn download(&self, _url: &str, _limit: usize) -> std::io::Result<Vec<u8>> {
        Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
    }
    /// Check `signature`, an armored detached OpenPGP signature, of `data` against
    /// Oxyde's package signing key.
    fn verify_openpgp(&self, _data: &std::path::Path, _signature: &[u8]) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
    }
    /// Install the verified release and arrange for Oxyde to start again once this
    /// process has exited: Windows runs its Setup, which waits; Linux installs at once.
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
    /// Checks the password remembered for an encrypted archive in a confined worker.
    fn verify_archive_password(&self, path: &std::path::Path) -> Result<(), crate::models::CommandError>;
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
    /// Target of a captured symbolic link that transfers copy as a link. `None`
    /// keeps the entry refused, as Windows does for every reparse point.
    fn source_link_target(&self, _captured: &std::fs::File, _metadata: &std::fs::Metadata) -> Option<std::io::Result<std::path::PathBuf>> {
        None
    }
    /// Create a symbolic link in the held parent and return the link itself.
    /// `replace` swaps it in over an existing non-directory in one rename.
    fn create_link(&self, _parent: &std::fs::File, _path: &std::path::Path, _target: &std::path::Path, _replace: bool) -> std::io::Result<std::fs::File> {
        Err(std::io::ErrorKind::Unsupported.into())
    }
    /// Move a link itself between held parents of one volume and return it;
    /// another volume fails with `CrossesDevices`.
    fn rename_link(&self, _source_parent: &std::fs::File, _source: &std::path::Path, _parent: &std::fs::File, _destination: &std::path::Path, _replace: bool) -> std::io::Result<std::fs::File> {
        Err(std::io::ErrorKind::Unsupported.into())
    }
    /// Remove a link of the held parent once `verify` accepts that exact link.
    fn remove_link(&self, _parent: &std::fs::File, _path: &std::path::Path, _verify: &mut dyn FnMut(&std::fs::File) -> std::io::Result<()>) -> std::io::Result<()> {
        Err(std::io::ErrorKind::Unsupported.into())
    }
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
    /// Remove the selected object and its children by retained identities.
    /// Reparse entries and links are removed themselves; their targets are never traversed.
    /// `workers` above one removes independent subfolders in parallel;
    /// the checkpoint is then called from several threads.
    fn remove_operation_tree(&self, parent: &std::fs::File, path: &std::path::Path, workers: usize, checkpoint: &(dyn Fn() -> std::io::Result<()> + Sync)) -> std::io::Result<()>;
    /// Whether Turbo removes several items of the folder of `path` faster at once
    /// than one after the other.
    fn parallel_deletion(&self, _path: &std::path::Path) -> bool {
        true
    }
    /// Exclusively create hidden copy staging and a retained non-deletable marker on Windows.
    fn create_copy_staging(&self, parent: &std::fs::File, path: &std::path::Path) -> std::io::Result<(std::fs::File, std::fs::File)>;
    /// Create a private archive writer relative to staging; retain rename/delete
    /// access and exclude external writers/replacement on Windows.
    fn create_operation_output(&self, parent: &std::fs::File, path: &std::path::Path) -> std::io::Result<std::fs::File>;
    /// Reopen a staging folder an ended process left, as `create_copy_staging` holds
    /// it, so `remove_copy_staging` can remove it; a link or reparse point is refused.
    fn open_copy_staging(&self, path: &std::path::Path) -> std::io::Result<std::fs::File>;
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
    /// Open a cache entry for eviction: refused while another reader holds it open.
    fn open_cache_cleanup(&self, path: &std::path::Path, directory: bool) -> std::io::Result<std::fs::File>;
    /// Confirm, just before deletion, that no reader opened an entry since its cleanup
    /// handle was obtained. Windows sharing already refuses such openers.
    fn confirm_cache_cleanup(&self, _file: &std::fs::File) -> std::io::Result<()> {
        Ok(())
    }
    fn ensure_archive_directory(&self, path: &std::path::Path) -> std::io::Result<std::fs::File>;
    fn create_archive_file(&self, path: &std::path::Path) -> std::io::Result<std::fs::File>;
    /// Read/write bounded native Internet provenance; unsupported platforms use None.
    fn read_file_provenance(&self, path: &std::path::Path) -> std::io::Result<Option<Vec<u8>>>;
    fn write_file_provenance(&self, path: &std::path::Path, data: &[u8]) -> std::io::Result<()>;
    /// Mark the exclusively created output relative to its retained parent.
    fn write_archive_output_provenance(&self, parent: &std::fs::File, file: &std::fs::File, path: &std::path::Path, data: &[u8]) -> std::io::Result<()>;
    fn read_archive_output_provenance(&self, parent: &std::fs::File, file: &std::fs::File, path: &std::path::Path) -> std::io::Result<Option<Vec<u8>>>;
    /// Put back a copy's access date after Oxyde read it for verification: FUSE file
    /// systems such as ntfs-3g update it despite `O_NOATIME`. Windows keeps its own.
    fn restore_access_date(&self, _file: &std::fs::File, _accessed: std::time::SystemTime) -> std::io::Result<()> {
        Ok(())
    }
    /// Whether one rename can move an item from the first folder into the second.
    /// Windows keeps moving folders item by item.
    fn same_mount(&self, _first: &std::fs::File, _second: &std::fs::File) -> std::io::Result<bool> {
        Ok(false)
    }
    /// Extended attributes and ACLs a copy keeps from this source. None on Windows,
    /// whose native copy keeps its own.
    fn extended_attributes(&self, _file: &std::fs::File) -> std::io::Result<ExtendedAttributes> {
        Ok(ExtendedAttributes::default())
    }
    /// Give a directory created by a copy the attributes of its source, once its
    /// content is in place. Its owner keeps full access. Windows keeps its own.
    fn restore_directory_attributes(&self, _directory: &std::fs::File, _source: &DirectoryAttributes) -> std::io::Result<()> {
        Ok(())
    }
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

    /// Names a directory hides through its own list (`.hidden` on Linux, as GIO and Nautilus read it).
    fn listed_hidden_names(&self, _directory: &std::path::Path) -> std::collections::HashSet<String> {
        std::collections::HashSet::new()
    }

    /// Whether the file system exposes a System attribute distinct from Hidden.
    fn has_system_file_attribute(&self) -> bool { false }

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
    /// Shares of a `\\server`, or on Linux the entries of a remote URI that `connect_remote` browses.
    fn list_network_shares(&self, server: &str) -> Result<Vec<crate::models::FileEntry>, String>;
    /// Drops a server from the recent servers the network list shows.
    fn forget_recent_server(&self, _uri: &str) {}
    /// Locations that `connect_remote` opens, such as `smb://server/share` on Linux.
    fn is_remote_uri(&self, _text: &str) -> bool { false }
    /// Mounts the location when needed, asking `ask` for credentials or a choice; setting
    /// `cancel` gives up. Blocking.
    fn connect_remote(
        &self, _uri: &str, _ask: &mut dyn FnMut(&RemotePrompt) -> RemoteAnswer, _cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<RemoteLocation, RemoteError> {
        Err(RemoteError::Unsupported)
    }

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

    /// The grid's video frame, at two seconds or the first of a shorter video.
    fn video_grid_thumbnail(&self, path: String) -> Result<image::RgbaImage, crate::models::CommandError> {
        video_thumbnail::grid_frame(path, |path, on_frame| self.video_frames(path, on_frame, Box::new(|| false)))
    }

    fn office_text_preview(&self, path: String) -> Result<String, crate::models::CommandError> {
        thumbnails::get_office_text_preview(path)
    }

    /// Complete document for the Viewer, as `tag<TAB>text` lines.
    fn office_document(&self, path: String) -> Result<String, crate::models::CommandError> {
        thumbnails::get_office_document(path)
    }

    /// Complete-or-error search text. Windows must never fall back to local parsing.
    fn office_search_text(&self, _path: &std::path::Path, _checkpoint: &dyn Fn() -> std::io::Result<()>) -> std::io::Result<String> {
        Err(std::io::ErrorKind::Unsupported.into())
    }

    /// Cover-only requests use a reusable codec session and raw, bounded pixels.
    fn audio_cover(&self, path: &std::path::Path, thumbnail: bool) -> std::io::Result<Option<image::RgbaImage>> {
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

    /// Audio tags and already decoded cover pixels; called away from the UI thread.
    fn audio_metadata(&self, path: &std::path::Path) -> std::io::Result<crate::utils::audio_metadata::AudioMetadata> {
        let bytes = self.run_preview_worker("audio-metadata", path)?;
        if bytes.len() > crate::utils::audio_metadata::MAX_REPLY {
            return Err(std::io::Error::other("Audio metadata reply exceeds its limit"));
        }
        let metadata: crate::utils::audio_metadata::AudioMetadata =
            serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
        metadata.validate()?;
        Ok(metadata)
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
    /// Brings the media decoders' plugin registry up to date; opens no document.
    #[doc(hidden)]
    fn run_media_registry_worker(&self) -> std::io::Result<()> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "Media registry worker unavailable"))
    }

    fn volumes(&self) -> &dyn Volumes;
    /// Install on the native window's UI thread; never changes system preferences.
    fn install_disk_image_autoplay_handler(&self, window: raw_window_handle::RawWindowHandle) -> Result<(), String>;

    /// Ouvre un fichier ou dossier avec l'application par défaut de l'OS.
    fn open_item(&self, path: &str) -> Result<(), String>;
    /// Whether Oxyde offers its own « Open With » for files, rather than the system menu's.
    fn offers_open_with(&self) -> bool { false }
    /// Applications able to open every one of `paths`, the default first.
    fn open_with_choices(&self, _paths: &[String]) -> Result<Vec<crate::models::ApplicationChoice>, String> {
        Err("Open With is not available on this system".into())
    }
    /// Opens `paths` with the application `id`; `remember` makes it their types' default.
    fn open_with(&self, _id: &str, _paths: &[String], _remember: bool) -> Result<(), String> {
        Err("Open With is not available on this system".into())
    }
    /// Whether other applications open folders with Oxyde; None when the system offers
    /// no such choice, or names no installed Oxyde to choose. Reads files: off the UI thread.
    fn opens_folders_by_default(&self) -> Option<bool> { None }
    /// Makes Oxyde the default for folders and returns the application it replaces, or
    /// gives folders back to `previous`, the system's default without it.
    fn open_folders_by_default(&self, _enabled: bool, _previous: Option<&str>) -> Result<Option<String>, String> {
        Err("Choosing the default for folders is not available on this system".into())
    }
    /// Whether applications choose their files with Oxyde, through the desktop portal
    /// or in place of the Windows dialogs; None when the system has no portal, or names
    /// no Oxyde it could start. Reads files: off the UI thread.
    fn chooses_files(&self) -> Option<bool> { None }
    /// Makes Oxyde the portal's file chooser, or gives that back to the previous one.
    /// Writes files and restarts the portal: off the UI thread.
    fn choose_files(&self, _enabled: bool) -> Result<(), String> {
        Err("Choosing files for other applications is not available on this system".into())
    }
    /// Gives the dialogs of other applications back to the system, before Oxyde is
    /// uninstalled.
    fn withdraw_from_file_choosers(&self) {}
    /// Whether this system offers to open a folder in a terminal.
    fn offers_terminal(&self) -> bool { false }
    fn open_terminal(&self, _path: &str) -> Result<(), String> {
        Err("Opening a terminal is not available on this system".into())
    }

    fn quick_access(&self) -> &dyn QuickAccess;
    /// Best-effort native name of a virtual Shell item; ordinary properties keep their fallback.
    fn shell_item_friendly_name(&self, path: &str) -> Result<Option<String>, String> {
        let _ = path;
        Ok(None)
    }
    fn peek_status(&self) -> Result<PeekStatus, String>;
    fn open_peek(&self, path: &str) -> Result<(), String>;
    fn shortcut_info(&self, path: &std::path::Path) -> Option<crate::models::ShortcutInfo>;
    fn set_shortcut_info(&self, path: &str, info: &crate::models::ShortcutInfo) -> Result<(), String>;

    /// Unix permissions shown by the Permissions tab; None where the system shows
    /// its own (Windows) or the item cannot be read.
    fn unix_permissions(&self, _path: &std::path::Path) -> Option<crate::models::permissions::UnixPermissions> {
        None
    }
    /// The process umask, which new files receive; None where it cannot be read.
    fn umask(&self) -> Option<u32> {
        None
    }
    /// Type and description from the system's MIME database; None on Windows.
    fn mime_type(&self, _path: &std::path::Path, _is_dir: bool, _language: &str) -> Option<crate::models::MimeType> {
        None
    }
    /// The MIME type's name alone, as `mime_type` finds it; None on Windows.
    fn content_type(&self, _path: &std::path::Path, _is_dir: bool) -> Option<&'static str> {
        None
    }
    /// Whether a MIME type answers a file chooser's type, such as `image/png` or
    /// `image/*`, through the system's aliases and subclasses.
    fn content_type_matches(&self, mime: &str, pattern: &str) -> bool {
        mime.eq_ignore_ascii_case(pattern)
    }
    /// Stops between enclosed items once `cancel` is set.
    fn set_unix_permissions(
        &self,
        _path: &std::path::Path,
        _change: &crate::models::permissions::PermissionChange,
        _cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<(), crate::models::permissions::PermissionError> {
        Err(crate::models::permissions::PermissionError::Failed("Unix permissions are unavailable on this platform".into()))
    }

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

    fn file_clipboard(&self) -> &dyn FileClipboard;

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
#[cfg(target_os = "windows")]
mod windows_file_clipboard;
#[cfg(target_os = "windows")]
mod windows_system_paste;
#[cfg(not(target_os = "windows"))]
mod linux_file_clipboard;
#[cfg(target_os = "windows")]
mod windows_quick_access;
#[cfg(not(target_os = "windows"))]
mod linux_quick_access;
mod hardware;
#[cfg(target_os = "windows")]
pub mod icons;
#[cfg(not(target_os = "windows"))]
mod linux_impl;
#[cfg(not(target_os = "windows"))]
mod linux_gpu;
#[cfg(not(target_os = "windows"))]
mod linux_hidden_list;
#[cfg(not(target_os = "windows"))]
mod linux_settings;
#[cfg(not(target_os = "windows"))]
mod linux_terminal;
#[cfg(not(target_os = "windows"))]
mod linux_window_controls;
#[cfg(not(target_os = "windows"))]
mod linux_startup;
#[cfg(not(target_os = "windows"))]
mod linux_instance;
#[cfg(not(target_os = "windows"))]
mod linux_portal;
#[cfg(not(target_os = "windows"))]
mod linux_gio;
#[cfg(not(target_os = "windows"))]
mod linux_gstreamer;
#[cfg(not(target_os = "windows"))]
mod linux_library;
#[cfg(not(target_os = "windows"))]
mod linux_media;
#[cfg(not(target_os = "windows"))]
mod linux_pdf;
#[cfg(not(target_os = "windows"))]
mod linux_sandbox;
#[cfg(not(target_os = "windows"))]
mod linux_video;
#[cfg(not(target_os = "windows"))]
mod linux_worker;
#[cfg(not(target_os = "windows"))]
mod linux_trash;
#[cfg(not(target_os = "windows"))]
mod linux_volumes;
#[cfg(not(target_os = "windows"))]
mod linux_update;
#[cfg(not(target_os = "windows"))]
mod linux_download;
#[cfg(not(target_os = "windows"))]
mod linux_signature;
#[cfg(not(target_os = "windows"))]
pub use linux_trash::{KdeLimitAction, KdeTrashLimits, TrashRetention};
#[cfg(target_os = "windows")]
pub mod network;
mod thumbnails;
mod video_thumbnail;
#[cfg(target_os = "windows")]
mod windows_disk_images;
#[cfg(target_os = "windows")]
mod windows_pointer;
#[cfg(target_os = "windows")]
mod windows_file_drag;
#[cfg(target_os = "windows")]
mod windows_window_owner;
#[cfg(target_os = "windows")]
mod windows_system_menu;

#[cfg(target_os = "windows")]
mod windows_hover_window;
#[cfg(target_os = "windows")]
mod windows_fixed_dialog;
#[cfg(target_os = "windows")]
mod windows_drives;
#[cfg(target_os = "windows")]
mod windows_eject_holders;
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
pub fn local_preview_worker_reply(mode: &str, path: &std::path::Path) -> crate::utils::preview_protocol::Reply {
    platform().decode_preview_worker(mode, path)
}

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
mod linux_archive_io;
#[cfg(target_os = "linux")]
mod linux_archive_list;
#[cfg(target_os = "linux")]
mod linux_preview;
#[cfg(target_os = "linux")]
mod linux_source_selection;
#[cfg(target_os = "linux")]
mod linux_links;
#[cfg(target_os = "linux")]
mod linux_rename;
#[cfg(target_os = "linux")]
mod linux_file_copy;
#[cfg(target_os = "linux")]
mod linux_extended_attributes;
#[cfg(target_os = "linux")]
mod linux_permissions;
#[cfg(target_os = "linux")]
mod linux_mime;
#[cfg(target_os = "linux")]
mod linux_file_names;
#[cfg(target_os = "linux")]
mod linux_access;
#[cfg(target_os = "linux")]
mod linux_cache;
#[cfg(target_os = "linux")]
mod linux_file_tree;
#[cfg(target_os = "linux")]
mod linux_held;

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
mod windows_wsl;
#[cfg(target_os = "windows")]
mod windows_startup;
#[cfg(target_os = "windows")]
mod windows_instance;
#[cfg(target_os = "windows")]
mod windows_folder_handler;
#[cfg(target_os = "windows")]
mod windows_explorer_browse;
#[cfg(target_os = "windows")]
mod windows_shell_window;
#[cfg(target_os = "windows")]
mod windows_jump_list;
/// The switch Oxyde's taskbar task runs it with, to start itself as administrator.
#[cfg(target_os = "windows")]
pub use windows_jump_list::AS_ADMINISTRATOR;
#[cfg(target_os = "windows")]
mod windows_folder_launcher;
#[cfg(target_os = "windows")]
pub use windows_folder_launcher::run as run_folder_launcher;
#[cfg(target_os = "windows")]
mod windows_file_dialog;
#[cfg(target_os = "windows")]
mod windows_download;
#[cfg(target_os = "windows")]
mod windows_file_kind;
