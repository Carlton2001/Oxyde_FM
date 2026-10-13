//! A parent-only signing secret, DPAPI protected at rest and never sent to workers.
use super::*;
use std::{
    fs::File,
    io::{Read, Write},
    os::windows::io::AsRawHandle,
    path::Path,
};
use windows::Win32::{
    Foundation::{GENERIC_READ, GENERIC_WRITE, HANDLE},
    Security::{
        Authorization::{
            ConvertSecurityDescriptorToStringSecurityDescriptorW,
            ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo, SDDL_REVISION_1,
            SE_FILE_OBJECT,
        },
        Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG},
        SetKernelObjectSecurity, DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
        PSECURITY_DESCRIPTOR,
    },
    Storage::FileSystem::{FILE_SHARE_READ, READ_CONTROL, WRITE_DAC},
};
struct Descriptor(PSECURITY_DESCRIPTOR);
impl Drop for Descriptor {
    fn drop(&mut self) {
        unsafe {
            LocalFree(Some(HLOCAL(self.0 .0)));
        }
    }
}
fn native(error: windows::core::Error) -> io::Error {
    io::Error::other(error.to_string())
}
fn check_permissions(file: &File, expected: &str) -> io::Result<()> {
    let mut raw = PSECURITY_DESCRIPTOR::default();
    unsafe {
        GetSecurityInfo(
            HANDLE(file.as_raw_handle()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            None,
            None,
            Some(&mut raw),
        )
    }
    .ok()
    .map_err(native)?;
    let descriptor = Descriptor(raw);
    let mut text = windows::core::PWSTR::null();
    unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor.0,
            SDDL_REVISION_1,
            DACL_SECURITY_INFORMATION,
            &mut text,
            None,
        )
    }
    .map_err(native)?;
    struct Text(windows::core::PWSTR);
    impl Drop for Text {
        fn drop(&mut self) {
            unsafe {
                LocalFree(Some(HLOCAL(self.0 .0.cast())));
            }
        }
    }
    let allocation = Text(text);
    let actual =
        unsafe { allocation.0.to_string() }.map_err(|error| io::Error::other(error.to_string()))?;
    if actual != expected {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Cache signing key permissions are not private",
        ));
    }
    Ok(())
}
pub(in crate::platform) fn load(path: &Path) -> io::Result<[u8; 32]> {
    let expected = format!(
        "D:P(A;;FA;;;{})(A;;FA;;;SY)",
        super::super::windows_worker_sandbox::parent_user_sid()?
    );
    let created = super::super::windows_no_reparse::open(
        path,
        false,
        GENERIC_READ.0 | GENERIC_WRITE.0 | READ_CONTROL.0 | WRITE_DAC.0,
        0,
        2,
    );
    match created {
        Ok(mut file) => {
            let sddl: Vec<_> = expected.encode_utf16().chain([0]).collect();
            let mut raw = PSECURITY_DESCRIPTOR::default();
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    PCWSTR(sddl.as_ptr()),
                    SDDL_REVISION_1,
                    &mut raw,
                    None,
                )
            }
            .map_err(native)?;
            let descriptor = Descriptor(raw);
            unsafe {
                SetKernelObjectSecurity(
                    HANDLE(file.as_raw_handle()),
                    DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                    descriptor.0,
                )
            }
            .map_err(native)?;
            check_permissions(&file, &expected)?;
            let mut key = [0; 32];
            unsafe { BCryptGenRandom(None, &mut key, BCRYPT_USE_SYSTEM_PREFERRED_RNG) }
                .ok()
                .map_err(native)?;
            let encrypted = dpapi(&key, true)?;
            file.write_all(&encrypted)?;
            file.sync_all()?;
            Ok(key)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let mut file = super::super::windows_no_reparse::open(
                path,
                false,
                GENERIC_READ.0 | READ_CONTROL.0,
                FILE_SHARE_READ.0,
                1,
            )?;
            let metadata = file.metadata()?;
            if !metadata.is_file()
                || super::super::platform().file_attributes(&metadata, "").2
                || metadata.len() > 4096
            {
                return Err(io::Error::other("Invalid cache signing key file"));
            }
            check_permissions(&file, &expected)?;
            let mut encrypted = Vec::new();
            (&mut file).take(4097).read_to_end(&mut encrypted)?;
            let key = dpapi(&encrypted, false)?;
            key.try_into()
                .map_err(|_| io::Error::other("Invalid cache signing key length"))
        }
        Err(error) => Err(error),
    }
}
