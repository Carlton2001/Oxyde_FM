struct PopupMenuGuard(windows::Win32::UI::WindowsAndMessaging::HMENU);
impl Drop for PopupMenuGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyMenu(self.0);
        }
    }
}

use super::windows_com::{take_string, ComApartment, TaskMemory};
use crate::models::{CommandError, QuickAccessItem, WinMenuItem};
use crate::utils::path_security::validate_path;

fn read_verb_with(read: impl FnOnce(&mut [u8]) -> windows::core::Result<()>) -> Option<String> {
    let mut buffer = [0u8; 128];
    read(&mut buffer).ok()?;
    let verb = std::ffi::CStr::from_bytes_until_nul(&buffer)
        .ok()?
        .to_string_lossy();
    (!verb.is_empty()).then(|| verb.into_owned())
}

fn canonical_verb(
    context_menu: &windows::Win32::UI::Shell::IContextMenu,
    id: u32,
) -> Option<String> {
    if !(1..=0x7FFF).contains(&id) {
        return None;
    }
    read_verb_with(|buffer| unsafe {
        context_menu.GetCommandString(
            (id - 1) as usize,
            windows::Win32::UI::Shell::GCS_VERBA,
            None,
            windows::core::PSTR(buffer.as_mut_ptr()),
            buffer.len() as u32,
        )
    })
}

fn localized_pin_label(label: &str, is_unpin: bool) -> bool {
    let clean: String = crate::utils::remove_accents(&label.to_lowercase())
        .chars()
        .filter(|character| !matches!(character, '&' | '\'' | '’'))
        .collect();
    let unpin = ["desepingl", "unpin", "retirer", "detacher", "losen"]
        .iter()
        .any(|word| clean.contains(word));
    let pin = ["epingl", "pin", "attach", "anheft"]
        .iter()
        .any(|word| clean.contains(word));
    let target = ["acces", "accueil", "favori", "quick", "home", "schnell"]
        .iter()
        .any(|word| clean.contains(word));
    target && if is_unpin { unpin } else { pin && !unpin }
}

fn query_menu_with(
    query: impl FnOnce(windows::Win32::UI::WindowsAndMessaging::HMENU) -> windows::core::HRESULT,
) -> Result<PopupMenuGuard, CommandError> {
    let menu = PopupMenuGuard(unsafe {
        windows::Win32::UI::WindowsAndMessaging::CreatePopupMenu()
            .map_err(|error| CommandError::SystemError(error.to_string()))?
    });
    query(menu.0)
        .ok()
        .map_err(|error| CommandError::SystemError(format!("QueryContextMenu failed: {error}")))?;
    Ok(menu)
}

fn query_menu(
    context_menu: &windows::Win32::UI::Shell::IContextMenu,
    flags: u32,
) -> Result<PopupMenuGuard, CommandError> {
    query_menu_with(|menu| unsafe { context_menu.QueryContextMenu(menu, 0, 1, 0x7FFF, flags) })
}

fn invoke_menu_with(
    id: u32,
    hwnd: windows::Win32::Foundation::HWND,
    invoke: impl FnOnce(&windows::Win32::UI::Shell::CMINVOKECOMMANDINFO) -> windows::core::Result<()>,
) -> Result<(), CommandError> {
    if !(1..=0x7FFF).contains(&id) {
        return Err(CommandError::SystemError("Invalid Shell command ID".into()));
    }
    let info = windows::Win32::UI::Shell::CMINVOKECOMMANDINFO {
        cbSize: std::mem::size_of::<windows::Win32::UI::Shell::CMINVOKECOMMANDINFO>() as u32,
        hwnd,
        lpVerb: windows::core::PCSTR((id - 1) as *const u8),
        nShow: windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL.0,
        ..Default::default()
    };
    invoke(&info)
        .map_err(|error| CommandError::SystemError(format!("InvokeCommand failed: {error}")))
}

fn run_powershell(script: &str, path: &str) -> Result<(), CommandError> {
    use std::os::windows::process::CommandExt;
    let mut command = std::process::Command::new(
        super::platform()
            .system_executable(super::SystemExecutable::PowerShell)
            .map_err(|error| CommandError::SystemError(error.to_string()))?,
    );
    command
    .arg("-NoProfile")
    .arg("-NonInteractive")
    .arg("-Command")
    .arg(script)
    .env("OXYDE_SHELL_TARGET", path)
    .creation_flags(0x08000000);
    let output = super::windows_process_launch::output(command)
    .map_err(|error| CommandError::SystemError(format!("PowerShell failed to start: {error}")))?;
    if output.status.success() {
        return Ok(());
    }
    let message: String = String::from_utf8_lossy(&output.stderr)
        .trim()
        .chars()
        .take(2048)
        .collect();
    Err(CommandError::SystemError(format!(
        "PowerShell failed ({}): {message}",
        output.status
    )))
}

