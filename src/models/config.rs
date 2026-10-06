use crate::models::CommandError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Mutex;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashDriveSettings {
    pub drive_path: String,
    pub max_size_mb: u64,
    pub use_trash: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub layout: String,
    #[serde(default)]
    pub show_hidden: bool,
    #[serde(default)]
    pub show_system: bool,
    #[serde(default)]
    pub use_system_icons: bool,
    #[serde(default)]
    pub date_format: String,
    #[serde(default = "default_true")]
    pub show_previews: bool,
    #[serde(default)]
    pub zip_quality: String,
    #[serde(default)]
    pub seven_zip_quality: String,
    #[serde(default)]
    pub zstd_quality: String,
    #[serde(default)]
    pub font_size: u32,
    /// GPUI reference in logical pixels per rem, independent of legacy font_size.
    #[serde(default = "default_ui_rem_size")]
    pub ui_rem_size: u32,
    #[serde(default)]
    pub search_limit: u32,
    #[serde(default)]
    pub default_turbo_mode: bool,
    #[serde(default)]
    pub show_grid_thumbnails: bool,
    #[serde(default)]
    pub show_checkboxes: bool,
    #[serde(default = "default_true")]
    pub sync_sidebar_with_path: bool,
    #[serde(default)]
    pub show_network: bool,
    /// Window-local top-left coordinates for the operations panel.
    #[serde(default)]
    pub operations_panel_position: Option<[f32; 2]>,
    #[serde(default)]
    pub group_by_date: bool,
    #[serde(default)]
    pub trash_drive_settings: Vec<TrashDriveSettings>,
    #[serde(default = "default_true")]
    pub confirm_delete: bool,
    #[serde(default = "default_true")]
    pub first_launch: bool,
    /// Preserve options written by other versions when saving UI preferences.
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

fn default_theme() -> String {
    "dark".to_string()
}
fn default_language() -> String {
    "fr".to_string()
}

fn default_true() -> bool {
    true
}

fn default_ui_rem_size() -> u32 { 16 }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            language: default_language(),
            layout: "standard".to_string(),
            show_hidden: false,
            show_system: false,
            use_system_icons: false,
            date_format: String::new(),
            show_previews: true,
            zip_quality: "fast".to_string(),
            seven_zip_quality: "fast".to_string(),
            zstd_quality: "fast".to_string(),
            font_size: 16,
            ui_rem_size: default_ui_rem_size(),
            search_limit: 3000,
            default_turbo_mode: true,
            show_grid_thumbnails: false,
            show_checkboxes: false,
            sync_sidebar_with_path: true,
            show_network: true,
            operations_panel_position: None,
            group_by_date: false,
            trash_drive_settings: Vec::new(),
            confirm_delete: true,
            first_launch: true,
            extra: Default::default(),
        }
    }
}

impl AppConfig {
    /// Restore known preferences while retaining workspace and extension data.
    pub fn reset_preferences(&mut self) {
        let extra = std::mem::take(&mut self.extra);
        let operations_panel_position = self.operations_panel_position;
        *self = Self { extra, operations_panel_position, ..Self::default() };
    }
}

#[cfg(test)]
mod reset_tests {
    use super::AppConfig;

    #[test]
    fn reset_restores_defaults_and_preserves_workspace_and_unknown_options() {
        let mut config = AppConfig::default();
        config.theme = "light".into();
        config.language = "en".into();
        config.ui_rem_size = 24;
        config.show_hidden = true;
        config.zip_quality = "best".into();
        config.extra.insert("workspace".into(), serde_json::json!({"tabs": ["C:/"]}));
        config.extra.insert("unknown".into(), serde_json::json!(42));
        config.operations_panel_position = Some([20.0, 30.0]);
        let extra = config.extra.clone();
        config.reset_preferences();
        assert_eq!(config.theme, "dark");
        assert_eq!(config.language, "fr");
        assert_eq!(config.ui_rem_size, 16);
        assert!(!config.show_hidden);
        assert_eq!(config.zip_quality, "fast");
        assert_eq!(config.extra, extra);
        assert_eq!(config.operations_panel_position, Some([20.0, 30.0]));
    }
}

