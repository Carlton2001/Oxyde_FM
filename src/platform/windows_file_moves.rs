use std::fs::File;
use std::io::{Error, ErrorKind, Result};
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use windows::Win32::Foundation::{GENERIC_READ, HANDLE};
use windows::Win32::Storage::FileSystem::{
    DELETE, FILE_BASIC_INFO, FILE_DISPOSITION_INFO, FILE_READ_ATTRIBUTES, FILE_RENAME_INFO,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_WRITE_ATTRIBUTES, FileBasicInfo,
    FileDispositionInfo, SetFileInformationByHandle,
};

#[repr(C)]
struct IoStatusBlock {
    status: usize,
    information: usize,
}
#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtSetInformationFile(
        file: *mut std::ffi::c_void,
        status: *mut IoStatusBlock,
        information: *const std::ffi::c_void,
        length: u32,
        class: u32,
    ) -> i32;
    fn RtlNtStatusToDosError(status: i32) -> u32;
}

fn io_error(error: windows::core::Error) -> Error {
    // These APIs report HRESULT_FROM_WIN32(GetLastError()).
    Error::from_raw_os_error((error.code().0 as u32 & 0xffff) as i32)
}

pub(super) fn open(path: &Path) -> Result<File> {
    crate::systems::path_parents::HeldParents::new(path)?.open_file(path)
}

pub(super) fn hold_parent(path: &Path) -> Result<File> {
    super::windows_no_reparse::open(
        path,
        true,
        GENERIC_READ.0,
        FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0,
        1,
    )
}

pub(super) fn ensure_directory(parent: &File, path: &Path) -> Result<(File, bool)> {
    let metadata = parent.metadata()?;
    if !metadata.is_dir() || super::platform().file_attributes(&metadata, "").2 {
        return Err(Error::other("Operation directory changed during creation"));
    }
    let leaf = path
        .file_name()
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "Directory has no file name"))?;
    super::windows_no_reparse::ensure_directory_relative(parent, leaf)
}

pub(super) fn track_directory(parent: &File, path: &Path) -> Result<File> {
    let leaf = path
        .file_name()
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "Directory has no file name"))?;
    // Metadata-only access permits the later DELETE open and ordinary file workers.
    // Keep this handle alive to prevent reuse of its identity, not to freeze its name.
    super::windows_no_reparse::open_relative(
        parent,
        leaf,
        true,
        FILE_READ_ATTRIBUTES.0,
        FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0 | FILE_SHARE_DELETE.0,
        1,
    )
}

pub(super) fn create_journal_directory(parent: &File, path: &Path, hidden: bool) -> Result<File> {
    let metadata = parent.metadata()?;
    if !metadata.is_dir() || super::platform().file_attributes(&metadata, "").2 {
        return Err(Error::other(
            "Journal parent is no longer an ordinary directory",
        ));
    }
    let leaf = path
        .file_name()
        .ok_or_else(|| Error::other("Directory has no file name"))?;
    super::windows_no_reparse::create_relative(
        parent,
        leaf,
        true,
        FILE_READ_ATTRIBUTES.0 | FILE_WRITE_ATTRIBUTES.0,
        FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0 | FILE_SHARE_DELETE.0,
        if hidden { 2 } else { 0x80 },
    )
}

pub(super) fn restore_journal_directory(
    directory: &File,
    permissions: std::fs::Permissions,
    hidden: bool,
) -> Result<()> {
    use std::os::windows::fs::MetadataExt;
    let metadata = directory.metadata()?;
    if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
        return Err(Error::other("Journal directory became a reparse object"));
    }
    // Zero timestamps leave them unchanged. Only restore the two attributes the
    // journal recorded, on the exact object returned by exclusive creation.
    let mut attributes = metadata.file_attributes() & !(1 | 2 | 0x80);
    if permissions.readonly() {
        attributes |= 1;
    }
    if hidden {
        attributes |= 2;
    }
    let info = FILE_BASIC_INFO {
        FileAttributes: attributes,
        ..Default::default()
    };
    unsafe {
        SetFileInformationByHandle(
            HANDLE(directory.as_raw_handle()),
            FileBasicInfo,
            (&info as *const FILE_BASIC_INFO).cast(),
            std::mem::size_of_val(&info) as u32,
        )
        .map_err(io_error)
    }
}

