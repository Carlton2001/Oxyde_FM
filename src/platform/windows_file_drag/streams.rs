//! Files a source provides as streams only (`FileGroupDescriptorW` and
//! `FileContents`): an Explorer ZIP folder, Outlook attachments and messages.
//! Entering reads the descriptor alone; the contents are written at Drop, on a
//! worker thread that reaches the source through the global interface table.
use super::data;
use crate::platform::file_drag::{DropStaging, StreamedFiles, StreamedSource};
use crate::utils::archive_security::{relative_archive_path, ExtractionRoot};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use windows::{
    core::*,
    Win32::{
        Foundation::*,
        Storage::FileSystem::FILE_ATTRIBUTE_DIRECTORY,
        System::Com::{StructuredStorage::*, *},
        System::Ole::*,
        UI::Shell::*,
    },
};

const MAX_ENTRIES: usize = 50_000;
const CHUNK: usize = 1024 * 1024;
const STAGING: &str = "oxyde-drops";
/// Staging folders left by a crash are removed by a later drop once this old.
const STALE: Duration = Duration::from_secs(24 * 60 * 60);
const CLSID_STD_GLOBAL_INTERFACE_TABLE: GUID = GUID::from_u128(0x00000323_0000_0000_c000_000000000046);

#[derive(Debug, PartialEq)]
pub(super) struct Entry {
    pub relative: PathBuf,
    pub directory: bool,
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
}

/// What a streamed source offers, named in a staging folder not created yet.
#[derive(Debug, PartialEq)]
pub(super) struct Listing {
    pub staging: PathBuf,
    pub entries: Vec<Entry>,
    /// Absolute top-level items, the paths the interface sees.
    pub top: Vec<String>,
}

pub(super) fn read_listing(object: &IDataObject) -> Option<Listing> {
    let id = data::registered(CFSTR_FILEDESCRIPTORW);
    if id == 0 {
        return None;
    }
    let mut storage = unsafe { object.GetData(&data::format(id)) }.ok()?;
    let bytes = data::read_global(&storage);
    unsafe { ReleaseStgMedium(&mut storage) };
    let entries = parse_descriptors(&bytes.ok()?)
        .map_err(|error| log::info!("Streamed drop refused: {error}"))
        .ok()?;
    let staging = crate::app_paths::temporary_dir()
        .ok()?
        .join(STAGING)
        .join(uuid::Uuid::new_v4().to_string());
    listing(staging, entries)
}

pub(super) fn listing(staging: PathBuf, entries: Vec<Entry>) -> Option<Listing> {
    let mut top: Vec<String> = Vec::new();
    for entry in &entries {
        let first = staging.join(entry.relative.components().next()?);
        let first = first.to_str()?.to_owned();
        if !top.contains(&first) {
            top.push(first);
        }
    }
    (!top.is_empty()).then_some(Listing { staging, entries, top })
}

/// `FILEGROUPDESCRIPTORW`: a count, then that many `FILEDESCRIPTORW`.
pub(super) fn parse_descriptors(bytes: &[u8]) -> std::result::Result<Vec<Entry>, String> {
    let size = std::mem::size_of::<FILEDESCRIPTORW>();
    let count = bytes
        .get(..4)
        .map(|count| u32::from_le_bytes(count.try_into().unwrap()) as usize)
        .ok_or("truncated descriptor")?;
    if count == 0 || count > MAX_ENTRIES || bytes.len() < 4 + count * size {
        return Err(format!("{count} descriptors in {} bytes", bytes.len()));
    }
    (0..count)
        .map(|index| {
            let offset = 4 + index * size;
            let descriptor = unsafe {
                std::ptr::read_unaligned(bytes[offset..offset + size].as_ptr().cast::<FILEDESCRIPTORW>())
            };
            // The structure is packed: copy the name before borrowing it.
            let file_name = descriptor.cFileName;
            let length = file_name.iter().position(|unit| *unit == 0).ok_or("unterminated name")?;
            let name = String::from_utf16(&file_name[..length]).map_err(|_| "invalid name")?;
            let relative = relative_archive_path(&name).map_err(|error| error.to_string())?;
            if relative.as_os_str().is_empty() {
                return Err(format!("empty name {name:?}"));
            }
            let flags = descriptor.dwFlags as i32;
            let directory = flags & FD_ATTRIBUTES.0 != 0
                && descriptor.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0;
            let size = (flags & FD_FILESIZE.0 != 0)
                .then(|| (u64::from(descriptor.nFileSizeHigh) << 32) | u64::from(descriptor.nFileSizeLow));
            let modified = (flags & FD_WRITESTIME.0 != 0).then(|| file_time(descriptor.ftLastWriteTime)).flatten();
            Ok(Entry { relative, directory, size, modified })
        })
        .collect()
}

