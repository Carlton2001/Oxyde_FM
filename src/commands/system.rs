use crate::models::{
    CommandError, DriveInfo, QuickAccessItem, SessionManager, SnapRect, WinMenuItem,
};
use crate::utils::path_security::validate_path;
use crate::WindowState;
use log::info;

// app_ready removed

pub fn get_drives(skip_hardware_info: Option<bool>) -> Vec<DriveInfo> {
    crate::platform::platform().list_drives(skip_hardware_info.unwrap_or(false))
}

pub fn get_accent_color() -> String {
    crate::platform::platform().get_accent_color()
}

pub async fn open_item(path: String) -> Result<(), CommandError> {
    let normalized_path = if path.starts_with("::{") || path.starts_with("?") {
        path
    } else {
        validate_path(&path)?.to_string_lossy().into_owned()
    };
    info!("Opening item: {}", normalized_path);
    crate::platform::platform()
        .open_item(&normalized_path)
        .map_err(CommandError::SystemError)
}

// set_webview_background removed

// show_native_context_menu removed or needs refactoring for GPUI
#[cfg(target_os = "windows")]
struct ComApartmentGuard;

#[cfg(target_os = "windows")]
impl ComApartmentGuard {
    fn initialize_sta() -> Result<Self, CommandError> {
        let result = unsafe {
            windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
            )
        };
        if result.is_err() {
            return Err(CommandError::SystemError(format!(
                "COM initialization failed: 0x{:08x}",
                result.0
            )));
        }
        Ok(Self)
    }
}

#[cfg(target_os = "windows")]
impl Drop for ComApartmentGuard {
    fn drop(&mut self) {
        unsafe { windows::Win32::System::Com::CoUninitialize() };
    }
}