pub(super) fn remove_empty_directory(
    parent: &File,
    path: &Path,
    expected: super::FileIdentity,
) -> Result<()> {
    let leaf = path
        .file_name()
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "Directory has no file name"))?;
    let directory = super::windows_no_reparse::open_relative(
        parent,
        leaf,
        true,
        FILE_READ_ATTRIBUTES.0 | DELETE.0,
        FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0,
        1,
    )?;
    if super::windows_archive_source::identity(&directory)? != expected {
        return Err(Error::other(
            "Source directory changed; its replacement was preserved",
        ));
    }
    // No enumeration, permission changes, or recursive removal: Windows rejects
    // nonempty directories, including children arriving just before this call.
    remove(&directory)
}

pub(super) fn open_relative(parent: &File, path: &Path, read_contents: bool) -> Result<File> {
    let leaf = path
        .file_name()
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "Operation path has no file name"))?;
    super::windows_no_reparse::open_relative(
        parent,
        leaf,
        false,
        (if read_contents {
            GENERIC_READ.0
        } else {
            FILE_READ_ATTRIBUTES.0
        }) | DELETE.0,
        FILE_SHARE_READ.0,
        1,
    )
}

pub(super) fn open_copied_relative(parent: &File, path: &Path) -> Result<File> {
    let leaf = path
        .file_name()
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "Copy has no file name"))?;
    super::windows_no_reparse::open_relative(
        parent,
        leaf,
        false,
        GENERIC_READ.0 | DELETE.0 | FILE_WRITE_ATTRIBUTES.0,
        FILE_SHARE_READ.0,
        1,
    )
}

pub(super) fn rename(file: &File, destination: &Path) -> Result<()> {
    rename_with(file, destination, false)
}
pub(super) fn replace(file: &File, destination: &Path) -> Result<()> {
    rename_with(file, destination, true)
}
fn rename_with(file: &File, destination: &Path, replace: bool) -> Result<()> {
    if !destination.is_absolute() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "Move destination must be absolute",
        ));
    }
    let parents = crate::systems::path_parents::HeldParents::new(destination)?;
    rename_relative(file, parents.directory(), destination, replace)
}

pub(super) fn rename_relative(
    file: &File,
    parent: &File,
    destination: &Path,
    replace: bool,
) -> Result<()> {
    let leaf = destination
        .file_name()
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "Move destination has no file name"))?;
    let name = super::windows_no_reparse::leaf_name(leaf)?;
    let name_bytes = name
        .len()
        .checked_mul(2)
        .and_then(|size| u32::try_from(size).ok())
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "Move destination is too long"))?;
    let offset = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
    let size = std::mem::size_of::<FILE_RENAME_INFO>() + name_bytes as usize;
    let size_u32 = u32::try_from(size)
        .map_err(|_| Error::new(ErrorKind::InvalidInput, "Move destination is too long"))?;
    // Pointer-sized storage provides FILE_RENAME_INFO alignment, including its
    // variable-length UTF-16 tail. The extra terminator remains zero-filled.
    let mut storage = vec![0usize; size.div_ceil(std::mem::size_of::<usize>())];
    unsafe {
        let info = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        (*info).Anonymous.ReplaceIfExists = replace;
        (*info).RootDirectory = HANDLE(parent.as_raw_handle());
        (*info).FileNameLength = name_bytes;
        std::ptr::copy_nonoverlapping(
            name.as_ptr(),
            storage.as_mut_ptr().cast::<u8>().add(offset).cast::<u16>(),
            name.len(),
        );
        // Native FileRenameInformation accepts a retained RootDirectory. The
        // Win32 wrapper rejected that relative form on the tested Windows host.
        // FILE_RENAME_INFO has the same C layout as FILE_RENAME_INFORMATION.
        let mut status = IoStatusBlock {
            status: 0,
            information: 0,
        };
        let result =
            NtSetInformationFile(file.as_raw_handle(), &mut status, info.cast(), size_u32, 10);
        if result < 0 {
            Err(Error::from_raw_os_error(
                RtlNtStatusToDosError(result) as i32
            ))
        } else {
            Ok(())
        }
    }
}

pub(super) fn remove(file: &File) -> Result<()> {
    let info = FILE_DISPOSITION_INFO { DeleteFile: true };
    unsafe {
        SetFileInformationByHandle(
            HANDLE(file.as_raw_handle()),
            FileDispositionInfo,
            (&info as *const FILE_DISPOSITION_INFO).cast(),
            std::mem::size_of_val(&info) as u32,
        )
        .map_err(io_error)
    }
}
