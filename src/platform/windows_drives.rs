use crate::models::DriveInfo;

pub fn list_drive_roots() -> Vec<DriveInfo> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDriveStringsW};
    let mut buffer = [0u16; 260];
    let length = unsafe { GetLogicalDriveStringsW(Some(&mut buffer)) } as usize;
    if length == 0 || length >= buffer.len() {
        return Vec::new();
    }
    String::from_utf16_lossy(&buffer[..length])
        .split('\0')
        .filter(|path| !path.is_empty())
        .filter_map(|path| {
            let wide: Vec<_> = path.encode_utf16().chain(std::iter::once(0)).collect();
            let kind = unsafe { GetDriveTypeW(PCWSTR(wide.as_ptr())) };
            let drive_type = match kind {
                2 => "removable",
                3 => "fixed",
                4 => "remote",
                5 => "cdrom",
                0 | 1 => return None,
                _ => "unknown",
            };
            Some(DriveInfo {
                path: path.into(),
                label: "Local Disk".into(),
                drive_type: drive_type.into(),
                is_readonly: kind == 5,
                total_bytes: 0,
                free_bytes: 0,
                media_type: None,
                physical_id: None,
                remote_path: None,
            })
        })
        .collect()
}

/// PnP interface notifications need no open volume/directory handles.
pub fn watch_drive_changes(
    revision: std::sync::Arc<std::sync::atomic::AtomicU64>,
) -> Result<Box<dyn Send>, String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    use windows::Win32::Devices::DeviceAndDriverInstallation::*;
    use windows::Win32::System::Ioctl::{GUID_DEVINTERFACE_DISK, GUID_DEVINTERFACE_VOLUME};
    struct Registration {
        handles: Vec<usize>,
        // Stable Arc allocation remains alive until all callbacks are unregistered.
        revision: std::sync::Arc<AtomicU64>,
    }
    impl Drop for Registration {
        fn drop(&mut self) {
            for handle in self.handles.drain(..) {
                unsafe {
                    CM_Unregister_Notification(HCMNOTIFICATION(handle as *mut _));
                }
            }
        }
    }
    unsafe extern "system" fn callback(
        _notification: HCMNOTIFICATION,
        context: *const std::ffi::c_void,
        action: CM_NOTIFY_ACTION,
        _data: *const CM_NOTIFY_EVENT_DATA,
        _size: u32,
    ) -> u32 {
        if action == CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL
            || action == CM_NOTIFY_ACTION_DEVICEINTERFACEREMOVAL
        {
            // No IO, allocations or UI access inside a PnP callback.
            unsafe { &*context.cast::<AtomicU64>() }.fetch_add(1, Ordering::Release);
        }
        0
    }
    let mut registration = Registration {
        handles: Vec::new(),
        revision,
    };
    for guid in [GUID_DEVINTERFACE_DISK, GUID_DEVINTERFACE_VOLUME] {
        let filter = CM_NOTIFY_FILTER {
            cbSize: std::mem::size_of::<CM_NOTIFY_FILTER>() as u32,
            FilterType: CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE,
            u: CM_NOTIFY_FILTER_0 {
                DeviceInterface: CM_NOTIFY_FILTER_0_0 { ClassGuid: guid },
            },
            ..Default::default()
        };
        let mut handle = HCMNOTIFICATION::default();
        let status = unsafe {
            CM_Register_Notification(
                &filter,
                Some(std::sync::Arc::as_ptr(&registration.revision).cast()),
                Some(callback),
                &mut handle,
            )
        };
        if status != CR_SUCCESS {
            return Err(format!(
                "PnP notification registration failed: {}",
                status.0
            ));
        }
        registration.handles.push(handle.0 as usize);
    }
    Ok(Box::new(registration))
}

