use crate::models::{DriveInfo, QuickAccessItem};
use super::Platform;

pub struct WindowsPlatform;

impl WindowsPlatform {
    pub fn new() -> Self { Self }
}

impl Platform for WindowsPlatform {
    fn name(&self) -> &'static str { "windows" }

    fn list_drives(&self, skip_hardware_info: bool) -> Vec<DriveInfo> {
        crate::commands::system::get_drives(Some(skip_hardware_info))
    }

    fn open_item(&self, path: &str) -> Result<(), String> {
        use windows::core::{HSTRING, PCWSTR};
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        if path.starts_with("::{") || path.starts_with('?') {
            unsafe {
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
                    return Err(format!("ShellExecuteW failed avec le code {}", res.0 as usize));
                }
            }
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
        crate::commands::system::get_accent_color()
    }

    fn resolve_shortcut(&self, path: &str) -> Option<String> {
        // La résolution .lnk est gérée par get_shortcut_info() dans io.rs,
        // appelée depuis resolve_shortcut_target(). On ne duplique pas ici.
        let _ = path;
        None
    }

    fn show_properties_dialog(&self, path: &str) {
        use windows::core::{HSTRING, PCWSTR};
        use windows::Win32::UI::Shell::{ShellExecuteExW, SHELLEXECUTEINFOW, SEE_MASK_INVOKEIDLIST};
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        // Normalize path: replace / with \ and ensure drive roots have a trailing backslash (e.g. "C:\" instead of "C:")
        let mut normalized = path.replace('/', "\\");
        if normalized.len() == 2 && normalized.ends_with(':') {
            normalized.push('\\');
        }

        std::thread::spawn(move || {
            let wide_path = HSTRING::from(&normalized);
            let verb = HSTRING::from("properties");
            
            let mut info = SHELLEXECUTEINFOW::default();
            info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
            info.fMask = SEE_MASK_INVOKEIDLIST;
            info.lpVerb = PCWSTR(verb.as_ptr());
            info.lpFile = PCWSTR(wide_path.as_ptr());
            info.nShow = SW_SHOWNORMAL.0 as i32;

            unsafe {
                let _ = ShellExecuteExW(&mut info);
            }
        });
    }

    fn set_application_icon(&self, icon_bytes: &[u8]) {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, SendMessageW, WM_SETICON, ICON_BIG, ICON_SMALL,
            CreateIconFromResourceEx, LR_DEFAULTSIZE,
        };
        use windows::Win32::Foundation::{WPARAM, LPARAM};

        unsafe {
            let hicon = CreateIconFromResourceEx(
                icon_bytes,
                true,       // fIcon: true for icon, false for cursor
                0x00030000, // dwVersion: standard value for icons
                0, 0,       // width/height: use default
                LR_DEFAULTSIZE,
            );
            if let Ok(icon) = hicon {
                // On récupère la fenêtre au premier plan (celle de GPUI qui vient d'ouvrir)
                let hwnd = GetForegroundWindow();
                if !hwnd.is_invalid() {
                    let _ = SendMessageW(hwnd, WM_SETICON, Some(WPARAM(ICON_BIG as usize)), Some(LPARAM(icon.0 as isize)));
                    let _ = SendMessageW(hwnd, WM_SETICON, Some(WPARAM(ICON_SMALL as usize)), Some(LPARAM(icon.0 as isize)));
                }
            }
        }
    }
}
