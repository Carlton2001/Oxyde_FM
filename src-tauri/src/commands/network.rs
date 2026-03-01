use crate::models::NetResource;
use windows::Win32::NetworkManagement::WNet::{
    WNetOpenEnumW, WNetEnumResourceW, WNetCloseEnum, WNetAddConnection2W, WNetCancelConnection2W,
    RESOURCE_GLOBALNET, RESOURCETYPE_ANY, NETRESOURCEW, RESOURCEUSAGE_CONTAINER,
    WNET_OPEN_ENUM_USAGE, RESOURCETYPE_DISK, CONNECT_UPDATE_PROFILE, NET_CONNECT_FLAGS
};
use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::UI::Shell::{
    SHGetKnownFolderItem, FOLDERID_NetworkFolder, KF_FLAG_DEFAULT, IShellItem,
    IEnumShellItems, SIGDN_NORMALDISPLAY, SIGDN_DESKTOPABSOLUTEPARSING, BHID_EnumItems,
    IContextMenu, CMF_NORMAL, GCS_VERBA, BHID_SFUIObject
};
use windows::Win32::UI::WindowsAndMessaging::{CreatePopupMenu, GetMenuItemCount, GetMenuItemID, DestroyMenu};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, IBindCtx};
use windows::core::{PWSTR, PSTR, PCWSTR};