pub struct ConfigManager(pub Mutex<AppConfig>);

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigManager {
    pub fn new() -> Self {
        Self(Mutex::new(AppConfig::default()))
    }

    pub fn save(&self) -> Result<(), CommandError> {
        let config = self
            .0
            .lock()
            .map_err(|_| CommandError::SystemError("Failed to lock config".to_string()))?;
        self.save_config(&config)
    }

    pub fn config_path() -> Result<std::path::PathBuf, CommandError> {
        dirs::config_dir()
            .map(|dir| dir.join("oxyde").join("config.json"))
            .ok_or_else(|| CommandError::SystemError("Failed to get config directory".into()))
    }

    pub fn read_from_path(path: &std::path::Path) -> Result<AppConfig, CommandError> {
        match fs::read_to_string(path) {
            Ok(content) => {
                serde_json::from_str(&content).map_err(|e| CommandError::Other(e.to_string()))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(AppConfig::default()),
            Err(error) => Err(CommandError::IoError(error.to_string())),
        }
    }

    /// Replace only after the complete configuration has been written successfully.
    pub fn write_to_path(path: &std::path::Path, config: &AppConfig) -> Result<(), CommandError> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        fs::create_dir_all(parent).map_err(|e| CommandError::IoError(e.to_string()))?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .map_err(|e| CommandError::IoError(e.to_string()))?;
        serde_json::to_writer_pretty(&mut temporary, config)
            .map_err(|e| CommandError::Other(e.to_string()))?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|e| CommandError::IoError(e.to_string()))?;
        temporary
            .persist(path)
            .map_err(|e| CommandError::IoError(e.to_string()))?;
        Ok(())
    }

    pub fn save_config(&self, config: &AppConfig) -> Result<(), CommandError> {
        Self::write_to_path(&Self::config_path()?, config)
    }

    pub fn load(&self) -> Result<(), CommandError> {
        let loaded_config = Self::read_from_path(&Self::config_path()?)?;
        let mut config = self
            .0
            .lock()
            .map_err(|_| CommandError::SystemError("Failed to lock config".to_string()))?;
        *config = loaded_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::preferences::{Language, ThemeId};

    #[test]
    fn older_configs_have_defaults_and_legacy_identifiers_still_resolve() {
        let missing: AppConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(missing.theme, "dark");
        assert_eq!(missing.language, "fr");
        assert_eq!(missing.ui_rem_size, 16);
        assert!(!missing.use_system_icons);
        assert!(!missing.show_checkboxes);
        assert!(missing.sync_sidebar_with_path);
        let legacy: AppConfig =
            serde_json::from_str(r#"{"theme":"oxyde-light","language":"en-US"}"#).unwrap();
        assert_eq!(ThemeId::from_id(&legacy.theme), ThemeId::Light);
        assert_eq!(Language::from_id(&legacy.language), Language::English);
        assert_eq!(ThemeId::from_id(""), ThemeId::Dark);
        assert_eq!(ThemeId::from_id("unknown"), ThemeId::Dark);
        assert_eq!(Language::from_id("de-DE"), Language::French);
    }

    #[test]
    fn preferences_survive_reload_without_discarding_other_options() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, r#"{"theme":"oxyde-dark","language":"fr","search_limit":123,"future_option":{"enabled":true}}"#).unwrap();
        let mut config = ConfigManager::read_from_path(&path).unwrap();
        config.operations_panel_position = Some([42.5, 87.25]);
        config.ui_rem_size = 20;
        config.date_format = "ISO".into();
        for (theme, language) in [
            (ThemeId::Light, Language::English),
            (ThemeId::Dark, Language::French),
        ] {
            config.theme = theme.as_str().into();
            config.language = language.as_str().into();
            config.use_system_icons = theme == ThemeId::Light;
            config.show_checkboxes = theme == ThemeId::Light;
            config.show_network = theme == ThemeId::Light;
            config.sync_sidebar_with_path = theme == ThemeId::Light;
            ConfigManager::write_to_path(&path, &config).unwrap();
            config = ConfigManager::read_from_path(&path).unwrap();
            assert_eq!(ThemeId::from_id(&config.theme), theme);
            assert_eq!(Language::from_id(&config.language), language);
            assert_eq!(config.search_limit, 123);
            assert_eq!(config.use_system_icons, theme == ThemeId::Light);
            assert_eq!(config.show_checkboxes, theme == ThemeId::Light);
            assert_eq!(config.sync_sidebar_with_path, theme == ThemeId::Light);
            assert_eq!(config.ui_rem_size, 20);
            assert_eq!(config.date_format, "ISO");
            assert_eq!(config.show_network, theme == ThemeId::Light);
            assert_eq!(config.operations_panel_position, Some([42.5, 87.25]));
            assert_eq!(config.extra["future_option"]["enabled"], true);
        }
    }

    #[test]
    fn corrupt_config_is_reported_and_left_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, "{broken").unwrap();
        assert!(ConfigManager::read_from_path(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{broken");
        assert_eq!(
            ConfigManager::read_from_path(&dir.path().join("missing"))
                .unwrap()
                .language,
            "fr"
        );
    }
}
