use std::fs;

pub fn run_migration() {
    let old_id = "com.oxyde.app";

    let config_dir = dirs::config_dir().map(|p| p.join("oxyde"));
    let local_data_dir = dirs::data_local_dir().map(|p| p.join("oxyde"));

    // 1. Migrate Roaming (Config)
    if let Some(new_config_dir) = config_dir {
        if let Some(roaming_root) = new_config_dir.parent() {
            let old_config_dir = roaming_root.join(old_id);
            if old_config_dir.exists() {
                let _ = fs::create_dir_all(&new_config_dir);
                if let Ok(entries) = fs::read_dir(&old_config_dir) {
                    for entry in entries.flatten() {
                        let target = new_config_dir.join(entry.file_name());
                        if target.exists() {
                            let _ = fs::remove_file(&target);
                        }
                        let _ = fs::rename(entry.path(), &target);
                    }
                }
                let _ = fs::remove_dir_all(&old_config_dir);
            }
        }
    }

    // 2. Migrate Local (Cache)
    if let Some(new_local_dir) = local_data_dir.clone() {
        if let Some(local_root) = new_local_dir.parent() {
            let old_local_dir = local_root.join(old_id);
            if old_local_dir.exists() {
                let _ = fs::create_dir_all(&new_local_dir);
                if let Ok(entries) = fs::read_dir(&old_local_dir) {
                    for entry in entries.flatten() {
                        let target = new_local_dir.join(entry.file_name());
                        if target.exists() && target.is_file() {
                            let _ = fs::remove_file(&target);
                        }
                        let _ = fs::rename(entry.path(), &target);
                    }
                }
                let _ = fs::remove_dir_all(&old_local_dir);
            }
        }
    }
}
