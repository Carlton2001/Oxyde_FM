use super::{Platform, ThreadPriority};
use crate::models::{DriveInfo, QuickAccessItem};

pub struct LinuxPlatform;

impl LinuxPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Platform for LinuxPlatform {
    fn recycle_bin_usage(&self) -> Result<(u64, u64), String> {
        let entries = crate::commands::ops::list_trash().map_err(|error| error.to_string())?;
        let mut size = 0_u64;
        for entry in &entries {
            let mut stack = vec![std::path::PathBuf::from(&entry.path)];
            while let Some(path) = stack.pop() {
                let Ok(metadata) = std::fs::symlink_metadata(&path) else { continue; };
                if metadata.is_dir() {
                    if let Ok(children) = std::fs::read_dir(path) { stack.extend(children.filter_map(Result::ok).map(|child| child.path())); }
                } else { size = size.saturating_add(metadata.len()); }
            }
        }
        Ok((size, entries.len() as u64))
    }

    fn recycle_bin_config(&self, _path: String) -> Result<crate::commands::ops::TrashSystemConfig, String> {
        Err("Recycle bin capacity settings are not supported on Linux".into())
    }
    fn set_recycle_bin_config(&self, _config: crate::commands::ops::TrashSystemConfig) -> Result<(), String> {
        Err("Recycle bin capacity settings are not supported on Linux".into())
    }

    fn finish_process_shutdown(&self) -> Result<(), String> {
        Ok(())
    }

