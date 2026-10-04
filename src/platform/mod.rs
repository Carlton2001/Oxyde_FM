use crate::models::{DriveInfo, QuickAccessItem};

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

/// Abstraction des opérations spécifiques à l'OS.
/// Toute fonctionnalité native (lecteurs, ouverture, raccourcis, etc.)
/// doit passer par ce trait plutôt qu'appeler directement les API OS.
pub trait Platform: Send + Sync {
    /// Final process exit, called only after application cleanup and runtime shutdown.
    /// Windows bypasses DLL detach callbacks that can hang in native graphics drivers.
    fn finish_process_shutdown(&self) -> Result<(), String>;

    /// Load native icons on a background worker.
    fn file_icon(&self, path: String, large: bool) -> Result<image::RgbaImage, String>;

    fn name(&self) -> &'static str;
    /// Current user's regional formatting locale.
    fn regional_locale(&self) -> Option<String>;

    /// Returns hidden, system, and reparse point flags for a directory entry.
    fn file_attributes(&self, metadata: &std::fs::Metadata, name: &str) -> (bool, bool, bool);

    /// Mark an application-owned staging or undo-backup file as hidden.
    fn set_hidden_attribute(&self, path: &std::path::Path, hidden: bool) -> Result<(), String>;

    /// Adjusts the current worker thread's scheduling priority.
    fn set_thread_priority(&self, priority: ThreadPriority);

    /// Deletes files permanently or sends them to the system trash.
    fn shell_delete(&self, paths: &[std::path::PathBuf], recycle: bool) -> Result<(), String>;

    /// Lists entries in the operating system's network neighborhood.
    fn list_network_vicinity(&self) -> Result<Vec<crate::models::FileEntry>, String>;
    fn list_network_shares(&self, server: &str) -> Result<Vec<crate::models::FileEntry>, String>;

    fn is_network_path(&self, path: &std::path::Path) -> bool;
    /// Blocking network operations; call on a background worker.
    fn map_network_drive(&self, letter: String, path: String, reconnect: bool, username: Option<String>, password: Option<String>) -> Result<(), String>;
    fn disconnect_network_drive(&self, path: String, force: bool) -> Result<(), String>;
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
    ) -> Result<(), crate::models::CommandError> {
        thumbnails::stream_video_frames_sync(path, on_frame)
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

    /// Couleur d'accent système (format hex "#rrggbb").
    fn get_accent_color(&self) -> String;

    /// Résout un fichier de raccourci (.lnk sur Windows, .desktop sur Linux).
    /// Retourne le chemin cible, ou `None` si le fichier n'est pas un raccourci connu.
    fn resolve_shortcut(&self, path: &str) -> Option<String>;

    /// Affiche la boîte de dialogue propriétés native pour un fichier/dossier.
    fn show_properties_dialog(&self, path: &str);

    /// Définit l'icône de l'application pour la barre des tâches et la fenêtre.
    fn set_application_icon(&self, window: raw_window_handle::RawWindowHandle, icon_bytes: &[u8]);

    /// Transfers selected file paths through the native file clipboard.
    fn set_clipboard_files(&self, paths: Vec<String>, cut: bool) -> Result<(), String>;

    /// Reads file paths and their copy/move intent from the native clipboard.
    fn get_clipboard_files(&self) -> Result<(Vec<String>, bool), String>;

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
