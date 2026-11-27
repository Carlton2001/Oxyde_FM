use super::{classify_cloud_file, wide_path};
use crate::models::CloudAvailability;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::Path;
use windows::core::{HRESULT, PCWSTR};
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_NO_MORE_FILES, HANDLE};
use windows::Win32::Storage::CloudFilters::{
    CF_PLACEHOLDER_STATE, CF_PLACEHOLDER_STATE_INVALID, CF_PLACEHOLDER_STATE_PLACEHOLDER,
    CF_PLACEHOLDER_STATE_SYNC_ROOT,
};
use windows::Win32::Storage::FileSystem::{
    FindClose, FindExInfoBasic, FindExSearchNameMatch, FindFirstFileExW, FindNextFileW,
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FIND_FIRST_EX_LARGE_FETCH,
    FIND_FIRST_EX_ON_DISK_ENTRIES_ONLY, WIN32_FIND_DATAW,
};

struct FindHandle(HANDLE);

impl Drop for FindHandle {
    fn drop(&mut self) {
        let _ = unsafe { FindClose(self.0) };
    }
}

pub(super) fn contents_ready(
    root: &Path,
    availability: CloudAvailability,
    placeholder_state: impl Fn(&WIN32_FIND_DATAW) -> CF_PLACEHOLDER_STATE,
) -> Option<bool> {
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let pattern = wide_path(&directory.join("*"))?;
        let mut data = WIN32_FIND_DATAW::default();
        // Inspect cached directory entries without fetching entries from the provider.
        let first = unsafe {
            FindFirstFileExW(
                PCWSTR(pattern.as_ptr()),
                FindExInfoBasic,
                std::ptr::from_mut(&mut data).cast(),
                FindExSearchNameMatch,
                None,
                FIND_FIRST_EX_ON_DISK_ENTRIES_ONLY | FIND_FIRST_EX_LARGE_FETCH,
            )
        };
        let handle = match first {
            Ok(handle) => FindHandle(handle),
            Err(error) if error.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0) => continue,
            Err(_) => return None,
        };
        loop {
            let name = &data.cFileName[..data.cFileName.iter().position(|&unit| unit == 0)?];
            if !matches!(name, [46] | [46, 46]) {
                let attrs = data.dwFileAttributes;
                let state = placeholder_state(&data);
                if state == CF_PLACEHOLDER_STATE_INVALID {
                    return None;
                }
                // Avoid other sync roots, junctions and symbolic links.
                let cloud = state.contains(CF_PLACEHOLDER_STATE_PLACEHOLDER);
                let unrelated = state.contains(CF_PLACEHOLDER_STATE_SYNC_ROOT)
                    || (!cloud && attrs & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0);
                if !unrelated {
                    if classify_cloud_file(state, attrs)
                        .is_some_and(|status| !availability.is_confirmed_by(status))
                    {
                        return Some(false);
                    }
                    if attrs & FILE_ATTRIBUTE_DIRECTORY.0 != 0 {
                        pending.push(directory.join(OsString::from_wide(name)));
                    }
                }
            }
            match unsafe { FindNextFileW(handle.0, &mut data) } {
                Ok(()) => {}
                Err(error) if error.code() == HRESULT::from_win32(ERROR_NO_MORE_FILES.0) => break,
                Err(_) => return None,
            }
        }
    }
    Some(true)
}

#[cfg(test)]
#[path = "tests/folder.rs"]
mod tests;
