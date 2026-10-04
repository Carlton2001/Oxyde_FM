use super::{Platform, ThreadPriority};
use crate::models::{DriveInfo, QuickAccessItem};

pub struct WindowsPlatform;

impl WindowsPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Platform for WindowsPlatform {
    fn list_network_shares(&self, server: &str) -> Result<Vec<crate::models::FileEntry>, String> {
        super::network::list_network_entries(Some(server.to_string())).map_err(|error| error.to_string())
    }
    fn map_network_drive(&self, letter: String, path: String, reconnect: bool, username: Option<String>, password: Option<String>) -> Result<(), String> {
        super::network::map_network_drive(letter, path, reconnect, username, password)
    }
    fn disconnect_network_drive(&self, path: String, force: bool) -> Result<(), String> {
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
        crate::commands::clipboard::set_clipboard_files(paths, cut)
            .map_err(|error| format!("{error:?}"))
    }

    fn get_clipboard_files(&self) -> Result<(Vec<String>, bool), String> {
        crate::commands::clipboard::get_clipboard_files().map_err(|error| format!("{error:?}"))
    }

    fn regional_locale(&self) -> Option<String> {
        let mut buffer = [0u16; 85];
        let length = unsafe { windows::Win32::Globalization::GetUserDefaultLocaleName(&mut buffer) };
        (length > 0).then(|| String::from_utf16_lossy(&buffer[..length as usize - 1]))
    }
    fn name(&self) -> &'static str {
        "windows"
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

    fn get_accent_color(&self) -> String {
        #[cfg(target_os = "windows")]
        {
            use windows::core::PCWSTR;
            use windows::Win32::System::Registry::{
                RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, REG_DWORD,
                REG_VALUE_TYPE,
            };

            let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\DWM\0"
                .encode_utf16()
                .collect();
            let value_name: Vec<u16> = "AccentColor\0".encode_utf16().collect();
            let mut hkey: HKEY = HKEY::default();

            unsafe {
                if RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    PCWSTR(subkey.as_ptr()),
                    Some(0),
                    KEY_READ,
                    &mut hkey,
                )
                .is_ok()
                {
                    let mut color_type: REG_VALUE_TYPE = REG_VALUE_TYPE::default();
                    let mut color_value: u32 = 0;
                    let mut data_len: u32 = std::mem::size_of::<u32>() as u32;

                    if RegQueryValueExW(
                        hkey,
                        PCWSTR(value_name.as_ptr()),
                        None,
                        Some(&mut color_type),
                        Some(&mut color_value as *mut u32 as *mut u8),
                        Some(&mut data_len),
                    )
                    .is_ok()
                        && color_type == REG_DWORD
                    {
                        // AccentColor is in ABGR format (0xAABBGGRR)
                        let r = (color_value & 0xFF) as u8;
                        let g = ((color_value >> 8) & 0xFF) as u8;
                        let b = ((color_value >> 16) & 0xFF) as u8;
                        return format!("#{:02x}{:02x}{:02x}", r, g, b);
                    }
                }
            }
        }
        "#0078d7".to_string() // Default Windows Blue
    }

    fn resolve_shortcut(&self, path: &str) -> Option<String> {
        // La résolution .lnk est gérée par get_shortcut_info() dans io.rs,
        // appelée depuis resolve_shortcut_target(). On ne duplique pas ici.
        let _ = path;
        None
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