fn pin_fallback_script(is_unpin: bool) -> String {
    let source = if is_unpin {
        r#"$qa = $sh.Namespace('shell:::{679f85cb-0220-4080-b29b-5540cc05aab6}')
    if (-not $qa) { throw 'Quick Access is unavailable' }
    $item = $qa.Items() | Where-Object { $_.Path -eq $target -or $_.GetFolder.Self.Path -eq $target } | Select-Object -First 1
    if (-not $item) { exit 0 }"#
    } else {
        r#"$folder = $sh.Namespace($target)
    if (-not $folder) { throw 'Target folder is unavailable' }
    $item = $folder.Self"#
    };
    let (pattern, excluded, canonical) = if is_unpin {
        (
            "unpin|desepingler|retirer|detacher|losen",
            "^$",
            "$item.InvokeVerb('unpinfromhome'); $item.InvokeVerb('unpinfromquickaccess')",
        )
    } else {
        (
            "pin|epingler|attacher|anheft",
            "unpin|desepingler|retirer|detacher|losen",
            "$item.InvokeVerb('pintohome'); $item.InvokeVerb('pintofavorites')",
        )
    };
    // The child environment carries the literal path, including Unicode and
    // quotes; it is never interpolated into PowerShell source code.
    format!(
        r#"$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
try {{
    $target = [Environment]::GetEnvironmentVariable('OXYDE_SHELL_TARGET')
    $sh = New-Object -ComObject Shell.Application
    {source}
    $verb = $item.Verbs() | Where-Object {{
        $name = $_.Name.Replace('&', '').Normalize([Text.NormalizationForm]::FormD)
        $name = -join ($name.ToCharArray() | Where-Object {{ [Globalization.CharUnicodeInfo]::GetUnicodeCategory($_) -ne [Globalization.UnicodeCategory]::NonSpacingMark }})
        $name -match '{pattern}' -and $name -notmatch '{excluded}'
    }} | Select-Object -First 1
    if ($verb) {{ $verb.DoIt() }} else {{ {canonical} }}
}} catch {{
    [Console]::Error.WriteLine($_.Exception.Message)
    exit 1
}}"#
    )
}

