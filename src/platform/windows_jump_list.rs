//! The menu of Oxyde's taskbar button: the pinned folders, each opened as a tab, and a
//! task that starts Oxyde as administrator. The list belongs to an application
//! identity, the one the Setup gives its shortcuts: the process takes it at start, so
//! that the button and its menu are the same however Oxyde was started. Entries are
//! shortcuts to this program; a shortcut cannot ask for elevation, so the task runs
//! Oxyde with a switch that makes it start itself elevated.
//!
//! Windows refuses an application's own categories when the user turned off recently
//! opened items in Start and jump lists; tasks are not concerned. The pinned folders
//! are then listed as tasks, above the one that starts Oxyde as administrator.
use super::windows_com::ComApartment;
use super::TaskbarMenu;
use windows::core::{w, Interface, HSTRING, PCWSTR};
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Shell::Common::{IObjectArray, IObjectCollection};
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, PSCoerceToCanonicalValue};
use windows::Win32::UI::Shell::{
    DestinationList, EnumerableObjectCollection, ICustomDestinationList, IShellLinkW, SHGetStockIconInfo,
    SetCurrentProcessExplicitAppUserModelID, ShellExecuteW, ShellLink,
    SHGSI_ICONLOCATION, SHSTOCKICONID, SHSTOCKICONINFO, SIID_FOLDER, SIID_SHIELD,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// The switch of the task: Oxyde starts itself again, elevated.
pub const AS_ADMINISTRATOR: &str = "--run-as-administrator";
/// The identity of the Setup's shortcuts (`AppUserModelID` in `oxyde.iss`); a debug
/// build keeps a button and a menu of its own.
const IDENTITY: &str = if cfg!(debug_assertions) { "Oxyde.GPUI.Windows.Devel" } else { "Oxyde.GPUI.Windows" };
/// More folders than the menu shows are left out by the system; these are sent at most.
const FOLDERS: usize = 20;
/// The title a shortcut shows in the menu.
const TITLE: PROPERTYKEY = PROPERTYKEY { fmtid: windows::core::GUID::from_u128(0xf29f85e0_4ff9_1068_ab91_08002b27b3d9), pid: 2 };
/// What makes a shortcut a line between tasks.
const SEPARATOR: PROPERTYKEY = PROPERTYKEY { fmtid: windows::core::GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3), pid: 6 };

/// The command line that opens `folder`. A root ends with a separator, which would
/// escape the closing quote: Oxyde reads `C:"` as the root it was.
fn arguments(folder: &str) -> String {
    format!("\"{folder}\"")
}

/// The folders to list: those with a path Oxyde can be started on, without the ones
/// the user removed from the menu, which the system refuses to list again.
fn listed<'a>(pinned: &'a [(String, String)], removed: &[String]) -> Vec<&'a (String, String)> {
    pinned.iter()
        .filter(|(_, path)| std::path::Path::new(path).is_absolute())
        .filter(|(_, path)| !removed.iter().any(|removed| removed.eq_ignore_ascii_case(&arguments(path))))
        .take(FOLDERS)
        .collect()
}

struct Program(HSTRING);

impl Program {
    fn shortcut(&self, arguments: &str, title: &str, icon: SHSTOCKICONID) -> windows::core::Result<IShellLinkW> {
        unsafe {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            link.SetPath(&self.0)?;
            link.SetArguments(&HSTRING::from(arguments))?;
            let mut stock = SHSTOCKICONINFO { cbSize: std::mem::size_of::<SHSTOCKICONINFO>() as u32, ..Default::default() };
            if SHGetStockIconInfo(icon, SHGSI_ICONLOCATION, &mut stock).is_ok() {
                link.SetIconLocation(PCWSTR(stock.szPath.as_ptr()), stock.iIcon)?;
            }
            let mut value = PROPVARIANT::from(title);
            PSCoerceToCanonicalValue(&TITLE, &mut value)?;
            let properties: IPropertyStore = link.cast()?;
            properties.SetValue(&TITLE, &value)?;
            properties.Commit()?;
            Ok(link)
        }
    }
}

