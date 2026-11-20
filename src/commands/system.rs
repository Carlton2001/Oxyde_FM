use crate::models::{CommandError, DriveInfo, QuickAccessItem};

pub fn get_drives(skip_hardware_info: Option<bool>) -> Vec<DriveInfo> {
    crate::platform::platform().volumes().list(skip_hardware_info.unwrap_or(false))
}

pub fn get_quick_access_items() -> Result<Vec<QuickAccessItem>, CommandError> {
    Ok(crate::platform::platform().quick_access().items())
}

pub fn add_to_quick_access(path: String) -> Result<(), CommandError> {
    crate::platform::platform().quick_access().pin(&path)
}

pub fn remove_from_quick_access(path: String) -> Result<(), CommandError> {
    crate::platform::platform().quick_access().unpin(&path)
}

pub fn clear_app_cache() -> Result<(), CommandError> {
    #[cfg(target_os = "windows")]
    let _pause = crate::platform::icons::pause_cache_writes();
    #[cfg(target_os = "windows")]
    crate::platform::icons::purge_icon_cache();
    if let Some(root) = crate::app_paths::cache_dir() {
        clear_generated_cache(&root)?;
    }
    Ok(())
}

/// Both folders are tried: one held open (a thumbnail being written) must not
/// leave the other in place while the interface has already dropped its images.
fn clear_generated_cache(root: &std::path::Path) -> Result<(), CommandError> {
    let mut first_error = None;
    for name in ["icons", "thumbnails"] {
        let path = root.join(name);
        match std::fs::remove_dir_all(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                first_error.get_or_insert(CommandError::IoError(format!("{}: {error}", path.display())));
            }
        }
    }
    first_error.map_or(Ok(()), Err)
}

pub async fn open_peek(path: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || crate::platform::platform().open_peek(&path)).await.map_err(|error| error.to_string())?
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
