//! A background menu has no selection. Use the displayed folder as the Shell item for sharing.

use super::{invoke_menu_with, with_menu_snapshot};
use crate::models::{CommandError, WinMenuItem};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::IContextMenu;

const SHARING_VERB: &str = "Windows.Share";

/// Reads the background menu while a second STA thread reads the displayed
/// folder's sharing submenu, so the menu waits only for the slower query.
pub(in crate::platform) fn read_beside_folder_sharing(
    paths: &[String],
    read_background: impl FnOnce() -> Result<Vec<WinMenuItem>, CommandError>,
) -> Result<Vec<WinMenuItem>, CommandError> {
    // The scope joins the thread on every return: a Shell extension that hangs
    // holds this worker, which the bound on native menu loads already counts.
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("oxyde-folder-sharing".into())
            .spawn_scoped(scope, || folder_sharing_submenu(paths));
        let mut items = read_background()?;
        let sharing = match worker {
            Ok(worker) => worker.join().ok().flatten(),
            Err(_) => folder_sharing_submenu(paths),
        };
        place_folder_sharing_submenu(&mut items, sharing);
        Ok(items)
    })
}

fn folder_sharing_submenu(paths: &[String]) -> Option<WinMenuItem> {
    let mut sharing = with_folder_sharing(paths, |_, sharing| Ok(sharing))
        .ok()
        .flatten()?;
    mark_sharing_commands(&mut sharing.children);
    Some(sharing)
}

/// The background lists sharing as an empty popup: the folder's submenu takes
/// its place, or the popup goes when the folder offers no sharing command.
fn place_folder_sharing_submenu(items: &mut Vec<WinMenuItem>, sharing: Option<WinMenuItem>) {
    let position = items.iter().position(|item| {
        item.has_submenu
            && item
                .verb
                .as_deref()
                .is_some_and(|verb| verb.eq_ignore_ascii_case(SHARING_VERB))
    });
    match (position, sharing) {
        (Some(index), _) if !items[index].children.is_empty() => {}
        (Some(index), Some(sharing)) => items[index] = sharing,
        (Some(index), None) => {
            items.remove(index);
        }
        (None, Some(sharing)) => items.push(sharing),
        (None, None) => {}
    }
}

fn mark_sharing_commands(items: &mut [WinMenuItem]) {
    for item in items {
        if item.has_submenu {
            mark_sharing_commands(&mut item.children);
        } else {
            item.is_folder_sharing_command = true;
        }
    }
}

pub(in crate::platform) fn execute_folder_sharing_command(
    paths: &[String],
    command: &WinMenuItem,
) -> Result<(), CommandError> {
    with_folder_sharing_command(paths, command, |menu, id| {
        invoke_menu_with(id, HWND::default(), |info| unsafe {
            menu.InvokeCommand(info)
        })
    })
}

fn with_folder_sharing<T>(
    paths: &[String],
    read: impl FnOnce(&IContextMenu, WinMenuItem) -> Result<T, CommandError>,
) -> Result<Option<T>, CommandError> {
    let [folder] = paths else { return Ok(None) };
    if !super::super::windows_shell_selection::shell_path(folder)?.is_dir() {
        return Ok(None);
    }
    with_menu_snapshot(paths, false, Some(SHARING_VERB), true, |menu, mut items| {
        if items.len() == 1 {
            read(menu, items.remove(0)).map(Some)
        } else {
            Ok(None)
        }
    })
}

fn with_folder_sharing_command<T>(
    paths: &[String],
    command: &WinMenuItem,
    invoke: impl FnOnce(&IContextMenu, u32) -> Result<T, CommandError>,
) -> Result<T, CommandError> {
    let missing = || {
        CommandError::SystemError(
            "Windows no longer offers this sharing command for the folder".into(),
        )
    };
    with_folder_sharing(paths, |menu, sharing| {
        // The folder selection has its own command IDs, and those can change on each query.
        let id = matching_command_id(&sharing.children, command).ok_or_else(missing)?;
        invoke(menu, id)
    })?
    .ok_or_else(missing)
}

fn matching_command_id(items: &[WinMenuItem], command: &WinMenuItem) -> Option<u32> {
    fn collect(items: &[WinMenuItem], command: &WinMenuItem, matches: &mut Vec<u32>) {
        for item in items {
            if item.has_submenu {
                collect(&item.children, command, matches);
            } else if item.label == command.label && item.verb == command.verb {
                matches.push(item.id as u32);
            }
        }
    }
    let mut matches = Vec::new();
    collect(items, command, &mut matches);
    match matches.as_slice() {
        [id] => Some(*id),
        _ => None,
    }
}

#[cfg(test)]
#[path = "tests/sharing.rs"]
mod tests;
