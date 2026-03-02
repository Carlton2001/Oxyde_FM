use tauri::{AppHandle, Manager};
use std::fs;

pub fn run_migration(app: &AppHandle) {
    let old_id = "com.oxyde.app";
    let roaming_path = app.path().app_config_dir().ok();
    let local_path = app.path().app_local_data_dir().ok();

    // 1. Migrate Roaming (Config)
    if let Some(new_config_dir) = roaming_path.clone() {
        if let Some(roaming_root) = new_config_dir.parent() {
            let old_config_dir = roaming_root.join(old_id);
            if old_config_dir.exists() {
                let _ = fs::create_dir_all(&new_config_dir);
                if let Ok(entries) = fs::read_dir(&old_config_dir) {
                    for entry in entries.flatten() {
                        let target = new_config_dir.join(entry.file_name());
                        // On Windows, rename might fail if target exists. Let's delete it first.
                        if target.exists() { let _ = fs::remove_file(&target); }
                        let _ = fs::rename(entry.path(), target);
                    }
                }
                let _ = fs::remove_dir_all(&old_config_dir);
            }
        }
    }

    // 2. Migrate Local (Cache)
    if let Some(new_local_dir) = local_path.clone() {
        if let Some(local_root) = new_local_dir.parent() {
            let old_local_dir = local_root.join(old_id);
            if old_local_dir.exists() {
                let _ = fs::create_dir_all(&new_local_dir);
                if let Ok(entries) = fs::read_dir(&old_local_dir) {
                    for entry in entries.flatten() {
                        let target = new_local_dir.join(entry.file_name());
                        if target.exists() && target.is_file() { let _ = fs::remove_file(&target); }
                        let _ = fs::rename(entry.path(), target);
                    }
                }
                let _ = fs::remove_dir_all(&old_local_dir);
            }
        }
    }

    // 3. Move session from Roaming -> Local
    if let (Some(config_dir), Some(local_dir)) = (roaming_path, local_path) {
        let roaming_session = config_dir.join("session.json");
        let local_session = local_dir.join("session.json");

        if roaming_session.exists() {
            if !local_dir.exists() { let _ = fs::create_dir_all(&local_dir); }
            if local_session.exists() { let _ = fs::remove_file(&local_session); }
            let _ = fs::rename(roaming_session, local_session);
        }
    }
}