pub(super) fn get_native_context_menu_items(
    path: String,
    is_background: bool,
) -> Result<Vec<WinMenuItem>, CommandError> {
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::Shell::{
        IContextMenu, IContextMenu2, IContextMenu3, IShellFolder, SHBindToParent,
        SHParseDisplayName, CMF_CANRENAME, CMF_EXPLORE, CMF_NORMAL,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetMenuItemCount, GetMenuItemID, GetMenuStringW, GetSubMenu, MF_BYPOSITION,
        WM_INITMENUPOPUP,
    };

    let pb = validate_path(&path)?;
    let path_norm = pb.to_string_lossy().replace("/", "\\");
    let path_u16: Vec<u16> = path_norm.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let _com =
            ComApartment::sta().map_err(|error| CommandError::SystemError(error.to_string()))?;

        let mut pidl_full = std::ptr::null_mut();
        SHParseDisplayName(PCWSTR(path_u16.as_ptr()), None, &mut pidl_full, 0, None)
            .map_err(|e| CommandError::SystemError(format!("SHParseDisplayName failed: {}", e)))?;

        let _pidl = TaskMemory::new(pidl_full);
        let mut pidl_relative = std::ptr::null_mut();
        let parent_folder: IShellFolder = SHBindToParent(pidl_full, Some(&mut pidl_relative))
            .map_err(|e| CommandError::SystemError(format!("SHBindToParent failed: {:?}", e)))?;

        let context_menu: IContextMenu = if is_background {
            // For background menu, we need the IShellFolder of the target itself
            let target_folder: IShellFolder = parent_folder
                .BindToObject(pidl_relative, None)
                .map_err(|e| CommandError::SystemError(format!("BindToObject failed: {:?}", e)))?;

            // Use CreateViewObject to get the background context menu
            target_folder
                .CreateViewObject(HWND::default())
                .map_err(|e| CommandError::SystemError(format!("CreateViewObject failed: {}", e)))?
        } else {
            let pidl_relative_slice = [pidl_relative as *const _];
            parent_folder
                .GetUIObjectOf(HWND::default(), &pidl_relative_slice, None)
                .map_err(|e| CommandError::SystemError(format!("GetUIObjectOf failed: {}", e)))?
        };

        let menu = query_menu(&context_menu, CMF_NORMAL | CMF_EXPLORE | CMF_CANRENAME)?;
        let hmenu = menu.0;

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
                    let len = GetMenuStringW(hmenu, i as u32, Some(&mut label_buf), MF_BYPOSITION);
                    let mut label = String::from_utf16_lossy(&label_buf[..len as usize]);

                    // Clean up & in labels (e.g. "&Open" -> "Open")
                    label = label.replace("&", "");

                    if label.is_empty() || label == "-" {
                        continue;
                    }

                    let is_new_menu =
                        label.to_lowercase().contains("nouveau") || label.to_lowercase() == "new";

                    // Filter standard verbs
                    let verb = canonical_verb(context_menu, id);
                    if !is_new_menu {
                        if let Some(verb) = &verb {
                            let verb = verb.to_lowercase();
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
                    if let Some(verb) = verb {
                        item_verb = Some(verb);
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

        Ok(items)
    }
}

pub(super) fn execute_native_menu_item(
    path: String,
    id: i32,
    is_background: bool,
) -> Result<(), CommandError> {
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::{
        IContextMenu, IContextMenu2, IContextMenu3, IShellFolder, SHBindToParent,
        SHParseDisplayName, CMF_CANRENAME, CMF_EXPLORE, CMF_NORMAL,
    };

    if !(1..=0x7FFF).contains(&id) {
        return Err(CommandError::SystemError("Invalid Shell command ID".into()));
    }

    let pb = validate_path(&path)?;
    let path_norm = pb.to_string_lossy().replace("/", "\\");
    let path_u16: Vec<u16> = path_norm.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let _com =
            ComApartment::sta().map_err(|error| CommandError::SystemError(error.to_string()))?;

        let main_hwnd = HWND::default(); // needs proper HWND for full functionality

        let mut pidl_full = std::ptr::null_mut();
        SHParseDisplayName(PCWSTR(path_u16.as_ptr()), None, &mut pidl_full, 0, None)
            .map_err(|e| CommandError::SystemError(format!("SHParseDisplayName failed: {}", e)))?;

        let _pidl = TaskMemory::new(pidl_full);
        let mut pidl_relative = std::ptr::null_mut();
        let parent_folder: IShellFolder = SHBindToParent(pidl_full, Some(&mut pidl_relative))
            .map_err(|e| CommandError::SystemError(format!("SHBindToParent failed: {:?}", e)))?;

        let context_menu: IContextMenu = if is_background {
            let target_folder: IShellFolder = parent_folder
                .BindToObject(pidl_relative, None)
                .map_err(|e| CommandError::SystemError(format!("BindToObject failed: {:?}", e)))?;

            target_folder
                .CreateViewObject(main_hwnd)
                .map_err(|e| CommandError::SystemError(format!("CreateViewObject failed: {}", e)))?
        } else {
            let pidl_relative_slice = [pidl_relative as *const _];
            parent_folder
                .GetUIObjectOf(main_hwnd, &pidl_relative_slice, None)
                .map_err(|e| CommandError::SystemError(format!("GetUIObjectOf failed: {}", e)))?
        };

        // We MUST call QueryContextMenu before InvokeCommand so the extension knows what IDs we are talking about
        let menu = query_menu(&context_menu, CMF_NORMAL | CMF_EXPLORE | CMF_CANRENAME)?;
        let hmenu = menu.0;

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

        invoke_menu_with(id as u32, main_hwnd, |info| {
            context_menu.InvokeCommand(info)
        })?;
    }
    Ok(())
}

pub(super) fn get_quick_access_items() -> Result<Vec<QuickAccessItem>, CommandError> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows::Win32::UI::Shell::{
        IEnumIDList, ILCombine, IShellFolder, IShellItem, SHBindToObject, SHCreateItemFromIDList,
        SHParseDisplayName, SHCONTF_FOLDERS, SIGDN_FILESYSPATH, SIGDN_NORMALDISPLAY,
    };

    unsafe {
        let _com = ComApartment::sta_or_existing()
            .map_err(|error| CommandError::SystemError(error.to_string()))?;

        // Quick Access virtual folder CLSID
        let qa_path = "shell:::{679f85cb-0220-4080-b29b-5540cc05aab6}";
        let qa_u16: Vec<u16> = qa_path.encode_utf16().chain(std::iter::once(0)).collect();

        let mut pidl_qa: *mut ITEMIDLIST = std::ptr::null_mut();
        if SHParseDisplayName(PCWSTR(qa_u16.as_ptr()), None, &mut pidl_qa, 0, None).is_err()
            || pidl_qa.is_null()
        {
            return Ok(Vec::new());
        }

        let _parent_pidl = TaskMemory::new(pidl_qa);
        let folder: IShellFolder = match SHBindToObject(None, pidl_qa as *const _, None) {
            Ok(f) => f,
            Err(_) => {
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
            let _child_pidl = TaskMemory::new(pidl_child);

            // Combine parent PIDL + child PIDL to form an absolute PIDL,
            // then resolve it to a real filesystem path via IShellItem.
            let pidl_abs = ILCombine(Some(pidl_qa as *const _), Some(pidl_child as *const _));
            if !pidl_abs.is_null() {
                let _absolute_pidl = TaskMemory::new(pidl_abs);
                if let Ok(si) = SHCreateItemFromIDList::<IShellItem>(pidl_abs as *const _) {
                    let path_opt = si
                        .GetDisplayName(SIGDN_FILESYSPATH)
                        .ok()
                        .and_then(|pw| take_string(pw).ok());

                    if let Some(path) = path_opt {
                        if !path.is_empty() && !path.starts_with("::{") {
                            let name = si
                                .GetDisplayName(SIGDN_NORMALDISPLAY)
                                .ok()
                                .and_then(|pw| take_string(pw).ok())
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
            }
        }

        Ok(items)
    }
}

pub(super) fn execute_shell_verb_by_canonical_name(
    path: &str,
    target_verbs: &[&str],
) -> Result<(), CommandError> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::{
        IContextMenu, IShellFolder, SHBindToParent, SHParseDisplayName, CMF_NORMAL,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetMenuItemCount, GetMenuItemID, GetMenuStringW, MF_BYPOSITION,
    };

    let pb = validate_path(path)?;
    let mut path_norm = pb.to_string_lossy().replace("/", "\\");
    if path_norm.len() == 2 && path_norm.ends_with(':') {
        path_norm.push('\\');
    }
    let path_u16: Vec<u16> = path_norm.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let _com = ComApartment::sta_or_existing()
            .map_err(|error| CommandError::SystemError(error.to_string()))?;
        let is_unpin = target_verbs.iter().any(|v| v.contains("unpin"));
        let mut pidl_full = std::ptr::null_mut();
        if let Err(e) = SHParseDisplayName(PCWSTR(path_u16.as_ptr()), None, &mut pidl_full, 0, None)
        {
            if is_unpin {
                // Report process/COM failures instead of discarding the fallback result.
                return run_powershell(&pin_fallback_script(true), &path_norm);
            }
            return Err(CommandError::SystemError(format!(
                "SHParseDisplayName failed: {}",
                e
            )));
        }

        let _pidl = TaskMemory::new(pidl_full);
        let mut pidl_relative = std::ptr::null_mut();
        let parent_folder: IShellFolder = SHBindToParent(pidl_full, Some(&mut pidl_relative))
            .map_err(|e| CommandError::SystemError(format!("SHBindToParent failed: {:?}", e)))?;

        let pidl_relative_slice = [pidl_relative as *const _];
        let context_menu: IContextMenu = parent_folder
            .GetUIObjectOf(HWND(std::ptr::null_mut()), &pidl_relative_slice, None)
            .map_err(|e| CommandError::SystemError(format!("GetUIObjectOf failed: {}", e)))?;

        let menu = query_menu(&context_menu, CMF_NORMAL)?;
        let hmenu = menu.0;

        let count = GetMenuItemCount(Some(hmenu));
        let mut target_id: Option<u32> = None;

        for i in 0..count {
            let id = GetMenuItemID(hmenu, i);
            if (1..=0x7FFF).contains(&id) {
                // 1. Try canonical verb lookup first
                if let Some(verb) = canonical_verb(&context_menu, id) {
                    let verb = verb.to_lowercase();
                    if target_verbs.iter().any(|&v| verb == v) {
                        target_id = Some(id);
                        break;
                    }
                }

                // 2. Try localized label matching as fallback (Flexible/Substrings)
                let mut label_buf = [0u16; 256];
                let len = GetMenuStringW(hmenu, i as u32, Some(&mut label_buf), MF_BYPOSITION);
                if len > 0 {
                    let label = String::from_utf16_lossy(&label_buf[..len as usize]);
                    if localized_pin_label(&label, is_unpin) {
                        target_id = Some(id);
                        break;
                    }
                }
            }
        }

        if let Some(id) = target_id {
            // An invocation failure may already have produced effects. Do not
            // launch a second command through PowerShell after that failure.
            invoke_menu_with(id, HWND::default(), |info| context_menu.InvokeCommand(info))
        } else {
            run_powershell(&pin_fallback_script(is_unpin), &path_norm)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::core::{Error, HRESULT};
    use windows::Win32::Foundation::{E_ACCESSDENIED, E_FAIL, HWND};
    use windows::Win32::UI::WindowsAndMessaging::{CreatePopupMenu, IsMenu};

    #[test]
    fn localized_labels_distinguish_pin_from_unpin_and_preserve_accents() {
        for (pin, unpin) in [
            ("&Pin to Quick access", "Unpin from Quick access"),
            (
                "É&pingler à l’accès rapide",
                "Désépingler de l’accès rapide",
            ),
            ("An Schnellzugriff anheften", "Von Schnellzugriff lösen"),
        ] {
            assert!(localized_pin_label(pin, false), "{pin}");
            assert!(!localized_pin_label(pin, true), "{pin}");
            assert!(localized_pin_label(unpin, true), "{unpin}");
            assert!(!localized_pin_label(unpin, false), "{unpin}");
        }
        for label in [
            "Open",
            "Attach to archive",
            "Remove shortcut",
            "Quick access",
        ] {
            assert!(!localized_pin_label(label, false));
            assert!(!localized_pin_label(label, true));
        }
    }

    #[test]
    fn canonical_verb_refuses_unterminated_or_failed_response() {
        assert!(read_verb_with(|buffer| {
            buffer.fill(b'x');
            Ok(())
        })
        .is_none());
        assert!(read_verb_with(|buffer| {
            buffer[..5].copy_from_slice(b"open\0");
            Err(Error::from_hresult(E_FAIL))
        })
        .is_none());
        assert!(read_verb_with(|_| Ok(())).is_none());
    }

    #[test]
    fn canonical_verb_accepts_last_byte_terminator_and_keeps_lossy_decoding() {
        let mut calls = 0;
        let verb = read_verb_with(|buffer| {
            calls += 1;
            buffer.fill(b'x');
            buffer[127] = 0;
            Ok(())
        })
        .unwrap();
        assert_eq!(calls, 1);
        assert_eq!(verb.len(), 127);
        assert_eq!(
            read_verb_with(|buffer| {
                buffer[..7].copy_from_slice(b"verb\xff\0x");
                Ok(())
            })
            .unwrap(),
            "verb\u{fffd}"
        );
    }

    #[test]
    fn failed_query_destroys_menu_and_accepts_successful_command_count() {
        let mut handle = Default::default();
        let result = query_menu_with(|menu| {
            handle = menu;
            E_ACCESSDENIED
        });
        let error = result.err().unwrap().to_string();
        assert!(error.contains("QueryContextMenu failed") && error.contains("80070005"));
        assert!(!unsafe { IsMenu(handle) }.as_bool());
        let menu = query_menu_with(|_| HRESULT(7)).unwrap();
        let handle = menu.0;
        assert!(unsafe { IsMenu(handle) }.as_bool());
        drop(menu);
        assert!(!unsafe { IsMenu(handle) }.as_bool());
    }

    #[test]
    fn invocation_propagates_failure_and_refuses_invalid_offsets() {
        let mut called = false;
        let error = invoke_menu_with(1, HWND::default(), |info| {
            called = true;
            assert_eq!(info.lpVerb.0 as usize, 0);
            Err(Error::from_hresult(E_ACCESSDENIED))
        })
        .unwrap_err()
        .to_string();
        assert!(called);
        assert!(error.contains("InvokeCommand failed") && error.contains("80070005"));
        invoke_menu_with(0x7FFF, HWND::default(), |info| {
            assert_eq!(info.lpVerb.0 as usize, 0x7FFE);
            Ok(())
        })
        .unwrap();
        for id in [0, 0x8000, u32::MAX] {
            assert!(
                invoke_menu_with(id, HWND::default(), |_| panic!("invalid command invoked"))
                    .is_err()
            );
        }
    }

    fn fake_shell_prefix(pinned: bool, mode: &str) -> String {
        // Replace the COM factory in this child process. No Shell.Application
        // instance or user Quick Access state is accessed.
        format!(
            r#"$global:pinned = ${pinned}
$global:mode = '{mode}'
$global:calls = 0
$global:item = [pscustomobject]@{{ Path = [Environment]::GetEnvironmentVariable('OXYDE_SHELL_TARGET') }}
$global:item | Add-Member NoteProperty GetFolder ([pscustomobject]@{{ Self = [pscustomobject]@{{ Path = $global:item.Path }} }})
if ($global:mode -eq 'proxy') {{ $global:item.Path = '::{{Quick Access proxy}}' }}
$global:verb = [pscustomobject]@{{ Name = "&Désépingler de l’accès rapide" }}
if (-not $global:pinned) {{ $global:verb.Name = "É&pingler à l’accès rapide" }}
$global:verb | Add-Member ScriptMethod DoIt {{
    $global:calls++
    if ($global:calls -ne 1) {{ throw 'duplicate invocation' }}
    if ($global:mode -eq 'throws') {{ throw 'injected Shell error' }}
    if ($global:mode -eq 'unicode-error') {{ throw 'Erreur simulée 日本語' }}
    $global:pinned = -not $global:pinned
}}
$global:item | Add-Member ScriptMethod Verbs {{
    if ($global:mode -ne 'canonical') {{ return $global:verb }}
}}
$global:item | Add-Member ScriptMethod InvokeVerb {{
    param($name)
    if ($global:mode -ne 'canonical') {{ throw 'unexpected canonical fallback' }}
    if ($name -notin @('unpinfromhome', 'unpinfromquickaccess')) {{ throw 'incorrect command' }}
}}
$global:qa = [pscustomobject]@{{}}
$global:qa | Add-Member ScriptMethod Items {{
    if ($global:pinned) {{ return $global:item }}
}}
$global:folder = [pscustomobject]@{{ Self = $global:item }}
$global:shell = [pscustomobject]@{{}}
$global:shell | Add-Member ScriptMethod Namespace {{
    param($path)
    if ($global:mode -eq 'missing') {{ return $null }}
    if ($path.StartsWith('shell:::')) {{ return $global:qa }}
    if ($path -eq $global:item.Path) {{ return $global:folder }}
    throw 'incorrect literal path'
}}
function New-Object {{ param($ComObject) return $global:shell }}
"#
        )
    }

    #[test]
    fn powershell_fallback_handles_localized_verbs_and_literal_paths_without_user_shell() {
        for (pinned, unpin, mode) in [
            (false, false, "works"),
            (true, true, "works"),
            (true, true, "canonical"),
            (true, true, "proxy"),
            (false, true, "throws"), // already absent; must not invoke
        ] {
            let script = fake_shell_prefix(pinned, mode) + &pin_fallback_script(unpin);
            run_powershell(&script, "C:\\Oxyde 日本語 test's ’; throw 'path injected'").unwrap();
        }
    }

    #[test]
    fn powershell_fallback_reports_process_and_shell_errors() {
        for (mode, message) in [
            ("throws", "injected Shell error"),
            ("unicode-error", "Erreur simulée 日本語"),
            ("missing", "Quick Access is unavailable"),
        ] {
            let script = fake_shell_prefix(true, mode) + &pin_fallback_script(true);
            let error = run_powershell(&script, "C:\\fixture")
                .unwrap_err()
                .to_string();
            assert!(error.contains(message), "{error}");
        }
        let error = run_powershell(
            "[Console]::Error.WriteLine('injected process error'); exit 7",
            "",
        )
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("exit code: 7") && error.contains("injected process error"),
            "{error}"
        );
    }

    #[test]
    fn native_popup_menu_is_destroyed_on_error_and_unwind() {
        unsafe {
            let menu = CreatePopupMenu().unwrap();
            let fail = || -> Result<(), ()> {
                let _menu = PopupMenuGuard(menu);
                Err(())
            };
            assert!(fail().is_err());
            assert!(!IsMenu(menu).as_bool());

            let menu = CreatePopupMenu().unwrap();
            assert!(std::panic::catch_unwind(|| {
                let _menu = PopupMenuGuard(menu);
                panic!("injected menu failure");
            })
            .is_err());
            assert!(!IsMenu(menu).as_bool());
        }
    }
}
