//! Read the states reported by Cloud Files and Google Drive's Shell overlays.
use super::windows_com::ComApartment;
use super::windows_registry::{open, string, subkey_names};
use super::CloudStatusProbe;
use crate::models::{CloudAvailability, CloudStatus};
use std::ffi::c_void;
use std::os::windows::{ffi::OsStrExt, fs::MetadataExt};
use std::path::{Component, Path, Prefix};
use std::sync::OnceLock;
use windows::core::{s, w, Interface, HRESULT, HSTRING, PCWSTR};
use windows::Win32::Foundation::{FreeLibrary, S_OK};
use windows::Win32::Storage::CloudFilters::{
    CF_PLACEHOLDER_STATE, CF_PLACEHOLDER_STATE_INVALID, CF_PLACEHOLDER_STATE_IN_SYNC,
    CF_PLACEHOLDER_STATE_PARTIAL, CF_PLACEHOLDER_STATE_PARTIALLY_ON_DISK,
    CF_PLACEHOLDER_STATE_PLACEHOLDER, CF_SYNC_ROOT_BASIC_INFO, CF_SYNC_ROOT_INFO_BASIC,
    CF_SYNC_ROOT_INFO_CLASS,
};
use windows::Win32::Storage::FileSystem::{
    FindClose, FindFirstFileW, GetVolumeInformationW, GetVolumePathNameW, FILE_ATTRIBUTE_DIRECTORY,
    FILE_ATTRIBUTE_OFFLINE, FILE_ATTRIBUTE_PINNED, FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS,
    FILE_ATTRIBUTE_RECALL_ON_OPEN, FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_UNPINNED,
    WIN32_FIND_DATAW,
};
use windows::Win32::System::Com::{CLSIDFromString, CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::System::LibraryLoader::{
    GetModuleHandleW, GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32,
};
use windows::Win32::System::Registry::HKEY_LOCAL_MACHINE;
use windows::Win32::UI::Shell::IShellIconOverlayIdentifier;

mod folder;

pub(super) fn probe(directory: &Path) -> CloudStatusProbe {
    let cloud_files = CloudFilesApi::get();
    let in_sync_root = cloud_files.is_some_and(|api| api.contains_path(directory));
    let google = GoogleOverlays::load(directory);
    Box::new(move |path, metadata| {
        let attributes = metadata.file_attributes();
        let check_cloud_files = in_sync_root || attributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0;
        if !check_cloud_files && google.is_none() {
            return None;
        }
        let path = wide_path(path)?;
        let path = PCWSTR(path.as_ptr());
        let cloud_status = if check_cloud_files {
            cloud_files.and_then(|api| api.status(path))
        } else {
            None
        };
        cloud_status.or_else(|| google.as_ref()?.status(path, attributes))
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

pub(super) fn folder_contents_ready(path: &Path, availability: CloudAvailability) -> Option<bool> {
    let Some(api) = CloudFilesApi::get() else {
        return Some(true);
    };
    let wide = wide_path(path)?;
    // Google Drive's Shell overlays already supply their own progress state.
    let data = api.find(PCWSTR(wide.as_ptr()))?;
    let state = api.state(&data);
    if state == CF_PLACEHOLDER_STATE_INVALID {
        return None;
    }
    if !state.contains(CF_PLACEHOLDER_STATE_PLACEHOLDER) {
        return Some(true);
    }
    if !availability.is_confirmed_by(classify_cloud_file(state, data.dwFileAttributes)?) {
        return Some(false);
    }
    api.with_placeholder_tags(|| folder::contents_ready(path, availability, |data| api.state(data)))
}

struct CloudFilesApi {
    placeholder_state: unsafe extern "system" fn(u32, u32) -> CF_PLACEHOLDER_STATE,
    sync_root_info: unsafe extern "system" fn(
        PCWSTR,
        CF_SYNC_ROOT_INFO_CLASS,
        *mut c_void,
        u32,
        *mut u32,
    ) -> HRESULT,
    set_compatibility_mode: unsafe extern "system" fn(i8) -> i8,
}

impl CloudFilesApi {
    fn get() -> Option<&'static Self> {
        static API: OnceLock<Option<CloudFilesApi>> = OnceLock::new();
        // Cloud Files arrived in Windows 10 version 1709. Keep the system DLL
        // loaded for the lifetime of the cached function pointers.
        API.get_or_init(|| unsafe {
            let ntdll = GetModuleHandleW(w!("ntdll.dll")).ok()?;
            let set_mode = GetProcAddress(ntdll, s!("RtlSetThreadPlaceholderCompatibilityMode"))?;
            let library =
                LoadLibraryExW(w!("cldapi.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32).ok()?;
            let state = GetProcAddress(library, s!("CfGetPlaceholderStateFromAttributeTag"));
            let root_info = GetProcAddress(library, s!("CfGetSyncRootInfoByPath"));
            if let (Some(state), Some(root_info)) = (state, root_info) {
                Some(Self {
                    placeholder_state: std::mem::transmute(state),
                    sync_root_info: std::mem::transmute(root_info),
                    set_compatibility_mode: std::mem::transmute(set_mode),
                })
            } else {
                let _ = FreeLibrary(library);
                None
            }
        })
        .as_ref()
    }

    fn contains_path(&self, path: &Path) -> bool {
        let Some(path) = wide_path(path) else {
            return false;
        };
        let mut info = CF_SYNC_ROOT_BASIC_INFO::default();
        let result = unsafe {
            (self.sync_root_info)(
                PCWSTR(path.as_ptr()),
                CF_SYNC_ROOT_INFO_BASIC,
                std::ptr::from_mut(&mut info).cast(),
                size_of::<CF_SYNC_ROOT_BASIC_INFO>() as u32,
                std::ptr::null_mut(),
            )
        };
        result == S_OK
    }

    fn status(&self, path: PCWSTR) -> Option<CloudStatus> {
        let data = self.find(path)?;
        classify_cloud_file(self.state(&data), data.dwFileAttributes)
    }

    fn find(&self, path: PCWSTR) -> Option<WIN32_FIND_DATAW> {
        let mut data = WIN32_FIND_DATAW::default();
        let handle = self.with_placeholder_tags(|| unsafe { FindFirstFileW(path, &mut data) });
        let _ = unsafe { FindClose(handle.ok()?) };
        Some(data)
    }

    fn state(&self, data: &WIN32_FIND_DATAW) -> CF_PLACEHOLDER_STATE {
        unsafe { (self.placeholder_state)(data.dwFileAttributes, data.dwReserved0) }
    }

    fn with_placeholder_tags<T>(&self, read: impl FnOnce() -> T) -> T {
        const EXPOSE_PLACEHOLDERS: i8 = 2;
        // Leave the listing's file types intact outside these metadata calls.
        let previous_mode = unsafe { (self.set_compatibility_mode)(EXPOSE_PLACEHOLDERS) };
        let result = read();
        if previous_mode >= 0 {
            unsafe { (self.set_compatibility_mode)(previous_mode) };
        }
        result
    }
}

fn classify_cloud_file(state: CF_PLACEHOLDER_STATE, attributes: u32) -> Option<CloudStatus> {
    if state == CF_PLACEHOLDER_STATE_INVALID || !state.contains(CF_PLACEHOLDER_STATE_PLACEHOLDER) {
        return None;
    }
    if !state.contains(CF_PLACEHOLDER_STATE_IN_SYNC) {
        return Some(CloudStatus::SyncPending);
    }
    let pinned = attributes & FILE_ATTRIBUTE_PINNED.0 != 0;
    // OneDrive keeps a directory's metadata on disk after freeing its contents.
    // Unlike a file's data, the directory can therefore stay fully populated.
    if attributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0
        && !pinned
        && attributes & FILE_ATTRIBUTE_UNPINNED.0 != 0
    {
        return Some(CloudStatus::OnlineOnly);
    }
    let recall = FILE_ATTRIBUTE_OFFLINE.0
        | FILE_ATTRIBUTE_RECALL_ON_OPEN.0
        | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS.0;
    Some(
        if attributes & recall != 0 || state.contains(CF_PLACEHOLDER_STATE_PARTIALLY_ON_DISK) {
            CloudStatus::OnlineOnly
        } else if pinned && state.contains(CF_PLACEHOLDER_STATE_PARTIAL) {
            CloudStatus::Syncing
        } else if pinned {
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

pub(super) fn is_google_volume(directory: &Path) -> bool {
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