fn separator() -> windows::core::Result<IShellLinkW> {
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        let properties: IPropertyStore = link.cast()?;
        properties.SetValue(&SEPARATOR, &PROPVARIANT::from(true))?;
        properties.Commit()?;
        Ok(link)
    }
}

fn collection() -> windows::core::Result<IObjectCollection> {
    unsafe { CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER) }
}

/// The command lines of the shortcuts the user removed since the last list.
fn removed(shortcuts: &IObjectArray) -> Vec<String> {
    unsafe {
        (0..shortcuts.GetCount().unwrap_or(0)).filter_map(|index| {
            let link: IShellLinkW = shortcuts.GetAt(index).ok()?;
            let mut arguments = [0u16; 1024];
            link.GetArguments(&mut arguments).ok()?;
            let length = arguments.iter().position(|unit| *unit == 0).unwrap_or(arguments.len());
            Some(String::from_utf16_lossy(&arguments[..length]))
        }).collect()
    }
}

/// Gives the process its identity. Before its first window, which takes it.
pub(super) fn identify() {
    if let Err(error) = unsafe { SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(IDENTITY)) } {
        log::warn!("Cannot set the application identity: {error}");
    }
}

pub(super) fn set(menu: &TaskbarMenu) -> windows::core::Result<()> {
    write(menu, IDENTITY)
}

fn write(menu: &TaskbarMenu, identity: &str) -> windows::core::Result<()> {
    let _apartment = ComApartment::sta_or_existing()?;
    let program = std::env::current_exe().map_err(|error| windows::core::Error::new(windows::Win32::Foundation::E_FAIL, error.to_string()))?;
    let program = Program(HSTRING::from(program.as_os_str()));
    unsafe {
        let list: ICustomDestinationList = CoCreateInstance(&DestinationList, None, CLSCTX_INPROC_SERVER)?;
        list.SetAppID(&HSTRING::from(identity))?;
        let mut shown = 0u32;
        let removed = removed(&list.BeginList::<IObjectArray>(&mut shown)?);
        let folders = listed(&menu.pinned, &removed).into_iter()
            .map(|(name, path)| program.shortcut(&arguments(path), name, SIID_FOLDER))
            .collect::<windows::core::Result<Vec<_>>>()?;
        let category = collection()?;
        for folder in &folders {
            category.AddObject(folder)?;
        }
        let tasks = collection()?;
        if !folders.is_empty() {
            if let Err(error) = list.AppendCategory(&HSTRING::from(menu.category.as_str()), &category.cast::<IObjectArray>()?) {
                // Recently opened items are turned off: only tasks are listed.
                log::info!("Pinned folders listed as taskbar tasks: {error}");
                for folder in &folders {
                    tasks.AddObject(folder)?;
                }
                tasks.AddObject(&separator()?)?;
            }
        }
        tasks.AddObject(&program.shortcut(AS_ADMINISTRATOR, &menu.administrator, SIID_SHIELD)?)?;
        list.AddUserTasks(&tasks.cast::<IObjectArray>()?)?;
        list.CommitList()
    }
}

/// Starts this program again as administrator, after the user's consent.
pub(super) fn start_as_administrator() -> std::io::Result<()> {
    let program = HSTRING::from(std::env::current_exe()?.as_os_str());
    let started = unsafe { ShellExecuteW(None, w!("runas"), &program, None, None, SW_SHOWNORMAL) };
    // Above 32 the program started; anything else is an error, a refused consent included.
    if started.0 as usize > 32 { Ok(()) } else { Err(std::io::Error::last_os_error()) }
}

#[cfg(test)]
#[path = "windows_jump_list/tests/folders.rs"]
mod tests;
