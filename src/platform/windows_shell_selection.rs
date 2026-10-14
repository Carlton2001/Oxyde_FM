//! Build one Shell context menu for the complete selection, including search results.
use crate::models::CommandError;
use crate::utils::path_security::validate_path;
use super::windows_com::TaskMemory;
use windows::core::PCWSTR;
use windows::Win32::UI::Shell::{
    IContextMenu, IShellItemArray, SHCreateShellItemArrayFromIDLists, SHParseDisplayName, BHID_SFUIObject,
};

/// The caller owns an initialized COM apartment for the lifetime of the returned menu.
pub(super) fn selection_context_menu(paths: &[String]) -> Result<IContextMenu, CommandError> {
    let selection = shell_selection(paths)?;
    unsafe {
        selection.BindToHandler(None, &BHID_SFUIObject)
            .map_err(|error| CommandError::SystemError(error.to_string()))
    }
}

fn shell_selection(paths: &[String]) -> Result<IShellItemArray, CommandError> {
    unsafe {
        let mut allocations = Vec::with_capacity(paths.len());
        let mut pidls = Vec::with_capacity(paths.len());
        for path in paths {
            let normalized = validate_path(path)?.to_string_lossy().replace('/', "\\");
            let wide: Vec<_> = normalized.encode_utf16().chain(Some(0)).collect();
            let mut pidl = std::ptr::null_mut();
            SHParseDisplayName(PCWSTR(wide.as_ptr()), None, &mut pidl, 0, None)
                .map_err(|error| CommandError::SystemError(error.to_string()))?;
            allocations.push(TaskMemory::new(pidl));
            pidls.push(pidl as *const _);
        }
        SHCreateShellItemArrayFromIDLists(&pidls)
            .map_err(|error| CommandError::SystemError(error.to_string()))
    }
}

#[cfg(test)]
#[path = "windows_shell_selection/tests/selection.rs"]
mod selection_tests;