    fn install_disk_image_autoplay_handler(&self, _window: raw_window_handle::RawWindowHandle) -> Result<(), String> {
        Ok(())
    }
    fn mounted_disk_images(&self) -> Result<Vec<super::MountedDiskImage>, String> {
        Ok(Vec::new())
    }
    fn mount_disk_image(&self, _path: &str) -> Result<Vec<String>, String> {
        Err("Disk image mounting is only supported on Windows".into())
    }
    fn unmount_disk_image(&self, _path: &str) -> Result<(), String> {
        Err("Disk image unmounting is only supported on Windows".into())
    }
    fn list_network_shares(&self, _server: &str) -> Result<Vec<crate::models::FileEntry>, String> {
        Err("Network discovery is only available on Windows".into())
    }
    fn map_network_drive(&self, _letter: String, _path: String, _reconnect: bool, _username: Option<String>, _password: Option<String>) -> Result<(), String> {
        Err("Network drive mapping is only available on Windows".into())
    }
    fn disconnect_network_drive(&self, _path: String, _force: bool) -> Result<(), super::NetworkDisconnectError> {
        Err(super::NetworkDisconnectError::Other("Network drive disconnect is only available on Windows".into()))
    }
    fn eject_drive(&self, path: &str) -> Result<(), String> {
        let output = std::process::Command::new("findmnt")
            .args(["--noheadings", "--output", "SOURCE", "--mountpoint", path])
            .output().map_err(|error| error.to_string())?;
        let device = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !output.status.success() || !device.starts_with("/dev/") {
            return Err("Cannot identify the mounted device".into());
        }
        let output = std::process::Command::new("udisksctl")
            .args(["unmount", "--block-device", &device, "--no-user-interaction"])
            .output().map_err(|error| error.to_string())?;
        if output.status.success() { Ok(()) } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().into())
        }
    }

    fn file_icon(&self, _path: String, _large: bool) -> Result<image::RgbaImage, String> {
        Err("Native icons unavailable".into())
    }

    fn set_hidden_attribute(&self, _path: &std::path::Path, _hidden: bool) -> Result<(), String> {
        // Linux uses dot-prefixed names for app-owned staging files.
        Ok(())
    }

    fn is_network_path(&self, path: &std::path::Path) -> bool {
        super::hardware::is_network_path(path)
    }

    fn physical_disk_id(&self, path: &std::path::Path) -> u64 {
        super::hardware::get_physical_disk_id(path)
    }

    fn is_ssd(&self, path: &std::path::Path) -> bool {
        super::hardware::is_ssd(path)
    }

    fn list_network_vicinity(&self) -> Result<Vec<crate::models::FileEntry>, String> {
        Err("Network neighborhood is unavailable on this platform".to_string())
    }

    fn shell_delete(&self, paths: &[std::path::PathBuf], recycle: bool) -> Result<(), String> {
        if recycle {
            return trash::delete_all(paths).map_err(|error| error.to_string());
        }
        for path in paths {
            let kind = std::fs::symlink_metadata(path)
                .map_err(|error| error.to_string())?
                .file_type();
            if kind.is_dir() {
                std::fs::remove_dir_all(path).map_err(|error| error.to_string())?;
            } else {
                std::fs::remove_file(path).map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    }

    fn set_thread_priority(&self, _priority: ThreadPriority) {}

    fn file_attributes(&self, _metadata: &std::fs::Metadata, name: &str) -> (bool, bool, bool) {
        (name.starts_with('.'), false, false)
    }

    fn set_clipboard_files(&self, _paths: Vec<String>, _cut: bool) -> Result<(), String> {
        Err("File clipboard is not available on this platform yet".to_string())
    }

    fn get_clipboard_files(&self) -> Result<(Vec<String>, bool), String> {
        Err("File clipboard is not available on this platform yet".to_string())
    }

    fn regional_locale(&self) -> Option<String> {
        ["LC_ALL", "LC_TIME", "LANG"].into_iter().filter_map(|key| std::env::var(key).ok()).find(|value| !value.is_empty())
    }
    fn interface_locale(&self) -> Option<String> {
        ["LC_ALL", "LANGUAGE", "LC_MESSAGES", "LANG"]
            .into_iter()
            .filter_map(|key| std::env::var(key).ok())
            .find(|value| !value.is_empty())
            .and_then(|value| value.split(':').find(|locale| !locale.is_empty()).map(str::to_owned))
    }
    fn name(&self) -> &'static str {
        "linux"
    }

    fn list_drives(&self, _skip_hardware_info: bool) -> Vec<DriveInfo> {
        // Systèmes de fichiers virtuels à ignorer
        const SKIP_FS: &[&str] = &[
            "proc",
            "sysfs",
            "devtmpfs",
            "devpts",
            "tmpfs",
            "cgroup",
            "cgroup2",
            "pstore",
            "bpf",
            "tracefs",
            "debugfs",
            "hugetlbfs",
            "mqueue",
            "fusectl",
            "configfs",
            "ramfs",
            "overlay",
            "aufs",
            "squashfs",
            "efivarfs",
            "securityfs",
            "autofs",
        ];

        let content = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
        let mut drives: Vec<DriveInfo> = content
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(4, ' ');
                let device = parts.next()?;
                let mount = parts.next()?;
                let fs_type = parts.next()?;
                let options = parts.next().unwrap_or("");

                if SKIP_FS.contains(&fs_type) {
                    return None;
                }
                if mount.starts_with("/proc")
                    || mount.starts_with("/sys")
                    || mount.starts_with("/dev/pts")
                    || mount.starts_with("/run")
                {
                    return None;
                }

                let label = if mount == "/" {
                    "Root".to_string()
                } else {
                    std::path::Path::new(mount)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| mount.to_string())
                };

                let drive_type = if device.starts_with("//")
                    || device.starts_with("cifs")
                    || fs_type == "nfs"
                    || fs_type == "nfs4"
                    || fs_type == "smbfs"
                {
                    "remote"
                } else if device.starts_with("/dev/sr")
                    || device.starts_with("/dev/cdrom")
                    || fs_type == "iso9660"
                    || fs_type == "udf"
                {
                    "cdrom"
                } else if device.starts_with("/dev/sd")
                    || device.starts_with("/dev/nvme")
                    || device.starts_with("/dev/mmcblk")
                    || device.starts_with("/dev/vd")
                {
                    "fixed"
                } else {
                    "fixed"
                }
                .to_string();

                let is_readonly = options.split(',').any(|o| o == "ro");

                Some(DriveInfo {
                    path: mount.to_string(),
                    label,
                    drive_type,
                    is_readonly,
                    // TODO: remplir via statvfs (nécessite la crate `nix`)
                    total_bytes: 0,
                    free_bytes: 0,
                    media_type: None,
                    physical_id: None,
                    remote_path: if device.starts_with("//") {
                        Some(device.to_string())
                    } else {
                        None
                    },
                })
            })
            .collect();

        if drives.is_empty() {
            drives.push(DriveInfo {
                path: "/".to_string(),
                label: "Root".to_string(),
                drive_type: "fixed".to_string(),
                is_readonly: false,
                total_bytes: 0,
                free_bytes: 0,
                media_type: None,
                physical_id: None,
                remote_path: None,
            });
        }

        drives
    }

    fn open_item(&self, path: &str) -> Result<(), String> {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn get_quick_access_items(&self) -> Vec<QuickAccessItem> {
        // Format GTK bookmarks : "file:///chemin/vers/dossier [Nom optionnel]"
        let home = dirs::home_dir().unwrap_or_default();
        let bookmarks_path = home.join(".config/gtk-3.0/bookmarks");

        let content = match std::fs::read_to_string(&bookmarks_path) {
            Ok(c) => c,
            Err(_) => {
                return vec![QuickAccessItem {
                    name: "Dossier personnel".to_string(),
                    path: home.to_string_lossy().to_string(),
                }]
            }
        };

        content
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() {
                    return None;
                }

                let mut parts = line.splitn(2, ' ');
                let uri = parts.next()?;
                let name_hint = parts.next();

                let path = uri.strip_prefix("file://")?;
                let decoded = percent_encoding::percent_decode_str(path)
                    .decode_utf8_lossy()
                    .to_string();

                let name = name_hint.map(|n| n.to_string()).unwrap_or_else(|| {
                    std::path::Path::new(&decoded)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| decoded.clone())
                });

                Some(QuickAccessItem {
                    name,
                    path: decoded,
                })
            })
            .collect()
    }

    fn get_accent_color(&self) -> String {
        // TODO: lire depuis les settings GTK / KDE
        "#0078d4".to_string()
    }

    fn resolve_shortcut(&self, path: &str) -> Option<String> {
        // Fichiers .desktop (freedesktop.org)
        let p = std::path::Path::new(path);
        if p.extension()?.eq_ignore_ascii_case("desktop") {
            let content = std::fs::read_to_string(p).ok()?;
            // Cherche d'abord Path= (répertoire de travail), puis Exec=
            for line in content.lines() {
                if let Some(val) = line.strip_prefix("Path=") {
                    let val = val.trim();
                    if !val.is_empty() {
                        return Some(val.to_string());
                    }
                }
            }
            for line in content.lines() {
                if let Some(exec) = line.strip_prefix("Exec=") {
                    // Supprime les codes de champs (%u, %f, %F, etc.)
                    let cmd = exec.trim().split_whitespace().next()?;
                    let cmd = cmd.trim_end_matches(|c: char| c == '%' || c.is_ascii_uppercase());
                    if !cmd.is_empty() {
                        return Some(cmd.to_string());
                    }
                }
            }
        }
        None
    }

    fn show_properties_dialog(&self, path: &str) {
        // Tentative via `gio info` ou `nautilus --properties` selon l'environnement
        let _ = std::process::Command::new("gio")
            .args(["open", path])
            .spawn();
    }

    fn set_application_icon(&self, _window: raw_window_handle::RawWindowHandle, _icon_bytes: &[u8]) {
        // TODO
    }
}
