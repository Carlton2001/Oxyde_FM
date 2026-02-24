use tauri::{AppHandle, State};
use crate::models::{AppConfig, ConfigManager, CommandError};

#[tauri::command]
pub fn get_config(state: State<'_, ConfigManager>) -> Result<AppConfig, CommandError> {
    let config = state.0.lock().map_err(|_| CommandError::SystemError("Failed to lock config".to_string()))?;
    Ok(config.clone())
}

#[tauri::command]
pub fn set_config_value(
    app: AppHandle,
    state: State<'_, ConfigManager>,
    key: String,
    value: String,
) -> Result<(), CommandError> {
    let mut config = state.0.lock().map_err(|_| CommandError::SystemError("Failed to lock config".to_string()))?;
    
    match key.as_str() {
        "theme" => config.theme = value,
        "language" => config.language = value,
        "layout" => config.layout = value,
        "show_hidden" => config.show_hidden = value.parse().unwrap_or(false),
        "show_system" => config.show_system = value.parse().unwrap_or(false),
        "use_system_icons" => config.use_system_icons = value.parse().unwrap_or(true),
        "date_format" => config.date_format = value,
        "show_previews" => config.show_previews = value.parse().unwrap_or(true),
        "zip_quality" => config.zip_quality = value,
        "seven_zip_quality" => config.seven_zip_quality = value,
        "zstd_quality" => config.zstd_quality = value,
        "font_size" => config.font_size = value.parse().unwrap_or(16),
        "search_limit" => config.search_limit = value.parse().unwrap_or(3000),
        "default_turbo_mode" => config.default_turbo_mode = value.parse().unwrap_or(false),
        "show_grid_thumbnails" => config.show_grid_thumbnails = value.parse().unwrap_or(true),
        "show_checkboxes" => config.show_checkboxes = value.parse().unwrap_or(false),
        _ => return Err(CommandError::Other(format!("Unknown config key: {}", key))),
    }

    // Save using the internal helper without re-locking, 
    // OR we could drop lock and call save(). 
    // Since we hold the lock and have the updated data, we can just call save_config with it.
    state.save_config(&app, &config)?;
    Ok(())
}
