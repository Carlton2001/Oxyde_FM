//! Find and invoke the Shell's New submenu in the background menu of a folder.

use super::{canonical_verb, invoke_menu_with, popup_id, query_menu, with_context_menu};
use crate::models::{CommandError, WinMenuItem};
use image::RgbaImage;
use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::HBITMAP;
use windows::Win32::UI::Shell::{
    CMF_CANRENAME, CMF_EXPLORE, CMF_NORMAL, IContextMenu, IContextMenu2, IContextMenu3,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CopyImage, GetMenuItemCount, GetMenuItemInfoW, GetMenuStringW, GetSubMenu, HMENU, IMAGE_BITMAP,
    LR_CREATEDIBSECTION, MENUITEMINFOW, MF_BYPOSITION, MFS_DISABLED, MIIM_BITMAP, MIIM_ID,
    MIIM_STATE, WM_INITMENUPOPUP,
};
use windows::core::Interface;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellNewCommand {
    pub label: String,
    verb: Option<String>,
}

pub struct ShellNewEntry {
    pub command: ShellNewCommand,
    pub extension: Option<String>,
    /// Native BGRA pixels, when Windows publishes a bitmap for the command.
    pub icon: Option<RgbaImage>,
}

fn label(menu: HMENU, index: u32) -> String {
    let mut buffer = [0u16; 512];
    let length = unsafe { GetMenuStringW(menu, index, Some(&mut buffer), MF_BYPOSITION) };
    clean_label(&String::from_utf16_lossy(&buffer[..length.max(0) as usize]))
}

fn clean_label(label: &str) -> String {
    let mut result = String::new();
    let mut chars = label.split('\t').next().unwrap_or_default().trim().chars();
    while let Some(character) = chars.next() {
        if character != '&' {
            result.push(character);
        } else if chars.as_str().starts_with('&') {
            result.push('&');
            chars.next();
        }
    }
    result
}

fn folder_label(label: &str, system_label: Option<&str>) -> bool {
    system_label.is_some_and(|system| clean_label(system) == label)
        || matches!(
            label.to_lowercase().as_str(),
            "dossier" | "folder" | "ordner" | "carpeta" | "cartella"
        )
}

pub(super) fn new_label(label: &str) -> bool {
    matches!(
        label.to_lowercase().as_str(),
        "nouveau" | "new" | "neu" | "nuevo" | "nuovo"
    )
}

pub(in crate::platform) fn remove_new_submenu(items: &mut Vec<WinMenuItem>) {
    let index = items.iter().position(|item| {
        item.has_submenu
            && (item
                .verb
                .as_deref()
                .is_some_and(|verb| verb.eq_ignore_ascii_case("new"))
                || new_label(item.label.trim()))
    });
    let index = index.or_else(|| {
        let folder = clean_label(&super::super::windows_shell_new::folder_menu_label()?);
        unique(items.iter().enumerate().filter_map(|(index, item)| {
            (item.has_submenu
                && item
                    .children
                    .first()
                    .is_some_and(|child| child.label == folder))
            .then_some(index)
        }))
    });
    if let Some(index) = index {
        items.remove(index);
    }
}

fn init_submenu(
    submenu: HMENU,
    index: i32,
    cm2: Option<&IContextMenu2>,
    cm3: Option<&IContextMenu3>,
) {
    let wparam = WPARAM(submenu.0 as usize);
    let lparam = LPARAM((index & 0xFFFF) as isize);
    unsafe {
        if let Some(cm) = cm2 {
            let _ = cm.HandleMenuMsg(WM_INITMENUPOPUP, wparam, lparam);
        } else if let Some(cm) = cm3 {
            let _ = cm.HandleMenuMsg(WM_INITMENUPOPUP, wparam, lparam);
        }
    }
}

