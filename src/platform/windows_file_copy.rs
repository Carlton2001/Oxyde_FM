use std::fs::{File, OpenOptions};
use std::io::{self, ErrorKind};
use std::os::windows::io::FromRawHandle;
use std::os::windows::{ffi::OsStrExt, fs::OpenOptionsExt};
use std::path::Path;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{
    COPY_FILE_COPY_SYMLINK, COPY_FILE_FAIL_IF_EXISTS, COPYPROGRESSROUTINE_PROGRESS, CopyFileExW,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ, FILE_SHARE_WRITE, FlushFileBuffers,
    LPPROGRESS_ROUTINE_CALLBACK_REASON, PROGRESS_CONTINUE, PROGRESS_STOP,
};
use windows::core::PCWSTR;

fn io_error(error: windows::core::Error) -> io::Error {
    io::Error::from_raw_os_error((error.code().0 as u32 & 0xffff) as i32)
}
struct Context<'a> {
    created: &'a mut dyn FnMut(&File) -> io::Result<()>,
    captured: bool,
    progress: &'a mut dyn FnMut(u64) -> io::Result<()>,
    main_bytes: u64,
    /// The main stream's writer once all its bytes are written, while no other
    /// stream has started: kept to finish the file if a secondary stream is refused.
    main_written: Option<File>,
    secondary_started: bool,
    error: Option<io::Error>,
}
unsafe extern "system" fn callback(
    _total: i64,
    _copied: i64,
    stream_size: i64,
    stream_copied: i64,
    stream: u32,
    _reason: LPPROGRESS_ROUTINE_CALLBACK_REASON,
    _source: HANDLE,
    destination: HANDLE,
    data: *const std::ffi::c_void,
) -> COPYPROGRESSROUTINE_PROGRESS {
    // CopyFileEx is synchronous; this exclusive context outlives every callback.
    let context = unsafe { &mut *data.cast_mut().cast::<Context<'_>>() };
    if context.error.is_some() {
        return PROGRESS_STOP;
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if !context.captured {
            // Borrow the native handle only during this callback. The observer
            // may duplicate it, but the OS copy remains its sole original owner.
            let borrowed =
                std::mem::ManuallyDrop::new(unsafe { File::from_raw_handle(destination.0) });
            (context.created)(&borrowed)?;
            context.captured = true;
        }
        if stream == 1 {
            context.main_bytes = stream_copied.max(0) as u64;
        } else {
            context.secondary_started = true;
        }
        (context.progress)(context.main_bytes)?;
        // Flush each completed stream while the native write handle is still
        // open, before CopyFileEx applies the final dates and readonly attribute.
        if stream_copied == stream_size {
            let _timer = crate::systems::copy_diagnostics::Timer::start(crate::systems::copy_diagnostics::Phase::Flush);
            unsafe { FlushFileBuffers(destination) }.map_err(io_error)?;
            if stream == 1 && context.main_written.is_none() {
                let borrowed =
                    std::mem::ManuallyDrop::new(unsafe { File::from_raw_handle(destination.0) });
                context.main_written = Some(borrowed.try_clone()?);
            }
        }
        Ok::<_, io::Error>(())
    }));
    match result {
        Ok(Ok(())) => PROGRESS_CONTINUE,
        Ok(Err(error)) => {
            context.error = Some(error);
            PROGRESS_STOP
        }
        Err(_) => {
            context.error = Some(io::Error::other("Copy progress callback panicked"));
            PROGRESS_STOP
        }
    }
}
fn wide(path: &Path) -> io::Result<Vec<u16>> {
    let mut result: Vec<_> = path.as_os_str().encode_wide().collect();
    if result.contains(&0) {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "Copy path contains NUL",
        ));
    }
    result.push(0);
    Ok(result)
}
/// The output CopyFileEx created by path lies in the held parent: an empty parent
/// may have become a junction between the caller's check and the creation.
pub(super) fn verify_child(parent: &File, file: &File, path: &Path) -> io::Result<()> {
    let folder = path
        .parent()
        .ok_or_else(|| io::Error::other("Copy destination has no parent"))?;
    let expected = super::windows_final_path::of_handle(parent, folder)?;
    let actual = super::windows_final_path::of_handle(file, path)?;
    if actual.parent() != Some(expected.as_path()) {
        return Err(io::Error::other(format!(
            "Copy output was created outside its folder: {}",
            actual.display()
        )));
    }
    Ok(())
}

#[repr(C)]
struct IoStatusBlock {
    status: usize,
    information: usize,
}
#[repr(C)]
#[derive(Default)]
struct ObjectBasicInformation {
    attributes: u32,
    granted_access: u32,
    reserved: [u32; 12],
}
#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQueryObject(handle: *mut std::ffi::c_void, class: u32, information: *mut ObjectBasicInformation, length: u32, returned: *mut u32) -> i32;
    fn NtQueryInformationFile(handle: *mut std::ffi::c_void, status: *mut IoStatusBlock, information: *mut u32, length: u32, class: u32) -> i32;
}

