use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use tauri::command;
use windows::core::PCWSTR;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::{CF_HDROP, CF_UNICODETEXT};
use windows::Win32::UI::Shell::{IShellItem, SHCreateItemFromParsingName, DROPFILES};
use crate::models::CommandError;
use crate::utils::path_security::validate_path;
use log::{info, warn};

// Custom format for Preferred DropEffect
fn get_drop_effect_format() -> u32 {
    use windows::core::PCSTR;
    use windows::Win32::System::DataExchange::RegisterClipboardFormatA;
    unsafe { RegisterClipboardFormatA(PCSTR(c"Preferred DropEffect".as_ptr() as *const _)) }
}

#[command]
pub fn get_clipboard_files() -> Result<(Vec<String>, bool), CommandError> {
    let mut files: Vec<String> = Vec::new();
    let mut is_cut = false;

    unsafe {
        // Open clipboard
        if OpenClipboard(None).is_err() {
            return Ok((files, is_cut));
        }

        // Get CF_HDROP data
        let hdrop = GetClipboardData(CF_HDROP.0 as u32);
        if let Ok(handle) = hdrop {
            if !handle.is_invalid() {
                let ptr = GlobalLock(std::mem::transmute::<
                    HANDLE,
                    windows::Win32::Foundation::HGLOBAL,
                >(handle));
                if !ptr.is_null() {
                    let dropfiles = ptr as *const DROPFILES;
                    let offset = (*dropfiles).pFiles as usize;
                    let is_wide = (*dropfiles).fWide.as_bool();

                    if is_wide {
                        // Parse wide strings (UTF-16)
                        let data_ptr = (ptr as *const u8).add(offset) as *const u16;
                        let mut current = data_ptr;

                        loop {
                            if *current == 0 {
                                break;
                            }

                            // Find end of string
                            let mut len = 0;
                            while *current.add(len) != 0 {
                                len += 1;
                            }

                            let slice = std::slice::from_raw_parts(current, len);
                            if let Ok(s) = String::from_utf16(slice) {
                                files.push(s);
                            }

                            current = current.add(len + 1);
                        }
                    }

                    let _ = GlobalUnlock(std::mem::transmute::<
                        HANDLE,
                        windows::Win32::Foundation::HGLOBAL,
                    >(handle));
                }
            }
        }

        // Check DropEffect
        if !files.is_empty() {
            let drop_effect_format = get_drop_effect_format();
            if drop_effect_format != 0 {
                if let Ok(handle) = GetClipboardData(drop_effect_format) {
                    if !handle.is_invalid() {
                        let ptr = GlobalLock(std::mem::transmute::<
                            HANDLE,
                            windows::Win32::Foundation::HGLOBAL,
                        >(handle));
                        if !ptr.is_null() {
                            let effect = *(ptr as *const u32);
                            is_cut = effect == 2; // DROPEFFECT_MOVE
                            let _ = GlobalUnlock(std::mem::transmute::<
                                HANDLE,
                                windows::Win32::Foundation::HGLOBAL,
                            >(handle));
                        }
                    }
                }
            }
        }

        let _ = CloseClipboard();
    }

    Ok((files, is_cut))
}

