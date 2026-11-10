//! A folder opened from inside an Explorer window stays in Explorer. Once Oxyde owns
//! the `open` verb of folders, Explorer runs it for a double click in its own list too,
//! instead of browsing in place; whoever uses Explorer would be sent to Oxyde at each
//! folder. The launcher Explorer started therefore asks the Explorer tab the folder
//! came from to browse to it: the active tab of the foreground window, or, when another
//! window took the foreground meanwhile, the only active tab where the folder is
//! selected.
use super::windows_com::ComApartment;
use windows::core::{w, Interface, HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, IServiceProvider, CLSCTX_LOCAL_SERVER};
use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    IShellBrowser, IShellItemArray, IShellWindows, SHParseDisplayName, ShellWindows, SBSP_ABSOLUTE, SBSP_SAMEBROWSER, SID_STopLevelBrowser,
    SIGDN_FILESYSPATH, SVGIO_SELECTION,
};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowExW, GetAncestor, GetClassNameW, GetForegroundWindow, GA_ROOT};

/// Explorer's file windows.
const EXPLORER_WINDOW: &str = "CabinetWClass";
const EXPLORER_TAB: PCWSTR = w!("ShellTabWindowClass");
/// The desktop and the taskbar, which are Explorer's too: a folder opened there is Oxyde's.
const DESKTOP_WINDOWS: [&str; 4] = ["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd"];
/// Selections longer than this are not searched; the Shell starts one launch per folder.
const SELECTION_LIMIT: u32 = 256;

fn class_name(window: HWND) -> String {
    let mut name = [0u16; 64];
    let length = unsafe { GetClassNameW(window, &mut name) };
    String::from_utf16_lossy(&name[..length.max(0) as usize])
}

/// An active Explorer tab and the window it belongs to.
struct Tab {
    browser: IShellBrowser,
    window: HWND,
}

/// The active tab of each Explorer window. Every tab is a child of its top window and
/// reads as visible; the active one is the first in their order.
fn active_tabs() -> windows::core::Result<Vec<Tab>> {
    let windows: IShellWindows = unsafe { CoCreateInstance(&ShellWindows, None, CLSCTX_LOCAL_SERVER) }?;
    let mut tabs = Vec::new();
    for index in 0..unsafe { windows.Count() }? {
        let Ok(item) = (unsafe { windows.Item(&VARIANT::from(index)) }) else { continue };
        let Ok(provider) = item.cast::<IServiceProvider>() else { continue };
        let Ok(browser) = (unsafe { provider.QueryService::<IShellBrowser>(&SID_STopLevelBrowser) }) else { continue };
        let Ok(tab) = (unsafe { browser.GetWindow() }) else { continue };
        let window = unsafe { GetAncestor(tab, GA_ROOT) };
        if unsafe { FindWindowExW(Some(window), None, EXPLORER_TAB, None) }.is_ok_and(|active| active == tab) {
            tabs.push(Tab { browser, window });
        }
    }
    Ok(tabs)
}

/// Whether `folder` is among the items selected in the tab.
fn selects(browser: &IShellBrowser, folder: &str) -> bool {
    let selected = || -> windows::core::Result<bool> {
        unsafe {
            let items: IShellItemArray = browser.QueryActiveShellView()?.GetItemObject(SVGIO_SELECTION)?;
            for index in 0..items.GetCount()?.min(SELECTION_LIMIT) {
                let Ok(name) = items.GetItemAt(index)?.GetDisplayName(SIGDN_FILESYSPATH) else { continue };
                let path = name.to_string();
                CoTaskMemFree(Some(name.0.cast()));
                if path.is_ok_and(|path| same_folder(&path, folder)) {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    };
    selected().unwrap_or(false)
}

fn same_folder(left: &str, right: &str) -> bool {
    let plain = |path: &str| path.trim_end_matches('\\').to_lowercase();
    plain(left) == plain(right)
}

/// Whether Explorer started this process, as it does for a verb run from its windows.
fn started_by_explorer() -> bool {
    let own = std::process::id();
    let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else { return false };
    let mut processes = Vec::new();
    let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
    let mut more = unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok();
    while more {
        let length = entry.szExeFile.iter().position(|unit| *unit == 0).unwrap_or(entry.szExeFile.len());
        processes.push((entry.th32ProcessID, entry.th32ParentProcessID, String::from_utf16_lossy(&entry.szExeFile[..length])));
        more = unsafe { Process32NextW(snapshot, &mut entry) }.is_ok();
    }
    unsafe { let _ = CloseHandle(snapshot); }
    let parent = processes.iter().find(|(id, _, _)| *id == own).map(|(_, parent, _)| *parent);
    processes.iter().any(|(id, _, name)| Some(*id) == parent && name.eq_ignore_ascii_case("explorer.exe"))
}

fn browse(browser: &IShellBrowser, folder: &str) -> windows::core::Result<()> {
    let mut item: *mut ITEMIDLIST = std::ptr::null_mut();
    unsafe {
        SHParseDisplayName(PCWSTR(HSTRING::from(folder).as_ptr()), None, &mut item, 0, None)?;
        let browsed = browser.BrowseObject(item, (SBSP_SAMEBROWSER | SBSP_ABSOLUTE) as u32);
        CoTaskMemFree(Some(item.cast()));
        browsed
    }
}

/// The tab `folder` was opened from, if it was opened from one.
fn origin(folder: &str, foreground: HWND, foreground_class: &str) -> windows::core::Result<Option<IShellBrowser>> {
    // Another program opening a folder is its parent, whatever is in the foreground.
    if !started_by_explorer() {
        return Ok(None);
    }
    let mut tabs = active_tabs()?.into_iter();
    if foreground_class == EXPLORER_WINDOW {
        return Ok(tabs.find(|tab| tab.window == foreground).map(|tab| tab.browser));
    }
    let mut selecting = tabs.filter(|tab| selects(&tab.browser, folder));
    Ok(match (selecting.next(), selecting.next()) {
        (Some(tab), None) => Some(tab.browser),
        _ => None,
    })
}

/// Shows `folder` in the Explorer tab it was opened from; false when it was opened
/// from anywhere else, or when Explorer does not answer, and the folder is then Oxyde's.
pub(super) fn browse_where_opened(folder: &str) -> bool {
    let foreground = unsafe { GetForegroundWindow() };
    let foreground_class = class_name(foreground);
    if DESKTOP_WINDOWS.contains(&foreground_class.as_str()) {
        return false;
    }
    let Ok(_apartment) = ComApartment::sta() else { return false };
    match origin(folder, foreground, &foreground_class).and_then(|browser| browser.map(|browser| browse(&browser, folder)).transpose()) {
        Ok(Some(())) => true,
        Ok(None) => false,
        Err(error) => {
            log::warn!("Cannot browse the Explorer window to {folder}: {error}");
            false
        }
    }
}

#[cfg(test)]
#[path = "windows_explorer_browse/tests/folders.rs"]
mod tests;
