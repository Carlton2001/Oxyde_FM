//! Atomic native namespace lookup: refuse reparse points in every component.
use std::{
    ffi::{c_void, OsStr},
    fs::File,
    io,
    os::windows::{
        ffi::OsStrExt,
        fs::MetadataExt,
        io::{AsRawHandle, FromRawHandle},
    },
    path::Path,
};

mod mounted;

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
    // `absolute` refuses a NUL left over.
    let absolute = std::path::absolute(crate::utils::path_text::decoded(path))?;
    match open_absolute(&absolute, directory, access, sharing, disposition) {
        Err(error) if refuses_reparse(&error) => match mounted::rebase(&absolute) {
            Some(rebased) => open_absolute(&rebased, directory, access, sharing, disposition),
            None => Err(error),
        },
        result => result,
    }
}

fn refuses_reparse(error: &io::Error) -> bool {
    error.raw_os_error() == Some(ERROR_REPARSE_POINT_ENCOUNTERED)
        || error.get_ref().is_some_and(|inner| inner.to_string() == REPARSE_OBJECT_REFUSED)
}

const REPARSE_OBJECT_REFUSED: &str = "Native lookup refuses a reparse object";

fn open_absolute(
    absolute: &Path,
    directory: bool,
    access: u32,
    sharing: u32,
    disposition: u32,
) -> io::Result<File> {
    // Windows 10 walks from the volume root directly, and so does a kernel
    // without OBJ_DONT_REPARSE, whose lookups must stay single components.
    if matches!(kernel(), Kernel::DriveLinksRefused | Kernel::NoDontReparse) {
        return open_by_components(absolute, directory, access, sharing, disposition);
    }
    // One native lookup elsewhere. Even where `\??\C:` is accepted (Windows 11),
    // other links can be refused (`\??\UNC`, mapped drives): a refusal may be
    // such a link or a real reparse point, and the walk decides.
    match open_name(native_name(absolute), std::ptr::null_mut(), Some(directory), access, sharing, disposition, true) {
        Err(error) if error.raw_os_error() == Some(ERROR_REPARSE_POINT_ENCOUNTERED) => {
            open_by_components(absolute, directory, access, sharing, disposition)
        }
        result => result,
    }
}

const ERROR_REPARSE_POINT_ENCOUNTERED: i32 = 4395;
const ERROR_INVALID_PARAMETER: i32 = 87;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kernel {
    /// Windows 11: `\??\C:` resolves under OBJ_DONT_REPARSE.
    DriveLinksAccepted,
    /// Windows 10 refuses the `\??\C:` object manager link under OBJ_DONT_REPARSE.
    DriveLinksRefused,
    /// Kernels older than OBJ_DONT_REPARSE reject the unknown attribute.
    NoDontReparse,
    Unknown,
}

/// A volume root is never a reparse point, so the lookup of the system
/// drive's root tells the kernel's behavior, probed once per process.
fn kernel() -> Kernel {
    static KERNEL: std::sync::OnceLock<Kernel> = std::sync::OnceLock::new();
    *KERNEL.get_or_init(|| {
        let drive = std::env::var_os("SystemDrive").unwrap_or_else(|| "C:".into());
        let mut root = std::ffi::OsString::from(&drive);
        root.push("\\");
        let root = std::path::PathBuf::from(root);
        // FILE_READ_ATTRIBUTES, all sharing, FILE_OPEN.
        match create_file(native_name(&root), std::ptr::null_mut(), Some(true), 0x80, 7, 1, true, 0x80) {
            Ok(_) => Kernel::DriveLinksAccepted,
            Err(error) => match error.raw_os_error() {
                Some(ERROR_REPARSE_POINT_ENCOUNTERED) => Kernel::DriveLinksRefused,
                Some(ERROR_INVALID_PARAMETER) => Kernel::NoDontReparse,
                _ => Kernel::Unknown,
            },
        }
    })
}

/// Opens the volume root alone, then refuses reparse points component by
/// component below it.
fn open_by_components(
    absolute: &Path,
    directory: bool,
    access: u32,
    sharing: u32,
    disposition: u32,
) -> io::Result<File> {
    let mut root = std::path::PathBuf::new();
    let mut names = Vec::new();
    for component in absolute.components() {
        match component {
            std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                root.push(component.as_os_str())
            }
            std::path::Component::Normal(name) => names.push(name),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Native path is not normalized",
                ))
            }
        }
    }
    let Some((last, parents)) = names.split_last() else {
        return open_name(native_name(&root), std::ptr::null_mut(), Some(directory), access, sharing, disposition, false);
    };
    // FILE_TRAVERSE | FILE_READ_ATTRIBUTES, all sharing.
    let mut parent = open_name(native_name(&root), std::ptr::null_mut(), Some(true), 0x20 | 0x80, 7, 1, false)?;
    for name in parents {
        parent = open_relative(&parent, name, true, 0x20 | 0x80, 7, 1)?;
    }
    open_relative(&parent, last, directory, access, sharing, disposition)
}

fn native_name(path: &Path) -> Vec<u16> {
    let dos: Vec<u16> = path.as_os_str().encode_wide().collect();
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
    name
}

pub(super) fn leaf_name(leaf: &OsStr) -> io::Result<Vec<u16>> {
    let name: Vec<u16> = leaf.encode_wide().collect();
    if name.is_empty()
        || name == [46]
        || name == [46, 46]
        || name.iter().any(|c| [0, 47, 92, 58].contains(c))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Operation requires a single file name",
        ));
    }
    Ok(name)
}

