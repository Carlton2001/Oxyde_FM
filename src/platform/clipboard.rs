#![cfg(target_os = "windows")]

use crate::models::CommandError;
use crate::utils::path_security::validate_path;
use log::info;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::{CF_HDROP, CF_UNICODETEXT};
use windows::Win32::UI::Shell::DROPFILES;

// Custom format for Preferred DropEffect
fn get_drop_effect_format() -> u32 {
    use windows::core::PCSTR;
    use windows::Win32::System::DataExchange::RegisterClipboardFormatA;
    unsafe { RegisterClipboardFormatA(PCSTR(c"Preferred DropEffect".as_ptr() as *const _)) }
}

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

pub fn set_clipboard_files(paths: Vec<String>, is_cut: bool) -> Result<(), CommandError> {
    info!("Setting clipboard (cut={}): {:?}", is_cut, paths);
    let validated_paths: Vec<String> = paths
        .iter()
        .map(|p| validate_path(p).map(|pb: std::path::PathBuf| pb.to_string_lossy().to_string()))
        .collect::<Result<Vec<String>, CommandError>>()?;

    let transfer_paths: Vec<_> = validated_paths.iter().map(std::path::PathBuf::from).collect();
    let trash_entries = super::platform().trash_transfer_entries(&transfer_paths)
        .map_err(CommandError::TrashError)?;
    if !trash_entries.is_empty() {
        return super::windows_trash_clipboard::set_files(transfer_paths, is_cut)
            .map_err(CommandError::SystemError);
    }

    unsafe {
        // Open clipboard
        OpenClipboard(None)
            .map_err(|e| CommandError::SystemError(format!("Failed to open clipboard: {:?}", e)))?;

        // Empty clipboard
        if EmptyClipboard().is_err() {
            let _ = CloseClipboard();
            return Err(CommandError::SystemError(
                "Failed to empty clipboard".to_string(),
            ));
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
            // Normalize path to use backslashes on Windows for better clipboard compatibility
            let normalized = path.replace('/', "\\");
            let wide: Vec<u16> = OsStr::new(&normalized)
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
            return Err(CommandError::SystemError(
                "Failed to lock memory".to_string(),
            ));
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

pub fn set_clipboard_text(text: String) -> Result<(), CommandError> {
    unsafe {
        OpenClipboard(None)
            .map_err(|e| CommandError::SystemError(format!("Failed to open clipboard: {:?}", e)))?;

        if EmptyClipboard().is_err() {
            let _ = CloseClipboard();
            return Err(CommandError::SystemError(
                "Failed to empty clipboard".to_string(),
            ));
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
            return Err(CommandError::SystemError(
                "Failed to lock memory".to_string(),
            ));
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