pub fn get_native_context_menu_items(
    path: String,
    is_background: bool,
) -> Result<Vec<WinMenuItem>, CommandError> {
    #[cfg(target_os = "windows")]
    {
        use windows::core::{Interface, PCWSTR, PSTR};
        use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
        use windows::Win32::UI::Shell::{
            IContextMenu, IContextMenu2, IContextMenu3, IShellFolder, SHBindToParent,
            SHParseDisplayName, CMF_CANRENAME, CMF_EXPLORE, CMF_NORMAL, GCS_VERBA,
        };
        use windows::Win32::UI::WindowsAndMessaging::{
            CreatePopupMenu, DestroyMenu, GetMenuItemCount, GetMenuItemID, GetMenuStringW,
            GetSubMenu, MF_BYPOSITION, WM_INITMENUPOPUP,
        };

        let pb = validate_path(&path)?;
        let path_norm = pb.to_string_lossy().replace("/", "\\");
        let path_u16: Vec<u16> = path_norm.encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            let _com = ComApartmentGuard::initialize_sta()?;

            let mut pidl_full = std::ptr::null_mut();
            SHParseDisplayName(PCWSTR(path_u16.as_ptr()), None, &mut pidl_full, 0, None).map_err(
                |e| CommandError::SystemError(format!("SHParseDisplayName failed: {}", e)),
            )?;

            let mut pidl_relative = std::ptr::null_mut();
            let parent_folder: IShellFolder = SHBindToParent(pidl_full, Some(&mut pidl_relative))
                .map_err(|e| {
                windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                CommandError::SystemError(format!("SHBindToParent failed: {:?}", e))
            })?;

            let context_menu: IContextMenu = if is_background {
                // For background menu, we need the IShellFolder of the target itself
                let target_folder: IShellFolder = parent_folder
                    .BindToObject(pidl_relative, None)
                    .map_err(|e| {
                    windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                    CommandError::SystemError(format!("BindToObject failed: {:?}", e))
                })?;

                // Use CreateViewObject to get the background context menu
                target_folder
                    .CreateViewObject(HWND::default())
                    .map_err(|e| {
                        windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                        CommandError::SystemError(format!("CreateViewObject failed: {}", e))
                    })?
            } else {
                let pidl_relative_slice = [pidl_relative as *const _];
                parent_folder
                    .GetUIObjectOf(HWND::default(), &pidl_relative_slice, None)
                    .map_err(|e| {
                        windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                        CommandError::SystemError(format!("GetUIObjectOf failed: {}", e))
                    })?
            };

            let hmenu = CreatePopupMenu().map_err(|e| {
                windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                CommandError::SystemError(e.to_string())
            })?;
            let _ = context_menu.QueryContextMenu(
                hmenu,
                0,
                1,
                0x7FFF,
                CMF_NORMAL | CMF_EXPLORE | CMF_CANRENAME,
            );

            let cm2: Option<IContextMenu2> = context_menu.cast().ok();
            let cm3: Option<IContextMenu3> = context_menu.cast().ok();

            fn scrape_level(
                hmenu: windows::Win32::UI::WindowsAndMessaging::HMENU,
                context_menu: &IContextMenu,
                cm2: Option<&IContextMenu2>,
                cm3: Option<&IContextMenu3>,
            ) -> Vec<WinMenuItem> {
                unsafe {
                    let count = GetMenuItemCount(Some(hmenu));
                    if count < 0 {
                        return Vec::new();
                    }

                    let mut items = Vec::new();
                    let forbidden_verbs = [
                        "cut",
                        "copy",
                        "paste",
                        "delete",
                        "rename",
                        "properties",
                        "link",
                        "shortcut",
                        "open",
                    ];

                    for i in 0..count {
                        let id = GetMenuItemID(hmenu, i);
                        let submenu = GetSubMenu(hmenu, i);

                        // If it has a submenu, we MUST initialize it for extensions like NanaZip
                        if !submenu.is_invalid() {
                            let wparam = WPARAM(submenu.0 as usize);
                            let lparam = LPARAM((i & 0xFFFF) as isize); // loword: index, hiword: fSystemMenu
                            if let Some(cm) = cm2 {
                                let _ = cm.HandleMenuMsg(WM_INITMENUPOPUP, wparam, lparam);
                            } else if let Some(cm) = cm3 {
                                let _ = cm.HandleMenuMsg(WM_INITMENUPOPUP, wparam, lparam);
                            }
                        }

                        let mut label_buf = [0u16; 256];
                        let len =
                            GetMenuStringW(hmenu, i as u32, Some(&mut label_buf), MF_BYPOSITION);
                        let mut label = String::from_utf16_lossy(&label_buf[..len as usize]);

                        // Clean up & in labels (e.g. "&Open" -> "Open")
                        label = label.replace("&", "");

                        if label.is_empty() || label == "-" || label.is_empty() {
                            continue;
                        }

                        let is_new_menu = label.to_lowercase().contains("nouveau")
                            || label.to_lowercase() == "new";

                        // Filter standard verbs
                        if (1..=0x7FFF).contains(&id) && !is_new_menu {
                            let mut verb_buf = [0u8; 128];
                            if context_menu
                                .GetCommandString(
                                    (id - 1) as usize,
                                    GCS_VERBA,
                                    None,
                                    PSTR(verb_buf.as_mut_ptr()),
                                    verb_buf.len() as u32,
                                )
                                .is_ok()
                            {
                                let verb = std::ffi::CStr::from_ptr(verb_buf.as_ptr() as *const i8)
                                    .to_string_lossy()
                                    .to_lowercase();
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
                        let mut item_verb: Option<String> = is_new_menu.then(|| "new".to_string());
                        if (1..=0x7FFF).contains(&id) {
                            let mut verb_buf = [0u8; 128];
                            if context_menu
                                .GetCommandString(
                                    (id - 1) as usize,
                                    GCS_VERBA,
                                    None,
                                    PSTR(verb_buf.as_mut_ptr()),
                                    verb_buf.len() as u32,
                                )
                                .is_ok()
                            {
                                let v = std::ffi::CStr::from_ptr(verb_buf.as_ptr() as *const i8)
                                    .to_string_lossy()
                                    .to_string();
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
            Ok(items)
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(Vec::new())
    }
}

pub fn execute_native_menu_item(
    path: String,
    id: i32,
    is_background: bool,
) -> Result<(), CommandError> {
    #[cfg(target_os = "windows")]
    {
        use windows::core::{Interface, PCSTR, PCWSTR};
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::Shell::{
            IContextMenu, IContextMenu2, IContextMenu3, IShellFolder, SHBindToParent,
            SHParseDisplayName, CMF_CANRENAME, CMF_EXPLORE, CMF_NORMAL, CMINVOKECOMMANDINFO,
        };
        use windows::Win32::UI::WindowsAndMessaging::{
            CreatePopupMenu, DestroyMenu, SW_SHOWNORMAL,
        };

        let pb = validate_path(&path)?;
        let path_norm = pb.to_string_lossy().replace("/", "\\");
        let path_u16: Vec<u16> = path_norm.encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            let _com = ComApartmentGuard::initialize_sta()?;

            let main_hwnd = HWND::default(); // needs proper HWND for full functionality

            let mut pidl_full = std::ptr::null_mut();
            SHParseDisplayName(PCWSTR(path_u16.as_ptr()), None, &mut pidl_full, 0, None).map_err(
                |e| CommandError::SystemError(format!("SHParseDisplayName failed: {}", e)),
            )?;

            let mut pidl_relative = std::ptr::null_mut();
            let parent_folder: IShellFolder = SHBindToParent(pidl_full, Some(&mut pidl_relative))
                .map_err(|e| {
                windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                CommandError::SystemError(format!("SHBindToParent failed: {:?}", e))
            })?;

            let context_menu: IContextMenu = if is_background {
                let target_folder: IShellFolder = parent_folder
                    .BindToObject(pidl_relative, None)
                    .map_err(|e| {
                    windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                    CommandError::SystemError(format!("BindToObject failed: {:?}", e))
                })?;

                target_folder.CreateViewObject(main_hwnd).map_err(|e| {
                    windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                    CommandError::SystemError(format!("CreateViewObject failed: {}", e))
                })?
            } else {
                let pidl_relative_slice = [pidl_relative as *const _];
                parent_folder
                    .GetUIObjectOf(main_hwnd, &pidl_relative_slice, None)
                    .map_err(|e| {
                        windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                        CommandError::SystemError(format!("GetUIObjectOf failed: {}", e))
                    })?
            };

            // We MUST call QueryContextMenu before InvokeCommand so the extension knows what IDs we are talking about
            let hmenu = CreatePopupMenu().map_err(|e| {
                windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                CommandError::SystemError(e.to_string())
            })?;
            let _ = context_menu.QueryContextMenu(
                hmenu,
                0,
                1,
                0x7FFF,
                CMF_NORMAL | CMF_EXPLORE | CMF_CANRENAME,
            );

            // CRITICAL: For submenus like "New", we must initialize them so the extension populates the IDs
            let cm2: Option<IContextMenu2> = context_menu.cast().ok();
            let cm3: Option<IContextMenu3> = context_menu.cast().ok();

            fn init_all_submenus(
                hmenu: windows::Win32::UI::WindowsAndMessaging::HMENU,
                cm2: Option<&IContextMenu2>,
                cm3: Option<&IContextMenu3>,
            ) {
                unsafe {
                    use windows::Win32::Foundation::{LPARAM, WPARAM};
                    use windows::Win32::UI::WindowsAndMessaging::{
                        GetMenuItemCount, GetSubMenu, WM_INITMENUPOPUP,
                    };

                    let count = GetMenuItemCount(Some(hmenu));
                    for i in 0..count {
                        let submenu = GetSubMenu(hmenu, i);
                        if !submenu.is_invalid() {
                            let wparam = WPARAM(submenu.0 as usize);
                            let lparam = LPARAM((i & 0xFFFF) as isize);
                            if let Some(cm) = cm2 {
                                let _ = cm.HandleMenuMsg(WM_INITMENUPOPUP, wparam, lparam);
                            } else if let Some(cm) = cm3 {
                                let _ = cm.HandleMenuMsg(WM_INITMENUPOPUP, wparam, lparam);
                            }

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
        }
    }
    Ok(())
}
pub async fn get_mounted_images() -> Result<Vec<String>, CommandError> {
    tokio::task::spawn_blocking(|| {
        crate::platform::platform()
            .mounted_disk_images()
            .map(|images| images.into_iter().map(|image| image.image_path).collect())
            .map_err(CommandError::SystemError)
    })
    .await
    .map_err(|error| CommandError::SystemError(error.to_string()))?
}

pub async fn mount_disk_image(path: String) -> Result<(), CommandError> {
    tokio::task::spawn_blocking(move || {
        crate::platform::platform()
            .mount_disk_image(&path)
            .map(|_| ())
            .map_err(CommandError::SystemError)
    })
    .await
    .map_err(|error| CommandError::SystemError(error.to_string()))?
}

pub async fn unmount_disk_image(
    session_manager: &SessionManager,
    path: String,
) -> Result<(), CommandError> {
    let images = tokio::task::spawn_blocking(|| crate::platform::platform().mounted_disk_images())
        .await
        .map_err(|error| CommandError::SystemError(error.to_string()))?
        .map_err(CommandError::SystemError)?;
    let image = images
        .into_iter()
        .find(|image| {
            image.image_path.eq_ignore_ascii_case(&path)
                || image.roots.iter().any(|root| {
                    root.trim_end_matches(['/', '\\'])
                        .eq_ignore_ascii_case(path.trim_end_matches(['/', '\\']))
                })
        })
        .ok_or_else(|| CommandError::SystemError("Image is not mounted".into()))?;
    {
        let mut session = session_manager
            .0
            .lock()
            .map_err(|error| CommandError::SystemError(error.to_string()))?;
        let clean_panel = |panel: &mut crate::models::session::PanelState| {
            let affected = |path: &std::path::Path| {
                image.roots.iter().any(|root| {
                    path.to_string_lossy()
                        .to_lowercase()
                        .starts_with(&root.to_lowercase())
                })
            };
            panel.tabs.retain(|tab| !affected(&tab.path));
            if panel.tabs.is_empty() {
                let id = uuid::Uuid::new_v4().to_string();
                panel.tabs.push(crate::models::Tab {
                    id: id.clone(),
                    path: dirs::home_dir().unwrap_or_default(),
                    back_stack: Vec::new(),
                    forward_stack: Vec::new(),
                    version: 0,
                });
                panel.active_tab_id = id;
            } else if !panel.tabs.iter().any(|tab| tab.id == panel.active_tab_id) {
                panel.active_tab_id = panel.tabs[0].id.clone();
            }
            panel.update_watcher();
        };
        clean_panel(&mut session.left_panel);
        clean_panel(&mut session.right_panel);
    }
    tokio::task::spawn_blocking(move || {
        crate::platform::platform()
            .unmount_disk_image(&image.image_path)
            .map_err(CommandError::SystemError)
    })
    .await
    .map_err(|error| CommandError::SystemError(error.to_string()))?
}

pub fn oxide_sync_snap_rect(state: &WindowState, rect: SnapRect) {
    let mut m = state.maximize_button_rect.lock().unwrap();
    *m = Some(rect);
}

pub fn get_quick_access_items() -> Result<Vec<QuickAccessItem>, CommandError> {
    #[cfg(target_os = "windows")]
    {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::HWND;
        use windows::Win32::System::Com::{
            CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED,
        };
        use windows::Win32::UI::Shell::Common::ITEMIDLIST;
        use windows::Win32::UI::Shell::{
            IEnumIDList, ILCombine, ILFree, IShellFolder, IShellItem, SHBindToObject,
            SHCreateItemFromIDList, SHParseDisplayName, SHCONTF_FOLDERS, SIGDN_FILESYSPATH,
            SIGDN_NORMALDISPLAY,
        };

        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            // Quick Access virtual folder CLSID
            let qa_path = "shell:::{679f85cb-0220-4080-b29b-5540cc05aab6}";
            let qa_u16: Vec<u16> = qa_path.encode_utf16().chain(std::iter::once(0)).collect();

            let mut pidl_qa: *mut ITEMIDLIST = std::ptr::null_mut();
            if SHParseDisplayName(PCWSTR(qa_u16.as_ptr()), None, &mut pidl_qa, 0, None).is_err()
                || pidl_qa.is_null()
            {
                CoUninitialize();
                return Ok(Vec::new());
            }

            let folder: IShellFolder = match SHBindToObject(None, pidl_qa as *const _, None) {
                Ok(f) => f,
                Err(_) => {
                    ILFree(Some(pidl_qa as *const _));
                    CoUninitialize();
                    return Ok(Vec::new());
                }
            };

            // EnumObjects returns HRESULT and writes the enumerator to an out-param
            let mut enum_opt: Option<IEnumIDList> = None;
            let hr = folder.EnumObjects(
                HWND(std::ptr::null_mut()),
                SHCONTF_FOLDERS.0 as u32,
                &mut enum_opt,
            );
            let enum_obj = match (hr.is_ok(), enum_opt) {
                (true, Some(e)) => e,
                _ => {
                    ILFree(Some(pidl_qa as *const _));
                    CoUninitialize();
                    return Ok(Vec::new());
                }
            };

            let mut items = Vec::new();
            loop {
                // Next takes a slice as out-param; fetched tells us how many were filled
                let mut pidl_arr: [*mut ITEMIDLIST; 1] = [std::ptr::null_mut()];
                let mut fetched: u32 = 0;
                let _ = enum_obj.Next(&mut pidl_arr, Some(&mut fetched));
                if fetched == 0 || pidl_arr[0].is_null() {
                    break;
                }
                let pidl_child = pidl_arr[0];

                // Combine parent PIDL + child PIDL to form an absolute PIDL,
                // then resolve it to a real filesystem path via IShellItem.
                let pidl_abs = ILCombine(Some(pidl_qa as *const _), Some(pidl_child as *const _));
                if !pidl_abs.is_null() {
                    if let Ok(si) = SHCreateItemFromIDList::<IShellItem>(pidl_abs as *const _) {
                        let path_opt = si.GetDisplayName(SIGDN_FILESYSPATH).ok().and_then(|pw| {
                            let s = pw.to_string().ok();
                            CoTaskMemFree(Some(pw.0 as *const core::ffi::c_void));
                            s
                        });

                        if let Some(path) = path_opt {
                            if !path.is_empty() && !path.starts_with("::{") {
                                let name = si
                                    .GetDisplayName(SIGDN_NORMALDISPLAY)
                                    .ok()
                                    .and_then(|pw| {
                                        let s = pw.to_string().ok();
                                        CoTaskMemFree(Some(pw.0 as *const core::ffi::c_void));
                                        s
                                    })
                                    .unwrap_or_else(|| {
                                        std::path::Path::new(&path)
                                            .file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or(&path)
                                            .to_string()
                                    });
                                items.push(QuickAccessItem { name, path });
                            }
                        }
                    }
                    ILFree(Some(pidl_abs as *const _));
                }

                ILFree(Some(pidl_child as *const _));
            }

            ILFree(Some(pidl_qa as *const _));
            CoUninitialize();
            Ok(items)
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(crate::platform::platform().get_quick_access_items())
    }
}

pub fn add_to_quick_access(path: String) -> Result<(), CommandError> {
    #[cfg(target_os = "windows")]
    {
        execute_shell_verb_by_canonical_name(&path, &["pintohome", "pintofavorites"])
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(CommandError::SystemError(
            "Quick access is only supported on Windows".to_string(),
        ))
    }
}

pub fn remove_from_quick_access(path: String) -> Result<(), CommandError> {
    #[cfg(target_os = "windows")]
    {
        execute_shell_verb_by_canonical_name(&path, &["unpinfromhome", "unpinfromquickaccess"])
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(CommandError::SystemError(
            "Quick access is only supported on Windows".to_string(),
        ))
    }
}

#[cfg(target_os = "windows")]
fn execute_shell_verb_by_canonical_name(
    path: &str,
    target_verbs: &[&str],
) -> Result<(), CommandError> {
    use windows::core::{PCSTR, PCWSTR, PSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::{
        IContextMenu, IShellFolder, SHBindToParent, SHParseDisplayName, CMF_NORMAL,
        CMINVOKECOMMANDINFO, GCS_VERBA,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreatePopupMenu, DestroyMenu, GetMenuItemCount, GetMenuItemID, GetMenuStringW,
        MF_BYPOSITION, SW_SHOWNORMAL,
    };

    let pb = validate_path(path)?;
    let mut path_norm = pb.to_string_lossy().replace("/", "\\");
    if path_norm.len() == 2 && path_norm.ends_with(':') {
        path_norm.push('\\');
    }
    let path_u16: Vec<u16> = path_norm.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let is_unpin = target_verbs.iter().any(|v| v.contains("unpin"));
        let mut pidl_full = std::ptr::null_mut();
        if let Err(e) = SHParseDisplayName(PCWSTR(path_u16.as_ptr()), None, &mut pidl_full, 0, None)
        {
            if is_unpin {
                // FALLBACK for missing paths (e.g. disconnected network drives)
                use std::os::windows::process::CommandExt;
                let p_safe = path_norm.replace("'", "''");
                let script = format!(
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
                );
                let _ = std::process::Command::new("powershell")
                    .arg("-NoProfile")
                    .arg("-Command")
                    .arg(script)
                    .creation_flags(0x08000000)
                    .output();
                return Ok(());
            }
            return Err(CommandError::SystemError(format!(
                "SHParseDisplayName failed: {}",
                e
            )));
        }

        let mut pidl_relative = std::ptr::null_mut();
        let parent_folder: IShellFolder = SHBindToParent(pidl_full, Some(&mut pidl_relative))
            .map_err(|e| {
                windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                CommandError::SystemError(format!("SHBindToParent failed: {:?}", e))
            })?;

        let pidl_relative_slice = [pidl_relative as *const _];
        let context_menu: IContextMenu = parent_folder
            .GetUIObjectOf(HWND(std::ptr::null_mut()), &pidl_relative_slice, None)
            .map_err(|e| {
                windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
                CommandError::SystemError(format!("GetUIObjectOf failed: {}", e))
            })?;

        let hmenu = CreatePopupMenu().map_err(|e| CommandError::SystemError(e.to_string()))?;
        let _ = context_menu.QueryContextMenu(hmenu, 0, 1, 0x7FFF, CMF_NORMAL);

        let count = GetMenuItemCount(Some(hmenu));
        let mut target_id: Option<u32> = None;

        for i in 0..count {
            let id = GetMenuItemID(hmenu, i);
            if id != u32::MAX && id > 0 {
                // 1. Try canonical verb lookup first
                let mut verb_buf = [0u8; 128];
                if context_menu
                    .GetCommandString(
                        (id - 1) as usize,
                        GCS_VERBA,
                        None,
                        PSTR(verb_buf.as_mut_ptr()),
                        verb_buf.len() as u32,
                    )
                    .is_ok()
                {
                    let verb = std::ffi::CStr::from_ptr(verb_buf.as_ptr() as *const i8)
                        .to_string_lossy()
                        .to_lowercase();
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
                        let has_unpin_core = clean.contains("desepingl")
                            || clean.contains("unpin")
                            || clean.contains("retirer")
                            || clean.contains("detacher")
                            || clean.contains("lösen")
                            || clean.contains("epingl"); // some systems use "épingler" for toggle
                        let has_target_core = clean.contains("acces")
                            || clean.contains("accueil")
                            || clean.contains("favori")
                            || clean.contains("quick")
                            || clean.contains("home")
                            || clean.contains("schnell");

                        if has_unpin_core && has_target_core {
                            target_id = Some(id);
                            break;
                        }
                    } else {
                        // Match "Épingler", "Pin", "Attacher" AND ("Accès", "Accueil", "Favori", "Quick", "Home")
                        let has_pin_core = clean.contains("epingl")
                            || clean.contains("pin")
                            || clean.contains("attach")
                            || clean.contains("anheft");
                        let has_target_core = clean.contains("acces")
                            || clean.contains("accueil")
                            || clean.contains("favori")
                            || clean.contains("quick")
                            || clean.contains("home")
                            || clean.contains("schnell");

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
            context_menu
                .InvokeCommand(&ici)
                .map_err(|e| CommandError::SystemError(format!("InvokeCommand failed: {}", e)))
        } else {
            Err(CommandError::SystemError(
                "No matching verb found".to_string(),
            ))
        };

        // 3. ULTIMATE RECOURSE: PowerShell Script
        if result.is_err() {
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

            let output = std::process::Command::new("powershell")
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

pub fn clear_app_cache() -> Result<(), CommandError> {
    // 1. Clear Icon RAM Cache
    super::icons::purge_icon_cache();

    // 2. Clear app_cache_dir (Icons, etc.)
    if let Some(mut path) = dirs::cache_dir() {
        path.push("oxyde");
        if path.exists() {
            let _ = std::fs::remove_dir_all(&path);
            let _ = std::fs::create_dir_all(&path);
        }
    }

    // 3. Clear local data (thumbnails, thumbnails.db, etc.)
    if let Some(mut path) = dirs::data_local_dir() {
        path.push("oxyde");
        if path.exists() {
            if let Ok(entries) = std::fs::read_dir(&path) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let entry_path = entry.path();
                    let file_name = entry_path.file_name().unwrap_or_default().to_string_lossy();

                    if file_name.to_lowercase().contains("ebwebview")
                        || file_name.to_lowercase() == "webview2"
                    {
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

pub fn restart_app() {
    if let Ok(current_exe) = std::env::current_exe() {
        let _ = std::process::Command::new(current_exe).spawn();
        std::process::exit(0);
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Default)]
pub struct PeekStatus {
    pub installed: bool,
    pub enabled: bool,
    pub space_enabled: bool,
    pub activation_shortcut: Option<String>,
}

pub async fn get_peek_status() -> Result<PeekStatus, String> {
    let mut status = PeekStatus::default();

    #[cfg(target_os = "windows")]
    {
        use std::path::Path;
        let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let program_files = std::env::var("ProgramFiles").unwrap_or_default();

        let exe_paths = [
            format!(
                "{}\\PowerToys\\WinUI3Apps\\PowerToys.Peek.UI.exe",
                local_app_data
            ),
            format!(
                "{}\\Microsoft\\PowerToys\\PowerToys.Peek.UI.exe",
                local_app_data
            ),
            format!(
                "{}\\PowerToys\\WinUI3Apps\\PowerToys.Peek.UI.exe",
                program_files
            ),
            format!("{}\\PowerToys\\PowerToys.Peek.UI.exe", program_files),
        ];

        status.installed = exe_paths.iter().any(|p| Path::new(p).exists());
        if !status.installed {
            return Ok(status);
        }

        // Check if enabled in general settings
        let general_settings_path =
            format!("{}\\Microsoft\\PowerToys\\settings.json", local_app_data);
        if let Ok(content) = std::fs::read_to_string(general_settings_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(enabled) = json
                    .get("enabled")
                    .and_then(|e| e.get("Peek"))
                    .and_then(|p| p.as_bool())
                {
                    status.enabled = enabled;
                }
            }
        }

        // Check Peek specific settings
        let peek_settings_path = format!(
            "{}\\Microsoft\\PowerToys\\Peek\\settings.json",
            local_app_data
        );
        if let Ok(content) = std::fs::read_to_string(peek_settings_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(props) = json.get("properties") {
                    status.space_enabled = props
                        .get("EnableSpaceToActivate")
                        .and_then(|v| v.get("value"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    if let Some(shortcut) = props.get("ActivationShortcut") {
                        let mut parts = Vec::new();
                        if shortcut
                            .get("ctrl")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        {
                            parts.push("Ctrl");
                        }
                        if shortcut
                            .get("shift")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        {
                            parts.push("Shift");
                        }
                        if shortcut
                            .get("alt")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        {
                            parts.push("Alt");
                        }
                        if shortcut
                            .get("win")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        {
                            parts.push("Win");
                        }

                        let code = shortcut.get("code").and_then(|v| v.as_u64()).unwrap_or(0);
                        let key_str = match code {
                            32 => Some("Space"),
                            48 => Some("0"),
                            49 => Some("1"),
                            50 => Some("2"),
                            51 => Some("3"),
                            52 => Some("4"),
                            53 => Some("5"),
                            54 => Some("6"),
                            55 => Some("7"),
                            56 => Some("8"),
                            57 => Some("9"),
                            65 => Some("A"),
                            66 => Some("B"),
                            67 => Some("C"),
                            68 => Some("D"),
                            69 => Some("E"),
                            70 => Some("F"),
                            71 => Some("G"),
                            72 => Some("H"),
                            73 => Some("I"),
                            74 => Some("J"),
                            75 => Some("K"),
                            76 => Some("L"),
                            77 => Some("M"),
                            78 => Some("N"),
                            79 => Some("O"),
                            80 => Some("P"),
                            81 => Some("Q"),
                            82 => Some("R"),
                            83 => Some("S"),
                            84 => Some("T"),
                            85 => Some("U"),
                            86 => Some("V"),
                            87 => Some("W"),
                            88 => Some("X"),
                            89 => Some("Y"),
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

pub async fn open_peek(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let program_files = std::env::var("ProgramFiles").unwrap_or_default();

        let paths = vec![
            format!(
                "{}\\PowerToys\\WinUI3Apps\\PowerToys.Peek.UI.exe",
                local_app_data
            ),
            format!(
                "{}\\Microsoft\\PowerToys\\PowerToys.Peek.UI.exe",
                local_app_data
            ),
            format!(
                "{}\\PowerToys\\WinUI3Apps\\PowerToys.Peek.UI.exe",
                program_files
            ),
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
            let mut cmd = std::process::Command::new(exe);
            cmd.arg(&path);

            // Try to set the CWD to the parent of the file to help Peek discover siblings
            if let Some(parent) = std::path::Path::new(&path).parent() {
                cmd.current_dir(parent);
            }

            cmd.spawn().map_err(|e| e.to_string())?;
            Ok(())
        } else {
            Err("PowerToys Peek not found".to_string())
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Peek is only available on Windows".to_string())
    }
}
