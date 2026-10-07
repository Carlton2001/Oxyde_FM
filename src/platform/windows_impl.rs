use super::{Platform, ThreadPriority};
use crate::models::{DriveInfo, QuickAccessItem};

pub struct WindowsPlatform;

impl WindowsPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Platform for WindowsPlatform {
    fn trash_original_path(&self, id: &std::path::Path) -> Option<std::path::PathBuf> {
        super::windows_trash::original_path(id)
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
        use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
        let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).is_ok() };
        let result = super::icons::get_file_icon(
            path,
            if large { "large" } else { "small" }.into(),
        ).map_err(|error| error.to_string()).and_then(|bytes| {
            image::load_from_memory(&bytes)
                .map(|image| image.to_rgba8())
                .map_err(|error| error.to_string())
        });
        if initialized { unsafe { CoUninitialize(); } }
        result
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
        crate::commands::system::get_native_context_menu_items(path.to_string(), is_background)
            .map_err(|error| error.to_string())
    }

    fn execute_native_context_menu_item(
        &self,
        path: &str,
        id: i32,
        is_background: bool,
    ) -> Result<(), String> {
        crate::commands::system::execute_native_menu_item(path.to_string(), id, is_background)
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

    fn recycle_for_history(&self, path: &std::path::Path) -> Result<(), String> {
        trash::delete(path).map_err(|error| error.to_string())
    }

    fn history_trash_items(&self) -> Result<Vec<trash::TrashItem>, String> {
        trash::os_limited::list().map_err(|error| error.to_string())
    }

    fn restore_history_item(&self, item: trash::TrashItem) -> Result<(), String> {
        trash::os_limited::restore_all([item]).map_err(|error| error.to_string())
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

    fn regional_locale(&self) -> Option<String> {
        let mut buffer = [0u16; 85];
        let length = unsafe { windows::Win32::Globalization::GetUserDefaultLocaleName(&mut buffer) };
        (length > 0).then(|| String::from_utf16_lossy(&buffer[..length as usize - 1]))
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
            use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
            // Network-device Shell extensions require COM on this worker.
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }
                .map_err(|error| error.to_string())?;
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
            unsafe { CoUninitialize() };
            result?;
        } else {
            std::process::Command::new("explorer")
                .arg(path)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn get_quick_access_items(&self) -> Vec<QuickAccessItem> {
        crate::commands::system::get_quick_access_items().unwrap_or_default()
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
                let _ = std::process::Command::new("explorer.exe")
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
