#![cfg(target_os = "windows")]

use super::windows_com::{take_string, ComApartment};
use crate::models::NetResource;
use log::info;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{WIN32_ERROR, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS};
use windows::Win32::NetworkManagement::WNet::{
    WNetAddConnection2W, WNetCancelConnection2W, WNetCloseEnum, WNetEnumResourceW, WNetOpenEnumW,
    CONNECT_UPDATE_PROFILE, NETRESOURCEW, NET_CONNECT_FLAGS, RESOURCETYPE_ANY, RESOURCETYPE_DISK,
    RESOURCEUSAGE_CONTAINER, RESOURCE_CONNECTED, RESOURCE_GLOBALNET, WNET_OPEN_ENUM_USAGE,
};
use windows::Win32::System::Com::IBindCtx;
use windows::Win32::UI::Shell::{
    BHID_EnumItems, FOLDERID_NetworkFolder, IEnumShellItems,
    IShellItem, SHGetKnownFolderItem, KF_FLAG_DEFAULT,
    SIGDN_DESKTOPABSOLUTEPARSING, SIGDN_NORMALDISPLAY,
};

use crate::models::FileEntry;

/// Read the actual Network-folder columns while the shell item is alive on its COM worker.
unsafe fn shell_network_details(item: &IShellItem) -> Option<crate::models::file_entry::NetworkDetails> {
    use windows::Win32::UI::Shell::{SHGetIDListFromObject, SHBindToParent, IShellFolder2, StrRetToStrW};
    use windows::Win32::UI::Shell::Common::{ITEMIDLIST, SHELLDETAILS};
    use windows::Win32::System::Com::CoTaskMemFree;
    let pidl = unsafe { SHGetIDListFromObject(item) }.ok()?;
    let result = (|| {
        let mut child: *mut ITEMIDLIST = std::ptr::null_mut();
        let folder: IShellFolder2 = unsafe { SHBindToParent(pidl, Some(&mut child)) }.ok()?;
        let get = |column| -> String {
            let mut details = SHELLDETAILS::default();
            if unsafe { folder.GetDetailsOf(child, column, &mut details) }.is_err() { return String::new(); }
            // SHELLDETAILS is packed: copy STRRET before passing its address.
            let mut text = details.str;
            let mut value = PWSTR::null();
            if unsafe { StrRetToStrW(&mut text, Some(child), &mut value) }.is_err() { return String::new(); }
            let result = unsafe { value.to_string() }.unwrap_or_default();
            unsafe { CoTaskMemFree(Some(value.as_ptr().cast())) };
            result
        };
        Some(crate::models::file_entry::NetworkDetails { category: get(1), workgroup: get(2), location: get(3) })
    })();
    unsafe { CoTaskMemFree(Some(pidl.cast())) };
    result
}

pub fn list_network_vicinity() -> Result<Vec<FileEntry>, crate::models::CommandError> {
    list_network_entries(None)
}

pub fn list_network_entries(path: Option<String>) -> Result<Vec<FileEntry>, crate::models::CommandError> {
    let shares_only = path.is_some();
    let resources = get_network_resources(path).map_err(crate::models::CommandError::Other)?;

    Ok(resources
        .into_iter()
        .filter(|r| !shares_only || r.resource_type == RESOURCETYPE_DISK.0)
        .map(|r| {
            FileEntry {
                native_kind: None,
                created: None,
                content_stamp: None,
                archive_path: None,
                network_details: r.network_details,
                name: r.name,
                path: r.remote_path,
                is_dir: !r.is_media_device.unwrap_or(false),
                is_hidden: false,
                is_system: false,
                is_symlink: false,
                is_junction: false,
                size: 0,
                modified: 0,
                is_calculated: true,
                folders_count: None,
                files_count: None,
                original_path: None,
                deleted_time: None,
            }
        })
        .collect())
}

