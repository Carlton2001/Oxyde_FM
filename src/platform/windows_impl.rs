use super::windows_com::ComApartment;
use super::{Platform, ThreadPriority};
use crate::models::{DriveInfo, QuickAccessItem};

pub struct WindowsPlatform;

impl WindowsPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Platform for WindowsPlatform {
    fn open_archive_source(&self, path: &std::path::Path, allow_changes: bool) -> std::io::Result<std::fs::File> {
        super::windows_archive_source::open(path, allow_changes)
    }
    fn file_identity(&self, file: &std::fs::File) -> std::io::Result<super::FileIdentity> {
        super::windows_archive_source::identity(file)
    }
    fn system_executable(&self, executable: super::SystemExecutable) -> std::io::Result<std::path::PathBuf> {
        super::windows_executables::resolve(executable)
    }
    fn hold_archive_directory(&self, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        super::windows_archive_security::hold_directory(path)
    }
    fn hold_operation_directory(&self, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        super::windows_archive_security::hold_operation_directory(path)
    }
    fn hold_resolved_operation_directory(&self, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        super::windows_file_moves::hold_parent(path)
    }
    fn open_operation_file(&self, parent: &std::fs::File, path: &std::path::Path, read_contents: bool) -> std::io::Result<std::fs::File> {
        super::windows_file_moves::open_relative(parent, path, read_contents)
    }
    fn rename_operation_file(&self, file: &std::fs::File, _source: &std::path::Path, parent: &std::fs::File, destination: &std::path::Path, replace: bool) -> std::io::Result<()> {
        super::windows_file_moves::rename_relative(file, parent, destination, replace)
    }
    fn remove_operation_tree(&self, parent: &std::fs::File, path: &std::path::Path, checkpoint: &mut dyn FnMut() -> std::io::Result<()>) -> std::io::Result<()> {
        super::windows_file_tree::remove(parent, path, checkpoint)
    }
    fn create_copy_staging(&self, parent: &std::fs::File, path: &std::path::Path) -> std::io::Result<(std::fs::File, std::fs::File)> {
        super::windows_copy_staging::create(parent, path)
    }
    fn remove_copy_staging(&self, directory: &std::fs::File, pin: std::fs::File, _path: &std::path::Path) -> std::io::Result<()> {
        super::windows_copy_staging::remove(directory, pin)
    }
    fn lock_cache_file(&self, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        super::windows_cache::lock_file(path)
    }
    fn read_cache_file(&self, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        super::windows_cache::read_file(path)
    }
    fn open_cache_cleanup(&self, path: &std::path::Path, directory: bool) -> std::io::Result<std::fs::File> {
        super::windows_cache::open_cleanup(path, directory)
    }
    fn ensure_archive_directory(&self, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        super::windows_archive_security::ensure_directory(path)
    }
    fn create_archive_file(&self, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        super::windows_archive_security::create_file(path)
    }
    fn read_file_provenance(&self, path: &std::path::Path) -> std::io::Result<Option<Vec<u8>>> {
        super::windows_archive_security::read_provenance(path)
    }
    fn write_file_provenance(&self, path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
        super::windows_archive_security::write_provenance(path, data)
    }
    fn copy_file_with_progress(
        &self,
        source: &std::path::Path,
        destination: &std::path::Path,
        held_source: Option<&std::fs::File>,
        destination_parent: &std::fs::File,
        progress: &mut dyn FnMut(u64) -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        super::windows_file_copy::copy(source, destination, held_source, destination_parent, progress)
    }
    fn open_file_for_move(&self, path: &std::path::Path) -> std::io::Result<std::fs::File> {
        super::windows_file_moves::open(path)
    }
    fn rename_open_file_exclusive(
        &self,
        file: &std::fs::File,
        _source: &std::path::Path,
        destination: &std::path::Path,
    ) -> std::io::Result<()> {
        super::windows_file_moves::rename(file, destination)
    }
    fn remove_open_file(
        &self,
        file: &std::fs::File,
        _path: &std::path::Path,
    ) -> std::io::Result<()> {
        super::windows_file_moves::remove(file)
    }
    fn replace_open_file(
        &self,
        file: &std::fs::File,
        _source: &std::path::Path,
        destination: &std::path::Path,
    ) -> std::io::Result<()> {
        super::windows_file_moves::replace(file, destination)
    }
    fn list_trash_items(&self) -> Result<Vec<trash::TrashItem>, String> {
        trash::os_limited::list().map_err(|error| error.to_string())
    }
    fn restore_trash_items(&self, items: Vec<trash::TrashItem>) -> Result<(), String> {
        trash::os_limited::restore_all(items).map_err(|error| error.to_string())
    }
    fn purge_trash_items(&self, items: Vec<trash::TrashItem>) -> Result<(), String> {
        let paths: Vec<_> = items
            .into_iter()
            .map(|item| std::path::PathBuf::from(item.id))
            .collect();
        self.shell_delete(&paths, false)
    }
    fn empty_trash(&self) -> Result<(), String> {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::Shell::{
            SHEmptyRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND,
        };
        unsafe {
            SHEmptyRecycleBinW(
                Some(HWND::default()),
                PCWSTR::default(),
                SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
            )
            .map_err(|error| format!("Failed to empty recycle bin: {error}"))
        }
    }
    fn bulk_expand_excluded(&self, path: &str, name: &str) -> bool {
        let system_root = std::env::var_os("SystemRoot").map(std::path::PathBuf::from);
        let drive = system_root.and_then(|root| root.parent().map(|parent| parent.to_path_buf()));
        bulk_expand_excluded(path, name, drive.as_deref())
    }