/// Resolve the mounted volume to its physical disk, then ask PnP for safe removal.
pub fn eject_drive(path: &str) -> Result<(), String> {
    use windows::core::PCWSTR;
    use windows::Win32::Devices::DeviceAndDriverInstallation::*;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows::Win32::System::Ioctl::{
        GUID_DEVINTERFACE_DISK, IOCTL_STORAGE_GET_DEVICE_NUMBER, STORAGE_DEVICE_NUMBER,
    };
    use windows::Win32::System::IO::DeviceIoControl;

    let bytes = path.as_bytes();
    if !(bytes.len() == 2 || bytes.len() == 3)
        || !bytes[0].is_ascii_alphabetic()
        || bytes[1] != b':'
        || (bytes.len() == 3 && !matches!(bytes[2], b'\\' | b'/'))
    {
        return Err("Invalid drive root".into());
    }
    unsafe fn disk_number(path: PCWSTR) -> Result<STORAGE_DEVICE_NUMBER, String> {
        let handle = unsafe {
            CreateFileW(
                path,
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )
        }
        .map_err(|error| error.to_string())?;
        let mut number = STORAGE_DEVICE_NUMBER::default();
        let mut returned = 0;
        let result = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_STORAGE_GET_DEVICE_NUMBER,
                None,
                0,
                Some(&mut number as *mut _ as *mut _),
                std::mem::size_of_val(&number) as u32,
                Some(&mut returned),
                None,
            )
        };
        let _ = unsafe { CloseHandle(handle) };
        result.map_err(|error| error.to_string())?;
        Ok(number)
    }

    let volume: Vec<u16> = format!("\\\\.\\{}:", bytes[0] as char)
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let target = disk_number(PCWSTR(volume.as_ptr()))?;
        let devices = SetupDiGetClassDevsW(
            Some(&GUID_DEVINTERFACE_DISK),
            PCWSTR::null(),
            None,
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )
        .map_err(|error| error.to_string())?;
        // Always destroy the device list, including on early errors in the closure.
        let result = (|| {
            let mut index = 0;
            loop {
                let mut interface = SP_DEVICE_INTERFACE_DATA {
                    cbSize: std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                    ..Default::default()
                };
                if SetupDiEnumDeviceInterfaces(
                    devices,
                    None,
                    &GUID_DEVINTERFACE_DISK,
                    index,
                    &mut interface,
                )
                .is_err()
                {
                    break;
                }
                index += 1;
                let mut required = 0;
                let _ = SetupDiGetDeviceInterfaceDetailW(
                    devices,
                    &interface,
                    None,
                    0,
                    Some(&mut required),
                    None,
                );
                if required < std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32 {
                    continue;
                }
                // Pointer-sized allocation keeps the variable-length structure aligned.
                let mut buffer =
                    vec![0usize; (required as usize).div_ceil(std::mem::size_of::<usize>())];
                let detail = buffer
                    .as_mut_ptr()
                    .cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
                (*detail).cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
                let mut info = SP_DEVINFO_DATA {
                    cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
                    ..Default::default()
                };
                if SetupDiGetDeviceInterfaceDetailW(
                    devices,
                    &interface,
                    Some(detail),
                    required,
                    None,
                    Some(&mut info),
                )
                .is_err()
                {
                    continue;
                }
                let device_path = PCWSTR(std::ptr::addr_of!((*detail).DevicePath).cast());
                let Ok(number) = disk_number(device_path) else {
                    continue;
                };
                if number.DeviceNumber != target.DeviceNumber
                    || number.DeviceType != target.DeviceType
                {
                    continue;
                }
                // Stop at the first physically removable node in the disk's ancestry.
                let mut node = info.DevInst;
                loop {
                    let mut status = CM_DEVNODE_STATUS_FLAGS::default();
                    let mut problem = CM_PROB::default();
                    if CM_Get_DevNode_Status(&mut status, &mut problem, node, 0) != CR_SUCCESS {
                        return Err("Cannot query device status".into());
                    }
                    if status.0 & DN_REMOVABLE.0 != 0 {
                        break;
                    }
                    let mut parent = 0;
                    if CM_Get_Parent(&mut parent, node, 0) != CR_SUCCESS {
                        return Err("No removable device found for this drive".into());
                    }
                    node = parent;
                }
                let mut veto = PNP_VETO_TYPE::default();
                let mut name = [0u16; 260];
                let result = CM_Request_Device_EjectW(node, Some(&mut veto), Some(&mut name), 0);
                return if result == CR_SUCCESS {
                    Ok(())
                } else {
                    let end = name.iter().position(|c| *c == 0).unwrap_or(name.len());
                    Err(format!(
                        "Windows PnP: code {}, veto {} ({})",
                        result.0,
                        veto.0,
                        String::from_utf16_lossy(&name[..end])
                    ))
                };
            }
            Err("Cannot identify the physical device for this drive".into())
        })();
        let _ = SetupDiDestroyDeviceInfoList(devices);
        result
    }
}