#[command(rename_all = "snake_case")]
pub fn set_clipboard_files(paths: Vec<String>, is_cut: bool) -> Result<(), CommandError> {
    info!("Setting clipboard (cut={}): {:?}", is_cut, paths);
    let validated_paths: Vec<String> = paths.iter()
        .map(|p| validate_path(p).map(|pb: std::path::PathBuf| pb.to_string_lossy().to_string()))
        .collect::<Result<Vec<String>, CommandError>>()?;

    unsafe {
        // Open clipboard
        OpenClipboard(None).map_err(|e| CommandError::SystemError(format!("Failed to open clipboard: {:?}", e)))?;

        // Empty clipboard
        if EmptyClipboard().is_err() {
            let _ = CloseClipboard();
            return Err(CommandError::SystemError("Failed to empty clipboard".to_string()));
        }

        if paths.is_empty() {
            let _ = CloseClipboard();
            return Ok(());
        }

        // Build DROPFILES structure
        // Format: DROPFILES struct + null-terminated wide strings + final null
        let mut wide_paths: Vec<Vec<u16>> = Vec::new();
        let mut total_chars = 0;

        for path in &validated_paths {
            let wide: Vec<u16> = OsStr::new(path)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            total_chars += wide.len();
            wide_paths.push(wide);
        }
        total_chars += 1; // Final null terminator

        let dropfiles_size = std::mem::size_of::<DROPFILES>();
        let total_size = dropfiles_size + total_chars * 2;

        let hglobal = GlobalAlloc(GMEM_MOVEABLE, total_size).map_err(|e| {
            let _ = CloseClipboard();
            CommandError::SystemError(format!("Failed to allocate memory: {:?}", e))
        })?;

        let ptr = GlobalLock(hglobal);
        if ptr.is_null() {
            let _ = CloseClipboard();
            return Err(CommandError::SystemError("Failed to lock memory".to_string()));
        }

        // Write DROPFILES header
        let dropfiles = ptr as *mut DROPFILES;
        (*dropfiles).pFiles = dropfiles_size as u32;
        (*dropfiles).pt.x = 0;
        (*dropfiles).pt.y = 0;
        (*dropfiles).fNC = false.into();
        (*dropfiles).fWide = true.into();

        // Write file paths
        let mut dest = (ptr as *mut u8).add(dropfiles_size) as *mut u16;
        for wide_path in &wide_paths {
            std::ptr::copy_nonoverlapping(wide_path.as_ptr(), dest, wide_path.len());
            dest = dest.add(wide_path.len());
        }
        *dest = 0; // Final null terminator

        let _ = GlobalUnlock(hglobal);

        // Set clipboard data
        let handle = HANDLE(hglobal.0);
        SetClipboardData(CF_HDROP.0 as u32, Some(handle)).map_err(|e| {
            let _ = CloseClipboard();
            CommandError::SystemError(format!("Failed to set clipboard data: {:?}", e))
        })?;

        // Set DropEffect
        let drop_effect_format = get_drop_effect_format();
        if drop_effect_format != 0 {
            let effect: u32 = if is_cut { 2 } else { 1 };

            let effect_global = GlobalAlloc(GMEM_MOVEABLE, 4).map_err(|e| {
                let _ = CloseClipboard();
                CommandError::SystemError(format!("Failed to allocate effect memory: {:?}", e))
            })?;

            let effect_ptr = GlobalLock(effect_global);
            if !effect_ptr.is_null() {
                *(effect_ptr as *mut u32) = effect;
                let _ = GlobalUnlock(effect_global);

                let effect_handle = HANDLE(effect_global.0);
                let _ = SetClipboardData(drop_effect_format, Some(effect_handle));
            }
        }

        let _ = CloseClipboard();
    }

    Ok(())
}

#[command]
pub fn get_clipboard_text() -> Result<String, CommandError> {
    unsafe {
        if OpenClipboard(None).is_err() {
            return Err(CommandError::SystemError("Failed to open clipboard".to_string()));
        }

        let mut result = String::new();
        if let Ok(handle) = GetClipboardData(CF_UNICODETEXT.0 as u32) {
            if !handle.is_invalid() {
                let ptr = GlobalLock(std::mem::transmute::<
                    HANDLE,
                    windows::Win32::Foundation::HGLOBAL,
                >(handle));
                
                if !ptr.is_null() {
                    let mut len = 0;
                    let wide_ptr = ptr as *const u16;
                    while *wide_ptr.add(len) != 0 {
                        len += 1;
                    }
                    
                    let slice = std::slice::from_raw_parts(wide_ptr, len);
                    if let Ok(s) = String::from_utf16(slice) {
                        result = s;
                    }
                    
                    let _ = GlobalUnlock(std::mem::transmute::<
                        HANDLE,
                        windows::Win32::Foundation::HGLOBAL,
                    >(handle));
                }
            }
        }

        let _ = CloseClipboard();
        Ok(result)
    }
}