pub(super) fn open_relative(
    parent: &File,
    leaf: &OsStr,
    directory: bool,
    access: u32,
    sharing: u32,
    disposition: u32,
) -> io::Result<File> {
    open_name(
        leaf_name(leaf)?,
        parent.as_raw_handle(),
        Some(directory),
        access,
        sharing,
        disposition,
        true,
    )
}

/// The sole allowed alternate-stream name is supplied by the parent, never IPC.
pub(super) fn open_provenance_stream(parent: &File, leaf: &OsStr) -> io::Result<File> {
    let mut name = leaf_name(leaf)?;
    name.extend(":Zone.Identifier".encode_utf16());
    open_name(
        name,
        parent.as_raw_handle(),
        Some(false),
        windows::Win32::Foundation::GENERIC_READ.0 | windows::Win32::Foundation::GENERIC_WRITE.0,
        7,
        5,
        true,
    ) // all sharing; FILE_OVERWRITE_IF, confined to this stream.
}
pub(super) fn read_provenance_stream(parent: &File, leaf: &OsStr) -> io::Result<File> {
    let mut name = leaf_name(leaf)?;
    name.extend(":Zone.Identifier".encode_utf16());
    open_name(name, parent.as_raw_handle(), Some(false), windows::Win32::Foundation::GENERIC_READ.0, 7, 1, true)
}

/// Exclusively create one child of an already retained directory.
pub(super) fn create_relative(
    parent: &File,
    leaf: &OsStr,
    directory: bool,
    access: u32,
    sharing: u32,
    attributes: u32,
) -> io::Result<File> {
    open_name_with_attributes(
        leaf_name(leaf)?,
        parent.as_raw_handle(),
        Some(directory),
        access,
        sharing,
        2, // FILE_CREATE: never open or replace an existing object.
        true,
        attributes,
    )
    .map(|(file, _created)| file)
}

pub(super) fn ensure_directory_relative(parent: &File, leaf: &OsStr) -> io::Result<(File, bool)> {
    open_name_with_attributes(
        leaf_name(leaf)?,
        parent.as_raw_handle(),
        Some(true),
        windows::Win32::Foundation::GENERIC_READ.0,
        3, // FILE_SHARE_READ | FILE_SHARE_WRITE; deny parent replacement.
        3, // FILE_OPEN_IF; return whether this call actually created the child.
        true,
        0x80,
    )
}

/// A single child of a retained parent; OPEN_REPARSE_POINT opens the link itself.
pub(super) fn open_entry(
    parent: &File,
    leaf: &OsStr,
    access: u32,
    sharing: u32,
) -> io::Result<File> {
    open_name(
        leaf_name(leaf)?,
        parent.as_raw_handle(),
        None,
        access,
        sharing,
        1,
        false,
    )
}

fn open_name(
    name: Vec<u16>,
    root: *mut c_void,
    directory: Option<bool>,
    access: u32,
    sharing: u32,
    disposition: u32,
    reject_reparse: bool,
) -> io::Result<File> {
    open_name_with_attributes(
        name,
        root,
        directory,
        access,
        sharing,
        disposition,
        reject_reparse,
        0x80,
    )
    .map(|(file, _created)| file)
}

#[allow(clippy::too_many_arguments)]
fn open_name_with_attributes(
    name: Vec<u16>,
    root: *mut c_void,
    directory: Option<bool>,
    access: u32,
    sharing: u32,
    disposition: u32,
    reject_reparse: bool,
    file_attributes: u32,
) -> io::Result<(File, bool)> {
    // Without OBJ_DONT_REPARSE, `open` only looks up single components below
    // a retained parent: OPEN_REPARSE_POINT and the check below then refuse
    // the same objects.
    let dont_reparse = reject_reparse && kernel() != Kernel::NoDontReparse;
    let (file, created) = create_file(
        name,
        root,
        directory,
        access,
        sharing,
        disposition,
        dont_reparse,
        file_attributes,
    )?;
    // OPEN_REPARSE_POINT can return a final reparse object without following it.
    // Refuse that object too, including directories opened with FILE_OPEN_IF.
    if reject_reparse && file.metadata()?.file_attributes() & 0x400 != 0 {
        return Err(io::Error::other(REPARSE_OBJECT_REFUSED));
    }
    Ok((file, created))
}

#[allow(clippy::too_many_arguments)]
fn create_file(
    mut name: Vec<u16>,
    root: *mut c_void,
    directory: Option<bool>,
    access: u32,
    sharing: u32,
    disposition: u32,
    dont_reparse: bool,
    file_attributes: u32,
) -> io::Result<(File, bool)> {
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
        root,
        name: &mut unicode,
        // OBJ_CASE_INSENSITIVE | OBJ_DONT_REPARSE, checked during native lookup.
        attributes: 0x40 | if dont_reparse { 0x1000 } else { 0 },
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
    let options = 0x20
        | 0x200000
        | match directory {
            Some(true) => 0x4000 | 1,
            Some(false) => 0x40,
            None => 0x4000,
        };
    // All pointers refer to live, correctly aligned buffers. Synchronous access
    // returns an owned handle; only successful NTSTATUS transfers ownership.
    let result = unsafe {
        NtCreateFile(
            &mut handle,
            access | 0x00100000,
            &attributes,
            &mut status,
            std::ptr::null(),
            file_attributes,
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
    Ok((file, status.information == 2)) // FILE_CREATED, from this native call.
}

#[cfg(test)]
#[path = "windows_no_reparse/tests/drive_paths.rs"]
mod tests;
