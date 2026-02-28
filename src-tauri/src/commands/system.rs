use crate::models::{DriveInfo, WinMenuItem, QuickAccessItem, CommandError, SessionManager, SnapRect};
use crate::WindowState;
use crate::utils::path_security::validate_path;
use log::info;
use std::process::Command;
use tauri::{AppHandle, Emitter, State};
use std::path::PathBuf;

#[tauri::command]
pub fn get_drives(skip_hardware_info: Option<bool>) -> Vec<DriveInfo> {
    let skip_hardware_info = skip_hardware_info.unwrap_or(false);
    #[cfg(target_os = "windows")]
    {
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::{
            GetDriveTypeW, GetLogicalDriveStringsW, GetVolumeInformationW, GetDiskFreeSpaceExW, CreateFileW, 
            FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS
        };
        use windows::Win32::System::Ioctl::{IOCTL_STORAGE_QUERY_PROPERTY, IOCTL_STORAGE_GET_DEVICE_NUMBER, STORAGE_PROPERTY_QUERY, StorageDeviceSeekPenaltyProperty, DEVICE_SEEK_PENALTY_DESCRIPTOR, STORAGE_DEVICE_NUMBER, StorageAdapterProperty, STORAGE_ADAPTER_DESCRIPTOR, PropertyStandardQuery};
        use windows::Win32::System::IO::DeviceIoControl;

        let mut drives = Vec::new();
        unsafe {
            let mut buffer = [0u16; 260];
            let len = GetLogicalDriveStringsW(Some(&mut buffer));
            if len > 0 {
                let drive_strings = String::from_utf16_lossy(&buffer[..len as usize]);
                for drive_path in drive_strings.split('\0').filter(|s| !s.is_empty()) {
                    let root_path: Vec<u16> = drive_path
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect();

                    let win_type = GetDriveTypeW(PCWSTR(root_path.as_ptr()));
                    if win_type <= 1 {
                        continue;
                    }

                    let drive_type = match win_type {
                        2 => "removable".to_string(),
                        3 => "fixed".to_string(),
                        4 => "remote".to_string(),
                        5 => "cdrom".to_string(),
                        _ => "unknown".to_string(),
                    };

                    let mut volume_name = [0u16; 261];
                    let mut fs_name = [0u16; 261];
                    let mut flags = 0u32;

                    let (label, is_readonly_vol) = if GetVolumeInformationW(
                        PCWSTR(root_path.as_ptr()),
                        Some(&mut volume_name),
                        None,
                        None,
                        Some(&mut flags),
                        Some(&mut fs_name),
                    )
                    .is_ok()
                    {
                        (
                            String::from_utf16_lossy(&volume_name)
                                .trim_matches(char::from(0))
                                .to_string(),
                            (flags & 0x00080000) != 0, // FILE_READ_ONLY_VOLUME
                        )
                    } else {
                        (String::new(), false)
                    };

                    // Get disk space information
                    let mut free_bytes_available = 0u64;
                    let mut total_bytes = 0u64;
                    let mut total_free_bytes = 0u64;

                    let _ = GetDiskFreeSpaceExW(
                        PCWSTR(root_path.as_ptr()),
                        Some(&mut free_bytes_available),
                        Some(&mut total_bytes),
                        Some(&mut total_free_bytes),
                    );

                    // Get Hardware Info (SSD/HDD/Bus)
                    let mut media_type = None;
                    let mut physical_id = None;
                    
                    if !skip_hardware_info && (win_type == 3 || win_type == 2 || win_type == 5) { // Fixed, Removable or CD-ROM
                        let drive_root_unf = format!("\\\\.\\{}:", &drive_path[0..1]);
                        let wide_path: Vec<u16> = drive_root_unf.encode_utf16().chain(std::iter::once(0)).collect();
                        
                        let handle = CreateFileW(
                            PCWSTR(wide_path.as_ptr()),
                            0,
                            FILE_SHARE_READ | FILE_SHARE_WRITE,
                            None,
                            OPEN_EXISTING,
                            FILE_FLAG_BACKUP_SEMANTICS,
                            None,
                        );

                        if let Ok(h) = handle {
                            if !h.is_invalid() {
                                // 1. Get Physical ID
                                let mut device_number = STORAGE_DEVICE_NUMBER::default();
                                let mut bytes_returned = 0u32;
                                if DeviceIoControl(h, IOCTL_STORAGE_GET_DEVICE_NUMBER, None, 0, Some(&mut device_number as *mut _ as *mut _), std::mem::size_of::<STORAGE_DEVICE_NUMBER>() as u32, Some(&mut bytes_returned), None).is_ok() {
                                    physical_id = Some(format!("Disk {}", device_number.DeviceNumber));
                                }

                                // 2. Detect SSD vs HDD (Seek Penalty)
                                let mut query = STORAGE_PROPERTY_QUERY {
                                    PropertyId: StorageDeviceSeekPenaltyProperty,
                                    QueryType: PropertyStandardQuery,
                                    ..Default::default()
                                };
                                let mut descriptor = DEVICE_SEEK_PENALTY_DESCRIPTOR::default();
                                if DeviceIoControl(h, IOCTL_STORAGE_QUERY_PROPERTY, Some(&mut query as *mut _ as *mut _), std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32, Some(&mut descriptor as *mut _ as *mut _), std::mem::size_of::<DEVICE_SEEK_PENALTY_DESCRIPTOR>() as u32, Some(&mut bytes_returned), None).is_ok() {
                                    media_type = Some(if descriptor.IncursSeekPenalty { "HDD".to_string() } else { "SSD".to_string() });
                                }

                                // 3. Detect Bus Type (USB, Virtual, etc)
                                let mut adapter_query = STORAGE_PROPERTY_QUERY {
                                    PropertyId: StorageAdapterProperty,
                                    QueryType: PropertyStandardQuery,
                                    ..Default::default()
                                };
                                let mut adapter_desc = STORAGE_ADAPTER_DESCRIPTOR::default();
                                if DeviceIoControl(h, IOCTL_STORAGE_QUERY_PROPERTY, Some(&mut adapter_query as *mut _ as *mut _), std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32, Some(&mut adapter_desc as *mut _ as *mut _), std::mem::size_of::<STORAGE_ADAPTER_DESCRIPTOR>() as u32, Some(&mut bytes_returned), None).is_ok() {
                                    match adapter_desc.BusType {
                                        7 => { // BusTypeUsb
                                            media_type = media_type.map(|m| format!("USB-{}", m)).or(Some("USB".to_string()));
                                        },
                                        15 => { // BusTypeFileBackedVirtual
                                            media_type = media_type.map(|m| format!("IMG-{}", m)).or(Some("IMAGE".to_string()));
                                        },
                                        _ => {}
                                    }
                                }

                                let _ = windows::Win32::Foundation::CloseHandle(h);
                            }
                        }
                    } 
                    
                    if media_type.is_none() && win_type == 5 {
                        media_type = Some("CD/DVD".to_string());
                    } else if win_type == 4 {
                        media_type = Some("NAS".to_string());
                        physical_id = Some("Network".to_string());
                    }

                    drives.push(DriveInfo {
                        path: drive_path.to_string(),
                        label: if label.is_empty() {
                            "Local Disk".to_string()
                        } else {
                            label
                        },
                        drive_type,
                        is_readonly: is_readonly_vol || win_type == 5, // CD-ROM is read-only
                        total_bytes,
                        free_bytes: free_bytes_available,
                        media_type,
                        physical_id,
                    });
                }
            }
        }

        // Sort drives alphabetically
        drives.sort_by(|a, b| a.path.cmp(&b.path));
        drives
    }
    #[cfg(not(target_os = "windows"))]
    {
        vec![DriveInfo {
            path: "/".to_string(),
            label: "Root".to_string(),
            drive_type: "fixed".to_string(),
            is_readonly: false,
            total_bytes: 0,
            free_bytes: 0,
        }]
    }
}

