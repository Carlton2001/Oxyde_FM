//! A folder opened from inside an Explorer window stays in Explorer. Once Oxyde owns
//! the `open` verb of folders, Explorer runs it for a double click in its own list too,
//! instead of browsing in place; whoever uses Explorer would be sent to Oxyde at each
//! folder. The launcher Explorer started therefore asks the Explorer tab the folder
//! came from to browse to it. Opening a folder in a list selects it, so that tab is an
//! active one where the folder is selected: the foreground window's, or the only one
//! when another window took the foreground meanwhile. A folder Explorer opens for the
//! Run dialog, the Start menu or a jump list is selected nowhere and is Oxyde's,
//! whatever Explorer window is in the foreground. Opened among other selected items,
//! the folder gets an Explorer window of its own, as Explorer gives each of them.
use super::windows_com::ComApartment;
use windows::core::{w, Interface, HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, IServiceProvider, CLSCTX_LOCAL_SERVER};
use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    BHID_LinkTargetItem, IShellBrowser, IShellItem, IShellItemArray, IShellWindows, SHParseDisplayName, ShellWindows, SBSP_ABSOLUTE,
    SBSP_SAMEBROWSER, SID_STopLevelBrowser, SIGDN_FILESYSPATH, SVGIO_SELECTION,
};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowExW, GetAncestor, GetClassNameW, GetForegroundWindow, GA_ROOT};

const EXPLORER_TAB: PCWSTR = w!("ShellTabWindowClass");
/// The desktop and the taskbar, which are Explorer's too: a folder opened there is
/// Oxyde's, even when an Explorer window has it selected.
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

/// How a tab has the folder selected.
enum Selected {
    Alone,
    /// With other items, each opened with it.
    Among,
}

/// The folder a selected item opens: itself, or the target of a shortcut.
fn opened(item: &IShellItem) -> Option<String> {
    let path = |item: &IShellItem| unsafe {
        let name = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let path = name.to_string().ok();
        CoTaskMemFree(Some(name.0.cast()));
        path
    };
    let own = path(item)?;
    if own.to_ascii_lowercase().ends_with(".lnk") {
        return unsafe { item.BindToHandler::<_, IShellItem>(None, &BHID_LinkTargetItem) }.ok().and_then(|target| path(&target));
    }
    Some(own)
}

/// Whether `folder` is among the items selected in the tab, and whether alone.
fn selects(browser: &IShellBrowser, folder: &str) -> Option<Selected> {
    unsafe {
        let items: IShellItemArray = browser.QueryActiveShellView().ok()?.GetItemObject(SVGIO_SELECTION).ok()?;
        let count = items.GetCount().ok()?;
        (0..count.min(SELECTION_LIMIT))
            .filter_map(|index| items.GetItemAt(index).ok())
            .any(|item| opened(&item).is_some_and(|path| same_folder(&path, folder)))
            .then_some(if count == 1 { Selected::Alone } else { Selected::Among })
    }
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

/// Which of the windows whose active tab selects the folder it came from: the
/// foreground one, else the only one.
fn origin<T: PartialEq>(windows: &[T], foreground: &T) -> Option<usize> {
    windows.iter().position(|window| window == foreground).or((windows.len() == 1).then_some(0))
}

/// A new Explorer window on `folder`. Named on Explorer's command line, a folder does
/// not go through the verb Oxyde replaced.
fn open_window(folder: &str) -> std::io::Result<()> {
    let root = std::env::var_os("SystemRoot").ok_or_else(|| std::io::Error::other("SystemRoot is not set"))?;
    std::process::Command::new(std::path::Path::new(&root).join("explorer.exe")).arg(folder).spawn().map(drop)
}

/// Keeps `folder` in Explorer when it was opened from one of its tabs.
fn keep(folder: &str, foreground: HWND) -> Result<bool, Box<dyn std::error::Error>> {
    let mut tabs: Vec<(Tab, Selected)> = active_tabs()?.into_iter()
        .filter_map(|tab| { let selected = selects(&tab.browser, folder)?; Some((tab, selected)) })
        .collect();
    let windows: Vec<HWND> = tabs.iter().map(|(tab, _)| tab.window).collect();
    let Some(index) = origin(&windows, &foreground) else { return Ok(false) };
    match tabs.swap_remove(index) {
        (tab, Selected::Alone) => browse(&tab.browser, folder)?,
        (_, Selected::Among) => open_window(folder)?,
    }
    Ok(true)
}

/// Shows `folder` in the Explorer tab it was opened from; false when it was opened
/// from anywhere else, or when Explorer does not answer, and the folder is then Oxyde's.
pub(super) fn browse_where_opened(folder: &str) -> bool {
    let foreground = unsafe { GetForegroundWindow() };
    // Another program opening a folder is its parent, whatever Explorer has selected.
    if DESKTOP_WINDOWS.contains(&class_name(foreground).as_str()) || !started_by_explorer() {
        return false;
    }
    let Ok(_apartment) = ComApartment::sta() else { return false };
    keep(folder, foreground).unwrap_or_else(|error| {
        log::warn!("Cannot keep {folder} in the Explorer window: {error}");
        false
    })
}

#[cfg(test)]
#[path = "windows_explorer_browse/tests/folders.rs"]
mod tests;
