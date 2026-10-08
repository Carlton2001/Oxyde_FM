use std::fs::OpenOptions;
use std::io::{self, ErrorKind};
use std::os::windows::{ffi::OsStrExt, fs::OpenOptionsExt};
use std::path::Path;
use windows::core::PCWSTR;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{
    CopyFileExW, FlushFileBuffers, COPYPROGRESSROUTINE_PROGRESS, COPY_FILE_FAIL_IF_EXISTS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ, LPPROGRESS_ROUTINE_CALLBACK_REASON,
    PROGRESS_CONTINUE, PROGRESS_STOP,
};

fn io_error(error: windows::core::Error) -> io::Error {
    io::Error::from_raw_os_error((error.code().0 as u32 & 0xffff) as i32)
}
struct Context<'a> {
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
        if stream == 1 {
            context.main_bytes = stream_copied.max(0) as u64;
        }
        (context.progress)(context.main_bytes)?;
        // Flush each completed stream while the native write handle is still
        // open, before CopyFileEx applies the final dates and readonly attribute.
        if stream_copied == stream_size {
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
    progress: &mut dyn FnMut(u64) -> io::Result<()>,
) -> io::Result<()> {
    let _source_parents = crate::systems::path_parents::HeldParents::new(source)?;
    let _destination_parents = crate::systems::path_parents::HeldParents::new(destination)?;
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
            COPY_FILE_FAIL_IF_EXISTS,
        )
    }
    .map_err(io_error);
    if let Some(error) = context.error {
        return Err(error);
    }
    result
}
