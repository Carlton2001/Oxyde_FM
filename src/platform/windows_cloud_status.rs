//! Read the states reported by Cloud Files and Google Drive's Shell overlays.
use super::windows_com::ComApartment;
use super::windows_onedrive::{open, string, subkey_names};
use super::CloudStatusProbe;
use crate::models::CloudStatus;
use std::os::windows::{ffi::OsStrExt, fs::MetadataExt};
use std::path::{Component, Path, Prefix};
use std::sync::OnceLock;
use windows::core::{s, w, Interface, HSTRING, PCWSTR};
use windows::Win32::Foundation::{FreeLibrary, S_OK};
use windows::Win32::Storage::CloudFilters::{
    CF_PLACEHOLDER_STATE, CF_PLACEHOLDER_STATE_INVALID, CF_PLACEHOLDER_STATE_IN_SYNC,
    CF_PLACEHOLDER_STATE_PARTIALLY_ON_DISK, CF_PLACEHOLDER_STATE_PLACEHOLDER,
};
use windows::Win32::Storage::FileSystem::{
    FindClose, FindFirstFileW, GetVolumeInformationW, GetVolumePathNameW, FILE_ATTRIBUTE_OFFLINE,
    FILE_ATTRIBUTE_PINNED, FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS, FILE_ATTRIBUTE_RECALL_ON_OPEN,
    FILE_ATTRIBUTE_REPARSE_POINT, WIN32_FIND_DATAW,
};
use windows::Win32::System::Com::{CLSIDFromString, CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::System::LibraryLoader::{
    GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32,
};
use windows::Win32::System::Registry::HKEY_LOCAL_MACHINE;
use windows::Win32::UI::Shell::IShellIconOverlayIdentifier;

pub(super) fn probe(directory: &Path) -> CloudStatusProbe {
    let google = GoogleOverlays::load(directory);
    Box::new(move |path, metadata| {
        let attributes = metadata.file_attributes();
        if attributes & FILE_ATTRIBUTE_REPARSE_POINT.0 == 0 && google.is_none() {
            return None;
        }
        let path = wide_path(path)?;
        let path = PCWSTR(path.as_ptr());
        cloud_files_status(path, attributes).or_else(|| google.as_ref()?.status(path, attributes))
    })
}

fn wide_path(path: &Path) -> Option<Vec<u16>> {
    let mut wide: Vec<_> = path.as_os_str().encode_wide().collect();
    if wide.contains(&0) {
        return None;
    }
    wide.push(0);
    Some(wide)
}

fn cloud_files_status(path: PCWSTR, attributes: u32) -> Option<CloudStatus> {
    if attributes & FILE_ATTRIBUTE_REPARSE_POINT.0 == 0 {
        return None;
    }
    let placeholder_state = placeholder_state_api()?;
    // FindFirstFile gives the reparse tag without opening or hydrating the file.
    let mut data = WIN32_FIND_DATAW::default();
    let handle = unsafe { FindFirstFileW(path, &mut data) }.ok()?;
    let _ = unsafe { FindClose(handle) };
    let state = unsafe { placeholder_state(data.dwFileAttributes, data.dwReserved0) };
    classify_cloud_file(state, data.dwFileAttributes)
}

type PlaceholderStateApi = unsafe extern "system" fn(u32, u32) -> CF_PLACEHOLDER_STATE;

fn placeholder_state_api() -> Option<PlaceholderStateApi> {
    static API: OnceLock<Option<PlaceholderStateApi>> = OnceLock::new();
    // Cloud Files arrived in Windows 10 version 1709. Keep the system DLL
    // loaded for the lifetime of the cached function pointer.
    *API.get_or_init(|| unsafe {
        let library = LoadLibraryExW(w!("cldapi.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32).ok()?;
        if let Some(symbol) = GetProcAddress(library, s!("CfGetPlaceholderStateFromAttributeTag")) {
            Some(std::mem::transmute::<
                unsafe extern "system" fn() -> isize,
                PlaceholderStateApi,
            >(symbol))
        } else {
            let _ = FreeLibrary(library);
            None
        }
    })
}

fn classify_cloud_file(state: CF_PLACEHOLDER_STATE, attributes: u32) -> Option<CloudStatus> {
    if state == CF_PLACEHOLDER_STATE_INVALID || !state.contains(CF_PLACEHOLDER_STATE_PLACEHOLDER) {
        return None;
    }
    if !state.contains(CF_PLACEHOLDER_STATE_IN_SYNC) {
        return Some(CloudStatus::SyncPending);
    }
    let recall = FILE_ATTRIBUTE_OFFLINE.0
        | FILE_ATTRIBUTE_RECALL_ON_OPEN.0
        | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS.0;
    Some(
        if attributes & recall != 0 || state.contains(CF_PLACEHOLDER_STATE_PARTIALLY_ON_DISK) {
            CloudStatus::OnlineOnly
        } else if attributes & FILE_ATTRIBUTE_PINNED.0 != 0 {
            CloudStatus::AlwaysAvailable
        } else {
            CloudStatus::LocallyAvailable
        },
    )
}

struct GoogleOverlays {
    handlers: Vec<(CloudStatus, IShellIconOverlayIdentifier)>,
    // Release the COM interfaces before leaving their apartment.
    _apartment: ComApartment,
}

impl GoogleOverlays {
    fn load(directory: &Path) -> Option<Self> {
        const KEY: &str =
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\ShellIconOverlayIdentifiers";
        let registrations = open(HKEY_LOCAL_MACHINE, KEY)?;
        if !is_google_volume(directory) {
            return None;
        }
        let names = subkey_names(registrations.0);
        let apartment = ComApartment::sta().ok()?;
        let handlers = [
            (
                "GoogleDriveProgressOverlayIconHandler",
                CloudStatus::Syncing,
            ),
            (
                "GoogleDrivePinnedOverlayIconHandler",
                CloudStatus::AlwaysAvailable,
            ),
            (
                "GoogleDriveCloudOverlayIconHandler",
                CloudStatus::OnlineOnly,
            ),
        ]
        .into_iter()
        .filter_map(|(name, status)| {
            let key = open(
                registrations.0,
                names
                    .iter()
                    .find(|key| key.trim().eq_ignore_ascii_case(name))?,
            )?;
            let class = HSTRING::from(string(key.0, PCWSTR::null())?);
            let clsid = unsafe { CLSIDFromString(PCWSTR(class.as_ptr())) }.ok()?;
            let handler = unsafe { CoCreateInstance(&clsid, None, CLSCTX_INPROC_SERVER) }.ok()?;
            Some((status, handler))
        })
        .collect();
        Some(Self {
            handlers,
            _apartment: apartment,
        })
    }

    fn status(&self, path: PCWSTR, attributes: u32) -> Option<CloudStatus> {
        self.handlers.iter().find_map(|(status, handler)| {
            // The generated Result treats S_FALSE as success, too. It means no overlay.
            let result =
                unsafe { (handler.vtable().IsMemberOf)(handler.as_raw(), path, attributes) };
            (result == S_OK).then_some(*status)
        })
    }
}

fn is_google_volume(directory: &Path) -> bool {
    // Skip UNC locations before querying a volume. A streaming volume can use
    // any letter or be mounted in a folder on another drive.
    if !matches!(directory.components().next(), Some(Component::Prefix(prefix))
        if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
    {
        return false;
    }
    let Some(path) = wide_path(directory) else {
        return false;
    };
    let mut root = vec![0; 32_768];
    if unsafe { GetVolumePathNameW(PCWSTR(path.as_ptr()), &mut root) }.is_err() {
        return false;
    }
    let mut label = [0; 261];
    if unsafe {
        GetVolumeInformationW(
            PCWSTR(root.as_ptr()),
            Some(&mut label),
            None,
            None,
            None,
            None,
        )
    }
    .is_err()
    {
        return false;
    }
    let end = label
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(label.len());
    String::from_utf16_lossy(&label[..end]).eq_ignore_ascii_case("Google Drive")
}

#[cfg(test)]
#[path = "windows_cloud_status/tests/classification.rs"]
mod tests;