pub fn get_network_resources(path: Option<String>) -> Result<Vec<NetResource>, String> {
    #[cfg(target_os = "windows")]
    {
        let mut resources = Vec::new();

        unsafe {
            if path.is_none() {
                let _com = ComApartment::sta_or_existing()
                    .map_err(|error| error.to_string())?;

                if let Ok(folder_item) = SHGetKnownFolderItem::<IShellItem>(
                    &FOLDERID_NetworkFolder,
                    KF_FLAG_DEFAULT,
                    None,
                ) {
                    let bind_ctx: Option<&IBindCtx> = None;
                    if let Ok(enum_items) = folder_item
                        .BindToHandler::<Option<&IBindCtx>, IEnumShellItems>(
                            bind_ctx,
                            &BHID_EnumItems,
                        )
                    {
                        let mut item: [Option<IShellItem>; 1] = [None];
                        let mut fetched = 0;

                        while enum_items.Next(&mut item, Some(&mut fetched)).is_ok() && fetched == 1
                        {
                            if let Some(i) = &item[0] {
                                let name =
                                    if let Ok(name_ptr) = i.GetDisplayName(SIGDN_NORMALDISPLAY) {
                                        take_string(name_ptr).unwrap_or_default()
                                    } else {
                                        "Unknown".to_string()
                                    };

                                let mut remote_path = if let Ok(p_ptr) =
                                    i.GetDisplayName(SIGDN_DESKTOPABSOLUTEPARSING)
                                {
                                    take_string(p_ptr).unwrap_or_default()
                                } else {
                                    "".to_string()
                                };

                                let mut is_media_device = Some(false);

                                // Clean up non-UNC paths
                                if remote_path.is_empty() {
                                    remote_path = format!("\\\\{}", name);
                                } else if remote_path.starts_with("::{")
                                    || remote_path.starts_with("?")
                                {
                                    // Found a UPnP or WSD device! Ensure we keep the full path so ShellExecute works.
                                    is_media_device = Some(true);
                                }

                                // Ignore empty unreadable names
                                if !name.is_empty() {
                                    resources.push(NetResource {
                                        network_details: shell_network_details(i),
                                        name,
                                        remote_path,
                                        resource_type: 1, // Disk
                                        is_media_device,
                                    });
                                }
                            }
                        }
                    }
                }

                // --- NEW: Discover Connected Servers (Mapped Drives / Active UNC Sessions) ---
                // This captures servers like PCS16 that are reachable but might not respond to WSD/UPnP discovery
                let mut connected_handle = windows::Win32::Foundation::HANDLE::default();
                if WNetOpenEnumW(
                    RESOURCE_CONNECTED,
                    RESOURCETYPE_ANY,
                    WNET_OPEN_ENUM_USAGE(0),
                    None,
                    &mut connected_handle,
                )
                .is_ok()
                {
                    let mut buffer = vec![0u8; 16384];
                    loop {
                        let mut count = 0xFFFFFFFFu32;
                        let mut buffer_size = buffer.len() as u32;
                        if WNetEnumResourceW(
                            connected_handle,
                            &mut count,
                            buffer.as_mut_ptr() as *mut _,
                            &mut buffer_size,
                        )
                        .is_ok()
                        {
                            let ptr = buffer.as_ptr() as *const NETRESOURCEW;
                            for i in 0..count as usize {
                                let item = &*ptr.add(i);
                                if !item.lpRemoteName.is_null() {
                                    let remote_path =
                                        item.lpRemoteName.to_string().unwrap_or_default();
                                    if let Some(stripped) = remote_path.strip_prefix("\\\\") {
                                        // Extract server name from \\SERVER\SHARE or \\SERVER
                                        let parts: Vec<&str> = stripped.split('\\').collect();
                                        if let Some(server_name) = parts.first() {
                                            let server_path = format!("\\\\{}", server_name);

                                            // Only add if not already discovered by Shell
                                            let exists = resources.iter().any(|r| {
                                                r.remote_path.to_lowercase()
                                                    == server_path.to_lowercase()
                                                    || r.name.to_lowercase()
                                                        == server_name.to_lowercase()
                                            });

                                            if !exists && !server_name.is_empty() {
                                                resources.push(NetResource {
                                                    network_details: None,
                                                    name: server_name.to_string(),
                                                    remote_path: server_path,
                                                    resource_type: 1, // Disk
                                                    is_media_device: Some(false),
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            break;
                        }
                    }
                    let _ = WNetCloseEnum(connected_handle);
                }
                // Sort by name
                resources.sort_by_key(|resource| resource.name.to_lowercase());
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
                &mut handle,
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
                        &mut buffer_size,
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

                            let name = if !item.lpRemoteName.is_null() {
                                let rn = item.lpRemoteName.to_string().unwrap_or_default();
                                let parts: Vec<&str> =
                                    rn.split('\\').filter(|s| !s.is_empty()).collect();
                                parts.last().unwrap_or(&"Unknown").to_string()
                            } else if !item.lpComment.is_null()
                                && !item.lpComment.to_string().unwrap_or_default().is_empty()
                            {
                                item.lpComment.to_string().unwrap_or_default()
                            } else {
                                "Unknown".to_string()
                            };

                            resources.push(NetResource {
                                network_details: None,
                                name,
                                remote_path: remote_name,
                                resource_type: item.dwType.0,
                                is_media_device: None,
                            });
                        }
                    } else {
                        if enum_res == ERROR_MORE_DATA {
                            buffer.resize(buffer_size as usize, 0);
                            continue;
                        }
                        if enum_res == ERROR_NO_MORE_ITEMS { break; }
                        let _ = WNetCloseEnum(handle);
                        return Err(std::io::Error::from_raw_os_error(enum_res.0 as i32).to_string());
                    }
                }
                let _ = WNetCloseEnum(handle);
            } else {
                return Err(std::io::Error::from_raw_os_error(res.0 as i32).to_string());
            }
        }

        resources.sort_by_key(|resource| resource.name.to_lowercase());
        Ok(resources)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Network discovery is only available on Windows".to_string())
    }
}

pub fn map_network_drive(
    letter: String,
    path: String,
    reconnect: bool,
    username: Option<String>,
    password: Option<String>,
) -> Result<(), String> {
    info!(
        "Mapping network drive: {} -> {} (reconnect: {}, user: {:?})",
        letter, path, reconnect, username
    );
    #[cfg(target_os = "windows")]
    {
        unsafe {
            let mut nr = NETRESOURCEW {
                dwType: RESOURCETYPE_DISK,
                ..Default::default()
            };

            // Ensure path starts with \\
            let clean_path = if !path.starts_with("\\\\") {
                format!("\\\\{}", path.trim_start_matches('\\'))
            } else {
                path.clone()
            };

            let mut wide_remote: Vec<u16> = clean_path
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            // Make sure the letter is formatted e.g., "Z:" not just "Z"
            let local_name = if letter.len() == 1 || !letter.ends_with(':') {
                format!("{}:", letter)
            } else {
                letter.clone()
            };
            let mut wide_local: Vec<u16> = local_name
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            nr.lpLocalName = PWSTR(wide_local.as_mut_ptr());
            nr.lpRemoteName = PWSTR(wide_remote.as_mut_ptr());

            // Do not silently replace an existing connection or close its open files.
            // WNetAddConnection2W reports an occupied drive letter to the caller.
            let flags = if reconnect {
                CONNECT_UPDATE_PROFILE
            } else {
                NET_CONNECT_FLAGS(0)
            };

            let wide_user: Option<Vec<u16>> =
                username.map(|u| u.encode_utf16().chain(std::iter::once(0)).collect());
            let wide_pass: Option<Vec<u16>> =
                password.map(|p| p.encode_utf16().chain(std::iter::once(0)).collect());

            let user_ptr = wide_user
                .as_ref()
                .map_or(PCWSTR::null(), |v| PCWSTR(v.as_ptr()));
            let pass_ptr = wide_pass
                .as_ref()
                .map_or(PCWSTR::null(), |v| PCWSTR(v.as_ptr()));

            // WNetAddConnection2W returns a WIN32_ERROR (u32 wrapped), we check if it is 0 (NO_ERROR)
            let result = WNetAddConnection2W(&nr, pass_ptr, user_ptr, flags);
            if result == WIN32_ERROR(0) {
                info!("Successfully mapped network drive {}", local_name);
                Ok(())
            } else {
                Err(std::io::Error::from_raw_os_error(result.0 as i32).to_string())
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Network drive mapping is only available on Windows".to_string())
    }
}

pub fn disconnect_network_drive(letter: String, force: bool) -> Result<(), super::NetworkDisconnectError> {
    #[cfg(target_os = "windows")]
    {
        unsafe {
            let local_name = letter.trim_end_matches(['\\', '/']).to_string();
            let wide_local: Vec<u16> = local_name
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            let result =
                WNetCancelConnection2W(PCWSTR(wide_local.as_ptr()), CONNECT_UPDATE_PROFILE, force);
            if result == WIN32_ERROR(0) {
                Ok(())
            } else if result == windows::Win32::Foundation::ERROR_OPEN_FILES
                || result == windows::Win32::Foundation::ERROR_DEVICE_IN_USE {
                Err(super::NetworkDisconnectError::Busy)
            } else {
                Err(super::NetworkDisconnectError::Other(std::io::Error::from_raw_os_error(result.0 as i32).to_string()))
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(super::NetworkDisconnectError::Other("Network drive disconnect is only available on Windows".to_string()))
    }
}