    fn trash_original_path(&self, id: &std::path::Path) -> Option<std::path::PathBuf> {
        super::windows_trash::original_path(id)
    }

    fn trash_transfer_entries(&self, paths: &[std::path::PathBuf]) -> Result<Vec<super::TrashTransferEntry>, String> {
        super::windows_trash::transfer_entries(paths)
    }
    fn detach_trash_metadata(&self, entry: &super::TrashTransferEntry) -> Result<Option<(std::path::PathBuf, std::path::PathBuf)>, String> {
        super::windows_trash::detach_metadata(entry)
    }

    fn peek_status(&self) -> Result<super::PeekStatus, String> { super::windows_peek::peek_status() }
    fn open_peek(&self, path: &str) -> Result<(), String> { super::windows_peek::open_peek(path) }
    fn shortcut_info(&self, path: &std::path::Path) -> Option<crate::models::ShortcutInfo> { super::windows_shortcuts::get_shortcut_info(path) }
    fn set_shortcut_info(&self, path: &str, info: &crate::models::ShortcutInfo) -> Result<(), String> { super::windows_shortcuts::set_shortcut_info(path, info) }
    fn watch_quick_access_changes(&self, revision: std::sync::Arc<std::sync::atomic::AtomicU64>) -> Result<Option<Box<dyn Send>>, String> {
        let root = std::env::var_os("APPDATA").ok_or("APPDATA is unavailable")?;
        let recent = std::path::PathBuf::from(root).join("Microsoft/Windows/Recent");
        crate::systems::quick_access_watcher::watch(&recent, revision).map(|watcher| Some(Box::new(watcher) as Box<dyn Send>))
    }

    fn recycle_bin_usage(&self) -> Result<(u64, u64), String> {
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::GetLogicalDriveStringsW;
        use windows::Win32::UI::Shell::{SHQueryRecycleBinW, SHQUERYRBINFO};

        let mut total_size = 0u64;
        let mut total_items = 0u64;

        unsafe {
            let mut buffer = [0u16; 260];
            let len = GetLogicalDriveStringsW(Some(&mut buffer));
            if len > 0 {
                let drive_strings = String::from_utf16_lossy(&buffer[..len as usize]);
                for drive_path in drive_strings.split('\0').filter(|s| !s.is_empty()) {
                    let wide_path: Vec<u16> = drive_path
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect();

                    let mut info = SHQUERYRBINFO {
                        cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
                        i64Size: 0,
                        i64NumItems: 0,
                    };

                    if SHQueryRecycleBinW(PCWSTR(wide_path.as_ptr()), &mut info).is_ok() {
                        total_size += info.i64Size as u64;
                        total_items += info.i64NumItems as u64;
                    }
                }
            }
        }

        Ok((total_size, total_items))

    }

    fn recycle_bin_config(&self, path: String) -> Result<crate::commands::ops::TrashSystemConfig, String> {
        read_recycle_bin_config(path).map_err(|error| error.to_string())
    }
    fn set_recycle_bin_config(&self, config: crate::commands::ops::TrashSystemConfig) -> Result<(), String> {
        write_recycle_bin_config(config).map_err(|error| error.to_string())
    }

    fn finish_process_shutdown(&self) -> Result<(), String> {
        // A captured shutdown dump showed the final thread spinning in Intel's
        // igd10um64xe through d3d11 / Windows.Data.Pdf during process detach,
        // after main, GPUI and Tokio had finished. ExitProcess (including
        // std::process::exit) still invokes those DLL callbacks.
        // All application cleanup must have completed before reaching this API.
        unsafe {
            windows::Win32::System::Threading::TerminateProcess(
                windows::Win32::System::Threading::GetCurrentProcess(), 0,
            ).map_err(|error| error.to_string())
        }
    }

