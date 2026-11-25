//! Walk/delete retained objects without reopening a pathname or following links.
use std::ffi::{OsString, c_void};
use std::fs::File;
use std::io;
use std::os::windows::{ffi::OsStringExt, fs::MetadataExt, io::AsRawHandle};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex};
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
    names: Names,
}

/// Small: a deletion keeps one buffer per level of the folder being walked.
const WALK_BUFFER: usize = 4 * 1024;

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

/// The names of one directory handle, read a buffer of entries at a time: on
/// a share, one entry per query costs one round trip per entry.
struct Names {
    buffer: Vec<u32>,
    filled: usize,
    /// The next entry in `buffer`, `None` once it is spent.
    offset: Option<usize>,
    first: bool,
}

impl Names {
    fn new(bytes: usize) -> Self {
        Self { buffer: vec![0; bytes.div_ceil(4)], filled: 0, offset: None, first: true }
    }

    fn fill(&mut self, reader: &File) -> io::Result<bool> {
        let mut status = IoStatusBlock {
            status: 0,
            information: 0,
        };
        let capacity = std::mem::size_of_val(self.buffer.as_slice());
        let first = std::mem::replace(&mut self.first, false);
        let result = unsafe {
            NtQueryDirectoryFile(
                reader.as_raw_handle(),
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                &mut status,
                self.buffer.as_mut_ptr().cast(),
                capacity as u32,
                12,
                0,
                std::ptr::null(),
                first as u8,
            )
        };
        if end_of_listing(result, first) {
            return Ok(false);
        }
        if result < 0 {
            return Err(io::Error::from_raw_os_error(unsafe { RtlNtStatusToDosError(result) } as i32));
        }
        let size = status.information;
        if !(12..=capacity).contains(&size) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Incomplete directory enumeration",
            ));
        }
        self.filled = size;
        self.offset = Some(0);
        Ok(true)
    }

    fn next(&mut self, reader: &File) -> io::Result<Option<OsString>> {
        let offset = match self.offset {
            Some(offset) => offset,
            None => {
                if !self.fill(reader)? {
                    return Ok(None);
                }
                0
            }
        };
        // Aligned owned storage, bounded by the checked number of returned bytes.
        let bytes = unsafe { std::slice::from_raw_parts(self.buffer.as_ptr().cast::<u8>(), self.filled) };
        let entry = &bytes[offset..];
        let invalid = || io::Error::new(io::ErrorKind::InvalidData, "Invalid directory entry");
        if entry.len() < 12 {
            return Err(invalid());
        }
        let next = u32::from_le_bytes(entry[..4].try_into().unwrap()) as usize;
        let length = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
        if length == 0 || !length.is_multiple_of(2) || length > entry.len() - 12 {
            return Err(invalid());
        }
        // A following entry starts past this name and inside the returned bytes.
        if next != 0 && (next < 12 + length || next >= entry.len()) {
            return Err(invalid());
        }
        let name = OsString::from_wide(
            &entry[12..12 + length]
                .as_chunks::<2>().0.iter()
                .map(|pair| u16::from_le_bytes(*pair))
                .collect::<Vec<_>>(),
        );
        self.offset = (next != 0).then_some(offset + next);
        Ok(Some(name))
    }
}

/// STATUS_NO_MORE_FILES ends every listing. Redirectors, WinFsp or Dokan
/// answer the first query of an empty folder (without `.` and `..`) with
/// STATUS_NO_SUCH_FILE instead.
fn end_of_listing(status: i32, restart: bool) -> bool {
    const STATUS_NO_MORE_FILES: u32 = 0x8000_0006;
    const STATUS_NO_SUCH_FILE: u32 = 0xC000_000F;
    status as u32 == STATUS_NO_MORE_FILES || (restart && status as u32 == STATUS_NO_SUCH_FILE)
}

pub(super) fn enumerate(
    directory: &File,
    visitor: &mut dyn FnMut(OsString) -> io::Result<()>,
) -> io::Result<()> {
    let metadata = directory.metadata()?;
    if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
        return Err(io::Error::other("Source directory is no longer ordinary"));
    }
    let mut names = Names::new(64 * 1024);
    while let Some(name) = names.next(directory)? {
        if name != "." && name != ".." {
            visitor(name)?;
        }
    }
    Ok(())
}

pub(super) fn remove(
    parent: &File,
    path: &Path,
    workers: usize,
    checkpoint: &(dyn Fn() -> io::Result<()> + Sync),
) -> io::Result<()> {
    let leaf = path
        .file_name()
        .ok_or_else(|| io::Error::other("Deletion path has no file name"))?;
    let selected = open_for_delete(parent, leaf)?;
    let metadata = selected.metadata()?;
    // Do not enumerate a selected junction or symlink, even when it is a directory.
    if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
        checkpoint()?;
        return super::windows_file_moves::remove(&selected);
    }
    let reader = directory_reader(parent, leaf)?;
    let root = Directory { object: selected, reader, names: Names::new(WALK_BUFFER) };
    if workers <= 1 {
        return traverse(root, checkpoint);
    }
    remove_parallel(root, workers, checkpoint)
}

