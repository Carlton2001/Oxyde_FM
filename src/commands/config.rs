use crate::models::{AppConfig, CommandError, ConfigManager};

pub fn get_config(state: &ConfigManager) -> Result<AppConfig, CommandError> {
    let config = state
        .0
        .lock()
        .map_err(|_| CommandError::SystemError("Failed to lock config".to_string()))?;
    Ok(config.clone())
}

pub fn set_config_value(
    state: &ConfigManager,
    key: String,
    value: String,
) -> Result<(), CommandError> {
    let mut config = state
        .0
        .lock()
        .map_err(|_| CommandError::SystemError("Failed to lock config".to_string()))?;

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
        "show_network" => config.show_network = value.parse().unwrap_or(true),
        "group_by_date" => config.group_by_date = value.parse().unwrap_or(false),
        "confirm_delete" => config.confirm_delete = value.parse().unwrap_or(true),
        "first_launch" => config.first_launch = value.parse().unwrap_or(true),
        _ => return Err(CommandError::Other(format!("Unknown config key: {}", key))),
    }

    state.save_config(&config)?;
    Ok(())
}

pub fn set_trash_settings(
    state: &ConfigManager,
    settings: Vec<crate::models::TrashDriveSettings>,
) -> Result<(), CommandError> {
    let mut config = state
        .0
        .lock()
        .map_err(|_| CommandError::SystemError("Failed to lock config".to_string()))?;
    config.trash_drive_settings = settings;
    state.save_config(&config)?;
    Ok(())
}

pub fn reset_config_to_default(state: &ConfigManager) -> Result<(), CommandError> {
    let default_config = AppConfig::default();
    let mut config = state
        .0
        .lock()
        .map_err(|_| CommandError::SystemError("Failed to lock config".to_string()))?;
    *config = default_config;
    state.save_config(&config)?;
    Ok(())
}