    fn install_disk_image_autoplay_handler(&self, window: raw_window_handle::RawWindowHandle) -> Result<(), String> {
        let raw_window_handle::RawWindowHandle::Win32(handle) = window else { return Ok(()); };
        super::windows_disk_images::install_autoplay_handler(windows::Win32::Foundation::HWND(handle.hwnd.get() as *mut std::ffi::c_void))
    }
    fn mounted_disk_images(&self) -> Result<Vec<super::MountedDiskImage>, String> {
        super::windows_disk_images::mounted_images()
    }
    fn mount_disk_image(&self, path: &str) -> Result<Vec<String>, String> {
        super::windows_disk_images::mount(path)
    }
    fn unmount_disk_image(&self, path: &str) -> Result<(), String> {
        super::windows_disk_images::unmount(path)
    }
    fn list_network_shares(&self, server: &str) -> Result<Vec<crate::models::FileEntry>, String> {
        super::network::list_network_entries(Some(server.to_string())).map_err(|error| error.to_string())
    }
    fn map_network_drive(&self, letter: String, path: String, reconnect: bool, username: Option<String>, password: Option<String>) -> Result<(), String> {
        super::network::map_network_drive(letter, path, reconnect, username, password)
    }
    fn disconnect_network_drive(&self, path: String, force: bool) -> Result<(), super::NetworkDisconnectError> {
        super::network::disconnect_network_drive(path, force)
    }
    fn list_drive_roots(&self) -> Vec<DriveInfo> {
        super::windows_drives::list_drive_roots()
    }

    fn watch_drive_changes(&self, revision: std::sync::Arc<std::sync::atomic::AtomicU64>)
        -> Result<Option<Box<dyn Send>>, String> {
        super::windows_drives::watch_drive_changes(revision).map(Some)
    }

    fn eject_drive(&self, path: &str) -> Result<(), String> {
        super::windows_drives::eject_drive(path)
    }

    fn file_icon(&self, path: String, large: bool) -> Result<image::RgbaImage, String> {
        let _com = ComApartment::mta_or_existing().map_err(|error| error.to_string())?;
        super::icons::get_file_icon(path, if large { "large" } else { "small" }.into())
            .map_err(|error| error.to_string())
            .and_then(|bytes| {
                image::load_from_memory(&bytes)
                    .map(|image| image.to_rgba8())
                    .map_err(|error| error.to_string())
            })
    }

