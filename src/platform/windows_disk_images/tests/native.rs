use super::*;

#[test]
fn roots_match_all_partitions_and_keep_disk_and_cdrom_numbers_distinct() {
    let disk = STORAGE_DEVICE_NUMBER {
        DeviceType: 7,
        DeviceNumber: 3,
        PartitionNumber: u32::MAX,
    };
    let drives = vec![
        (
            "D:\\".into(),
            STORAGE_DEVICE_NUMBER {
                PartitionNumber: 1,
                ..disk
            },
        ),
        (
            "E:\\".into(),
            STORAGE_DEVICE_NUMBER {
                PartitionNumber: 2,
                ..disk
            },
        ),
        (
            "F:\\".into(),
            STORAGE_DEVICE_NUMBER {
                DeviceType: 2,
                ..disk
            },
        ),
        (
            "G:\\".into(),
            STORAGE_DEVICE_NUMBER {
                DeviceNumber: 4,
                ..disk
            },
        ),
    ];
    assert_eq!(roots_for(&disk, &drives), ["D:\\", "E:\\"]);
    assert!(roots_for(&disk, &[]).is_empty());
}

#[test]
fn dependency_strings_are_bounded_by_the_native_allocation() {
    let mut buffer = vec![usize::MAX; 16];
    let text: Vec<u16> = "épreuve 日本語\0".encode_utf16().collect();
    unsafe {
        std::ptr::copy_nonoverlapping(text.as_ptr(), buffer.as_mut_ptr().cast(), text.len());
    }
    let pointer = PWSTR(buffer.as_mut_ptr().cast());
    assert_eq!(
        dependency_string(&buffer, pointer).unwrap(),
        "épreuve 日本語"
    );
    assert!(dependency_string(&buffer, PWSTR::null()).is_err());
    assert!(dependency_string(&buffer, PWSTR((pointer.0 as usize + 1) as *mut u16)).is_err());
    let end = unsafe { buffer.as_mut_ptr().add(buffer.len()).cast() };
    assert!(dependency_string(&buffer, PWSTR(end)).is_err());
    buffer.fill(usize::MAX);
    assert!(dependency_string(&buffer, pointer).is_err());
}

#[test]
fn paths_reach_windows_literally_and_img_selects_the_iso_provider() {
    let path = Path::new("C:\\épreuve 日本語 '$();.IMG");
    assert!(is_optical(path));
    assert_eq!(
        String::from_utf16(&wide(path).unwrap()[..path.as_os_str().encode_wide().count()]).unwrap(),
        path.to_str().unwrap()
    );
    assert!(!is_optical(Path::new("C:\\disk.vhdx")));
    assert!(wide(Path::new("C:\\bad\0.iso")).is_err());
}

#[test]
fn native_enumeration_returns_only_absolute_image_paths() {
    for image in mounted_images().unwrap() {
        assert!(
            Path::new(&image.image_path).is_absolute(),
            "{}",
            image.image_path
        );
    }
}

fn create_virtual_disk(path: &Path, device: u32) {
    let name = wide(path).unwrap();
    let storage = VIRTUAL_STORAGE_TYPE {
        DeviceId: device,
        VendorId: VIRTUAL_STORAGE_TYPE_VENDOR_MICROSOFT,
    };
    let parameters = CREATE_VIRTUAL_DISK_PARAMETERS {
        Version: CREATE_VIRTUAL_DISK_VERSION_2,
        Anonymous: CREATE_VIRTUAL_DISK_PARAMETERS_0 {
            Version2: CREATE_VIRTUAL_DISK_PARAMETERS_0_1 {
                MaximumSize: 16 * 1024 * 1024,
                ..Default::default()
            },
        },
    };
    let mut handle = HANDLE::default();
    status(unsafe {
        CreateVirtualDisk(
            &storage,
            PCWSTR(name.as_ptr()),
            VIRTUAL_DISK_ACCESS_NONE,
            None,
            CREATE_VIRTUAL_DISK_FLAG_NONE,
            0,
            &parameters,
            None,
            &mut handle,
        )
    })
    .unwrap();
    drop(DiskHandle(handle));
}

#[test]
#[ignore = "requires the Windows virtual-disk provider; creates only temporary VHD/VHDX files"]
fn both_virtual_disk_formats_open_before_attachment() {
    let temp = tempfile::tempdir().unwrap();
    for (extension, device) in [
        ("vhd", VIRTUAL_STORAGE_TYPE_DEVICE_VHD),
        ("vhdx", VIRTUAL_STORAGE_TYPE_DEVICE_VHDX),
    ] {
        let path = temp.path().join(format!("Oxyde épreuve '$().{extension}"));
        create_virtual_disk(&path, device);
        let handle = open_image(
            &path,
            VIRTUAL_DISK_ACCESS_GET_INFO | VIRTUAL_DISK_ACCESS_DETACH,
        )
        .unwrap();
        assert!(matches!(
            physical_path(&handle),
            Err(ERROR_NOT_READY | ERROR_DEV_NOT_EXIST)
        ));
        drop(handle);
        // No lingering native handle may prevent moving/deleting the image.
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
#[ignore = "requires SeManageVolumePrivilege; attaches only generated temporary VHD/VHDX files"]
fn unformatted_virtual_disks_are_discovered_without_drive_letters() {
    let temp = tempfile::tempdir().unwrap();
    for (extension, device) in [
        ("vhd", VIRTUAL_STORAGE_TYPE_DEVICE_VHD),
        ("vhdx", VIRTUAL_STORAGE_TYPE_DEVICE_VHDX),
    ] {
        let path = temp.path().join(format!("Oxyde épreuve '$().{extension}"));
        create_virtual_disk(&path, device);
        struct Cleanup<'a>(&'a Path);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                let _ = unmount(self.0);
            }
        }
        let _cleanup = Cleanup(&path);
        assert!(mount(&path).unwrap().is_empty());
        // A fresh enumeration has no memory of mount(), and the mounting
        // handle has already been closed: this also covers persistent attachment.
        let images = mounted_images().unwrap();
        let image = images
            .iter()
            .find(|image| {
                image
                    .image_path
                    .eq_ignore_ascii_case(path.to_str().unwrap())
            })
            .unwrap_or_else(|| {
                panic!("Generated {extension} missing from native enumeration: {images:?}")
            });
        assert!(image.roots.is_empty());
        assert!(mount(&path).unwrap().is_empty());
        unmount(&path).unwrap();
        assert!(!mounted_images().unwrap().iter().any(|image| image
            .image_path
            .eq_ignore_ascii_case(path.to_str().unwrap())));
    }
}