#[tauri::command]
pub async fn get_network_resources(path: Option<String>) -> Result<Vec<NetResource>, String> {
    #[cfg(target_os = "windows")]
    {
        let mut resources = Vec::new();
        
        unsafe {
            if path.is_none() {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

                if let Ok(folder_item) = SHGetKnownFolderItem::<IShellItem>(
                    &FOLDERID_NetworkFolder,
                    KF_FLAG_DEFAULT,
                    None
                ) {
                    let bind_ctx: Option<&IBindCtx> = None;
                    if let Ok(enum_items) = folder_item.BindToHandler::<Option<&IBindCtx>, IEnumShellItems>(
                        bind_ctx,
                        &BHID_EnumItems
                    ) {
                        let mut item: [Option<IShellItem>; 1] = [None];
                        let mut fetched = 0;
                        
                        while enum_items.Next(&mut item, Some(&mut fetched)).is_ok() && fetched == 1 {
                            if let Some(i) = &item[0] {
                                let name = if let Ok(name_ptr) = i.GetDisplayName(SIGDN_NORMALDISPLAY) {
                                    name_ptr.to_string().unwrap_or_default()
                                } else {
                                    "Unknown".to_string()
                                };
                                
                                let mut remote_path = if let Ok(p_ptr) = i.GetDisplayName(SIGDN_DESKTOPABSOLUTEPARSING) {
                                    p_ptr.to_string().unwrap_or_default()
                                } else {
                                    "".to_string()
                                };
                                
                                let mut is_media_device = Some(false);
                                let mut has_web_page = Some(false);
                                
                                // Clean up non-UNC paths
                                if remote_path.is_empty() {
                                    remote_path = format!("\\\\{}", name);
                                } else if remote_path.starts_with("::{") || remote_path.starts_with("?") {
                                    // Found a UPnP or WSD device! Ensure we keep the full path so ShellExecute works.
                                    is_media_device = Some(true);
                                    
                                    // Check if it has an "open" verb (means it has a webpage/app for these devices)
                                    let bind_ctx: Option<&IBindCtx> = None;
                                    if let Ok(context_menu) = i.BindToHandler::<Option<&IBindCtx>, IContextMenu>(bind_ctx, &BHID_SFUIObject) {
                                        if let Ok(hmenu) = CreatePopupMenu() {
                                            if context_menu.QueryContextMenu(hmenu, 0, 1, 0x7FFF, CMF_NORMAL).is_ok() {
                                                let count = GetMenuItemCount(Some(hmenu));
                                                for idx in 0..count {
                                                    let id = GetMenuItemID(hmenu, idx);
                                                    if id > 0 {
                                                        let mut name_buf = [0u8; 128];
                                                        if context_menu.GetCommandString(
                                                            (id - 1) as usize,
                                                            GCS_VERBA as u32,
                                                            None,
                                                            PSTR::from_raw(name_buf.as_mut_ptr()),
                                                            name_buf.len() as u32
                                                        ).is_ok() {
                                                            let len = name_buf.iter().position(|&x| x == 0).unwrap_or(name_buf.len());
                                                            let verb = String::from_utf8_lossy(&name_buf[..len]);
                                                            if verb == "open" {
                                                                has_web_page = Some(true);
                                                                break;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            let _ = DestroyMenu(hmenu);
                                        }
                                    }
                                }
                                
                                // Ignore empty unreadable names
                                if !name.is_empty() {
                                    resources.push(NetResource {
                                        name,
                                        remote_path,
                                        resource_type: 1, // Disk
                                        display_type: 1, // Server
                                        usage: 2, // Container
                                        provider: Some("Windows Shell".to_string()),
                                        is_media_device,
                                        has_web_page,
                                    });
                                }
                            }
                        }
                    }
                }
                
                CoUninitialize();
                
                // Sort by name
                resources.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                return Ok(resources);
            }

            // Path is Some (a specific server like \\SERVER)
            let mut handle = windows::Win32::Foundation::HANDLE::default();
            let mut nr = NETRESOURCEW::default();
            let mut wide_path: Vec<u16>;
            
            let p = path.unwrap();
            wide_path = p.encode_utf16().chain(std::iter::once(0)).collect();
            nr.lpRemoteName = PWSTR(wide_path.as_mut_ptr());
            nr.dwUsage = RESOURCEUSAGE_CONTAINER.0;
            let nr_ptr = &nr as *const _;

            let res = WNetOpenEnumW(
                RESOURCE_GLOBALNET,
                RESOURCETYPE_ANY,
                WNET_OPEN_ENUM_USAGE(0),
                Some(nr_ptr),
                &mut handle
            );

            if res.is_ok() {
                let mut buffer = vec![0u8; 16384];
                loop {
                    let mut count = 0xFFFFFFFFu32;
                    let mut buffer_size = buffer.len() as u32;
                    
                    let enum_res = WNetEnumResourceW(
                        handle,
                        &mut count,
                        buffer.as_mut_ptr() as *mut _,
                        &mut buffer_size
                    );

                    if enum_res.is_ok() {
                        let ptr = buffer.as_ptr() as *const NETRESOURCEW;
                        for i in 0..count as usize {
                            let item = &*ptr.add(i);
                            let remote_name = if !item.lpRemoteName.is_null() {
                                item.lpRemoteName.to_string().unwrap_or_default()
                            } else {
                                String::new()
                            };

                            let name = if !item.lpComment.is_null() && !item.lpComment.to_string().unwrap_or_default().is_empty() {
                                item.lpComment.to_string().unwrap_or_default()
                            } else if !item.lpRemoteName.is_null() {
                                let rn = item.lpRemoteName.to_string().unwrap_or_default();
                                let parts: Vec<&str> = rn.split('\\').collect();
                                parts.last().unwrap_or(&"Unknown").to_string()
                            } else {
                                "Unknown".to_string()
                            };

                            resources.push(NetResource {
                                name,
                                remote_path: remote_name,
                                resource_type: item.dwType.0,
                                display_type: item.dwDisplayType,
                                usage: item.dwUsage,
                                provider: if !item.lpProvider.is_null() {
                                    Some(item.lpProvider.to_string().unwrap_or_default())
                                } else {
                                    None
                                },
                                is_media_device: None,
                                has_web_page: None,
                            });
                        }
                    } else {
                        break;
                    }
                }
                let _ = WNetCloseEnum(handle);
            } else {
                // Return empty instead of error, so interface doesn't complain endlessly if unreachable
                return Ok(resources);
            }
        }
        
        resources.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(resources)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Network discovery is only available on Windows".to_string())
    }
}

#[tauri::command]
pub async fn map_network_drive(letter: String, path: String, reconnect: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        unsafe {
            let mut nr = NETRESOURCEW::default();
            nr.dwType = RESOURCETYPE_DISK;
            
            let mut wide_remote: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

            // Make sure the letter is formatted e.g., "Z:" not just "Z"
            let local_name = if letter.len() == 1 { format!("{}:", letter) } else { letter.clone() };
            let mut wide_local: Vec<u16> = local_name.encode_utf16().chain(std::iter::once(0)).collect();

            nr.lpLocalName = PWSTR(wide_local.as_mut_ptr());
            nr.lpRemoteName = PWSTR(wide_remote.as_mut_ptr());

            let flags = if reconnect { CONNECT_UPDATE_PROFILE } else { NET_CONNECT_FLAGS(0) };

            // WNetAddConnection2W returns a WIN32_ERROR (u32 wrapped), we check if it is 0 (NO_ERROR)
            let result = WNetAddConnection2W(&nr, PCWSTR::null(), PCWSTR::null(), flags);
            if result == WIN32_ERROR(0) {
                return Ok(());
            } else {
                return Err(format!("WNetAddConnection2W failed with code {:?}", result));
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Network drive mapping is only available on Windows".to_string())
    }
}

#[tauri::command]
pub async fn disconnect_network_drive(letter: String, force: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        unsafe {
            let local_name = letter.trim_end_matches('\\').to_string();
            let wide_local: Vec<u16> = local_name.encode_utf16().chain(std::iter::once(0)).collect();

            let result = WNetCancelConnection2W(PCWSTR(wide_local.as_ptr()), CONNECT_UPDATE_PROFILE, force);
            if result == WIN32_ERROR(0) {
                return Ok(());
            } else {
                return Err(format!("WNetCancelConnection2W failed with code {:?}", result));
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Network drive disconnect is only available on Windows".to_string())
    }
}

