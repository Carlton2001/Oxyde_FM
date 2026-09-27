use crate::models::DriveInfo;

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