    fn set_hidden_attribute(&self, path: &std::path::Path, hidden: bool) -> Result<(), String> {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::{
            GetFileAttributesW, SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN,
            FILE_FLAGS_AND_ATTRIBUTES,
        };

        let wide_path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        unsafe {
            let attributes = GetFileAttributesW(PCWSTR(wide_path.as_ptr()));
            if attributes == u32::MAX {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let attributes = if hidden {
                attributes | FILE_ATTRIBUTE_HIDDEN.0
            } else {
                attributes & !FILE_ATTRIBUTE_HIDDEN.0
            };
            SetFileAttributesW(
                PCWSTR(wide_path.as_ptr()),
                FILE_FLAGS_AND_ATTRIBUTES(attributes),
            )
            .map_err(|error| error.to_string())
        }
    }

    fn native_context_menu_items(
        &self,
        path: &str,
        is_background: bool,
    ) -> Result<Vec<crate::models::WinMenuItem>, String> {
        super::windows_shell::get_native_context_menu_items(path.to_string(), is_background)
            .map_err(|error| error.to_string())
    }

    fn execute_native_context_menu_item(
        &self,
        path: &str,
        id: i32,
        is_background: bool,
    ) -> Result<(), String> {
        super::windows_shell::execute_native_menu_item(path.to_string(), id, is_background)
            .map_err(|error| error.to_string())
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
        super::network::list_network_vicinity().map_err(|error| error.to_string())
    }

    fn shell_delete(&self, paths: &[std::path::PathBuf], recycle: bool) -> Result<(), String> {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::Shell::{
            SHFileOperationW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_NO_UI,
            FOF_SILENT, FO_DELETE, SHFILEOPSTRUCTW,
        };
        if paths.is_empty() {
            return Ok(());
        }
        let mut buffer = Vec::new();
        for path in paths {
            buffer.extend(path.to_string_lossy().replace('/', "\\").encode_utf16());
            buffer.push(0);
        }
        buffer.push(0);
        // Explicitly honor the per-volume setting even when the caller requests undo.
        let immediate = |path: &std::path::PathBuf| path.ancestors().last()
            .and_then(|root| read_recycle_bin_config(root.to_string_lossy().into_owned()).ok())
            .is_some_and(|config| config.nuke_on_delete);
        if recycle && paths.len() > 1 {
            let (permanent, recyclable): (Vec<_>, Vec<_>) = paths.iter().cloned().partition(immediate);
            if !permanent.is_empty() {
                self.shell_delete(&permanent, false)?;
                if !recyclable.is_empty() { self.shell_delete(&recyclable, true)?; }
                return Ok(());
            }
        }
        let recycle = recycle && !paths.first().is_some_and(immediate);
        let mut flags = FOF_NOCONFIRMATION.0 | FOF_NOERRORUI.0 | FOF_SILENT.0;
        if recycle {
            flags |= FOF_ALLOWUNDO.0;
        } else {
            flags |= FOF_NO_UI.0;
        }
        let mut operation = SHFILEOPSTRUCTW {
            hwnd: HWND(std::ptr::null_mut()),
            wFunc: FO_DELETE,
            pFrom: PCWSTR(buffer.as_ptr()),
            pTo: PCWSTR::null(),
            fFlags: flags as u16,
            fAnyOperationsAborted: Default::default(),
            hNameMappings: std::ptr::null_mut(),
            lpszProgressTitle: PCWSTR::null(),
        };
        unsafe {
            let code = SHFileOperationW(&mut operation);
            if code != 0 {
                if recycle && (code == 0x4C7 || code == 0x80070005u32 as i32) {
                    return Err(
                        "TrashFull: Operation cancelled or requires administrator rights"
                            .to_string(),
                    );
                }
                return Err(format!(
                    "Windows Shell error (0x{code:X}) while deleting files"
                ));
            }
            if operation.fAnyOperationsAborted.as_bool() {
                return Err("Deletion was aborted".to_string());
            }
        }
        Ok(())
    }

    fn set_thread_priority(&self, priority: ThreadPriority) {
        use windows::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN,
            THREAD_MODE_BACKGROUND_END, THREAD_PRIORITY_HIGHEST,
        };
        let native_priority = match priority {
            ThreadPriority::Background => THREAD_MODE_BACKGROUND_BEGIN,
            ThreadPriority::Normal => THREAD_MODE_BACKGROUND_END,
            ThreadPriority::High => THREAD_PRIORITY_HIGHEST,
        };
        unsafe {
            let _ = SetThreadPriority(GetCurrentThread(), native_priority);
        }
    }

    fn file_attributes(&self, metadata: &std::fs::Metadata, name: &str) -> (bool, bool, bool) {
        use std::os::windows::fs::MetadataExt;
        let attrs = metadata.file_attributes();
        (
            (attrs & 0x2) != 0 || name.starts_with('.'),
            (attrs & 0x4) != 0,
            (attrs & 0x400) != 0,
        )
    }

    fn set_clipboard_files(&self, paths: Vec<String>, cut: bool) -> Result<(), String> {
        super::clipboard::set_clipboard_files(paths, cut)
            .map_err(|error| format!("{error:?}"))
    }

    fn get_clipboard_files(&self) -> Result<(Vec<String>, bool), String> {
        super::clipboard::get_clipboard_files().map_err(|error| format!("{error:?}"))
    }

    fn clipboard_has_file_format(&self) -> bool {
        super::clipboard::has_file_format()
    }

    fn regional_locale(&self) -> Option<String> {
        let mut buffer = [0u16; 85];
        let length = unsafe { windows::Win32::Globalization::GetUserDefaultLocaleName(&mut buffer) };
        (length > 0).then(|| String::from_utf16_lossy(&buffer[..length as usize - 1]))
    }
    fn calendar_timezone_signature(&self) -> Option<String> {
        use windows::Win32::System::Time::{
            GetDynamicTimeZoneInformation, DYNAMIC_TIME_ZONE_INFORMATION, TIME_ZONE_ID_INVALID,
        };
        let mut info = DYNAMIC_TIME_ZONE_INFORMATION::default();
        if unsafe { GetDynamicTimeZoneInformation(&mut info) } == TIME_ZONE_ID_INVALID {
            return None;
        }
        let key = String::from_utf16_lossy(&info.TimeZoneKeyName);
        Some(format!(
            "{}:{}:{}:{}:{:?}:{:?}:{}",
            key.trim_end_matches('\0'),
            info.Bias,
            info.StandardBias,
            info.DaylightBias,
            info.StandardDate,
            info.DaylightDate,
            info.DynamicDaylightTimeDisabled
        ))
    }

