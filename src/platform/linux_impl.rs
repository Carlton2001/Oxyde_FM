use super::{Platform, ThreadPriority};

pub struct LinuxPlatform;

impl LinuxPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Platform for LinuxPlatform {
    fn file_name_rules(&self, directory: &std::fs::File) -> std::io::Result<super::FileNameRules> {
        super::linux_file_names::rules(directory)
    }
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
    #[cfg(target_os = "linux")]
    fn source_link_target(&self, captured: &std::fs::File, metadata: &std::fs::Metadata) -> Option<std::io::Result<std::path::PathBuf>> {
        metadata.file_type().is_symlink().then(|| super::linux_links::target(captured))
    }
    #[cfg(target_os = "linux")]
    fn create_link(&self, parent: &std::fs::File, path: &std::path::Path, target: &std::path::Path, replace: bool) -> std::io::Result<std::fs::File> {
        super::linux_links::create(parent, path, target, replace)
    }
    #[cfg(target_os = "linux")]
    fn rename_link(&self, source_parent: &std::fs::File, source: &std::path::Path, parent: &std::fs::File, destination: &std::path::Path, replace: bool) -> std::io::Result<std::fs::File> {
        super::linux_links::rename(source_parent, source, parent, destination, replace)
    }
    #[cfg(target_os = "linux")]
    fn remove_link(&self, parent: &std::fs::File, path: &std::path::Path, verify: &mut dyn FnMut(&std::fs::File) -> std::io::Result<()>) -> std::io::Result<()> {
        super::linux_links::remove(parent, path, verify)
    }
    fn open_copied_file(&self, parent: &std::fs::File, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::OpenOptionsExt;
            if !std::fs::symlink_metadata(path)?.is_file() {
                return Err(std::io::Error::other("Operation source is not an ordinary file"));
            }
            // O_NOATIME | O_NOFOLLOW: verifying a copy must not replace the access date it kept.
            // Only the owner may ask for it; another owner's file is opened normally.
            match std::fs::OpenOptions::new().read(true).custom_flags(0x40000 | 0x20000).open(path) {
                Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {}
                result => return result,
            }
        }
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
            use std::os::fd::AsFd;
            let leaf = |path: &std::path::Path| -> std::io::Result<std::path::PathBuf> {
                path.file_name().map(std::path::PathBuf::from).ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "Rename path has no file name")
                })
            };
            // Linux does not pin source names against concurrent external replacement.
            super::linux_rename::exclusive(source_parent.as_fd(), &leaf(source)?, parent.as_fd(), &leaf(destination)?)
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
    fn remove_operation_tree(&self, _parent: &std::fs::File, path: &std::path::Path, _workers: usize, checkpoint: &(dyn Fn() -> std::io::Result<()> + Sync)) -> std::io::Result<()> {
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
        // FUSE learns of closes asynchronously and keeps names unlinked while open as
        // .fuse_hidden files until then.
        for _ in 0..40 {
            match std::fs::remove_dir(path) {
                Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty && only_fuse_hidden(path) => {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                result => return result,
            }
        }
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
        held_source: Option<&std::fs::File>,
        _destination_parent: &std::fs::File,
        created: &mut dyn FnMut(&std::fs::File) -> std::io::Result<()>,
        progress: &mut dyn FnMut(u64) -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        #[cfg(target_os = "linux")]
        { super::linux_file_copy::copy(source, destination, held_source, created, progress) }
        #[cfg(not(target_os = "linux"))]
        { let _ = (source, destination, held_source, created, progress); Err(std::io::ErrorKind::Unsupported.into()) }
    }
    fn restore_access_date(&self, file: &std::fs::File, accessed: std::time::SystemTime) -> std::io::Result<()> {
        file.set_times(std::fs::FileTimes::new().set_accessed(accessed))
    }
    #[cfg(target_os = "linux")]
    fn same_mount(&self, first: &std::fs::File, second: &std::fs::File) -> std::io::Result<bool> {
        super::linux_rename::same_mount(first, second)
    }
    #[cfg(target_os = "linux")]
    fn unix_permissions(&self, path: &std::path::Path) -> Option<crate::models::permissions::UnixPermissions> {
        super::linux_permissions::read(path)
            .inspect_err(|error| log::warn!("Permissions of {}: {error}", path.display()))
            .ok()
    }
    #[cfg(target_os = "linux")]
    fn umask(&self) -> Option<u32> {
        super::linux_permissions::umask()
    }
    #[cfg(target_os = "linux")]
    fn mime_type(&self, path: &std::path::Path, is_dir: bool, language: &str) -> Option<crate::models::MimeType> {
        Some(super::linux_mime::mime_type(path, is_dir, language))
    }
    #[cfg(target_os = "linux")]
    fn set_unix_permissions(
        &self,
        path: &std::path::Path,
        change: &crate::models::permissions::PermissionChange,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<(), crate::models::permissions::PermissionError> {
        super::linux_permissions::write(path, change, cancel)
    }
    #[cfg(target_os = "linux")]
    fn extended_attributes(&self, file: &std::fs::File) -> std::io::Result<super::ExtendedAttributes> {
        super::linux_extended_attributes::read(file)
    }
    #[cfg(target_os = "linux")]
    fn restore_directory_attributes(&self, directory: &std::fs::File, source: &super::DirectoryAttributes) -> std::io::Result<()> {
        super::linux_file_copy::restore_directory(directory, source)
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
        // The no-replace rename also publishes on FAT and exFAT, which have no hard links.
        // FUSE file systems such as ntfs-3g refuse the flag and keep the hard link.
        #[cfg(target_os = "linux")]
        match rustix::fs::renameat_with(rustix::fs::CWD, source, rustix::fs::CWD, destination, rustix::fs::RenameFlags::NOREPLACE) {
            Err(rustix::io::Errno::INVAL) => {}
            result => return result.map_err(std::io::Error::from),
        }
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
        items.iter().try_for_each(super::linux_trash::restore)
    }
    fn purge_trash_items(&self, items: Vec<trash::TrashItem>) -> Result<(), String> {
        super::linux_trash::purge(&items)
    }
    fn empty_trash(&self) -> Result<(), String> {
        self.purge_trash_items(self.list_trash_items()?)
    }
    fn trash_content_path(&self, item: &trash::TrashItem) -> Option<std::path::PathBuf> {
        super::linux_trash::content_path(item)
    }
    fn trash_retention(&self) -> Result<super::TrashRetention, String> {
        super::linux_trash::retention()
    }
    fn set_trash_retention(&self, retention: &super::TrashRetention) -> Result<(), String> {
        super::linux_trash::set_retention(retention)
    }
    fn bulk_expand_excluded(&self, _path: &str, name: &str) -> bool {
        // Preserve the previous shared exclusions; Windows drive rules stay native.
        matches!(
            name.to_ascii_lowercase().as_str(),
            "$recycle.bin" | "system volume information"
        )
    }

    fn trash_original_path(&self, _id: &std::path::Path) -> Option<std::path::PathBuf> { None }

    fn trash_transfer_entries(&self, paths: &[std::path::PathBuf]) -> Result<Vec<super::TrashTransferEntry>, String> {
        // Moving content alone would orphan its record; items leave the trash by restoration.
        if paths.iter().any(|path| super::linux_trash::inside_trash(path)) {
            return Err("Items in the trash must be restored before being copied or moved".into());
        }
        Ok(Vec::new())
    }
    fn detach_trash_metadata(&self, _entry: &super::TrashTransferEntry) -> Result<Option<super::DetachedTrashMetadata>, String> { Ok(None) }

    fn peek_status(&self) -> Result<super::PeekStatus, String> { Ok(super::PeekStatus::default()) }
    fn open_peek(&self, _path: &str) -> Result<(), String> { Err("Peek is only available on Windows".into()) }
    fn shortcut_info(&self, _path: &std::path::Path) -> Option<crate::models::ShortcutInfo> { None }
    fn set_shortcut_info(&self, _path: &str, _info: &crate::models::ShortcutInfo) -> Result<(), String> { Err("Shortcut editing is only available on Windows".into()) }
    fn quick_access(&self) -> &dyn super::QuickAccess {
        &super::linux_quick_access::LinuxQuickAccess
    }

    fn recycle_bin_usage(&self) -> Result<(u64, u64), String> {
        super::linux_trash::usage()
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
    fn volumes(&self) -> &dyn super::Volumes {
        &super::linux_volumes::LinuxVolumes
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
    fn file_icon_source(&self, path: &str, _is_dir: bool, _preserve_path: bool) -> String {
        path.to_string()
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

    fn file_clipboard(&self) -> &dyn super::FileClipboard {
        &super::linux_file_clipboard::LinuxFileClipboard
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
    /// xdg-open hands the item to the desktop and exits with its verdict; outside a
    /// desktop it may run the application itself, which is then reaped in the background.
    fn open_item(&self, path: &str) -> Result<(), String> {
        use std::process::Stdio;
        let mut child = std::process::Command::new("xdg-open")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("xdg-open: {error}"))?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            match child.try_wait().map_err(|error| error.to_string())? {
                Some(status) if status.success() => return Ok(()),
                Some(status) => {
                    // GNOME's gio reports a missing file and a missing application alike.
                    return Err(match status.code() {
                        Some(3) => "xdg-open misses a required tool".to_string(),
                        _ => format!("The desktop could not open {path}"),
                    })
                }
                None if std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                None => {
                    std::thread::spawn(move || child.wait());
                    return Ok(());
                }
            }
        }
    }

    /// Linux desktops share no properties dialog; Oxyde's own Properties window serves
    /// them, and the interface offers no system button. `gio open` used to open the item.
    fn show_properties_dialog(&self, _path: &str) {}

    fn install_pointer_capture(&self, _window: raw_window_handle::RawWindowHandle) -> Result<(), String> {
        // X11/Wayland already provide a button grab through GPUI.
        Ok(())
    }

    fn set_window_owner(&self, _window: raw_window_handle::RawWindowHandle, _owner: raw_window_handle::RawWindowHandle) -> Result<(), String> {
        // Owned windows are opened as GPUI floating windows, parented by Wayland/X11 at creation.
        Ok(())
    }

    fn set_application_icon(&self, _window: raw_window_handle::RawWindowHandle, _icon_bytes: &[u8]) {
        // TODO
    }

    fn configure_hover_window(&self, _window: raw_window_handle::RawWindowHandle, _owner: raw_window_handle::RawWindowHandle) -> Result<(), String> {
        Err("Native hover windows are unavailable on this platform".into())
    }

    fn place_hover_window(&self, _window: raw_window_handle::RawWindowHandle, _size: Option<(f32, f32, f32)>, _scale: f32) -> Result<(), String> {
        Err("Native hover windows are unavailable on this platform".into())
    }

    fn set_popup_anchor(&self, _window: raw_window_handle::RawWindowHandle, _bounds: Option<(f32, f32, f32, f32)>, _open_left: bool, _beside: bool, _above: bool) -> Result<(), String> {
        Err("Native popup anchors are unavailable on this platform".into())
    }

    fn configure_fixed_dialog(&self, _window: raw_window_handle::RawWindowHandle, _owner: raw_window_handle::RawWindowHandle) -> Result<(), String> {
        Err("Fixed dialog placement is unavailable on this platform".into())
    }

    fn follow_fixed_dialog_owner(&self, _owner: raw_window_handle::RawWindowHandle) -> Result<(), String> {
        Ok(())
    }

    fn configure_menu_window(&self, _window: raw_window_handle::RawWindowHandle, _owner: raw_window_handle::RawWindowHandle, _coordinate_owner: raw_window_handle::RawWindowHandle, _interactive: bool) -> Result<(), String> { Err("Native popup menus unavailable".into()) }
    fn popup_point_to_owner(&self, _window: raw_window_handle::RawWindowHandle, _owner: raw_window_handle::RawWindowHandle, point: (f32, f32)) -> Result<(f32, f32), String> { Ok(point) }
    fn popup_family_active(&self, _owner: raw_window_handle::RawWindowHandle) -> bool { false }
    fn popup_work_area_size(&self, _window: raw_window_handle::RawWindowHandle) -> Result<(f32, f32), String> { Err("Native popup work area unavailable".into()) }
}

fn only_fuse_hidden(directory: &std::path::Path) -> bool {
    std::fs::read_dir(directory).is_ok_and(|mut entries| {
        entries.all(|entry| entry.is_ok_and(|entry| entry.file_name().as_encoded_bytes().starts_with(b".fuse_hidden")))
    })
}
