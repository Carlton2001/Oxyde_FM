use super::{Platform, ThreadPriority};
use crate::models::{DriveInfo, QuickAccessItem};

pub struct LinuxPlatform;

impl LinuxPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Platform for LinuxPlatform {
    fn retained_file_path(&self, _file: &std::fs::File, fallback: &std::path::Path) -> std::io::Result<std::path::PathBuf> { Ok(fallback.to_path_buf()) }
    fn run_archive_io_worker(&self, _operation: crate::utils::archive_io_protocol::Operation,
        _controls: &dyn Fn() -> std::io::Result<crate::utils::archive_io_protocol::Control>,
        _handler: &mut dyn FnMut(crate::utils::archive_io_protocol::Frame) -> std::io::Result<crate::utils::archive_io_protocol::Frame>) -> std::io::Result<std::time::Duration> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "Archive process isolation is unavailable on this platform"))
    }
    fn index_archive_contents(&self, path: &std::path::Path, checkpoint: &dyn Fn() -> std::io::Result<()>) -> Result<Vec<crate::models::FileEntry>, crate::models::CommandError> {
        crate::commands::archive::index_archive_contents_local(path, checkpoint)
    }
    fn list_archive_directory(&self, path: &std::path::Path, internal: &str) -> Result<Vec<crate::models::FileEntry>, crate::models::CommandError> {
        crate::commands::archive::list_archive_contents_local(path.to_string_lossy().into_owned(), internal.into())
    }
    fn open_archive_source(&self, path: &std::path::Path, _allow_changes: bool) -> std::io::Result<std::fs::File> {
        let file = std::fs::File::open(path)?;
        if !file.metadata()?.is_file() {return Err(std::io::Error::other("Archive source is not a regular file"));}
        Ok(file)
    }
    fn file_identity(&self, file: &std::fs::File) -> std::io::Result<super::FileIdentity> {
        use std::os::unix::fs::MetadataExt;
        let metadata = file.metadata()?;
        let mut value = [0u8; 16];
        value[..8].copy_from_slice(&metadata.ino().to_le_bytes());
        Ok(super::FileIdentity {volume: metadata.dev(), value, kind: 3})
    }
    fn system_executable(&self, _executable: super::SystemExecutable) -> std::io::Result<std::path::PathBuf> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "Windows system executable is unavailable on this platform"))
    }
    fn hold_archive_directory(&self, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        let metadata = std::fs::symlink_metadata(path)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(std::io::Error::other("Archive parent is not an ordinary directory"));
        }
        std::fs::File::open(path)
    }
    fn hold_operation_directory(&self, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        std::fs::File::open(path)
    }
    fn hold_resolved_operation_directory(&self, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        self.hold_archive_directory(path)
    }
    fn ensure_operation_directory(&self, _parent: &std::fs::File, path: &std::path::Path) -> std::io::Result<(std::fs::File, bool)> {
        let created = match std::fs::create_dir(path) {
            Ok(()) => true,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
            Err(error) => return Err(error),
        };
        Ok((self.hold_archive_directory(path)?, created))
    }
    fn track_operation_directory(&self, _parent: &std::fs::File, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        self.hold_archive_directory(path)
    }
    fn create_journal_directory(&self, _parent: &std::fs::File, path: &std::path::Path, _hidden: bool) -> std::io::Result<std::fs::File> {
        std::fs::create_dir(path)?;
        self.hold_archive_directory(path)
    }
    fn restore_journal_directory(&self, directory: &std::fs::File, _path: &std::path::Path, permissions: std::fs::Permissions, _hidden: bool) -> std::io::Result<()> {
        directory.set_permissions(permissions)
    }
    fn remove_empty_operation_directory(&self, parent: &std::fs::File, path: &std::path::Path, expected: super::FileIdentity) -> std::io::Result<()> {
        let directory = self.track_operation_directory(parent, path)?;
        if self.file_identity(&directory)? != expected {
            return Err(std::io::Error::other("Source directory changed; its replacement was preserved"));
        }
        // This fallback is still path based; it does not claim Windows' atomic guarantees.
        std::fs::remove_dir(path)
    }
    fn open_operation_file(&self, _parent: &std::fs::File, path: &std::path::Path, _read_contents: bool) -> std::io::Result<std::fs::File> {
        if !std::fs::symlink_metadata(path)?.is_file() {
            return Err(std::io::Error::other("Operation source is not an ordinary file"));
        }
        self.open_file_for_move(path)
    }
    fn track_source_item(&self, parent: &std::fs::File, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        #[cfg(target_os = "linux")]
        { super::linux_source_selection::open(parent, path, None) }
        #[cfg(not(target_os = "linux"))]
        { let _ = (parent, path); Err(std::io::ErrorKind::Unsupported.into()) }
    }
    fn open_source_directory(&self, parent: &std::fs::File, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        #[cfg(target_os = "linux")]
        { super::linux_source_selection::open(parent, path, Some(true)) }
        #[cfg(not(target_os = "linux"))]
        { let _ = (parent, path); Err(std::io::ErrorKind::Unsupported.into()) }
    }
    fn open_copy_source(&self, parent: &std::fs::File, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        #[cfg(target_os = "linux")]
        { super::linux_source_selection::open(parent, path, Some(false)) }
        #[cfg(not(target_os = "linux"))]
        { let _ = (parent, path); Err(std::io::ErrorKind::Unsupported.into()) }
    }
    fn enumerate_source_directory(&self, directory: &std::fs::File, visitor: &mut dyn FnMut(std::ffi::OsString) -> std::io::Result<()>) -> std::io::Result<()> {
        #[cfg(target_os = "linux")]
        { super::linux_source_selection::enumerate(directory, visitor) }
        #[cfg(not(target_os = "linux"))]
        { let _ = (directory, visitor); Err(std::io::ErrorKind::Unsupported.into()) }
    }
    fn open_copied_file(&self, parent: &std::fs::File, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        self.open_operation_file(parent, path, true)
    }
    fn open_rename_item(
        &self,
        _parent: &std::fs::File,
        path: &std::path::Path,
    ) -> std::io::Result<std::fs::File> {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // O_PATH | O_NOFOLLOW: metadata-only, including the final symlink itself.
            std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(0x200000 | 0x20000)
                .open(path)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = path;
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Exclusive item rename is unavailable on this platform",
            ))
        }
    }
    fn rename_operation_item(
        &self,
        _file: &std::fs::File,
        source: &std::path::Path,
        source_parent: &std::fs::File,
        parent: &std::fs::File,
        destination: &std::path::Path,
    ) -> std::io::Result<()> {
        #[cfg(target_os = "linux")]
        {
            use std::ffi::{CString, c_char, c_int, c_uint};
            use std::os::unix::{ffi::OsStrExt, io::AsRawFd};
            unsafe extern "C" {
                fn renameat2(
                    old_dir: c_int,
                    old_name: *const c_char,
                    new_dir: c_int,
                    new_name: *const c_char,
                    flags: c_uint,
                ) -> c_int;
            }
            let leaf = |path: &std::path::Path| -> std::io::Result<CString> {
                let name = path.file_name().ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Rename path has no file name",
                    )
                })?;
                CString::new(name.as_bytes()).map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid rename name")
                })
            };
            let source = leaf(source)?;
            let destination = leaf(destination)?;
            // RENAME_NOREPLACE. No path-based replacement fallback if unsupported.
            // Linux does not pin source names against concurrent external replacement.
            let result = unsafe {
                renameat2(
                    source_parent.as_raw_fd(),
                    source.as_ptr(),
                    parent.as_raw_fd(),
                    destination.as_ptr(),
                    1,
                )
            };
            if result == 0 {
                Ok(())
            } else {
                Err(std::io::Error::last_os_error())
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (source, source_parent, parent, destination);
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Exclusive item rename is unavailable on this platform",
            ))
        }
    }
    fn remove_copied_file(&self, _file: &std::fs::File, path: &std::path::Path) -> std::io::Result<()> {
        std::fs::remove_file(path)
    }
    fn rename_operation_file(&self, file: &std::fs::File, source: &std::path::Path, _parent: &std::fs::File, destination: &std::path::Path, replace: bool) -> std::io::Result<()> {
        if replace { self.replace_open_file(file, source, destination) }
        else { self.rename_open_file_exclusive(file, source, destination) }
    }
    fn remove_operation_tree(&self, _parent: &std::fs::File, path: &std::path::Path, checkpoint: &mut dyn FnMut() -> std::io::Result<()>) -> std::io::Result<()> {
        checkpoint()?;
        if std::fs::symlink_metadata(path)?.is_dir() { std::fs::remove_dir_all(path) }
        else { std::fs::remove_file(path) }
    }
    fn create_copy_staging(&self, _parent: &std::fs::File, path: &std::path::Path) -> std::io::Result<(std::fs::File, std::fs::File)> {
        std::fs::create_dir(path)?;
        let result = (|| {
            let directory = std::fs::File::open(path)?;
            let pin = std::fs::OpenOptions::new().read(true).write(true).create_new(true).open(path.join(".pin"))?;
            Ok((directory, pin))
        })();
        if result.is_err() { let _ = std::fs::remove_dir(path); }
        result
    }
    fn create_operation_output(&self, _parent: &std::fs::File, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        std::fs::OpenOptions::new().read(true).write(true).create_new(true).open(path)
    }
    fn remove_copy_staging(&self, _directory: &std::fs::File, pin: std::fs::File, path: &std::path::Path) -> std::io::Result<()> {
        drop(pin);
        std::fs::remove_file(path.join(".pin"))?;
        std::fs::remove_dir(path)
    }
    fn lock_cache_file(&self, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        let file = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).open(path)?;
        file.try_lock().map_err(std::io::Error::from)?;
        Ok(file)
    }
    fn read_cache_file(&self, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        if !std::fs::symlink_metadata(path)?.is_file() {
            return Err(std::io::Error::other("Cache marker is not an ordinary file"));
        }
        std::fs::File::open(path)
    }
    fn write_preview_cache(
        &self,
        path: &std::path::Path,
        temporary: &std::path::Path,
        write: &mut dyn FnMut(&mut std::fs::File) -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        let path = std::path::absolute(path)?;
        let temporary = std::path::absolute(temporary)?;
        if path == temporary || path.parent() != temporary.parent() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Cache staging requires a separate sibling path",
            ));
        }
        std::fs::create_dir_all(
            path.parent()
                .ok_or_else(|| std::io::Error::other("Cache has no parent"))?,
        )?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let result = (|| {
            write(&mut file)?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(&temporary, &path)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    }
    fn open_cache_cleanup(&self, _path: &std::path::Path, _directory: bool) -> std::io::Result<std::fs::File> {
        // Unix flock cannot prove that external readers have closed a file.
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "Safe cache eviction requires Windows sharing checks"))
    }
    fn ensure_archive_directory(&self, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        match std::fs::create_dir(path) {
            Ok(()) => {},
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {},
            Err(error) => return Err(error),
        }
        self.hold_archive_directory(path)
    }
    fn create_archive_file(&self, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        std::fs::OpenOptions::new().write(true).create_new(true).open(path)
    }
    fn read_file_provenance(&self, _path: &std::path::Path) -> std::io::Result<Option<Vec<u8>>> {
        Ok(None)
    }
    fn write_file_provenance(&self, _path: &std::path::Path, _data: &[u8]) -> std::io::Result<()> {
        Ok(())
    }
    fn write_archive_output_provenance(&self, _parent: &std::fs::File, _file: &std::fs::File, _path: &std::path::Path, _data: &[u8]) -> std::io::Result<()> { Ok(()) }
    fn read_archive_output_provenance(&self, _parent: &std::fs::File, _file: &std::fs::File, _path: &std::path::Path) -> std::io::Result<Option<Vec<u8>>> { Ok(None) }
    fn replace_open_file(
        &self,
        _file: &std::fs::File,
        source: &std::path::Path,
        destination: &std::path::Path,
    ) -> std::io::Result<()> {
        std::fs::rename(source, destination)
    }
    fn copy_file_with_progress(
        &self,
        source: &std::path::Path,
        destination: &std::path::Path,
        _held_source: Option<&std::fs::File>,
        _destination_parent: &std::fs::File,
        created: &mut dyn FnMut(&std::fs::File) -> std::io::Result<()>,
        progress: &mut dyn FnMut(u64) -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        use std::io::{Read, Write};
        let mut input = std::fs::File::open(source)?;
        let metadata = input.metadata()?;
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;
        created(&output)?;
        let mut buffer = vec![0_u8; 1024 * 1024];
        let mut copied = 0;
        loop {
            progress(copied)?;
            let count = input.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            output.write_all(&buffer[..count])?;
            copied += count as u64;
        }
        output.sync_all()?;
        output.set_permissions(metadata.permissions())?;
        output.set_times(std::fs::FileTimes::new().set_modified(metadata.modified()?))?;
        Ok(())
    }
    fn open_file_for_move(&self, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        std::fs::File::open(path)
    }
    fn rename_open_file_exclusive(
        &self,
        _file: &std::fs::File,
        source: &std::path::Path,
        destination: &std::path::Path,
    ) -> std::io::Result<()> {
        std::fs::hard_link(source, destination)?;
        if let Err(error) = std::fs::remove_file(source) {
            let _ = std::fs::remove_file(destination);
            return Err(error);
        }
        Ok(())
    }
    fn remove_open_file(
        &self,
        _file: &std::fs::File,
        path: &std::path::Path,
    ) -> std::io::Result<()> {
        std::fs::remove_file(path)
    }
    fn remove_open_empty_directory(
        &self,
        directory: &std::fs::File,
        path: &std::path::Path,
    ) -> std::io::Result<()> {
        let metadata = directory.metadata()?;
        if !metadata.is_dir() || self.file_attributes(&metadata, "").2 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Ordinary directory required",
            ));
        }
        let parents = crate::systems::path_parents::HeldParents::new(path)?;
        // Existing fallback remains path based; no Windows-style atomic claim.
        self.remove_empty_operation_directory(
            parents.directory(),
            &parents.resolved_path(path)?,
            self.file_identity(directory)?,
        )
    }
    fn list_trash_items(&self) -> Result<Vec<trash::TrashItem>, String> {
        trash::os_limited::list().map_err(|error| error.to_string())
    }
    fn restore_trash_items(&self, items: Vec<trash::TrashItem>) -> Result<(), String> {
        trash::os_limited::restore_all(items).map_err(|error| error.to_string())
    }
    fn purge_trash_items(&self, items: Vec<trash::TrashItem>) -> Result<(), String> {
        trash::os_limited::purge_all(items).map_err(|error| error.to_string())
    }
    fn empty_trash(&self) -> Result<(), String> {
        self.purge_trash_items(self.list_trash_items()?)
    }
    fn bulk_expand_excluded(&self, _path: &str, name: &str) -> bool {
        // Preserve the previous shared exclusions; Windows drive rules stay native.
        matches!(
            name.to_ascii_lowercase().as_str(),
            "$recycle.bin" | "system volume information"
        )
    }

    fn trash_original_path(&self, _id: &std::path::Path) -> Option<std::path::PathBuf> { None }

    fn trash_transfer_entries(&self, _paths: &[std::path::PathBuf]) -> Result<Vec<super::TrashTransferEntry>, String> { Ok(Vec::new()) }
    fn detach_trash_metadata(&self, _entry: &super::TrashTransferEntry) -> Result<Option<super::DetachedTrashMetadata>, String> { Ok(None) }

    fn peek_status(&self) -> Result<super::PeekStatus, String> { Ok(super::PeekStatus::default()) }
    fn open_peek(&self, _path: &str) -> Result<(), String> { Err("Peek is only available on Windows".into()) }
    fn shortcut_info(&self, _path: &std::path::Path) -> Option<crate::models::ShortcutInfo> { None }
    fn set_shortcut_info(&self, _path: &str, _info: &crate::models::ShortcutInfo) -> Result<(), String> { Err("Shortcut editing is only available on Windows".into()) }
    fn watch_quick_access_changes(&self, _revision: std::sync::Arc<std::sync::atomic::AtomicU64>) -> Result<Option<Box<dyn Send>>, String> { Ok(None) }

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

    fn clipboard_has_file_format(&self) -> bool {
        false
    }

    fn regional_locale(&self) -> Option<String> {
        ["LC_ALL", "LC_TIME", "LANG"].into_iter().filter_map(|key| std::env::var(key).ok()).find(|value| !value.is_empty())
    }
    fn calendar_timezone_signature(&self) -> Option<String> { None }

    fn interface_locale(&self) -> Option<String> {
        ["LC_ALL", "LANGUAGE", "LC_MESSAGES", "LANG"]
            .into_iter()
            .filter_map(|key| std::env::var(key).ok())
            .find(|value| !value.is_empty())
            .and_then(|value| value.split(':').find(|locale| !locale.is_empty()).map(str::to_owned))
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

                Some(DriveInfo {
                    path: mount.to_string(),
                    label,
                    drive_type,
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
