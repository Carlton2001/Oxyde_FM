use crate::models::{DriveInfo, QuickAccessItem};

/// Abstraction des opérations spécifiques à l'OS.
/// Toute fonctionnalité native (lecteurs, ouverture, raccourcis, etc.)
/// doit passer par ce trait plutôt qu'appeler directement les API OS.
pub trait Platform: Send + Sync {
    fn name(&self) -> &'static str;

    /// Liste les points de montage / lecteurs disponibles.
    fn list_drives(&self, skip_hardware_info: bool) -> Vec<DriveInfo>;

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
    fn set_application_icon(&self, icon_bytes: &[u8]);
}

#[cfg(target_os = "windows")]
mod windows_impl;
#[cfg(not(target_os = "windows"))]
mod linux_impl;

#[cfg(target_os = "windows")]
use windows_impl::WindowsPlatform as CurrentPlatform;
#[cfg(not(target_os = "windows"))]
use linux_impl::LinuxPlatform as CurrentPlatform;

/// Retourne l'implémentation platform courante (singleton).
pub fn platform() -> &'static dyn Platform {
    use std::sync::OnceLock;
    static INSTANCE: OnceLock<CurrentPlatform> = OnceLock::new();
    INSTANCE.get_or_init(CurrentPlatform::new)
}