#[command]
pub fn set_clipboard_text(text: String) -> Result<(), CommandError> {
    unsafe {
        OpenClipboard(None).map_err(|e| CommandError::SystemError(format!("Failed to open clipboard: {:?}", e)))?;
        
        if EmptyClipboard().is_err() {
            let _ = CloseClipboard();
            return Err(CommandError::SystemError("Failed to empty clipboard".to_string()));
        }

        let wide_chars: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let size = wide_chars.len() * 2;

        let hglobal = GlobalAlloc(GMEM_MOVEABLE, size).map_err(|e| {
            let _ = CloseClipboard();
            CommandError::SystemError(format!("Failed to allocate memory: {:?}", e))
        })?;

        let ptr = GlobalLock(hglobal);
        if ptr.is_null() {
            let _ = CloseClipboard();
            return Err(CommandError::SystemError("Failed to lock memory".to_string()));
        }

        std::ptr::copy_nonoverlapping(wide_chars.as_ptr(), ptr as *mut u16, wide_chars.len());
        let _ = GlobalUnlock(hglobal);

        let handle = HANDLE(hglobal.0);
        SetClipboardData(CF_UNICODETEXT.0 as u32, Some(handle)).map_err(|e| {
            let _ = CloseClipboard();
            CommandError::SystemError(format!("Failed to set clipboard data: {:?}", e))
        })?;

        let _ = CloseClipboard();
        Ok(())
    }
}