fn with_new_submenu<T>(
    folder: &str,
    read: impl FnOnce(&IContextMenu, HMENU, Option<&str>) -> T,
) -> Result<Option<T>, CommandError> {
    with_context_menu(&[folder.to_owned()], true, |context| {
        let menu = query_menu(context, CMF_NORMAL | CMF_EXPLORE | CMF_CANRENAME)?;
        let cm2 = context.cast::<IContextMenu2>().ok();
        let cm3 = context.cast::<IContextMenu3>().ok();
        let system_folder_label = super::super::windows_shell_new::folder_menu_label();
        let submenus = (0..unsafe { GetMenuItemCount(Some(menu.0)) }).filter_map(|index| {
            let submenu = unsafe { GetSubMenu(menu.0, index) };
            (!submenu.is_invalid()).then_some((index, submenu))
        });
        let named = submenus.clone().find(|(index, _)| {
            popup_id(menu.0, *index)
                .and_then(|id| canonical_verb(context, id))
                .is_some_and(|verb| verb.eq_ignore_ascii_case("new"))
                || new_label(&label(menu.0, *index as u32))
        });
        if let Some((index, submenu)) = named {
            init_submenu(submenu, index, cm2.as_ref(), cm3.as_ref());
            return Ok(Some(read(context, submenu, system_folder_label.as_deref())));
        }

        // Some Windows languages expose only a translated caption for New. Its
        // first command is the standard folder entry when creation is available.
        let Some(system_folder_label) = system_folder_label.as_deref() else {
            return Ok(None);
        };
        let found = submenus.filter_map(|(index, submenu)| {
            init_submenu(submenu, index, cm2.as_ref(), cm3.as_ref());
            (label(submenu, 0) == clean_label(system_folder_label)).then_some(submenu)
        });
        Ok(unique(found).map(|submenu| read(context, submenu, Some(system_folder_label))))
    })
}

fn unique<T>(mut items: impl Iterator<Item = T>) -> Option<T> {
    let first = items.next()?;
    items.next().is_none().then_some(first)
}

fn commands(
    context: &IContextMenu,
    menu: HMENU,
    icons: bool,
) -> impl Iterator<Item = (u32, ShellNewCommand, HBITMAP)> + '_ {
    (0..unsafe { GetMenuItemCount(Some(menu)) }).filter_map(move |index| {
        let mut info = MENUITEMINFOW {
            cbSize: size_of::<MENUITEMINFOW>() as u32,
            fMask: MIIM_ID | MIIM_STATE,
            ..Default::default()
        };
        if icons {
            info.fMask |= MIIM_BITMAP;
        }
        unsafe { GetMenuItemInfoW(menu, index as u32, true, &mut info) }.ok()?;
        if !(1..=0x7FFF).contains(&info.wID)
            || (info.fState & MFS_DISABLED).0 != 0
            || !unsafe { GetSubMenu(menu, index) }.is_invalid()
        {
            return None;
        }
        let label = label(menu, index as u32);
        if label.is_empty() {
            return None;
        }
        let verb = canonical_verb(context, info.wID);
        Some((info.wID, ShellNewCommand { label, verb }, info.hbmpItem))
    })
}

fn bitmap_image(bitmap: HBITMAP) -> Option<RgbaImage> {
    if bitmap.is_invalid() {
        return None;
    }
    // CopyImage gives the conversion function ownership of a copy. The original
    // belongs to the menu, and HBMMENU_CALLBACK values are not bitmap handles.
    let copy =
        unsafe { CopyImage(HANDLE(bitmap.0), IMAGE_BITMAP, 0, 0, LR_CREATEDIBSECTION) }.ok()?;
    let pixels = super::super::windows_thumbnail_bitmap::into_image(HBITMAP(copy.0)).ok()?;
    pixels.pixels().any(|pixel| pixel[3] != 0).then_some(pixels)
}

pub(in crate::platform) fn entries(
    folder: &str,
    icons: bool,
) -> Result<Vec<ShellNewEntry>, CommandError> {
    let mut entries: Vec<_> = with_new_submenu(folder, |context, menu, system_folder_label| {
        commands(context, menu, icons)
            .filter(|(_, command, _)| !folder_label(&command.label, system_folder_label))
            .map(|(_, command, bitmap)| ShellNewEntry {
                command,
                extension: None,
                icon: bitmap_image(bitmap),
            })
            .collect()
    })?
    .unwrap_or_default();
    if entries.iter().any(|entry| entry.icon.is_none()) {
        let types = super::super::windows_shell_new::icon_types();
        for entry in entries.iter_mut().filter(|entry| entry.icon.is_none()) {
            entry.extension = types.extension(&entry.command.label).map(str::to_owned);
        }
    }
    Ok(entries)
}

pub(in crate::platform) fn execute(
    folder: &str,
    chosen: &ShellNewCommand,
) -> Result<(), CommandError> {
    let unavailable =
        || CommandError::SystemError("The Windows New command is no longer available".into());
    with_new_submenu(folder, |context, menu, _| {
        // IDs are assigned afresh on every QueryContextMenu and can move when
        // registrations change. Match inside this submenu before invoking.
        let id = unique(
            commands(context, menu, false)
                .filter_map(|(id, command, _)| (&command == chosen).then_some(id)),
        )
        .ok_or_else(unavailable)?;
        invoke_menu_with(id, HWND::default(), |info| unsafe {
            context.InvokeCommand(info)
        })
    })?
    .ok_or_else(unavailable)?
}

#[cfg(test)]
#[path = "tests/new_menu.rs"]
mod tests;
