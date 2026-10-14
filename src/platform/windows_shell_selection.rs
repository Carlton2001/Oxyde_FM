//! Build one Shell context menu for the complete selection, including search results.
use super::windows_com::TaskMemory;
use crate::models::CommandError;
use crate::utils::path_security::validate_path;
use windows::core::PCWSTR;
use windows::Win32::UI::Shell::{
    BHID_SFUIObject, IContextMenu, IShellItemArray, SHCreateShellItemArrayFromIDLists,
    SHParseDisplayName,
};
#[path = "windows_shell_selection/controls.rs"]
mod controls;
#[path = "windows_shell_selection/thread.rs"]
mod thread;

/// The caller owns an initialized COM apartment for the lifetime of the returned menu.
pub(super) fn selection_context_menu(paths: &[String]) -> Result<IContextMenu, CommandError> {
    let selection = shell_selection(paths)?;
    unsafe {
        selection
            .BindToHandler(None, &BHID_SFUIObject)
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

#[cfg(test)]
#[path = "windows_shell_selection/tests/native_dialog.rs"]
mod native_dialog_tests;

#[cfg(test)]
fn properties_data_object(
    paths: &[String],
) -> Result<windows::Win32::System::Com::IDataObject, CommandError> {
    properties_ui_object(paths)
}

fn properties_ui_object<T: windows::core::Interface>(paths: &[String]) -> Result<T, CommandError> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::{
        ILFindLastID, IShellFolder, SHBindToParent, SHGetDesktopFolder,
    };
    if paths.is_empty() {
        return Err(CommandError::SystemError("Empty Shell selection".into()));
    }
    unsafe {
        let mut allocations = Vec::with_capacity(paths.len());
        let mut pidls = Vec::with_capacity(paths.len());
        let mut parents = Vec::with_capacity(paths.len());
        for path in paths {
            let validated = validate_path(path)?;
            parents.push(
                validated
                    .parent()
                    .map(|parent| parent.to_string_lossy().replace('/', "\\").to_lowercase()),
            );
            let wide: Vec<_> = validated
                .to_string_lossy()
                .replace('/', "\\")
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let mut pidl = std::ptr::null_mut();
            SHParseDisplayName(PCWSTR(wide.as_ptr()), None, &mut pidl, 0, None)
                .map_err(|error| CommandError::SystemError(error.to_string()))?;
            allocations.push(TaskMemory::new(pidl));
            pidls.push(pidl as *const _);
        }
        // Ask the filesystem folder itself for sibling items. The array handler
        // may manufacture a Desktop identity that merged properties cannot resolve.
        if parents[0].is_some() && parents.iter().all(|parent| parent == &parents[0]) {
            let folder: IShellFolder = SHBindToParent(pidls[0], None)
                .map_err(|error| CommandError::SystemError(error.to_string()))?;
            let children: Vec<_> = pidls
                .iter()
                .map(|pidl| ILFindLastID(*pidl) as *const _)
                .collect();
            folder
                .GetUIObjectOf(HWND::default(), &children, None)
                .map_err(|error| CommandError::SystemError(error.to_string()))
        } else {
            let desktop = SHGetDesktopFolder()
                .map_err(|error| CommandError::SystemError(error.to_string()))?;
            desktop
                .GetUIObjectOf(HWND::default(), &pidls, None)
                .map_err(|error| CommandError::SystemError(error.to_string()))
        }
    }
}

pub(super) fn show_properties(paths: &[String]) -> Result<(), CommandError> {
    use windows::core::{PCSTR, PSTR};
    use windows::Win32::UI::Shell::{CMINVOKECOMMANDINFO, GCS_VERBA, SEE_MASK_NOASYNC};
    use windows::Win32::UI::WindowsAndMessaging::{CreatePopupMenu, DestroyMenu, SW_SHOWNORMAL};
    if paths.is_empty() {
        return Err(CommandError::SystemError("Empty Shell selection".into()));
    }
    let _com = super::windows_com::ComApartment::sta()
        .map_err(|error| CommandError::SystemError(error.to_string()))?;
    let _controls = controls::Controls::new().map_err(CommandError::SystemError)?;
    let shell_thread = thread::ShellThread::new().map_err(CommandError::SystemError)?;
    let menu: IContextMenu = properties_ui_object(paths)?;
    unsafe {
        let popup =
            CreatePopupMenu().map_err(|error| CommandError::SystemError(error.to_string()))?;
        let queried = menu.QueryContextMenu(popup, 0, 1, 0x7fff, 0);
        let result = (|| {
            queried
                .ok()
                .map_err(|error| CommandError::SystemError(error.to_string()))?;
            let count = queried.0 as u32 & 0xffff;
            let offset = (0..count)
                .find(|offset| {
                    let mut buffer = [0u8; 128];
                    menu.GetCommandString(
                        *offset as usize,
                        GCS_VERBA,
                        None,
                        PSTR(buffer.as_mut_ptr()),
                        buffer.len() as u32,
                    )
                    .is_ok()
                        && std::ffi::CStr::from_bytes_until_nul(&buffer)
                            .is_ok_and(|verb| verb.to_bytes() == b"properties")
                })
                .ok_or_else(|| {
                    CommandError::SystemError("No properties command for the selection".into())
                })?;
            let info = CMINVOKECOMMANDINFO {
                cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
                fMask: SEE_MASK_NOASYNC,
                lpVerb: PCSTR(offset as usize as *const u8),
                nShow: SW_SHOWNORMAL.0,
                ..Default::default()
            };
            menu.InvokeCommand(&info)
                .map_err(|error| CommandError::SystemError(error.to_string()))
        })();
        let _ = DestroyMenu(popup);
        drop(menu);
        shell_thread.wait();
        result
    }
}
