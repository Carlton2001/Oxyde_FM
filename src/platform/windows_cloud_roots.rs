//! Discover the current user's registered cloud folders without scanning files.
use super::windows_registry::{open, string, subkey_names};
use super::windows_user_token::current_user_sid;
use crate::models::QuickAccessItem;
use std::path::Path;
use windows::Win32::Globalization::{CSTR_EQUAL, CompareStringOrdinal};
use windows::Win32::System::Registry::{HKEY, HKEY_LOCAL_MACHINE};
use windows::Win32::UI::Shell::SHLoadIndirectString;
use windows::core::{HSTRING, PCWSTR, w};

const SYNC_ROOT_MANAGER: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\SyncRootManager";

pub(super) fn folders() -> Vec<QuickAccessItem> {
    let Some(sid) = current_user_sid() else {
        return Vec::new();
    };
    let Some(manager) = open(HKEY_LOCAL_MACHINE, SYNC_ROOT_MANAGER) else {
        return Vec::new();
    };
    let sid = HSTRING::from(sid);
    let mut roots: Vec<_> = subkey_names(manager.0)
        .into_iter()
        .filter_map(|registration| folder(manager.0, &registration, &sid))
        .collect();
    roots.sort_by_cached_key(|item| (item.name.to_lowercase(), item.path.clone()));

    // Several registrations can point to the same folder. Retain the first
    // label in display order so the sidebar is stable across refreshes.
    let mut unique: Vec<QuickAccessItem> = Vec::with_capacity(roots.len());
    for root in roots {
        if !unique.iter().any(|item| same_path(&item.path, &root.path)) {
            unique.push(root);
        }
    }
    unique
}

fn folder(manager: HKEY, registration: &str, sid: &HSTRING) -> Option<QuickAccessItem> {
    // The desktop registry is shared across users. The SID value is the
    // filesystem path for this user, even when a registration has other users.
    let user_roots = open(manager, &format!(r"{registration}\UserSyncRoots"))?;
    let path = string(user_roots.0, PCWSTR(sid.as_ptr()))?;
    let directory = Path::new(&path);
    if !directory.is_absolute() || !directory.is_dir() {
        return None;
    }
    let name = open(manager, registration)
        .and_then(|root| string(root.0, w!("DisplayNameResource")))
        .and_then(display_name)
        .or_else(|| directory.file_name()?.to_str().map(str::to_owned))
        .unwrap_or_else(|| path.clone());
    Some(QuickAccessItem { name, path })
}

fn display_name(resource: String) -> Option<String> {
    if !resource.starts_with('@') {
        return (!resource.trim().is_empty()).then_some(resource);
    }
    let resource = HSTRING::from(resource);
    let mut buffer = [0u16; 1024];
    unsafe { SHLoadIndirectString(PCWSTR(resource.as_ptr()), &mut buffer, None) }.ok()?;
    let end = buffer.iter().position(|&unit| unit == 0)?;
    let name = String::from_utf16(&buffer[..end]).ok()?;
    (!name.trim().is_empty()).then_some(name)
}

fn same_path(left: &str, right: &str) -> bool {
    let wide_path = |path: &str| {
        path.trim_end_matches(['\\', '/'])
            .replace('/', "\\")
            .encode_utf16()
            .collect::<Vec<_>>()
    };
    unsafe { CompareStringOrdinal(&wide_path(left), &wide_path(right), true) == CSTR_EQUAL }
}