#[tauri::command]
pub fn get_accent_color() -> String {
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

#[tauri::command]
pub async fn open_item(path: String) -> Result<(), CommandError> {
    let pb = validate_path(&path)?;
    info!("Opening item: {:?}", pb);
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(pb)
            .spawn()
            .map_err(|e| CommandError::SystemError(e.to_string()))?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        // fallback for linux/mac if needed later
    }
    Ok(())
}

#[tauri::command]
pub fn set_webview_background(window: tauri::Window, color: String) -> Result<(), CommandError> {
    // Parse hex color (e.g., "#1e1e1e" or "#ffffff")
    let color = color.trim_start_matches('#');

    if color.len() != 6 {
        return Err(CommandError::Other("Invalid color format. Use #RRGGBB".to_string()));
    }

    let r = u8::from_str_radix(&color[0..2], 16).map_err(|_| CommandError::Other("Invalid red component".into()))?;
    let g = u8::from_str_radix(&color[2..4], 16).map_err(|_| CommandError::Other("Invalid green component".into()))?;
    let b = u8::from_str_radix(&color[4..6], 16).map_err(|_| CommandError::Other("Invalid blue component".into()))?;

    window
        .set_background_color(Some(tauri::window::Color(r, g, b, 255)))
        .map_err(|e| CommandError::SystemError(format!("Failed to set background color: {}", e)))?;

    Ok(())
}