/// Turbo: the calling thread enumerates the selected folder and opens each
/// child itself; first-level subfolders, already retained, are handed to a
/// bounded pool and removed there. The selected folder is removed last.
fn remove_parallel(
    root: Directory,
    workers: usize,
    checkpoint: &(dyn Fn() -> io::Result<()> + Sync),
) -> io::Result<()> {
    let stop = AtomicBool::new(false);
    let failure = Mutex::new(None::<io::Error>);
    // Workers stop at their next checkpoint after any failure or cancellation.
    let guarded = || {
        if stop.load(Ordering::Relaxed) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "Deletion stopped"));
        }
        checkpoint()
    };
    let (sender, receiver) = mpsc::sync_channel::<Directory>(workers);
    let receiver = Mutex::new(receiver);
    let worker = || loop {
        let job = receiver.lock().unwrap_or_else(|p| p.into_inner()).recv();
        let Ok(directory) = job else { break };
        if stop.load(Ordering::Relaxed) {
            continue; // Release the queued handles undeleted.
        }
        if let Err(error) = traverse(directory, &guarded) {
            stop.store(true, Ordering::Relaxed);
            failure.lock().unwrap_or_else(|p| p.into_inner()).get_or_insert(error);
        }
    };
    let (object, result) = std::thread::scope(|scope| {
        // Workers start with the subfolders they take: deleting many folders
        // that hold only files starts no thread.
        let mut started = 0;
        let Directory { object, reader, mut names } = root;
        let result = traverse_children(&object, &reader, &mut names, &guarded, &mut |child| {
            if started < workers {
                scope.spawn(&worker);
                started += 1;
            }
            sender.send(child).map_err(|_| io::Error::other("Deletion workers stopped"))
        });
        if result.is_err() {
            stop.store(true, Ordering::Relaxed);
        }
        drop(sender);
        (object, result)
    });
    // Every worker has finished: report its first failure before the caller's.
    if let Some(error) = failure.into_inner().unwrap_or_else(|p| p.into_inner()) {
        if !matches!(&result, Err(own) if own.kind() != io::ErrorKind::Interrupted) {
            return Err(error);
        }
    }
    result?;
    checkpoint()?;
    super::windows_file_moves::remove(&object)
}

fn open_for_delete(parent: &File, leaf: &std::ffi::OsStr) -> io::Result<File> {
    super::windows_no_reparse::open_entry(
        parent,
        leaf,
        DELETE.0 | FILE_READ_ATTRIBUTES.0,
        FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0,
    )
}

/// Removes the direct children of `object`: files and links in place,
/// subfolders through `subfolder`. The directory itself is left in place.
fn traverse_children(
    object: &File,
    reader: &File,
    names: &mut Names,
    checkpoint: &(dyn Fn() -> io::Result<()> + Sync),
    subfolder: &mut dyn FnMut(Directory) -> io::Result<()>,
) -> io::Result<()> {
    loop {
        checkpoint()?;
        let Some(name) = names.next(reader)? else {
            return Ok(());
        };
        if name == "." || name == ".." {
            continue;
        }
        if let Some(directory) = remove_or_open(object, &name, checkpoint)? {
            subfolder(directory)?;
        }
    }
}

/// Removes a non-directory child, or returns a retained ordinary subfolder.
fn remove_or_open(
    parent: &File,
    name: &std::ffi::OsStr,
    checkpoint: &(dyn Fn() -> io::Result<()> + Sync),
) -> io::Result<Option<Directory>> {
    checkpoint()?;
    let child = match open_for_delete(parent, name) {
        Ok(child) => child,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let metadata = child.metadata()?;
    if metadata.is_dir() && metadata.file_attributes() & 0x400 == 0 {
        let reader = directory_reader(parent, name)?;
        return Ok(Some(Directory { object: child, reader, names: Names::new(WALK_BUFFER) }));
    }
    checkpoint()?;
    super::windows_file_moves::remove(&child)?;
    Ok(None)
}

fn traverse(
    root: Directory,
    checkpoint: &(dyn Fn() -> io::Result<()> + Sync),
) -> io::Result<()> {
    let mut stack = vec![root];
    // Stack space, buffers and handles scale with depth, not with the number
    // of children. No Rust recursion on untrusted trees.
    while let Some(directory) = stack.last_mut() {
        checkpoint()?;
        let Some(name) = directory.names.next(&directory.reader)? else {
            let directory = stack.pop().unwrap();
            drop(directory.reader);
            checkpoint()?;
            super::windows_file_moves::remove(&directory.object)?;
            continue;
        };
        if name == "." || name == ".." {
            continue;
        }
        if let Some(child) = remove_or_open(&directory.object, &name, checkpoint)? {
            stack.push(child);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "windows_file_tree/tests/listing_end.rs"]
mod tests;

#[cfg(test)]
#[path = "windows_file_tree/tests/listing.rs"]
mod listing_tests;