    fn interface_locale(&self) -> Option<String> {
        use windows::Win32::Globalization::{GetUserDefaultUILanguage, LCIDToLocaleName};
        let mut buffer = [0u16; 85];
        let length = unsafe {
            LCIDToLocaleName(u32::from(GetUserDefaultUILanguage()), Some(&mut buffer), 0)
        };
        (length > 0).then(|| String::from_utf16_lossy(&buffer[..length as usize - 1]))
    }
    fn list_drives(&self, skip_hardware_info: bool) -> Vec<DriveInfo> {
        super::windows_drives::list_drives(skip_hardware_info)
    }

    fn open_item(&self, path: &str) -> Result<(), String> {
        use windows::core::{HSTRING, PCWSTR};
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        if path.starts_with("::{") || path.starts_with('?') {
            // Network-device Shell extensions require COM on this worker.
            let _com = ComApartment::sta().map_err(|error| error.to_string())?;
            let result = unsafe {
                let wide = HSTRING::from(path);
                let res = ShellExecuteW(
                    None,
                    PCWSTR::null(),
                    PCWSTR(wide.as_ptr()),
                    PCWSTR::null(),
                    PCWSTR::null(),
                    SW_SHOWNORMAL,
                );
                if res.0 as usize <= 32 {
                    Err(format!(
                        "ShellExecuteW failed avec le code {}",
                        res.0 as usize
                    ))
                } else { Ok(()) }
            };
            result?;
        } else {
            std::process::Command::new(self.system_executable(super::SystemExecutable::Explorer).map_err(|error| error.to_string())?)
                .arg(path)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn get_quick_access_items(&self) -> Vec<QuickAccessItem> {
        super::windows_shell::get_quick_access_items().unwrap_or_default()
    }

    fn pin_quick_access(&self, path: &str) -> Result<(), crate::models::CommandError> {
        super::windows_shell::execute_shell_verb_by_canonical_name(
            path,
            &["pintohome", "pintofavorites"],
        )
    }

    fn unpin_quick_access(&self, path: &str) -> Result<(), crate::models::CommandError> {
        super::windows_shell::execute_shell_verb_by_canonical_name(
            path,
            &["unpinfromhome", "unpinfromquickaccess"],
        )
    }

    fn shell_item_friendly_name(&self, path: &str) -> Result<Option<String>, String> {
        super::windows_shell_properties::friendly_name(path)
    }

    fn show_properties_dialog(&self, path: &str) {
        use windows::core::{HSTRING, PCWSTR};
        use windows::Win32::UI::Shell::{
            ShellExecuteExW, SEE_MASK_INVOKEIDLIST, SHELLEXECUTEINFOW,
        };
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        // Map virtual paths to their real Windows Shell equivalents.
        // trash://              → Recycle Bin CLSID
        // __network_vincinity__ → open "Réseau et Internet" in Windows Settings
        if path == "__network_vincinity__" {
            std::thread::spawn(|| {
                let Ok(explorer) = super::platform().system_executable(super::SystemExecutable::Explorer) else { return; };
                let _ = std::process::Command::new(explorer)
                    .arg("ms-settings:network")
                    .spawn();
            });
            return;
        }

        let normalized = if path == "trash://" {
            "::{645FF040-5081-101B-9F08-00AA002F954E}".to_string()
        } else {
            let mut s = path.replace('/', "\\");
            if s.len() == 2 && s.ends_with(':') {
                s.push('\\');
            }
            s
        };

        std::thread::spawn(move || {
            let wide_path = HSTRING::from(&normalized);
            let verb = HSTRING::from("properties");

            let mut info = SHELLEXECUTEINFOW {
                cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
                fMask: SEE_MASK_INVOKEIDLIST,
                lpVerb: PCWSTR(verb.as_ptr()),
                lpFile: PCWSTR(wide_path.as_ptr()),
                nShow: SW_SHOWNORMAL.0,
                ..Default::default()
            };

            unsafe {
                let _ = ShellExecuteExW(&mut info);
            }
        });
    }

    fn set_application_icon(&self, window: raw_window_handle::RawWindowHandle, icon_bytes: &[u8]) {
        use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateIconFromResourceEx, SendMessageW, ICON_BIG, ICON_SMALL,
            LR_DEFAULTSIZE, WM_SETICON,
        };

        unsafe {
            let raw_window_handle::RawWindowHandle::Win32(handle) = window else { return; };
            let hwnd = HWND(handle.hwnd.get() as *mut std::ffi::c_void);
            let hicon = CreateIconFromResourceEx(
                icon_bytes,
                true,       // fIcon: true for icon, false for cursor
                0x00030000, // dwVersion: standard value for icons
                0,
                0, // width/height: use default
                LR_DEFAULTSIZE,
            );
            if let Ok(icon) = hicon {
                if !hwnd.is_invalid() {
                    let _ = SendMessageW(
                        hwnd,
                        WM_SETICON,
                        Some(WPARAM(ICON_BIG as usize)),
                        Some(LPARAM(icon.0 as isize)),
                    );
                    let _ = SendMessageW(
                        hwnd,
                        WM_SETICON,
                        Some(WPARAM(ICON_SMALL as usize)),
                        Some(LPARAM(icon.0 as isize)),
                    );
                }
            }
        }
    }
}

mod recycle_settings {
    use crate::commands::ops::TrashSystemConfig;
    use crate::models::CommandError;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetVolumeNameForVolumeMountPointW;
    use windows::Win32::System::Registry::{RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_DWORD};
    use windows::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_FLUSH};
pub(super) fn read_recycle_bin_config(drive_path: String) -> Result<TrashSystemConfig, CommandError> {
    let wide_drive: Vec<u16> = drive_path
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut max_size_mb = 0u64;
    let mut nuke_on_delete = false;

    unsafe {
        let mut volume_name = [0u16; 50];
        if GetVolumeNameForVolumeMountPointW(PCWSTR(wide_drive.as_ptr()), &mut volume_name).is_ok()
        {
            let vol_string = String::from_utf16_lossy(&volume_name);
            let vol_trimmed = vol_string.trim_matches(char::from(0));
            if let Some(start) = vol_trimmed.find("Volume{") {
                let guid_part = &vol_trimmed[start + 6..];
                if let Some(end) = guid_part.find('}') {
                    let guid_str = &guid_part[..=end];
                    let key_path = format!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\BitBucket\\Volume\\{}", guid_str);
                    let wide_key: Vec<u16> =
                        key_path.encode_utf16().chain(std::iter::once(0)).collect();

                    let mut hkey = windows::Win32::System::Registry::HKEY::default();
                    if RegOpenKeyExW(
                        HKEY_CURRENT_USER,
                        PCWSTR(wide_key.as_ptr()),
                        Some(0),
                        KEY_READ,
                        &mut hkey,
                    )
                    .0 == 0
                    {
                        // Read MaxCapacity
                        let mut data = 0u32;
                        let mut data_size = std::mem::size_of::<u32>() as u32;
                        let mut key_type =
                            windows::Win32::System::Registry::REG_VALUE_TYPE::default();
                        let wide_cap: Vec<u16> = "MaxCapacity"
                            .encode_utf16()
                            .chain(std::iter::once(0))
                            .collect();
                        if RegQueryValueExW(
                            hkey,
                            PCWSTR(wide_cap.as_ptr()),
                            None,
                            Some(&mut key_type as *mut _),
                            Some(&mut data as *mut _ as *mut u8),
                            Some(&mut data_size as *mut _),
                        )
                        .0 == 0
                        {
                            max_size_mb = data as u64;
                        }

                        // Read NukeOnDelete
                        let wide_nuke: Vec<u16> = "NukeOnDelete"
                            .encode_utf16()
                            .chain(std::iter::once(0))
                            .collect();
                        if RegQueryValueExW(
                            hkey,
                            PCWSTR(wide_nuke.as_ptr()),
                            None,
                            Some(&mut key_type as *mut _),
                            Some(&mut data as *mut _ as *mut u8),
                            Some(&mut data_size as *mut _),
                        )
                        .0 == 0
                        {
                            nuke_on_delete = data == 1;
                        }
                        let _ = windows::Win32::System::Registry::RegCloseKey(hkey);
                    }
                }
            }
        }
    }

    Ok(TrashSystemConfig {
        drive_path,
        max_size_mb,
        nuke_on_delete,
    })
}

pub(super) fn write_recycle_bin_config(config: TrashSystemConfig) -> Result<(), CommandError> {
    use windows::Win32::System::Registry::{RegCreateKeyExW, RegCloseKey, REG_OPTION_NON_VOLATILE};
    let fail = |error: windows::core::Error| CommandError::SystemError(error.to_string());
    let capacity = u32::try_from(config.max_size_mb).map_err(|_| CommandError::SystemError("Capacity exceeds Windows limit".into()))?;
    let drive: Vec<u16> = config.drive_path.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let mut volume = [0u16; 50];
        GetVolumeNameForVolumeMountPointW(PCWSTR(drive.as_ptr()), &mut volume).map_err(fail)?;
        let volume = String::from_utf16_lossy(&volume);
        let start = volume.find("Volume{").ok_or_else(|| CommandError::SystemError("Invalid volume identifier".into()))? + 6;
        let end = volume[start..].find('}').ok_or_else(|| CommandError::SystemError("Invalid volume identifier".into()))? + start;
        let key = format!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\BitBucket\Volume\{}", &volume[start..=end]);
        let key: Vec<u16> = key.encode_utf16().chain(Some(0)).collect();
        let mut handle = windows::Win32::System::Registry::HKEY::default();
        RegCreateKeyExW(HKEY_CURRENT_USER, PCWSTR(key.as_ptr()), Some(0), None, REG_OPTION_NON_VOLATILE, KEY_WRITE, None, &mut handle, None).ok().map_err(fail)?;
        let result: Result<(), CommandError> = (|| {
            for (name, value) in [("MaxCapacity", capacity), ("NukeOnDelete", u32::from(config.nuke_on_delete))] {
                let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
                RegSetValueExW(handle, PCWSTR(name.as_ptr()), Some(0), REG_DWORD, Some(&value.to_le_bytes())).ok().map_err(fail)?;
            }
            Ok(())
        })();
        let _ = RegCloseKey(handle);
        result?;
        SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_FLUSH, None, None);
    }
    Ok(())
}

}
use recycle_settings::{read_recycle_bin_config, write_recycle_bin_config};