#[tauri::command]
pub fn show_native_context_menu(window: tauri::Window, path: String) -> Result<(), CommandError> {
    #[cfg(target_os = "windows")]
    {
        use windows::core::{PCWSTR, PCSTR, PSTR, Interface};
        use windows::Win32::Foundation::{HWND, POINT, LPARAM, WPARAM, LRESULT};
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED, CoUninitialize};
        use windows::Win32::UI::Shell::{
             IContextMenu, IContextMenu2, IContextMenu3, IShellFolder, SHBindToParent, SHParseDisplayName, 
             CMINVOKECOMMANDINFO, CMF_NORMAL, CMF_EXPLORE, CMF_CANRENAME,
             GCS_VERBA
        };
        use windows::Win32::UI::WindowsAndMessaging::{
            CreatePopupMenu, GetCursorPos, TrackPopupMenu, SW_SHOWNORMAL, TPM_LEFTALIGN,
            TPM_RETURNCMD, TPM_RIGHTBUTTON, SetForegroundWindow, DestroyMenu,
            GetMenuItemCount, DeleteMenu, GetMenuItemID, MF_BYPOSITION,
            CreateWindowExW, DefWindowProcW, RegisterClassW, WNDCLASSW,
            CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, WS_OVERLAPPEDWINDOW,
            SetWindowLongPtrW, GetWindowLongPtrW, GWLP_USERDATA,
            WM_INITMENUPOPUP, WM_DRAWITEM, WM_MEASUREITEM, WM_MENUCHAR, DestroyWindow
        };

        let pb = validate_path(&path)?;
        let path_norm = pb.to_string_lossy().replace("/", "\\");
        let path_u16: Vec<u16> = path_norm.encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            let hwnd_raw = window.hwnd().map_err(|e| CommandError::SystemError(e.to_string()))?;
            let main_hwnd = HWND(hwnd_raw.0 as *mut _);

            let mut pidl_full = std::ptr::null_mut();
            SHParseDisplayName(PCWSTR(path_u16.as_ptr()), None, &mut pidl_full, 0, None)
                .map_err(|e| CommandError::SystemError(format!("SHParseDisplayName failed: {}", e)))?;

            let mut pidl_relative = std::ptr::null_mut();
            let parent_folder: IShellFolder = SHBindToParent(pidl_full, Some(&mut pidl_relative))
                .map_err(|e| {
                    windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                    CommandError::SystemError(format!("SHBindToParent failed: {:?}", e))
                })?;

            let pidl_relative_slice = [pidl_relative as *const _];
            let context_menu: IContextMenu = parent_folder
                .GetUIObjectOf(
                    main_hwnd,
                    &pidl_relative_slice,
                    None,
                )
                .map_err(|e| {
                    windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                    CommandError::SystemError(format!("GetUIObjectOf failed: {}", e))
                })?;

            // Cast to IContextMenu2/3 for submenu support
            let context_menu2: Option<IContextMenu2> = context_menu.cast().ok();
            let context_menu3: Option<IContextMenu3> = context_menu.cast().ok();

            // --- Window Class for Message Relay ---
            extern "system" fn menu_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
                unsafe {
                    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut (Option<IContextMenu2>, Option<IContextMenu3>);
                    if !ptr.is_null() {
                        let (ref cm2, ref cm3) = *ptr;
                        match msg {
                            WM_INITMENUPOPUP => {
                                if let Some(ref cm) = cm2 { let _ = cm.HandleMenuMsg(msg, wparam, lparam); }
                                else if let Some(ref cm) = cm3 { let _ = cm.HandleMenuMsg(msg, wparam, lparam); }
                            }
                            WM_DRAWITEM | WM_MEASUREITEM | WM_MENUCHAR => {
                                if let Some(ref cm) = cm3 {
                                    let mut result = LRESULT(0);
                                    if cm.HandleMenuMsg2(msg, wparam, lparam, Some(&mut result)).is_ok() {
                                        return result;
                                    }
                                } else if let Some(ref cm) = cm2 {
                                    if cm.HandleMenuMsg(msg, wparam, lparam).is_ok() {
                                        return LRESULT(0);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }

            let class_name: Vec<u16> = "TauriMenuRelay\0".encode_utf16().collect();
            let wnd_class = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(menu_wnd_proc),
                hInstance: windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap_or_default().into(),
                lpszClassName: PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };
            RegisterClassW(&wnd_class);

            let relay_hwnd = CreateWindowExW(
                Default::default(),
                PCWSTR(class_name.as_ptr()),
                None,
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT,
                None, None, Some(wnd_class.hInstance), None
            ).map_err(|e| CommandError::SystemError(e.to_string()))?;

            let mut context_data = (context_menu2, context_menu3);
            SetWindowLongPtrW(relay_hwnd, GWLP_USERDATA, &mut context_data as *mut _ as isize);
            // ---------------------------------------

            let hmenu = CreatePopupMenu().map_err(|e| CommandError::SystemError(e.to_string()))?;
            let _ = context_menu.QueryContextMenu(hmenu, 0, 1, 0x7FFF, CMF_NORMAL | CMF_EXPLORE | CMF_CANRENAME);

            // Filter standard verbs
            let count = GetMenuItemCount(Some(hmenu));
            let forbidden_verbs = ["cut", "copy", "paste", "delete", "rename", "properties", "link", "shortcut"];
            for i in (0..count).rev() {
                let id = GetMenuItemID(hmenu, i);
                if (1..=0x7FFF).contains(&id) {
                    let mut verb_buf = [0u8; 128];
                    if context_menu.GetCommandString((id - 1) as usize, GCS_VERBA, None, PSTR(verb_buf.as_mut_ptr()), verb_buf.len() as u32).is_ok() {
                        let verb = std::ffi::CStr::from_ptr(verb_buf.as_ptr() as *const i8).to_string_lossy().to_lowercase();
                        if forbidden_verbs.iter().any(|&v| verb.contains(v)) {
                            let _ = DeleteMenu(hmenu, i as u32, MF_BYPOSITION);
                        }
                    }
                }
            }

            let mut pos = POINT::default();
            let _ = GetCursorPos(&mut pos);
            let _ = SetForegroundWindow(relay_hwnd);

            let command = TrackPopupMenu(
                hmenu,
                TPM_LEFTALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD,
                pos.x, pos.y, Some(0),
                relay_hwnd,
                None
            );

            if command.0 > 0 {
                let info = CMINVOKECOMMANDINFO {
                    cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
                    hwnd: main_hwnd,
                    lpVerb: PCSTR((command.0 - 1) as *mut u8),
                    nShow: SW_SHOWNORMAL.0,
                    ..Default::default()
                };
                let _ = context_menu.InvokeCommand(&info);
            }

            let _ = DestroyMenu(hmenu);
            let _ = DestroyWindow(relay_hwnd);
            windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
            CoUninitialize();
        }
    }
    Ok(())
}
#[tauri::command]
pub fn get_native_context_menu_items(path: String, is_background: bool) -> Result<Vec<WinMenuItem>, CommandError> {
    #[cfg(target_os = "windows")]
    {
        use windows::core::{Interface, PCWSTR, PSTR};
        use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED, CoUninitialize};
        use windows::Win32::UI::Shell::{
            IContextMenu, IContextMenu2, IContextMenu3, IShellFolder, SHBindToParent, SHParseDisplayName,
            CMF_NORMAL, CMF_EXPLORE, CMF_CANRENAME, GCS_VERBA
        };
        use windows::Win32::UI::WindowsAndMessaging::{
            CreatePopupMenu, DestroyMenu, GetMenuItemCount, GetMenuItemID, GetMenuStringW,
            GetSubMenu, MF_BYPOSITION, WM_INITMENUPOPUP
        };

        let pb = validate_path(&path)?;
        let path_norm = pb.to_string_lossy().replace("/", "\\");
        let path_u16: Vec<u16> = path_norm.encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            let mut pidl_full = std::ptr::null_mut();
            SHParseDisplayName(PCWSTR(path_u16.as_ptr()), None, &mut pidl_full, 0, None)
                .map_err(|e| CommandError::SystemError(format!("SHParseDisplayName failed: {}", e)))?;

            let mut pidl_relative = std::ptr::null_mut();
            let parent_folder: IShellFolder = SHBindToParent(pidl_full, Some(&mut pidl_relative))
                .map_err(|e| {
                    windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                    CommandError::SystemError(format!("SHBindToParent failed: {:?}", e))
                })?;

            let context_menu: IContextMenu = if is_background {
                // For background menu, we need the IShellFolder of the target itself
                let target_folder: IShellFolder = parent_folder.BindToObject(pidl_relative, None)
                    .map_err(|e| {
                        windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                        CommandError::SystemError(format!("BindToObject failed: {:?}", e))
                    })?;
                
                // Use CreateViewObject to get the background context menu
                target_folder.CreateViewObject(HWND::default())
                    .map_err(|e| {
                        windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                        CommandError::SystemError(format!("CreateViewObject failed: {}", e))
                    })?
            } else {
                let pidl_relative_slice = [pidl_relative as *const _];
                parent_folder.GetUIObjectOf(
                    HWND::default(),
                    &pidl_relative_slice,
                    None,
                ).map_err(|e| {
                    windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                    CommandError::SystemError(format!("GetUIObjectOf failed: {}", e))
                })?
            };

            let hmenu = CreatePopupMenu().map_err(|e| CommandError::SystemError(e.to_string()))?;
            let _ = context_menu.QueryContextMenu(hmenu, 0, 1, 0x7FFF, CMF_NORMAL | CMF_EXPLORE | CMF_CANRENAME);

            let cm2: Option<IContextMenu2> = context_menu.cast().ok();
            let cm3: Option<IContextMenu3> = context_menu.cast().ok();

            fn scrape_level(
                hmenu: windows::Win32::UI::WindowsAndMessaging::HMENU,
                context_menu: &IContextMenu,
                cm2: Option<&IContextMenu2>,
                cm3: Option<&IContextMenu3>
            ) -> Vec<WinMenuItem> {
                unsafe {
                    let count = GetMenuItemCount(Some(hmenu));
                    if count < 0 { return Vec::new(); }
                    
                    let mut items = Vec::new();
                    let forbidden_verbs = ["cut", "copy", "paste", "delete", "rename", "properties", "link", "shortcut", "open"];

                    for i in 0..count {
                        let id = GetMenuItemID(hmenu, i);
                        let submenu = GetSubMenu(hmenu, i);
                        
                        // If it has a submenu, we MUST initialize it for extensions like NanaZip
                        if !submenu.is_invalid() {
                            let wparam = WPARAM(submenu.0 as usize);
                            let lparam = LPARAM((i & 0xFFFF) as isize); // loword: index, hiword: fSystemMenu
                            if let Some(cm) = cm2 { let _ = cm.HandleMenuMsg(WM_INITMENUPOPUP, wparam, lparam); }
                            else if let Some(cm) = cm3 { let _ = cm.HandleMenuMsg(WM_INITMENUPOPUP, wparam, lparam); }
                        }

                        let mut label_buf = [0u16; 256];
                        let len = GetMenuStringW(hmenu, i as u32, Some(&mut label_buf), MF_BYPOSITION);
                        let mut label = String::from_utf16_lossy(&label_buf[..len as usize]);
                        
                        // Clean up & in labels (e.g. "&Open" -> "Open")
                        label = label.replace("&", "");

                        if label.is_empty() || label == "-" || label.is_empty() {
                            continue;
                        }

                        let is_new_menu = label.to_lowercase().contains("nouveau") || label.to_lowercase() == "new";

                        // Filter standard verbs
                        if (1..=0x7FFF).contains(&id) && !is_new_menu {
                            let mut verb_buf = [0u8; 128];
                            if context_menu.GetCommandString((id - 1) as usize, GCS_VERBA, None, PSTR(verb_buf.as_mut_ptr()), verb_buf.len() as u32).is_ok() {
                                let verb = std::ffi::CStr::from_ptr(verb_buf.as_ptr() as *const i8).to_string_lossy().to_lowercase();
                                if forbidden_verbs.iter().any(|&v| verb.contains(v)) {
                                    // If it's a submenu, we generally want to keep it (like "New" or context extensions)
                                    // unless it's one of the explicitly forbidden actions
                                    if submenu.is_invalid() {
                                        continue;
                                    }
                                }
                            }
                        }

                        // Attempt to extract the verb
                        let mut item_verb: Option<String> = None;
                        if (1..=0x7FFF).contains(&id) {
                            let mut verb_buf = [0u8; 128];
                            if context_menu.GetCommandString((id - 1) as usize, GCS_VERBA, None, PSTR(verb_buf.as_mut_ptr()), verb_buf.len() as u32).is_ok() {
                                let v = std::ffi::CStr::from_ptr(verb_buf.as_ptr() as *const i8).to_string_lossy().to_string();
                                if !v.is_empty() {
                                    item_verb = Some(v);
                                }
                            }
                        }

                        let children = if !submenu.is_invalid() {
                            scrape_level(submenu, context_menu, cm2, cm3)
                        } else {
                            Vec::new()
                        };

                        items.push(WinMenuItem {
                            id: id as i32,
                            label,
                            verb: item_verb,
                            has_submenu: !submenu.is_invalid(),
                            children,
                        });
                    }
                    items
                }
            }

            let items = scrape_level(hmenu, &context_menu, cm2.as_ref(), cm3.as_ref());

            let _ = DestroyMenu(hmenu);
            windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
            CoUninitialize();

            Ok(items)
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(Vec::new())
    }
}

#[tauri::command]
pub fn execute_native_menu_item(window: tauri::Window, path: String, id: i32, is_background: bool) -> Result<(), CommandError> {
    #[cfg(target_os = "windows")]
    {
        use windows::core::{PCWSTR, PCSTR, Interface};
        use windows::Win32::Foundation::HWND;
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED, CoUninitialize};
        use windows::Win32::UI::Shell::{
            IContextMenu, IContextMenu2, IContextMenu3, IShellFolder, SHBindToParent, SHParseDisplayName, 
            CMINVOKECOMMANDINFO, CMF_NORMAL, CMF_EXPLORE, CMF_CANRENAME
        };
        use windows::Win32::UI::WindowsAndMessaging::{CreatePopupMenu, DestroyMenu, SW_SHOWNORMAL};

        let pb = validate_path(&path)?;
        let path_norm = pb.to_string_lossy().replace("/", "\\");
        let path_u16: Vec<u16> = path_norm.encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            let hwnd_raw = window.hwnd().map_err(|e| CommandError::SystemError(e.to_string()))?;
            let main_hwnd = HWND(hwnd_raw.0 as *mut _);

            let mut pidl_full = std::ptr::null_mut();
            SHParseDisplayName(PCWSTR(path_u16.as_ptr()), None, &mut pidl_full, 0, None)
                .map_err(|e| CommandError::SystemError(format!("SHParseDisplayName failed: {}", e)))?;

            let mut pidl_relative = std::ptr::null_mut();
            let parent_folder: IShellFolder = SHBindToParent(pidl_full, Some(&mut pidl_relative))
                .map_err(|e| {
                    windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                    CommandError::SystemError(format!("SHBindToParent failed: {:?}", e))
                })?;

            let context_menu: IContextMenu = if is_background {
                let target_folder: IShellFolder = parent_folder.BindToObject(pidl_relative, None)
                    .map_err(|e| {
                        windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                        CommandError::SystemError(format!("BindToObject failed: {:?}", e))
                    })?;
                
                target_folder.CreateViewObject(main_hwnd)
                    .map_err(|e| {
                        windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                        CommandError::SystemError(format!("CreateViewObject failed: {}", e))
                    })?
            } else {
                let pidl_relative_slice = [pidl_relative as *const _];
                parent_folder.GetUIObjectOf(
                    main_hwnd,
                    &pidl_relative_slice,
                    None,
                ).map_err(|e| {
                    windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                    CommandError::SystemError(format!("GetUIObjectOf failed: {}", e))
                })?
            };

            // We MUST call QueryContextMenu before InvokeCommand so the extension knows what IDs we are talking about
            let hmenu = CreatePopupMenu().map_err(|e| CommandError::SystemError(e.to_string()))?;
            let _ = context_menu.QueryContextMenu(hmenu, 0, 1, 0x7FFF, CMF_NORMAL | CMF_EXPLORE | CMF_CANRENAME);

            // CRITICAL: For submenus like "New", we must initialize them so the extension populates the IDs
            let cm2: Option<IContextMenu2> = context_menu.cast().ok();
            let cm3: Option<IContextMenu3> = context_menu.cast().ok();

            fn init_all_submenus(
                hmenu: windows::Win32::UI::WindowsAndMessaging::HMENU,
                cm2: Option<&IContextMenu2>,
                cm3: Option<&IContextMenu3>
            ) {
                unsafe {
                    use windows::Win32::UI::WindowsAndMessaging::{GetMenuItemCount, GetSubMenu, WM_INITMENUPOPUP};
                    use windows::Win32::Foundation::{WPARAM, LPARAM};

                    let count = GetMenuItemCount(Some(hmenu));
                    for i in 0..count {
                        let submenu = GetSubMenu(hmenu, i);
                        if !submenu.is_invalid() {
                            let wparam = WPARAM(submenu.0 as usize);
                            let lparam = LPARAM((i & 0xFFFF) as isize);
                            if let Some(cm) = cm2 { let _ = cm.HandleMenuMsg(WM_INITMENUPOPUP, wparam, lparam); }
                            else if let Some(cm) = cm3 { let _ = cm.HandleMenuMsg(WM_INITMENUPOPUP, wparam, lparam); }
                            
                            // Recurse
                            init_all_submenus(submenu, cm2, cm3);
                        }
                    }
                }
            }
            init_all_submenus(hmenu, cm2.as_ref(), cm3.as_ref());

            if id > 0 {
                let info = CMINVOKECOMMANDINFO {
                    cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
                    hwnd: main_hwnd,
                    lpVerb: PCSTR((id - 1) as *mut u8),
                    nShow: SW_SHOWNORMAL.0,
                    ..Default::default()
                };

                let _ = context_menu.InvokeCommand(&info);
            }

            let _ = DestroyMenu(hmenu);
            windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
            CoUninitialize();
        }
    }
    Ok(())
}
#[tauri::command]
pub async fn get_mounted_images() -> Result<Vec<String>, CommandError> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        use std::os::windows::process::CommandExt;

        let script = "Get-Volume | Get-DiskImage -ErrorAction SilentlyContinue | Select-Object -ExpandProperty ImagePath";

        let output = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(script)
            .creation_flags(0x08000000)
            .output()
            .map_err(|e| CommandError::SystemError(e.to_string()))?;

        let res = String::from_utf8_lossy(&output.stdout);
        let mut paths = Vec::new();
        for line in res.lines() {
            let path = line.trim();
            if !path.is_empty() {
                paths.push(path.to_string());
            }
        }
        Ok(paths)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(Vec::new())
    }
}

