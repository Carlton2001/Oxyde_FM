//! Walk/delete retained objects without reopening a pathname or following links.
use std::ffi::{OsString, c_void};
use std::fs::File;
use std::io;
use std::os::windows::{ffi::OsStringExt, fs::MetadataExt, io::AsRawHandle};
use std::path::Path;
use windows::Win32::Storage::FileSystem::{
    DELETE, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ,
    FILE_SHARE_WRITE,
};

#[repr(C)]
struct IoStatusBlock {
    status: usize,
    information: usize,
}
#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQueryDirectoryFile(
        file: *mut c_void,
        event: *mut c_void,
        apc: *const c_void,
        context: *const c_void,
        status: *mut IoStatusBlock,
        information: *mut c_void,
        length: u32,
        class: u32,
        single: u8,
        pattern: *const c_void,
        restart: u8,
    ) -> i32;
    fn RtlNtStatusToDosError(status: i32) -> u32;
}

struct Directory {
    object: File,
    reader: File,
}

fn directory_reader(parent: &File, leaf: &std::ffi::OsStr) -> io::Result<File> {
    // The mutation handle already denies directory renaming. Reopen only this
    // child of its retained parent for enumeration; reject a newly added link.
    super::windows_no_reparse::open_relative(
        parent,
        leaf,
        true,
        FILE_LIST_DIRECTORY.0 | FILE_READ_ATTRIBUTES.0,
        FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0 | FILE_SHARE_DELETE.0,
        1,
    )
}

fn next_name(reader: &File, buffer: &mut [u32], restart: bool) -> io::Result<Option<OsString>> {
    let mut status = IoStatusBlock {
        status: 0,
        information: 0,
    };
    let capacity = std::mem::size_of_val(buffer);
    let result = unsafe {
        NtQueryDirectoryFile(
            reader.as_raw_handle(),
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            &mut status,
            buffer.as_mut_ptr().cast(),
            capacity as u32,
            12,
            1,
            std::ptr::null(),
            restart as u8,
        )
    };
    if result < 0 {
        let error = unsafe { RtlNtStatusToDosError(result) };
        if error == 18 {
            return Ok(None);
        } // ERROR_NO_MORE_FILES
        return Err(io::Error::from_raw_os_error(error as i32));
    }
    let size = status.information;
    if !(12..=capacity).contains(&size) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Incomplete directory enumeration",
        ));
    }
    // Aligned owned storage, bounded by the checked number of returned bytes.
    let bytes = unsafe { std::slice::from_raw_parts(buffer.as_ptr().cast::<u8>(), size) };
    let next = u32::from_le_bytes(bytes[..4].try_into().unwrap());
    let length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if next != 0 || length == 0 || length % 2 != 0 || length > size - 12 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid directory entry",
        ));
    }
    let wide: Vec<_> = bytes[12..12 + length]
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    Ok(Some(OsString::from_wide(&wide)))
}

pub(super) fn enumerate(
    directory: &File,
    visitor: &mut dyn FnMut(OsString) -> io::Result<()>,
) -> io::Result<()> {
    let metadata = directory.metadata()?;
    if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
        return Err(io::Error::other("Source directory is no longer ordinary"));
    }
    let mut buffer = vec![0u32; (64 * 1024 + 16) / 4];
    let mut restart = true;
    while let Some(name) = next_name(directory, &mut buffer, restart)? {
        restart = false;
        if name != "." && name != ".." {
            visitor(name)?;
        }
    }
    Ok(())
}

pub(super) fn remove(
    parent: &File,
    path: &Path,
    checkpoint: &mut dyn FnMut() -> io::Result<()>,
) -> io::Result<()> {
    let leaf = path
        .file_name()
        .ok_or_else(|| io::Error::other("Deletion path has no file name"))?;
    let open = |parent: &File, leaf: &std::ffi::OsStr| {
        super::windows_no_reparse::open_entry(
            parent,
            leaf,
            DELETE.0 | FILE_READ_ATTRIBUTES.0,
            FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0,
        )
    };
    let selected = open(parent, leaf)?;
    let metadata = selected.metadata()?;
    // Do not enumerate a selected junction or symlink, even when it is a directory.
    if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
        checkpoint()?;
        return super::windows_file_moves::remove(&selected);
    }
    let reader = directory_reader(parent, leaf)?;
    let mut stack = vec![Directory {
        object: selected,
        reader,
    }];
    // One shared enumeration buffer; stack space and handles scale with depth,
    // not with the number of children. No Rust recursion on untrusted trees.
    let mut buffer = vec![0u32; (64 * 1024 + 16) / 4];
    while let Some(directory) = stack.last() {
        checkpoint()?;
        let Some(name) = next_name(&directory.reader, &mut buffer, false)? else {
            let directory = stack.pop().unwrap();
            drop(directory.reader);
            checkpoint()?;
            super::windows_file_moves::remove(&directory.object)?;
            continue;
        };
        if name == "." || name == ".." {
            continue;
        }
        checkpoint()?;
        let child = match open(&directory.object, &name) {
            Ok(child) => child,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let metadata = child.metadata()?;
        if metadata.is_dir() && metadata.file_attributes() & 0x400 == 0 {
            let reader = directory_reader(&directory.object, &name)?;
            stack.push(Directory {
                object: child,
                reader,
            });
        } else {
            checkpoint()?;
            super::windows_file_moves::remove(&child)?;
        }
    }
    Ok(())
}