fn file_time(time: FILETIME) -> Option<SystemTime> {
    // 100 ns intervals since 1601-01-01.
    let ticks = (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
    let unix = ticks.checked_sub(116_444_736_000_000_000)?;
    SystemTime::UNIX_EPOCH.checked_add(Duration::from_nanos(unix.checked_mul(100)?))
}

fn global_table() -> Result<IGlobalInterfaceTable> {
    unsafe { CoCreateInstance(&CLSID_STD_GLOBAL_INTERFACE_TABLE, None, CLSCTX_INPROC_SERVER) }
}

/// Registers the source for the worker; an asynchronous source is told the
/// transfer continues after Drop returns.
pub(super) fn streamed(object: &IDataObject, listing: Listing) -> Option<StreamedFiles> {
    let table = global_table().ok()?;
    let cookie = unsafe { table.RegisterInterfaceInGlobal(object, &IDataObject::IID) }.ok()?;
    let asynchronous = object.cast::<IDataObjectAsyncCapability>().ok().is_some_and(|capability| unsafe {
        capability.GetAsyncMode().is_ok_and(|mode| mode.as_bool()) && capability.StartOperation(None).is_ok()
    });
    Some(StreamedFiles(Box::new(Source { cookie, asynchronous, listing: Some(listing) })))
}

struct Source {
    cookie: u32,
    asynchronous: bool,
    listing: Option<Listing>,
}

impl StreamedSource for Source {
    fn extract(mut self: Box<Self>) -> std::result::Result<(Vec<String>, DropStaging), String> {
        let listing = self.listing.take().expect("streamed listing");
        let cookie = self.cookie;
        let asynchronous = self.asynchronous;
        // The worker owns its apartment, whatever the caller's thread uses.
        std::thread::Builder::new()
            .name("streamed-drop".into())
            .spawn(move || in_apartment(|| {
                let object = object(cookie)?;
                let result = write_all(&object, &listing);
                finish(&object, cookie, asynchronous, result.is_ok());
                result.map(|staging| (listing.top, staging))
            }))
            .map_err(|error| error.to_string())?
            .join()
            .map_err(|_| "Streamed drop worker panicked".to_string())?
    }
}

impl Drop for Source {
    /// Unused: release the source and tell it nothing was transferred.
    fn drop(&mut self) {
        if self.listing.is_none() {
            return;
        }
        let (cookie, asynchronous) = (self.cookie, self.asynchronous);
        let _ = std::thread::Builder::new().name("streamed-drop".into()).spawn(move || {
            let _ = in_apartment(|| {
                let object = object(cookie)?;
                finish(&object, cookie, asynchronous, false);
                Ok(())
            });
        });
    }
}

fn in_apartment<T>(work: impl FnOnce() -> std::result::Result<T, String>) -> std::result::Result<T, String> {
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
    let result = work();
    if initialized {
        unsafe { CoUninitialize() };
    }
    result
}

fn object(cookie: u32) -> std::result::Result<IDataObject, String> {
    let table = global_table().map_err(|error| error.to_string())?;
    let mut raw = std::ptr::null_mut();
    unsafe { table.GetInterfaceFromGlobal(cookie, &IDataObject::IID, &mut raw) }.map_err(|error| error.to_string())?;
    Ok(unsafe { IDataObject::from_raw(raw) })
}

fn finish(object: &IDataObject, cookie: u32, asynchronous: bool, copied: bool) {
    if asynchronous {
        if let Ok(capability) = object.cast::<IDataObjectAsyncCapability>() {
            let (result, effect) = if copied { (S_OK, DROPEFFECT_COPY) } else { (E_ABORT, DROPEFFECT_NONE) };
            let _ = unsafe { capability.EndOperation(result, None, effect.0) };
        }
    }
    if let Ok(table) = global_table() {
        let _ = unsafe { table.RevokeInterfaceFromGlobal(cookie) };
    }
}

fn write_all(object: &IDataObject, listing: &Listing) -> std::result::Result<DropStaging, String> {
    let parent = listing.staging.parent().ok_or("staging without parent")?;
    remove_stale(parent);
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    std::fs::create_dir(&listing.staging).map_err(|error| error.to_string())?;
    // From here on, a failure removes what was written.
    let staging = DropStaging(listing.staging.clone());
    let root = ExtractionRoot::new(&listing.staging).map_err(|error| error.to_string())?;
    let contents = data::registered(CFSTR_FILECONTENTS);
    let mut buffer = vec![0_u8; CHUNK];
    for (index, entry) in listing.entries.iter().enumerate() {
        let name = entry.relative.to_string_lossy();
        let destination = root.destination(&name, entry.directory).map_err(|error| error.to_string())?;
        if entry.directory {
            std::fs::create_dir_all(&destination.path).map_err(|error| error.to_string())?;
            continue;
        }
        write_entry(object, contents, index, entry, &destination.path, &mut buffer)
            .map_err(|error| format!("{}: {error}", entry.relative.display()))?;
    }
    Ok(staging)
}

fn write_entry(
    object: &IDataObject,
    contents: u16,
    index: usize,
    entry: &Entry,
    path: &Path,
    buffer: &mut [u8],
) -> Result<()> {
    let request = FORMATETC {
        cfFormat: contents,
        dwAspect: DVASPECT_CONTENT.0,
        lindex: index as i32,
        tymed: (TYMED_ISTREAM.0 | TYMED_HGLOBAL.0 | TYMED_ISTORAGE.0) as u32,
        ..Default::default()
    };
    let mut medium = unsafe { object.GetData(&request) }?;
    let result = write_medium(&medium, entry, path, buffer);
    unsafe { ReleaseStgMedium(&mut medium) };
    result?;
    if let Some(modified) = entry.modified {
        if let Ok(file) = std::fs::OpenOptions::new().write(true).open(path) {
            let _ = file.set_modified(modified);
        }
    }
    Ok(())
}

/// The staging folder is temporary and read back at once: nothing is synced.
fn write_medium(medium: &STGMEDIUM, entry: &Entry, path: &Path, buffer: &mut [u8]) -> Result<()> {
    let create = || std::fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(io_error);
    match TYMED(medium.tymed as i32) {
        TYMED_ISTREAM => {
            let stream = unsafe { medium.u.pstm.as_ref() }.ok_or_else(|| Error::from(E_POINTER))?;
            let mut file = create()?;
            loop {
                let mut read = 0_u32;
                unsafe { stream.Read(buffer.as_mut_ptr().cast(), buffer.len() as u32, Some(&mut read)) }.ok()?;
                if read == 0 {
                    break;
                }
                file.write_all(&buffer[..read as usize]).map_err(io_error)?;
            }
            Ok(())
        }
        TYMED_HGLOBAL => {
            let handle = unsafe { medium.u.hGlobal };
            let length = unsafe { windows::Win32::System::Memory::GlobalSize(handle) };
            let mut file = create()?;
            if length > 0 {
                let pointer = unsafe { windows::Win32::System::Memory::GlobalLock(handle) }.cast::<u8>();
                if pointer.is_null() {
                    return Err(Error::from_thread());
                }
                // A global block may be larger than the file it holds. Without
                // FD_FILESIZE, its size is all the source tells.
                let length = entry.size.map_or(length, |size| length.min(size as usize));
                let written = file.write_all(unsafe { std::slice::from_raw_parts(pointer, length) });
                let _ = unsafe { windows::Win32::System::Memory::GlobalUnlock(handle) };
                written.map_err(io_error)?;
            }
            Ok(())
        }
        TYMED_ISTORAGE => {
            // An Outlook message: a compound file.
            let source = unsafe { medium.u.pstg.as_ref() }.ok_or_else(|| Error::from(E_POINTER))?;
            let name = HSTRING::from(path.as_os_str());
            let target = unsafe {
                StgCreateDocfile(&name, STGM_READWRITE | STGM_SHARE_EXCLUSIVE | STGM_FAILIFTHERE, None)
            }?;
            unsafe { source.CopyTo(None, None, &target) }?;
            unsafe { target.Commit(STGC_DEFAULT.0 as u32) }
        }
        _ => Err(DV_E_TYMED.into()),
    }
}

fn io_error(error: std::io::Error) -> Error {
    Error::new(E_FAIL, error.to_string())
}

fn remove_stale(parent: &Path) {
    let Ok(children) = std::fs::read_dir(parent) else {
        return;
    };
    for child in children.flatten() {
        let stale = child
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age > STALE);
        if stale {
            let _ = std::fs::remove_dir_all(child.path());
        }
    }
}

#[cfg(test)]
#[path = "tests/streams.rs"]
mod tests;