#[tauri::command]
pub async fn mount_disk_image(app: AppHandle, path: String) -> Result<(), CommandError> {
    #[cfg(target_os = "windows")]
    {
        let pb = validate_path(&path)?;
        info!("Mounting disk image: {:?}", pb);
        // Use PowerShell to mount the disk image
        let output = Command::new("powershell")
            .arg("-Command")
            .arg(format!(
                "$OutputEncoding = [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; \
                Mount-DiskImage -ImagePath \"{}\"", 
                pb.to_string_lossy()
            ))
            .output()
            .map_err(|e| CommandError::SystemError(e.to_string()))?;

        if !output.status.success() {
            let err_final = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(CommandError::SystemError(format!("Failed to mount image: {}", err_final)));
        }

        // Notify frontend
        let _ = app.emit("drives-changed", ());

        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(CommandError::SystemError("Disk image mounting is only supported on Windows".to_string()))
    }
}


#[tauri::command]
pub async fn unmount_disk_image(
    app: AppHandle,
    state: State<'_, SessionManager>,
    path: String
) -> Result<(), CommandError> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;

        // 1. Identify all affected drive letters BEFORE locking session
        let mut affected_drives = Vec::new();
        {
            let target_lower = path.to_lowercase();
            // Check if it's a simple drive letter (length 2 or 3, e.g., "F:" or "F:\")
            let is_drive_root = target_lower.len() <= 3 && target_lower.contains(':');
            
            if is_drive_root {
                let drive_letter = &target_lower[0..1];
                let check_cmd = format!(
                    "Get-Partition -DriveLetter {} -ErrorAction SilentlyContinue | Get-Disk | Get-Partition | Where-Object DriveLetter | Select-Object -ExpandProperty DriveLetter",
                    drive_letter
                );
                if let Ok(out) = Command::new("powershell").arg("-Command").arg(&check_cmd).output() {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    for line in stdout.lines() {
                        let l = line.trim().to_lowercase();
                        if !l.is_empty() {
                            affected_drives.push(format!("{}:", l));
                        }
                    }
                }
                // Ensure the direct target is included
                let target_root = format!("{}:", drive_letter);
                if !affected_drives.contains(&target_root) {
                    affected_drives.push(target_root);
                }
            } else {
                // It's a full path to a disk image file (.iso, .vhd)
                // We ONLY want to find the drive letter(s) that were mounted FROM this file.
                // We must NOT include the drive where the file itself is stored.
                let check_cmd = format!(
                    "Get-DiskImage -ImagePath \"{}\" -ErrorAction SilentlyContinue | Get-Volume | Select-Object -ExpandProperty DriveLetter",
                    path
                );
                if let Ok(out) = Command::new("powershell").arg("-Command").arg(&check_cmd).output() {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    for line in stdout.lines() {
                        let l = line.trim().to_lowercase();
                        if !l.is_empty() {
                            affected_drives.push(format!("{}:", l));
                        }
                    }
                }
            }
        }

        // 2. Handle Session State (Close tabs on these drives)
        {
            // Scope the lock so it releases before we run the slow unmount command
            let mut session = state.0.lock().map_err(|_| CommandError::SystemError("Failed to lock session state".to_string()))?;
            
            let clean_panel = |panel: &mut crate::models::session::PanelState| {
                let mut tabs_to_keep = Vec::new();
                let mut active_id_invalidated = false;

                for tab in &panel.tabs {
                    let tab_path_lower = tab.path.to_string_lossy().to_lowercase();
                    if affected_drives.iter().any(|d| tab_path_lower.starts_with(d)) {
                        if tab.id == panel.active_tab_id {
                            active_id_invalidated = true;
                        }
                    } else {
                        tabs_to_keep.push(tab.clone());
                    }
                }

                if tabs_to_keep.is_empty() {
                    // Panel becomes empty, must add fallback
                    let new_id = uuid::Uuid::new_v4().to_string();
                    tabs_to_keep.push(crate::models::Tab {
                        id: new_id.clone(),
                        path: PathBuf::from("C:\\"),
                        version: 0,
                    });
                    panel.active_tab_id = new_id;
                } else if active_id_invalidated {
                    // Active tab closed, switch to another one
                    if let Some(last) = tabs_to_keep.last() {
                        panel.active_tab_id = last.id.clone();
                    }
                }
                panel.tabs = tabs_to_keep;
            };

            clean_panel(&mut session.left_panel);
            clean_panel(&mut session.right_panel);

            // CRITICAL: Release file watchers on the drive before unmounting/ejecting
            session.left_panel.update_watcher(&app);
            session.right_panel.update_watcher(&app);

            let _ = app.emit("session_changed", session.clone());
        } // session lock released here

        if path.len() <= 3 && path.contains(':') {
            let drive_letter = path.chars().next().ok_or(CommandError::PathError("Empty drive path".to_string()))?;
            let cmd = format!(
                "$OutputEncoding = [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; \
                $driveLetter = '{}:'; \
                $sa = New-Object -ComObject Shell.Application; \
                $ns = $sa.NameSpace(17); \
                $item = $ns.ParseName($driveLetter); \
                if ($item) {{ \
                    $verbs = $item.Verbs() | Where-Object {{ $_.Name.Replace('&','') -match 'Eject|Éjecter|Ejection|Auswerfen|Ejectar' }}; \
                    if ($verbs) {{ \
                        foreach ($v in $verbs) {{ $v.DoIt(); break; }} \
                    }} else {{ \
                        $item.InvokeVerb('Eject'); \
                    }} \
                }} \
                if (Get-DiskImage -DevicePath \"\\\\.\\$driveLetter\" -ErrorAction SilentlyContinue) {{ \
                    Dismount-DiskImage -DevicePath \"\\\\.\\$driveLetter\" -ErrorAction SilentlyContinue; \
                }} \
                $timeout = 20; \
                while ($timeout -gt 0) {{ \
                    if (!(Get-PSDrive $driveLetter.Replace(':','') -ErrorAction SilentlyContinue)) {{ \
                        exit 0; \
                    }} \
                    Start-Sleep -Milliseconds 100; \
                    $timeout--; \
                }} \
                exit 1;",
                drive_letter
            );

            let output = Command::new("powershell")
                .arg("-Command")
                .arg(cmd)
                .output()
                .map_err(|e| CommandError::SystemError(e.to_string()))?;

            if !output.status.success() {
                return Err(CommandError::SystemError("Failed to eject. The drive might be in use by another program.".to_string()));
            }
        } else {
            let pb = validate_path(&path)?;
            info!("Unmounting disk image: {:?}", pb);
            let cmd = format!(
                "$OutputEncoding = [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; \
                Dismount-DiskImage -ImagePath \"{}\"", 
                pb.to_string_lossy()
            );
            let output = Command::new("powershell")
                .arg("-Command")
                .arg(cmd)
                .output()
                .map_err(|e| CommandError::SystemError(e.to_string()))?;

            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr).to_string();
                return Err(CommandError::SystemError(format!("Failed to unmount image: {}", err)));
            }
        };

        // Notify Shell of change (Force Explorer refresh)
        unsafe {
            use windows::Win32::UI::Shell::{SHChangeNotify, SHCNE_DRIVEREMOVED, SHCNF_PATHW};
            let path_u16: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            SHChangeNotify(SHCNE_DRIVEREMOVED, SHCNF_PATHW, Some(path_u16.as_ptr() as *const _), None);
        }

        // Notify frontend that drive list might have changed
        let _ = app.emit("drives-changed", ());

        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(CommandError::SystemError("Disk image unmounting is only supported on Windows".to_string()))
    }
}