#[cfg(test)]
mod deletion_history_tests {
    use super::*;
    use crate::models::{HistoryManager, Transaction, TransactionDetails, TransactionType};
    use std::path::PathBuf;

    struct Fixture {
        root: tempfile::TempDir,
        paths: Vec<PathBuf>,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            let drive = root.path().ancestors().last().unwrap();
            let config = read_recycle_bin_config(drive.to_string_lossy().into_owned()).unwrap();
            assert!(!config.nuke_on_delete, "test volume must recycle files");
            Self {
                root,
                paths: Vec::new(),
            }
        }

        fn verify_cycle(&self, verify: impl Fn()) {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let history = HistoryManager::new();
            WindowsPlatform::new()
                .shell_delete(&self.paths, true)
                .unwrap();
            assert!(self.paths.iter().all(|path| !path.exists()));
            history.push(Transaction::new(
                TransactionType::Delete,
                TransactionDetails {
                    paths: self
                        .paths
                        .iter()
                        .map(|path| path.to_string_lossy().into_owned())
                        .collect(),
                    target_dir: None,
                    old_path: None,
                    new_path: None,
                    created_files: None,
                    created_dirs: None,
                    overwritten_files: None,
                    moved_files: None,
                    trash_metadata_backups: None,
                    moved_dirs: None,
                    copied_files: None,
                    copied_file_hashes: None,
                },
            ));
            runtime.block_on(async {
                crate::commands::ops::undo_last_action(&history)
                    .await
                    .unwrap();
                verify();
                assert_eq!(history.get_state().redo_stack.len(), 1);
                crate::commands::ops::redo_last_action(&history)
                    .await
                    .unwrap();
                assert!(self.paths.iter().all(|path| !path.exists()));
                assert_eq!(history.get_state().undo_stack.len(), 1);
                crate::commands::ops::undo_last_action(&history)
                    .await
                    .unwrap();
                verify();
            });
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let Ok(entries) = crate::commands::ops::list_trash() else {
                return;
            };
            let ids: Vec<_> = entries
                .into_iter()
                .filter(|entry| self.paths.contains(&PathBuf::from(&entry.original_path)))
                .map(|entry| entry.path)
                .collect();
            if !ids.is_empty() {
                let result = std::thread::spawn(move || {
                    tokio::runtime::Runtime::new()
                        .unwrap()
                        .block_on(crate::commands::ops::restore_items(ids))
                })
                .join();
                assert!(
                    matches!(result, Ok(Ok(_))),
                    "generated fixture cleanup failed: {result:?}"
                );
            }
        }
    }

    #[test]
    #[ignore = "recycles/restores a generated sparse file larger than the former 4 GiB limit"]
    fn native_delete_redo_above_old_size_limit() {
        use std::io::{Read, Seek, SeekFrom, Write};
        use std::os::windows::io::AsRawHandle;
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::Ioctl::FSCTL_SET_SPARSE;
        use windows::Win32::System::IO::DeviceIoControl;
        use windows::Win32::UI::Shell::{SHQueryRecycleBinW, SHQUERYRBINFO};

        let mut fixture = Fixture::new();
        let size = 4 * 1024 * 1024 * 1024 + 1_u64;
        let drive = fixture.root.path().ancestors().last().unwrap();
        let config = read_recycle_bin_config(drive.to_string_lossy().into_owned()).unwrap();
        let wide: Vec<u16> = drive
            .to_string_lossy()
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut usage = SHQUERYRBINFO {
            cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
            ..Default::default()
        };
        unsafe {
            SHQueryRecycleBinW(PCWSTR(wide.as_ptr()), &mut usage).unwrap();
        }
        // Leave ample headroom so this generated fixture cannot evict existing entries.
        assert!(
            config.max_size_mb * 1024 * 1024 > usage.i64Size.max(0) as u64 + 2 * size,
            "test volume must have more than 8 GiB of Recycle Bin headroom"
        );
        let path = fixture
            .root
            .path()
            .join("Fichier école supérieur à 4 Gio.bin");
        fixture.paths.push(path.clone());
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        unsafe {
            let mut returned = 0;
            DeviceIoControl(
                HANDLE(file.as_raw_handle()),
                FSCTL_SET_SPARSE,
                None,
                0,
                None,
                0,
                Some(&mut returned),
                None,
            )
            .unwrap();
        }
        file.set_len(size).unwrap();
        file.write_all(b"start").unwrap();
        file.seek(SeekFrom::End(-3)).unwrap();
        file.write_all(b"end").unwrap();
        drop(file);
        fixture.verify_cycle(|| {
            let mut file = std::fs::File::open(&path).unwrap();
            assert_eq!(file.metadata().unwrap().len(), size);
            let mut start = [0; 5];
            file.read_exact(&mut start).unwrap();
            assert_eq!(&start, b"start");
            file.seek(SeekFrom::End(-3)).unwrap();
            let mut end = [0; 3];
            file.read_exact(&mut end).unwrap();
            assert_eq!(&end, b"end");
        });
    }

    #[test]
    #[ignore = "recycles/restores only 1001 generated files to exercise the former item limit"]
    fn native_delete_redo_above_old_item_limit() {
        let mut fixture = Fixture::new();
        for index in 0..1001 {
            let path = fixture
                .root
                .path()
                .join(format!("Fichier école {index}.txt"));
            std::fs::write(&path, index.to_string()).unwrap();
            fixture.paths.push(path);
        }
        fixture.verify_cycle(|| {
            for (index, path) in fixture.paths.iter().enumerate() {
                assert_eq!(std::fs::read_to_string(path).unwrap(), index.to_string());
            }
        });
    }
}

