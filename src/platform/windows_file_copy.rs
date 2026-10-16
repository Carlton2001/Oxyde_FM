use std::fs::{File, OpenOptions};
use std::io::{self, ErrorKind};
use std::os::windows::io::FromRawHandle;
use std::os::windows::{ffi::OsStrExt, fs::OpenOptionsExt};
use std::path::Path;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{
    COPY_FILE_COPY_SYMLINK, COPY_FILE_FAIL_IF_EXISTS, COPYPROGRESSROUTINE_PROGRESS, CopyFileExW,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ, FlushFileBuffers,
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
        }
        (context.progress)(context.main_bytes)?;
        // Flush each completed stream while the native write handle is still
        // open, before CopyFileEx applies the final dates and readonly attribute.
        if stream_copied == stream_size {
            let _timer = crate::systems::copy_diagnostics::Timer::start(crate::systems::copy_diagnostics::Phase::Flush);
            unsafe { FlushFileBuffers(destination) }.map_err(io_error)?;
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
pub(super) fn copy(
    source: &Path,
    destination: &Path,
    held_source: Option<&std::fs::File>,
    destination_parent: &std::fs::File,
    created: &mut dyn FnMut(&File) -> io::Result<()>,
    progress: &mut dyn FnMut(u64) -> io::Result<()>,
) -> io::Result<()> {
    let _source_parents = crate::systems::path_parents::HeldParents::new(source)?;
    // Staging already retains this directory with DELETE access. Reopening its
    // parent chain without FILE_SHARE_DELETE would conflict with that handle.
    let metadata = destination_parent.metadata()?;
    if !metadata.is_dir() || super::platform().file_attributes(&metadata, "").2 {
        return Err(io::Error::other(
            "Copy destination parent is not an ordinary directory",
        ));
    }
    // Read access suffices for Copy. Deny source writes/deletion during the
    // native call without requiring DELETE access from the source's ACL.
    let source_guard = if held_source.is_none() {
        Some(
            OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ.0)
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
    let source = wide(&std::fs::canonicalize(source)?)?;
    let name = destination
        .file_name()
        .ok_or_else(|| io::Error::other("Copy destination has no name"))?;
    let destination = std::fs::canonicalize(
        destination
            .parent()
            .ok_or_else(|| io::Error::other("Copy destination has no parent"))?,
    )?
    .join(name);
    let target = wide(&destination)?;
    let mut context = Context {
        created,
        captured: false,
        progress,
        main_bytes: 0,
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
    result
}
