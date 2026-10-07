#![cfg(target_os = "windows")]

use crate::models::CommandError;
use crate::utils::path_security::validate_path;
use log::info;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::DataExchange::{
    CloseClipboard, GetClipboardData, OpenClipboard,
};
use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_HDROP;
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

    super::windows_clipboard_write::set_files(&validated_paths, is_cut)
}

pub fn set_clipboard_text(text: String) -> Result<(), CommandError> {
    super::windows_clipboard_write::set_text(&text)
}