fn bulk_expand_excluded(
    path: &str,
    name: &str,
    system_drive_root: Option<&std::path::Path>,
) -> bool {
    let name = name.to_ascii_lowercase();
    if matches!(name.as_str(), "$recycle.bin" | "system volume information") {
        return true;
    }

    // Avoid recursively opening OS-managed branches on the Windows system drive.
    let Some(system_drive_root) = system_drive_root else {
        return false;
    };
    let Some(parent) = std::path::Path::new(path).parent() else {
        return false;
    };
    if !parent
        .as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&system_drive_root.as_os_str().to_string_lossy())
    {
        return false;
    }
    matches!(
        name.as_str(),
        "windows" | "program files" | "program files (x86)" | "programdata"
    )
}

#[cfg(test)]
mod bulk_expand_tests {
    use super::bulk_expand_excluded;
    use std::path::Path;
    #[test]
    fn protected_branches_are_limited_to_the_system_drive() {
        let drive = Some(Path::new(r"C:\"));
        assert!(bulk_expand_excluded(
            r"D:\$Recycle.bin",
            "$RECYCLE.BIN",
            drive
        ));
        assert!(bulk_expand_excluded(
            r"D:\System Volume Information",
            "System Volume Information",
            None
        ));
        for name in [
            "Windows",
            "Program Files",
            "Program Files (x86)",
            "ProgramData",
        ] {
            assert!(bulk_expand_excluded(&format!(r"c:\{name}"), name, drive));
            assert!(!bulk_expand_excluded(&format!(r"D:\{name}"), name, drive));
            assert!(!bulk_expand_excluded(
                &format!(r"C:\Users\{name}"),
                name,
                drive
            ));
            assert!(!bulk_expand_excluded(&format!(r"C:\{name}"), name, None));
        }
        assert!(!bulk_expand_excluded(
            r"C:\Windows-old",
            "Windows-old",
            drive
        ));
    }
}