/// Special cut operation for recycle bin items using Shell API
/// Uses OleSetClipboard with IDataObject so files stay in trash until paste
#[command(rename_all = "snake_case")]
pub fn set_clipboard_from_trash(trash_paths: Vec<String>) -> Result<Vec<String>, CommandError> {
    info!("Setting clipboard from trash: {:?}", trash_paths);
    let validated_paths: Vec<String> = trash_paths.iter()
        .map(|p| validate_path(p).map(|pb: std::path::PathBuf| pb.to_string_lossy().to_string()))
        .collect::<Result<Vec<String>, CommandError>>()?;

    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows::Win32::System::Ole::OleSetClipboard;
    use windows::Win32::UI::Shell::BHID_DataObject;

    unsafe {
        // Initialize COM
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        // Create IShellItems from paths using our existing helper
        let mut shell_items: Vec<IShellItem> = Vec::new();

        for path in &validated_paths {
            let wide_path: Vec<u16> = OsStr::new(path)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            if let Ok(item) = SHCreateItemFromParsingName(
                PCWSTR(wide_path.as_ptr()),
                None::<&windows::Win32::System::Com::IBindCtx>,
            ) {
                shell_items.push(item);
            }
        }

        if shell_items.is_empty() {
            CoUninitialize();
            return Err(CommandError::SystemError("Failed to create shell items from trash paths".to_string()));
        }

        // Create IShellItemArray from multiple items using PIDLs
        // Use SHGetIDListFromObject to get PIDL from each IShellItem
        use windows::Win32::UI::Shell::Common::ITEMIDLIST;
        use windows::Win32::UI::Shell::{
            ILFree, SHCreateShellItemArrayFromIDLists, SHGetIDListFromObject,
        };

        // Store the PIDLs returned by SHGetIDListFromObject
        // SHGetIDListFromObject returns PIDLIST_ABSOLUTE
        let mut pidl_holders = Vec::new();

        for item in &shell_items {
            if let Ok(pidl) = SHGetIDListFromObject(item) {
                pidl_holders.push(pidl);
            }
        }

        if pidl_holders.is_empty() {
            CoUninitialize();
            return Err(CommandError::SystemError("Failed to get PIDLs from shell items".to_string()));
        }

        // Convert PIDLIST_ABSOLUTE to const ITEMIDLIST pointers for SHCreateShellItemArrayFromIDLists
        // Use transmute since PIDLIST_ABSOLUTE wraps *mut ITEMIDLIST
        let pidls: Vec<*const ITEMIDLIST> = pidl_holders
            .iter()
            .map(|p| std::mem::transmute::<_, *const ITEMIDLIST>(*p))
            .collect();

        // Create IShellItemArray from PIDLs
        let item_array: windows::Win32::UI::Shell::IShellItemArray =
            SHCreateShellItemArrayFromIDLists(&pidls).map_err(|e| {
                // Free PIDLs on error
                for pidl in &pidl_holders {
                    ILFree(Some(*pidl));
                }
                CoUninitialize();
                CommandError::SystemError(format!("Failed to create shell item array: {:?}", e))
            })?;

        // Free the PIDLs after use
        for pidl in &pidl_holders {
            ILFree(Some(*pidl));
        }

        // Get IDataObject from the shell item array
        let data_object: Result<windows::Win32::System::Com::IDataObject, _> =
            item_array.BindToHandler(None, &BHID_DataObject);

        let data_obj = match data_object {
            Ok(obj) => obj,
            Err(e) => {
                CoUninitialize();
                return Err(CommandError::SystemError(format!("Failed to get IDataObject: {:?}", e)));
            }
        };

        // Set CFSTR_PREFERREDDROPEFFECT to DROPEFFECT_MOVE (2) to indicate cut
        if let Err(e) = set_drop_effect_on_data_object(&data_obj, 2) {
            warn!("Could not set drop effect: {}", e);
        }

        // Use OleSetClipboard to put the data object on clipboard
        let result = OleSetClipboard(&data_obj);
        if result.is_ok() {
            use windows::Win32::System::Ole::OleFlushClipboard;
            let _ = OleFlushClipboard();
        }

        CoUninitialize();

        match result {
            Ok(_) => Ok(validated_paths),
            Err(e) => Err(CommandError::SystemError(format!("OleSetClipboard failed: {:?}", e))),
        }
    }
}

/// Helper to set CFSTR_PREFERREDDROPEFFECT on a data object
fn set_drop_effect_on_data_object(
    data_obj: &windows::Win32::System::Com::IDataObject,
    effect: u32,
) -> Result<(), String> {
    use windows::core::PCSTR;
    use windows::Win32::System::Com::{DVASPECT_CONTENT, FORMATETC, STGMEDIUM, TYMED_HGLOBAL};
    use windows::Win32::System::DataExchange::RegisterClipboardFormatA;

    unsafe {
        let format = RegisterClipboardFormatA(PCSTR(c"Preferred DropEffect".as_ptr() as *const _));
        if format == 0 {
            return Err("Failed to register drop effect format".to_string());
        }

        // Allocate memory for the drop effect
        let hglobal =
            GlobalAlloc(GMEM_MOVEABLE, 4).map_err(|e| format!("GlobalAlloc failed: {:?}", e))?;

        let ptr = GlobalLock(hglobal);
        if ptr.is_null() {
            return Err("GlobalLock failed".to_string());
        }
        *(ptr as *mut u32) = effect;
        let _ = GlobalUnlock(hglobal);

        let formatetc = FORMATETC {
            cfFormat: format as u16,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };

        let stgmedium = STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            u: windows::Win32::System::Com::STGMEDIUM_0 { hGlobal: hglobal },
            pUnkForRelease: std::mem::ManuallyDrop::new(None),
        };

        data_obj
            .SetData(&formatetc, &stgmedium, true)
            .map_err(|e| format!("SetData failed: {:?}", e))?;

        Ok(())
    }
}
