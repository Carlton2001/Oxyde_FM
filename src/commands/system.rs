use crate::models::{CommandError, DriveInfo, QuickAccessItem};
use crate::utils::path_security::validate_path;
use log::info;

pub fn get_drives(skip_hardware_info: Option<bool>) -> Vec<DriveInfo> {
    crate::platform::platform().list_drives(skip_hardware_info.unwrap_or(false))
}

pub async fn open_item(path: String) -> Result<(), CommandError> {
    let normalized_path = if path.starts_with("::{") || path.starts_with("?") {
        path
    } else {
        validate_path(&path)?.to_string_lossy().into_owned()
    };
    info!("Opening item: {}", normalized_path);
    crate::platform::platform()
        .open_item(&normalized_path)
        .map_err(CommandError::SystemError)
}

pub fn get_quick_access_items() -> Result<Vec<QuickAccessItem>, CommandError> {
    Ok(crate::platform::platform().get_quick_access_items())
}

pub fn add_to_quick_access(path: String) -> Result<(), CommandError> {
    crate::platform::platform().pin_quick_access(&path)
}

pub fn remove_from_quick_access(path: String) -> Result<(), CommandError> {
    crate::platform::platform().unpin_quick_access(&path)
}

pub fn clear_app_cache() -> Result<(), CommandError> {
    #[cfg(target_os = "windows")]
    crate::platform::icons::purge_icon_cache();
    if let Some(root) = crate::app_paths::cache_dir() {
        clear_generated_cache(&root)?;
    }
    Ok(())
}

fn clear_generated_cache(root: &std::path::Path) -> Result<(), CommandError> {
    for name in ["icons", "thumbnails"] {
        let path = root.join(name);
        match std::fs::remove_dir_all(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(CommandError::IoError(error.to_string())),
        }
    }
    Ok(())
}

#[cfg(test)]
mod cache_tests {
    use super::clear_generated_cache;

    #[test]
    fn cleanup_preserves_configuration_and_unrelated_data() {
        let root = tempfile::tempdir().unwrap();
        for name in ["icons", "thumbnails", "other-data"] {
            let directory = root.path().join(name);
            std::fs::create_dir(&directory).unwrap();
            std::fs::write(directory.join("asset"), b"data").unwrap();
        }
        std::fs::write(root.path().join("config.json"), b"preferences").unwrap();
        clear_generated_cache(root.path()).unwrap();
        assert!(!root.path().join("icons").exists());
        assert!(!root.path().join("thumbnails").exists());
        assert!(root.path().join("other-data/asset").exists());
        assert_eq!(std::fs::read(root.path().join("config.json")).unwrap(), b"preferences");
        clear_generated_cache(root.path()).unwrap();
    }

    #[test]
    fn cleanup_reports_failure_for_an_invalid_cache_directory() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("icons"), b"invalid directory").unwrap();
        assert!(clear_generated_cache(root.path()).is_err());
    }
}

pub async fn open_peek(path: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || crate::platform::platform().open_peek(&path)).await.map_err(|error| error.to_string())?
}
