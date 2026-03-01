use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
#[cfg(target_os = "windows")]
use windows::Win32::System::Registry::{RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ};
#[cfg(target_os = "windows")]
use windows::core::PCWSTR;
use crate::models::CommandError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub theme: String,
    pub language: String,
    pub layout: String,
    pub show_hidden: bool,
    pub show_system: bool,
    pub use_system_icons: bool,
    pub date_format: String,
    pub show_previews: bool,
    pub zip_quality: String,
    pub seven_zip_quality: String,
    pub zstd_quality: String,
    pub font_size: u32,
    pub search_limit: u32,
    pub default_turbo_mode: bool,
    pub show_grid_thumbnails: bool,
    pub show_checkboxes: bool,
    pub show_network: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        #[cfg(target_os = "windows")]
        let (theme, language, date_format) = {
            // 1. Detect Theme (Personalize\AppsUseLightTheme)
            let mut apps_use_light = 0u32;
            let mut data_len = std::mem::size_of::<u32>() as u32;
            let theme = unsafe {
                let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
                    .encode_utf16()
                    .collect();
                let value_name: Vec<u16> = "AppsUseLightTheme\0".encode_utf16().collect();
                let mut hkey = windows::Win32::System::Registry::HKEY::default();
                
                if RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(subkey.as_ptr()), Some(0), KEY_READ, &mut hkey).is_ok() {
                    if RegQueryValueExW(hkey, PCWSTR(value_name.as_ptr()), None, None, Some(&mut apps_use_light as *mut u32 as *mut u8), Some(&mut data_len)).is_ok() {
                        if apps_use_light == 1 { "oxyde-light".to_string() } else { "oxyde-dark".to_string() }
                    } else {
                        "oxyde-dark".to_string()
                    }
                } else {
                    "oxyde-dark".to_string()
                }
            };
            // 2. Detect Language
            let language = unsafe {
                let len = windows::Win32::Globalization::GetUserDefaultUILanguage();
                if len == 0x040c { "fr".to_string() } else { "en".to_string() }
            };

            // 3. Detect Date Format (Simplified)
            let date_format = unsafe {
                let mut locale_buf = [0u16; 85];
                let len = windows::Win32::Globalization::GetUserDefaultLocaleName(&mut locale_buf);
                if len > 0 {
                    let name = String::from_utf16_lossy(&locale_buf[..len as usize]);
                    if name.contains("US") || name.contains("en-US") {
                        "US".to_string()
                    } else {
                        "European".to_string()
                    }
                } else {
                    "European".to_string()
                }
            };

            (theme, language, date_format)
        };

        #[cfg(not(target_os = "windows"))]
        let (theme, language, date_format) = ("oxyde-dark".to_string(), "en".to_string(), "European".to_string());

        Self {
            theme,
            language,
            layout: "standard".to_string(),
            show_hidden: false,
            show_system: false,
            use_system_icons: false,
            date_format,
            show_previews: true,
            zip_quality: "fast".to_string(),
            seven_zip_quality: "fast".to_string(),
            zstd_quality: "fast".to_string(),
            font_size: 16,
            search_limit: 3000,
            default_turbo_mode: true,
            show_grid_thumbnails: false,
            show_checkboxes: false,
            show_network: true,
        }
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

    pub fn save(&self, app_handle: &AppHandle) -> Result<(), CommandError> {
        let config = self.0.lock().map_err(|_| CommandError::SystemError("Failed to lock config".to_string()))?;
        self.save_config(app_handle, &config)
    }

    pub fn save_config(&self, app_handle: &AppHandle, config: &AppConfig) -> Result<(), CommandError> {
        let config_dir = app_handle.path().app_config_dir().map_err(|e| CommandError::IoError(e.to_string()))?;
        
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).map_err(|e| CommandError::IoError(e.to_string()))?;
        }
        
        let config_path = config_dir.join("config.json");
        let json = serde_json::to_string_pretty(config).map_err(|e| CommandError::Other(e.to_string()))?;
        
        fs::write(config_path, json).map_err(|e| CommandError::IoError(e.to_string()))?;
        Ok(())
    }

    pub fn load(&self, app_handle: &AppHandle) -> Result<(), CommandError> {
        let config_dir = app_handle.path().app_config_dir().map_err(|e| CommandError::IoError(e.to_string()))?;
        let config_path = config_dir.join("config.json");

        if config_path.exists() {
            let content = fs::read_to_string(config_path).map_err(|e| CommandError::IoError(e.to_string()))?;
            let loaded_config: AppConfig = serde_json::from_str(&content).map_err(|e| CommandError::Other(e.to_string()))?;
            
            let mut config = self.0.lock().map_err(|_| CommandError::SystemError("Failed to lock config".to_string()))?;
            *config = loaded_config;
        }
        Ok(())
    }
}