#[cfg(test)]
mod eject_tests {
    #[test]
    fn native_drive_notifications_can_be_registered_and_released() {
        let revision = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let guard = super::watch_drive_changes(revision.clone()).unwrap();
        drop(guard);
        // Registration context ownership is released after callbacks are unregistered.
        assert_eq!(std::sync::Arc::strong_count(&revision), 1);
    }

    #[test]
    fn rejects_non_volume_targets_before_accessing_devices() {
        for path in [
            "",
            "C",
            "C:\\folder",
            "\\\\server\\share",
            "1:",
            "C:x",
            "é:",
        ] {
            assert_eq!(super::eject_drive(path), Err("Invalid drive root".into()));
        }
    }
}

pub fn list_drives(skip_hardware_info: bool) -> Vec<DriveInfo> {
    use windows::core::PCWSTR;
    use windows::Win32::NetworkManagement::WNet::WNetGetConnectionW;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDriveStringsW,
        GetVolumeInformationW, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, FILE_SHARE_WRITE,
        OPEN_EXISTING,
    };
    use windows::Win32::System::Ioctl::{
        PropertyStandardQuery, StorageAdapterProperty, StorageDeviceSeekPenaltyProperty,
        DEVICE_SEEK_PENALTY_DESCRIPTOR, IOCTL_STORAGE_GET_DEVICE_NUMBER,
        IOCTL_STORAGE_QUERY_PROPERTY, STORAGE_ADAPTER_DESCRIPTOR, STORAGE_DEVICE_NUMBER,
        STORAGE_PROPERTY_QUERY,
    };
    use windows::Win32::System::IO::DeviceIoControl;

    let mut drives = Vec::new();
    unsafe {
        let mut buffer = [0u16; 260];
        let len = GetLogicalDriveStringsW(Some(&mut buffer));
        if len > 0 {
            let drive_strings = String::from_utf16_lossy(&buffer[..len as usize]);
            for drive_path in drive_strings.split('\0').filter(|s| !s.is_empty()) {
                let root_path: Vec<u16> = drive_path
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();

                let win_type = GetDriveTypeW(PCWSTR(root_path.as_ptr()));
                if win_type <= 1 {
                    continue;
                }

                let drive_type = match win_type {
                    2 => "removable".to_string(),
                    3 => "fixed".to_string(),
                    4 => "remote".to_string(),
                    5 => "cdrom".to_string(),
                    _ => "unknown".to_string(),
                };

                let mut volume_name = [0u16; 261];
                let mut fs_name = [0u16; 261];
                let mut flags = 0u32;

                let (label, is_readonly_vol) = if GetVolumeInformationW(
                    PCWSTR(root_path.as_ptr()),
                    Some(&mut volume_name),
                    None,
                    None,
                    Some(&mut flags),
                    Some(&mut fs_name),
                )
                .is_ok()
                {
                    (
                        String::from_utf16_lossy(&volume_name)
                            .trim_matches(char::from(0))
                            .to_string(),
                        (flags & 0x00080000) != 0, // FILE_READ_ONLY_VOLUME
                    )
                } else {
                    (String::new(), false)
                };

                // Get disk space information
                let mut free_bytes_available = 0u64;
                let mut total_bytes = 0u64;
                let mut total_free_bytes = 0u64;

                let _ = GetDiskFreeSpaceExW(
                    PCWSTR(root_path.as_ptr()),
                    Some(&mut free_bytes_available),
                    Some(&mut total_bytes),
                    Some(&mut total_free_bytes),
                );

                let mut media_type = None;
                let mut physical_id = None;
                let mut remote_path = None;

                if win_type == 4 {
                    // Remote drive
                    let mut buffer = [0u16; 1024];
                    let mut size = buffer.len() as u32;
                    let root_path_no_slash: Vec<u16> = drive_path[..2]
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect();

                    if WNetGetConnectionW(
                        windows::core::PCWSTR(root_path_no_slash.as_ptr()),
                        Some(windows::core::PWSTR(buffer.as_mut_ptr())),
                        &mut size,
                    )
                    .is_ok()
                    {
                        remote_path = Some(
                            String::from_utf16_lossy(&buffer[..size as usize - 1])
                                .trim_matches(char::from(0))
                                .to_string(),
                        );
                    }
                }

                if !skip_hardware_info && (win_type == 3 || win_type == 2 || win_type == 5) {
                    // Fixed, Removable or CD-ROM
                    let drive_root_unf = format!("\\\\.\\{}:", &drive_path[0..1]);
                    let wide_path: Vec<u16> = drive_root_unf
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect();

                    let handle = CreateFileW(
                        PCWSTR(wide_path.as_ptr()),
                        0,
                        FILE_SHARE_READ | FILE_SHARE_WRITE,
                        None,
                        OPEN_EXISTING,
                        FILE_FLAG_BACKUP_SEMANTICS,
                        None,
                    );

                    if let Ok(h) = handle {
                        if !h.is_invalid() {
                            // 1. Get Physical ID
                            let mut device_number = STORAGE_DEVICE_NUMBER::default();
                            let mut bytes_returned = 0u32;
                            if DeviceIoControl(
                                h,
                                IOCTL_STORAGE_GET_DEVICE_NUMBER,
                                None,
                                0,
                                Some(&mut device_number as *mut _ as *mut _),
                                std::mem::size_of::<STORAGE_DEVICE_NUMBER>() as u32,
                                Some(&mut bytes_returned),
                                None,
                            )
                            .is_ok()
                            {
                                physical_id = Some(format!("Disk {}", device_number.DeviceNumber));
                            }

                            // 2. Detect SSD vs HDD (Seek Penalty)
                            let mut query = STORAGE_PROPERTY_QUERY {
                                PropertyId: StorageDeviceSeekPenaltyProperty,
                                QueryType: PropertyStandardQuery,
                                ..Default::default()
                            };
                            let mut descriptor = DEVICE_SEEK_PENALTY_DESCRIPTOR::default();
                            if DeviceIoControl(
                                h,
                                IOCTL_STORAGE_QUERY_PROPERTY,
                                Some(&mut query as *mut _ as *mut _),
                                std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
                                Some(&mut descriptor as *mut _ as *mut _),
                                std::mem::size_of::<DEVICE_SEEK_PENALTY_DESCRIPTOR>() as u32,
                                Some(&mut bytes_returned),
                                None,
                            )
                            .is_ok()
                            {
                                media_type = Some(if descriptor.IncursSeekPenalty {
                                    "HDD".to_string()
                                } else {
                                    "SSD".to_string()
                                });
                            }

                            // 3. Detect Bus Type (USB, Virtual, etc)
                            let mut adapter_query = STORAGE_PROPERTY_QUERY {
                                PropertyId: StorageAdapterProperty,
                                QueryType: PropertyStandardQuery,
                                ..Default::default()
                            };
                            let mut adapter_desc = STORAGE_ADAPTER_DESCRIPTOR::default();
                            if DeviceIoControl(
                                h,
                                IOCTL_STORAGE_QUERY_PROPERTY,
                                Some(&mut adapter_query as *mut _ as *mut _),
                                std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
                                Some(&mut adapter_desc as *mut _ as *mut _),
                                std::mem::size_of::<STORAGE_ADAPTER_DESCRIPTOR>() as u32,
                                Some(&mut bytes_returned),
                                None,
                            )
                            .is_ok()
                            {
                                match adapter_desc.BusType {
                                    7 => {
                                        // BusTypeUsb
                                        media_type = media_type
                                            .map(|m| format!("USB-{}", m))
                                            .or(Some("USB".to_string()));
                                    }
                                    15 => {
                                        // BusTypeFileBackedVirtual
                                        media_type = media_type
                                            .map(|m| format!("IMG-{}", m))
                                            .or(Some("IMAGE".to_string()));
                                    }
                                    _ => {}
                                }
                            }

                            let _ = windows::Win32::Foundation::CloseHandle(h);
                        }
                    }
                }

                if media_type.is_none() && win_type == 5 {
                    media_type = Some("CD/DVD".to_string());
                } else if win_type == 4 {
                    media_type = Some("NAS".to_string());
                    physical_id = Some("Network".to_string());
                }

                drives.push(DriveInfo {
                    path: drive_path.to_string(),
                    label: if label.is_empty() {
                        "Local Disk".to_string()
                    } else {
                        label
                    },
                    drive_type,
                    is_readonly: is_readonly_vol || win_type == 5, // CD-ROM is read-only
                    total_bytes,
                    free_bytes: free_bytes_available,
                    media_type,
                    physical_id,
                    remote_path,
                });
            }
        }
    }

    // Sort drives alphabetically
    drives.sort_by(|a, b| a.path.cmp(&b.path));
    drives
}
