#[cfg(target_os = "windows")]
fn execute_shell_verb_by_canonical_name(_app: &AppHandle, path: &str, target_verbs: &[&str]) -> Result<(), CommandError> {
    use windows::core::{PCWSTR, PCSTR};
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
                // 1. Canonical verb
                let mut verb_buf = [0u8; 128];
                if context_menu.GetCommandString((id - 1) as usize, GCS_VERBA, None, PSTR(verb_buf.as_mut_ptr()), verb_buf.len() as u32).is_ok() {
                    let verb = std::ffi::CStr::from_ptr(verb_buf.as_ptr() as *const i8).to_string_lossy().to_lowercase();
                    if target_verbs.iter().any(|&v| verb == v) {
                        target_id = Some(id);
                        break;
                    }
                }

                // 2. Localized label
                let mut label_buf = [0u16; 256];
                let len = GetMenuStringW(hmenu, i as u32, Some(&mut label_buf), MF_BYPOSITION);
                if len > 0 {
                    let label = String::from_utf16_lossy(&label_buf[..len as usize]).to_lowercase();
                    let clean_label = label.replace("&", "").replace("'", "").replace("’", "");
                    
                    if is_unpin {
                        if (clean_label.contains("desepingler") && clean_label.contains("acces rapide")) ||
                           (clean_label.contains("unpin") && clean_label.contains("quick access")) ||
                           (clean_label.contains("unpin") && clean_label.contains("home")) ||
                           (clean_label.contains("retirer") && clean_label.contains("acces rapide")) ||
                           (clean_label.contains("desepingler") && clean_label.contains("accueil"))
                        {
                            target_id = Some(id);
                            break;
                        }
                    } else {
                        if (clean_label.contains("epingler") && clean_label.contains("acces rapide")) ||
                           (clean_label.contains("pin") && clean_label.contains("quick access")) ||
                           (clean_label.contains("pin") && clean_label.contains("home")) ||
                           (clean_label.contains("epingler") && clean_label.contains("accueil"))
                        {
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
                nShow: SW_SHOWNORMAL.0 as i32,
                ..Default::default()
            };
            context_menu.InvokeCommand(&ici).map_err(|e| CommandError::SystemError(format!("InvokeCommand by ID failed: {}", e)))
        } else {
            // Try strings as last resort
            let mut last_res = Err(CommandError::SystemError(format!("Could not find suitable shell verb for: {:?}", target_verbs)));
            for &verb in target_verbs {
                let v_string = format!("{}\0", verb);
                let ici = CMINVOKECOMMANDINFO {
                    cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
                    fMask: 0,
                    hwnd: HWND(std::ptr::null_mut()),
                    lpVerb: PCSTR(v_string.as_ptr()),
                    nShow: SW_SHOWNORMAL.0 as i32,
                    ..Default::default()
                };
                if context_menu.InvokeCommand(&ici).is_ok() {
                    last_res = Ok(());
                    break;
                }
            }
            last_res
        };

        let _ = DestroyMenu(hmenu);
        windows::Win32::UI::Shell::ILFree(Some(pidl_full as *const _));
        CoUninitialize();

        result
    }
}
