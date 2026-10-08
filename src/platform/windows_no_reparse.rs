//! Atomic native namespace lookup: refuse reparse points in every component.
use std::{
    ffi::c_void,
    fs::File,
    io,
    os::windows::{ffi::OsStrExt, fs::MetadataExt, io::FromRawHandle},
    path::Path,
};
#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *mut u16,
}
#[repr(C)]
struct ObjectAttributes {
    length: u32,
    root: *mut c_void,
    name: *mut UnicodeString,
    attributes: u32,
    security: *mut c_void,
    qos: *mut c_void,
}
#[repr(C)]
struct IoStatusBlock {
    status: usize,
    information: usize,
}
#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtCreateFile(
        file: *mut *mut c_void,
        access: u32,
        attributes: *const ObjectAttributes,
        status: *mut IoStatusBlock,
        allocation: *const i64,
        file_attributes: u32,
        sharing: u32,
        disposition: u32,
        options: u32,
        ea: *const c_void,
        ea_length: u32,
    ) -> i32;
    fn RtlNtStatusToDosError(status: i32) -> u32;
}
pub(super) fn open(
    path: &Path,
    directory: bool,
    access: u32,
    sharing: u32,
    disposition: u32,
) -> io::Result<File> {
    let absolute = std::path::absolute(path)?;
    let dos: Vec<u16> = absolute.as_os_str().encode_wide().collect();
    let prefix: Vec<u16> = "\\??\\".encode_utf16().collect();
    let extended: Vec<u16> = "\\\\?\\".encode_utf16().collect();
    let unc: Vec<u16> = "\\\\".encode_utf16().collect();
    let mut name = prefix;
    if dos.starts_with(&extended) {
        name.extend_from_slice(&dos[4..]);
    } else if dos.starts_with(&unc) {
        name.extend("UNC\\".encode_utf16());
        name.extend_from_slice(&dos[2..]);
    } else {
        name.extend(dos);
    }
    if name.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Native path contains NUL",
        ));
    }
    let length = u16::try_from(
        name.len()
            .checked_mul(2)
            .ok_or_else(|| io::Error::other("Native path is too long"))?,
    )
    .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Native path is too long"))?;
    let mut unicode = UnicodeString {
        length,
        maximum_length: length,
        buffer: name.as_mut_ptr(),
    };
    let attributes = ObjectAttributes {
        length: std::mem::size_of::<ObjectAttributes>() as u32,
        root: std::ptr::null_mut(),
        name: &mut unicode,
        // OBJ_CASE_INSENSITIVE | OBJ_DONT_REPARSE, checked during native lookup.
        attributes: 0x40 | 0x1000,
        security: std::ptr::null_mut(),
        qos: std::ptr::null_mut(),
    };
    let mut status = IoStatusBlock {
        status: 0,
        information: 0,
    };
    let mut handle = std::ptr::null_mut();
    // FILE_SYNCHRONOUS_IO_NONALERT | FILE_OPEN_FOR_BACKUP_INTENT |
    // FILE_OPEN_REPARSE_POINT | FILE_DIRECTORY_FILE / FILE_NON_DIRECTORY_FILE.
    let options = 0x20 | 0x200000 | if directory { 0x4000 | 1 } else { 0x40 };
    // All pointers refer to live, correctly aligned buffers. Synchronous access
    // returns an owned handle; only successful NTSTATUS transfers ownership.
    let result = unsafe {
        NtCreateFile(
            &mut handle,
            access | 0x00100000,
            &attributes,
            &mut status,
            std::ptr::null(),
            0x80,
            sharing,
            disposition,
            options,
            std::ptr::null(),
            0,
        )
    };
    if result < 0 {
        return Err(io::Error::from_raw_os_error(
            unsafe { RtlNtStatusToDosError(result) } as i32,
        ));
    }
    let file = unsafe { File::from_raw_handle(handle) };
    // OPEN_REPARSE_POINT can return a final reparse object without following it.
    // Refuse that object too, including directories opened with FILE_OPEN_IF.
    if file.metadata()?.file_attributes() & 0x400 != 0 {
        return Err(io::Error::other("Native lookup refuses a reparse object"));
    }
    Ok(file)
}