/// The creation handle CopyFileEx passes to its callback reads the output and shares
/// nothing, not even reading (measured on NTFS): verification reads through it rather
/// than reopen a file just written, which waits for the antivirus scan. Its I/O is
/// asynchronous, read by position only; an unbuffered one would need aligned reads.
pub(super) fn writer_verifies(writer: &File) -> bool {
    use std::os::windows::io::AsRawHandle;
    const FILE_READ_DATA: u32 = 0x1;
    const FILE_NO_INTERMEDIATE_BUFFERING: u32 = 0x8;
    let handle = writer.as_raw_handle();
    let mut basic = ObjectBasicInformation::default();
    // ObjectBasicInformation; FileModeInformation.
    let granted = unsafe {
        NtQueryObject(handle, 0, &mut basic, std::mem::size_of::<ObjectBasicInformation>() as u32, std::ptr::null_mut())
    } >= 0
        && basic.granted_access & FILE_READ_DATA != 0;
    let mut status = IoStatusBlock { status: 0, information: 0 };
    let mut mode = 0u32;
    granted
        && unsafe { NtQueryInformationFile(handle, &mut status, &mut mode, 4, 16) } >= 0
        && mode & FILE_NO_INTERMEDIATE_BUFFERING == 0
}

pub(super) fn copy(
    source: &Path,
    destination: &Path,
    held_source: Option<&std::fs::File>,
    destination_parent: &std::fs::File,
    created: &mut dyn FnMut(&File) -> io::Result<()>,
    progress: &mut dyn FnMut(u64) -> io::Result<()>,
) -> io::Result<()> {
    let _source_parents = crate::systems::path_parents::HeldParents::new(source)?;
    // The caller holds this directory and its parent chain. Reopening them
    // without FILE_SHARE_DELETE would conflict with those handles.
    let metadata = destination_parent.metadata()?;
    if !metadata.is_dir() || super::platform().file_attributes(&metadata, "").2 {
        return Err(io::Error::other(
            "Copy destination parent is not an ordinary directory",
        ));
    }
    // Read access suffices for Copy. Deny source deletion during the
    // native call without requiring DELETE access from the source's ACL.
    let source_guard = if held_source.is_none() {
        Some(
            OpenOptions::new()
                .access_mode(super::windows_source_selection::READ_CONTENTS)
                .share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0)
                .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
                .open(source)?,
        )
    } else {
        None
    };
    let source_file = held_source
        .or(source_guard.as_ref())
        .expect("copy source is held");
    let metadata = source_file.metadata()?;
    if !metadata.is_file() || super::platform().file_attributes(&metadata, "").2 {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "Copy source is not an ordinary file",
        ));
    }
    let source = wide(&super::windows_final_path::canonicalize(source)?)?;
    let name = destination
        .file_name()
        .ok_or_else(|| io::Error::other("Copy destination has no name"))?;
    let parent = destination
        .parent()
        .ok_or_else(|| io::Error::other("Copy destination has no parent"))?;
    let destination = super::windows_final_path::canonicalize(parent)?.join(name);
    let target = wide(&destination)?;
    let mut context = Context {
        created,
        captured: false,
        progress,
        main_bytes: 0,
        main_written: None,
        secondary_started: false,
        error: None,
    };
    let result = unsafe {
        CopyFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(target.as_ptr()),
            Some(callback),
            Some((&mut context as *mut Context<'_>).cast()),
            None,
            // Also reject a pre-existing dangling destination symlink, rather
            // than creating its target. A newly substituted source symlink is
            // copied itself and then rejected by the output's native lookup.
            COPY_FILE_FAIL_IF_EXISTS | COPY_FILE_COPY_SYMLINK,
        )
    }
    .map_err(io_error);
    if let Some(error) = context.error {
        return Err(error);
    }
    match (result, context.main_written) {
        // A drop folder lets its user create a file but not open it again to add
        // an alternate stream, such as the Mark of the Web: Explorer keeps the
        // file without it. The dates and attributes CopyFileEx would have applied
        // last are set through the main stream's writer.
        (Err(error), Some(written))
            if error.kind() == ErrorKind::PermissionDenied
                && !context.secondary_started
                && context.main_bytes == metadata.len() =>
        {
            log::warn!(
                "Alternate streams not copied to {}: {error}",
                destination.display()
            );
            finish_without_streams(&written, &metadata)
        }
        (result, _) => result,
    }
}

fn finish_without_streams(written: &File, source: &std::fs::Metadata) -> io::Result<()> {
    use std::os::windows::fs::FileTimesExt;
    let times = std::fs::FileTimes::new()
        .set_accessed(source.accessed()?)
        .set_modified(source.modified()?)
        .set_created(source.created()?);
    written.set_times(times)?;
    if source.permissions().readonly() {
        written.set_permissions(source.permissions())?;
    }
    Ok(())
}