#[tauri::command]
pub fn oxide_sync_snap_rect(state: tauri::State<'_, WindowState>, rect: SnapRect) {
    let mut m = state.maximize_button_rect.lock().unwrap();
    *m = Some(rect);
}

#[tauri::command]
pub fn get_quick_access_items() -> Result<Vec<QuickAccessItem>, CommandError> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        use std::os::windows::process::CommandExt;

        // Use PowerShell to get Quick Access pinned items. 
        // This is much more reliable across Windows versions than low-level COM enumeration.
        let script = "
            $OutputEncoding = [Console]::OutputEncoding = [System.Text.Encoding]::UTF8;
            $sh = New-Object -ComObject Shell.Application;
            $quickAccess = $sh.Namespace('shell:::{679f85cb-0220-4080-b29b-5540cc05aab6}');
            if ($quickAccess) {
                $items = $quickAccess.Items() | Where-Object { $_.IsFolder -eq $true };
                $results = foreach ($item in $items) {
                    if ($item.Path -and $item.Path -notlike '::{*') {
                        [PSCustomObject]@{
                            name = $item.Name;
                            path = $item.Path;
                        }
                    }
                }
                $results | ConvertTo-Json -Compress
            } else {
                '[]'
            }
        ";

        let output = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(script)
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output()
            .map_err(|e| CommandError::SystemError(e.to_string()))?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() || stdout == "[]" {
            return Ok(Vec::new());
        }

        // Handle both single object and array output from PowerShell
        if stdout.starts_with('{') {
             if let Ok(item) = serde_json::from_str::<QuickAccessItem>(&stdout) {
                 return Ok(vec![item]);
             }
        }
        
        let items: Vec<QuickAccessItem> = serde_json::from_str(&stdout).unwrap_or_default();
        Ok(items)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(Vec::new())
    }
}

