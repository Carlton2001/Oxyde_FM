//! Harmless junction fixtures confined to a caller-owned temporary directory.
use std::fs::{File, OpenOptions};
use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle, process::CommandExt};
use std::path::Path;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_WRITE_ATTRIBUTES,
};
use windows::Win32::System::{
    Ioctl::{FSCTL_DELETE_REPARSE_POINT, FSCTL_GET_REPARSE_POINT, FSCTL_SET_REPARSE_POINT},
    IO::DeviceIoControl,
};

pub(crate) fn junction(link: &Path, target: &Path) {
    let result = std::process::Command::new(super::platform().system_executable(super::SystemExecutable::PowerShell).unwrap())
        .args(["-NoProfile", "-NonInteractive", "-Command", "New-Item -ItemType Junction -Path $env:OXYDE_TEST_LINK -Value $env:OXYDE_TEST_TARGET -ErrorAction Stop | Out-Null"])
        .env("OXYDE_TEST_LINK", link).env("OXYDE_TEST_TARGET", target)
        .creation_flags(0x08000000).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

pub(crate) struct Retarget {
    writer: File,
    data: Vec<u8>,
}
impl Retarget {
    pub(crate) fn new(link: &Path, template: &Path) -> Self {
        let reader = OpenOptions::new()
            .read(true)
            .share_mode(7)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0 | FILE_FLAG_OPEN_REPARSE_POINT.0)
            .open(template)
            .unwrap();
        let mut data = vec![0u8; 16 * 1024];
        let mut size = 0;
        unsafe {
            DeviceIoControl(
                HANDLE(reader.as_raw_handle()),
                FSCTL_GET_REPARSE_POINT,
                None,
                0,
                Some(data.as_mut_ptr().cast()),
                data.len() as u32,
                Some(&mut size),
                None,
            )
        }
        .unwrap();
        data.truncate(size as usize);
        let writer = OpenOptions::new()
            .access_mode(FILE_WRITE_ATTRIBUTES.0)
            .share_mode(7)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0 | FILE_FLAG_OPEN_REPARSE_POINT.0)
            .open(link)
            .unwrap();
        Self { writer, data }
    }
    pub(crate) fn apply(&self) {
        self.try_apply().unwrap();
    }
    pub(crate) fn try_apply(&self) -> windows::core::Result<()> {
        let mut size = 0;
        unsafe {
            DeviceIoControl(
                HANDLE(self.writer.as_raw_handle()),
                FSCTL_SET_REPARSE_POINT,
                Some(self.data.as_ptr().cast()),
                self.data.len() as u32,
                None,
                0,
                Some(&mut size),
                None,
            )
        }
    }
    pub(crate) fn clear(&self) {
        let mut header = [0u8; 8];
        header[..4].copy_from_slice(&self.data[..4]);
        let mut size = 0;
        unsafe {
            DeviceIoControl(
                HANDLE(self.writer.as_raw_handle()),
                FSCTL_DELETE_REPARSE_POINT,
                Some(header.as_ptr().cast()),
                header.len() as u32,
                None,
                0,
                Some(&mut size),
                None,
            )
        }
        .unwrap();
    }
}