#[tauri::command]
pub fn add_to_quick_access(app: AppHandle, path: String) -> Result<(), CommandError> {
    #[cfg(target_os = "windows")]
    {
        let res = execute_shell_verb_by_canonical_name(&app, &path, &["pintohome", "pintofavorites"]);
        if res.is_ok() {
            let _ = app.emit("quick-access-changed", ());
        }
        res
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(CommandError::SystemError("Quick access is only supported on Windows".to_string()))
    }
}

#[tauri::command]
pub fn remove_from_quick_access(app: AppHandle, path: String) -> Result<(), CommandError> {
    #[cfg(target_os = "windows")]
    {
        let res = execute_shell_verb_by_canonical_name(&app, &path, &["unpinfromhome", "unpinfromquickaccess"]);
        if res.is_ok() {
            let _ = app.emit("quick-access-changed", ());
        }
        res
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(CommandError::SystemError("Quick access is only supported on Windows".to_string()))
    }
}

#[cfg(target_os = "windows")]
fn execute_shell_verb_by_canonical_name(_app: &AppHandle, path: &str, target_verbs: &[&str]) -> Result<(), CommandError> {
    use windows::core::{PCWSTR, PCSTR, PSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED, CoUninitialize};
    use windows::Win32::UI::Shell::{
        IContextMenu, IShellFolder, SHBindToParent, SHParseDisplayName, 
        CMINVOKECOMMANDINFO, CMF_NORMAL, GCS_VERBA
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreatePopupMenu, DestroyMenu, GetMenuItemCount, GetMenuItemID, SW_SHOWNORMAL, GetMenuStringW, MF_BYPOSITION
    };

    let pb = validate_path(path)?;
    let mut path_norm = pb.to_string_lossy().replace("/", "\\");
    if path_norm.len() == 2 && path_norm.ends_with(':') {
        path_norm.push('\\');
    }
    let path_u16: Vec<u16> = path_norm.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let mut pidl_full = std::ptr::null_mut();
        SHParseDisplayName(PCWSTR(path_u16.as_ptr()), None, &mut pidl_full, 0, None)
            .map_err(|e| CommandError::SystemError(format!("SHParseDisplayName failed: {}", e)))?;

        let mut pidl_relative = std::ptr::null_mut();
        let parent_folder: IShellFolder = SHBindToParent(pidl_full, Some(&mut pidl_relative))
            .map_err(|e| {
                windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                CommandError::SystemError(format!("SHBindToParent failed: {:?}", e))
            })?;

        let pidl_relative_slice = [pidl_relative as *const _];
        let context_menu: IContextMenu = parent_folder.GetUIObjectOf(
            HWND(std::ptr::null_mut()),
            &pidl_relative_slice,
            None,
        ).map_err(|e| {
            windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
            CommandError::SystemError(format!("GetUIObjectOf failed: {}", e))
        })?;

        let hmenu = CreatePopupMenu().map_err(|e| CommandError::SystemError(e.to_string()))?;
        let _ = context_menu.QueryContextMenu(hmenu, 0, 1, 0x7FFF, CMF_NORMAL);

        let count = GetMenuItemCount(Some(hmenu));
        let mut target_id: Option<u32> = None;

        let is_unpin = target_verbs.iter().any(|v| v.contains("unpin"));

        for i in 0..count {
            let id = GetMenuItemID(hmenu, i);
            if id != u32::MAX && id > 0 {
                // 1. Try canonical verb lookup first
                let mut verb_buf = [0u8; 128];
                if context_menu.GetCommandString((id - 1) as usize, GCS_VERBA, None, PSTR(verb_buf.as_mut_ptr()), verb_buf.len() as u32).is_ok() {
                    let verb = std::ffi::CStr::from_ptr(verb_buf.as_ptr() as *const i8).to_string_lossy().to_lowercase();
                    if target_verbs.iter().any(|&v| verb == v) {
                        target_id = Some(id);
                        break;
                    }
                }

                // 2. Try localized label matching as fallback (Flexible/Substrings)
                let mut label_buf = [0u16; 256];
                let len = GetMenuStringW(hmenu, i as u32, Some(&mut label_buf), MF_BYPOSITION);
                if len > 0 {
                    let label = String::from_utf16_lossy(&label_buf[..len as usize]).to_lowercase();
                    // Clean symbols & accents for better matching
                    let clean = label.replace("&", "").replace("'", "").replace("’", "");
                    
                    if is_unpin {
                        // Match "Désépingler", "Unpin", "Retirer" AND ("Accès", "Accueil", "Favori", "Quick", "Home")
                        let has_unpin_core = clean.contains("desepingl") || clean.contains("unpin") || clean.contains("retirer") || clean.contains("detacher") || clean.contains("lösen") || clean.contains("epingl"); // some systems use "épingler" for toggle
                        let has_target_core = clean.contains("acces") || clean.contains("accueil") || clean.contains("favori") || clean.contains("quick") || clean.contains("home") || clean.contains("schnell");
                        
                        if has_unpin_core && has_target_core {
                            target_id = Some(id);
                            break;
                        }
                    } else {
                        // Match "Épingler", "Pin", "Attacher" AND ("Accès", "Accueil", "Favori", "Quick", "Home")
                        let has_pin_core = clean.contains("epingl") || clean.contains("pin") || clean.contains("attach") || clean.contains("anheft");
                        let has_target_core = clean.contains("acces") || clean.contains("accueil") || clean.contains("favori") || clean.contains("quick") || clean.contains("home") || clean.contains("schnell");
                        
                        if has_pin_core && has_target_core {
                            target_id = Some(id);
                            break;
                        }
                    }
                }
            }
        }

        let mut result = if let Some(id) = target_id {
            let ici = CMINVOKECOMMANDINFO {
                cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
                fMask: 0,
                hwnd: HWND(std::ptr::null_mut()),
                lpVerb: PCSTR((id - 1) as *mut u8),
                nShow: SW_SHOWNORMAL.0,
                ..Default::default()
            };
            context_menu.InvokeCommand(&ici).map_err(|e| CommandError::SystemError(format!("InvokeCommand failed: {}", e)))
        } else {
            Err(CommandError::SystemError("No matching verb found".to_string()))
        };

        // 3. ULTIMATE RECOURSE: PowerShell Script
        if result.is_err() {
            use std::process::Command;
            use std::os::windows::process::CommandExt;
            
            let p_safe = path_norm.replace("'", "''");
            let script = if is_unpin {
                format!(
                    "$sh = New-Object -ComObject Shell.Application; \
                     $qa = $sh.Namespace('shell:::{{679f85cb-0220-4080-b29b-5540cc05aab6}}'); \
                     if ($qa) {{ \
                         $target = '{}'; \
                         $item = $qa.Items() | Where-Object {{ $_.Path -eq $target -or $_.GetFolder.Self.Path -eq $target }}; \
                         if ($item) {{ \
                             $verbs = $item.Verbs() | Where-Object {{ $_.Name.Replace('&','') -match 'unpin|desepingler|retirer|detacher|losen' }}; \
                             if ($verbs) {{ foreach ($v in $verbs) {{ $v.DoIt(); break; }} }} \
                             else {{ $item.InvokeVerb('unpinfromhome'); $item.InvokeVerb('unpinfromquickaccess'); }} \
                         }} \
                     }}", p_safe
                )
            } else {
                format!(
                    "$sh = New-Object -ComObject Shell.Application; \
                     $folder = $sh.Namespace('{}'); \
                     if ($folder) {{ \
                         $item = $folder.Self; \
                         $verbs = $item.Verbs() | Where-Object {{ $_.Name.Replace('&','') -match 'pin|epingler|attacher|anheft' }}; \
                         if ($verbs) {{ foreach ($v in $verbs) {{ $v.DoIt(); break; }} }} \
                         else {{ $item.InvokeVerb('pintohome'); $item.InvokeVerb('pintofavorites'); }} \
                     }}", p_safe
                )
            };

            let output = Command::new("powershell")
                .arg("-NoProfile")
                .arg("-Command")
                .arg(script)
                .creation_flags(0x08000000)
                .output();
            
            if let Ok(out) = output {
                if out.status.success() {
                    result = Ok(());
                }
            }
        }

        let _ = DestroyMenu(hmenu);
        windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
        CoUninitialize();
        
        result
    }
}

#[tauri::command]
pub fn clear_app_cache(app: tauri::AppHandle) -> Result<(), CommandError> {
    use tauri::Manager;
    
    // Clear Webview Browsing Data (EBWebView)
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.clear_all_browsing_data();
    }

    // Clear local data (thumbnails, thumbnails.db, etc.) except EBWebView
    if let Ok(path) = app.path().app_local_data_dir() {
        if path.exists() {
            if let Ok(entries) = std::fs::read_dir(&path) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let entry_path = entry.path();
                    let file_name = entry_path.file_name().unwrap_or_default().to_string_lossy();
                    
                    // Do not attempt to manually delete EBWebView directory since webview uses it
                    if file_name.to_lowercase().contains("ebwebview") || file_name.to_lowercase() == "webview2" {
                        continue;
                    }

                    if entry_path.is_dir() {
                        let _ = std::fs::remove_dir_all(&entry_path);
                    } else {
                        let _ = std::fs::remove_file(&entry_path);
                    }
                }
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn restart_app(app: tauri::AppHandle) {
    if let Ok(current_exe) = std::env::current_exe() {
        let _ = std::process::Command::new(current_exe).spawn();
        app.exit(0);
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Default)]
pub struct PeekStatus {
    pub installed: bool,
    pub enabled: bool,
    pub space_enabled: bool,
    pub activation_shortcut: Option<String>,
}

#[tauri::command]
pub async fn get_peek_status() -> Result<PeekStatus, String> {
    let mut status = PeekStatus::default();

    #[cfg(target_os = "windows")]
    {
        use std::path::Path;
        let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let program_files = std::env::var("ProgramFiles").unwrap_or_default();

        let exe_paths = vec![
            format!("{}\\PowerToys\\WinUI3Apps\\PowerToys.Peek.UI.exe", local_app_data),
            format!("{}\\Microsoft\\PowerToys\\PowerToys.Peek.UI.exe", local_app_data),
            format!("{}\\PowerToys\\WinUI3Apps\\PowerToys.Peek.UI.exe", program_files),
            format!("{}\\PowerToys\\PowerToys.Peek.UI.exe", program_files),
        ];

        status.installed = exe_paths.iter().any(|p| Path::new(p).exists());
        if !status.installed {
            return Ok(status);
        }

        // Check if enabled in general settings
        let general_settings_path = format!("{}\\Microsoft\\PowerToys\\settings.json", local_app_data);
        if let Ok(content) = std::fs::read_to_string(general_settings_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(enabled) = json.get("enabled").and_then(|e| e.get("Peek")).and_then(|p| p.as_bool()) {
                    status.enabled = enabled;
                }
            }
        }

        // Check Peek specific settings
        let peek_settings_path = format!("{}\\Microsoft\\PowerToys\\Peek\\settings.json", local_app_data);
        if let Ok(content) = std::fs::read_to_string(peek_settings_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(props) = json.get("properties") {
                    status.space_enabled = props.get("EnableSpaceToActivate")
                        .and_then(|v| v.get("value"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    
                    if let Some(shortcut) = props.get("ActivationShortcut") {
                        let mut parts = Vec::new();
                        if shortcut.get("ctrl").and_then(|v| v.as_bool()).unwrap_or(false) { parts.push("Ctrl"); }
                        if shortcut.get("shift").and_then(|v| v.as_bool()).unwrap_or(false) { parts.push("Shift"); }
                        if shortcut.get("alt").and_then(|v| v.as_bool()).unwrap_or(false) { parts.push("Alt"); }
                        if shortcut.get("win").and_then(|v| v.as_bool()).unwrap_or(false) { parts.push("Win"); }
                        
                        let code = shortcut.get("code").and_then(|v| v.as_u64()).unwrap_or(0);
                        let key_str = match code {
                            32 => Some("Space"),
                            48 => Some("0"), 49 => Some("1"), 50 => Some("2"), 51 => Some("3"), 52 => Some("4"),
                            53 => Some("5"), 54 => Some("6"), 55 => Some("7"), 56 => Some("8"), 57 => Some("9"),
                            65 => Some("A"), 66 => Some("B"), 67 => Some("C"), 68 => Some("D"), 69 => Some("E"),
                            70 => Some("F"), 71 => Some("G"), 72 => Some("H"), 73 => Some("I"), 74 => Some("J"),
                            75 => Some("K"), 76 => Some("L"), 77 => Some("M"), 78 => Some("N"), 79 => Some("O"),
                            80 => Some("P"), 81 => Some("Q"), 82 => Some("R"), 83 => Some("S"), 84 => Some("T"),
                            85 => Some("U"), 86 => Some("V"), 87 => Some("W"), 88 => Some("X"), 89 => Some("Y"),
                            90 => Some("Z"),
                            _ => None,
                        };
                        
                        if let Some(k) = key_str {
                            parts.push(k);
                            status.activation_shortcut = Some(parts.join("+"));
                        }
                    }
                }
            }
        }
    }
    Ok(status)
}

#[tauri::command]
pub async fn open_peek(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let program_files = std::env::var("ProgramFiles").unwrap_or_default();

        let paths = vec![
            format!("{}\\PowerToys\\WinUI3Apps\\PowerToys.Peek.UI.exe", local_app_data),
            format!("{}\\Microsoft\\PowerToys\\PowerToys.Peek.UI.exe", local_app_data),
            format!("{}\\PowerToys\\WinUI3Apps\\PowerToys.Peek.UI.exe", program_files),
            format!("{}\\PowerToys\\PowerToys.Peek.UI.exe", program_files),
        ];

        let mut exe_path = None;
        for p in paths {
            if std::path::Path::new(&p).exists() {
                exe_path = Some(p);
                break;
            }
        }

        if let Some(exe) = exe_path {
            let mut cmd = Command::new(exe);
            cmd.arg(&path);
            
            // Try to set the CWD to the parent of the file to help Peek discover siblings
            if let Some(parent) = std::path::Path::new(&path).parent() {
                cmd.current_dir(parent);
            }
            
            cmd.spawn().map_err(|e| e.to_string())?;
            return Ok(());
        } else {
            return Err("PowerToys Peek not found".to_string());
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Peek is only available on Windows".to_string())
    }
}
